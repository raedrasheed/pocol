"""The explorer gateway's transaction index (pocol-transactions-v1): the Transactions tab of explorer.pocol.net
(/api/transactions.php, explorer_api.py) and pocol_getTransactions on the gateway's /rpc.

A background thread follows the gateway host's own node over its loopback RPC (the gateway's `query`, 127.0.0.1
only) and keeps every included transaction of the canonical chain in one SQLite file (POCOL_TX_DB): the block, its
position, the block time, from, to, value and a kind (Transfer, Contract call, Contract creation, Undecoded). The
file survives restarts; the index resumes from its tip after checking that the tip is still the node's block.

Reorganizations: every indexed height keeps its block hash and parent hash for the newest RECENT_BLOCKS blocks. Each
poll compares the stored tip with the node's block at that height; on a difference it walks back to the newest height
whose hash still matches (the fork point), removes everything above it in one transaction and indexes the new branch
from there. A block is only stored when its parent hash is the stored hash below it, and a full block is read by the
hash of the light block it extends (eth_getBlockByHash), so a reorganization during a read is detected, never stored.
With no common block in the kept window the index starts again from genesis (logged); PoCol's finality makes that a
fault, not a normal event.

Bounds: at most `max_rows` transactions (default 500,000, about 300 MB on disk with its indexes, plus room for the
WAL). Past it the oldest whole blocks are removed, and the answer's index.from names the first block whose
transactions are all still kept; the explorer then says the history starts there (a reorganization below it lowers it
to the fork point's next block). Node reads are paced (`pace` seconds between reads while catching up)
and back off after failures, so the index does not take the node's RPC from the gateway's own readers.

Pages: `page()` opens its own read-only connection (WAL mode: readers never wait for the writer) with a short busy
timeout and runs in the request's thread; the follower runs in a daemon thread with its own connection. The gateway
runs pages behind their own two slots (gateway.TX_SLOTS), not the node reads' slots, so pages never make the rest of
the explorer busy. A page's work is bounded rather than only timed: the newest and oldest orders walk the primary key,
and the value orders walk the covering index txs_value (the page's keys first, then its rows by primary key), so no
order sorts the whole table; a deep page skips index entries only. Measured on 500,000 rows (a synthetic index with this
schema): page 5,000 of 100 rows in any order under 0.15 s; a one-digit search prefix (about 3/16 of the rows match)
about 0.9 s, the slowest page; PAGE_SECONDS interrupts anything slower. Standard library only (Python 3.10+).
"""
import json
import logging
import re
import sqlite3
import threading
import time
from pathlib import Path

SCHEMA = 'pocol-transactions-v1'
DB_VERSION = 1
ORDERS = ('newest', 'oldest', 'value_desc', 'value_asc')
SIZES = (25, 50, 100)
PAGE_MAX = 1_000_000_000
MAX_ID = 9007199254740991
DEFAULT_MAX_ROWS = 500_000
RECENT_BLOCKS = 4096           # block hashes kept for fork detection and snapshot checks
MAX_TXS_PER_BLOCK = 100_000    # a node answer with more is refused as malformed
COMMIT_BLOCKS = 200            # blocks per write transaction while catching up
POLL_SECONDS = 2.0
PACE_SECONDS = 0.005
BACKOFF_MAX = 30.0
STALE_SECONDS = 30.0           # no node answer for this long: state "reconnecting"
PAGE_SECONDS = 2.0             # a page query running longer is interrupted (sqlite3 OperationalError: the relay's 503)
PROGRESS_STEPS = 10000         # SQLite virtual machine steps between two checks of that limit
IDENTITY_SECONDS = 60.0        # how often the follower checks the node's chain ID and genesis hash
KINDS = ('Transfer', 'Contract call', 'Contract creation', 'Undecoded')
HASH = re.compile(r'0x[0-9a-fA-F]{64}')
ADDRESS = re.compile(r'0x[0-9a-fA-F]{40}')
QUANTITY = re.compile(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,63})')
DATA = re.compile(r'0x(?:[0-9a-fA-F]{2})*')
QUERY = re.compile(r'0x[0-9a-f]{1,64}')
LOG = logging.getLogger('pocol_transactions')


class SnapshotChanged(RuntimeError):
    """The snapshot a page was asked at is no longer a block of the indexed canonical chain."""


class NodeAnswerInvalid(RuntimeError):
    """A node answer that is not a block or transaction of the expected shape: nothing of it is stored."""


def valid_options(options):
    """The pocol_getTransactions options explorer_api.transaction_options builds (and /rpc may send): exactly page
    (1..10^9), size (25, 50, 100), order, q ('' or a lower-case 0x prefix of 1..64 hex digits) and an optional
    snapshot {height, hash}; integers are not booleans."""
    if not isinstance(options, dict):
        return False
    keys = set(options)
    if not {'page', 'size', 'order', 'q'} <= keys <= {'page', 'size', 'order', 'q', 'snapshot'}:
        return False
    page, size, order, q = options['page'], options['size'], options['order'], options['q']
    if type(page) is not int or not 1 <= page <= PAGE_MAX or type(size) is not int or size not in SIZES:
        return False
    if not isinstance(order, str) or order not in ORDERS or not isinstance(q, str) or (q and not QUERY.fullmatch(q)):
        return False
    if 'snapshot' in options:
        snap = options['snapshot']
        if not isinstance(snap, dict) or set(snap) != {'height', 'hash'}:
            return False
        if type(snap['height']) is not int or not 0 <= snap['height'] <= MAX_ID:
            return False
        if not isinstance(snap['hash'], str) or not re.fullmatch(r'0x[0-9a-f]{64}', snap['hash']):
            return False
    return True


def _qty(value, what, bits=256):
    if not isinstance(value, str) or not QUANTITY.fullmatch(value):
        raise NodeAnswerInvalid('Malformed ' + what)
    number = int(value, 16)
    if number >> bits:
        raise NodeAnswerInvalid('Out of range ' + what)
    return number


def _hash(value, what):
    if not isinstance(value, str) or not HASH.fullmatch(value):
        raise NodeAnswerInvalid('Malformed ' + what)
    return value.lower()


def _address(value, what):
    if value is None:
        return None
    if not isinstance(value, str) or not ADDRESS.fullmatch(value):
        raise NodeAnswerInvalid('Malformed ' + what)
    return value.lower()


def light_block(value):
    """(height, hash, parent hash, timestamp, [transaction hashes]) of an eth_getBlockByNumber(n, false) answer."""
    if not isinstance(value, dict):
        raise NodeAnswerInvalid('Malformed block')
    txs = value.get('transactions')
    if not isinstance(txs, list) or len(txs) > MAX_TXS_PER_BLOCK:
        raise NodeAnswerInvalid('Malformed block transactions')
    return (_qty(value.get('number'), 'block number', 63), _hash(value.get('hash'), 'block hash'),
            _hash(value.get('parentHash'), 'parent hash'), _qty(value.get('timestamp'), 'block time', 63),
            [_hash(t, 'transaction hash') for t in txs])


def transaction_row(tx, height, block_hash, position, timestamp):
    """One full transaction object of block `height` as an index row. A transaction the node could not decode (it
    answers its raw bytes and no sender) is Undecoded, with from, to and value null."""
    if not isinstance(tx, dict):
        raise NodeAnswerInvalid('Malformed transaction')
    tx_hash = _hash(tx.get('hash'), 'transaction hash')
    if (_hash(tx.get('blockHash'), 'transaction block hash') != block_hash
            or _qty(tx.get('blockNumber'), 'transaction block number', 63) != height
            or _qty(tx.get('transactionIndex'), 'transaction position', 63) != position):
        raise NodeAnswerInvalid('Transaction does not belong to its block')
    if 'from' not in tx:
        if not isinstance(tx.get('raw'), str) or not DATA.fullmatch(tx['raw']):
            raise NodeAnswerInvalid('Malformed undecoded transaction')
        return (height, position, tx_hash, block_hash, timestamp, None, None, 'Undecoded', None, None)
    sender, to = _address(tx.get('from'), 'sender'), _address(tx.get('to'), 'recipient')
    if sender is None:
        raise NodeAnswerInvalid('Malformed sender')
    value = _qty(tx.get('value'), 'value')
    data = tx.get('input')
    if not isinstance(data, str) or not DATA.fullmatch(data):
        raise NodeAnswerInvalid('Malformed input')
    kind = 'Contract creation' if to is None else 'Contract call' if len(data) > 2 else 'Transfer'
    return (height, position, tx_hash, block_hash, timestamp, hex(value), value.to_bytes(32, 'big'), kind, sender, to)


SCHEMA_SQL = '''
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS blocks(height INTEGER PRIMARY KEY, hash TEXT NOT NULL, parent TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS txs(height INTEGER NOT NULL, position INTEGER NOT NULL, hash TEXT NOT NULL,
    block_hash TEXT NOT NULL, timestamp INTEGER NOT NULL, value TEXT, value_key BLOB, kind TEXT NOT NULL,
    sender TEXT, recipient TEXT, PRIMARY KEY(height, position)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS txs_hash ON txs(hash);
CREATE INDEX IF NOT EXISTS txs_sender ON txs(sender);
CREATE INDEX IF NOT EXISTS txs_recipient ON txs(recipient);
CREATE INDEX IF NOT EXISTS txs_value ON txs(value_key, height, position);
'''
ORDER_SQL = {
    'newest': 'height DESC, position DESC',
    'oldest': 'height ASC, position ASC',
}
# The value orders are the txs_value index's own order (forwards or backwards), so a page never sorts: valued rows by
# value, equal values by block and position in the same direction (highest value: newest first among equals; lowest
# value: oldest first), then the Undecoded rows (no value), newest first, in both orders.
VALUE_SQL = {'value_desc': 'value_key DESC, height DESC, position DESC',
             'value_asc': 'value_key ASC, height ASC, position ASC'}
UNDECODED_SQL = 'height DESC, position DESC'
COLUMNS = 'hash, block_hash, height, position, timestamp, value, kind, sender, recipient'


def _upper(prefix):
    """The smallest string above every string that starts with the lower-case hex `prefix` ('g' follows 'f')."""
    return prefix + 'g'


class TransactionIndex:
    """TransactionIndex(path, query, network=..): `query(method, params)` is one node read answering the JSON-RPC
    envelope (Gateway.query); `network` the operator's validated network (chain_id, genesis), stored in the file on
    first use and required to match afterwards, so a file of another network is refused rather than served."""

    def __init__(self, path, query, network=None, *, max_rows=DEFAULT_MAX_ROWS, poll=POLL_SECONDS, pace=PACE_SECONDS,
                 recent=RECENT_BLOCKS, commit_blocks=COMMIT_BLOCKS, clock=time.time):
        if network is None:
            raise ValueError('The transaction index needs the configured network')
        if type(max_rows) is not int or max_rows < 1:
            raise ValueError('max_rows must be a positive integer')
        self.path = str(Path(path))
        self.query = query
        self.chain_id, self.genesis = network.chain_id, network.genesis
        self.max_rows, self.poll, self.pace, self.recent, self.commit_blocks = max_rows, poll, pace, recent, commit_blocks
        self.clock = clock
        self.lock = threading.Lock()
        self.head = -1                 # the node's head height at the last answer
        self.last_contact = None       # clock() of the last node answer
        self.failing = False
        self.identity_checked = None
        self.stop_event = threading.Event()
        self.thread = None
        self.writer = self._open()
        try:
            self._check_identity_meta()
        except BaseException:
            self.writer.close()  # a refused file is not left open (Windows cannot move or delete an open file)
            raise

    # ---------------- storage ----------------
    def _open(self):
        conn = sqlite3.connect(self.path, timeout=5, isolation_level=None, check_same_thread=False)
        conn.execute('PRAGMA journal_mode=WAL')
        conn.execute('PRAGMA synchronous=NORMAL')
        conn.execute('PRAGMA cache_size=-8000')
        conn.executescript(SCHEMA_SQL)
        return conn

    def _meta(self, conn, key, default=None):
        row = conn.execute('SELECT value FROM meta WHERE key=?', (key,)).fetchone()
        return default if row is None else row[0]

    def _check_identity_meta(self):
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            stored = self._meta(conn, 'network')
            mine = json.dumps({'chainId': self.chain_id, 'genesisHash': self.genesis, 'version': DB_VERSION})
            if stored is None:
                conn.execute('INSERT INTO meta VALUES(?,?)', ('network', mine))
                conn.execute('INSERT OR REPLACE INTO meta VALUES(?,?)', ('from', '0'))
                conn.execute('INSERT OR REPLACE INTO meta VALUES(?,?)', ('rows', '0'))
            elif stored != mine:
                raise RuntimeError(f'{self.path} is the transaction index of another network or version; '
                                   'move it away to index this network')
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    def tip(self, conn=None):
        row = (conn or self.writer).execute('SELECT height, hash FROM blocks ORDER BY height DESC LIMIT 1').fetchone()
        return (row[0], row[1]) if row else None

    # ---------------- node reads ----------------
    def _read(self, method, params):
        answer = self.query(method, params)
        if not isinstance(answer, dict) or 'error' in answer or 'result' not in answer:
            raise RuntimeError('Node read failed: ' + method)
        with self.lock:
            self.last_contact = self.clock()
        return answer['result']

    def _block_at(self, height):
        value = self._read('eth_getBlockByNumber', [hex(height), False])
        if value is None:
            return None
        block = light_block(value)
        if block[0] != height:
            raise NodeAnswerInvalid('Block at another height')
        return block

    def _check_identity(self):
        now = self.clock()
        if self.identity_checked is not None and now - self.identity_checked < IDENTITY_SECONDS:
            return
        params = self._read('pocol_getParams', [])
        genesis = params.get('genesisHash') if isinstance(params, dict) else None
        if (not isinstance(params, dict) or type(params.get('chainId')) is not int or params['chainId'] != self.chain_id
                or not isinstance(genesis, str) or genesis.lower() != self.genesis):
            raise RuntimeError('The local node does not belong to the configured network')
        self.identity_checked = now

    # ---------------- following ----------------
    def start(self):
        if self.thread is None:
            self.thread = threading.Thread(target=self._run, name='pocol-transactions', daemon=True)
            self.thread.start()
        return self

    def stop(self, timeout=10):
        self.stop_event.set()
        if self.thread is not None:
            self.thread.join(timeout)

    def _run(self):
        backoff = self.poll
        while not self.stop_event.is_set():
            try:
                caught_up = self.step()
                with self.lock:
                    self.failing = False
                backoff = self.poll
                if caught_up:
                    self.stop_event.wait(self.poll)
            except Exception as error:  # the follower never ends: it retries with a growing pause
                with self.lock:
                    self.failing = True
                LOG.warning('transaction index: %s', error)
                self.stop_event.wait(backoff)
                backoff = min(BACKOFF_MAX, backoff * 2)

    def step(self, limit=None):
        """One round: the identity check, the fork check, then up to `limit` (default: until the head) new blocks.
        True when the index reached the node's head."""
        self._check_identity()
        head = self._block_at_latest()
        with self.lock:
            self.head = head[0]
        self._reconcile(head)
        tip = self.tip()
        next_height = 0 if tip is None else tip[0] + 1
        target = head[0] if limit is None else min(head[0], next_height + limit - 1)
        pending = []
        expected_parent = None if tip is None else tip[1]
        height = next_height
        while height <= target and not self.stop_event.is_set():
            block = self._block_at(height)
            if block is None:
                break
            if expected_parent is not None and block[2] != expected_parent:
                break  # the chain moved under this read: the next round finds the fork point
            rows = self._rows(block) if block[4] else []
            pending.append((block, rows))
            expected_parent = block[1]
            height += 1
            if len(pending) >= self.commit_blocks:
                self._store(pending)
                pending = []
            if self.pace:
                time.sleep(self.pace)
        if pending:
            self._store(pending)
        tip = self.tip()
        return tip is not None and tip[0] >= head[0]

    def _block_at_latest(self):
        value = self._read('eth_getBlockByNumber', ['latest', False])
        if value is None:
            raise RuntimeError('The node has no head block')
        return light_block(value)

    def _rows(self, block):
        height, block_hash, _, timestamp, hashes = block
        full = self._read('eth_getBlockByHash', [block_hash, True])
        if not isinstance(full, dict) or _hash(full.get('hash'), 'block hash') != block_hash:
            raise NodeAnswerInvalid('The full block is not the light block')
        txs = full.get('transactions')
        if not isinstance(txs, list) or len(txs) != len(hashes):
            raise NodeAnswerInvalid('The full block has other transactions')
        rows = [transaction_row(tx, height, block_hash, i, timestamp) for i, tx in enumerate(txs)]
        if [r[2] for r in rows] != hashes:
            raise NodeAnswerInvalid('The full block has other transactions')
        return rows

    def _reconcile(self, head):
        """Remove the indexed blocks that are no longer canonical (above the fork point)."""
        tip = self.tip()
        if tip is None:
            return
        height = min(tip[0], head[0])
        if height == tip[0]:
            node = head if head[0] == height else self._block_at(height)
            if node is not None and node[1] == tip[1]:
                return
        oldest = self.writer.execute('SELECT MIN(height) FROM blocks').fetchone()[0]
        fork = None
        while height >= oldest:
            stored = self.writer.execute('SELECT hash FROM blocks WHERE height=?', (height,)).fetchone()
            node = head if head[0] == height else self._block_at(height)
            if stored is not None and node is not None and node[1] == stored[0]:
                fork = height
                break
            height -= 1
        self._truncate(fork)

    def _truncate(self, fork):
        """Remove every block and transaction above `fork` (None: everything; the index starts again from genesis)."""
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            if fork is None:
                LOG.warning('transaction index: no common block with the node in the kept window; indexing again')
                conn.execute('DELETE FROM txs')
                conn.execute('DELETE FROM blocks')
                conn.execute("UPDATE meta SET value='0' WHERE key IN ('from','rows')")
            else:
                removed = conn.execute('DELETE FROM txs WHERE height>?', (fork,)).rowcount
                conn.execute('DELETE FROM blocks WHERE height>?', (fork,))
                rows = int(self._meta(conn, 'rows', '0')) - removed
                conn.execute("UPDATE meta SET value=? WHERE key='rows'", (str(rows),))
                # A fork point below the first whole block (older blocks were removed by the row bound): the new
                # branch is indexed from fork + 1, so every block from there on is whole again.
                first = int(self._meta(conn, 'from', '0'))
                if first > fork + 1:
                    conn.execute("UPDATE meta SET value=? WHERE key='from'", (str(fork + 1),))
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    def _store(self, pending):
        """The blocks and their rows in one write transaction, then the bounds (oldest whole blocks past max_rows,
        block hashes older than the kept window)."""
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            tip = self.tip(conn)
            if tip is not None and pending[0][0][2] != tip[1]:
                raise RuntimeError('The stored tip changed')
            added = 0
            for block, rows in pending:
                conn.execute('INSERT INTO blocks VALUES(?,?,?)', (block[0], block[1], block[2]))
                conn.executemany('INSERT INTO txs VALUES(?,?,?,?,?,?,?,?,?,?)', rows)
                added += len(rows)
            rows = int(self._meta(conn, 'rows', '0')) + added
            if rows > self.max_rows:
                over = conn.execute('SELECT height FROM txs ORDER BY height DESC, position DESC LIMIT 1 OFFSET ?',
                                    (self.max_rows,)).fetchone()
                if over is not None:
                    rows -= conn.execute('DELETE FROM txs WHERE height<=?', (over[0],)).rowcount
                    conn.execute("UPDATE meta SET value=? WHERE key='from'", (str(over[0] + 1),))
            conn.execute("UPDATE meta SET value=? WHERE key='rows'", (str(rows),))
            newest = pending[-1][0][0]
            conn.execute('DELETE FROM blocks WHERE height<=?', (newest - self.recent,))
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    # ---------------- answers ----------------
    def state(self):
        with self.lock:
            head, contact, failing = self.head, self.last_contact, self.failing
        return head, contact, failing

    @staticmethod
    def _value_page(conn, condition, args, order, size, offset, total, searched):
        """A value-order page: the keys of its rows from the covering index txs_value (valued rows, then Undecoded
        ones), then the rows by primary key. The Undecoded rows (value_key NULL, rare) are always read through that
        index; the valued rows too without a search (INDEXED BY), so the page skips index entries and never sorts. A
        search lets SQLite pick its prefix indexes and sort the matches (many only for a short prefix)."""
        null_rows = ' INDEXED BY txs_value WHERE value_key IS NULL AND '
        undecoded = conn.execute('SELECT COUNT(*) FROM txs' + null_rows + condition, args).fetchone()[0]
        valued = total - undecoded
        keys = []
        if offset < valued:
            keys = conn.execute('SELECT height, position FROM txs' + ('' if searched else ' INDEXED BY txs_value')
                                + ' WHERE value_key IS NOT NULL AND ' + condition + ' ORDER BY ' + VALUE_SQL[order]
                                + ' LIMIT ? OFFSET ?', args + [size, offset]).fetchall()
        if len(keys) < size and undecoded:
            keys += conn.execute('SELECT height, position FROM txs' + null_rows + condition + ' ORDER BY ' + UNDECODED_SQL
                                 + ' LIMIT ? OFFSET ?', args + [size - len(keys), max(0, offset - valued)]).fetchall()
        return [conn.execute('SELECT ' + COLUMNS + ' FROM txs WHERE height=? AND position=?', key).fetchone()
                for key in keys]

    def page(self, options):
        """The pocol-transactions-v1 page for `options` (valid_options). Raises SnapshotChanged when the asked
        snapshot is no longer an indexed canonical block."""
        if not valid_options(options):
            raise ValueError('Invalid transaction options')
        conn = sqlite3.connect(f'file:{self.path}?mode=ro', uri=True, timeout=0.5, isolation_level=None)
        try:
            conn.execute('PRAGMA query_only=1')
            deadline = time.monotonic() + PAGE_SECONDS
            conn.set_progress_handler(lambda: 1 if time.monotonic() > deadline else 0, PROGRESS_STEPS)
            conn.execute('BEGIN')  # one read snapshot for every query below
            tip = self.tip(conn)
            first = int(self._meta(conn, 'from', '0'))
            snapshot = options.get('snapshot')
            if snapshot is not None:
                row = conn.execute('SELECT hash FROM blocks WHERE height=?', (snapshot['height'],)).fetchone()
                if row is None or row[0] != snapshot['hash']:
                    raise SnapshotChanged('The snapshot is no longer an indexed canonical block')
                limit_height = snapshot['height']
            elif tip is not None:
                snapshot = {'height': tip[0], 'hash': tip[1]}
                limit_height = tip[0]
            else:
                limit_height = -1
            where, args = ['height<=?'], [limit_height]
            q = options['q']
            if q:
                parts, part_args = ['(hash>=? AND hash<?)'], [q, _upper(q)]
                if len(q) <= 42:
                    parts += ['(sender>=? AND sender<?)', '(recipient>=? AND recipient<?)']
                    part_args += [q, _upper(q), q, _upper(q)]
                where.append('(' + ' OR '.join(parts) + ')')
                args += part_args
            condition = ' AND '.join(where)
            total = conn.execute('SELECT COUNT(*) FROM txs WHERE ' + condition, args).fetchone()[0]
            size, offset, order = options['size'], (options['page'] - 1) * options['size'], options['order']
            if offset >= total:
                rows = []
            elif order in ORDER_SQL:
                rows = conn.execute('SELECT ' + COLUMNS + ' FROM txs WHERE ' + condition + ' ORDER BY ' + ORDER_SQL[order]
                                    + ' LIMIT ? OFFSET ?', args + [size, offset]).fetchall()
            else:
                rows = self._value_page(conn, condition, args, order, size, offset, total, bool(q))
            conn.execute('COMMIT')
        finally:
            conn.close()
        head, contact, failing = self.state()
        height = -1 if tip is None else tip[0]
        now = self.clock()
        if failing or contact is None or now - contact > STALE_SECONDS:
            state = 'reconnecting' if contact is not None or failing else 'syncing'
        else:
            state = 'ready' if height >= head >= 0 else 'syncing'
        return {
            'schema': SCHEMA, 'chainId': self.chain_id, 'genesisHash': self.genesis,
            'page': options['page'], 'size': size, 'order': options['order'], 'q': q, 'total': total,
            'index': {'height': height, 'head': max(head, height), 'from': first, 'updatedAt': int(contact or 0),
                      'state': state},
            'snapshot': snapshot,
            'rows': [{'hash': r[0], 'blockHash': r[1], 'blockNumber': hex(r[2]), 'transactionIndex': hex(r[3]),
                      'timestamp': hex(r[4]), 'value': r[5], 'kind': r[6], 'from': r[7], 'to': r[8]} for r in rows],
        }
