#!/usr/bin/env python3
"""Registration fill harness for the launch evidence on a devnet (development/public-network/REHEARSAL.md M1, M3 and
R-off).

Against a running PoCol v3 network over JSON-RPC, it registers up to N fresh identities (default 10,000) with the v3
MinerRegistry, mostly idle zero-bond (ACCRUING) members, some bonded (HELD) ones and a configurable number of ACCRUING
members that mine (each a local `pocol start` node with that identity's key). It reports progress, registrations per
block, failures by reason and the M1 measures the node's RPC exposes. `m3` runs the capacity path at a full set, and
`replace` registers into one named member's seat (R-off).

What it does not do: it changes no consensus rule or parameter, closes no gate and judges no threshold. It records
what the RPC shows; the M1 run is a devnet's, read against REHEARSAL.md.

Signing and keys reuse the release tools: each identity's key is a fresh key file made by `pocol-key generate` in a
harness-owned directory (mode 0700, each file 0600), and every transaction is signed by `pocol-sign --key-file`. This
process never reads a key file's bytes, never prints a key and deletes the keys only on request (`delete-keys`).
Newcomers get their first fee from an operator key the harness is given (`--operator-key FILE`), in batched ordinary
transfers; there is no default operator and the node's own miner key is refused unless explicitly allowed.

The registration rate respects the registry's per-block limits (PoColSystemV3.sol checkBudgets): at most R_max
registrations per block (the profile's registrationPerBlock, 4) within REG_OPS operations, and none past M_max active or
REG_RECORDS_MAX records (Capacity). By default the harness keeps at most R_max of its own registrations in flight (sent
and not yet included), so no block can carry more than R_max of them, except after a reorganization: the node returns an
orphaned block's registrations to its pool beside the R_max sent after it, and the extra ones revert. Its rate is R_max
per block only while a registration sent after a head is included in the next block (half that when inclusion takes two
blocks; --inflight up to 2 x R_max pipelines, at the risk of collisions). Another registrant in the same block can still
use part of the budget. Such a revert is reported as RegOpsLimit (inferred) and retried while the identity's attempts
and funds allow; the registrations-per-block counts hold successful registrations only.

Every transaction is signed and recorded (hash and signed envelope, public data) in state.json before it is sent; one
the node no longer holds is sent again unchanged, never re-signed while a copy may be held (pocold answers a parked one
-32016 "held for the next block"). One command uses a harness directory at a time (its lock file).

Revert reasons: pocold's receipts carry no revert data and the registry's user paths revert BadPhase under eth_call
(their phase check needs the block's own beginBlock), so a reverted registration's reason is inferred from the
public state at the block before it and the block's own registry transactions, and is labelled "(inferred)".

Chain id 2656089850 (the mainnet) is refused unless --allow-mainnet-chain is given; the targets are a devnet of the
rehearsal profile (chain 1656550731) and local devnets. Standard library only; it imports the keccak and registry reader of
deploy/rewards/pocol_rewards.py.
"""
import argparse
import http.client
import json
import math
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import urllib.parse
import weakref
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'deploy/rewards'))
from pocol_rewards import keccak256, REGISTRY, CAPS, ACTIVE_PAGE, ACTIVE_SLICE_GAS, CALL_GAS, RewardUnavailable, _abi_two_arrays  # noqa: E402

MAINNET_CHAIN_ID = 2656089850
REHEARSAL_CHAIN_ID = 1656550731
SCHEMA = 'pocol-registration-fill-v1'

# The gas the native wallet signs these calls with (prototypes/pocold/src/wallet.rs; test_constants_mirror_the_wallet).
REGISTER_GAS = 400_000        # EARNIN_REGISTER_GAS: register(address), any bond 0..B_reg
REPLACE_GAS = 500_000         # REPLACE_GAS: registerReplacing(address,address)
REGISTRY_CALL_GAS = 1_000_000  # REGISTRY_CALL_GAS: deregister, claimBond (and removeInactive here)
TRANSFER_GAS = 21_000
PRIORITY_WEI = 10**9          # pocol-sign's fixed max priority fee per gas
GWEI = 10**9
DEFAULT_FEE_CAP_WEI = 3 * GWEI
DEFAULT_ATTEMPTS = 2
# The node's answer to a valid transaction it parks at the pool's gas bound (prototypes/pocold/src/node.rs POOL_PARKED,
# code -32016): it is held, relayed and a template candidate, and is admitted at a later head. Not a refusal.
PARKED = 'held for the next block'
# Blocks of the canonical chain whose hashes and settlements the harness keeps (in state.json too, so a reorganization
# while it is stopped is undone on the next run): pocold's own reorganization bound, R_KEEP (chain.rs).
REORG_WINDOW = 2048

# CP indices (CP schema v2; the Caps program's get(n), as PoColSystemV3.sol cfg(n) reads them).
CP_INDEX = {'T_blk': 2, 'B_reg': 16, 'R_max': 17, 'M_max': 18, 'M_min': 19, 'REG_RECORDS_MAX': 20, 'REG_OPS': 23,
            'W_act': 24, 'Theta_inact': 25, 'E_win': 26, 'U': 27, 'GAS_LIMIT': 30}
STATUS_LABELS = {0: 'NONE', 1: 'ACTIVE', 2: 'EXITED'}
BOND_STATE_LABELS = {0: 'NONE', 1: 'HELD', 2: 'BURNED', 3: 'PAID', 4: 'ACCRUING'}
# MinerRegistry.removal codes (FN-2b C4).
REMOVAL_LABELS = {0: 'REMOVABLE', 1: 'NOT_LIVE_ACTIVE', 2: 'SET_NOT_FULL', 3: 'AT_MINIMUM', 4: 'RECENTLY_ACTIVE'}
U64 = (1 << 64) - 1
ERRORS = ('BadCalldata()', 'BadValue()', 'Closing()', 'RegOpsLimit()', 'BadState()', 'Capacity()', 'RewardNotDurable()',
          'PaymentFailed()', 'Unauthorized()', 'BadPhase()', 'HeightOverflow()', 'NotRemovable()')
ERROR_SELECTORS = {'0x' + keccak256(e.encode())[:4].hex(): e[:-2] for e in ERRORS}
SIGNATURES = ('register(address)', 'registerReplacing(address,address)', 'removeInactive(address)', 'deregister()',
              'claimBond()', 'reactivate()', 'topUp()', 'setRewardAddress(address)')
SELECTORS = {s: keccak256(s.encode())[:4] for s in SIGNATURES}
REGISTRATION_SELECTORS = {SELECTORS['register(address)'], SELECTORS['registerReplacing(address,address)']}
REREGISTER_KINDS = ('register', 'registerReplacing')   # M3 check 5: the victim registers again (either path)
ADDRESS = re.compile(r'0x[0-9a-f]{40}')


class Refused(Exception):
    """The harness refuses to start (a precondition, never a partial run)."""


class Stopped(Exception):
    """SIGINT or SIGTERM during m3 or replace: what was sent is saved as sent, and nothing more is sent."""


class RpcError(Exception):
    def __init__(self, code, message, data=None):
        super().__init__(f'{code} {message}')
        self.code, self.message, self.data = code, message, data


# pocold answers state reads (eth_call) only within its retained trie window, the last K_hist blocks (32 on the
# rehearsal profile), and -32017 beyond it (rpc.rs state_for).
STATE_NOT_RETAINED = -32017
NOT_RETAINED = 'not retained (K_hist)'


class StateGone(Exception):
    """The registry state at a past height that the harness needs is no longer retained by the node (-32017), and was
    not kept by the harness when it was within the window."""

    def __init__(self, height):
        super().__init__(f'the state at height {height} is {NOT_RETAINED}')
        self.height = height


def word(value):
    return int(value).to_bytes(32, 'big')


def addr_word(address):
    return bytes(12) + bytes.fromhex(address[2:])


def calldata(signature, *args):
    """ABI calldata of a registry call: the selector, then one 32-byte word per argument (address or integer)."""
    out = keccak256(signature.encode())[:4]
    for a in args:
        out += addr_word(a) if isinstance(a, str) else word(a)
    return '0x' + out.hex()


def words(raw_hex, count):
    raw = bytes.fromhex(raw_hex[2:])
    if len(raw) != 32 * count:
        raise RpcError(-32000, f'expected {count} words, got {len(raw)} bytes')
    return [int.from_bytes(raw[i:i + 32], 'big') for i in range(0, len(raw), 32)]


def record_fields(w0, w3=0):
    return {'status': STATUS_LABELS.get(w0 & 255, str(w0 & 255)), 'tomb': bool((w0 >> 8) & 255),
            'bondState': BOND_STATE_LABELS.get((w0 >> 16) & 255, str((w0 >> 16) & 255)),
            'lastShare': (w0 >> 88) & U64, 'registeredAt': (w0 >> 152) & U64,
            'exitHeight': w3 & U64, 'dueHeight': (w3 >> 64) & U64, 'graceStart': w3 >> 192}


def tx_hash(raw):
    """The transaction hash pocold gives a signed envelope: keccak256 of its bytes (lp4-execution wire.rs)."""
    return '0x' + keccak256(bytes.fromhex(raw[2:])).hex()


def without_raw(undo):
    """An undo record as saved. A registry call's signed envelope is kept (public data, within REORG_WINDOW: after a
    reorganization while the harness was stopped it is sent again unchanged); a funding transfer's is not (a dropped
    transfer is made again from the identity's balance)."""
    out = dict(undo)
    if 'item' in out and out['item'].get('kind') == 'fund':
        out['item'] = {k: v for k, v in out['item'].items() if k != 'raw'}
    prev = out.get('prev')
    if isinstance(prev, (list, tuple)) and len(prev) == 2 and isinstance(prev[1], dict):
        out['prev'] = [prev[0], {k: v for k, v in prev[1].items() if k != 'raw'}]
    return out


def percentile(values, p):
    if not values:
        return None
    v = sorted(values)
    return v[min(len(v) - 1, int(len(v) * p / 100))]


# ---------------------------------------------------------------- planning and limits (pure; unit tested)

def check_chain(chain_id, allow_mainnet=False):
    """The chain refusal: the mainnet id only with the explicit flag."""
    if chain_id == MAINNET_CHAIN_ID and not allow_mainnet:
        raise Refused(f'chain id {chain_id} is the mainnet; the harness runs on a devnet (the rehearsal '
                      f'profile\'s chain {REHEARSAL_CHAIN_ID}, or a local devnet; pass --allow-mainnet-chain to '
                      'override explicitly)')
    return chain_id


def default_fund_wei(attempts=DEFAULT_ATTEMPTS, fee_cap=DEFAULT_FEE_CAP_WEI):
    """Each newcomer's first fee: `attempts` registrations at REPLACE_GAS (enough for register or registerReplacing)
    at `fee_cap` wei per gas. At the defaults, 0.003 PCOL."""
    return attempts * REPLACE_GAS * fee_cap


def fund_wei_of(args):
    """--fund-wei, or the default for the run's --attempts and --fee-cap-wei."""
    return args.fund_wei if args.fund_wei is not None else default_fund_wei(args.attempts, args.fee_cap_wei)


def plan(count, cp, active, records, miners=0, bonded=0, bond_wei=None, fund_wei=None, clamp=True, inflight=None,
         fee_cap=DEFAULT_FEE_CAP_WEI, attempts=DEFAULT_ATTEMPTS):
    """The fill plan: how many identities register (never past M_max active or REG_RECORDS_MAX records), their roles,
    the blocks at R_max per block and the operator's total: each identity's funding and bond, and its transfer's fee
    at most (TRANSFER_GAS at the fee cap, which bounds every transfer's max fee). Raises Refused on an impossible
    request."""
    if count < 0 or miners < 0 or bonded < 0:
        raise Refused('counts must be non-negative')
    if miners + bonded > count:
        raise Refused(f'--miners {miners} + --bonded {bonded} exceed --count {count}')
    r_max, m_max, rec_max = cp['R_max'], cp['M_max'], cp['REG_RECORDS_MAX']
    if r_max < 1:
        raise Refused('R_max is 0: the registry takes no registration')
    inflight = r_max if inflight is None else inflight
    if not 1 <= inflight <= 2 * r_max:
        raise Refused(f'--inflight is 1 to 2 x R_max ({2 * r_max})')
    bond = cp['B_reg'] if bond_wei is None else bond_wei
    if not 0 <= bond <= cp['B_reg']:
        raise Refused(f'the bond is at most B_reg ({cp["B_reg"]} wei)')
    fund = default_fund_wei(attempts, fee_cap) if fund_wei is None else fund_wei
    if fund < REGISTER_GAS * PRIORITY_WEI:
        raise Refused(f'--fund-wei {fund} cannot cover one registration ({REGISTER_GAS} gas at the 1 gwei priority fee)')
    seats = max(0, min(m_max - active, rec_max - records))
    planned = count
    if count > seats:
        if not clamp:
            raise Refused(f'{count} registrations requested, but only {seats} seats are free (M_max {m_max}, active {active}, '
                          f'REG_RECORDS_MAX {rec_max}, records {records})')
        planned = seats
    miners_n = min(miners, planned)
    bonded_n = min(bonded, planned - miners_n)
    blocks = math.ceil(planned / r_max) if planned else 0
    fees = planned * TRANSFER_GAS * fee_cap
    total = planned * fund + bonded_n * bond + fees
    return {'requested': count, 'planned': planned, 'clamped': planned < count, 'freeSeats': seats,
            'roles': {'miner': miners_n, 'bonded': bonded_n, 'idle': planned - miners_n - bonded_n},
            'fundWei': fund, 'bondWei': bond, 'transferFeesMaxWei': fees, 'operatorTotalWei': total,
            'rMax': r_max, 'mMax': m_max, 'inflightMax': inflight,
            'perBlockBound': ('R_max (except after a reorganization, which can put an orphaned block\'s registrations '
                              'beside the next ones: the extra ones revert RegOpsLimit and are retried)') if inflight <= r_max
                             else 'none (inflight above R_max: RegOpsLimit reverts are possible and retried)',
            'blocksAtRMax': blocks, 'hoursAtRMax': round(blocks * cp['T_blk'] / 3600, 2)}


def roles(planned_roles):
    """The role of each planned identity in registration order: miners first (they start mining early), then the
    bonded ones, then the idle ones."""
    return (['miner'] * planned_roles['miner'] + ['bonded'] * planned_roles['bonded'] + ['idle'] * planned_roles['idle'])


def registration_slots(r_max, inflight, active, m_max, ready):
    """How many registrations to send now: at most `r_max` of the harness's own in flight (R_max by default, so that no
    block can carry more than R_max of them: every transaction of a block was in flight just before it, unless a
    reorganization returns an orphaned block's registrations to flight after the next ones were sent), never past M_max
    active counting those in flight, and no more than are ready."""
    return max(0, min(r_max - inflight, m_max - active - inflight, ready))


WAITING = 'waiting'
# The node's answers to eth_sendRawTransaction that pool nothing for now (node.rs: POOL_FULL_GAS, POOL_FULL_SENDER,
# '-32016 pool full', POOL_EVICT_UNDERPRICED, POOL_REPLACE_LIMIT; SUBMIT_DEADLINE, rpc.rs -32020; -32021 busy after the
# client's retries). -32016 "held for the next block" (PARKED) is not among them: that one is held.
TRANSIENT_CODES = (-32016, -32020, -32021)
FILL_ROLES = ('miner', 'bonded', 'idle')


def gas_of(kind):
    """The gas a registry call of this kind is signed with."""
    return REPLACE_GAS if kind == 'registerReplacing' else REGISTER_GAS if kind == 'register' else REGISTRY_CALL_GAS


def max_fee_for(base_fee, fee_cap, balance, gas, value=0):
    """(fee, None): the max fee per gas the harness signs with, the wallet's 2 x base + priority capped by --fee-cap-wei
    and by what the identity's balance covers, at least base + priority. (None, WAITING) when the fee cap is below the
    next base fee plus the priority fee (the call waits for the base fee to fall); (None, 'underfunded') when the
    balance cannot cover it."""
    floor = base_fee + PRIORITY_WEI
    want = min(2 * base_fee + PRIORITY_WEI, fee_cap)
    if want < floor:
        return None, WAITING
    afford = (balance - value) // gas if balance > value else 0
    fee = min(want, afford)
    return (fee, None) if fee >= floor else (None, 'underfunded')


def infer_reason(kind, facts):
    """The reason a reverted registry call most likely reverted, from public facts (`facts` keys: gasUsed, gas, rMax,
    regsBefore (successful registrations earlier in its block), opsBefore (successful registry operations earlier in its
    block), regOps, activeBefore, mMax, recordsBefore, recordsMax, senderRecord (the sender had a record), victimCode
    (removalStatus of the victim at the block before)). Every answer but OutOfGas is labelled inferred."""
    if facts.get('gasUsed') is not None and facts.get('gas') is not None and facts['gasUsed'] >= facts['gas']:
        return 'OutOfGas'
    registering = kind in ('register', 'registerReplacing')
    extra = 1 if kind == 'registerReplacing' else 0
    if registering and facts.get('regsBefore', 0) >= facts.get('rMax', 1 << 30):
        return 'RegOpsLimit (inferred: R_max registrations earlier in the block)'
    if kind in ('register', 'registerReplacing', 'removeInactive', 'deregister') and \
            facts.get('opsBefore', 0) + extra >= facts.get('regOps', 1 << 30):
        return 'RegOpsLimit (inferred: REG_OPS operations earlier in the block)'
    if registering and facts.get('senderRecord'):
        return 'BadState (inferred: the sender already has a registry record)'
    if kind == 'registerReplacing':
        code = facts.get('victimCode')
        if code == 1:
            return 'BadState (inferred: the victim is not a live active record)'
        if code in (2, 3, 4):
            return f'NotRemovable (inferred: victim removal code {code} {REMOVAL_LABELS[code]})'
    if kind == 'register' and facts.get('opsBefore', 0) == 0 and (
            facts.get('activeBefore', -1) >= facts.get('mMax', 1 << 30) or facts.get('recordsBefore', -1) >= facts.get('recordsMax', 1 << 62)):
        return 'Capacity (inferred: the set was full at the block before and no registry operation preceded it)'
    if kind == 'removeInactive' and facts.get('victimCode') in (1, 2, 3, 4):
        code = facts['victimCode']
        return f'BadState (inferred: removal code {code} {REMOVAL_LABELS[code]})'
    return 'reverted (reason not observable over RPC)'


# ---------------------------------------------------------------- key directory

def secure_dir(path, create=True):
    """A harness-owned directory with mode 0700: created so, or refused when it is a link, another user's, or open to
    group or others."""
    path = Path(path)
    try:
        if create and not path.exists():
            path.mkdir(mode=0o700, parents=True)
            os.chmod(path, 0o700)
        st = os.lstat(path)
    except OSError as e:
        raise Refused(f'{path} cannot be created or read ({e.strerror}): create it first as the user that runs the harness, '
                      'for example sudo install -d -o "$USER" -m 0700 DIR (the harness and its mining nodes run as that user, '
                      'not as root)') from None
    if stat.S_ISLNK(st.st_mode) or not stat.S_ISDIR(st.st_mode):
        raise Refused(f'{path} must be a real directory')
    if os.name != 'nt':
        if st.st_uid != os.geteuid():
            raise Refused(f'{path} must be owned by this user')
        if st.st_mode & 0o077:
            raise Refused(f'{path} must have mode 0700 (it holds identity keys); it has {oct(st.st_mode & 0o777)}')
    return path


class DirLock:
    """An exclusive lock on a harness directory (its `lock` file), held while one command uses it: a second fill, m3
    or replace on the same directory, stop-miners or delete-keys is refused while it is held."""

    def __init__(self, directory):
        self.fd = os.open(Path(directory) / 'lock', os.O_RDWR | os.O_CREAT, 0o600)
        try:
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(self.fd, msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(self.fd)
            raise Refused(f'{directory} is in use by another harness command (fill, m3 or replace); wait for it or stop it '
                          'first') from None
        self._close = weakref.finalize(self, os.close, self.fd)

    def release(self):
        self._close()


def check_operator_key(path):
    p = Path(path)
    if not p.is_file():
        raise Refused('the operator key file does not exist')
    if os.name != 'nt' and p.stat().st_mode & 0o077:
        raise Refused('the operator key file must not be readable by group or others')
    return p


# ---------------------------------------------------------------- RPC

class Rpc:
    """JSON-RPC over one keep-alive HTTP connection, with the node's busy answer (-32021) retried and each method's
    round-trip times kept (the last 4,096)."""

    def __init__(self, url, timeout=30, clock=time.monotonic, sleep=time.sleep):
        u = urllib.parse.urlsplit(url)
        if u.scheme != 'http' or not u.hostname:
            raise Refused('--rpc must be an http:// URL of the node (its RPC is loopback-only)')
        self.host, self.port, self.path = u.hostname, u.port or 80, u.path or '/'
        self.timeout, self.clock, self.sleep, self.conn = timeout, clock, sleep, None
        self.latency, self.calls, self.errors = {}, 0, 0

    def _post(self, body):
        for attempt in range(2):
            try:
                if self.conn is None:
                    self.conn = http.client.HTTPConnection(self.host, self.port, timeout=self.timeout)
                self.conn.request('POST', self.path, body, {'Content-Type': 'application/json'})
                r = self.conn.getresponse()
                return json.loads(r.read())
            except (OSError, http.client.HTTPException, ValueError):
                if self.conn is not None:
                    self.conn.close()
                self.conn = None
                if attempt:
                    raise
        raise AssertionError('unreachable')

    def call(self, method, params=()):
        for _ in range(40):
            t0 = self.clock()
            v = self._post(json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}))
            lat = self.latency.setdefault(method, [])
            lat.append((self.clock() - t0) * 1000)
            del lat[:-4096]
            self.calls += 1
            if 'error' in v:
                e = v['error']
                if e.get('code') == -32021:
                    self.sleep((e.get('data') or {}).get('retryAfterMs', 250) / 1000)
                    continue
                self.errors += 1
                raise RpcError(e.get('code'), e.get('message', ''), e.get('data'))
            return v['result']
        raise RpcError(-32021, f'{method}: the node stayed busy')

    def latency_summary(self):
        return {m: {'count': len(v), 'p50Ms': round(percentile(v, 50), 2), 'p99Ms': round(percentile(v, 99), 2),
                    'maxMs': round(max(v), 2)} for m, v in sorted(self.latency.items()) if v}


class Chain:
    """The registry and node reads the harness uses (every one a documented pocold RPC method or a registry view)."""

    def __init__(self, rpc):
        self.rpc = rpc

    def eth_call(self, to, data, tag='latest', gas=CALL_GAS):
        return self.rpc.call('eth_call', [{'to': to, 'data': data, 'gas': hex(gas)}, tag])

    def cp(self, tag='latest'):
        return {k: words(self.eth_call(CAPS, calldata('get(uint256)', i), tag), 1)[0] for k, i in CP_INDEX.items()}

    def lengths(self, tag='latest'):
        records, active, _, bonds = words(self.eth_call(REGISTRY, calldata('lengths()'), tag), 4)
        return {'records': records, 'active': active, 'bondTotalWei': bonds}

    def record(self, address, tag='latest'):
        w0, w1, w2, w3 = words(self.eth_call(REGISTRY, calldata('recordWords(address)', address), tag), 4)
        return None if w0 == 0 else {**record_fields(w0, w3), 'bondWei': w2, 'reward': '0x%040x' % w1}

    def removal(self, victim, tag='latest'):
        return words(self.eth_call(REGISTRY, calldata('removalStatus(address)', victim), tag), 1)[0]

    def census(self, tag=None):
        """The active list by bond state (activeSlice pages of ACTIVE_PAGE, as the reward gateway reads it), every page
        read at one height (the head when no tag is given; the census is read again at a new head if the pages
        disagree, as when that block is reorganized away during the read)."""
        for attempt in range(3):
            try:
                return self._census(tag or hex(self.head()))
            except RewardUnavailable:
                if tag or attempt == 2:
                    raise
        raise AssertionError('unreachable')

    def _census(self, tag):
        active = self.lengths(tag)['active']
        counts, ids = {}, set()
        for start in range(0, active, ACTIVE_PAGE):
            n = min(ACTIVE_PAGE, active - start)
            raw = bytes.fromhex(self.eth_call(REGISTRY, calldata('activeSlice(uint256,uint256)', start, ACTIVE_PAGE), tag,
                                              ACTIVE_SLICE_GAS)[2:])
            got, w0s = _abi_two_arrays(raw, n)
            for a, w0 in zip(got, w0s):
                label = BOND_STATE_LABELS.get((w0 >> 16) & 255, '?')
                counts[label] = counts.get(label, 0) + 1
                ids.add(a.lower() if isinstance(a, str) else '0x%040x' % a)
        return {'active': active, 'byBondState': counts}, ids

    def head(self):
        return int(self.rpc.call('eth_blockNumber'), 16)

    def block(self, n, full=False):
        return self.rpc.call('eth_getBlockByNumber', [hex(n), full])

    def next_base_fee(self):
        # eth_gasPrice is the next block's base fee plus the 1 gwei priority (rpc.rs).
        return max(0, int(self.rpc.call('eth_gasPrice'), 16) - PRIORITY_WEI)

    def balance(self, a, tag='pending'):
        return int(self.rpc.call('eth_getBalance', [a, tag]), 16)

    def nonce(self, a, tag='pending'):
        return int(self.rpc.call('eth_getTransactionCount', [a, tag]), 16)


# ---------------------------------------------------------------- signing (pocol-key, pocol-sign)

class Signer:
    """The release's key and signing tools. Their output is an address or a signed envelope, never a key."""

    def __init__(self, bin_dir, chain_id):
        exe = '.exe' if os.name == 'nt' else ''
        self.key_tool, self.sign_tool = Path(bin_dir) / ('pocol-key' + exe), Path(bin_dir) / ('pocol-sign' + exe)
        for t in (self.key_tool, self.sign_tool):
            if not t.is_file():
                raise Refused(f'{t} not found (pass --bin-dir, the directory of the release binaries)')
        self.chain_id = chain_id

    @staticmethod
    def _run(cmd):
        # In a session of its own: a terminal's Ctrl-C reaches the harness (which then stops between steps), not the
        # tool in the middle of a signature.
        p = subprocess.run([str(c) for c in cmd], capture_output=True, text=True, timeout=60, stdin=subprocess.DEVNULL,
                           start_new_session=True)
        if p.returncode:
            # The tools write no secret to stderr (keyfile.rs); only its first line is kept.
            raise RuntimeError((p.stderr.strip().splitlines() or ['failed'])[0][:200])
        return p.stdout.strip()

    def generate(self, path):
        out = self._run([self.key_tool, 'generate', '--out', path]).lower()
        if not ADDRESS.fullmatch(out):
            raise RuntimeError('pocol-key returned no address')
        return out

    def address(self, key):
        out = self._run([self.sign_tool, '--key-file', key, '--chain-id', self.chain_id, '--address']).lower()
        if not ADDRESS.fullmatch(out):
            raise RuntimeError('pocol-sign returned no address')
        return out

    def sign(self, key, to, nonce, gas, max_fee, value=0, data=None):
        cmd = [self.sign_tool, '--key-file', key, '--chain-id', self.chain_id, '--to', to, '--value', value,
               '--nonce', nonce, '--gas', gas, '--max-fee', max_fee]
        if data:
            cmd += ['--data', data]
        raw = self._run(cmd)
        if not re.fullmatch(r'0x[0-9a-fA-F]+', raw):
            raise RuntimeError('pocol-sign returned no transaction')
        return raw


# ---------------------------------------------------------------- the harness

class Harness:
    def __init__(self, args, rpc=None, signer=None, sleep=time.sleep, out=sys.stdout, now=time.time):
        self.args, self.sleep, self.out, self.now = args, sleep, out, now
        self.rpc = rpc or Rpc(args.rpc)
        self.chain = Chain(self.rpc)
        self.chain_id = check_chain(int(self.rpc.call('eth_chainId'), 16), args.allow_mainnet_chain)
        if args.expect_chain_id is not None and self.chain_id != args.expect_chain_id:
            raise Refused(f'the node is on chain {self.chain_id}, not --expect-chain-id {args.expect_chain_id}')
        self.genesis = self.rpc.call('eth_getBlockByNumber', ['0x0', False])['hash']
        self.dir = secure_dir(args.dir)
        # One command at a time on a directory (status only reads it).
        self.lock = DirLock(self.dir) if getattr(args, 'command', None) != 'status' else None
        self.keys = secure_dir(self.dir / 'keys')
        self._signer = signer
        self.state_path = self.dir / 'state.json'
        self.state = self._load()
        if self.state.get('keysDeleted') and self.lock is not None:
            self.close()
            raise Refused(f'the keys of {self.dir} were deleted (delete-keys); use a fresh --dir')
        self.cp = self.chain.cp()
        self.inflight = {}
        self.stopping = False
        self.blocks = {}       # height -> {ts, attempt, txs, ours}
        # height -> the hash of the block the harness processed (or settled from) there, and height -> what settling each
        # of the harness's transactions there changed (undone on a reorg). Both are kept for the last REORG_WINDOW
        # heights and saved with the state, so a block orphaned while the harness was stopped is undone when it starts.
        view = self.state.get('chainView') or {}
        self.hashes = {int(h): x for h, x in (view.get('hashes') or {}).items()}
        self.settled = {int(h): x for h, x in (view.get('settled') or {}).items()}
        self.samples = []
        self.last_block = None
        self.head = None
        self.progress_at = 0
        self.next_sample = 0
        self.waiting = None
        self.miner_procs = []   # this run's mining nodes (they outlive it; stop-miners stops them)
        self.poll()

    def close(self):
        if self.lock is not None:
            self.lock.release()

    @property
    def signer(self):
        if self._signer is None:
            self._signer = Signer(self.args.bin_dir, self.chain_id)
        return self._signer

    # -- persistent public state
    def _load(self):
        if self.state_path.exists():
            s = json.loads(self.state_path.read_text(encoding='utf-8'))
            if s.get('schema') != SCHEMA:
                raise Refused(f'{self.state_path} is not a harness state file')
            if s['chainId'] != self.chain_id or s['genesisHash'] != self.genesis:
                raise Refused(f'{self.dir} belongs to chain {s["chainId"]} genesis {s["genesisHash"]}, not this node\'s network')
            return s
        return {'schema': SCHEMA, 'chainId': self.chain_id, 'genesisHash': self.genesis, 'identities': [], 'miners': [],
                'perBlock': {}, 'runs': []}

    def save(self):
        self.state['chainView'] = {'hashes': {str(h): x for h, x in self.hashes.items()},
                                   'settled': {str(h): [without_raw(u) for u in us] for h, us in self.settled.items()}}
        tmp = self.state_path.with_suffix('.tmp')
        tmp.write_text(json.dumps(self.state, separators=(',', ':'), sort_keys=True), encoding='utf-8')
        os.replace(tmp, self.state_path)

    def say(self, text):
        print(text, file=self.out, flush=True)

    # -- identities
    def new_identity(self, role, bond=0):
        i = len(self.state['identities'])
        if role == 'miner':
            home = secure_dir(self.dir / 'miners' / f'{i:06d}')
            key = home / 'identity.pocol-key'
        else:
            key = self.keys / f'{i:06d}.pocol-key'
        if os.path.lexists(key):
            # A key file an interrupted run generated but never recorded (identities are recorded before any of them is
            # funded or used): adopted, or replaced when it is incomplete.
            if not stat.S_ISREG(os.lstat(key).st_mode):
                raise Refused(f'{key} is not a regular file')
            try:
                address = self.signer.address(key)
            except RuntimeError:
                os.unlink(key)
                address = self.signer.generate(key)
        else:
            address = self.signer.generate(key)
        ident = {'i': i, 'address': address, 'key': str(key.relative_to(self.dir)), 'role': role, 'bondWei': bond,
                 'nonce': 0, 'fundedWei': 0, 'fund': None, 'attempts': [], 'registeredAt': None, 'outcome': None}
        self.state['identities'].append(ident)
        return ident

    def key_of(self, ident):
        return self.dir / ident['key']

    # -- operator
    def operator(self):
        if not self.args.operator_key:
            raise Refused('--operator-key FILE is required to fund newcomers (there is no default operator key)')
        key = check_operator_key(self.args.operator_key)
        address = self.signer.address(key)
        if not self.args.allow_member_operator:
            miner = (self.rpc.call('pocol_status').get('miner') or '').lower()
            if address == miner:
                raise Refused('the operator key is this node\'s miner key; give a separate funding key '
                              '(or --allow-member-operator to use it explicitly)')
            if self.chain.record(address) is not None:
                raise Refused('the operator key has a registry record (a member\'s key); give a separate funding key '
                              '(or --allow-member-operator to use it explicitly)')
        return key, address

    # -- sending
    def send(self, raw):
        """(hash, None) when the node pooled the transaction or holds it parked (pocold answers a valid transaction it
        parks at the pool's gas bound with -32016 '... held for the next block': it is relayed and admitted at a later
        head, so it is in flight like a pooled one); (None, 'waiting: ...') when the node did not pool it for now (its
        pool is full in any of the -32016 answers, its compute deadline passed, -32020, or it stayed busy, -32021:
        nothing was pooled or relayed); else (None, the refusal)."""
        tx = tx_hash(raw)
        try:
            got = self.rpc.call('eth_sendRawTransaction', [raw])
        except RpcError as e:
            if e.code == -32016 and PARKED in (e.message or ''):
                return tx, None
            if e.code in TRANSIENT_CODES:
                return None, f'{WAITING}: not pooled for now ({(e.message or "")[:160]})'
            return None, f'refused by the node: {e.message[:160]}'
        return (got.lower() if isinstance(got, str) else tx), None

    def track(self, tx, kind, ident=None, **extra):
        """`extra` holds the sender and nonce (how expire() tells a pooled transaction from a dropped one), the gas and
        the signed envelope (sent again, unchanged, when the node no longer holds it)."""
        self.inflight[tx] = {'kind': kind, 'i': None if ident is None else ident['i'], 'sentAt': self.head, **extra}

    def inflight_of(self, kinds):
        """The harness identities with a transaction of these kinds in flight (an identity's transactions share a nonce,
        so at most one of them is included)."""
        return len({v['i'] for v in self.inflight.values() if v['kind'] in kinds})

    # -- blocks and receipts
    def poll(self):
        """Processes every block up to the head: the harness's included transactions, block timing and attempts. A
        reorganization (a processed height whose block hash changed, or a head below it) is undone back to the fork:
        what was settled from the orphaned blocks returns to flight, and the new blocks are processed."""
        head = self.chain.head()
        if self.last_block is None:
            self.start(head)
        fork = self.find_fork(head)
        if fork is not None:
            self.state['reorgsSeen'] = self.state.get('reorgsSeen', 0) + 1
            self.rewind(fork)
        n = self.last_block + 1
        while n <= head:
            b = self.chain.block(n)
            if b is None:
                break
            parent = self.hashes.get(n - 1)
            if parent is not None and b.get('parentHash') not in (None, parent):
                break   # the chain moved during this poll; the next one finds the fork
            self.hashes[n] = b['hash']
            info = {'ts': int(b['timestamp'], 16), 'attempt': (b.get('pocol') or {}).get('attempt'),
                    'signed': (b.get('pocol') or {}).get('signed'), 'txs': len(b['transactions']), 'ours': 0}
            self.blocks[n] = info
            for tx in b['transactions']:
                if tx in self.inflight:
                    self.settle(tx, n)
                    info['ours'] += 1
            self.last_block = n
            n += 1
        self.head = self.last_block
        low = self.head - REORG_WINDOW
        for d in (self.hashes, self.settled):
            for h in [h for h in d if h < low]:
                del d[h]
        kept = self.state.get('stateAt') or {}
        for k in [k for k, v in kept.items() if v['height'] < low]:
            del kept[k]
        return self.head

    def start(self, head):
        """The first poll of a run. Every hash kept from an earlier run is compared with the canonical chain (at most
        REORG_WINDOW block reads), and a block orphaned while the harness was stopped is undone back to the fork. The
        run then processes the blocks from the highest height still canonical (at most REORG_WINDOW below the head) to
        the head, so that the recorded hashes are one linked chain, as find_fork's shortcut needs."""
        if not self.hashes:
            self.last_block = head
            b = self.chain.block(head)
            if b is not None:
                self.hashes[head] = b['hash']
            return
        self.last_block = max(self.hashes)
        fork = self.find_fork(head, full=True)
        if fork is not None:
            self.state['reorgsSeen'] = self.state.get('reorgsSeen', 0) + 1
            self.rewind(fork)
        self.last_block = min(head, max(self.last_block, head - REORG_WINDOW))
        if self.last_block not in self.hashes:
            b = self.chain.block(self.last_block)
            if b is not None:
                self.hashes[self.last_block] = b['hash']

    def find_fork(self, head, full=False):
        """The lowest recorded height whose block is no longer canonical (or above the head), or None. It descends from
        the highest one and stops at the first that still matches: the recorded hashes are one linked chain (start()
        and poll() record them so, and nothing else adds one), so that block's ancestors are canonical too, and nothing
        moved costs one block read. `full` compares every recorded height (a run's start, where the hashes kept by the
        earlier run are not linked to the head)."""
        fork = None
        for h in sorted(self.hashes, reverse=True):
            if h <= head:
                b = self.chain.block(h)
                if b is not None and b['hash'] == self.hashes[h]:
                    if full:
                        continue
                    break
            fork = h
        return fork

    def rewind(self, fork):
        for h in sorted((h for h in self.settled if h >= fork), reverse=True):
            for undo in reversed(self.settled.pop(h)):
                self.unsettle(undo)
        for h in [h for h in self.hashes if h >= fork]:
            del self.hashes[h]
            self.blocks.pop(h, None)
        kept = self.state.get('stateAt') or {}
        for k in [k for k, v in kept.items() if v['height'] >= fork]:
            del kept[k]   # (past() reads a height again whenever its block changed; these are orphaned)
        self.last_block = min(self.last_block, fork - 1)

    def settle(self, tx, n):
        """Records an included transaction's outcome at canonical height `n` (and how to undo it on a reorg)."""
        # n is a processed height (n <= last_block): its hash is recorded, unless it is below what the harness keeps.
        item = self.inflight.pop(tx)
        rc = self.rpc.call('eth_getTransactionReceipt', [tx])
        ok = rc is not None and int(rc['status'], 16) == 1
        gas_used = int(rc['gasUsed'], 16) if rc else None
        ident = None if item['i'] is None else self.state['identities'][item['i']]
        undo = {'tx': tx, 'item': item, 'height': n}
        self.settled.setdefault(n, []).append(undo)
        if item['kind'] == 'fund':
            undo['prev'] = (ident['fundedWei'], ident['fund'])
            if ok:
                ident['fundedWei'] = item['have'] + item['value']
                ident['fund'] = {'tx': tx, 'block': n}
            else:
                ident['fund'] = {'tx': tx, 'block': n, 'failed': True}
            return
        # Every attempt with this hash: a retry signed at the same nonce and fee is the same envelope (pocol-sign signs
        # deterministically), so the transaction included is each of them, including one given up earlier.
        indices = [k for k, a in enumerate(ident['attempts']) if a['tx'] == tx]
        attempt = ident['attempts'][indices[-1]]
        undo['attempts'] = indices
        undo['before'] = [{k: ident['attempts'][j][k] for k in ('status', 'reason', 'handled') if k in ident['attempts'][j]}
                          for j in indices[:-1]]
        for j in indices:
            a = ident['attempts'][j]
            for k in ('raw', 'reason', 'handled'):
                a.pop(k, None)
            a.update({'block': n, 'status': 1 if ok else 0, 'gasUsed': gas_used})
        if ok:
            if item['kind'] in ('register', 'registerReplacing'):
                undo['prev'] = (ident['registeredAt'], ident['outcome'])
                ident['registeredAt'] = n
                ident['outcome'] = 'registered'
                per = self.state['perBlock']
                per[str(n)] = per.get(str(n), 0) + 1
            if item['kind'] == 'registerReplacing' and item.get('victim'):
                # The registry exited the victim in this transaction: a harness victim no longer holds its seat.
                victim = next((v for v in self.state['identities'] if v['address'] == item['victim']), None)
                if victim is not None and victim['outcome'] == 'registered':
                    undo['victimPrev'] = (victim['i'], victim['outcome'])
                    victim['outcome'] = 'replaced (m3)' if getattr(self.args, 'command', None) == 'm3' else 'replaced'
        else:
            attempt['reason'] = self.reason_for(item, ident, rc, n)
        self.keep_settled(item, n)

    def unsettle(self, undo):
        """Undoes settle() for a transaction whose block was orphaned: it is in flight again."""
        n = undo['height']
        if undo.get('record'):
            # An outcome taken from the registry record (reconcile): it is read again when the identity is next settled.
            ident = self.state['identities'][undo['i']]
            ident['registeredAt'], ident['outcome'] = undo['prev']
            ident.pop('reconciledFromRecord', None)
            self.per_block_add(n, -1)
            return
        item = undo['item']
        # Checked at the next expire(): the node usually pools an orphaned block's transactions again (pooled); one it
        # does not hold is sent again unchanged, which cannot conflict with a copy held elsewhere.
        item['recheckAt'] = None
        item['sentAt'] = None
        self.inflight[undo['tx']] = item
        ident = None if item['i'] is None else self.state['identities'][item['i']]
        if item['kind'] == 'fund':
            ident['fundedWei'], ident['fund'] = undo['prev']
            return
        indices = undo['attempts'] if 'attempts' in undo else [undo['attempt']]
        for j in indices:
            attempt = ident['attempts'][j]
            for k in ('block', 'status', 'gasUsed', 'reason', 'handled'):
                attempt.pop(k, None)
        # The latest attempt with the hash is in flight again; earlier ones (given up before) are as they were.
        for j, before in zip(indices[:-1], undo.get('before') or []):
            ident['attempts'][j].update(before)
        if item.get('raw'):
            ident['attempts'][indices[-1]]['raw'] = item['raw']
        if 'prev' in undo:
            ident['registeredAt'], ident['outcome'] = undo['prev']
            self.per_block_add(n, -1)
        if 'victimPrev' in undo:
            i, outcome = undo['victimPrev']
            self.state['identities'][i]['outcome'] = outcome

    def per_block_add(self, n, k):
        per = self.state['perBlock']
        per[str(n)] = per.get(str(n), 0) + k
        if not per[str(n)]:
            del per[str(n)]

    def past(self, what, height, *args):
        """A registry read at a past height (`what`: lengths, record or removal), kept in state.json (stateAt) with the
        hash of the block at that height and reused while that block is canonical: pocold answers state reads only
        within the last K_hist blocks (-32017 beyond them: StateGone), and a resumed run may decide a step long after
        its block. A height whose block changed is read again; a value is kept only when the block at that height was
        the same before and after the read."""
        key = f'{what}:{":".join(args)}@{height}'
        kept = self.state.setdefault('stateAt', {})
        b = self.chain.block(height)
        if b is not None and (kept.get(key) or {}).get('hash') == b['hash']:
            return kept[key]['value']
        try:
            value = getattr(self.chain, what)(*args, hex(height))
        except RpcError as e:
            if e.code == STATE_NOT_RETAINED:
                raise StateGone(height) from None
            raise
        if b is not None and (self.chain.block(height) or {}).get('hash') == b['hash']:
            kept[key] = {'height': height, 'hash': b['hash'], 'value': value}
        return value

    def keep_before(self, prepared):
        """m3: before a registry call is sent at the head, the state its block would be decided from if the next block
        includes it (the facts reason_for and the M3 checks read at the block before), kept by past()."""
        if getattr(self.args, 'command', None) != 'm3' or self.head is None:
            return
        try:
            self.past('lengths', self.head)
            self.past('record', self.head, prepared['ident']['address'])
            if prepared.get('victim'):
                self.past('removal', self.head, prepared['victim'])
        except StateGone:
            pass

    def keep_settled(self, item, n):
        """m3: while settling a registry call at height n (usually the head), the state its M3 decision reads at a past
        height, kept by past(): a run stopped before deciding the step decides it from these, later than K_hist."""
        if getattr(self.args, 'command', None) != 'm3':
            return
        try:
            if item['kind'] == 'deregister':
                self.past('lengths', n)
            elif item['kind'] == 'removeInactive' and item.get('victim'):
                self.past('removal', n - 1, item['victim'])
            elif item['kind'] == 'registerReplacing' and item.get('victim'):
                self.past('record', n, item['victim'])
            elif item['kind'] == 'register':
                self.past('lengths', n - 1)
            if item['kind'] in REREGISTER_KINDS and item.get('sender'):
                self.past('record', n - 1, item['sender'])   # check 5: the victim's record before it registers again
        except StateGone:
            pass

    def reason_for(self, item, ident, rc, n):
        """The inferred revert reason of a settled call (infer_reason). The facts read from the state at the block before
        come from past(); when that state is no longer retained only what the block's receipts show is inferred (OutOfGas,
        the registration limits), and otherwise the reason says the state is not retained."""
        try:
            facts = {'gasUsed': int(rc['gasUsed'], 16) if rc else None, 'gas': item.get('gas'), 'rMax': self.cp['R_max'],
                     'regOps': self.cp['REG_OPS'], 'mMax': self.cp['M_max'], 'recordsMax': self.cp['REG_RECORDS_MAX']}
            gone = None
            try:
                before = self.past('lengths', n - 1)
                facts.update({'activeBefore': before['active'], 'recordsBefore': before['records']})
                facts['senderRecord'] = self.past('record', n - 1, ident['address']) is not None
                if item.get('victim'):
                    facts['victimCode'] = self.past('removal', n - 1, item['victim'])
            except StateGone as e:
                gone = e.height
                for k in ('activeBefore', 'recordsBefore', 'senderRecord', 'victimCode'):
                    facts.pop(k, None)
            b = self.chain.block(n, True)
            index = int(rc['transactionIndex'], 16) if rc else len(b['transactions'])
            regs = ops = 0
            for t in b['transactions'][:index]:
                if (t.get('to') or '').lower() != REGISTRY:
                    continue
                r = self.rpc.call('eth_getTransactionReceipt', [t['hash']])
                if r is None or int(r['status'], 16) != 1:
                    continue
                sel = bytes.fromhex((t.get('input') or '0x')[2:10])
                if sel == SELECTORS['reactivate()']:
                    continue   # reactivate draws on R_react, not REG_OPS (V3-09)
                ops += 2 if sel == SELECTORS['registerReplacing(address,address)'] else 1
                regs += 1 if sel in REGISTRATION_SELECTORS else 0
            facts.update({'regsBefore': regs, 'opsBefore': ops})
            reason = infer_reason(item['kind'], facts)
            if gone is not None and reason == 'reverted (reason not observable over RPC)':
                reason = f'reverted (reason not observable: state at {gone} {NOT_RETAINED})'
            return reason
        except RpcError as e:
            return f'reverted (state not readable: {e.message[:80]})'

    def wait_blocks(self, count=1, timeout=None):
        """Waits until `count` more blocks are processed (or until the next block when count is 1)."""
        start = self.head
        deadline = self.now() + (timeout or max(120, count * self.cp['T_blk'] * 6))
        while self.poll() < start + count:
            if self.stopping or self.now() > deadline:
                return False
            self.sleep(self.args.poll)
        return True

    def wait_included(self, txs, blocks):
        """m3 and replace: True when the transactions are settled, False after `blocks` blocks; raises Stopped on a
        stop (what was sent stays recorded as sent, to be settled by the next run)."""
        start = self.head
        while any(t in self.inflight for t in txs):
            self.expire()
            if not any(t in self.inflight for t in txs):
                break
            if self.head > start + blocks:
                return False
            self.step(1)
        return True

    def step(self, count=1, timeout=None):
        """m3 and replace: wait_blocks, then Stopped on a stop."""
        ok = self.wait_blocks(count, timeout)
        if self.stopping:
            raise Stopped('stopped; what was sent is recorded in state.json and settled by the next run')
        return ok

    # -- funding
    def fund(self, idents, op_key, op_addr, need):
        """Ordinary transfers from the operator to each identity short of `need(ident)` wei (sequential nonces, one batch
        at a time, each signed at most at --fee-cap-wei). Returns the transfers sent, or None while the fee cap is below
        the next base fee plus the priority fee (nothing is signed). The caller has no operator transfer in flight: the
        operator's nonce is read at 'pending', which does not count a parked transfer, and a second transfer at a
        nonce whose first may still be held is never signed."""
        if self.inflight_of(('fund',)):
            raise RuntimeError('an operator transfer is in flight (settle_transfers first)')
        short = []
        for ident in idents:
            want = need(ident)
            have = self.chain.balance(ident['address'])
            if have < want:
                short.append((ident, want - have, have))
            else:
                ident['fundedWei'] = max(ident['fundedWei'], have)
        if not short:
            return []
        base = self.chain.next_base_fee()
        max_fee = min(2 * base + PRIORITY_WEI, self.args.fee_cap_wei)
        if max_fee < base + PRIORITY_WEI:
            self.waiting = f'{WAITING}: the next base fee {base} plus the priority fee is above --fee-cap-wei {self.args.fee_cap_wei}'
            return None
        nonce = self.chain.nonce(op_addr)
        if self.chain.balance(op_addr) < sum(v for _, v, _ in short) + len(short) * TRANSFER_GAS * max_fee:
            raise Refused(f'the operator {op_addr} cannot cover this funding batch')
        # Written ahead: the batch is signed and recorded (with each transfer's hash) before any of it is sent, so a run
        # that stops while sending finds every transfer it may have sent.
        batch = []
        for ident, value, have in short:
            raw = self.signer.sign(op_key, ident['address'], nonce + len(batch), TRANSFER_GAS, max_fee, value=value)
            batch.append((ident, raw, {'tx': tx_hash(raw), 'raw': raw, 'nonce': nonce + len(batch), 'value': value, 'have': have,
                                       'sender': op_addr}))
        for ident, _, rec in batch:
            ident['fund'] = rec
        self.save()
        sent = []
        for k, (ident, raw, rec) in enumerate(batch):
            tx, err = self.send(raw)
            if tx is None:
                self.state.setdefault('fundRefusals', []).append(err)
                for later, _, _ in batch[k:]:   # this one and the later nonces were not pooled
                    later['fund'] = None
                break
            rec['tx'] = tx
            self.track(tx, 'fund', ident, value=rec['value'], have=rec['have'], sender=op_addr, nonce=rec['nonce'], raw=raw)
            sent.append(tx)
        self.save()
        return sent

    # -- registry transactions
    def prepare(self, ident, kind, data, gas, value=0, victim=None):
        """A signed registry call of `ident` (not yet sent), or the reason it cannot be signed."""
        base = self.chain.next_base_fee()
        balance = self.chain.balance(ident['address'])
        fee, why = max_fee_for(base, self.args.fee_cap_wei, balance, gas, value)
        if why == WAITING:
            return None, f'{WAITING}: the next base fee {base} plus the priority fee is above --fee-cap-wei {self.args.fee_cap_wei}'
        if fee is None:
            reason = f'underfunded (balance {balance} wei below {gas} gas at the base fee {base} plus the priority fee)'
            ident['attempts'].append({'tx': None, 'kind': kind, 'sentAt': self.head, 'reason': reason})
            return None, reason
        nonce = self.chain.nonce(ident['address'])
        raw = self.signer.sign(self.key_of(ident), REGISTRY, nonce, gas, fee, value=value, data=data)
        return {'ident': ident, 'raw': raw, 'kind': kind, 'gas': gas, 'nonce': nonce, 'fee': fee, 'victim': victim}, None

    def record_ahead(self, prepared):
        """Records a signed registry call as an attempt, with its hash and envelope, before it is sent (the caller saves
        the state, then sends): a run that stops while sending finds it, and resume() settles it, keeps it in flight or
        sends the same envelope again; it never signs another transaction at that nonce while the first may be held."""
        attempt = {'tx': tx_hash(prepared['raw']), 'kind': prepared['kind'], 'sentAt': self.head, 'maxFee': prepared['fee'],
                   'nonce': prepared['nonce'], 'raw': prepared['raw']}
        if prepared['victim']:
            attempt['victim'] = prepared['victim']
        prepared['ident']['attempts'].append(attempt)
        prepared['attempt'] = attempt
        self.keep_before(prepared)   # (saved with the attempt)

    def submit(self, prepared):
        ident = prepared['ident']
        if 'attempt' not in prepared:
            self.record_ahead(prepared)
            self.save()
        attempt = prepared['attempt']
        tx, err = self.send(prepared['raw'])
        if tx is None and err.startswith(WAITING):
            # Not pooled, so held nowhere: no attempt (the identity signs again at a later head, at the same nonce).
            ident['attempts'][:] = [x for x in ident['attempts'] if x is not attempt]
            del prepared['attempt']
            return None, err
        if tx is None:
            attempt.pop('raw', None)
            attempt.update({'tx': None, 'reason': err})
            return None, err
        attempt['tx'] = tx   # the node's answer (the same hash as recorded ahead, keccak256 of the envelope)
        ident['nonce'] = prepared['nonce'] + 1
        self.track(tx, prepared['kind'], ident, gas=prepared['gas'], victim=prepared['victim'], sender=ident['address'],
                   nonce=prepared['nonce'], raw=prepared['raw'])
        return tx, None

    def submit_all(self, prepared):
        """Several signed calls: all recorded and saved first, then sent back to back."""
        for p in prepared:
            self.record_ahead(p)
        self.save()
        return [self.submit(p) for p in prepared]

    def registry_tx(self, ident, kind, data, gas, value=0, victim=None):
        """m3 and replace: one registry call, signed and sent; signed again at a later head while the fee cap or the
        node's pool make it wait (at most --wait-blocks, then recorded as not sent)."""
        for _ in range(self.args.wait_blocks + 1):
            prepared, err = self.prepare(ident, kind, data, gas, value, victim)
            if prepared is not None:
                tx, err = self.submit(prepared)
                if tx is not None or not err.startswith(WAITING):
                    return tx, err
            elif not err.startswith(WAITING):
                return None, err
            self.step(1)
        ident['attempts'].append({'tx': None, 'kind': kind, 'sentAt': self.head, 'reason': f'not sent ({err})'})
        return None, err

    def register_args(self, ident):
        return ident, 'register', calldata('register(address)', ident['address']), REGISTER_GAS, ident['bondWei']

    def register(self, ident):
        return self.registry_tx(*self.register_args(ident))

    def replacing_args(self, ident, victim):
        return (ident, 'registerReplacing', calldata('registerReplacing(address,address)', ident['address'], victim), REPLACE_GAS,
                ident['bondWei'], victim)

    def register_replacing(self, ident, victim):
        return self.registry_tx(*self.replacing_args(ident, victim))

    # -- measures (M1, over RPC)
    def sample(self):
        s = {'height': self.head, 'wall': round(self.now(), 3)}
        try:
            s.update(self.chain.lengths())
            st = self.rpc.call('pocol_status')
            m = st.get('members') or {}
            s['members'] = {k: m.get(k) for k in ('reconstructions', 'reconstructUsP99', 'reconstructUsMax', 'faults', 'oracleMismatch')}
            s['membership'] = st.get('membership')
            s['diskObserved'] = st.get('diskObserved')
            s['a13BoundBytes'] = (st.get('a13') or {}).get('boundBytes')
            s['reorgs'] = (st.get('stats') or {}).get('reorgs')
            ret = st.get('retention') or {}
            s['maxReorgDepth'], s['reorgsGeKAlarm'] = ret.get('maxReorgDepth'), ret.get('reorgsGeKAlarm')
            s['pool'] = st.get('pool')
            caps = self.rpc.call('pocol_getCaps')
            s['accounts'] = int(caps['accounts'], 16)
            s['accountsPercent'] = ((caps.get('occupancy') or {}).get('accounts') or {}).get('percent')
        except (RpcError, KeyError, TypeError) as e:
            s['error'] = str(e)[:160]
        if self.args.node_pid:
            s['rssBytes'] = rss_of(self.args.node_pid)
        self.samples.append(s)
        return s

    def measures(self):
        hs = sorted(self.blocks)
        intervals = [self.blocks[b]['ts'] - self.blocks[a]['ts'] for a, b in zip(hs, hs[1:]) if b == a + 1]
        attempts = {}
        for h in hs:
            a = str(self.blocks[h]['attempt'])
            attempts[a] = attempts.get(a, 0) + 1
        out = {'blocksObserved': len(hs), 'fromHeight': hs[0] if hs else None, 'toHeight': hs[-1] if hs else None,
               'blockIntervalS': {'mean': round(sum(intervals) / len(intervals), 3) if intervals else None,
                                  'p50': percentile(intervals, 50), 'p99': percentile(intervals, 99),
                                  'max': max(intervals) if intervals else None},
               'attempts': attempts, 'unsignedBlocks': sum(1 for h in hs if self.blocks[h]['signed'] is False),
               'rpcLatency': self.rpc.latency_summary(), 'samples': len(self.samples)}
        disk = [s for s in self.samples if s.get('diskObserved')]
        if len(disk) >= 2 and disk[-1]['wall'] > disk[0]['wall']:
            days = (disk[-1]['wall'] - disk[0]['wall']) / 86400
            out['historyGrowthBytesPerDay'] = round((disk[-1]['diskObserved']['historyBytes'] - disk[0]['diskObserved']['historyBytes']) / days)
            out['stateGrowthBytesPerDay'] = round((disk[-1]['diskObserved']['stateBytes'] - disk[0]['diskObserved']['stateBytes']) / days)
        recon = [s['members']['reconstructUsP99'] for s in self.samples if (s.get('members') or {}).get('reconstructUsP99') is not None]
        if recon:
            out['memberReconstructUsP99Max'] = max(recon)
        rss = [s['rssBytes'] for s in self.samples if s.get('rssBytes')]
        if rss:
            out['rssPeakBytes'] = max(rss)
        out['notMeasuredHere'] = [
            'child_ctx p99: pocold does not expose child_ctx timing over RPC; members.reconstructUsP99/Max (pocol_status, '
            'the member-set reconstruction time of the last samples) is recorded as the nearest RPC figure',
            'gateway reward computation time (deploy/rewards; time the gateway itself)',
            'RSS, unless --node-pid names the node process on this host (read from /proc, not RPC)',
            'T4/T5 pass or miss: the raw attempts, unsigned blocks and reorg counters are recorded for comparison with the '
            'network\'s baseline']
        return out

    def failures(self):
        out = {}
        for ident in self.state['identities']:
            for a in ident['attempts']:
                if a.get('status') == 0 or a.get('tx') is None:
                    r = re.sub(r' \(.*', '', a.get('reason') or 'unknown')
                    out[r] = out.get(r, 0) + 1
            if ident['outcome'] and ident['outcome'] != 'registered':
                r = re.sub(r' \(.*', '', ident['outcome'])
                out['final: ' + r] = out.get('final: ' + r, 0) + 1
        return out

    def per_block_histogram(self):
        h = {}
        for v in self.state['perBlock'].values():
            h[str(v)] = h.get(str(v), 0) + 1
        return h

    def progress(self, force=False):
        if not force and self.head < self.progress_at:
            return
        self.progress_at = self.head + self.args.progress_every
        ids = self.state['identities']
        reg = sum(1 for i in ids if i['outcome'] == 'registered')
        L = self.chain.lengths()
        self.say(f'height {self.head} registered {reg}/{len(ids)} inflight reg {self.inflight_of(("register", "registerReplacing"))} '
                 f'fund {self.inflight_of(("fund",))} active {L["active"]}/{self.cp["M_max"]} records {L["records"]} '
                 f'failures {json.dumps(self.failures(), sort_keys=True)}' + (f' ({self.waiting})' if self.waiting else ''))

    def write_report(self, extra=None):
        ids = self.state['identities']
        report = {'schema': SCHEMA + '-report', 'chainId': self.chain_id, 'genesisHash': self.genesis, 'cp': self.cp,
                  'identities': len(ids), 'registered': sum(1 for i in ids if i['outcome'] == 'registered'),
                  'byRole': {r: sum(1 for i in ids if i['role'] == r and i['outcome'] == 'registered') for r in ('miner', 'bonded', 'idle')},
                  'registrationsPerBlock': self.per_block_histogram(),
                  'maxRegistrationsInABlock': max(self.state['perBlock'].values(), default=0),
                  'failuresByReason': self.failures(), 'measures': self.measures(), 'samples': self.samples,
                  'miners': self.state['miners'], 'note': 'Recorded over RPC by the registration fill harness; it judges '
                  'no threshold and closes no gate (development/public-network/REHEARSAL.md).'}
        if extra:
            report.update(extra)
        path = self.dir / 'report.json'
        path.write_text(json.dumps(report, indent=1, sort_keys=True), encoding='utf-8')
        return report

    # -- fill
    def fill_identities(self):
        return [i for i in self.state['identities'] if i['role'] in FILL_ROLES]

    def run_fill(self):
        a = self.args
        self.poll()
        if not a.dry_run:
            op_key, op_addr = self.operator()
            # Before the plan: what an interrupted run left in flight is settled (or kept in flight) first, so that its
            # registrations count as done and their seats are not planned again.
            self.resume()
            self.poll()
            # And an identity an earlier run recorded as failed whose given-up transaction was included after all.
            for ident in self.fill_identities():
                if (ident['outcome'] or '').startswith(('failed', 'not sent')):
                    self.reconcile(ident)
        L = self.chain.lengths()
        existing = [i for i in self.fill_identities() if i['outcome'] == 'registered']
        # max(0, ...): an identity recorded as failed whose transaction a peer included after all is registered too,
        # so a resumed fill can hold more than --count; the same command then plans nothing more.
        p = plan(max(0, a.count - len(existing)), self.cp, L['active'], L['records'],
                 max(0, a.miners - sum(1 for i in existing if i['role'] == 'miner')),
                 max(0, a.bonded - sum(1 for i in existing if i['role'] == 'bonded')), a.bond_wei, fund_wei_of(a),
                 clamp=not a.no_clamp, inflight=a.inflight, fee_cap=a.fee_cap_wei, attempts=a.attempts)
        p['registeredBefore'] = len(existing)
        if len(existing) > a.count:
            p['countExceededBy'] = len(existing) - a.count
        # What the operator still sends: an identity of an interrupted run that is not registered yet keeps what it was
        # funded with (fund() tops it up to its need, and sends nothing to one already funded), and each new identity
        # needs its whole funding, bond and transfer fee. On a fresh run this is the plan's total.
        # An operator transfer the interrupted run left in flight and that the node's pending layer holds (its nonce below
        # the operator's at 'pending') is already paid in the operator's balance at 'pending', which pocold reads on the
        # pool's layer: its identity counts as funded by it, so that it is charged once. One the pending layer does not
        # hold (parked, or not held) is not paid there, and is charged.
        todo = [i for i in self.fill_identities() if i['outcome'] is None]
        pooled = {}
        if not a.dry_run:
            op_pending = self.chain.nonce(op_addr, 'pending')
            pooled = {v['i']: v['have'] + v['value'] for v in self.inflight.values()
                      if v['kind'] == 'fund' and v.get('nonce') is not None and v['nonce'] < op_pending}
        fund_need = lambda i: p['fundWei'] + i['bondWei']
        fee = TRANSFER_GAS * a.fee_cap_wei
        short = [max(0, fund_need(i) - max(i['fundedWei'], pooled.get(i['i'], 0))) for i in todo]
        p['operatorRemainingWei'] = sum(s + fee for s in short if s) + sum(
            p['fundWei'] + (p['bondWei'] if r == 'bonded' else 0) + fee for r in roles(p['roles'])[len(todo):])
        self.say('plan ' + json.dumps(p, sort_keys=True))
        if a.dry_run:
            return {'plan': p}
        if self.chain.balance(op_addr) < p['operatorRemainingWei']:
            raise Refused(f'the operator {op_addr} holds less than the {p["operatorRemainingWei"]} wei the plan still sends')
        for k, role in enumerate(roles(p['roles'])[len(todo):]):
            if self.stopping:
                break   # a stop while the keys are made (each identity made so far is recorded, and resumed)
            self.new_identity(role, p['bondWei'] if role == 'bonded' else 0)
            if k % 256 == 255:
                self.save()
        self.save()
        self.state['runs'].append({'command': 'fill', 'startHeight': self.head, 'plan': p})
        self.sample()
        fill_ids = self.fill_identities()
        while not self.stopping:
            self.poll()
            self.expire()
            self.waiting = None
            busy = {v['i'] for v in self.inflight.values()}
            # Reverted or dropped registrations: retried while their attempts and funds allow, else final (never while
            # another transaction of the identity is in flight).
            for ident in fill_ids:
                last = ident['attempts'][-1] if ident['attempts'] else None
                if ident['outcome'] is None and last and last.get('status') == 0 and not last.get('handled') and ident['i'] not in busy:
                    if self.retire_or_retry(ident, last.get('reason')):
                        last['handled'] = True
            pending = [i for i in fill_ids if i['outcome'] is None]
            regs_inflight = self.inflight_of(('register',))
            if not pending and not self.inflight:
                break
            # Funding: one batch in flight at a time, a buffer of funded identities ahead of the registrations.
            if not self.inflight_of(('fund',)):
                unfunded = [i for i in pending if i['fundedWei'] < fund_need(i) and i['i'] not in busy]
                if unfunded and len(pending) - len(unfunded) < 2 * a.fund_batch:
                    self.fund(unfunded[:a.fund_batch], op_key, op_addr, fund_need)
            # Registrations: at most R_max identities in flight, never past M_max; signed, recorded and saved, then sent.
            L = self.chain.lengths()
            ready = [i for i in pending if i['fundedWei'] >= fund_need(i) and i['i'] not in busy and i['outcome'] is None]
            prepared = []
            for ident in ready[:registration_slots(p['inflightMax'], regs_inflight, L['active'], self.cp['M_max'], len(ready))]:
                pr, err = self.prepare(*self.register_args(ident))
                if pr is None and err.startswith(WAITING):
                    self.waiting = err
                    break
                if pr is None:
                    self.retire_or_retry(ident, err)
                else:
                    prepared.append(pr)
            if prepared:
                for (tx, err), pr in zip(self.submit_all(prepared), prepared):
                    if tx is None and err.startswith(WAITING):
                        self.waiting = err   # not pooled, nothing held: signed again at a later head
                    elif tx is None:
                        self.retire_or_retry(pr['ident'], err)
            # The seats are gone (another registrant may have taken some): nothing more can register plainly.
            if L['active'] >= self.cp['M_max'] and not regs_inflight:
                for ident in pending:
                    if ident['i'] not in busy and ident['outcome'] is None:
                        ident['outcome'] = 'not sent: Capacity (the set is full)'
            if self.head >= self.progress_at:
                self.progress()
            if self.head >= self.next_sample:
                self.next_sample = self.head + a.sample_every
                self.sample()
            self.start_miners()
            self.save()
            self.wait_blocks(1)
        self.poll()
        if not self.stopping:
            # An identity recorded as failed or not sent that the registry holds after all (a transaction of its own that
            # was given up and still included): registered, from its receipt or else its record.
            for ident in fill_ids:
                if (ident['outcome'] or '').startswith(('failed', 'not sent')):
                    self.reconcile(ident)
            self.poll()
            self.start_miners()   # a mining identity registered in the run's last block
        self.sample()
        self.progress(force=True)
        self.state['runs'][-1]['endHeight'] = self.head
        census, ids = self.chain.census()
        ours = [i for i in self.state['identities'] if i['outcome'] == 'registered']
        extra = {'census': census, 'registeredAndActiveAtEnd': sum(1 for i in ours if i['address'] in ids), 'plan': p}
        self.save()
        return self.write_report(extra)

    def resume(self):
        """A run after an interrupted one: each transaction it recorded (every one is recorded, with its hash and signed
        envelope, before it is sent) without an outcome is settled from its receipt, kept in flight while the node holds
        it, or sent again unchanged (check)."""
        for ident in self.state['identities']:
            f = ident.get('fund')
            if f and f.get('tx') and f.get('sender') and 'block' not in f and not f.get('dropped') and f['tx'] not in self.inflight:
                self.track(f['tx'], 'fund', ident, value=f['value'], have=f['have'], sender=f['sender'], nonce=f['nonce'], raw=f.get('raw'))
                self.check(f['tx'])
            for a in list(ident['attempts']):
                if not a.get('tx') or 'status' in a or a['tx'] in self.inflight:
                    continue
                self.track(a['tx'], a['kind'], ident, gas=gas_of(a['kind']), victim=a.get('victim'), sender=ident['address'],
                           nonce=a.get('nonce'), raw=a.get('raw'))
                self.inflight[a['tx']]['sentAt'] = a.get('sentAt') or self.head
                self.check(a['tx'])

    def fate(self, tx, sender, nonce):
        """('included', height) from the canonical receipt; ('pooled', None) while the pending layer of the node's pool
        holds the nonce (pocold's eth_getTransactionByHash and receipts answer only for canonical blocks, never for the
        pool, so the pool is read as the sender's nonce at 'pending' above 'latest'); ('replaced', None) when another
        transaction used the nonce; ('missing', None) when the pending nonce is at it. A missing transaction may still be
        held: parked (outside the pending layer), in the revalidation backlog after a new head, or in a peer's pool."""
        latest = self.chain.nonce(sender, 'latest')
        rc = self.rpc.call('eth_getTransactionReceipt', [tx])
        if rc is not None:
            return 'included', int(rc['blockNumber'], 16)
        if nonce is not None and latest > nonce:
            return 'replaced', None
        pending = self.chain.nonce(sender, 'pending')
        return ('pooled', None) if pending > (latest if nonce is None else nonce) else ('missing', None)

    def record_of(self, v, tx):
        """The recorded attempt (or funding transfer) of an in-flight transaction."""
        if v['i'] is None:
            return None
        ident = self.state['identities'][v['i']]
        if v['kind'] == 'fund':
            return ident['fund'] if (ident.get('fund') or {}).get('tx') == tx else None
        return next((a for a in reversed(ident['attempts']) if a['tx'] == tx), None)

    def check(self, tx):
        """Settles, keeps or gives up one in-flight transaction by its fate. A missing one is sent again unchanged (the
        same envelope and hash, so it can never conflict with a copy the node or a peer still holds; pocold answers a
        pooled or parked copy as held, and settles its revalidation backlog before it answers). It is given up only when
        the node refused it twice, --include-timeout blocks apart; the identity may then sign another at that nonce."""
        v = self.inflight[tx]
        what, n = self.fate(tx, v['sender'], v.get('nonce'))
        if what == 'included':
            if n <= self.last_block:
                self.settle(tx, n)   # a block this harness processed before the transaction was (re)tracked
            return what
        if what == 'pooled':
            v.pop('stale', None)
            v['recheckAt'] = self.head + self.args.include_timeout
            return what
        record = self.record_of(v, tx)
        if what == 'missing':
            sent, why = self.send(v['raw']) if v.get('raw') else (None, 'refused: its signed envelope is not kept')
            if sent is None and why.startswith(WAITING):
                # The node's pool is full for now: kept in flight (a peer may hold it) and sent again later; this is
                # not one of the two refusals that give it up.
                v['recheckAt'] = self.head + self.args.include_timeout
                return WAITING
            if sent is not None:
                v.pop('stale', None)
                v['recheckAt'] = self.head + self.args.include_timeout
                if record is not None:
                    record['resent'] = record.get('resent', 0) + 1
                return 'resent'
            if not v.get('stale'):
                v['stale'] = why
                v['recheckAt'] = self.head + self.args.include_timeout
                return 'stale'
            what, reason = 'dropped', f'not included (dropped by the node; sent again and {why})'
        else:
            reason = 'not included (its nonce was used by another transaction)'
        del self.inflight[tx]
        if v['i'] is not None:
            ident = self.state['identities'][v['i']]
            if v['kind'] == 'fund':
                if record is not None:
                    ident['fund'] = {'tx': tx, 'dropped': True}
            elif record is not None:
                for a in ident['attempts']:
                    if a['tx'] == tx and 'status' not in a:   # the same envelope signed twice is one transaction
                        a.pop('raw', None)
                        a.update({'status': 0, 'reason': reason})
                if what == 'replaced':
                    self.revive(ident, v.get('nonce'))
        return what

    def revive(self, ident, nonce=None):
        """An attempt of `ident` recorded as not included whose receipt shows it in a canonical block (the transaction
        that used its nonce, or one included after it was given up): tracked again and settled from that block."""
        for a in ident['attempts']:
            if not a.get('tx') or a['tx'] in self.inflight or a.get('status') == 1 or 'block' in a:
                continue
            if nonce is not None and a.get('nonce') != nonce:
                continue
            rc = self.rpc.call('eth_getTransactionReceipt', [a['tx']])
            if rc is None:
                continue
            for k in ('status', 'reason', 'handled'):
                a.pop(k, None)
            self.track(a['tx'], a['kind'], ident, gas=gas_of(a['kind']), victim=a.get('victim'), sender=ident['address'],
                       nonce=a.get('nonce'))
            n = int(rc['blockNumber'], 16)
            if n <= self.last_block:
                self.settle(a['tx'], n)   # else poll() settles it when it processes that block

    def reconcile(self, ident):
        """Before (and after) a final failure: True when the identity is registered after all, from the receipt of one of
        its own transactions (revive), or else from its registry record (an outcome the harness took from no transaction
        of its own, marked reconciledFromRecord, at the record's registeredAt)."""
        self.revive(ident)
        if ident['outcome'] == 'registered' or any(v['i'] == ident['i'] for v in self.inflight.values()):
            return True
        # Read at the last processed height, so that registeredAt is a height whose hash is recorded (a reorganization
        # of it is then undone); a registration in a later block is found at the next reconcile.
        rec = self.chain.record(ident['address'], hex(self.last_block))
        if rec is None:
            return False
        n = rec['registeredAt']
        self.settled.setdefault(n, []).append({'record': True, 'i': ident['i'], 'height': n,
                                               'prev': (ident['registeredAt'], ident['outcome'])})
        ident.update({'registeredAt': n, 'outcome': 'registered', 'reconciledFromRecord': True})
        self.per_block_add(n, 1)
        return True

    def retire_or_retry(self, ident, reason):
        """After a refused or reverted registration: retry while attempts remain and the reason can pass, else record the
        final outcome (unless the identity is registered after all: reconcile). Nothing is decided while a transaction
        of the identity is in flight; False then (decided later)."""
        if any(v['i'] == ident['i'] for v in self.inflight.values()):
            return False
        tries = len(ident['attempts'])
        final = reason and (reason.startswith('Capacity') or reason.startswith('BadState') or reason.startswith('underfunded'))
        if final or tries >= self.args.attempts:
            if not self.reconcile(ident):
                ident['outcome'] = f'failed: {reason}'
        return True

    def expire(self):
        """A transaction not included within --include-timeout blocks is checked by its fate (check): settled when its
        receipt shows a canonical block, re-checked after another timeout while the node holds it, sent again unchanged
        when it is missing, and given up only when the node refuses it twice."""
        for tx, v in list(self.inflight.items()):
            if tx not in self.inflight:
                continue   # settled by an earlier check (revive)
            if v['sentAt'] is not None and self.head - v['sentAt'] <= self.args.include_timeout:
                continue   # (None: due now, as after a reorg)
            if v.get('recheckAt') is not None and self.head < v['recheckAt']:
                continue
            self.check(tx)

    # -- mining identities (local pocol start nodes)
    def start_miners(self):
        if not self.args.network:
            return
        running = {m['i'] for m in self.state['miners']}
        for ident in self.state['identities']:
            if ident['role'] != 'miner' or ident['outcome'] != 'registered' or ident['i'] in running:
                continue
            k = len(self.state['miners'])
            p2p, rpc = self.args.miner_port_base + 2 * k, self.args.miner_port_base + 2 * k + 1
            home = (self.dir / ident['key']).parent.resolve()   # absolute: stop-miners finds it from any --dir spelling
            log = open(home / 'node.log', 'ab')
            cmd = [str(Path(self.args.bin_dir) / ('pocol' + ('.exe' if os.name == 'nt' else ''))), 'start', '--home', str(home),
                   '--network', str(self.args.network), '--listen', f'127.0.0.1:{p2p}', '--rpc', f'127.0.0.1:{rpc}']
            for peer in self.args.peer or []:
                cmd += ['--peer', peer]
            proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
            log.close()
            self.miner_procs.append(proc)
            self.state['miners'].append({'i': ident['i'], 'address': ident['address'], 'pid': proc.pid,
                                         'home': str(home), 'p2p': p2p, 'rpc': rpc, 'startedAt': self.head})
            self.say(f'miner {ident["address"]} started (pid {proc.pid}, rpc 127.0.0.1:{rpc})')
            self.save()

    # -- M3: the capacity path at a full set
    def settle_transfers(self):
        """m3 and replace: waits until no operator transfer is in flight (one an earlier, stopped run left parked or
        refused once is kept, sent again or given up by check), so that fund() never signs at a nonce still held."""
        txs = [t for t, v in self.inflight.items() if v['kind'] == 'fund']
        if txs and not self.wait_included(txs, 2 * self.args.include_timeout + 2):
            raise RuntimeError('an earlier operator transfer is still in flight (it stays recorded; run again later)')

    def fresh_funded(self, op_key, op_addr, count, need):
        idents = [self.new_identity('m3') for _ in range(count)]
        self.save()
        self.ensure_funds(op_key, op_addr, idents, need)
        return idents

    def ensure_funds(self, op_key, op_addr, idents, need):
        """Funds `idents` to `need` wei each and waits for the transfers; a transfer the node refuses is made again at a
        later head. Raises when an identity is still short after --include-timeout tries."""
        for _ in range(self.args.include_timeout + 1):
            self.settle_transfers()
            before = len(self.state.get('fundRefusals', []))
            txs = self.fund(idents, op_key, op_addr, lambda i: need)
            if txs and not self.wait_included(txs, self.args.include_timeout):
                raise RuntimeError('the funding transfers were not included')
            if txs == [] and len(self.state.get('fundRefusals', [])) == before:
                return   # every identity holds `need`
            if txs:
                continue   # included: fund() reads the balances again
            self.step(1)   # the fee cap is below the base fee, or the node refused a transfer: a later head
        short = [i['address'] for i in idents if self.chain.balance(i['address']) < need]
        if short:
            last = (self.state.get('fundRefusals') or [self.waiting or 'not included'])[-1]
            raise RuntimeError(f'{len(short)} identities could not be funded to {need} wei ({last})')

    def removable_harness(self, want, exclude=()):
        """Up to `want` members the registry could remove at the next block (pocol_getRemovable, as the wallet lists
        them), each confirmed by the registry's removalStatus view: harness idle members unless --victims any."""
        rem = [x.lower() for x in self.rpc.call('pocol_getRemovable').get('removable', [])]
        ours = {i['address'] for i in self.state['identities'] if i['role'] == 'idle' and i['outcome'] == 'registered'}
        out = []
        for x in rem:
            if len(out) == want:
                break
            if x not in exclude and (x in ours or self.args.victims == 'any') and self.chain.removal(x) == 0:
                out.append(x)
        return out

    def wait_removable(self, want, exclude=()):
        for _ in range(self.args.wait_blocks + 1):
            victims = self.removable_harness(want, exclude)
            if len(victims) >= want:
                return victims[:want]
            self.step(1)
        raise RuntimeError(f'fewer than {want} removable harness members after {self.args.wait_blocks} blocks '
                           '(members become removable Theta_inact blocks after their last share or registration)')

    def included(self, what, tx, err, confirm=False):
        """m3 and replace: a call that decides a check was sent and included, or RuntimeError (exit 1, and no result is
        written): only an included transaction decides a check, never a refusal or one still in flight after
        --include-timeout blocks (it stays recorded in flight, and the next run settles it). With `confirm` (m3), its
        block is also at the node's safe height (m3_confirm)."""
        if tx is None:
            raise RuntimeError(f'the {what} was refused or not sent: {err}')
        if not self.wait_included([tx], self.args.include_timeout):
            raise RuntimeError(f'the {what} was not included within --include-timeout blocks (it stays recorded in flight)')
        if confirm:
            self.m3_confirm(what, [tx])

    def settled_at(self, tx):
        """The canonical height the harness settled `tx` at, or None (in flight, given up, or never settled). One settled
        below the heights the harness keeps (REORG_WINDOW, pocold's reorganization bound) is read from its record."""
        h = next((h for h, us in self.settled.items() for u in us if u.get('tx') == tx), None)
        if h is None and tx not in self.inflight:
            h = next((a['block'] for i in self.state['identities'] for a in i['attempts']
                      if a.get('tx') == tx and a.get('block') is not None and a['block'] < self.head - REORG_WINDOW), None)
        return h

    def still_canonical(self, h):
        """The block the harness processed at height h is the canonical one (a height below the ones it keeps is
        beyond pocold's reorganization bound)."""
        if h < self.head - REORG_WINDOW:
            return True
        return self.hashes.get(h) is not None and self.hashes[h] == (self.chain.block(h) or {}).get('hash')

    def safe_height(self):
        """The node's RPC `safe` height (pocol_finality: max(head - kSafe, finalized), kSafe 6 by default), a local,
        probabilistic confirmation depth, not consensus finality."""
        safe = self.rpc.call('pocol_finality')['safe']
        return int(safe, 16) if isinstance(safe, str) else int(safe)

    def m3_confirm(self, what, txs):
        """m3: a step is decided, and the next one sent, only once every block that includes its transactions is at or
        below the node's safe height and is still the block the harness settled them from. The drill sends each step
        once the one before is decided, so a reorganization of at most kSafe blocks (two miners' sibling blocks) can
        neither undo a decided step nor replay the drill's steps out of order or into one block. A transaction that such a
        reorganization returns to flight is waited for again (--include-timeout blocks), and its new block confirmed in
        turn. A deeper reorganization is read as before (m3_rewound, m3_disorder). Returns whether any of the blocks
        changed meanwhile; RuntimeError when a transaction is no longer included (it stays recorded) or its block is not
        safe within --wait-blocks blocks."""
        first = {t: self.settled_at(t) for t in txs}
        first = {t: (h, self.hashes.get(h)) for t, h in first.items()}
        start = self.head
        while True:
            if any(t in self.inflight for t in txs):
                if not self.wait_included(txs, self.args.include_timeout):
                    raise RuntimeError(f'the {what} was returned to flight by a reorganization and not included again within '
                                       '--include-timeout blocks (it stays recorded in flight)')
            heights = [self.settled_at(t) for t in txs]
            if None in heights:
                raise RuntimeError(f'the {what} is no longer included after a reorganization (not decided; its record is kept)')
            safe = self.safe_height()
            if all(h <= safe and self.still_canonical(h) for h in heights):
                return any(first[t] != (h, self.hashes.get(h)) for t, h in zip(txs, heights))
            if self.head > start + self.args.wait_blocks:
                raise RuntimeError(f'the {what}\'s block did not reach the node\'s safe height within --wait-blocks blocks')
            self.step(1)

    def m3_attempt(self, ident, kind, since=0):
        """m3: the latest attempt of `ident` of this kind (or of one of these kinds), from its `since`-th on, that a
        block included, or None."""
        kinds = (kind,) if isinstance(kind, str) else kind
        return next((x for x in reversed(ident['attempts'][since:]) if x['kind'] in kinds and 'block' in x), None)

    def m3_hash(self, attempt):
        """m3: the canonical hash of the block that included `attempt` (as settled), or None when it is not included."""
        b = self.chain.block(attempt['block']) if attempt and attempt.get('block') is not None else None
        return b['hash'] if b else None

    def m3_replacing(self, reps, victims):
        """M3 check 1 from what the chain shows, and the newcomers' outcomes: a newcomer is registered and its harness
        victim replaced (settle) only when its attempt succeeded; failed only when it reverted or was given up; one still
        in flight keeps no outcome (a later run settles it)."""
        at = [next((x for x in reversed(r['attempts']) if x['kind'] == 'registerReplacing'), {}) for r in reps]
        blocks = {x.get('block') for x in at}
        after = {v: self.m3_victim_after(v, x) if x.get('block') else None for v, x in zip(victims, at)}
        for ident, x in zip(reps, at):
            if x.get('status') == 1:
                ident['role'] = 'idle'
            elif x.get('status') == 0:
                ident['role'] = 'm3'
                ident['outcome'] = f'failed: {x.get("reason") or "not included"}'
        return {'pass': all(x.get('status') == 1 for x in at) and len(blocks) == 1 and None not in blocks and len(set(victims)) == 2
                        and all(r and r['status'] == 'EXITED' for r in after.values()),
                'block': at[0].get('block'), 'blocks': sorted(b for b in blocks if b is not None), 'victims': victims,
                'statuses': [x.get('status') for x in at], 'victimRecords': after,
                'reasons': [x.get('reason') for x in at if x.get('status') == 0], 'blockHashes': [self.m3_hash(x) for x in at]}

    def m3_victim_after(self, victim, att):
        """Check 1: the victim's record at the block that includes `att` (past). When that state is no longer retained,
        the latest record if it shows the exit at that block (a later state proves it); else StateGone (not graded)."""
        try:
            return self.past('record', att['block'], victim)
        except StateGone:
            latest = self.chain.record(victim)
            if att.get('status') == 1 and latest and latest['status'] == 'EXITED' and latest['exitHeight'] == att['block']:
                return latest
            raise

    def m3_register_alone(self, plain, active_before, err=None):
        """M3 check 2 from the included register at the full set. Its reason is inferred from the state at the block
        before: when that is no longer retained, the check is not graded (StateGone)."""
        att = self.m3_attempt(plain, 'register') or next((x for x in reversed(plain['attempts']) if x['kind'] == 'register'), {})
        if NOT_RETAINED in (att.get('reason') or ''):
            raise StateGone(att['block'] - 1)
        if plain['outcome'] != 'registered':
            plain['outcome'] = 'm3: register at a full set'
        return {'pass': att.get('status') == 0 and (att.get('reason') or '').startswith('Capacity'), 'activeBefore': active_before,
                'block': att.get('block'), 'status': att.get('status'), 'reason': att.get('reason') or err,
                'blockHash': self.m3_hash(att)}

    def m3_remove_inactive(self, run, remover, rm):
        """M3 check 3 from the included removeInactive, and the target's outcome."""
        target = run['target']
        code_before = self.past('removal', rm['block'] - 1, target) if rm.get('block') else None
        remover['outcome'] = 'm3: removeInactive below a full set'
        self.m3_target_outcome(target, rm)
        return {'pass': run['activeAfterDeregister'] < self.cp['M_max'] and rm.get('status') == 0 and code_before == 2,
                'deregisterBlock': run['deregisterBlock'], 'activeAfterDeregister': run['activeAfterDeregister'], 'target': target,
                'block': rm.get('block'), 'status': rm.get('status'), 'removalCodeBefore': code_before, 'reason': rm.get('reason'),
                'blockHash': self.m3_hash(rm)}

    def m3_target_outcome(self, target, rm):
        """The removeInactive target's outcome from the included removeInactive `rm`."""
        for ident in self.state['identities']:
            if ident['address'] == target:
                if rm.get('status') == 1:
                    ident['outcome'] = 'removed (m3)'
                elif ident['outcome'] == 'removed (m3)':
                    ident['outcome'] = 'registered'   # an earlier settlement from an orphaned block

    def m3_disorder(self, run, undone=()):
        """How the canonical chain breaks the order the drill sent its transactions in, or None. A drill sends each step
        once the one before is included, so each lands in a later block than the one before: check 1's two
        registerReplacing in one block, then check 2's register, the deregister, the removeInactive, the register
        into the free seat and, with --reregister-after-u, the victim's registration again. A reorganization can include them again in another order or together in one block (pocold
        pools an orphaned block's transactions again, so they usually go into the next block together), and then a
        receipt no longer tests its rule: a removeInactive in the deregister's block reads the removal code of the full
        set before it, and a register in check 1's block reverts for the block's registration limit, not Capacity.
        Only steps included so far are read (one not included yet has not been sent, or is decided by its phase); check
        2's register that is not included breaks the order, and so does check 1 outside one block when a reorganization
        undid it (`undone`, from m3_rewound: one the drill's own run saw outside one block stays its graded result).
        Reads state.json only."""
        ids = self.state['identities']
        last, prev = None, None
        if run.get('replacers'):
            at = [self.m3_attempt(ids[i], 'registerReplacing') for i in run['replacers']]
            blocks = {x['block'] for x in at if x is not None}
            if 'twoReplacingSameBlock' in undone and (None in at or len(blocks) != 1):
                return 'check 1\'s two registerReplacing are not included in one block'
            if blocks:
                last, prev = max(blocks), 'check 1\'s registerReplacing'
        steps = [('check 2\'s register', run.get('plain'), 'register', 0, True),
                 ('the deregister', run['leaver'], 'deregister', run['leaverSince'], False),
                 ('the removeInactive', run['remover'], 'removeInactive', 0, False),
                 ('the register into the free seat', run.get('filler'), 'register', 0, False)]
        rr = run.get('rereg')
        if rr is not None:
            steps.append(('the registration of the victim again', rr['i'], REREGISTER_KINDS, rr['since'], False))
        for what, i, kind, since, required in steps:
            att = self.m3_attempt(ids[i], kind, since) if i is not None else None
            if att is None:
                if required and i is not None:
                    return f'{what} is not included'
                break
            if last is not None and att['block'] <= last:
                return f'{what} is included at {att["block"]}, not after {prev} at {last}'
            last, prev = att['block'], what
        return None

    def m3_run_txs(self, run):
        """The saved drill's transactions that a block includes (as settled)."""
        ids = self.state['identities']
        steps = [(i, 'registerReplacing', 0) for i in run.get('replacers') or []] + [
            (run.get('plain'), 'register', 0), (run['leaver'], 'deregister', run['leaverSince']),
            (run['remover'], 'removeInactive', 0), (run.get('filler'), 'register', 0)]
        rr = run.get('rereg')
        if rr is not None:
            steps += [(rr['i'], 'claimBond', rr['since']), (rr['i'], REREGISTER_KINDS, rr['since'])]
        atts = [self.m3_attempt(ids[i], kind, since) for i, kind, since in steps if i is not None]
        return [x['tx'] for x in atts if x is not None and x.get('tx')]

    def m3_drop(self, run):
        """Clears a saved drill (the next m3 starts a fresh one), with the leaver's and the removeInactive target's
        outcomes as the canonical chain shows them."""
        ids = self.state['identities']
        leaver = ids[run['leaver']]
        dereg = self.m3_attempt(leaver, 'deregister', run['leaverSince'])
        if dereg is not None and dereg.get('status') == 1:
            leaver['outcome'] = 'deregistered (m3)'
        rm = self.m3_attempt(ids[run['remover']], 'removeInactive')
        if rm is not None:
            self.m3_target_outcome(run['target'], rm)
        del self.state['m3Run']
        self.save()

    def m3_rewound(self, run, redo=True):
        """The steps of a saved drill (m3Run) that a reorganization has undone since they were decided: each check, and
        the deregister, keeps the hash of the block that included its transaction, and one whose transaction is now in
        flight again or settled from another block (the canonical receipt may differ) is named. With `redo` (its
        transactions settled: run_m3 waits for those in flight first), checks 1 and 2 are recomputed from the canonical
        chain, and the drill goes back to the first later step that was undone (the deregister, the removeInactive or
        the register into the free seat), whose phase reads its included transaction again and sends nothing that is
        included; check 4 is always recomputed by the refill phase. Check 5 (--reregister-after-u) keeps the hashes of
        the blocks of its registration and its claimBond, and is recomputed by the reregister phase when one changed or
        an earlier step is run again."""
        ids = self.state['identities']
        checks = run['result']['checks']
        undone = []
        if run.get('replacers'):
            reps = [ids[i] for i in run['replacers']]
            at = [next((x for x in reversed(r['attempts']) if x['kind'] == 'registerReplacing'), {}) for r in reps]
            if [self.m3_hash(x) for x in at] != checks['twoReplacingSameBlock'].get('blockHashes'):
                undone.append('twoReplacingSameBlock')
                if redo:
                    checks['twoReplacingSameBlock'] = self.m3_replacing(reps, run['victims'])
        if run.get('plain') is not None:
            plain = ids[run['plain']]
            att = self.m3_attempt(plain, 'register')
            if self.m3_hash(att) != checks['registerAloneCapacity'].get('blockHash'):
                undone.append('registerAloneCapacity')
                if redo:
                    before = self.past('lengths', att['block'] - 1)['active'] if att else None
                    checks['registerAloneCapacity'] = self.m3_register_alone(plain, before)
        leaver = ids[run['leaver']]
        phase = run['phase']
        if phase in ('belowFull', 'refill', 'reregister') and \
                self.m3_hash(self.m3_attempt(leaver, 'deregister', run['leaverSince'])) != run.get('deregisterBlockHash'):
            undone.append('deregister')
            phase = 'deregister'
            if leaver['outcome'] == 'deregistered (m3)':
                leaver['outcome'] = 'registered'   # set again by the deregister phase when it is included
        if phase in ('refill', 'reregister') and \
                self.m3_hash(self.m3_attempt(ids[run['remover']], 'removeInactive')) != checks['removeInactiveBelowFullSet'].get('blockHash'):
            undone.append('removeInactiveBelowFullSet')
            phase = 'belowFull'
        filler = run.get('filler')
        if 'registerBelowFullSet' in checks and filler is not None and \
                self.m3_hash(self.m3_attempt(ids[filler], 'register')) != checks['registerBelowFullSet'].get('blockHash'):
            undone.append('registerBelowFullSet')
            if phase == 'reregister':
                phase = 'refill'
        rr = run.get('rereg')
        if 'victimReregistersAfterU' in checks and rr is not None:
            c5, victim = checks['victimReregistersAfterU'], ids[rr['i']]
            if self.m3_hash(self.m3_attempt(victim, REREGISTER_KINDS, rr['since'])) != c5.get('blockHash') or \
                    self.m3_hash(self.m3_attempt(victim, 'claimBond', rr['since'])) != c5.get('claimBondBlockHash'):
                undone.append('victimReregistersAfterU')
        if redo:
            if phase != run['phase']:
                run['phase'] = phase
                for k in ('removeInactiveBelowFullSet',) * (phase != 'refill') + ('registerBelowFullSet', 'victimReregistersAfterU'):
                    checks.pop(k, None)
            elif 'victimReregistersAfterU' in undone:
                checks.pop('victimReregistersAfterU')
        return undone

    def run_m3(self):
        """The M3 drill (m3_drill). A check that needs the state at a past height the node no longer retains, and that
        the harness did not keep while it was retained (past), is not graded: no m3 result is written, the saved drill is
        cleared with the outcomes the canonical chain shows (m3_drop), and the next m3 starts a fresh drill."""
        try:
            return self.m3_drill()
        except StateGone as e:
            if self.state.get('m3Run') is not None:
                self.m3_drop(self.state['m3Run'])
            self.save()
            raise RuntimeError(f'{e} (pocold answers state reads only within the last K_hist blocks, -32017), and a check of the '
                               'drill is decided from it; its checks are not graded, no m3 result is written, and the next m3 '
                               'starts a fresh drill at a full set (run fill first if the set is below it)') from None

    def m3_drill(self):
        a = self.args
        if a.reregister_after_u and a.victims == 'any':
            # The victim that registers again signs with its own key: another registrant's member cannot.
            raise Refused('--reregister-after-u needs a harness victim; it is not combined with --victims any')
        self.poll()
        self.resume()   # what a stopped m3 (or fill) left in flight is settled first
        # and waited for: a registerReplacing an earlier run left held would otherwise race this run's own on the same
        # victim (whichever lands second reverts, and check 1 would record that as the rule not holding).
        earlier = list(self.inflight)
        if earlier and not self.wait_included(earlier, 2 * a.include_timeout + 2):
            raise RuntimeError('an earlier transaction is still in flight (it stays recorded; run again later)')
        ids = self.state['identities']
        # A drill stopped below a full set (m3Run, saved before its deregister is sent) is resumed where it stopped, and
        # only a fresh drill needs a full set. One whose deregister was never included left the set as it was: the drill
        # starts afresh.
        run = self.state.get('m3Run')
        if run is not None and (run['phase'] == 'reregister' or run.get('rereg') is not None) and not a.reregister_after_u:
            # Its optional step (--reregister-after-u) has started: what it sent is graded only by that step.
            raise Refused('the saved drill (m3Run) is in its --reregister-after-u step; run m3 --reregister-after-u to finish it')
        # Its included transactions are confirmed first (m3_confirm: at the node's safe height), so that what is
        # recomputed below is read from blocks a shallow reorganization no longer changes.
        txs = self.m3_run_txs(run) if run else []
        if txs:
            self.m3_confirm('saved drill\'s transactions', txs)
        # A step of it that a reorganization undid since it was decided (while it was stopped, or settled again above) is
        # recomputed from the canonical chain, or its phase is run again.
        undone = self.m3_rewound(run) if run else []
        if undone:
            self.say(f'M3 recomputes what a reorganization undid in its saved drill: {", ".join(undone)}')
            self.save()
        if run and run['phase'] == 'deregister' and self.m3_attempt(ids[run['leaver']], 'deregister', run['leaverSince']) is None:
            self.m3_drop(run)
            run = None
        bad = self.m3_disorder(run, undone) if run else None
        if bad:
            # A reorganization replayed the drill's transactions out of its order: their receipts no longer test the
            # rules, so no check is graded from them (a pass or a fail would both be unfounded).
            self.m3_drop(run)
            raise RuntimeError(f'a reorganization included the drill\'s transactions out of its order ({bad}); its checks are '
                               'not graded, no m3 result is written, and the next m3 starts a fresh drill at a full set '
                               '(run fill first if the set is below it)')
        if run is not None and a.reregister_after_u and run['victims'][0] not in {i['address'] for i in ids}:
            # The resumed drill's victims were chosen by its earlier run (--victims any): the same refusal as above.
            raise Refused(f'--reregister-after-u needs a harness victim; the resumed drill\'s first victim {run["victims"][0]} '
                          'is another registrant\'s member (run m3 without it)')
        if run is None:
            L = self.chain.lengths()
            if L['active'] < self.cp['M_max']:
                raise Refused(f'M3 runs at a full set: active {L["active"]} < M_max {self.cp["M_max"]} (run fill first)')
        op_key, op_addr = self.operator()
        need = fund_wei_of(a)
        call_need = REGISTRY_CALL_GAS * a.fee_cap_wei
        if run is None:
            result = {'startHeight': self.head, 'checks': {}}
            checks = result['checks']
            # 1. Two registrants in one block, each with registerReplacing on a distinct removable victim.
            newcomers = self.fresh_funded(op_key, op_addr, 3, need)
            victims = self.wait_removable(2)
            self.step(1)
            # Both are signed first, then sent back to back right after a new head, so they enter the same block.
            prepared = []
            for ident, victim in zip(newcomers[:2], victims):
                p, err = self.prepare(*self.replacing_args(ident, victim))
                if p is None:
                    raise RuntimeError(f'registerReplacing could not be signed: {err}')
                prepared.append(p)
            sent = []
            for tx, err in self.submit_all(prepared):
                if tx is None:
                    self.save()
                    raise RuntimeError(f'registerReplacing was refused: {err}')
                sent.append(tx)
            self.save()
            done = self.wait_included(sent, a.include_timeout)
            moved = self.m3_confirm('registerReplacing', sent) if done else False
            check = self.m3_replacing(newcomers[:2], victims)
            self.save()
            if not done:
                raise RuntimeError('a registerReplacing was not included within --include-timeout blocks (it stays recorded in flight)')
            if moved and len(check['blocks']) != 1:
                # Included in one block, then split by a reorganization before the check was decided: the receipts no
                # longer test the rule (a pass or a fail would both be unfounded). Nothing below the full set was sent.
                raise RuntimeError('a reorganization included check 1\'s two registerReplacing again in different blocks before '
                                   'the check was decided; it is not graded, no m3 result is written, and the next m3 starts a '
                                   'fresh drill')
            checks['twoReplacingSameBlock'] = check
            self.say('M3 two registerReplacing: ' + json.dumps(checks['twoReplacingSameBlock'], sort_keys=True))
            # 2. register alone at the full set: Capacity.
            plain = newcomers[2]
            self.step(1)
            L = self.chain.lengths()
            tx, err = self.register(plain)
            self.included('register at the full set', tx, err, confirm=True)
            checks['registerAloneCapacity'] = self.m3_register_alone(plain, L['active'], err)
            self.say('M3 register alone: ' + json.dumps(checks['registerAloneCapacity'], sort_keys=True))
            # 3. Below a full set: one harness member deregisters, then removeInactive on an idle member reverts. The
            # leaver is one whose record is ACTIVE at the head: state.json does not see another registrant's
            # registerReplacing (the wallet's, at a full set) exiting a harness member.
            idle = [i for i in ids if i['role'] == 'idle' and i['outcome'] == 'registered' and i['address'] not in victims]
            leaver = next((i for i in idle if (self.chain.record(i['address']) or {}).get('status') == 'ACTIVE'), None)
            if leaver is None or len(idle) < 2:
                raise RuntimeError('M3 needs two more registered idle harness members')
            target = self.wait_removable(1, exclude=set(victims) | {leaver['address']})[0]
            remover = self.fresh_funded(op_key, op_addr, 1, call_need)[0]
            # Saved before the deregister is sent: a run stopped from here on resumes below the full set it leaves.
            run = {'phase': 'deregister', 'result': result, 'victims': victims, 'leaver': leaver['i'],
                   'leaverSince': len(leaver['attempts']), 'remover': remover['i'], 'target': target,
                   'replacers': [n['i'] for n in newcomers[:2]], 'plain': plain['i']}
            self.state['m3Run'] = run
            self.save()
            self.ensure_funds(op_key, op_addr, [leaver], call_need)
            tx, err = self.registry_tx(leaver, 'deregister', calldata('deregister()'), REGISTRY_CALL_GAS)
            self.included('deregister', tx, err, confirm=True)
        else:
            run['result'].setdefault('resumedAt', []).append(self.head)
            self.say(f'M3 resumes its saved drill (phase {run["phase"]}, started at {run["result"]["startHeight"]})')
        result, victims, target = run['result'], run['victims'], run['target']
        checks = result['checks']
        leaver, remover = ids[run['leaver']], ids[run['remover']]
        if run['phase'] == 'deregister':
            # (the last attempt when none was included: one given up)
            dereg = self.m3_attempt(leaver, 'deregister', run['leaverSince']) or leaver['attempts'][-1]
            if dereg.get('status') != 1:
                # The set is still full: a removeInactive now would remove a member, which says nothing about this rule.
                self.m3_drop(run)
                raise RuntimeError(f'the deregister of {leaver["address"]} reverted ({dereg.get("reason")}); removeInactive is not '
                                   'sent at a full set')
            leaver['outcome'] = 'deregistered (m3)'
            run.update({'phase': 'belowFull', 'deregisterBlock': dereg['block'], 'deregisterBlockHash': self.m3_hash(dereg),
                        'activeAfterDeregister': self.past('lengths', dereg['block'])['active']})
            self.save()
        if run['phase'] == 'belowFull':
            rm = self.m3_attempt(remover, 'removeInactive')
            if rm is None:
                if run['activeAfterDeregister'] < self.cp['M_max'] <= self.chain.lengths()['active']:
                    # Another registrant took the free seat while the drill was stopped: removeInactive would remove a
                    # member at a full set, which says nothing about this rule.
                    del self.state['m3Run']
                    self.save()
                    raise RuntimeError('the set is full again before removeInactive was sent; removeInactive is not sent at a '
                                       'full set (the next m3 starts a fresh drill)')
                tx, err = self.registry_tx(remover, 'removeInactive', calldata('removeInactive(address)', target), REGISTRY_CALL_GAS,
                                           victim=target)
                self.included('removeInactive', tx, err, confirm=True)
                rm = remover['attempts'][-1]
            checks['removeInactiveBelowFullSet'] = self.m3_remove_inactive(run, remover, rm)
            self.say('M3 removeInactive below a full set: ' + json.dumps(checks['removeInactiveBelowFullSet'], sort_keys=True))
            run['phase'] = 'refill'
            self.save()
        if run['phase'] == 'refill':
            self.m3_refill(run, op_key, op_addr, need)
            # A reorganization during the drill may have undone a step decided earlier in it: no result is written from
            # it, and the drill stays saved for the same command to recompute.
            self.m3_revalidate(run)
            if a.reregister_after_u:
                # The drill stays saved through the optional step: a stop during it (the U wait) resumes it, and its
                # checks are read again before the result is written.
                run['phase'] = 'reregister'
                self.save()
        if run['phase'] == 'reregister':
            # 5. Optional: a victim registers again after U (its record refunded and erased at dueHeight).
            checks['victimReregistersAfterU'] = self.reregister(run, op_key, op_addr, need)
            self.save()
            # A reorganization deeper than kSafe during the U wait or the registration may have undone checks 1 to 4
            # (or check 5's own blocks) after they were decided: read again, as above.
            self.m3_revalidate(run)
        # The set is full again and every check is decided: a run stopped from here on starts a fresh drill.
        del self.state['m3Run']
        result['endHeight'] = self.head
        result['pass'] = all(c['pass'] for c in checks.values())
        self.state['m3'] = result
        self.save()
        self.write_report({'m3': result})
        return result

    def m3_refill(self, run, op_key, op_addr, need):
        """M3 check 4: the free seat takes a plain register again (the set is full once more)."""
        ids, checks = self.state['identities'], run['result']['checks']
        filler = ids[run['filler']] if run.get('filler') is not None else None
        att = self.m3_attempt(filler, 'register') if filler is not None else None
        if att is None and self.chain.lengths()['active'] >= self.cp['M_max']:
            # Another registrant took the free seat (while the drill was stopped, or since the removeInactive): a register
            # now would revert Capacity at a full set, which says nothing about this rule.
            del self.state['m3Run']
            self.save()
            raise RuntimeError('the set is full again before the register into the free seat was sent; it is not sent at a '
                               'full set (the next m3 starts a fresh drill)')
        if filler is None:
            filler = self.fresh_funded(op_key, op_addr, 1, need)[0]
            filler['role'] = 'idle'
            run['filler'] = filler['i']
            self.save()
        err = None
        if att is None:
            tx, err = self.register(filler)
            self.included('register into the free seat', tx, err, confirm=True)
            att = filler['attempts'][-1]
        checks['registerBelowFullSet'] = {'pass': att.get('status') == 1, 'block': att.get('block'), 'reason': att.get('reason') or err,
                                          'activeAfter': self.chain.lengths()['active'], 'blockHash': self.m3_hash(att)}
        self.say('M3 register into the free seat: ' + json.dumps(checks['registerBelowFullSet'], sort_keys=True))

    def m3_revalidate(self, run):
        """Before the drill goes on to its optional step and before its result is written: every decided step (checks 1
        to 5 and the deregister) is still in the block it was decided from (m3_rewound) and the canonical chain keeps
        the drill's order (m3_disorder). If not, no result is written (exit 1) and the drill stays saved (m3Run): with
        the order kept, the same command recomputes it from the canonical chain (the past state its checks read
        included); with the order broken, the next m3 reads that again and clears the saved drill with no result (as
        a resumed drill whose order is broken), and the m3 after it starts a fresh drill."""
        undone = self.m3_rewound(run, redo=False)
        bad = self.m3_disorder(run, undone)
        if undone or bad:
            self.save()
            what = ' and '.join(([f'undid {", ".join(undone)} after it was decided'] if undone else []) +
                                ([f'included the drill\'s transactions out of its order ({bad})'] if bad else []))
            then = ('the next m3 clears the saved drill (m3Run) with no result, and the m3 after it starts a fresh drill '
                    'at a full set' if bad else 'the same command recomputes the saved drill (m3Run) from the canonical chain')
            raise RuntimeError(f'a reorganization {what}; no m3 result is written, and {then}')

    def reregister(self, run, op_key, op_addr, need):
        """M3 check 5 (--reregister-after-u): the drill's first victim registers again once its record is refunded and
        erased at dueHeight (with a claimBond if the record is still there). The step is saved in the drill (`rereg`: the
        victim and its first attempt of the step), so a resumed run reads what it included from the canonical receipts
        and sends nothing that is included. The check keeps the hashes of the blocks of the registration and of the
        claimBond (m3_rewound), and recordErased is read at the block before the registration (past)."""
        victim = run['victims'][0]
        ident = next((i for i in self.state['identities'] if i['address'] == victim), None)
        if ident is None:   # (run_m3 refuses this before it sends anything)
            raise RuntimeError(f'the victim {victim} is not a harness identity: its key cannot register it again')
        rr = run.get('rereg')
        if rr is None or rr['i'] != ident['i']:
            rr = run['rereg'] = {'i': ident['i'], 'since': len(ident['attempts'])}
            self.save()
        exited = (run['result']['checks'].get('twoReplacingSameBlock', {}).get('victimRecords') or {}).get(victim) or {}
        due = exited.get('dueHeight')
        if due is None:
            rec = self.chain.record(victim)
            due = rec['dueHeight'] if rec else self.head
        att = self.m3_attempt(ident, REREGISTER_KINDS, rr['since'])
        err = None
        if att is None:
            self.say(f'M3 waiting for the victim {victim} refund at dueHeight {due} (U = {self.cp["U"]} blocks)')
            while self.head < due:
                self.step(1, timeout=self.cp['T_blk'] * 60)
            self.ensure_funds(op_key, op_addr, [ident], need + REGISTRY_CALL_GAS * self.args.fee_cap_wei)
            for _ in range(self.args.wait_blocks):
                if self.chain.record(victim) is None:
                    break
                self.step(1)
            if self.chain.record(victim) is not None and self.m3_attempt(ident, 'claimBond', rr['since']) is None:
                tx, err = self.registry_tx(ident, 'claimBond', calldata('claimBond()'), REGISTRY_CALL_GAS)
                self.included('claimBond', tx, err, confirm=True)
            if self.chain.lengths()['active'] >= self.cp['M_max']:
                target = self.wait_removable(1, exclude={victim})[0]
                tx, err = self.register_replacing(ident, target)
            else:
                tx, err = self.register(ident)
            if tx is not None:
                ident['outcome'] = None   # in flight: settled from the chain, by this run or the next
            self.included('registration of the victim again', tx, err, confirm=True)
            att = self.m3_attempt(ident, REREGISTER_KINDS, rr['since'])
            if att is None:
                raise RuntimeError('the registration of the victim again is no longer included (not decided; its record is kept)')
        if att.get('status') != 1:
            ident['outcome'] = f'failed: {att.get("reason")}'
        claim = self.m3_attempt(ident, 'claimBond', rr['since'])
        erased = self.past('record', att['block'] - 1, victim) is None
        out = {'pass': erased and att.get('status') == 1, 'victim': victim, 'dueHeight': due,
               'claimBond': claim.get('status') if claim else None, 'claimBondBlockHash': self.m3_hash(claim),
               'recordErased': erased, 'kind': att['kind'], 'block': att.get('block'), 'status': att.get('status'),
               'reason': att.get('reason') or err, 'blockHash': self.m3_hash(att)}
        self.say('M3 victim registers again after U: ' + json.dumps(out, sort_keys=True))
        return out

    # -- R-off support: register into one named member's seat
    def replace_eligible(self, victim):
        """The victim's record once it is removable (code 0), or Refused / RuntimeError."""
        a = self.args
        rec = self.chain.record(victim)
        if rec is None:
            raise Refused(f'{victim} has no registry record')
        for _ in range(a.wait_blocks + 1):
            code = self.chain.removal(victim)
            if code == 0:
                return rec
            if code in (1, 3):
                raise Refused(f'{victim} cannot be removed: removal code {code} {REMOVAL_LABELS[code]}')
            self.step(1)
        raise RuntimeError(f'{victim} did not become removable within {a.wait_blocks} blocks (code {code} {REMOVAL_LABELS.get(code)})')

    def run_replace(self):
        a = self.args
        victim = a.victim.lower()
        if not ADDRESS.fullmatch(victim):
            raise Refused('--victim must be a full 0x address')
        self.poll()
        op_key, op_addr = self.operator()
        self.resume()
        ids = self.state['identities']
        # A replacement is saved (replaceRun) from before its newcomer is funded until its result is written: a run
        # stopped or failed meanwhile is finished by the same command, before any eligibility check (once the
        # registerReplacing is included the victim is EXITED, and refunded at dueHeight), never by a second newcomer.
        run = self.state.get('replaceRun')
        if run is not None and run['victim'] != victim:
            raise Refused(f'an unfinished replacement of {run["victim"]} is saved (replaceRun); finish it with replace --victim '
                          f'{run["victim"]} before replacing another member')
        ident = ids[run['newcomer']] if run is not None else None
        if run is not None:
            run.setdefault('resumedAt', []).append(self.head)
            self.say(f'replace resumes its saved replacement of {victim} (newcomer {ident["address"]})')
            mine = [t for t, v in self.inflight.items() if v['i'] == ident['i']]
            if mine and not self.wait_included(mine, a.include_timeout):
                self.save()
                raise RuntimeError('the registerReplacing was not included within --include-timeout blocks (it stays recorded in '
                                   'flight)')
            self.revive(ident)   # one given up earlier that a canonical block includes after all
            att = self.m3_attempt(ident, 'registerReplacing')
            if att is not None:
                return self.replace_result(run, ident, att)
            # Nothing of it is included or in flight (none sent, refused or given up): the same newcomer replaces the
            # victim afresh, if it is still eligible; if not, the saved replacement is cleared.
        try:
            rec = self.replace_eligible(victim)
        except (Refused, RuntimeError):
            if run is not None:
                del self.state['replaceRun']
                self.save()
            raise
        if run is None:
            ident = self.new_identity('m3')
            ident['role'] = 'idle'
            run = {'victim': victim, 'newcomer': ident['i'], 'startHeight': self.head}
            self.state['replaceRun'] = run
        run['before'] = rec
        self.save()
        self.ensure_funds(op_key, op_addr, [ident], fund_wei_of(a))
        tx, err = self.register_replacing(ident, victim)
        # Only an included registerReplacing is a result: one refused, not sent, not included within --include-timeout
        # blocks (it stays in flight) or given up is exit 1 with no replacement written, and the saved one is resumed.
        self.included('registerReplacing', tx, err)
        att = self.m3_attempt(ident, 'registerReplacing')
        if att is None:
            self.save()
            raise RuntimeError(f'the registerReplacing was not included ({ident["attempts"][-1].get("reason")})')
        return self.replace_result(run, ident, att)

    def replace_after(self, run, att, block_hash):
        """The victim's record at the block that includes `att` (`after`), and where it was read from. pocold answers
        state reads only within the last K_hist blocks (-32017 beyond them), and a resumed run may come long after that
        block (U blocks of refund wait), so `after` is saved with the block's hash when first read and reused while
        that block is still canonical. When the block changed (a reorganization while stopped) and its state is no
        longer retained, `after` is the latest record if it shows this exit, else the exit `att` made, with its
        dueHeight from U (the victim's record is refunded and erased at dueHeight, or registered again since)."""
        victim, block = run['victim'], att['block']
        saved = run.get('after')
        if saved is not None and saved.get('block') == block and saved.get('blockHash') == block_hash:
            return saved['record'], saved['source']
        try:
            record, source = self.chain.record(victim, hex(block)), 'inclusionBlock'
        except RpcError as e:
            if e.code != -32017:
                raise
            latest = self.chain.record(victim)
            if att.get('status') != 1:
                record, source = latest, f'latest at {self.head} (state at {block} not retained)'
            elif latest and latest['status'] == 'EXITED' and latest['exitHeight'] == block:
                record, source = latest, f'latest at {self.head} (state at {block} not retained)'
            else:
                record = {'status': 'EXITED', 'exitHeight': block, 'dueHeight': block + self.cp['U']}
                source = f'derived from the receipt (state at {block} not retained; latest record {latest and latest["status"]})'
        run['after'] = {'block': block, 'blockHash': block_hash, 'record': record, 'source': source}
        return record, source

    def replace_result(self, run, ident, att):
        """The replacement from its included registerReplacing `att`, as the canonical chain shows it (a resumed run
        recomputes it from the receipt, so a reorganization while it was stopped is read), and with --watch-refund the
        victim's record at its dueHeight. A stop during that wait keeps it saved for the same command; a reorganization
        that undid the replacement during the wait writes no result (exit 1) and keeps it saved."""
        victim = run['victim']
        if att.get('status') != 1:
            ident['outcome'] = f'failed: {att.get("reason")}'
        block_hash = self.m3_hash(att)
        after, source = self.replace_after(run, att, block_hash)
        out = {'victim': victim, 'before': run['before'], 'after': after, 'afterSource': source, 'newcomer': ident['address'],
               'block': att['block'], 'blockHash': block_hash, 'status': att.get('status'), 'reason': att.get('reason')}
        if run.get('resumedAt'):
            out['resumedAt'] = run['resumedAt']
        self.save()
        if att.get('status') == 1 and self.args.watch_refund and after:
            self.say(f'replace waiting for the victim {victim} refund at dueHeight {after["dueHeight"]}')
            while self.head < after['dueHeight']:
                self.step(1, timeout=self.cp['T_blk'] * 60)
            self.step(2)
            out['atDue'] = {'height': self.head, 'record': self.chain.record(victim)}
            if self.m3_hash(self.m3_attempt(ident, 'registerReplacing')) != out['blockHash']:
                self.save()
                raise RuntimeError('a reorganization undid the registerReplacing after it was decided; no replacement is written, '
                                   'and the same command recomputes it (replaceRun) from the canonical chain')
        del self.state['replaceRun']
        self.state.setdefault('replacements', []).append(out)
        self.save()
        self.say('replace ' + json.dumps(out, sort_keys=True))
        return out

    def run_status(self):
        self.poll()
        census, ids = self.chain.census()
        ours = {i['address'] for i in self.state['identities']}
        s = self.sample()
        out = {'height': self.head, 'census': census, 'harnessActive': len(ours & ids), 'harnessIdentities': len(ours),
               'sample': s, 'templateLatency': self.rpc.call('pocol_templateLatency').get('all'),
               'finality': self.rpc.call('pocol_finality')}
        self.say(json.dumps(out, sort_keys=True))
        return out


def rss_of(pid):
    try:
        for line in Path(f'/proc/{int(pid)}/status').read_text().splitlines():
            if line.startswith('VmRSS:'):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError):
        return None
    return None


def command_line(pid):
    """The process's arguments (/proc), or its command as one string (ps, where there is no /proc), or None."""
    try:
        return [x.decode('utf-8', 'surrogateescape') for x in Path(f'/proc/{int(pid)}/cmdline').read_bytes().split(b'\0')]
    except OSError:
        pass
    try:
        p = subprocess.run(['ps', '-ww', '-o', 'command=', '-p', str(int(pid))], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    return p.stdout.strip() if p.returncode == 0 else None


def miner_home(miner, base):
    home = Path(miner['home'])
    return home if home.is_absolute() else Path(base).resolve() / home


def miner_running(miner, base):
    """True when the recorded miner process still runs with its home (a reused pid with another command is not it);
    False when it does not; None when the pid is alive but its command line cannot be read (treated as running by
    delete-keys and never signalled by stop-miners)."""
    pid = int(miner['pid'])
    if os.name == 'nt':
        return None
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return False   # another user's process: not a node this harness started
    cmd = command_line(pid)
    if cmd is None:
        return None
    home = str(miner_home(miner, base))
    if isinstance(cmd, list):
        return home in cmd
    return f' {home} ' in f' {cmd} '


def load_state(base):
    path = Path(base) / 'state.json'
    return json.loads(path.read_text(encoding='utf-8')) if path.exists() else {}


def stop_miners(directory, wait=20):
    """Stops the mining nodes the harness started (whatever spelling of --dir is given: their homes are absolute)."""
    base = Path(directory).resolve()
    lock = DirLock(base) if base.is_dir() else None   # refused while a fill, m3 or replace uses the directory
    try:
        return _stop_miners(base, wait)
    finally:
        if lock is not None:
            lock.release()


def _stop_miners(base, wait):
    miners = load_state(base).get('miners', [])
    stopped, unknown = [], []
    def kill(pid, sig):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass   # it exited after the check: stopped, which is the goal
    for m in miners:
        running = miner_running(m, base)
        if running:
            kill(m['pid'], signal.SIGTERM)
            stopped.append(m['pid'])
        elif running is None:
            unknown.append(m['pid'])
    deadline = time.time() + wait
    while any(miner_running(m, base) for m in miners) and time.time() < deadline:
        time.sleep(0.2)
    for m in miners:
        if miner_running(m, base):
            kill(m['pid'], signal.SIGKILL)
    if unknown:
        raise Refused(f'stopped {stopped}; pid(s) {unknown} are alive but their command line cannot be read, so the '
                      'harness cannot tell whether they are its miners: stop them by hand')
    return stopped


def delete_keys(directory, discard_funds=False):
    """Deletes every harness key (keys/ and the miners' homes, which hold their keys) and keeps the public state and
    report. Refused while a recorded miner node still runs, or may run, and, unless `discard_funds`, while the state
    records identities: what their keys control (balances, HELD bonds, mining rewards) cannot be claimed or moved once
    the keys are gone, and the harness moves none of it back."""
    base = secure_dir(Path(directory).resolve(), create=False)
    lock = DirLock(base)   # refused while a fill, m3 or replace uses the directory
    try:
        return _delete_keys(base, discard_funds)
    finally:
        lock.release()


def abandoned(state):
    """What the recorded identities' keys control, from the public state (not read from the chain): the identities,
    the bonds recorded for the registered bonded ones (HELD in the registry's escrow, refunded only to the identity),
    and the mining identities (their rewards are paid to their own addresses)."""
    ids = state.get('identities', [])
    bonded = [i for i in ids if i.get('bondWei') and i.get('outcome') == 'registered']
    return {'identities': len(ids), 'registeredBonded': len(bonded), 'bondsWei': sum(i['bondWei'] for i in bonded),
            'miners': sum(1 for i in ids if i.get('role') == 'miner')}


def _delete_keys(base, discard_funds=False):
    state_path = base / 'state.json'
    state = load_state(base)
    if any(miner_running(m, base) is not False for m in state.get('miners', [])):
        raise Refused('miner nodes started by the harness still run (or cannot be confirmed stopped); stop them first (stop-miners)')
    if state.get('identities') and not discard_funds:
        raise Refused(f'the identities\' keys still control funds ({json.dumps(abandoned(state), sort_keys=True)}, and each '
                      'identity\'s unspent first fee): deleting the keys makes them unrecoverable. Deregister, claim the bonds '
                      'and move the balances first if they matter, then pass --discard-funds')
    removed = 0
    for sub in ('keys', 'miners'):
        d = base / sub
        if d.is_dir() and not d.is_symlink():
            removed += sum(1 for p in d.rglob('*.pocol-key'))
            shutil.rmtree(d)
    if state:
        state['keysDeleted'] = True
        state_path.write_text(json.dumps(state, indent=1, sort_keys=True), encoding='utf-8')
    return removed


def parser():
    p = argparse.ArgumentParser(description=__doc__.split('\n\n')[0], formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='command', required=True)

    def common(s, keys=True):
        s.add_argument('--rpc', default='http://127.0.0.1:8545', help='the node RPC (default http://127.0.0.1:8545)')
        s.add_argument('--dir', required=True, help='the harness directory (mode 0700): keys, state.json, report.json')
        s.add_argument('--bin-dir', default=str(ROOT / 'prototypes/pocold/target/release'), help='directory of pocol-key, pocol-sign, pocol')
        s.add_argument('--allow-mainnet-chain', action='store_true', help=f'allow chain id {MAINNET_CHAIN_ID} (refused by default)')
        s.add_argument('--expect-chain-id', type=int, help=f'refuse unless the node is on this chain (a devnet of the rehearsal profile is {REHEARSAL_CHAIN_ID})')
        s.add_argument('--poll', type=float, default=0.5, help='seconds between head polls')
        s.add_argument('--node-pid', type=int, help='sample this local node process\'s RSS from /proc (M1)')
        if keys:
            s.add_argument('--operator-key', help='key file (32 bytes, mode 0600) that funds the newcomers')
            s.add_argument('--allow-member-operator', action='store_true', help='allow an operator key that is the node\'s miner or a member')
            s.add_argument('--fund-wei', type=int, help='first-fee funding per identity (default: --attempts x 500,000 gas x '
                           '--fee-cap-wei, 0.003 PCOL at the defaults)')
            s.add_argument('--fee-cap-wei', type=int, default=DEFAULT_FEE_CAP_WEI, help='max fee per gas of every transaction signed, '
                           'the funding transfers included (default 3 gwei)')
            s.add_argument('--attempts', type=int, default=DEFAULT_ATTEMPTS, help='registration attempts per identity (default 2)')
            s.add_argument('--include-timeout', type=int, default=30, help='blocks before an unincluded transaction is checked')
            s.add_argument('--fund-batch', type=int, default=128, help='funding transfers per batch (default 128)')
            s.add_argument('--wait-blocks', type=int, default=400, help='blocks to wait for a removable member (default 400)')
            s.add_argument('--victims', choices=('harness', 'any'), default='harness', help='M3 victims: harness idle members only (default) or any removable member')
            s.add_argument('--progress-every', type=int, default=10, help='blocks between progress lines')
            s.add_argument('--sample-every', type=int, default=30, help='blocks between M1 samples')
        return s

    f = common(sub.add_parser('fill', help='register up to --count fresh identities'))
    f.add_argument('--count', type=int, default=10000, help='identities to register (default 10,000; clamped to the free seats)')
    f.add_argument('--no-clamp', action='store_true', help='refuse instead of clamping when --count exceeds the free seats')
    f.add_argument('--miners', type=int, default=0, help='ACCRUING identities that mine (each a local pocol start node)')
    f.add_argument('--bonded', type=int, default=0, help='identities registered with a bond (HELD at B_reg)')
    f.add_argument('--bond-wei', type=int, help='the bonded identities\' bond (default B_reg)')
    f.add_argument('--network', help='network.pocol.json for the mining identities\' nodes (needed with --miners)')
    f.add_argument('--peer', action='append', help='P2P address the mining nodes connect to (repeatable)')
    f.add_argument('--miner-port-base', type=int, default=39500, help='first local port of the mining nodes')
    f.add_argument('--inflight', type=int, help='registrations in flight at once (default R_max, which bounds a block to R_max of them; '
                   'up to 2 x R_max pipelines when inclusion takes two blocks, and a collision reverts with RegOpsLimit and is retried)')
    f.add_argument('--dry-run', action='store_true', help='print the plan and send nothing')
    m = common(sub.add_parser('m3', help='the M3 capacity path at a full set'))
    m.add_argument('--reregister-after-u', action='store_true', help='also wait U blocks and register one harness victim again '
                   '(its own key signs; refused with --victims any)')
    r = common(sub.add_parser('replace', help='register a fresh identity into --victim\'s seat (R-off)'))
    r.add_argument('--victim', required=True)
    r.add_argument('--watch-refund', action='store_true', help='wait until the victim\'s dueHeight and record its record then')
    common(sub.add_parser('status', help='the M1 measures now'), keys=False)
    s = sub.add_parser('stop-miners', help='stop the mining identities\' nodes')
    s.add_argument('--dir', required=True)
    s = sub.add_parser('delete-keys', help='delete every identity key (their balances, bonds and rewards become unrecoverable)')
    s.add_argument('--dir', required=True)
    s.add_argument('--discard-funds', action='store_true', help='required: the identities\' balances, HELD bonds and mining '
                   'rewards are lost with their keys (the operator key is not deleted)')
    return p


def main(argv=None, out=sys.stdout):
    args = parser().parse_args(argv)
    os.umask(0o077)
    try:
        if args.command == 'stop-miners':
            print(json.dumps({'stopped': stop_miners(args.dir)}), file=out)
            return 0
        if args.command == 'delete-keys':
            print(json.dumps({'deletedKeyFiles': delete_keys(args.dir, args.discard_funds)}), file=out)
            return 0
        if args.command == 'fill' and args.miners and not args.network and not args.dry_run:
            raise Refused('--miners needs --network (and usually --peer) to start the mining identities\' nodes')
        if args.command == 'status':
            args.operator_key = None
        h = Harness(args, out=out)
    except Refused as e:
        print(f'refused: {e}', file=sys.stderr)
        return 3

    def stop(*_):
        h.stopping = True
    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    try:
        result = {'fill': h.run_fill, 'm3': h.run_m3, 'replace': h.run_replace, 'status': h.run_status}[args.command]()
        if args.command == 'fill' and 'registered' in result:
            summary = {k: result[k] for k in ('identities', 'registered', 'byRole', 'registrationsPerBlock', 'maxRegistrationsInABlock',
                                              'failuresByReason', 'census', 'registeredAndActiveAtEnd') if k in result}
            print('summary ' + json.dumps(summary, sort_keys=True), file=out)
        if args.command == 'm3':
            return 0 if result['pass'] else 2
        return 0
    except Refused as e:
        print(f'refused: {e}', file=sys.stderr)
        return 3
    except (Stopped, RuntimeError, OSError, RpcError, RewardUnavailable) as e:
        # Every transaction was recorded before it was sent; the state is saved as it stands, and the same command
        # resumes from it.
        if args.command != 'status':
            h.save()
        print(f'{"stopped" if isinstance(e, Stopped) else "error"}: {e}', file=sys.stderr)
        return 130 if isinstance(e, Stopped) else 1
    finally:
        h.close()


if __name__ == '__main__':
    sys.exit(main())
