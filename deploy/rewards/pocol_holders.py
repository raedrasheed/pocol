"""The explorer gateway's PCOL holders index (pocol-holders-v1 pages, pocol-holders-stats-v1 statistics), the backend
of the explorer's Holders tab and of the miner map's registry, blocks-won and rewards figures
(development/explorer/MINERS-MAP-HOLDERS.md §2, §3, §5.1, §5.2).

A sibling of pocol_transactions.py in the gateway process. A daemon thread follows the gateway host's own node over
the gateway's loopback `query` (Gateway.query: 127.0.0.1 only) and keeps one SQLite file (POCOL_HOLDERS_DB):

- History pass, from genesis, blocks only: every block's hash, parent, time, `miner` (the record ID that won it,
  apply.rs ParentResult.winner, rpc.rs block_json) and base fee destroyed, baseFeePerGas x gasUsed (the header's
  gasUsed is the user phase's gas only: user_phase.rs commitments, chain.rs check 17; system gas is BlockOutput's
  separate system_gas, apply.rs); the full block when it has transactions, and the receipt of each contract creation.
  Per address: the height it was first seen, its top-level transactions sent or received and its blocks mined.
  Every block of the newest RECENT_BLOCKS keeps undo rows, so a reorganization within them is rolled back exactly
  (the pocol_transactions._reconcile/_truncate rules); a deeper one indexes everything again from genesis.
- Tip snapshots, inside the node's 32-block state window (K_hist): at the pinned height h of the history tip, which
  must be the node's head, the registry (pocol_getMiners, bracketed by two head reads), heldTotal (MinerRegistry slot
  4), burnedTotal (RewardEscrow slot 0), graceStart (word 3 of rec(id) = keccak(id || 7), bits 192..255) and the
  balances of the dirty set, every read with the hex height h, then the block hash at h read again (a change
  discards the snapshot). The supply identity sum of balances(h) == G(h) - F(h) is checked: G is the genesis supply
  plus issuance::cumulative(h) (ported exactly below), F the base fees destroyed. A difference makes the next round
  re-read every known address at a new pinned height (full rescan); a difference that survives a full rescan is
  published with verification.complete false and the difference. pocol_getCaps' `accounts` (the existing accounts
  outside C0C001..C0C005, state_summary.rs) is compared only when its `number` and `hash` are h.
- Rewards: for each new block h while it is inside the state window, the injected `rewards(h)` (by default
  pocol_rewards.snapshot_v3(height=h)) gives block h's executed flows; `block.beneficiaries` is added per address:
  amountWei to the reward address, bondAccruedWei to the miner ID (its bond). Those entries are every payment
  RewardEscrow makes out of a block's reward flows: settle(h-D) (winner part, proposer part, share distribution), the
  tip share and the evidence reporter's cut. Bond refunds (markClose, the sweep, claimBond) and user transfers are not
  mining rewards and are not counted. The first block covered is `rewardsFrom`; a gap longer than the state window
  makes rewards null until an operator calls reset_rewards() (they then restart from the next followed block).
- Published snapshots are materialized rankings (rank, address, totals, type, links, counters) read by (snapshot,
  rank), (snapshot, type, type rank) and (snapshot, address) indexes, never sorted by a request. The newest
  KEEP_SNAPSHOTS and every snapshot younger than KEEP_SECONDS are kept; a page pinned to a snapshot that is gone or no
  longer canonical raises SnapshotChanged (the relay's 409 reset). Statistics are computed at publication and stored
  with the snapshot, so stats() is one row read.

Not discoverable over RPC (stated, never estimated): an address that only received PCOL by an internal call of a
non-system contract. Such value shows as verification.unlocatedWei. Standard library only (Python 3.10+).
"""
import json
import logging
import re
import sqlite3
import threading
import time
from pathlib import Path

from pocol_rewards import (CAPS, DEFAULT_FORK_SCHEDULE, ESCROW, LEDGER, MAX_SPEC_BYTES, REGISTRY, _v3_cp, hex_bytes,
                           keccak256, rlp_decode, snapshot_v3, validated_fork_schedule, validated_network)

SCHEMA = 'pocol-holders-v1'
STATS_SCHEMA = 'pocol-holders-stats-v1'
DB_VERSION = 1
ORDERS = ('balance_desc', 'balance_asc')
SIZES = (25, 50, 100)
TYPES = ('all', 'wallet', 'miner', 'reward', 'contract', 'system')
QUERY_KEYS = ('page', 'size', 'order', 'type', 'q', 'height', 'hash')
PAGE_MAX = 1_000_000_000
MAX_ID = 9007199254740991
MAX_QUERY = 4096
RECENT_BLOCKS = 4096           # block hashes and undo rows kept for reorganizations
RECENT_KEEP = 20000            # block times and winners kept (the 24 h figures and the blocks-won window)
STATE_WINDOW = 32              # K_hist (lp4-execution admission.rs): state is served for [head - 31, head]
STATE_MARGIN = 4               # state reads only for heights at least this far inside the window
WINNERS_WINDOW = 8640          # about 24 h of 10 s blocks (MINERS-MAP-HOLDERS.md §4)
KEEP_SNAPSHOTS = 3
KEEP_SECONDS = 180.0
HISTORY_SECONDS = 3600         # one statistics history point per hour of block time
HISTORY_POINTS_MAX = 2000
RESCAN_SECONDS = 3600.0        # a full rescan at least this often, even when the identity holds
SNAPSHOT_SLOWDOWN = 2.0        # the next snapshot waits this many times the last snapshot's duration
COMMIT_BLOCKS = 200
READ_BATCH = 20                # pocold BATCH_MAX (rpc.rs)
POLL_SECONDS = 2.0
PACE_SECONDS = 0.005
BACKOFF_MAX = 30.0
STALE_SECONDS = 30.0
PAGE_SECONDS = 2.0
PROGRESS_STEPS = 10000
IDENTITY_SECONDS = 60.0
MAX_TXS_PER_BLOCK = 100_000
MAX_RECORDS = 24576            # REG_RECORDS_MAX of the Mainnet profile
FOUNDER_TOTAL_WEI = 10 ** 24   # pocold testnet.rs FOUNDER_TOTAL_WEI: allocation + B_reg per founder
PCOL = 10 ** 18
U64_MAX = (1 << 64) - 1
HALVINGS = 256
CP_D, CP_THETA_INACT, CP_S0, CP_H_HALF, CP_S_TAIL = 11, 25, 15, 52, 53
CHUNK_FACTORY = '0x0000000000000000000000000000000000c0c005'
PAY = '0x0000000000000000000000000000000000c0c0ff'
SYSTEM_CALLER = '0x' + 'ff' * 19 + 'fe'
LABELS = {REGISTRY: 'MinerRegistry', ESCROW: 'RewardEscrow', LEDGER: 'ShareLedger', CAPS: 'ChainCaps',
          CHUNK_FACTORY: 'ChunkFactory', PAY: 'PAY', SYSTEM_CALLER: 'SYSTEM'}
# The accounts pocol_getCaps' count leaves out (state_summary.rs is_system: the five programs C0C001..C0C005 only).
PROGRAMS = frozenset((REGISTRY, ESCROW, LEDGER, CAPS, CHUNK_FACTORY))
ZERO_ADDRESS = '0x' + '00' * 20
# Delta flags of one address in one block.
SENT, RECEIVED, CREATED, MINED, DISCOVERED = 1, 2, 4, 8, 16
# Balance buckets by Total in PCOL (§5.2): [0,1), [1,10), ..., [1M, infinity).
BUCKETS = tuple((lo * PCOL, None if hi is None else hi * PCOL) for lo, hi in
                ((0, 1), (1, 10), (10, 100), (100, 1000), (1000, 10 ** 4), (10 ** 4, 10 ** 5), (10 ** 5, 10 ** 6),
                 (10 ** 6, None)))
TOP_N = (1, 10, 50, 100)
HASH = re.compile(r'0x[0-9a-fA-F]{64}')
ADDRESS = re.compile(r'0x[0-9a-fA-F]{40}')
QUANTITY = re.compile(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,63})')
DATA = re.compile(r'0x(?:[0-9a-fA-F]{2})*')
DECIMAL = re.compile(r'0|[1-9][0-9]{0,77}')
QUERY = re.compile(r'0x[0-9a-f]{1,40}')
LOG = logging.getLogger('pocol_holders')


class SnapshotChanged(RuntimeError):
    """The snapshot a page was asked at is gone or no longer canonical (the relay's 409 reset)."""


class Unavailable(RuntimeError):
    """The index has no published snapshot yet (the relay's 503)."""


class NodeAnswerInvalid(RuntimeError):
    """A node answer of an unexpected shape: nothing of it is stored."""


# ---------------------------------------------------------------------------------------------------------------------
# Issuance (lp4-execution/src/issuance.rs), exact on Python integers.
# ---------------------------------------------------------------------------------------------------------------------
def issuance(h, s0, h_half, s_tail):
    """issuance.rs `issuance`: h == 0 ? 0 : max(k >= 256 ? 0 : S0 >> k, S_tail), k = h / H_half (H_half = 0: no S0
    term, as `checked_div(..).unwrap_or(u64::MAX)`)."""
    if h == 0:
        return 0
    k = h // h_half if h_half else U64_MAX
    return max(0 if k >= HALVINGS else s0 >> k, s_tail)


def cumulative(n, s0, h_half, s_tail):
    """issuance.rs `cumulative`: the sum of issuance(h) for h in 1..=n, in closed form per era, with the Rust u64
    saturating arithmetic of the era bounds."""
    if n == 0 or h_half == 0:
        return s_tail * n
    total, k = 0, 0
    while True:
        lo = max(min(k * h_half, U64_MAX), 1)
        if lo > n:
            break
        hi = min(n, max(min((k + 1) * h_half, U64_MAX) - 1, 0))
        if hi >= lo:
            total += issuance(lo, s0, h_half, s_tail) * (hi - lo + 1)
        if hi == n or hi == U64_MAX:
            break
        k += 1
        if k >= HALVINGS:
            total += s_tail * (n - hi)
            break
    return total


def genesis_members(spec):
    """The M_0List of a GenesisSpec ([version, chainId, CP, allocRoot, M_0List, sysCodeHash]): (id, reward) pairs."""
    top = rlp_decode(spec, MAX_SPEC_BYTES, 65536)
    members = top[4] if isinstance(top, list) and len(top) == 6 else None
    if not isinstance(members, list) or not members:
        raise NodeAnswerInvalid('GenesisSpec without an M_0List')
    out = []
    for entry in members:
        if (not isinstance(entry, list) or len(entry) != 2
                or not all(isinstance(x, bytes) and len(x) == 20 for x in entry)):
            raise NodeAnswerInvalid('Malformed M_0List entry')
        out.append(('0x' + entry[0].hex(), '0x' + entry[1].hex()))
    return out


def record_slot(miner):
    """apply.rs record_slot: keccak256(id as a 32-byte word || be256(7))."""
    return int.from_bytes(keccak256(int(miner, 16).to_bytes(32, 'big') + (7).to_bytes(32, 'big')), 'big')


def held(record):
    """A record whose bond is held (PoColSystemV3 live; v3_contracts.rs conserved): status != 0, not a tomb, bond
    state HELD (1) or ACCRUING (4)."""
    return record['status'] != 0 and not record['tomb'] and record['bondState'] in (1, 4)


def share_e10(total, supply):
    """Supply percent x 10^10 = floor(Total x 10^12 / S)."""
    return total * 10 ** 12 // supply if supply > 0 else 0


# ---------------------------------------------------------------------------------------------------------------------
# Options (GET /api/holders.php, §5.1).
# ---------------------------------------------------------------------------------------------------------------------
def valid_options(options):
    """The page options holder_options builds: exactly page (1..10^9), size (25, 50, 100), order, type, q ('' or a
    lower-case 0x prefix of 1..40 hex digits) and an optional snapshot {height, hash}; integers are not booleans."""
    if not isinstance(options, dict):
        return False
    keys = set(options)
    base = {'page', 'size', 'order', 'type', 'q'}
    if not base <= keys <= base | {'snapshot'}:
        return False
    page, size, order, kind, q = (options[k] for k in ('page', 'size', 'order', 'type', 'q'))
    if type(page) is not int or not 1 <= page <= PAGE_MAX or type(size) is not int or size not in SIZES:
        return False
    if not isinstance(order, str) or order not in ORDERS or not isinstance(kind, str) or kind not in TYPES:
        return False
    if not isinstance(q, str) or (q and not QUERY.fullmatch(q)):
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


def _unquote(text):
    from urllib.parse import unquote_to_bytes
    return unquote_to_bytes(text.replace('+', ' ')).decode('utf-8', 'replace')


def holder_options(query):
    """holders.php: the page options of a query string, or None (400). Defaults page 1, size 25, balance_desc, all,
    no search; a key of ours given twice or as an array (page[]=1) is refused, other keys are ignored; height and
    hash only together. Mirrors explorer_api.transaction_options."""
    if not isinstance(query, str) or len(query) > MAX_QUERY:
        return None
    found = {}
    for pair in query.split('&') if query else []:
        if not pair:
            continue
        raw_key, _, raw_value = pair.partition('=')
        key = _unquote(raw_key).lstrip(' ')
        base = key.split('[', 1)[0]
        if base not in QUERY_KEYS:
            continue
        if base != key or base in found:
            return None
        found[base] = _unquote(raw_value)
    page, size, order, kind, q = (found.get('page', '1'), found.get('size', '25'), found.get('order', 'balance_desc'),
                                  found.get('type', 'all'), found.get('q', ''))
    if (not re.fullmatch(r'[0-9]+', page) or not 1 <= int(page) <= PAGE_MAX or size not in ('25', '50', '100')
            or order not in ORDERS or kind not in TYPES or (q != '' and not re.fullmatch(r'0x[0-9a-fA-F]{1,40}', q))):
        return None
    options = {'page': int(page), 'size': int(size), 'order': order, 'type': kind, 'q': q.lower()}
    if 'height' in found or 'hash' in found:
        height, block = found.get('height'), found.get('hash')
        if (height is None or block is None or not re.fullmatch(r'[0-9]+', height) or int(height) > MAX_ID
                or not HASH.fullmatch(block)):
            return None
        options['snapshot'] = {'height': int(height), 'hash': block.lower()}
    return options


# ---------------------------------------------------------------------------------------------------------------------
# Node answers.
# ---------------------------------------------------------------------------------------------------------------------
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


def _address(value, what, optional=False):
    if value is None and optional:
        return None
    if not isinstance(value, str) or not ADDRESS.fullmatch(value):
        raise NodeAnswerInvalid('Malformed ' + what)
    return value.lower()


def _word(value, what):
    raw = value if isinstance(value, str) else None
    if raw is None or not re.fullmatch(r'0x[0-9a-fA-F]{64}', raw):
        raise NodeAnswerInvalid('Malformed ' + what)
    return int(raw, 16)


def _decimal(value, what):
    if not isinstance(value, str) or not DECIMAL.fullmatch(value):
        raise NodeAnswerInvalid('Malformed ' + what)
    return int(value)


def light_block(value):
    """The fields of an eth_getBlockByNumber(n, false) answer this index uses."""
    if not isinstance(value, dict):
        raise NodeAnswerInvalid('Malformed block')
    txs = value.get('transactions')
    if not isinstance(txs, list) or len(txs) > MAX_TXS_PER_BLOCK:
        raise NodeAnswerInvalid('Malformed block transactions')
    pocol = value.get('pocol')
    final = isinstance(pocol, dict) and pocol.get('final') is True
    return {'height': _qty(value.get('number'), 'block number', 63), 'hash': _hash(value.get('hash'), 'block hash'),
            'parent': _hash(value.get('parentHash'), 'parent hash'),
            'timestamp': _qty(value.get('timestamp'), 'block time', 63),
            'miner': _address(value.get('miner'), 'block miner'),
            'gasUsed': _qty(value.get('gasUsed'), 'gas used', 64),
            'baseFee': _qty(value.get('baseFeePerGas'), 'base fee'),
            'final': final, 'txs': [_hash(t, 'transaction hash') for t in txs]}


def parse_miners(value):
    """pocol_getMiners (rpc.rs): every registry record {id, status, tomb, bondState, lastShare, bond, registeredAt,
    reward, rewardBalance}; a record ID appears once."""
    if not isinstance(value, list) or len(value) > MAX_RECORDS:
        raise NodeAnswerInvalid('Malformed miner list')
    out, seen = [], set()
    small = lambda v, bits: type(v) is int and 0 <= v < 1 << bits
    for item in value:
        if not isinstance(item, dict):
            raise NodeAnswerInvalid('Malformed miner record')
        miner = _address(item.get('id'), 'miner ID')
        tomb = item.get('tomb')
        if (miner in seen or not small(item.get('status'), 8) or type(tomb) is not bool
                or not small(item.get('bondState'), 8) or not small(item.get('lastShare'), 64)
                or not small(item.get('registeredAt'), 64)):
            raise NodeAnswerInvalid('Malformed miner record')
        seen.add(miner)
        out.append({'id': miner, 'reward': _address(item.get('reward'), 'reward address'),
                    'status': item['status'], 'tomb': tomb, 'bondState': item['bondState'],
                    'lastShare': item['lastShare'], 'registeredAt': item['registeredAt'],
                    'bond': _qty(item.get('bond'), 'bond')})
    return out


def reward_flows(answer, height, block_hash):
    """Block `height`'s executed reward flows from a pocol-block-reward-v3 answer for that block: one (miner ID, reward
    address, amount paid, bond accrued) per block.beneficiaries entry (pocol_rewards._entries)."""
    if (not isinstance(answer, dict) or answer.get('blockHeight') != height
            or not isinstance(answer.get('blockHash'), str) or answer['blockHash'].lower() != block_hash):
        raise NodeAnswerInvalid('Reward flows of another block')
    block = answer.get('block')
    entries = block.get('beneficiaries') if isinstance(block, dict) else None
    if not isinstance(entries, list) or len(entries) > 4 * MAX_RECORDS:
        raise NodeAnswerInvalid('Malformed reward flows')
    out = []
    for entry in entries:
        if not isinstance(entry, dict) or not set(entry) <= {'minerId', 'rewardAddress', 'amountWei', 'bondAccruedWei'}:
            raise NodeAnswerInvalid('Malformed reward flow')
        paid = _decimal(entry.get('amountWei'), 'reward amount')
        accrued = _decimal(entry.get('bondAccruedWei', '0'), 'bond accrual')
        out.append((_address(entry.get('minerId'), 'reward miner'), _address(entry.get('rewardAddress'), 'reward address'),
                    paid, accrued))
    return out


SCHEMA_SQL = '''
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS blocks(height INTEGER PRIMARY KEY, hash TEXT NOT NULL, parent TEXT NOT NULL,
    timestamp INTEGER NOT NULL, fees TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS recent(height INTEGER PRIMARY KEY, timestamp INTEGER NOT NULL, miner TEXT);
CREATE TABLE IF NOT EXISTS accounts(address TEXT PRIMARY KEY, first_seen INTEGER NOT NULL, txs INTEGER NOT NULL,
    mined INTEGER NOT NULL, balance TEXT, nonce INTEGER, code INTEGER, checked INTEGER) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS accounts_contracts ON accounts(code);
CREATE TABLE IF NOT EXISTS deltas(height INTEGER NOT NULL, address TEXT NOT NULL, txs INTEGER NOT NULL,
    mined INTEGER NOT NULL, flags INTEGER NOT NULL, PRIMARY KEY(height, address)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS reward_flows(height INTEGER NOT NULL, miner TEXT NOT NULL, address TEXT NOT NULL,
    paid TEXT NOT NULL, accrued TEXT NOT NULL, PRIMARY KEY(height, miner, address)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS rewards_address(address TEXT PRIMARY KEY, wei TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS rewards_miner(miner TEXT PRIMARY KEY, paid TEXT NOT NULL, accrued TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS snapshots(id INTEGER PRIMARY KEY, height INTEGER NOT NULL, hash TEXT NOT NULL,
    created REAL NOT NULL, counts TEXT NOT NULL, stats TEXT NOT NULL, registry TEXT NOT NULL, winners TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS snapshots_block ON snapshots(height, hash);
CREATE TABLE IF NOT EXISTS holders(snap INTEGER NOT NULL, rank INTEGER NOT NULL, address TEXT NOT NULL,
    type TEXT NOT NULL, type_rank INTEGER NOT NULL, total TEXT NOT NULL, available TEXT NOT NULL, bonded TEXT NOT NULL,
    share TEXT NOT NULL, txs INTEGER NOT NULL, mined INTEGER, rewards TEXT, miner_ids TEXT NOT NULL,
    reward_address TEXT, label TEXT, PRIMARY KEY(snap, rank)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS holders_type ON holders(snap, type, type_rank);
CREATE INDEX IF NOT EXISTS holders_address ON holders(snap, address, type);
CREATE TABLE IF NOT EXISTS stage(address TEXT PRIMARY KEY, type TEXT NOT NULL, total TEXT NOT NULL,
    total_key BLOB NOT NULL, available TEXT NOT NULL, bonded TEXT NOT NULL, share TEXT NOT NULL, txs INTEGER NOT NULL,
    mined INTEGER, rewards TEXT, miner_ids TEXT NOT NULL, reward_address TEXT, label TEXT) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS history(height INTEGER PRIMARY KEY, timestamp INTEGER NOT NULL, holders INTEGER NOT NULL,
    supply TEXT NOT NULL, top10 TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS dirty(address TEXT PRIMARY KEY, nonce INTEGER NOT NULL, code INTEGER NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS scan(address TEXT PRIMARY KEY, balance TEXT NOT NULL, nonce INTEGER, code INTEGER)
    WITHOUT ROWID;
'''
SCAN_CHUNK = 1000              # addresses read and staged per write while a snapshot reads balances
ROW_COLUMNS = 'rank, address, type, label, miner_ids, reward_address, total, available, bonded, share, txs, mined, rewards'
# Every statement a page runs (test_pocol_holders checks their query plans): rank ranges on the primary key, type-rank
# ranges on holders_type, a search's address range on holders_address, then each row by its primary key.
PAGE_SQL = {
    'all_desc': f'SELECT {ROW_COLUMNS} FROM holders WHERE snap=? AND rank>=? AND rank<=? ORDER BY rank ASC',
    'all_asc': f'SELECT {ROW_COLUMNS} FROM holders WHERE snap=? AND rank>=? AND rank<=? ORDER BY rank DESC',
    'type_desc': f'SELECT {ROW_COLUMNS} FROM holders WHERE snap=? AND type=? AND type_rank>=? AND type_rank<=? '
                 'ORDER BY type_rank ASC',
    'type_asc': f'SELECT {ROW_COLUMNS} FROM holders WHERE snap=? AND type=? AND type_rank>=? AND type_rank<=? '
                'ORDER BY type_rank DESC',
    'search': 'SELECT rank FROM holders WHERE snap=? AND address>=? AND address<?',
    'search_type': 'SELECT rank FROM holders WHERE snap=? AND address>=? AND address<? AND type=?',
    'row': f'SELECT {ROW_COLUMNS} FROM holders WHERE snap=? AND rank=?',
    'newest': 'SELECT id, height, hash, counts, stats, registry, winners FROM snapshots ORDER BY id DESC LIMIT 1',
    'pinned': 'SELECT id, height, hash, counts, stats, registry, winners FROM snapshots WHERE height=? AND hash=? '
              'ORDER BY id DESC LIMIT 1',
    'canonical': 'SELECT hash FROM blocks WHERE height=?',
}
META_DEFAULTS = {'rewards_state': 'none', 'rewards_from': '-1', 'rewards_to': '-1', 'refreshed': '-1', 'full': '0',
                 'residual': '0', 'rescan_at': '0', 'covered_from': '-1', 'final': '-1'}


def default_rewards(query, network, fork_schedule=DEFAULT_FORK_SCHEDULE):
    """rewards(h): pocol_rewards.snapshot_v3 of block h over `query` (its own read and time budget)."""
    def rewards(height):
        return snapshot_v3(query, fork_schedule=fork_schedule, network=network, height=height)
    return rewards


class HolderIndex:
    """HolderIndex(path, query, network, ..): `query(method, params)` is one node read answering the JSON-RPC envelope
    (Gateway.query); `batch(calls)`, when given, answers a list of (method, params) with their envelopes in order (a
    pocold batch of at most READ_BATCH); `rewards(height)` answers block `height`'s pocol-block-reward-v3 projection
    (default_rewards); `network` is the operator's validated network, stored in the file on first use and required
    to match afterwards. `genesis_wei` overrides the genesis supply rule |M_0List| x FOUNDER_TOTAL_WEI (pocold
    testnet.rs check_founders: the Mainnet profile's only allocation)."""

    def __init__(self, path, query, network=None, *, clock=time.time, rewards=None, batch=None,
                 fork_schedule=DEFAULT_FORK_SCHEDULE, genesis_wei=None, poll=POLL_SECONDS, pace=PACE_SECONDS,
                 recent=RECENT_BLOCKS, commit_blocks=COMMIT_BLOCKS, keep_snapshots=KEEP_SNAPSHOTS,
                 keep_seconds=KEEP_SECONDS, rescan_seconds=RESCAN_SECONDS, slowdown=SNAPSHOT_SLOWDOWN):
        if network is None:
            raise ValueError('The holders index needs the configured network')
        self.network = validated_network(network)
        self.fork_schedule = validated_fork_schedule(fork_schedule)
        if genesis_wei is not None and (type(genesis_wei) is not int or genesis_wei < 0):
            raise ValueError('genesis_wei must be a non-negative integer')
        if type(keep_snapshots) is not int or keep_snapshots < 1:
            raise ValueError('keep_snapshots must be a positive integer')
        self.path = str(Path(path))
        self.query, self.batch = query, batch
        self.chain_id, self.genesis = self.network.chain_id, self.network.genesis
        self.rewards = rewards or default_rewards(query, self.network, self.fork_schedule)
        self.genesis_override = genesis_wei
        self.clock = clock
        self.poll, self.pace, self.recent, self.commit_blocks = poll, pace, recent, commit_blocks
        self.keep_snapshots, self.keep_seconds, self.rescan_seconds, self.slowdown = (
            keep_snapshots, keep_seconds, rescan_seconds, slowdown)
        self.lock = threading.Lock()
        self.head = -1
        self.last_contact = None
        self.failing = False
        self.identity_checked = None
        self.cp = None
        self.founders = None
        self.genesis_wei = None
        self.next_snapshot = 0.0
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
        try:
            conn.execute('PRAGMA journal_mode=WAL')
            conn.execute('PRAGMA synchronous=NORMAL')
            conn.execute('PRAGMA cache_size=-8000')
            conn.executescript(SCHEMA_SQL)
        except BaseException:
            conn.close()
            raise
        return conn

    def _reader(self):
        conn = sqlite3.connect(f'file:{self.path}?mode=ro', uri=True, timeout=0.5, isolation_level=None)
        try:
            conn.execute('PRAGMA query_only=1')
            deadline = time.monotonic() + PAGE_SECONDS
            conn.set_progress_handler(lambda: 1 if time.monotonic() > deadline else 0, PROGRESS_STEPS)
        except BaseException:
            conn.close()
            raise
        return conn

    @staticmethod
    def _meta(conn, key):
        row = conn.execute('SELECT value FROM meta WHERE key=?', (key,)).fetchone()
        return META_DEFAULTS.get(key) if row is None else row[0]

    @staticmethod
    def _set(conn, key, value):
        conn.execute('INSERT OR REPLACE INTO meta VALUES(?,?)', (key, str(value)))

    def _check_identity_meta(self):
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            stored = conn.execute("SELECT value FROM meta WHERE key='network'").fetchone()
            mine = json.dumps({'chainId': self.chain_id, 'genesisHash': self.genesis, 'version': DB_VERSION})
            if stored is None:
                conn.execute('INSERT INTO meta VALUES(?,?)', ('network', mine))
            elif stored[0] != mine:
                raise RuntimeError(f'{self.path} is the holders index of another network or version; '
                                   'move it away to index this network')
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    def close(self):
        self.stop()
        self.writer.close()

    def tip(self, conn=None):
        row = (conn or self.writer).execute('SELECT height, hash FROM blocks ORDER BY height DESC LIMIT 1').fetchone()
        return (row[0], row[1]) if row else None

    # ---------------- node reads ----------------
    def _envelope(self, answer, method):
        if not isinstance(answer, dict) or 'error' in answer or 'result' not in answer:
            raise RuntimeError('Node read failed: ' + method)
        return answer['result']

    def _read(self, method, params):
        result = self._envelope(self.query(method, params), method)
        with self.lock:
            self.last_contact = self.clock()
        return result

    def _read_many(self, calls):
        """The results of `calls` [(method, params)] in order: pocold batches of READ_BATCH when `batch` is given,
        single reads otherwise."""
        out = []
        if self.batch is None:
            for method, params in calls:
                out.append(self._read(method, params))
                if self.stop_event.is_set():
                    raise RuntimeError('Stopping')
            return out
        for start in range(0, len(calls), READ_BATCH):
            chunk = calls[start:start + READ_BATCH]
            answers = self.batch(chunk)
            if not isinstance(answers, list) or len(answers) != len(chunk):
                raise RuntimeError('Node batch read failed')
            out += [self._envelope(a, m) for a, (m, _) in zip(answers, chunk)]
            with self.lock:
                self.last_contact = self.clock()
            if self.stop_event.is_set():
                raise RuntimeError('Stopping')
        return out

    def _block_at(self, height):
        value = self._read('eth_getBlockByNumber', [hex(height), False])
        if value is None:
            return None
        block = light_block(value)
        if block['height'] != height:
            raise NodeAnswerInvalid('Block at another height')
        return block

    def _latest(self):
        value = self._read('eth_getBlockByNumber', ['latest', False])
        if value is None:
            raise RuntimeError('The node has no head block')
        return light_block(value)

    def _check_identity(self):
        now = self.clock()
        if self.identity_checked is not None and now - self.identity_checked < IDENTITY_SECONDS:
            return
        params = self._read('pocol_getParams', [])
        genesis = params.get('genesisHash') if isinstance(params, dict) else None
        if (not isinstance(params, dict) or type(params.get('chainId')) is not int or params['chainId'] != self.chain_id
                or not isinstance(genesis, str) or genesis.lower() != self.genesis):
            raise RuntimeError('The local node does not belong to the configured network')
        if self.cp is None:
            # The CP and the founders from the GenesisSpec the trusted genesis hash proves (pocol_rewards._v3_cp).
            cp = _v3_cp(params, self.network)
            founders = genesis_members(hex_bytes(params.get('genesisPre'), maximum=MAX_SPEC_BYTES))
            self.cp, self.founders = cp, founders
            self.genesis_wei = (self.genesis_override if self.genesis_override is not None
                                else len(founders) * FOUNDER_TOTAL_WEI)
        self.identity_checked = now

    # ---------------- following ----------------
    def start(self):
        if self.thread is None:
            self.thread = threading.Thread(target=self._run, name='pocol-holders', daemon=True)
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
                LOG.warning('holders index: %s', error)
                self.stop_event.wait(backoff)
                backoff = min(BACKOFF_MAX, backoff * 2)

    def step(self, limit=None):
        """One round: identity, fork check, up to `limit` new blocks (default: to the head), then, once at the head,
        the rewards of the new blocks and a tip snapshot when one is due. True when the history reached the head."""
        self._check_identity()
        head = self._latest()
        with self.lock:
            self.head = head['height']
        self._reconcile(head)
        if not self._follow(head, limit):
            return False
        self._follow_rewards(head['height'])
        if self._snapshot_due():
            self._snapshot()
        return True

    def _follow(self, head, limit):
        tip = self.tip()
        next_height = 0 if tip is None else tip[0] + 1
        target = head['height'] if limit is None else min(head['height'], next_height + limit - 1)
        pending, expected = [], None if tip is None else tip[1]
        height = next_height
        while height <= target and not self.stop_event.is_set():
            block = head if head['height'] == height else self._block_at(height)
            if block is None:
                break
            if expected is not None and block['parent'] != expected:
                break  # the chain moved under this read: the next round finds the fork point
            pending.append((block, self._parties(block) if block['txs'] else {}))
            expected = block['hash']
            height += 1
            if len(pending) >= self.commit_blocks:
                self._store(pending)
                pending = []
            if self.pace:
                time.sleep(self.pace)
        if pending:
            self._store(pending)
        tip = self.tip()
        return tip is not None and tip[0] >= head['height']

    def _parties(self, block):
        """{address: [transactions, flags]} of a block's top-level transactions: sender and recipient once each (a
        transfer to oneself counts once), a creation's contract (receipt contractAddress) as its recipient. A
        transaction the node could not decode has no known parties."""
        full = self._read('eth_getBlockByHash', [block['hash'], True])
        if not isinstance(full, dict) or _hash(full.get('hash'), 'block hash') != block['hash']:
            raise NodeAnswerInvalid('The full block is not the light block')
        txs = full.get('transactions')
        if not isinstance(txs, list) or len(txs) != len(block['txs']):
            raise NodeAnswerInvalid('The full block has other transactions')
        parties = {}
        def add(address, flag):
            entry = parties.setdefault(address, [0, 0])
            entry[1] |= flag
            return entry
        for i, tx in enumerate(txs):
            if not isinstance(tx, dict) or _hash(tx.get('hash'), 'transaction hash') != block['txs'][i]:
                raise NodeAnswerInvalid('The full block has other transactions')
            if 'from' not in tx:
                continue
            sender = _address(tx.get('from'), 'sender')
            to = _address(tx.get('to'), 'recipient', optional=True)
            if to is None:
                receipt = self._read('eth_getTransactionReceipt', [block['txs'][i]])
                if (not isinstance(receipt, dict) or _hash(receipt.get('blockHash'), 'receipt block') != block['hash']
                        or _hash(receipt.get('transactionHash'), 'receipt transaction') != block['txs'][i]):
                    raise NodeAnswerInvalid('Receipt of another transaction')
                to = _address(receipt.get('contractAddress'), 'created contract', optional=True)
            seen = set()
            add(sender, SENT)
            seen.add(sender)
            if to is not None:
                add(to, RECEIVED | (CREATED if tx.get('to') is None else 0))
                seen.add(to)
            for address in seen:
                parties[address][0] += 1
        return parties

    def _store(self, pending):
        """Blocks with their deltas and undo rows in one write transaction, then the windows."""
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            tip = self.tip(conn)
            if tip is not None and pending[0][0]['parent'] != tip[1]:
                raise RuntimeError('The stored tip changed')
            fees = 0 if tip is None else int(conn.execute('SELECT fees FROM blocks WHERE height=?',
                                                          (tip[0],)).fetchone()[0])
            for block, parties in pending:
                height = block['height']
                if height > 0:
                    fees += block['baseFee'] * block['gasUsed']
                conn.execute('INSERT INTO blocks VALUES(?,?,?,?,?)',
                             (height, block['hash'], block['parent'], block['timestamp'], str(fees)))
                miner = block['miner'] if height > 0 and block['miner'] != ZERO_ADDRESS else None
                conn.execute('INSERT INTO recent VALUES(?,?,?)', (height, block['timestamp'], miner))
                changes = {a: [n, 0, f] for a, (n, f) in parties.items()}
                if miner is not None:
                    entry = changes.setdefault(miner, [0, 0, 0])
                    entry[1] += 1
                    entry[2] |= MINED
                if height == 0:
                    for address in [*LABELS, *(a for pair in self.founders for a in pair)]:
                        changes.setdefault(address, [0, 0, 0])
                for address, change in changes.items():
                    self._account_change(conn, height, address, *change)
                if block['final']:
                    self._set(conn, 'final', height)
            newest = pending[-1][0]['height']
            for table in ('blocks', 'deltas', 'reward_flows'):
                conn.execute(f'DELETE FROM {table} WHERE height<=?', (newest - self.recent,))
            conn.execute('DELETE FROM recent WHERE height<=?', (newest - RECENT_KEEP,))
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    @staticmethod
    def _account_change(conn, height, address, txs, mined, flags):
        """One address's counters in block `height`, with its undo row (DISCOVERED: first seen here)."""
        if conn.execute('SELECT 1 FROM accounts WHERE address=?', (address,)).fetchone() is None:
            flags |= DISCOVERED
            conn.execute('INSERT INTO accounts VALUES(?,?,?,?,NULL,NULL,NULL,NULL)', (address, height, txs, mined))
        elif txs or mined:
            conn.execute('UPDATE accounts SET txs=txs+?, mined=mined+? WHERE address=?', (txs, mined, address))
        old = conn.execute('SELECT txs, mined, flags FROM deltas WHERE height=? AND address=?',
                           (height, address)).fetchone()
        if old is None:
            conn.execute('INSERT INTO deltas VALUES(?,?,?,?,?)', (height, address, txs, mined, flags))
        else:
            conn.execute('UPDATE deltas SET txs=?, mined=?, flags=? WHERE height=? AND address=?',
                         (old[0] + txs, old[1] + mined, old[2] | flags, height, address))

    def _reconcile(self, head):
        """Remove the indexed blocks that are no longer canonical (above the fork point)."""
        tip = self.tip()
        if tip is None:
            return
        height = min(tip[0], head['height'])
        if height == tip[0]:
            node = head if head['height'] == height else self._block_at(height)
            if node is not None and node['hash'] == tip[1]:
                return
        oldest = self.writer.execute('SELECT MIN(height) FROM blocks').fetchone()[0]
        fork = None
        while height >= oldest:
            stored = self.writer.execute('SELECT hash FROM blocks WHERE height=?', (height,)).fetchone()
            node = head if head['height'] == height else self._block_at(height)
            if stored is not None and node is not None and node['hash'] == stored[0]:
                fork = height
                break
            height -= 1
        self._truncate(fork)

    def _truncate(self, fork):
        """Undo every block above `fork` (None: everything; the index starts again from genesis)."""
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            if fork is None:
                LOG.warning('holders index: no common block with the node in the kept window; indexing again')
                for table in ('blocks', 'recent', 'accounts', 'deltas', 'reward_flows', 'rewards_address',
                              'rewards_miner', 'snapshots', 'holders', 'stage', 'history', 'dirty', 'scan'):
                    conn.execute(f'DELETE FROM {table}')
                conn.execute("DELETE FROM meta WHERE key!='network'")
            else:
                for height, address, txs, mined, flags in conn.execute(
                        'SELECT height, address, txs, mined, flags FROM deltas WHERE height>?', (fork,)).fetchall():
                    if flags & DISCOVERED:
                        conn.execute('DELETE FROM accounts WHERE address=?', (address,))
                    else:
                        conn.execute('UPDATE accounts SET txs=txs-?, mined=mined-? WHERE address=?',
                                     (txs, mined, address))
                self._undo_rewards(conn, fork)
                for (snap,) in conn.execute('SELECT id FROM snapshots WHERE height>?', (fork,)).fetchall():
                    conn.execute('DELETE FROM holders WHERE snap=?', (snap,))
                    conn.execute('DELETE FROM snapshots WHERE id=?', (snap,))
                for table in ('deltas', 'blocks', 'recent', 'history'):
                    conn.execute(f'DELETE FROM {table} WHERE height>?', (fork,))
                if int(self._meta(conn, 'refreshed')) > fork:
                    # Balances were read on the removed branch: every known address is read again.
                    self._set(conn, 'refreshed', fork)
                    self._set(conn, 'full', 1)
                if int(self._meta(conn, 'final')) > fork:
                    self._set(conn, 'final', -1)
                if int(self._meta(conn, 'covered_from')) > fork:
                    self._set(conn, 'covered_from', -1)
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    def _undo_rewards(self, conn, fork):
        rows = conn.execute('SELECT miner, address, paid, accrued FROM reward_flows WHERE height>?', (fork,)).fetchall()
        for miner, address, paid, accrued in rows:
            self._add_reward(conn, miner, address, -int(paid), -int(accrued))
        conn.execute('DELETE FROM reward_flows WHERE height>?', (fork,))
        if self._meta(conn, 'rewards_state') == 'ok':
            if int(self._meta(conn, 'rewards_from')) > fork:
                # Every covered block was removed: rewards start again from the next followed block.
                self._clear_rewards(conn, 'none')
            elif int(self._meta(conn, 'rewards_to')) > fork:
                self._set(conn, 'rewards_to', fork)

    @staticmethod
    def _add_reward(conn, miner, address, paid, accrued):
        """Adds one flow (negative amounts: its undo) to the totals: the amount paid to the reward address, the bond
        accrual to the miner ID, and both to the miner's own totals."""
        def add_address(key, amount):
            if amount:
                row = conn.execute('SELECT wei FROM rewards_address WHERE address=?', (key,)).fetchone()
                value = (int(row[0]) if row else 0) + amount
                if value < 0:
                    raise RuntimeError('Reward totals below zero')
                conn.execute('INSERT OR REPLACE INTO rewards_address VALUES(?,?)', (key, str(value)))
        add_address(address, paid)
        add_address(miner, accrued)
        if paid or accrued:
            row = conn.execute('SELECT paid, accrued FROM rewards_miner WHERE miner=?', (miner,)).fetchone()
            total_paid, total_accrued = (int(row[0]) + paid, int(row[1]) + accrued) if row else (paid, accrued)
            if total_paid < 0 or total_accrued < 0:
                raise RuntimeError('Reward totals below zero')
            conn.execute('INSERT OR REPLACE INTO rewards_miner VALUES(?,?,?)', (miner, str(total_paid), str(total_accrued)))

    def _clear_rewards(self, conn, state):
        for table in ('reward_flows', 'rewards_address', 'rewards_miner'):
            conn.execute(f'DELETE FROM {table}')
        self._set(conn, 'rewards_state', state)
        self._set(conn, 'rewards_from', -1)
        self._set(conn, 'rewards_to', -1)

    def reset_rewards(self):
        """Operator reset after a gap: the totals are cleared and rewards count again from the next followed block
        (its rewardsFrom). Call with the follower stopped, or between two steps."""
        conn = self.writer
        conn.execute('BEGIN IMMEDIATE')
        try:
            self._clear_rewards(conn, 'none')
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    # ---------------- rewards ----------------
    def _follow_rewards(self, head):
        conn = self.writer
        state = self._meta(conn, 'rewards_state')
        if state == 'gap':
            return
        tip = self.tip()
        lowest = head - (STATE_WINDOW - STATE_MARGIN)  # state h - 1 still readable with a margin
        if state == 'none':
            start = max(tip[0], 1)
        else:
            start = int(self._meta(conn, 'rewards_to')) + 1
            if start < lowest:
                LOG.warning('holders index: rewards gap at block %d (state window passed); rewards are null until '
                            'reset_rewards()', start)
                conn.execute('BEGIN IMMEDIATE')
                try:
                    self._clear_rewards(conn, 'gap')
                    conn.execute('COMMIT')
                except BaseException:
                    conn.execute('ROLLBACK')
                    raise
                return
        for height in range(start, tip[0] + 1):
            if self.stop_event.is_set():
                return
            stored = conn.execute('SELECT hash FROM blocks WHERE height=?', (height,)).fetchone()
            if stored is None:
                return
            try:
                flows = reward_flows(self.rewards(height), height, stored[0])
            except Exception as error:  # retried at the next round; past the window it becomes a gap
                LOG.warning('holders index: rewards of block %d: %s', height, error)
                return
            conn.execute('BEGIN IMMEDIATE')
            try:
                row = conn.execute('SELECT hash FROM blocks WHERE height=?', (height,)).fetchone()
                if row is None or row[0] != stored[0]:
                    raise RuntimeError('The block changed while its rewards were read')
                merged = {}
                for miner, address, paid, accrued in flows:
                    entry = merged.setdefault((miner, address), [0, 0])
                    entry[0] += paid
                    entry[1] += accrued
                for (miner, address), (paid, accrued) in merged.items():
                    conn.execute('INSERT INTO reward_flows VALUES(?,?,?,?,?)',
                                 (height, miner, address, str(paid), str(accrued)))
                    self._add_reward(conn, miner, address, paid, accrued)
                if self._meta(conn, 'rewards_state') == 'none':
                    self._set(conn, 'rewards_state', 'ok')
                    self._set(conn, 'rewards_from', height)
                self._set(conn, 'rewards_to', height)
                conn.execute('COMMIT')
            except BaseException:
                conn.execute('ROLLBACK')
                raise

    # ---------------- tip snapshots ----------------
    def _snapshot_due(self):
        tip = self.tip()
        if tip is None or self.clock() < self.next_snapshot:
            return False
        row = self.writer.execute('SELECT height FROM snapshots ORDER BY id DESC LIMIT 1').fetchone()
        return row is None or tip[0] > row[0] or self._meta(self.writer, 'full') == '1'

    def _snapshot(self):
        """One tip snapshot at the history tip h (the node's head): reads pinned to h, the block re-read, the supply
        identity, then the publication. Returns 'published', 'rescan' (a full rescan is due) or 'moved'."""
        started = time.monotonic()
        conn = self.writer
        h, block_hash = self.tip()
        now = self.clock()
        full = self._meta(conn, 'full') == '1' or now >= float(self._meta(conn, 'rescan_at'))
        head = self._latest()
        if (head['height'], head['hash']) != (h, block_hash):
            return 'moved'
        records = parse_miners(self._read('pocol_getMiners', []))
        again = self._latest()
        if (again['height'], again['hash']) != (h, block_hash):
            return 'moved'
        at = hex(h)
        held_total = _word(self._read('eth_getStorageAt', [REGISTRY, '0x4', at]), 'heldTotal')
        burned = _word(self._read('eth_getStorageAt', [ESCROW, '0x0', at]), 'burnedTotal')
        if sum(r['bond'] for r in records if held(r)) != held_total:
            return 'moved'  # pocol_getMiners was not the state at h
        active = [r for r in records if r['status'] == 1 and not r['tomb']]
        graces = self._read_many([('eth_getStorageAt', [REGISTRY, hex(record_slot(r['id']) + 3), at]) for r in active])
        grace = {r['id']: _word(v, 'graceStart') >> 192 for r, v in zip(active, graces)}

        registry_addresses = set()
        for r in records:
            registry_addresses.update((r['id'], r['reward']))
        new = sorted(a for a in registry_addresses
                     if conn.execute('SELECT 1 FROM accounts WHERE address=?', (a,)).fetchone() is None)
        self._dirty_set(conn, h, full, registry_addresses, new)
        # The dirty set's reads at h, in chunks of SCAN_CHUNK addresses staged in `scan` (bounded memory); applied
        # only after the block at h is read again.
        last = ''
        while True:
            chunk = conn.execute('SELECT address, nonce, code FROM dirty WHERE address>? ORDER BY address LIMIT ?',
                                 (last, SCAN_CHUNK)).fetchall()
            if not chunk:
                break
            last = chunk[-1][0]
            calls = []
            for address, nonce, code in chunk:
                calls.append(('eth_getBalance', [address, at]))
                if nonce:
                    calls.append(('eth_getTransactionCount', [address, at]))
                if code:
                    calls.append(('eth_getCode', [address, at]))
            results = iter(self._read_many(calls))
            staged = []
            for address, nonce, code in chunk:
                balance = _qty(next(results), 'balance')
                n = _qty(next(results), 'nonce', 64) if nonce else None
                c = None
                if code:
                    value = next(results)
                    if not isinstance(value, str) or not DATA.fullmatch(value):
                        raise NodeAnswerInvalid('Malformed code')
                    c = 1 if len(value) > 2 else 0
                staged.append((address, str(balance), n, c))
            conn.execute('BEGIN IMMEDIATE')
            try:
                conn.executemany('INSERT INTO scan VALUES(?,?,?,?)', staged)
                conn.execute('COMMIT')
            except BaseException:
                conn.execute('ROLLBACK')
                raise
        try:
            caps = self._read('pocol_getCaps', [])
        except RuntimeError:
            caps = None  # a cross-check only: without it the account count is not compared
        check = self._block_at(h)
        if check is None or check['hash'] != block_hash:
            return 'moved'

        conn.execute('BEGIN IMMEDIATE')
        try:
            if self.tip(conn) != (h, block_hash):
                raise RuntimeError('The stored tip changed')
            for address in new:
                self._account_change(conn, h, address, 0, 0, 0)
            conn.execute('UPDATE accounts SET '
                         'balance=(SELECT s.balance FROM scan s WHERE s.address=accounts.address), '
                         'nonce=COALESCE((SELECT s.nonce FROM scan s WHERE s.address=accounts.address), nonce), '
                         'code=COALESCE((SELECT s.code FROM scan s WHERE s.address=accounts.address), code), '
                         'checked=? WHERE address IN (SELECT address FROM scan)', (h,))
            conn.execute('DELETE FROM scan')
            conn.execute('DELETE FROM dirty')
            self._set(conn, 'refreshed', h)
            outcome = self._publish(conn, h, block_hash, check['timestamp'], records, grace, held_total, burned, caps,
                                    full, now)
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise
        duration = time.monotonic() - started
        # A rescan follows at once (at the next pinned height); otherwise snapshots keep to 1/slowdown of the time.
        self.next_snapshot = 0.0 if outcome == 'rescan' else self.clock() + duration * self.slowdown
        return outcome

    def _dirty_set(self, conn, h, full, registry_addresses, new):
        """Fills `dirty` (address, read the nonce, read the code): in a full rescan every known address (the code only
        where none was seen); otherwise the addresses never read, every contract, the transaction parties and created
        contracts since the last refresh (senders: nonce; creations: nonce and code), every registry ID and reward
        address and the system addresses. Registry addresses new to the index are read whole."""
        conn.execute('BEGIN IMMEDIATE')
        try:
            conn.execute('DELETE FROM dirty')
            conn.execute('DELETE FROM scan')
            if full:
                conn.execute('INSERT INTO dirty SELECT address, 1, CASE WHEN code=1 THEN 0 ELSE 1 END FROM accounts')
            else:
                conn.execute('INSERT INTO dirty SELECT address, 1, 1 FROM accounts WHERE checked IS NULL')
                conn.execute('INSERT OR IGNORE INTO dirty SELECT address, 0, 0 FROM accounts WHERE code=1')
                conn.execute('INSERT INTO dirty SELECT address, MAX((flags & ?) != 0), MAX((flags & ?) != 0) '
                             'FROM deltas WHERE height>? AND height<=? GROUP BY address '
                             'ON CONFLICT(address) DO UPDATE SET nonce=MAX(nonce, excluded.nonce), '
                             'code=MAX(code, excluded.code)',
                             (SENT | CREATED, CREATED, int(self._meta(conn, 'refreshed')), h))
                conn.executemany('INSERT OR IGNORE INTO dirty VALUES(?,0,0)',
                                 [(a,) for a in sorted({*LABELS, *registry_addresses})])
            conn.executemany('INSERT OR REPLACE INTO dirty VALUES(?,1,1)', [(a,) for a in new])
            conn.execute('COMMIT')
        except BaseException:
            conn.execute('ROLLBACK')
            raise

    def _publish(self, conn, h, block_hash, timestamp, records, grace, held_total, burned, caps, full, now):
        """The supply identity at h, then (or a full rescan) the materialized snapshot, its statistics, the history
        point and the retention, inside the caller's write transaction."""
        cp = self.cp
        final = int(self._meta(conn, 'final'))
        issued = cumulative(h if final < 0 else min(h, final - 1), cp[CP_S0], cp[CP_H_HALF], cp[CP_S_TAIL])
        minted = self.genesis_wei + issued
        fees = int(conn.execute('SELECT fees FROM blocks WHERE height=?', (h,)).fetchone()[0])
        expected = minted - fees
        balances, indexed = 0, 0
        for address, balance, nonce, code in conn.execute('SELECT address, balance, nonce, code FROM accounts'):
            value = int(balance or 0)
            balances += value
            if address not in PROGRAMS and (value or nonce or code):
                indexed += 1
        unlocated = expected - balances
        if unlocated and not full and str(unlocated) != self._meta(conn, 'residual'):
            # A difference not seen before: every known address is read again at a new pinned height.
            self._set(conn, 'full', 1)
            return 'rescan'
        escrow = int(conn.execute('SELECT balance FROM accounts WHERE address=?', (ESCROW,)).fetchone()[0] or 0)
        pending = escrow - held_total - burned
        if pending < 0:
            raise NodeAnswerInvalid('RewardEscrow holds less than its bonds and burns')
        supply = minted - fees - burned
        on_chain = None
        if (isinstance(caps, dict) and caps.get('number') == hex(h) and isinstance(caps.get('hash'), str)
                and caps['hash'].lower() == block_hash):
            on_chain = _qty(caps.get('accounts'), 'account count', 64)
        missing = None if on_chain is None else on_chain - indexed
        complete = unlocated == 0 and missing in (None, 0)
        rewards_ok = self._meta(conn, 'rewards_state') == 'ok'
        rewards_from = int(self._meta(conn, 'rewards_from')) if rewards_ok else None
        rewards_to = int(self._meta(conn, 'rewards_to')) if rewards_ok else None

        by_id = {r['id']: r for r in records}
        serves = {}
        for r in records:
            if r['reward'] != r['id']:
                serves.setdefault(r['reward'], []).append(r['id'])
        bonds = {r['id']: r['bond'] for r in records if held(r)}
        theta = cp[CP_THETA_INACT]
        registry = {'height': h, 'blockHash': block_hash, 'thetaInact': theta, 'records': [
            {'id': r['id'], 'reward': r['reward'], 'status': r['status'], 'tomb': r['tomb'], 'bondState': r['bondState'],
             'lastShare': r['lastShare'], 'graceStart': grace.get(r['id']), 'registeredAt': r['registeredAt'],
             'active': r['status'] == 1 and held(r),
             'inactive': r['status'] == 1 and held(r) and h + 1 >= max(r['lastShare'], grace.get(r['id'], 0)) + theta}
            for r in records]}

        # new24h: holders first seen at or after the first block of the last 24 h of block time, when this index has
        # followed the tip (snapshots without a gap past the state window) since before that block.
        start_24h = None
        boundary = conn.execute('SELECT MAX(height) FROM recent WHERE timestamp<?', (timestamp - 86400,)).fetchone()[0]
        oldest = conn.execute('SELECT MIN(height) FROM recent').fetchone()[0]
        if boundary is not None:
            start_24h = boundary + 1
        elif oldest == 0:
            start_24h = 0
        last = conn.execute('SELECT height FROM snapshots ORDER BY id DESC LIMIT 1').fetchone()
        covered = int(self._meta(conn, 'covered_from'))
        if covered < 0 or last is None or h - last[0] > STATE_WINDOW:
            covered = h
            self._set(conn, 'covered_from', h)
        if start_24h is not None and covered > start_24h:
            start_24h = None

        conn.execute('DELETE FROM stage')
        counts = {t: 0 for t in TYPES}
        stats = {'count': 0, 'withAvailable': 0, 'withBonded': 0, 'new24h': 0}
        buckets = [[0, 0] for _ in BUCKETS]
        circulating, system_total = 0, 0
        batch = []
        rows = conn.execute('SELECT a.address, a.first_seen, a.txs, a.mined, a.balance, a.code, r.wei '
                            'FROM accounts a LEFT JOIN rewards_address r ON r.address=a.address')
        for address, first_seen, txs, mined, balance, code, reward in rows:
            available = int(balance or 0)
            bonded = bonds.get(address, 0)
            if address in LABELS:
                kind = 'system'
                total = pending if address == ESCROW else available
                bonded = 0
            else:
                kind = ('miner' if address in by_id else 'reward' if address in serves else
                        'contract' if code else 'wallet')
                total = available + bonded
                circulating += available
            if kind == 'system':
                system_total += total
            if total <= 0:
                continue
            counts['all'] += 1
            counts[kind] += 1
            if kind != 'system':
                stats['count'] += 1
                stats['withAvailable'] += available > 0
                stats['withBonded'] += bonded > 0
                if start_24h is not None and first_seen >= start_24h:
                    stats['new24h'] += 1
                for i, (lo, hi) in enumerate(BUCKETS):
                    if total >= lo and (hi is None or total < hi):
                        buckets[i][0] += 1
                        buckets[i][1] += total
                        break
            record = by_id.get(address)
            batch.append((address, kind, str(total), total.to_bytes(32, 'big'), str(available), str(bonded),
                          str(share_e10(total, supply)), txs,
                          mined if record is not None or mined else None,
                          (reward or '0') if rewards_ok else None,
                          json.dumps(sorted(serves.get(address, []))),
                          record['reward'] if record is not None and record['reward'] != address else None,
                          LABELS.get(address)))
            if len(batch) >= 5000:
                conn.executemany('INSERT INTO stage VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)', batch)
                batch = []
        if batch:
            conn.executemany('INSERT INTO stage VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)', batch)

        snap = conn.execute('INSERT INTO snapshots(height, hash, created, counts, stats, registry, winners) '
                            "VALUES(?,?,?,?,'{}','{}','{}')", (h, block_hash, now, json.dumps(counts))).lastrowid
        # Ranks by Total descending, ties by address ascending; a per-type rank in the same order.
        conn.execute('INSERT INTO holders SELECT ?, ROW_NUMBER() OVER (ORDER BY total_key DESC, address ASC), address, '
                     'type, ROW_NUMBER() OVER (PARTITION BY type ORDER BY total_key DESC, address ASC), total, available, '
                     'bonded, share, txs, mined, rewards, miner_ids, reward_address, label FROM stage', (snap,))
        conn.execute('DELETE FROM stage')
        top, sums = [], {}
        for (total,) in conn.execute("SELECT total FROM holders WHERE snap=? AND type!='system' ORDER BY rank LIMIT ?",
                                     (snap, max(TOP_N))):
            top.append(int(total))
        for n in TOP_N:
            value = sum(top[:n])
            sums['top%d' % n] = {'totalWei': str(value), 'sharePercentE10': str(share_e10(value, supply))}

        # The hourly history point (from the first snapshot on; at most HISTORY_POINTS_MAX, the older half thinned).
        newest = conn.execute('SELECT timestamp FROM history ORDER BY height DESC LIMIT 1').fetchone()
        if newest is None or timestamp - newest[0] >= HISTORY_SECONDS:
            conn.execute('INSERT OR REPLACE INTO history VALUES(?,?,?,?,?)',
                         (h, timestamp, stats['count'], str(supply), sums['top10']['sharePercentE10']))
            self._thin_history(conn)
        points = [{'height': a, 'timestamp': b, 'holders': c, 'currentSupplyWei': d, 'top10SharePercentE10': e}
                  for a, b, c, d, e in conn.execute('SELECT height, timestamp, holders, supply, top10 FROM history '
                                                     'ORDER BY height')]
        winners = self._winners(conn, h, WINNERS_WINDOW)
        snapshot = {'height': h, 'hash': block_hash, 'timestamp': timestamp}
        data = {
            'snapshot': snapshot,
            'supply': {'genesisWei': str(self.genesis_wei), 'issuedWei': str(issued), 'mintedWei': str(minted),
                       'baseFeeBurnedWei': str(fees), 'protocolBurnedWei': str(burned),
                       'totalBurnedWei': str(fees + burned), 'currentSupplyWei': str(supply),
                       'circulatingWei': str(circulating), 'availableWei': str(balances), 'bondedWei': str(held_total),
                       'systemWei': str(system_total), 'pendingRewardsWei': str(pending)},
            'verification': {'complete': complete, 'balancesSumWei': str(balances), 'expectedSumWei': str(expected),
                             'unlocatedWei': str(unlocated), 'accountsIndexed': indexed, 'accountsOnChain': on_chain,
                             'accountsMissing': missing},
            'holders': {'count': stats['count'], 'withAvailable': stats['withAvailable'],
                        'withBonded': stats['withBonded'],
                        'new24h': stats['new24h'] if start_24h is not None else None, 'new24hFrom': start_24h},
            'concentration': sums,
            'distribution': [{'minWei': str(lo), 'maxWei': None if hi is None else str(hi), 'accounts': n,
                              'totalWei': str(t)} for (lo, hi), (n, t) in zip(BUCKETS, buckets)],
            'history': {'from': points[0]['height'] if points else None, 'points': points},
            'rewards': {'from': rewards_from, 'to': rewards_to}}
        conn.execute('UPDATE snapshots SET stats=?, registry=?, winners=? WHERE id=?',
                     (json.dumps(data, separators=(',', ':')), json.dumps(registry, separators=(',', ':')),
                      json.dumps(winners, separators=(',', ':')), snap))
        self._retain(conn, now)
        self._set(conn, 'full', 0)
        self._set(conn, 'residual', unlocated)
        if full:
            self._set(conn, 'rescan_at', now + self.rescan_seconds)
        return 'published'

    @staticmethod
    def _thin_history(conn):
        count = conn.execute('SELECT COUNT(*) FROM history').fetchone()[0]
        if count <= HISTORY_POINTS_MAX:
            return
        # Every second point of the older half goes (the first point, the index start, stays).
        heights = [r[0] for r in conn.execute('SELECT height FROM history ORDER BY height LIMIT ?', (count // 2,))]
        conn.executemany('DELETE FROM history WHERE height=?', [(x,) for x in heights[1::2]])

    def _retain(self, conn, now):
        rows = conn.execute('SELECT id, created FROM snapshots ORDER BY id DESC').fetchall()
        for snap, created in rows[self.keep_snapshots:]:
            if now - created > self.keep_seconds:
                conn.execute('DELETE FROM holders WHERE snap=?', (snap,))
                conn.execute('DELETE FROM snapshots WHERE id=?', (snap,))

    @staticmethod
    def _winners(conn, h, window):
        low = max(1, h - window + 1)
        rows = conn.execute('SELECT miner, COUNT(*) FROM recent WHERE height>=? AND height<=? AND miner IS NOT NULL '
                            'GROUP BY miner', (low, h)).fetchall()
        blocks = conn.execute('SELECT COUNT(*) FROM recent WHERE height>=? AND height<=?', (low, h)).fetchone()[0]
        first = conn.execute('SELECT MIN(height) FROM recent WHERE height>=? AND height<=?', (low, h)).fetchone()[0]
        return {'from': first if first is not None else low, 'to': h, 'blocks': blocks,
                'perMiner': {m: n for m, n in sorted(rows)}}

    # ---------------- answers ----------------
    def state(self):
        with self.lock:
            return self.head, self.last_contact, self.failing

    def _index(self, conn):
        tip = self.tip(conn)
        height = -1 if tip is None else tip[0]
        head, contact, failing = self.state()
        now = self.clock()
        if failing or contact is None or now - contact > STALE_SECONDS:
            state = 'reconnecting' if contact is not None or failing else 'syncing'
        else:
            state = 'ready' if height >= head >= 0 else 'syncing'
        return {'state': state, 'head': max(head, height), 'historyHeight': height, 'updatedAt': int(contact or 0)}

    def _snapshot_row(self, conn, pinned=None):
        if pinned is not None:
            row = conn.execute(PAGE_SQL['pinned'], (pinned['height'], pinned['hash'])).fetchone()
            canonical = conn.execute(PAGE_SQL['canonical'], (pinned['height'],)).fetchone()
            if row is None or canonical is None or canonical[0] != pinned['hash']:
                raise SnapshotChanged('The holder snapshot is gone or no longer canonical')
            return row
        row = conn.execute(PAGE_SQL['newest']).fetchone()
        if row is None:
            raise Unavailable('The holders index has no snapshot yet')
        return row

    def page(self, options):
        """The pocol-holders-v1 page for `options` (valid_options). Raises SnapshotChanged when a pinned snapshot is
        gone or no longer canonical and Unavailable before the first snapshot. Pages read by index only: a rank range
        on (snap, rank), a type-rank range on (snap, type, type_rank), or a search's address range on (snap, address,
        type) whose matching ranks are ordered in memory (bounded by the matches, and by PAGE_SECONDS)."""
        if not valid_options(options):
            raise ValueError('Invalid holder options')
        conn = self._reader()
        try:
            conn.execute('BEGIN')
            snap, height, block_hash, counts, stats, _, _ = self._snapshot_row(conn, options.get('snapshot'))
            counts, stats = json.loads(counts), json.loads(stats)
            size, kind, q, order = options['size'], options['type'], options['q'], options['order']
            offset = (options['page'] - 1) * size
            if q:
                # q + 'g' is the smallest string above every lower-case hex string starting with q.
                if kind == 'all':
                    found = conn.execute(PAGE_SQL['search'], (snap, q, q + 'g'))
                else:
                    found = conn.execute(PAGE_SQL['search_type'], (snap, q, q + 'g', kind))
                ranks = sorted(r[0] for r in found)
                total = len(ranks)
                if order == 'balance_asc':
                    ranks.reverse()
                rows = [conn.execute(PAGE_SQL['row'], (snap, r)).fetchone() for r in ranks[offset:offset + size]]
            else:
                # Ranks (and type ranks) are dense from 1, so a page is a key range.
                total = counts[kind]
                if order == 'balance_desc':
                    low, high, way = offset + 1, offset + size, 'desc'
                else:
                    low, high, way = total - offset - size + 1, total - offset, 'asc'
                if kind == 'all':
                    rows = conn.execute(PAGE_SQL['all_' + way], (snap, low, high)).fetchall()
                else:
                    rows = conn.execute(PAGE_SQL['type_' + way], (snap, kind, low, high)).fetchall()
            index = self._index(conn)
            conn.execute('COMMIT')
        finally:
            conn.close()
        return {
            'schema': SCHEMA, 'chainId': self.chain_id, 'genesisHash': self.genesis,
            'page': options['page'], 'size': size, 'order': order, 'type': kind, 'q': q, 'total': total,
            'snapshot': stats['snapshot'], 'index': index,
            'supply': {'currentSupplyWei': stats['supply']['currentSupplyWei']},
            'verification': {'complete': stats['verification']['complete']},
            'rows': [{'rank': r[0], 'address': r[1], 'type': r[2], 'label': r[3], 'minerIds': json.loads(r[4]),
                      'rewardAddress': r[5], 'totalWei': r[6], 'availableWei': r[7], 'bondedWei': r[8],
                      'sharePercentE10': r[9], 'transactions': r[10], 'blocksMined': r[11], 'rewardsWei': r[12]}
                     for r in rows]}

    def stats(self):
        """The pocol-holders-stats-v1 answer of the newest snapshot (one stored row)."""
        conn = self._reader()
        try:
            conn.execute('BEGIN')
            stats = json.loads(self._snapshot_row(conn)[4])
            index = self._index(conn)
            conn.execute('COMMIT')
        finally:
            conn.close()
        return {'schema': STATS_SCHEMA, 'chainId': self.chain_id, 'genesisHash': self.genesis,
                'snapshot': stats['snapshot'], 'index': index, **{k: v for k, v in stats.items() if k != 'snapshot'}}

    def status(self):
        """The follower's state and heights: {state, head, heights: {history, snapshot, rewardsFrom, rewardsTo}}."""
        conn = self._reader()
        try:
            conn.execute('BEGIN')
            index = self._index(conn)
            row = conn.execute('SELECT height FROM snapshots ORDER BY id DESC LIMIT 1').fetchone()
            ok = self._meta(conn, 'rewards_state') == 'ok'
            heights = {'history': index['historyHeight'], 'snapshot': row[0] if row else None,
                       'rewardsFrom': int(self._meta(conn, 'rewards_from')) if ok else None,
                       'rewardsTo': int(self._meta(conn, 'rewards_to')) if ok else None}
            conn.execute('COMMIT')
        finally:
            conn.close()
        return {'state': index['state'], 'head': index['head'], 'heights': heights}

    def registry(self):
        """The newest snapshot's registry: {height, blockHash, thetaInact, records: [{id, reward, status, tomb,
        bondState, lastShare, graceStart, registeredAt, active, inactive}]} (graceStart null when not an ActList
        member without a tomb). Raises Unavailable before the first snapshot."""
        conn = self._reader()
        try:
            conn.execute('BEGIN')
            value = json.loads(self._snapshot_row(conn)[5])
            conn.execute('COMMIT')
        finally:
            conn.close()
        return value

    def winners(self, window=WINNERS_WINDOW):
        """Blocks won per miner ID over the `window` blocks ending at the newest snapshot: {from, to, blocks,
        perMiner: {id: count}} (`blocks` counts the heights the index holds in [from, to])."""
        if type(window) is not int or not 1 <= window <= RECENT_KEEP:
            raise ValueError('Invalid winners window')
        conn = self._reader()
        try:
            conn.execute('BEGIN')
            row = self._snapshot_row(conn)
            value = json.loads(row[6]) if window == WINNERS_WINDOW else self._winners(conn, row[1], window)
            conn.execute('COMMIT')
        finally:
            conn.close()
        return value

    def rewards_by_address(self):
        """Mining rewards since the index start, or None while they are not exact (not started, or after a gap):
        {from, to, addresses: {address: wei}, miners: {id: {paidWei, bondAccruedWei}}} as strings of wei."""
        conn = self._reader()
        try:
            conn.execute('BEGIN')
            if self._meta(conn, 'rewards_state') != 'ok':
                conn.execute('COMMIT')
                return None
            value = {'from': int(self._meta(conn, 'rewards_from')), 'to': int(self._meta(conn, 'rewards_to')),
                     'addresses': {a: w for a, w in conn.execute('SELECT address, wei FROM rewards_address')},
                     'miners': {m: {'paidWei': p, 'bondAccruedWei': c}
                                for m, p, c in conn.execute('SELECT miner, paid, accrued FROM rewards_miner')}}
            conn.execute('COMMIT')
        finally:
            conn.close()
        return value
