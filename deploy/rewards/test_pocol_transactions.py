"""The explorer gateway's transaction index (pocol_transactions.py) against an in-process fake node: following the
chain, the page answer the explorer bundle validates, ordering, search, snapshots, reorganizations (also during a
read and while the gateway was stopped), restarts, the row bound, a file of another network, malformed node answers,
node failures, and readers never waiting for the writer. The gateway and /api/transactions.php paths are at the end.
Loopback only; no node process."""
import json
import os
import re
import sqlite3
import sys
import tempfile
import threading
import time
import unittest
import unittest.mock
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pocol_transactions as T  # noqa: E402
import gateway  # noqa: E402
import explorer_api as api  # noqa: E402
import explorer_fakes as F  # noqa: E402
from pocol_rewards import validated_network  # noqa: E402

NETWORK = validated_network(F.NET)
ZERO = '0x' + '00' * 32
A1, A2, A3 = '0x' + '11' * 20, '0x' + '22' * 20, '0x' + 'AB' * 20
TIME0 = 1_800_000_000


def bhash(branch, n):
    return '0x' + f'{branch:02x}' + f'{n:062x}'


class FakeChain:
    """A canonical chain of light/full blocks; `txs[n]` lists block n's transaction objects (without the block fields).
    `fail` makes every read raise; `calls` counts reads by method; `on_read` runs before each read (tests move the
    chain there)."""

    def __init__(self, height=0, txs=None):
        self.branch = 1
        self.blocks = []
        self.txs = txs or {}
        self.fail = False
        self.calls = {}
        self.on_read = None
        self.extra = {}  # hash -> block of a non-canonical branch (eth_getBlockByHash still finds it)
        for n in range(height + 1):
            self.append()

    def tx_objects(self, n, block_hash):
        out = []
        for i, spec in enumerate(self.txs.get((self.branch, n), self.txs.get(n, []))):
            tx = {'hash': spec.get('hash') or '0x' + f'{self.branch:02x}{n:030x}{i:032x}', 'blockHash': block_hash,
                  'blockNumber': hex(n), 'transactionIndex': hex(i)}
            tx.update({k: v for k, v in spec.items() if k != 'hash'})
            out.append(tx)
        return out

    def append(self):
        n = len(self.blocks)
        h = bhash(self.branch, n)
        parent = self.blocks[-1]['hash'] if self.blocks else ZERO
        block = {'number': hex(n), 'hash': h, 'parentHash': parent, 'timestamp': hex(TIME0 + 2 * n)}
        block['full'] = self.tx_objects(n, h)
        block['transactions'] = [t['hash'] for t in block['full']]
        self.blocks.append(block)
        return block

    def reorg(self, fork, new_height, txs=None):
        """Replace every block above `fork` with a new branch up to `new_height`."""
        for b in self.blocks[fork + 1:]:
            self.extra[b['hash']] = b
        self.blocks = self.blocks[:fork + 1]
        self.branch += 1
        if txs:
            self.txs.update({(self.branch, n): v for n, v in txs.items()})
        while len(self.blocks) <= new_height:
            self.append()

    def query(self, method, params, deadline=None):
        self.calls[method] = self.calls.get(method, 0) + 1
        if self.on_read:
            self.on_read(method, params)
        if self.fail:
            raise OSError('node down')
        if method == 'pocol_getParams':
            return {'jsonrpc': '2.0', 'id': 1, 'result': {'chainId': F.NET['chainId'], 'genesisHash': F.NET['genesisHash']}}
        if method == 'eth_getBlockByNumber':
            tag = params[0]
            n = len(self.blocks) - 1 if tag == 'latest' else int(tag, 16)
            b = self.blocks[n] if 0 <= n < len(self.blocks) else None
            return {'jsonrpc': '2.0', 'id': 1, 'result': None if b is None else self.light(b)}
        if method == 'eth_getBlockByHash':
            b = next((b for b in self.blocks if b['hash'] == params[0]), self.extra.get(params[0]))
            if b is None:
                return {'jsonrpc': '2.0', 'id': 1, 'result': None}
            return {'jsonrpc': '2.0', 'id': 1, 'result': {**self.light(b), 'transactions': b['full']}}
        return {'jsonrpc': '2.0', 'id': 1, 'error': {'code': -32601, 'message': 'Method not found'}}

    @staticmethod
    def light(b):
        return {k: v for k, v in b.items() if k != 'full'}

    def canonical_rows(self):
        """(height, position, hash) of every canonical transaction, newest first."""
        rows = [(int(b['number'], 16), i, t) for b in self.blocks for i, t in enumerate(b['transactions'])]
        return sorted(rows, reverse=True)


def transfer(value=1, sender=A1, to=A2, data='0x'):
    return {'from': sender, 'to': to, 'value': hex(value), 'input': data, 'nonce': '0x0', 'gas': '0x5208'}


def options(**changes):
    value = {'page': 1, 'size': 25, 'order': 'newest', 'q': ''}
    value.update(changes)
    return value


def bundle_valid(data):
    """The explorer bundle's own check of a transactions answer (function M in assets/index-*.js), in Python."""
    safe = lambda v: type(v) is int and 0 <= v <= 9007199254740991
    hx = lambda v: isinstance(v, str) and re.fullmatch(r'0x[0-9a-f]{1,64}', v) is not None
    hh = lambda v: isinstance(v, str) and re.fullmatch(r'0x[0-9a-f]{64}', v) is not None
    if not (data.get('schema') == 'pocol-transactions-v1' and safe(data.get('total')) and safe(data.get('page'))
            and data['page'] >= 1 and data.get('size') in (25, 50, 100) and data.get('order') in T.ORDERS
            and isinstance(data.get('q'), str) and isinstance(data.get('rows'), list) and len(data['rows']) <= data['size']):
        return False
    index = data.get('index')
    if not (isinstance(index, dict) and type(index.get('height')) is int and index['height'] >= -1
            and type(index.get('head')) is int and index['head'] >= -1 and safe(index.get('updatedAt'))
            and index.get('state') in ('ready', 'syncing', 'reconnecting')):
        return False
    snap = data.get('snapshot')
    if snap is not None and not (isinstance(snap, dict) and safe(snap.get('height')) and hh(snap.get('hash'))):
        return False
    seen = set()
    for r in data['rows']:
        if not (hh(r['hash']) and hh(r['blockHash']) and hx(r['blockNumber']) and hx(r['transactionIndex'])
                and hx(r['timestamp']) and int(r['timestamp'], 16) <= 8640000000000 and (r['value'] is None or hx(r['value']))
                and r['hash'] not in seen and r['kind'] in T.KINDS and snap is not None
                and int(r['blockNumber'], 16) <= snap['height']
                and all(x is None or re.fullmatch(r'0x[0-9a-fA-F]{40}', x) for x in (r['from'], r['to']))):
            return False
        seen.add(r['hash'])
    return True


class Base(unittest.TestCase):
    def setUp(self):
        # Cleanups run last-in first-out, so the directory is removed after everything the test opened is closed
        # (its own addCleanup(conn.close) calls included): Windows cannot delete an open SQLite file (WinError 32),
        # so a test that leaves one open fails there.
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.path = os.path.join(self.dir.name, 'tx.sqlite3')
        self.now = [TIME0]
        opened = []
        real_open = T.TransactionIndex._open
        def recording_open(index):
            conn = real_open(index)
            opened.append((index, conn))
            return conn
        def close_opened():
            for index, conn in opened:
                index.stop()
                conn.close()
        self.addCleanup(close_opened)
        patcher = unittest.mock.patch.object(T.TransactionIndex, '_open', recording_open)
        patcher.start()
        self.addCleanup(patcher.stop)

    def index(self, chain, **kw):
        kw.setdefault('pace', 0)
        return T.TransactionIndex(self.path, chain.query, NETWORK, clock=lambda: self.now[0], **kw)

    def hashes(self, page):
        return [r['hash'] for r in page['rows']]

    def indexed(self, index):
        conn = sqlite3.connect(self.path)
        try:
            return conn.execute('SELECT height, position, hash FROM txs ORDER BY height DESC, position DESC').fetchall()
        finally:
            conn.close()


class OptionsTests(unittest.TestCase):
    def test_valid_options_is_exactly_the_relay_shape(self):
        self.assertTrue(T.valid_options(options()))
        self.assertTrue(T.valid_options(options(page=10 ** 9, size=100, order='value_asc', q='0x' + 'a' * 64,
                                                snapshot={'height': 0, 'hash': '0x' + 'ee' * 32})))
        bad = [None, [], options(page=0), options(page=10 ** 9 + 1), options(page=True), options(page=1.0),
               options(size=10), options(size=True), options(order='largest'), options(q='0xAB'), options(q='ab'),
               options(q='0x' + 'a' * 65), options(q=None), {**options(), 'extra': 1}, {'page': 1, 'size': 25, 'order': 'newest'},
               options(snapshot={'height': -1, 'hash': '0x' + 'ee' * 32}), options(snapshot={'height': 1, 'hash': '0x' + 'EE' * 32}),
               options(snapshot={'height': 1}), options(snapshot={'height': True, 'hash': '0x' + 'ee' * 32}),
               options(snapshot={'height': 1, 'hash': '0x' + 'ee' * 32, 'x': 1}), options(snapshot=None)]
        for value in bad:
            with self.subTest(value=value):
                self.assertFalse(T.valid_options(value))

    def test_every_relay_option_is_valid(self):
        for query in ('', 'page=3&size=50&order=value_desc&q=0xDEAD', 'height=12&hash=0x' + 'EE' * 32, 'size=100&order=oldest'):
            with self.subTest(query=query):
                self.assertTrue(T.valid_options(api.transaction_options(query)))


class FollowTests(Base):
    def chain(self):
        txs = {2: [transfer(5)], 3: [transfer(7, A2, A3), transfer(0, A1, A3, '0x1234'), {'from': A3, 'to': None, 'value': '0x0', 'input': '0x60'}],
               5: [{'raw': '0x02ff'}], 6: [{**transfer(2 ** 200), 'hash': '0xc0ffee' + '00' * 29}]}
        return FakeChain(6, txs)

    def test_follows_the_chain_newest_first_with_kinds_and_the_bundle_shape(self):
        chain = self.chain()
        index = self.index(chain)
        self.assertTrue(index.step())
        page = index.page(options())
        self.assertEqual([(int(r['blockNumber'], 16), int(r['transactionIndex'], 16), r['hash']) for r in page['rows']],
                         chain.canonical_rows())
        self.assertEqual(page['total'], 6)
        self.assertTrue(bundle_valid(page), page)
        self.assertEqual(page['snapshot'], {'height': 6, 'hash': bhash(1, 6)})
        self.assertEqual(page['index'], {'height': 6, 'head': 6, 'from': 0, 'updatedAt': TIME0, 'state': 'ready'})
        kinds = {int(r['blockNumber'], 16) * 10 + int(r['transactionIndex'], 16): r for r in page['rows']}
        self.assertEqual(kinds[20]['kind'], 'Transfer')
        self.assertEqual((kinds[31]['kind'], kinds[31]['to']), ('Contract call', A3.lower()))
        self.assertEqual((kinds[32]['kind'], kinds[32]['to']), ('Contract creation', None))
        self.assertEqual((kinds[50]['kind'], kinds[50]['from'], kinds[50]['to'], kinds[50]['value']), ('Undecoded', None, None, None))
        self.assertEqual(kinds[60]['value'], hex(2 ** 200))
        self.assertEqual(kinds[30]['timestamp'], hex(TIME0 + 6))
        # Light blocks for every height; a full block only where the light block lists transactions.
        self.assertEqual(chain.calls['eth_getBlockByHash'], 4)

    def test_orders_search_and_pages(self):
        chain = self.chain()
        index = self.index(chain)
        index.step()
        newest = self.hashes(index.page(options()))
        self.assertEqual(self.hashes(index.page(options(order='oldest'))), newest[::-1])
        values = [int(r['value'], 16) if r['value'] else None for r in index.page(options(order='value_desc'))['rows']]
        self.assertEqual(values, [2 ** 200, 7, 5, 0, 0, None])
        values = [int(r['value'], 16) if r['value'] else None for r in index.page(options(order='value_asc'))['rows']]
        self.assertEqual(values, [0, 0, 5, 7, 2 ** 200, None])
        # q: a prefix of a hash, a sender or a recipient (lower-case); an address-length prefix never matches hashes.
        found = index.page(options(q=A3.lower()[:12]))
        self.assertEqual(found['total'], 3)
        self.assertEqual(self.hashes(index.page(options(q='0xc0ffee'))), ['0xc0ffee' + '00' * 29])
        self.assertEqual(index.page(options(q=newest[2]))['total'], 1)
        self.assertEqual(index.page(options(q='0xfff'))['total'], 0)
        # Pages of 25 over 60 transactions.
        chain2 = FakeChain(30, {n: [transfer(n), transfer(n + 1)] for n in range(1, 31)})
        os.remove(self.path)
        index = self.index(chain2)
        index.step()
        pages = [index.page(options(page=p)) for p in (1, 2, 3, 4)]
        self.assertEqual([len(p['rows']) for p in pages], [25, 25, 10, 0])
        self.assertEqual(sum((self.hashes(p) for p in pages), []), [r[2] for r in chain2.canonical_rows()])
        self.assertTrue(all(bundle_valid(p) for p in pages))

    def test_value_orders_page_through_valued_then_undecoded_rows(self):
        # 40 blocks: values with ties, an Undecoded transaction every fifth block, pages of 25 across the boundary
        # between the valued and the Undecoded rows, with and without a search.
        txs = {n: [transfer(n % 7, A1 if n % 2 else A2, A3)] + ([{'raw': '0x02'}] if n % 5 == 0 else []) for n in range(1, 41)}
        chain = FakeChain(40, txs)
        index = self.index(chain)
        index.step()
        rows = [(int(r['value'], 16) if r['value'] else None, int(r['blockNumber'], 16), int(r['transactionIndex'], 16), r['hash'])
                for p in (1, 2, 3) for r in index.page(options(size=25, page=p))['rows']]
        self.assertEqual(len(rows), 48)
        valued, undecoded = [r for r in rows if r[0] is not None], [r for r in rows if r[0] is None]
        undecoded.sort(key=lambda r: (r[1], r[2]), reverse=True)
        expected = {'value_desc': sorted(valued, key=lambda r: r[:3], reverse=True) + undecoded,
                    'value_asc': sorted(valued, key=lambda r: r[:3]) + undecoded}
        for order, want in expected.items():
            for q in ('', A1.lower()[:5]):
                with self.subTest(order=order, q=q):
                    want_q = [r[3] for r in want if not q or r[3].startswith(q) or (r[0] is not None and r[1] % 2)]
                    pages = [index.page(options(size=25, page=p, order=order, q=q)) for p in (1, 2, 3)]
                    self.assertEqual(sum((self.hashes(p) for p in pages), []), want_q)
                    self.assertTrue(all(p['total'] == len(want_q) and bundle_valid(p) for p in pages))

    def test_no_page_sorts_the_table_and_deep_value_pages_skip_index_entries_only(self):
        """Every statement a page runs without a search reads the primary key or an index in its own order: none
        sorts (no temporary B-tree), so a deep page of a value order costs a walk of the covering index, not a sort of
        every row (the reviewed denial of service: 2 s per page at 500,000 rows)."""
        chain = FakeChain(60, {n: [transfer(n * 13 % 17), transfer(n % 3, A2, A3)] + ([{'raw': '0x02'}] if n % 9 == 0 else []) for n in range(1, 61)})
        index = self.index(chain)
        index.step()
        traced = []
        real = sqlite3.connect

        def connect(*args, **kwargs):
            conn = real(*args, **kwargs)
            conn.set_trace_callback(traced.append)
            return conn
        plans = sqlite3.connect(self.path)
        self.addCleanup(plans.close)
        for order in T.ORDERS:
            for page in (1, 2, 5, 10 ** 9):
                with self.subTest(order=order, page=page):
                    traced.clear()
                    with unittest.mock.patch.object(T.sqlite3, 'connect', connect):
                        index.page(options(order=order, page=page, size=25))
                    selects = [sql for sql in traced if sql.lstrip().upper().startswith('SELECT')]
                    self.assertTrue(selects)
                    for sql in selects:
                        plan = ' | '.join(row[-1] for row in plans.execute('EXPLAIN QUERY PLAN ' + sql))
                        self.assertNotIn('TEMP B-TREE', plan, sql)
                        if order.startswith('value') and 'FROM txs' in sql and 'ORDER BY' in sql:
                            self.assertIn('COVERING INDEX txs_value', plan, sql)

    def test_a_snapshot_keeps_pages_stable_and_a_moved_snapshot_is_refused(self):
        chain = FakeChain(10, {n: [transfer(n)] for n in range(1, 11)})
        index = self.index(chain)
        index.step()
        first = index.page(options(size=25))
        snap = first['snapshot']
        chain.txs.update({n: [transfer(n)] for n in range(11, 14)})
        for _ in range(3):
            chain.append()
        index.step()
        self.assertEqual(index.page(options())['total'], 13)
        pinned = index.page(options(snapshot=snap))
        self.assertEqual((pinned['total'], pinned['snapshot']), (10, snap))
        self.assertEqual(self.hashes(pinned), self.hashes(first))
        with self.assertRaises(T.SnapshotChanged):
            index.page(options(snapshot={'height': snap['height'], 'hash': '0x' + 'ee' * 32}))
        with self.assertRaises(T.SnapshotChanged):
            index.page(options(snapshot={'height': 99, 'hash': snap['hash']}))
        chain.reorg(8, 14)
        index.step()
        with self.assertRaises(T.SnapshotChanged):
            index.page(options(snapshot=snap))

    def test_an_empty_index_answers_no_snapshot(self):
        chain = FakeChain(3)
        index = self.index(chain)
        page = index.page(options())
        self.assertEqual((page['rows'], page['snapshot'], page['total'], page['index']['height'], page['index']['state']),
                         ([], None, 0, -1, 'syncing'))
        self.assertTrue(bundle_valid(page))
        index.step(limit=2)
        page = index.page(options())
        self.assertEqual((page['index']['height'], page['index']['head'], page['index']['state']), (1, 3, 'syncing'))
        index.step()
        self.assertEqual(index.page(options())['index']['state'], 'ready')


class ReorgTests(Base):
    def test_a_reorganization_replaces_the_blocks_above_the_fork_point(self):
        chain = FakeChain(8, {n: [transfer(n)] for n in range(1, 9)})
        index = self.index(chain)
        index.step()
        old = self.indexed(index)
        chain.reorg(5, 9, {6: [transfer(600), transfer(601)], 9: [transfer(900)]})
        index.step()
        self.assertEqual(self.indexed(index), chain.canonical_rows())
        self.assertNotEqual(self.indexed(index), old)
        page = index.page(options())
        self.assertEqual(page['total'], len(chain.canonical_rows()))
        self.assertEqual(page['snapshot'], {'height': 9, 'hash': bhash(2, 9)})
        self.assertTrue(all(r['blockHash'] == bhash(2, int(r['blockNumber'], 16)) for r in page['rows'] if int(r['blockNumber'], 16) > 5))

    def test_a_shorter_new_branch_and_a_reorganization_to_a_lower_head(self):
        chain = FakeChain(8, {n: [transfer(n)] for n in range(1, 9)})
        index = self.index(chain)
        index.step()
        chain.reorg(3, 5, {4: [transfer(40)]})
        index.step()
        self.assertEqual(self.indexed(index), chain.canonical_rows())
        self.assertEqual(index.page(options())['snapshot'], {'height': 5, 'hash': bhash(2, 5)})

    def test_a_reorganization_during_the_read_is_repaired_on_the_next_round(self):
        chain = FakeChain(4, {n: [transfer(n)] for n in range(1, 13)})
        index = self.index(chain)
        index.step()
        for _ in range(6):
            chain.append()
        moved = []

        def move(method, params):
            # The chain reorganizes below the reader while it reads height 7.
            if method == 'eth_getBlockByNumber' and params[0] == hex(7) and not moved:
                moved.append(1)
                chain.reorg(5, 12, {6: [transfer(66)], 7: [transfer(77)]})
        chain.on_read = move
        index.step()
        chain.on_read = None
        index.step()
        self.assertEqual(self.indexed(index), chain.canonical_rows())
        conn = sqlite3.connect(self.path)
        blocks = dict(conn.execute('SELECT height, hash FROM blocks').fetchall())
        conn.close()
        self.assertEqual(blocks, {int(b['number'], 16): b['hash'] for b in chain.blocks})

    def test_a_full_block_that_is_not_the_light_block_is_refused(self):
        chain = FakeChain(3, {2: [transfer(1)]})
        index = self.index(chain)
        real = chain.query

        def swapped(method, params, deadline=None):
            answer = real(method, params)
            if method == 'eth_getBlockByHash' and answer['result']:
                answer['result']['transactions'] = [dict(answer['result']['transactions'][0], hash='0x' + '99' * 32)]
            return answer
        index.query = swapped
        with self.assertRaises(T.NodeAnswerInvalid):
            index.step()
        self.assertIsNone(index.tip(), 'nothing of the failed round is stored')
        self.assertEqual(self.indexed(index), [])

    def test_no_common_block_in_the_kept_window_indexes_again_from_genesis(self):
        chain = FakeChain(20, {n: [transfer(n)] for n in range(1, 21)})
        index = self.index(chain, recent=4)
        index.step()
        chain.reorg(2, 22, {n: [transfer(1000 + n)] for n in range(3, 23)})
        with self.assertLogs('pocol_transactions', 'WARNING'):
            index.step()
        index.step()
        self.assertEqual(self.indexed(index), chain.canonical_rows())


class RestartAndBoundTests(Base):
    def test_a_restart_resumes_from_the_tip_and_detects_a_reorganization_while_stopped(self):
        chain = FakeChain(6, {n: [transfer(n)] for n in range(1, 7)})
        index = self.index(chain)
        index.step()
        index.writer.close()
        before = chain.calls.get('eth_getBlockByHash', 0)
        for _ in range(2):
            chain.append()
        again = self.index(chain)
        again.step()
        self.assertEqual(self.indexed(again), chain.canonical_rows())
        self.assertEqual(chain.calls['eth_getBlockByHash'] - before, 0, 'nothing below the tip is read again')
        again.writer.close()
        chain.reorg(4, 9, {5: [transfer(55)]})
        third = self.index(chain)
        third.step()
        self.assertEqual(self.indexed(third), chain.canonical_rows())
        self.assertEqual(third.page(options())['total'], len(chain.canonical_rows()))

    def test_the_file_of_another_network_is_refused(self):
        self.index(FakeChain(1)).writer.close()
        other = validated_network({**F.NET, 'genesisHash': '0x' + '5b' * 32})
        real_open, refused = T.TransactionIndex._open, []
        with unittest.mock.patch.object(T.TransactionIndex, '_open', lambda index: refused.append(real_open(index)) or refused[-1]):
            with self.assertRaisesRegex(RuntimeError, 'another network'):
                T.TransactionIndex(self.path, FakeChain(1).query, other)
        with self.assertRaises(sqlite3.ProgrammingError, msg='the refused file is closed, not left open'):
            refused[0].execute('SELECT 1')
        with self.assertRaises(ValueError):
            T.TransactionIndex(self.path, FakeChain(1).query)

    def test_the_node_must_name_the_configured_network(self):
        chain = FakeChain(2)
        real = chain.query
        chain.query = lambda m, p, deadline=None: ({'jsonrpc': '2.0', 'id': 1, 'result': {'chainId': 1, 'genesisHash': F.NET['genesisHash']}}
                                                    if m == 'pocol_getParams' else real(m, p))
        index = T.TransactionIndex(self.path, chain.query, NETWORK, pace=0, clock=lambda: self.now[0])
        with self.assertRaisesRegex(RuntimeError, 'configured network'):
            index.step()
        self.assertIsNone(index.tip())

    def test_the_row_bound_removes_the_oldest_whole_blocks_and_names_the_first_kept_block(self):
        chain = FakeChain(20, {n: [transfer(n), transfer(n)] for n in range(1, 21)})
        index = self.index(chain, max_rows=15, commit_blocks=3)
        index.step()
        page = index.page(options(size=100))
        self.assertLessEqual(page['total'], 15)
        first = page['index']['from']
        heights = {int(r['blockNumber'], 16) for r in page['rows']}
        self.assertEqual(heights, set(range(first, 21)), 'every block from index.from on is whole')
        self.assertGreater(first, 1)
        conn = sqlite3.connect(self.path)
        rows = conn.execute("SELECT value FROM meta WHERE key='rows'").fetchone()[0]
        conn.close()
        self.assertEqual(int(rows), page['total'])

    def test_a_reorganization_below_the_first_kept_block_lowers_index_from(self):
        # The row bound removed blocks up to 15 (index.from 16); then the chain reorganizes at 12. The new branch is
        # indexed from 13, so index.from must say 13, never a height above the rows the page lists.
        chain = FakeChain(20, {n: [transfer(n)] for n in range(1, 21)})
        index = self.index(chain, max_rows=5)
        index.step()
        page = index.page(options())
        self.assertEqual((page['index']['from'], page['index']['height']), (16, 20))
        chain.reorg(12, 13, {13: [transfer(1300)]})
        index.step()
        page = index.page(options())
        heights = [int(r['blockNumber'], 16) for r in page['rows']]
        self.assertEqual((heights, page['index']['height']), ([13], 13))
        self.assertEqual(page['index']['from'], 13)
        self.assertLessEqual(page['index']['from'], min(heights))
        # Following the new branch past the bound moves index.from up again.
        chain.txs.update({n: [transfer(n)] for n in range(14, 22)})
        for _ in range(8):
            chain.append()
        index.step()
        page = index.page(options())
        self.assertEqual(page['index']['from'], min(int(r['blockNumber'], 16) for r in page['rows']))
        self.assertEqual(page['total'], 5)

    def test_block_hashes_are_kept_for_the_recent_window_only(self):
        chain = FakeChain(30)
        index = self.index(chain, recent=8)
        index.step()
        conn = sqlite3.connect(self.path)
        heights = [h for (h,) in conn.execute('SELECT height FROM blocks ORDER BY height')]
        conn.close()
        self.assertEqual(heights, list(range(23, 31)))


class FailureTests(Base):
    def test_malformed_node_answers_store_nothing(self):
        for bad in ({'from': 'x'}, {'from': A1, 'to': A2, 'value': '1', 'input': '0x'}, {'from': A1, 'to': A2, 'value': '0x1', 'input': '0x1'},
                    {'from': A1, 'to': A2, 'value': '0x01', 'input': '0x'}, {'raw': 5}, {'from': None, 'to': A2, 'value': '0x0', 'input': '0x'}):
            with self.subTest(bad=bad):
                if os.path.exists(self.path):
                    os.remove(self.path)
                chain = FakeChain(3, {2: [bad]})
                index = self.index(chain)
                with self.assertRaises(T.NodeAnswerInvalid):
                    index.step()
                self.assertEqual(self.indexed(index), [])
                index.writer.close()

    def test_node_failures_are_reported_and_the_last_index_is_still_served(self):
        chain = FakeChain(3, {2: [transfer(1)]})
        index = self.index(chain, poll=0.01)
        index.step()
        chain.fail = True
        index.start()
        try:
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline and not index.state()[2]:
                time.sleep(0.01)
            page = index.page(options())
            self.assertEqual((page['index']['state'], page['total']), ('reconnecting', 1))
            self.assertTrue(bundle_valid(page))
            chain.fail = False
            chain.txs[4] = [transfer(4)]
            chain.append()
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline and index.page(options())['total'] != 2:
                time.sleep(0.02)
            page = index.page(options())
            self.assertEqual((page['total'], page['index']['state']), (2, 'ready'))
        finally:
            index.stop()
        # A long silence (no node answer for STALE_SECONDS) is "reconnecting" too.
        self.now[0] += T.STALE_SECONDS + 1
        self.assertEqual(index.page(options())['index']['state'], 'reconnecting')

    def test_a_reader_never_waits_for_the_writer(self):
        chain = FakeChain(3, {2: [transfer(1)]})
        index = self.index(chain)
        index.step()
        writer = sqlite3.connect(self.path, isolation_level=None)
        writer.execute('BEGIN IMMEDIATE')
        writer.execute("INSERT INTO meta VALUES('held', '1')")
        try:
            started = time.monotonic()
            page = index.page(options())
            self.assertLess(time.monotonic() - started, 0.4)
            self.assertEqual(page['total'], 1)
        finally:
            writer.execute('ROLLBACK')
            writer.close()

    def test_a_page_query_past_its_time_limit_is_interrupted(self):
        chain = FakeChain(30, {n: [transfer(n), transfer(n + 1, A2, A3)] for n in range(1, 31)})
        index = self.index(chain)
        index.step()
        with unittest.mock.patch.object(T, 'PAGE_SECONDS', -1.0), unittest.mock.patch.object(T, 'PROGRESS_STEPS', 1):
            with self.assertRaises(sqlite3.OperationalError):
                index.page(options(order='value_desc', q='0x0'))
        self.assertEqual(index.page(options())['total'], 60, 'the next page is answered')

    def test_page_refuses_options_the_relay_would_not_send(self):
        index = self.index(FakeChain(1))
        with self.assertRaises(ValueError):
            index.page(options(q='0xAB'))


class GatewayTests(Base):
    """pocol_getTransactions through the gateway and /api/transactions.php through its relay, with the real index."""

    def setUp(self):
        super().setUp()
        self.chain = FakeChain(5, {n: [transfer(n)] for n in range(1, 6)})
        self.tx = self.index(self.chain)
        self.tx.step()
        self.gw = gateway.Gateway(1, None, self.tx, network=F.NET)
        self.server = gateway.Server(('127.0.0.1', 0), self.gw, 'http://127.0.0.1:8661')
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown(); self.server.server_close(); self.thread.join()
        self.tx.writer.close()
        super().tearDown()

    def test_the_gateway_takes_the_index_options_and_answers_the_page(self):
        self.assertIs(gateway.valid_options, T.valid_options, 'the gateway imports the index module')
        self.assertTrue(gateway.valid_call({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getTransactions', 'params': [options()]}))
        self.assertFalse(gateway.valid_call({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getTransactions', 'params': [options(q='0xAB')]}))
        status, _, raw = F.send(self.port, 'GET', '/api/transactions.php?page=1&size=25&order=newest&q=')
        self.assertEqual(status, 200)
        data = json.loads(raw)['data']
        self.assertTrue(bundle_valid(data))
        self.assertEqual([r['hash'] for r in data['rows']], [r[2] for r in self.chain.canonical_rows()])
        snap = data['snapshot']
        self.chain.reorg(2, 6)
        self.tx.step()
        with self.gw.lock:
            self.gw.cache.clear(); self.gw.cache_size = 0
        status, _, raw = F.send(self.port, 'GET', f'/api/transactions.php?page=1&size=25&order=newest&q=&height={snap["height"]}&hash={snap["hash"]}')
        self.assertEqual((status, json.loads(raw)), (409, {'error': 'The chain changed. Refreshing transaction history.', 'reset': True}))
        status, _, raw = F.send(self.port, 'GET', '/api/transactions.php?q=0xZZ')
        self.assertEqual(status, 400)

    def test_rpc_pocol_get_transactions(self):
        status, _, raw = F.post_json(self.port, '/rpc', {'jsonrpc': '2.0', 'id': 7, 'method': 'pocol_getTransactions', 'params': [options(size=50)]})
        answer = json.loads(raw)
        self.assertEqual((status, answer['id'], answer['result']['size'], answer['result']['total']), (200, 7, 50, 5))


if __name__ == '__main__':
    unittest.main()
