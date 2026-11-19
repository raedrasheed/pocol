#!/usr/bin/env python3
"""Unit tests of the registration fill harness: its plan and limits, its refusals, its reason inference and its key
handling, and the whole fill and M3 loops against an in-process fake node that applies the registry's per-block and
capacity rules (PoColSystemV3.sol checkBudgets, enroll, replacing, removal). No binaries, no network.

The harness on a real devnet is test_registration_fill_devnet.py (opt-in)."""
import copy
import hashlib
import io
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / 'deploy/public-testnet'))
import registration_fill as rf  # noqa: E402
import pocol_identity  # noqa: E402

PROFILE = json.loads((ROOT / 'deploy/mainnet/v3-rehearsal-profile.json').read_text(encoding='utf-8'))
WALLET = (ROOT / 'prototypes/pocold/src/wallet.rs').read_text(encoding='utf-8')
CONTRACT = (ROOT / 'prototypes/lp4-execution/contracts/system/PoColSystemV3.sol').read_text(encoding='utf-8')


def profile_cp(**over):
    cp = {k: int(PROFILE['cp'][k]) for k in rf.CP_INDEX}
    cp.update(over)
    return cp


class Plan(unittest.TestCase):
    def test_the_rehearsal_fill_is_about_2500_blocks_at_r_max_4(self):
        self.assertEqual(PROFILE['registrationPerBlock'], int(PROFILE['cp']['R_max']))
        p = rf.plan(10000, profile_cp(), active=4, records=4, miners=8, bonded=20)
        self.assertEqual((p['planned'], p['clamped'], p['rMax'], p['blocksAtRMax']), (9996, True, 4, 2499))
        self.assertAlmostEqual(p['hoursAtRMax'], 6.94, places=2)
        self.assertEqual(p['roles'], {'miner': 8, 'bonded': 20, 'idle': 9968})
        fees = 9996 * rf.TRANSFER_GAS * rf.DEFAULT_FEE_CAP_WEI   # each transfer's max fee is at most the fee cap
        self.assertEqual(p['transferFeesMaxWei'], fees)
        self.assertEqual(p['operatorTotalWei'], 9996 * rf.default_fund_wei() + 20 * 20 * 10**18 + fees)
        self.assertEqual(rf.default_fund_wei(), 3 * 10**15, '0.003 PCOL: two attempts at 500,000 gas and 3 gwei')

    def test_the_default_funding_follows_attempts_and_the_fee_cap(self):
        p = rf.plan(10, profile_cp(), active=0, records=0, fee_cap=20 * rf.GWEI, attempts=4)
        self.assertEqual(p['fundWei'], 4 * rf.REPLACE_GAS * 20 * rf.GWEI)
        args = rf.parser().parse_args(['fill', '--dir', 'x', '--fee-cap-wei', str(20 * rf.GWEI), '--attempts', '4'])
        self.assertEqual(rf.fund_wei_of(args), 4 * rf.REPLACE_GAS * 20 * rf.GWEI)
        args = rf.parser().parse_args(['fill', '--dir', 'x', '--fund-wei', '7'])
        self.assertEqual(rf.fund_wei_of(args), 7)

    def test_capacity_clamps_or_refuses(self):
        cp = profile_cp(M_max=8, REG_RECORDS_MAX=10)
        self.assertEqual(rf.plan(20, cp, active=2, records=2)['planned'], 6)
        self.assertEqual(rf.plan(20, cp, active=2, records=7)['planned'], 3, 'REG_RECORDS_MAX bounds it too')
        self.assertEqual(rf.plan(20, cp, active=8, records=8)['planned'], 0)
        with self.assertRaisesRegex(rf.Refused, 'only 6 seats are free'):
            rf.plan(20, cp, active=2, records=2, clamp=False)
        self.assertEqual(rf.plan(5, cp, active=2, records=2, clamp=False)['planned'], 5)

    def test_impossible_requests_are_refused(self):
        cp = profile_cp()
        for kwargs, why in [(dict(count=3, miners=2, bonded=2), 'exceed --count'), (dict(count=-1), 'non-negative'),
                            (dict(count=3, bond_wei=cp['B_reg'] + 1), 'at most B_reg'),
                            (dict(count=3, fund_wei=rf.REGISTER_GAS * rf.PRIORITY_WEI - 1), 'cannot cover one registration')]:
            with self.assertRaisesRegex(rf.Refused, why):
                rf.plan(cp=cp, active=0, records=0, **kwargs)
        with self.assertRaisesRegex(rf.Refused, 'R_max is 0'):
            rf.plan(3, profile_cp(R_max=0), 0, 0)
        for inflight in (0, 9):
            with self.assertRaisesRegex(rf.Refused, '--inflight is 1 to 2 x R_max'):
                rf.plan(3, cp, 0, 0, inflight=inflight)
        self.assertTrue(rf.plan(3, cp, 0, 0)['perBlockBound'].startswith('R_max (except after a reorganization'))
        self.assertTrue(rf.plan(3, cp, 0, 0, inflight=8)['perBlockBound'].startswith('none'))

    def test_roles_put_the_miners_first(self):
        self.assertEqual(rf.roles({'miner': 2, 'bonded': 1, 'idle': 2}), ['miner', 'miner', 'bonded', 'idle', 'idle'])

    def test_registration_slots_never_exceed_r_max_in_flight_or_m_max(self):
        for r_max in (1, 4):
            for inflight in range(r_max + 1):
                for active in range(0, 12):
                    for ready in (0, 1, 3, 9):
                        n = rf.registration_slots(r_max, inflight, active, 10, ready)
                        self.assertTrue(0 <= n <= ready and inflight + n <= r_max and active + inflight + n <= max(10, active + inflight))
        self.assertEqual(rf.registration_slots(4, 0, 0, 10, 100), 4)
        self.assertEqual(rf.registration_slots(4, 1, 8, 10, 100), 1)

    def test_max_fee_follows_the_wallet_and_the_balance(self):
        g = rf.GWEI
        self.assertEqual(rf.max_fee_for(g, 3 * g, 10**18, rf.REGISTER_GAS), (3 * g, None))
        self.assertEqual(rf.max_fee_for(10**7, 3 * g, 10**18, rf.REGISTER_GAS), (2 * 10**7 + g, None), '2 x base + priority')
        self.assertEqual(rf.max_fee_for(g, 3 * g, 2 * g * rf.REGISTER_GAS, rf.REGISTER_GAS), (2 * g, None), 'capped by the balance')
        self.assertEqual(rf.max_fee_for(g, 3 * g, g * rf.REGISTER_GAS, rf.REGISTER_GAS), (None, 'underfunded'), 'below base + priority')
        self.assertEqual(rf.max_fee_for(g, 3 * g, 10**18, rf.REGISTER_GAS, value=10**18), (None, 'underfunded'), 'the bond leaves no fee')
        self.assertEqual(rf.max_fee_for(3 * g, 3 * g, 10**18, rf.REGISTER_GAS), (None, rf.WAITING), 'the base fee is above the cap')


class Refusals(unittest.TestCase):
    def test_the_mainnet_chain_needs_the_explicit_flag(self):
        with self.assertRaisesRegex(rf.Refused, 'is the mainnet'):
            rf.check_chain(2656089850)
        self.assertEqual(rf.check_chain(2656089850, allow_mainnet=True), 2656089850)
        self.assertEqual(rf.check_chain(rf.REHEARSAL_CHAIN_ID), 1656550731)
        self.assertEqual(rf.MAINNET_CHAIN_ID, json.loads((ROOT / 'deploy/mainnet/v3-profile.json').read_text())['chainId'])
        self.assertEqual(rf.REHEARSAL_CHAIN_ID, PROFILE['chainId'])

    def test_the_key_directory_is_0700_and_owned(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = rf.secure_dir(Path(tmp) / 'h' / 'keys')
            self.assertEqual(stat.S_IMODE(os.stat(d).st_mode), 0o700)
            os.chmod(d, 0o755)
            with self.assertRaisesRegex(rf.Refused, 'mode 0700'):
                rf.secure_dir(d)
            os.chmod(d, 0o700)
            os.symlink(d, Path(tmp) / 'link')
            with self.assertRaisesRegex(rf.Refused, 'real directory'):
                rf.secure_dir(Path(tmp) / 'link')

    def test_the_operator_key_must_be_private(self):
        with tempfile.TemporaryDirectory() as tmp:
            k = Path(tmp) / 'op.key'
            with self.assertRaisesRegex(rf.Refused, 'does not exist'):
                rf.check_operator_key(k)
            k.write_bytes(os.urandom(32))
            os.chmod(k, 0o644)
            with self.assertRaisesRegex(rf.Refused, 'group or others'):
                rf.check_operator_key(k)
            os.chmod(k, 0o600)
            self.assertEqual(rf.check_operator_key(k), k)

    def test_an_uncreatable_directory_is_refused_with_the_setup_hint(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / 'file').write_text('')
            with self.assertRaisesRegex(rf.Refused, 'create it first'):
                rf.secure_dir(Path(tmp) / 'file' / 'h')

    @unittest.skipIf(os.name == 'nt', 'POSIX sessions')
    def test_the_signing_tools_run_outside_the_terminal_s_process_group(self):
        # A terminal's Ctrl-C (SIGINT to the foreground process group) reaches the harness, not pocol-sign.
        child = rf.Signer._run([sys.executable, '-c', 'import os; print(os.getpgid(0), os.getsid(0))']).split()
        self.assertNotEqual(int(child[0]), os.getpgid(0))
        self.assertNotEqual(int(child[1]), os.getsid(0))

    def test_main_refuses_miners_without_a_network(self):
        err = io.StringIO()
        with tempfile.TemporaryDirectory() as tmp:
            stderr, sys.stderr = sys.stderr, err
            try:
                self.assertEqual(rf.main(['fill', '--dir', tmp, '--miners', '1', '--operator-key', 'x']), 3)
            finally:
                sys.stderr = stderr
        self.assertIn('--miners needs --network', err.getvalue())


class RpcClient(unittest.TestCase):
    def test_busy_answers_are_retried_and_errors_raised(self):
        import http.server
        import threading
        answers = [{'error': {'code': -32021, 'message': 'busy', 'data': {'reason': 'busy', 'retryAfterMs': 1}}},
                   {'result': '0x10'}, {'error': {'code': -32000, 'message': 'nonce too low'}}]
        seen = []

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'

            def do_POST(self):
                seen.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
                body = json.dumps({'jsonrpc': '2.0', 'id': 1, **answers.pop(0)}).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *a):
                pass
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        r = rf.Rpc(f'http://127.0.0.1:{server.server_port}', sleep=lambda s: None)
        self.assertEqual(r.call('eth_blockNumber'), '0x10')
        with self.assertRaises(rf.RpcError) as e:
            r.call('eth_sendRawTransaction', ['0x00'])
        self.assertEqual((e.exception.code, e.exception.message), (-32000, 'nonce too low'))
        self.assertEqual([x['method'] for x in seen], ['eth_blockNumber', 'eth_blockNumber', 'eth_sendRawTransaction'])
        self.assertEqual(r.latency_summary()['eth_blockNumber']['count'], 2)
        r.conn.close()
        with self.assertRaisesRegex(rf.Refused, 'http://'):
            rf.Rpc('https://node.example')


class Mirrors(unittest.TestCase):
    """The constants and encodings the harness shares with the wallet, the registry and the CP schema."""

    def test_gas_constants_mirror_the_wallet(self):
        def const(name):
            return int(re.search(rf'const {name}:u64=([0-9_]+);', WALLET).group(1).replace('_', ''))
        self.assertEqual(rf.REGISTER_GAS, const('EARNIN_REGISTER_GAS'))
        self.assertEqual(rf.REPLACE_GAS, const('REPLACE_GAS'))
        self.assertEqual(rf.REGISTRY_CALL_GAS, const('REGISTRY_CALL_GAS'))

    def test_the_harness_is_not_a_release_file(self):
        for packager in ('deploy/standalone/package.py', 'deploy/release/assemble_release.py'):
            self.assertNotIn('rehearsal/', (ROOT / packager).read_text(encoding='utf-8'), packager)

    def test_cp_indices_follow_the_schema(self):
        names = [f[0] for f in pocol_identity.CP_SCHEMA_V2]
        for name, index in rf.CP_INDEX.items():
            self.assertEqual(names[index], name)
        # The registry's own cfg(n) reads: R_max 17, M_max 18, REG_RECORDS_MAX 20, REG_OPS 23, Theta_inact 25.
        self.assertIn('cfg(20)||active>=cfg(18))revert Capacity()', CONTRACT)
        self.assertIn('(c&65535)+(used&255)+extra>=cfg(23) || (registering && ((c>>16)&255)>=cfg(17))', CONTRACT)

    def test_calldata_shapes(self):
        a, v = '0x' + '11' * 20, '0x' + '22' * 20
        reg = bytes.fromhex(rf.calldata('register(address)', a)[2:])
        rep = bytes.fromhex(rf.calldata('registerReplacing(address,address)', a, v)[2:])
        self.assertEqual((len(reg), len(rep)), (36, 68), 'the registry checks msg.data.length 36 and 68')
        self.assertEqual(rep[16:36].hex(), '11' * 20)
        self.assertEqual(rep[48:68].hex(), '22' * 20, 'the victim word, as wallet.rs pending_victims reads data[48..68]')
        self.assertEqual(rf.calldata('get(uint256)', 18)[-2:], '12')
        registry = CONTRACT[CONTRACT.index('contract MinerRegistry'):]
        for name in rf.ERRORS:
            self.assertRegex(registry[:registry.index('uint256 constant')], rf'error {re.escape(name)};', 'a MinerRegistry error')
        # BadPhase() is the revert data a pocold eth_call of register returns (the registry's phase check).
        self.assertEqual(rf.ERROR_SELECTORS['0x301c306b'], 'BadPhase')

    def test_reason_inference(self):
        base = {'gas': 400000, 'gasUsed': 30000, 'rMax': 4, 'regOps': 16, 'mMax': 8, 'recordsMax': 100, 'activeBefore': 5,
                'recordsBefore': 5, 'regsBefore': 0, 'opsBefore': 0, 'senderRecord': False}
        r = lambda kind, **f: rf.infer_reason(kind, {**base, **f})
        self.assertEqual(r('register', gasUsed=400000), 'OutOfGas')
        self.assertTrue(r('register', regsBefore=4, opsBefore=4).startswith('RegOpsLimit'))
        self.assertTrue(r('registerReplacing', regsBefore=1, opsBefore=15).startswith('RegOpsLimit'), 'two REG_OPS units')
        self.assertTrue(r('register', activeBefore=8).startswith('Capacity'))
        self.assertTrue(r('register', recordsBefore=100).startswith('Capacity'))
        self.assertTrue(r('register', activeBefore=8, opsBefore=1).startswith('reverted'), 'an earlier operation may have changed it')
        self.assertTrue(r('register', senderRecord=True).startswith('BadState'))
        self.assertIn('RECENTLY_ACTIVE', r('registerReplacing', activeBefore=8, victimCode=4))
        self.assertTrue(r('registerReplacing', victimCode=1).startswith('BadState'))
        self.assertIn('SET_NOT_FULL', r('removeInactive', victimCode=2))
        self.assertTrue(r('register').startswith('reverted (reason not observable'))
        for kind in ('register', 'removeInactive'):
            self.assertTrue(r(kind, activeBefore=8, victimCode=2).endswith(')'))


# ---------------------------------------------------------------- the fake node

REG = rf.REGISTRY
SEL = {v.hex(): k for k, v in rf.SELECTORS.items()}
GETTERS = {rf.keccak256(s.encode())[:4].hex(): s for s in ('get(uint256)', 'lengths()', 'recordWords(address)', 'removalStatus(address)',
                                                            'activeSlice(uint256,uint256)')}


class FakeSigner:
    """Test keys: random 32-byte files (as pocol-key writes them); the 'signature' is the transaction as JSON."""

    def __init__(self):
        self.keys = {}

    def generate(self, path):
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        secret = os.urandom(32)
        os.write(fd, secret)
        os.close(fd)
        return self.address(path)

    def address(self, key):
        return '0x' + hashlib.sha256(Path(key).read_bytes()).hexdigest()[:40]

    def sign(self, key, to, nonce, gas, max_fee, value=0, data=None):
        tx = {'from': self.address(key), 'to': to, 'nonce': nonce, 'gas': gas, 'maxFee': max_fee, 'value': value, 'data': data or '0x'}
        return '0x' + json.dumps(tx, sort_keys=True).encode().hex()


class FakeNode:
    """A one-node chain with the v3 registry's user-path rules that the harness meets: R_max and REG_OPS per block,
    Capacity at M_max, registerReplacing on a removable victim (Theta_inact), deregister, removeInactive only at a full
    set, and the refund sweep at dueHeight. Reverted transactions are included and pay their fee. Like pocold, it answers
    eth_getTransactionByHash and receipts only for canonical blocks (never for the pool), and the nonce at 'pending'
    counts the pool. reorg() replaces the head block with another at the same height. Like pocold's pool (node.rs
    admit_tx, park, replace): a transaction `park` selects is held outside the pending layer for `park_blocks` blocks and
    answered with POOL_PARKED; a resent pooled one is answered with its hash, a resent parked one with POOL_PARKED; another
    one at a held nonce is an underpriced replacement (pocol-sign's tip is fixed); a max fee below the base fee is
    refused; a block skips a transaction whose nonce is no longer its sender's."""
    POOL_PARKED = '-32016 pool full (gas): held for the next block'   # node.rs POOL_PARKED

    def __init__(self, cp, miner, operator, chain_id=777990):
        self.cp, self.chain_id, self.miner = cp, chain_id, miner
        self.height, self.base, self.ts = 0, 10**7, 1_700_000_000
        self.accounts = {operator: {'balance': 10**24, 'nonce': 0}}
        self.reg = {'records': {miner: self.record(1, 0)}, 'active': [miner]}
        self.snaps = {0: copy.deepcopy(self.reg)}
        self.acct_snaps = {0: copy.deepcopy(self.accounts)}
        self.blocks = {0: {'hash': '0x' + '00' * 32, 'parent': '0x' + '00' * 32, 'txs': [], 'ts': self.ts}}
        self.forks = 0
        self.revert_replacing = 0   # how many registerReplacing to revert (to show an M3 check failing)
        self.pool, self.receipts, self.included = [], {}, {}
        self.parked, self.park, self.park_blocks = [], None, 1
        self.external = 0   # registrations another registrant makes at the start of the next block
        self.others = 0
        self.broken = set()  # rules this fake leaves out, to show that the M3 checks can fail

    @staticmethod
    def record(bond_state, h, bond=0):
        return {'status': 1, 'bondState': bond_state, 'lastShare': 0, 'registeredAt': h, 'grace': h, 'exit': 0, 'due': 0, 'bond': bond}

    def acct(self, a):
        return self.accounts.setdefault(a, {'balance': 0, 'nonce': 0})

    def pending_nonce(self, a):
        return self.acct(a)['nonce'] + sum(1 for t in self.pool if t['from'] == a)

    def removal(self, reg, victim, height):
        r = reg['records'].get(victim)
        if r is None or r['status'] != 1:
            return 1
        n = len(reg['active'])
        if n < self.cp['M_max']:
            return 2
        if n <= self.cp['M_min']:
            return 3
        return 4 if height < max(r['lastShare'], r['grace']) + self.cp['Theta_inact'] else 0

    def mine(self, *_):
        h = self.height + 1
        reg, regs, ops = self.reg, 0, 0
        for _ in range(self.external):   # another registrant's transactions, first in the block
            self.others += 1
            a = '0x%040x' % (0xe0000 + self.others)
            self.acct(a)['balance'] = 10**18
            self.pool.insert(0, {'from': a, 'to': REG, 'nonce': 0, 'gas': rf.REGISTER_GAS, 'maxFee': rf.GWEI, 'value': 0,
                                 'data': rf.calldata('register(address)', a), 'hash': '0x%064x' % (0xe0000 + self.others)})
        self.external = 0
        held = [t for t in self.parked if t['parkedUntil'] <= h]   # admitted at a later head, ahead of later arrivals
        self.parked = [t for t in self.parked if t['parkedUntil'] > h]
        self.pool = held + self.pool
        reg['records'][self.miner]['lastShare'] = h - 1
        for a, r in list(reg['records'].items()):   # the refund sweep
            if r['status'] == 2 and h >= r['due']:
                del reg['records'][a]
        txs = []
        for t in self.pool:
            sender, acct = t['from'], self.acct(t['from'])
            if t['nonce'] != acct['nonce']:
                continue   # its nonce was used (or is ahead): not valid in this block, and gone from the pool
            acct['nonce'] += 1
            ok, used = True, 21000
            if t['to'] == REG:
                data = bytes.fromhex(t['data'][2:])
                sel = SEL.get(data[:4].hex())
                arg = lambda k: '0x' + data[4 + 32 * k + 12:4 + 32 * (k + 1)].hex()
                used = 174_450 if sel and sel.startswith('register') else 60_000
                full = len(reg['active']) >= self.cp['M_max'] and 'capacity' not in self.broken
                if sel == 'register(address)':
                    ok = regs < self.cp['R_max'] and ops < self.cp['REG_OPS'] and sender not in reg['records'] and not full
                    if ok:
                        reg['records'][sender] = self.record(1 if t['value'] == self.cp['B_reg'] else 4, h, t['value'])
                        reg['active'].append(sender)
                        regs, ops = regs + 1, ops + 1
                elif sel == 'registerReplacing(address,address)':
                    victim = arg(1)
                    ok = regs < self.cp['R_max'] and ops + 1 < self.cp['REG_OPS'] and sender not in reg['records'] and \
                        self.removal(reg, victim, h) == 0
                    if ok and self.revert_replacing:
                        self.revert_replacing -= 1
                        ok = False
                    if ok:
                        self.exit(reg, victim, h)
                        reg['records'][sender] = self.record(1 if t['value'] == self.cp['B_reg'] else 4, h, t['value'])
                        reg['active'].append(sender)
                        regs, ops = regs + 1, ops + 2
                elif sel == 'deregister()':
                    ok = sender in reg['records'] and reg['records'][sender]['status'] == 1 and len(reg['active']) > self.cp['M_min']
                    if ok:
                        self.exit(reg, sender, h)
                        ops += 1
                elif sel == 'removeInactive(address)':
                    ok = self.removal(reg, arg(0), h) == 0 or ('removal' in self.broken and reg['records'].get(arg(0), {}).get('status') == 1)
                    if ok:
                        self.exit(reg, arg(0), h)
                        ops += 1
                elif sel == 'claimBond()':
                    r = reg['records'].get(sender)
                    ok = bool(r) and r['status'] == 2 and h >= r['due']
                    if ok:
                        del reg['records'][sender]
                else:
                    ok = False
                if ok:
                    acct['balance'] -= t['value']
            else:
                acct['balance'] -= t['value']
                self.acct(t['to'])['balance'] += t['value']
            acct['balance'] -= used * self.base
            self.receipts[t['hash']] = {'status': hex(int(ok)), 'gasUsed': hex(used), 'transactionIndex': hex(len(txs)), 'blockNumber': hex(h)}
            self.included[t['hash']] = (h, t)
            txs.append(t)
        self.pool = []
        self.ts += 10
        self.height = h
        self.blocks[h] = {'hash': '0x%064x' % (h | self.forks << 128), 'parent': self.blocks[h - 1]['hash'], 'txs': txs, 'ts': self.ts}
        self.snaps[h] = copy.deepcopy(reg)
        self.acct_snaps[h] = copy.deepcopy(self.accounts)

    def reorg(self, depth=1, keep=None, drop=()):
        """Orphans the top `depth` blocks and mines as many others at the same heights (other hashes): the first holds
        the orphaned transactions `keep` selects, those in `drop` are forgotten (the new chain's nodes never saw them)
        and the rest return to the pool, for the block after."""
        h = self.height
        old = []
        for n in range(h - depth + 1, h + 1):
            old += self.blocks.pop(n)['txs']
        for t in old:
            self.receipts.pop(t['hash'], None)
            self.included.pop(t['hash'], None)
        base = h - depth
        self.height, self.ts = base, self.blocks[base]['ts']
        self.reg = copy.deepcopy(self.snaps[base])
        self.accounts = copy.deepcopy(self.acct_snaps[base])
        rest = [t for t in old if t['hash'] not in drop]
        kept = [t for t in rest if keep is not None and keep(t)]
        later = [t for t in rest if t not in kept]
        self.forks += 1
        pool, self.pool = self.pool, kept
        for _ in range(depth):
            self.mine()
        self.pool = later + pool

    def exit(self, reg, a, h):
        r = reg['records'][a]
        r.update({'status': 2, 'exit': h, 'due': h + self.cp['U']})
        reg['active'].remove(a)

    # -- RPC
    def call(self, method, params=()):
        p = list(params)
        h = self.height
        if method == 'eth_chainId':
            return hex(self.chain_id)
        if method == 'eth_blockNumber':
            return hex(h)
        if method == 'eth_gasPrice':
            return hex(self.base + rf.PRIORITY_WEI)
        if method == 'eth_getBlockByNumber':
            n = h if p[0] in ('latest', 'pending') else int(p[0], 16)
            b = self.blocks.get(n)
            if b is None:
                return None
            txs = [({'hash': t['hash'], 'to': t['to'], 'input': t['data'], 'from': t['from']} if p[1] else t['hash']) for t in b['txs']]
            return {'hash': b['hash'], 'parentHash': b['parent'], 'number': hex(n), 'timestamp': hex(b['ts']), 'transactions': txs,
                    'pocol': {'attempt': 0, 'signed': True}}
        if method == 'eth_getBalance':
            return hex(self.acct(p[0])['balance'])
        if method == 'eth_getTransactionCount':
            return hex(self.pending_nonce(p[0]) if p[1] == 'pending' else self.acct(p[0])['nonce'])
        if method == 'eth_getTransactionReceipt':
            return self.receipts.get(p[0])
        if method == 'eth_getTransactionByHash':
            return {'hash': p[0]} if p[0] in self.receipts else None   # canonical only, as pocold's find_tx
        if method == 'eth_sendRawTransaction':
            raw = p[0]
            t = json.loads(bytes.fromhex(raw[2:]))
            t['hash'] = rf.tx_hash(raw)   # keccak256 of the envelope, as pocold
            if any(x['hash'] == t['hash'] for x in self.pool):
                return t['hash']
            if any(x['hash'] == t['hash'] for x in self.parked):
                raise rf.RpcError(-32016, self.POOL_PARKED)
            if any(x['from'] == t['from'] and x['nonce'] == t['nonce'] for x in self.pool + self.parked):
                raise rf.RpcError(-32000, '-32000 replacement transaction underpriced (both caps must rise by 10%)')
            parked = sum(1 for x in self.parked if x['from'] == t['from'])
            if t['nonce'] != self.pending_nonce(t['from']) + parked:
                raise rf.RpcError(-32000, 'nonce')
            if t['maxFee'] < self.base:
                raise rf.RpcError(-32000, '-32000 max fee per gas less than block base fee')
            if self.acct(t['from'])['balance'] < t['gas'] * t['maxFee'] + t['value']:
                raise rf.RpcError(-32000, 'insufficient funds')
            if parked or (self.park and self.park(t)):
                t['parkedUntil'] = self.height + self.park_blocks
                self.parked.append(t)
                raise rf.RpcError(-32016, self.POOL_PARKED)
            self.pool.append(t)
            return t['hash']
        if method == 'eth_call':
            return self.eth_call(p[0]['to'], bytes.fromhex(p[0]['data'][2:]), p[1])
        if method == 'pocol_status':
            return {'miner': self.miner, 'members': {'reconstructions': 0, 'reconstructUsP99': 5, 'reconstructUsMax': 9},
                    'membership': {}, 'diskObserved': {'historyBytes': 1000 + h, 'stateBytes': 2000 + h, 'canonBytes': 0},
                    'a13': {'boundBytes': '1'}, 'stats': {'reorgs': 0}, 'retention': {'maxReorgDepth': 0, 'reorgsGeKAlarm': 0},
                    'pool': len(self.pool)}
        if method == 'pocol_getCaps':
            return {'accounts': hex(len(self.accounts)), 'occupancy': {'accounts': {'percent': 0.0}}}
        if method == 'pocol_getRemovable':
            return {'removable': [a for a in self.reg['active'] if self.removal(self.reg, a, h + 1) == 0]}
        if method in ('pocol_templateLatency', 'pocol_finality'):
            return {'all': {}}
        raise rf.RpcError(-32601, f'{method} not in the fake')

    def eth_call(self, to, data, tag):
        reg = self.reg if tag in ('latest', 'pending') else self.snaps[int(tag, 16)]
        height = (self.height if tag in ('latest', 'pending') else int(tag, 16)) + 1
        sig = GETTERS[data[:4].hex()]
        arg = lambda k: int.from_bytes(data[4 + 32 * k:4 + 32 * (k + 1)], 'big')
        enc = lambda *ws: '0x' + b''.join(rf.word(w) for w in ws).hex()
        if to == rf.CAPS:
            return enc(next(self.cp[k] for k, i in rf.CP_INDEX.items() if i == arg(0)))
        addr = '0x%040x' % arg(0)
        if sig == 'lengths()':
            return enc(len(reg['records']), len(reg['active']), 0, sum(r['bond'] for r in reg['records'].values()))
        if sig == 'removalStatus(address)':
            return enc(self.removal(reg, addr, height))
        if sig == 'recordWords(address)':
            r = reg['records'].get(addr)
            if r is None:
                return enc(0, 0, 0, 0)
            return enc(self.w0(r), int(addr, 16), r['bond'], r['exit'] | r['due'] << 64 | r['grace'] << 192)
        if sig == 'activeSlice(uint256,uint256)':
            ids = reg['active'][arg(0):arg(0) + arg(1)]
            n = len(ids)
            return enc(64, 64 + 32 * (n + 1), n, *[int(a, 16) for a in ids], n, *[self.w0(reg['records'][a]) for a in ids])
        raise AssertionError(sig)

    @staticmethod
    def w0(r):
        return r['status'] | r['bondState'] << 16 | r['lastShare'] << 88 | r['registeredAt'] << 152


class FakeRpc:
    def __init__(self, node):
        self.node, self.latency = node, {}

    def call(self, method, params=()):
        self.latency.setdefault(method, []).append(0.1)
        return self.node.call(method, params)

    def latency_summary(self):
        return {m: len(v) for m, v in self.latency.items()}


class Loop(unittest.TestCase):
    """The fill, M3 and replace loops against the fake node."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.signer = FakeSigner()
        op = self.root / 'op.key'
        self.op = self.signer.generate(op)
        self.cp = profile_cp(M_max=12, REG_RECORDS_MAX=64, Theta_inact=3, U=12, T_blk=1)
        self.node = FakeNode(self.cp, miner='0x' + 'ab' * 20, operator=self.op)
        self.out = io.StringIO()

    def harness(self, *argv):
        """A harness command, as a new process would run it: the previous command on the directory has ended (its lock
        is released)."""
        if getattr(self, 'last', None) is not None:
            self.last.close()
        args = rf.parser().parse_args([argv[0], '--dir', str(self.root / 'h'), '--operator-key', str(self.root / 'op.key'),
                                       '--poll', '0', *argv[1:]])
        self.last = None
        self.last = rf.Harness(args, rpc=FakeRpc(self.node), signer=self.signer, sleep=self.node.mine, out=self.out, now=time.time)
        self.addCleanup(self.last.close)
        return self.last

    def keys_never_shown(self, h):
        shown = self.out.getvalue() + (h.dir / 'state.json').read_text() + (h.dir / 'report.json').read_text()
        keys = list((h.dir).rglob('*.pocol-key'))
        self.assertTrue(keys)
        for k in keys:
            self.assertEqual(stat.S_IMODE(os.stat(k).st_mode), 0o600)
            secret = k.read_bytes()
            self.assertNotIn(secret.hex(), shown.lower())
        self.assertEqual(stat.S_IMODE(os.stat(h.dir).st_mode), 0o700)

    def test_fill_registers_to_a_full_set_at_most_r_max_a_block(self):
        h = self.harness('fill', '--count', '50', '--bonded', '2', '--fund-batch', '4')
        report = h.run_fill()
        self.assertEqual(report['registered'], 11, 'clamped to M_max 12 less the founder')
        self.assertEqual(report['plan']['clamped'], True)
        self.assertLessEqual(report['maxRegistrationsInABlock'], self.cp['R_max'])
        self.assertEqual(report['maxRegistrationsInABlock'], 4)
        self.assertEqual(report['census'], {'active': 12, 'byBondState': {'HELD': 3, 'ACCRUING': 9}})
        self.assertEqual(report['registeredAndActiveAtEnd'], 11)
        self.assertEqual(report['failuresByReason'], {})
        self.assertEqual(report['byRole'], {'miner': 0, 'bonded': 2, 'idle': 9})
        for b, txs in self.node.blocks.items():
            regs = sum(1 for t in txs['txs'] if t['to'] == REG and self.node.receipts[t['hash']]['status'] == '0x1')
            self.assertLessEqual(regs, self.cp['R_max'], f'block {b}')
        m = report['measures']
        self.assertEqual(m['blockIntervalS']['mean'], 10)
        self.assertIn('historyGrowthBytesPerDay', m)
        self.assertTrue(any('child_ctx' in x for x in m['notMeasuredHere']))
        self.keys_never_shown(h)

    def test_a_competing_registrant_is_reported_and_retried(self):
        self.cp['M_max'] = 64
        h = self.harness('fill', '--count', '6', '--fund-batch', '8', '--attempts', '6')
        real = self.node.mine
        hits = []

        def mine(*a):
            # Another registrant uses three of the four registrations of every block in which the harness has any.
            if any(t['to'] == REG for t in self.node.pool):
                self.node.external = 3
                hits.append(self.node.height + 1)
            real()
        h.sleep = mine
        report = h.run_fill()
        self.assertTrue(hits)
        self.assertEqual(report['registered'], 6, 'every identity registers on a retry')
        self.assertGreater(report['failuresByReason'].get('RegOpsLimit', 0), 0, report['failuresByReason'])
        self.assertEqual(report['maxRegistrationsInABlock'], 1)

    def test_a_set_filled_by_others_stops_without_sending(self):
        h = self.harness('fill', '--count', '4')
        real = self.node.mine

        def mine(*a):
            if self.node.height == h.state['runs'][-1]['startHeight']:
                for i in range(10):   # someone else fills the set after the plan was made
                    a = '0x%040x' % (1000 + i)
                    self.node.reg['records'][a] = self.node.record(4, 0)
                    self.node.reg['active'].append(a)
            real()
        h.sleep = mine
        report = h.run_fill()
        self.assertEqual(report['registered'], 1)
        self.assertEqual(report['failuresByReason'].get('final: not sent: Capacity'), 3, report['failuresByReason'])

    def test_refusals_before_anything_is_sent(self):
        self.node.chain_id = rf.MAINNET_CHAIN_ID
        with self.assertRaisesRegex(rf.Refused, 'is the mainnet'):
            self.harness('fill', '--count', '1')
        self.assertFalse((self.root / 'h').exists(), 'no directory or key before the chain check')
        self.node.chain_id = 777990
        self.node.miner = self.op
        with self.assertRaisesRegex(rf.Refused, "node's miner key"):
            self.harness('fill', '--count', '1').run_fill()
        self.node.miner = '0x' + 'ab' * 20
        self.node.reg['records'][self.op] = self.node.record(1, 0)
        with self.assertRaisesRegex(rf.Refused, 'registry record'):
            self.harness('fill', '--count', '1').run_fill()
        del self.node.reg['records'][self.op]
        self.node.accounts[self.op]['balance'] = 10**15
        with self.assertRaisesRegex(rf.Refused, 'holds less than'):
            self.harness('fill', '--count', '1').run_fill()
        self.assertEqual(list((self.root / 'h' / 'keys').iterdir()), [], 'no key is generated for a refused plan')
        self.assertEqual(self.node.pool, [])
        with self.assertRaisesRegex(rf.Refused, 'expect-chain-id'):
            self.harness('fill', '--expect-chain-id', str(rf.REHEARSAL_CHAIN_ID))
        self.harness('fill').save()
        self.node.chain_id = 777991
        with self.assertRaisesRegex(rf.Refused, 'belongs to chain 777990'):
            self.harness('fill', '--count', '1')

    def test_an_interrupted_run_resumes(self):
        h = self.harness('fill', '--count', '4')
        real = self.node.mine

        def interrupt(*a):
            # The run stops right after its first registrations are sent (the process dies with them in flight).
            if any(t['to'] == REG for t in self.node.pool):
                h.stopping = True
                return
            real()
        h.sleep = interrupt
        h.run_fill()
        sent = [a for i in h.state['identities'] for a in i['attempts']]
        self.assertTrue(sent and all('status' not in a for a in sent), 'in flight when it stopped')
        h2 = self.harness('fill', '--count', '4')
        report = h2.run_fill()
        self.assertEqual(report['registered'], 4)
        self.assertEqual(len(h2.state['identities']), 4, 'the same identities, none generated again')
        self.assertEqual(sum(len(i['attempts']) for i in h2.state['identities']), 4, 'nothing sent twice')

    def test_dry_run_sends_nothing(self):
        h = self.harness('fill', '--count', '3', '--dry-run')
        self.assertEqual(h.run_fill()['plan']['planned'], 3)
        self.assertEqual(self.node.pool, [])
        self.assertEqual(h.state['identities'], [])

    def test_m3_capacity_path(self):
        self.harness('fill', '--count', '11').run_fill()
        with self.assertRaisesRegex(rf.Refused, 'full set'):
            self.node.reg['active'].pop()
            self.harness('m3').run_m3()
        self.node.reg['active'].append([a for a in self.node.reg['records'] if a not in self.node.reg['active']][0])
        h = self.harness('m3', '--reregister-after-u')
        result = h.run_m3()
        c = result['checks']
        self.assertTrue(result['pass'], json.dumps(c, indent=1))
        self.assertEqual(len(c['twoReplacingSameBlock']['blocks']), 1)
        self.assertEqual(len(set(c['twoReplacingSameBlock']['victims'])), 2)
        self.assertTrue(c['registerAloneCapacity']['reason'].startswith('Capacity'))
        self.assertEqual(c['removeInactiveBelowFullSet']['removalCodeBefore'], 2)
        self.assertTrue(c['victimReregistersAfterU']['recordErased'])
        self.assertEqual(c['victimReregistersAfterU']['kind'], 'registerReplacing')
        self.keys_never_shown(h)

    def test_m3_reports_a_rule_that_does_not_hold(self):
        self.harness('fill', '--count', '11').run_fill()
        self.node.broken = {'capacity', 'removal'}
        result = self.harness('m3').run_m3()
        c = result['checks']
        self.assertFalse(result['pass'])
        self.assertTrue(c['twoReplacingSameBlock']['pass'])
        self.assertEqual((c['registerAloneCapacity']['pass'], c['registerAloneCapacity']['status']), (False, 1))
        self.assertEqual((c['removeInactiveBelowFullSet']['pass'], c['removeInactiveBelowFullSet']['status']), (False, 1))

    def test_replace_names_its_victim(self):
        self.harness('fill', '--count', '11').run_fill()
        h = self.harness('replace', '--victim', self.node.miner)
        with self.assertRaisesRegex(RuntimeError, 'did not become removable'):
            h.args.wait_blocks = 3
            h.run_replace()   # the founder mines every block
        victim = next(a for a in self.node.reg['active'] if a != self.node.miner)
        h = self.harness('replace', '--victim', victim, '--watch-refund')
        out = h.run_replace()
        self.assertEqual((out['status'], out['after']['status']), (1, 'EXITED'))
        self.assertIsNone(out['atDue']['record'], 'refunded and erased at dueHeight')

    def test_delete_keys_refuses_while_a_miner_runs(self):
        # A stand-in `pocol` that sleeps, so start_miners really starts the mining identity's node; the fill runs with a
        # relative --dir, and stop-miners and delete-keys are given other spellings of it, from another directory.
        bin_dir = self.root / 'bin'
        bin_dir.mkdir()
        (bin_dir / 'pocol').write_text('#!' + sys.executable + '\nimport time\ntime.sleep(60)\n', encoding='utf-8')
        os.chmod(bin_dir / 'pocol', 0o700)
        (self.root / 'network.pocol.json').write_text('{}', encoding='utf-8')
        cwd = os.getcwd()
        self.addCleanup(os.chdir, cwd)
        os.chdir(self.root)
        args = rf.parser().parse_args(['fill', '--dir', 'h', '--operator-key', 'op.key', '--poll', '0', '--count', '2', '--miners', '1',
                                       '--network', 'network.pocol.json', '--bin-dir', str(bin_dir)])
        h = rf.Harness(args, rpc=FakeRpc(self.node), signer=self.signer, sleep=self.node.mine, out=self.out, now=time.time)
        h.run_fill()
        for op in (rf.delete_keys, rf.stop_miners):
            with self.assertRaisesRegex(rf.Refused, 'in use by another harness command'):
                op(str((self.root / 'h').resolve()))
        h.close()   # the fill command has ended
        self.assertEqual(len(h.state['miners']), 1)
        miner = h.state['miners'][0]
        self.addCleanup(lambda: rf.miner_running(miner, self.root / 'h') and os.kill(miner['pid'], 9))
        self.assertTrue(Path(miner['home']).is_absolute())
        self.assertEqual(Path(miner['home']).parent.name, 'miners')
        absolute = str((self.root / 'h').resolve())
        os.chdir('/')
        self.assertTrue(rf.miner_running(miner, absolute))
        with self.assertRaisesRegex(rf.Refused, 'stop them first'):
            rf.delete_keys(absolute)
        os.chdir(self.root)
        with self.assertRaisesRegex(rf.Refused, 'stop them first'):
            rf.delete_keys('./h/')
        os.chdir('/')
        self.assertEqual(rf.stop_miners(absolute), [miner['pid']])
        h.miner_procs[0].wait(timeout=20)   # the harness (this process) is the stand-in's parent
        self.assertFalse(rf.miner_running(miner, absolute))
        with self.assertRaisesRegex(rf.Refused, 'discard-funds'):
            rf.delete_keys(absolute)
        self.assertEqual(len(list((self.root / 'h').rglob('*.pocol-key'))), 2)
        self.assertEqual(rf.delete_keys(absolute, discard_funds=True), 2)
        self.assertEqual(list((self.root / 'h').rglob('*.pocol-key')), [])
        self.assertTrue(json.loads((self.root / 'h' / 'state.json').read_text())['keysDeleted'])

    def test_a_miner_whose_command_line_cannot_be_read_counts_as_running(self):
        proc = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
        self.addCleanup(proc.wait)
        self.addCleanup(proc.kill)
        h = self.harness('fill', '--count', '1')
        h.run_fill()
        h.state['miners'] = [{'i': 0, 'pid': proc.pid, 'home': str(self.root / 'h' / 'miners' / '000000')}]
        h.save()
        h.close()
        real = rf.command_line
        rf.command_line = lambda pid: None   # a host with neither /proc nor ps
        try:
            self.assertIsNone(rf.miner_running(h.state['miners'][0], h.dir))
            with self.assertRaisesRegex(rf.Refused, 'stop them first'):
                rf.delete_keys(h.dir)
            with self.assertRaisesRegex(rf.Refused, 'cannot be read'):
                rf.stop_miners(h.dir)
        finally:
            rf.command_line = real
        self.assertIsNone(proc.poll(), 'an unverified pid is never signalled')
        self.assertTrue(list(h.dir.rglob('*.pocol-key')))
        # The ps fallback (no /proc entry read): the home is found in the command string.
        self.assertFalse(rf.miner_running(h.state['miners'][0], h.dir), 'another command on that pid is not the miner')

    def test_an_interrupted_run_with_dropped_transactions_sends_them_again(self):
        h = self.harness('fill', '--count', '4')
        real = self.node.mine

        def interrupt(*a):
            if any(t['to'] == REG for t in self.node.pool):
                h.stopping = True
                return
            real()
        h.sleep = interrupt
        h.run_fill()
        lost = {t['hash'] for t in self.node.pool if t['to'] == REG}
        self.node.pool = []   # the node restarted and lost its pool
        h2 = self.harness('fill', '--count', '4')
        report = h2.run_fill()
        self.assertEqual(report['registered'], 4)
        self.assertEqual(report['failuresByReason'], {})
        for ident in h2.state['identities']:
            # The same signed transaction (same hash, same nonce) was sent again: no second signature at that nonce.
            self.assertEqual([(a['status'], a['resent']) for a in ident['attempts']], [(1, 1)])
            self.assertIn(ident['attempts'][0]['tx'], lost)

    def test_an_unrecorded_key_from_an_interrupted_generation_is_adopted(self):
        h = self.harness('fill', '--count', '3')
        keys = h.dir / 'keys'
        orphan = self.signer.generate(keys / '000000.pocol-key')   # generated, then the process died before save()
        incomplete = keys / '000001.pocol-key'
        incomplete.write_bytes(b'')
        os.chmod(incomplete, 0o600)
        self.signer.address = lambda key, real=self.signer.address: (_ for _ in ()).throw(RuntimeError('bad key')) \
            if Path(key).stat().st_size != 32 else real(key)
        report = h.run_fill()
        self.assertEqual(report['registered'], 3)
        ids = h.state['identities']
        self.assertEqual(ids[0]['address'], orphan)
        self.assertEqual(incomplete.stat().st_size, 32, 'the incomplete one was replaced by a fresh key')
        self.assertEqual(sorted(p.name for p in keys.iterdir()), ['000000.pocol-key', '000001.pocol-key', '000002.pocol-key'])

    def test_a_reorg_at_a_processed_height_is_undone_and_redone(self):
        self.cp['M_max'] = 64
        h = self.harness('fill', '--count', '12', '--fund-batch', '12', '--include-timeout', '2', '--attempts', '3')
        real = self.node.mine
        done, forgotten = [], []
        ours = lambda n: [t for t in self.node.blocks[n]['txs'] if t['to'] == REG and t['from'] in
                          {i['address'] for i in h.state['identities']}]

        def mine(*a):
            # The harness has processed the head when it sleeps.
            top = self.node.height
            if not done and len(ours(top)) >= 3:
                # Another block at the same height: only the first registration is in it, the second is forgotten and
                # the others go into the next block.
                regs = ours(top)
                self.node.reorg(keep=lambda t: t['hash'] == regs[0]['hash'] or t['to'] != REG, drop={regs[1]['hash']})
                forgotten.append(regs[1]['hash'])
                done.append(('same height', top))
            elif len(done) == 1 and top >= done[0][1] + 2 and ours(top) and ours(top - 1):
                # Two blocks deep, both holding settled registrations: all of them go into the block after.
                self.node.reorg(depth=2)
                done.append(('two deep', top))
            real()
        h.sleep = mine
        report = h.run_fill()
        self.assertEqual([d[0] for d in done], ['same height', 'two deep'])
        self.assertEqual(h.state.get('reorgsSeen'), 2)
        self.assertEqual(report['registered'], 12)
        self.assertEqual(report['registeredAndActiveAtEnd'], 12)
        # The harness's per-block record matches the canonical chain exactly.
        ours = {i['address'] for i in h.state['identities']}
        chain = {}
        for n in range(1, self.node.height + 1):
            k = sum(1 for t in self.node.blocks[n]['txs'] if t['from'] in ours and t['to'] == REG
                    and self.node.receipts[t['hash']]['status'] == '0x1')
            if k:
                chain[str(n)] = k
        self.assertEqual(h.state['perBlock'], chain)
        for ident in h.state['identities']:
            ok = [a for a in ident['attempts'] if a.get('status') == 1]
            self.assertEqual(len(ok), 1)
            self.assertEqual(self.node.receipts[ok[0]['tx']]['blockNumber'], hex(ok[0]['block']))
            self.assertEqual(ident['registeredAt'], ok[0]['block'])
        # The forgotten registration was sent again unchanged (no other signature at its nonce) and settled from its
        # canonical receipt (the two-deep reorg may have crowded its block: RegOpsLimit, then a retry).
        again = [a for i in h.state['identities'] for a in i['attempts'] if a['tx'] == forgotten[0]]
        self.assertEqual(len(again), 1)
        self.assertGreaterEqual(again[0].get('resent', 0), 1)
        self.assertEqual(self.node.receipts[forgotten[0]]['blockNumber'], hex(again[0]['block']))
        self.assertEqual(self.node.receipts[forgotten[0]]['status'], hex(again[0]['status']))
        self.assertNotIn('not included', report['failuresByReason'])

    def test_m3_records_a_failed_replacement_truthfully(self):
        self.harness('fill', '--count', '11').run_fill()
        self.node.revert_replacing = 1
        h = self.harness('m3')
        result = h.run_m3()
        c = result['checks']['twoReplacingSameBlock']
        self.assertFalse(result['pass'])
        self.assertFalse(c['pass'])
        self.assertEqual(sorted(c['statuses']), [0, 1])
        by = {i['address']: i for i in h.state['identities']}
        for victim, status, record in zip(c['victims'], c['statuses'], [c['victimRecords'][v] for v in c['victims']]):
            if status == 1:
                self.assertEqual((by[victim]['outcome'], record['status']), ('replaced (m3)', 'EXITED'))
            else:
                self.assertEqual(by[victim]['outcome'], 'registered', 'its replacement reverted: the victim keeps its seat')
                self.assertEqual(record['status'], 'ACTIVE')
        failed = [i for i in h.state['identities'] if i['role'] == 'm3' and (i['outcome'] or '').startswith('failed')]
        self.assertEqual(len(failed), 1, 'the reverted newcomer is not recorded as registered')
        self.assertTrue(result['checks']['removeInactiveBelowFullSet']['pass'], 'the later checks are not misattributed')

    # ---- review round 2: what the node holds, write-ahead sends, resume, reorgs across a stop, the census, stops, locks

    def chain_per_block(self, h):
        """The harness's successful registrations per block, as the fake's canonical chain holds them."""
        ours = {i['address'] for i in h.state['identities']}
        chain = {}
        for n in range(1, self.node.height + 1):
            k = sum(1 for t in self.node.blocks[n]['txs'] if t['from'] in ours and t['to'] == REG
                    and SEL.get(t['data'][2:10], '').startswith('register') and self.node.receipts[t['hash']]['status'] == '0x1')
            if k:
                chain[str(n)] = k
        return chain

    def assert_follows_the_chain(self, h, report):
        self.assertEqual(h.state['perBlock'], self.chain_per_block(h))
        for ident in h.state['identities']:
            onchain = self.node.reg['records'].get(ident['address'])
            self.assertEqual(ident['outcome'] == 'registered', onchain is not None, ident)
            if onchain is not None:
                self.assertEqual(ident['registeredAt'], onchain['registeredAt'])
        self.assertEqual(report['registered'], sum(1 for i in h.state['identities'] if i['address'] in self.node.reg['records']))
        for n, b in self.node.blocks.items():
            self.assertLessEqual(int(self.chain_per_block(h).get(str(n), 0)), self.cp['R_max'], n)

    def test_a_parked_registration_is_in_flight(self):
        # pocold answers a valid transaction it parks at the pool's gas bound with -32016 "... held for the next block"
        # and includes it later: the harness tracks it (and counts it against R_max), never signs another at n+1.
        self.cp['M_max'] = 64
        h = self.harness('fill', '--count', '8', '--fund-batch', '8')
        parked = []
        self.node.park = lambda t: t['to'] == REG and not parked and not parked.append(t['hash'])
        report = h.run_fill()
        self.assertEqual(len(parked), 1)
        self.assertEqual((report['registered'], report['registeredAndActiveAtEnd']), (8, 8))
        self.assertEqual(report['failuresByReason'], {})
        for ident in h.state['identities']:
            self.assertEqual([a['status'] for a in ident['attempts']], [1], ident)
        self.assertLessEqual(report['maxRegistrationsInABlock'], self.cp['R_max'])
        self.assert_follows_the_chain(h, report)

    def test_a_held_transaction_is_never_signed_again(self):
        # Held outside the pending layer (parked) past --include-timeout while the base fee moves: the harness sends the
        # same envelope again (answered held), never a new signature at that nonce (an underpriced replacement).
        h = self.harness('fill', '--count', '1', '--include-timeout', '2')
        self.node.park = lambda t: t['to'] == REG
        self.node.park_blocks = 6
        real = self.node.mine

        def mine(*a):
            if self.node.parked:
                self.node.base = 2 * self.node.base   # another fee for any new signature
                self.node.park = None
            real()
        h.sleep = mine
        report = h.run_fill()
        (ident,) = h.state['identities']
        self.assertEqual([(a['status'], a['nonce']) for a in ident['attempts']], [(1, 0)])
        self.assertGreaterEqual(ident['attempts'][0]['resent'], 1)
        self.assertEqual(report['failuresByReason'], {})
        self.assert_follows_the_chain(h, report)

    def test_a_transaction_given_up_and_then_included_is_settled(self):
        # The node loses A and refuses it again (its fee is below the base fee now), twice: given up, the identity
        # signs B at the same nonce. A peer then includes A: B's nonce is used, A's receipt settles the identity at
        # once (with attempts to spare it signs nothing more: no third transaction, at nonce 1, reverting BadState).
        h = self.harness('fill', '--count', '1', '--include-timeout', '2', '--attempts', '3')
        real = self.node.mine
        st = {}

        def mine(*a):
            regs = [t for t in self.node.pool if t['to'] == REG]
            if 'A' not in st and regs:
                st['A'] = regs[0]
                self.node.pool.remove(regs[0])
                self.node.base = 15 * 10**8   # 1.5 gwei: above A's max fee, below the fee cap
            elif 'A' in st and 'B' not in st and regs:
                st['B'] = regs[0]
                self.node.pool.remove(regs[0])
                self.node.pool.insert(0, st['A'])   # a peer's block carries A
            real()
        h.sleep = mine
        report = h.run_fill()
        (ident,) = h.state['identities']
        by = {a['tx']: a for a in ident['attempts']}
        self.assertEqual(set(by), {st['A']['hash'], st['B']['hash']})
        self.assertEqual(by[st['A']['hash']]['status'], 1)
        self.assertEqual(by[st['B']['hash']]['reason'], 'not included (its nonce was used by another transaction)')
        self.assertEqual(ident['outcome'], 'registered')
        self.assertEqual(ident['registeredAt'], int(self.node.receipts[st['A']['hash']]['blockNumber'], 16))
        self.assertEqual(report['registeredAndActiveAtEnd'], 1)
        self.assertNotIn('final: failed', ' '.join(report['failuresByReason']))
        self.assert_follows_the_chain(h, report)

    def test_a_resumed_run_does_not_plan_its_settled_registrations_again(self):
        h = self.harness('fill', '--count', '4')
        real = self.node.mine

        def interrupt(*a):
            if any(t['to'] == REG for t in self.node.pool):
                h.stopping = True
                return
            real()
        h.sleep = interrupt
        h.run_fill()
        self.node.mine()   # included while the harness is stopped
        h2 = self.harness('fill', '--count', '4')
        report = h2.run_fill()
        self.assertEqual(report['plan']['planned'], 0)
        self.assertEqual((report['identities'], report['registered']), (4, 4))

    def test_a_reorg_while_stopped_is_undone_on_the_next_run(self):
        for drop in (True, False):
            with self.subTest(dropped=drop):
                self.setUp()
                h = self.harness('fill', '--count', '2')
                real = self.node.mine

                def mine(*a, h=h):
                    if any(i['outcome'] == 'registered' for i in h.state['identities']):
                        h.stopping = True   # stopped right after its registrations settled
                        return
                    real()
                h.sleep = mine
                h.run_fill()
                top = self.node.height
                self.assertTrue(all(i['registeredAt'] == top for i in h.state['identities']))
                # The head block is orphaned while the harness is down: the new chain's nodes never saw the two
                # registrations, or they return to the pool for the block after.
                regs = {t['hash'] for t in self.node.blocks[top]['txs'] if t['to'] == REG}
                self.node.reorg(drop=regs if drop else ())
                h2 = self.harness('fill', '--count', '2')
                report = h2.run_fill()
                self.assertEqual(h2.state['reorgsSeen'], 1)
                self.assertEqual((report['registered'], report['registeredAndActiveAtEnd']), (2, 2))
                for ident in h2.state['identities']:
                    self.assertGreater(ident['registeredAt'], top)
                    self.assertLessEqual(ident['registeredAt'], top + 2, 'checked at once, not after --include-timeout')
                    # The orphaned registration itself settled again: sent again unchanged when the new chain lost it.
                    self.assertEqual([a['status'] for a in ident['attempts']], [1])
                    self.assertIn(ident['attempts'][0]['tx'], regs)
                self.assert_follows_the_chain(h2, report)

    def test_the_census_reads_one_height(self):
        self.cp['M_max'] = 64
        h = self.harness('fill', '--count', '3', '--fund-batch', '8')
        h.run_fill()
        real = h.rpc.call
        moved = []

        def call(method, params=()):
            if method == 'eth_call' and params[0]['data'].startswith(rf.calldata('activeSlice(uint256,uint256)', 0, 0)[:10]) and not moved:
                moved.append(self.node.height)
                self.node.external = 2   # a block lands between lengths() and the activeSlice page
                self.node.mine()
            return real(method, params)
        h.rpc.call = call
        census, ids = h.chain.census()
        self.assertEqual(moved, [self.node.height - 1])
        self.assertEqual((census['active'], len(ids)), (4, 4), 'read at the height before the block')

    def test_a_failure_between_two_sends_loses_nothing(self):
        # The node goes away after the first registration of a batch is sent (an OSError out of the RPC client, or a
        # Ctrl-C): every registration of the batch was recorded before any was sent, so the next run neither loses the
        # first nor signs a second one at nonce 1.
        self.cp['M_max'] = 64
        h = self.harness('fill', '--count', '4', '--fund-batch', '8')
        real = h.rpc.call
        sent = []

        def call(method, params=()):
            if method == 'eth_sendRawTransaction' and json.loads(bytes.fromhex(params[0][2:]))['to'] == REG:
                if len(sent) == 1:
                    raise ConnectionResetError('the node went away')
                sent.append(params[0])
            return real(method, params)
        h.rpc.call = call
        with self.assertRaises(ConnectionResetError):
            h.run_fill()
        saved = json.loads((h.dir / 'state.json').read_text())
        self.assertEqual(sum(1 for i in saved['identities'] for a in i['attempts'] if a.get('tx')), 4, 'all four recorded')
        h2 = self.harness('fill', '--count', '4', '--fund-batch', '8')
        report = h2.run_fill()
        self.assertEqual((report['identities'], report['registered']), (4, 4))
        self.assertEqual(report['failuresByReason'], {})
        for ident in h2.state['identities']:
            self.assertEqual([(a['status'], a['nonce']) for a in ident['attempts']], [(1, 0)])
        self.assert_follows_the_chain(h2, report)

    def test_m3_stops_without_inventing_outcomes(self):
        self.harness('fill', '--count', '11').run_fill()
        h = self.harness('m3')
        real = self.node.mine

        def mine(*a):
            if any(SEL.get(t['data'][2:10]) == 'registerReplacing(address,address)' for t in self.node.pool):
                h.stopping = True   # SIGINT with both registerReplacing pooled
                return
            real()
        h.sleep = mine
        with self.assertRaises(rf.Stopped):
            h.run_m3()
        newcomers = [i for i in h.state['identities'] if i['role'] == 'm3' and i['attempts']]
        self.assertEqual(len(newcomers), 2)
        self.assertTrue(all(i['outcome'] is None and 'status' not in i['attempts'][-1] for i in newcomers))
        self.assertEqual(sum(1 for t in self.node.pool if t['to'] == REG), 2, 'nothing more was sent')
        victims = [i['attempts'][-1]['victim'] for i in newcomers]
        by = {i['address']: i for i in h.state['identities']}
        self.assertTrue(all(by[v]['outcome'] == 'registered' for v in victims))
        self.node.mine()   # included while stopped
        h2 = self.harness('m3')
        h2.resume()
        by = {i['address']: i for i in h2.state['identities']}
        for n, v in zip(newcomers, victims):
            self.assertEqual(by[n['address']]['outcome'], 'registered')
            self.assertEqual(by[v]['outcome'], 'replaced (m3)')

    def test_one_command_at_a_time_on_a_directory(self):
        h = self.harness('fill', '--count', '1')
        h.run_fill()
        args = rf.parser().parse_args(['fill', '--dir', str(h.dir), '--operator-key', str(self.root / 'op.key'), '--count', '1'])
        with self.assertRaisesRegex(rf.Refused, 'in use by another harness command'):
            rf.Harness(args, rpc=FakeRpc(self.node), signer=self.signer, sleep=self.node.mine, out=self.out)
        with self.assertRaisesRegex(rf.Refused, 'in use by another harness command'):
            rf.delete_keys(h.dir)
        self.assertTrue(list(h.dir.rglob('*.pocol-key')))
        status = lambda: rf.Harness(rf.parser().parse_args(['status', '--dir', str(h.dir), '--poll', '0']), rpc=FakeRpc(self.node),
                                    signer=self.signer, sleep=self.node.mine, out=self.out).run_status()
        self.assertEqual(status()['harnessActive'], 1, 'status only reads, beside a running command')
        h.close()
        self.assertEqual(rf.delete_keys(h.dir, discard_funds=True), 1)
        with self.assertRaisesRegex(rf.Refused, 'were deleted'):
            self.harness('fill', '--count', '1')
        self.assertEqual(status()['harnessActive'], 1)

    # ---- review round 3: reorgs after the chain moved on, operator nonces, one envelope signed twice, funding, stops

    def stopped_after_settling(self, *argv):
        """A fill stopped right after its registrations settled; returns the harness."""
        h = self.harness('fill', *argv)
        real = self.node.mine

        def mine(*a):
            if any(i['outcome'] == 'registered' for i in h.state['identities']):
                h.stopping = True
                return
            real()
        h.sleep = mine
        h.run_fill()
        return h

    def test_a_reorg_while_stopped_is_undone_after_the_chain_moved_on(self):
        # As after a section 4 drill: the head block is orphaned while the harness is down, the new chain never saw the
        # two registrations, and it grows before the next run. The next run compares every hash it kept.
        for drop in (True, False):
            with self.subTest(dropped=drop):
                self.setUp()
                h = self.stopped_after_settling('--count', '2')
                top = self.node.height
                regs = {t['hash'] for t in self.node.blocks[top]['txs'] if t['to'] == REG}
                self.node.reorg(drop=regs if drop else ())
                for _ in range(3):
                    self.node.mine()
                h2 = self.harness('fill', '--count', '2')
                report = h2.run_fill()
                self.assertEqual(h2.state['reorgsSeen'], 1)
                self.assertEqual((report['registered'], report['registeredAndActiveAtEnd']), (2, 2))
                for ident in h2.state['identities']:
                    self.assertGreater(ident['registeredAt'], top)
                    self.assertEqual([a['status'] for a in ident['attempts']], [1])
                    self.assertIn(ident['attempts'][0]['tx'], regs)
                self.assert_follows_the_chain(h2, report)

    def test_a_two_deep_reorg_while_stopped_is_undone(self):
        # Two blocks deep while stopped (the lower one holds the harness's registrations, the upper one, processed by a
        # later run, none), and the chain grows before the next run: the fork is the lower height.
        self.cp['M_max'] = 64
        h = self.stopped_after_settling('--count', '2')
        top = self.node.height
        self.node.mine()
        h = self.harness('fill', '--count', '2', '--dry-run')   # processes the empty block above, keeps its hash
        h.run_fill()
        self.assertIn(top + 1, h.hashes)
        h.save()
        self.node.reorg(depth=2, drop={t['hash'] for t in self.node.blocks[top]['txs'] if t['to'] == REG})
        for _ in range(2):
            self.node.mine()
        report = self.harness('fill', '--count', '2').run_fill()
        self.assertEqual(self.last.state['reorgsSeen'], 1)
        self.assertEqual(report['registeredAndActiveAtEnd'], 2)
        self.assert_follows_the_chain(self.last, report)

    def test_replace_never_signs_over_a_parked_operator_transfer(self):
        # A replace is stopped while its funding transfer is parked at the pool's gas bound (outside 'pending'); the
        # next replace waits for that transfer before it signs another, so no second transfer at its nonce is signed.
        self.harness('fill', '--count', '11').run_fill()
        victim = next(a for a in self.node.reg['active'] if a != self.node.miner)
        self.node.park = lambda t: t['to'] != REG
        self.node.park_blocks = 8
        h = self.harness('replace', '--victim', victim)
        real = self.node.mine

        def mine(*a):
            if self.node.parked:
                h.stopping = True
                self.node.park = None
            real()
        h.sleep = mine
        with self.assertRaises(rf.Stopped):
            h.run_replace()
        h.save()
        self.assertTrue(self.node.parked, 'the transfer is parked when the run stops')
        out = self.harness('replace', '--victim', victim).run_replace()
        self.assertEqual(out['status'], 1, out)
        self.assertIsNone(self.last.state.get('fundRefusals'))
        ops = [t['nonce'] for b in self.node.blocks.values() for t in b['txs'] if t['from'] == self.op]
        self.assertEqual(len(ops), len(set(ops)), 'one transfer per operator nonce')

    def test_an_identity_left_unfunded_is_an_error_not_an_attempt(self):
        self.harness('fill', '--count', '11').run_fill()
        victim = next(a for a in self.node.reg['active'] if a != self.node.miner)
        real = self.node.call

        def call(method, params=()):
            if method == 'eth_sendRawTransaction' and json.loads(bytes.fromhex(params[0][2:]))['to'] != REG:
                raise rf.RpcError(-32000, '-32000 nonce too low')
            return real(method, params)
        self.node.call = call
        h = self.harness('replace', '--victim', victim, '--include-timeout', '2')
        with self.assertRaisesRegex(RuntimeError, 'could not be funded'):
            h.run_replace()
        self.assertFalse(h.state.get('replacements'), 'no replacement recorded as failed for a harness reason')

    def test_one_envelope_signed_twice_settles_once(self):
        # The fee cap binds (base 1.5 gwei), so a retry at the same nonce is the same envelope and hash. It is lost,
        # given up, signed again (the same hash), given up again, and then included by a peer while the harness is
        # stopped. Every later run reads one registration in that block, and the given-up copies are not failures.
        self.node.base = 15 * 10**8
        h = self.harness('fill', '--count', '1', '--include-timeout', '2', '--attempts', '2')
        real = self.node.mine
        st = {'stage': 0}

        def mine(*a):
            regs = [t for t in self.node.pool if t['to'] == REG]
            ident = h.state['identities'][0] if h.state['identities'] else None
            if st['stage'] in (0, 2) and regs:
                st['A'] = regs[0]
                self.node.pool.remove(regs[0])   # lost by the node
                self.node.base = 35 * 10**8      # its resends are refused
                st['stage'] += 1
            elif st['stage'] in (1, 3) and sum(1 for x in ident['attempts'] if x.get('status') == 0) == (st['stage'] + 1) // 2:
                self.node.base = 15 * 10**8
                st['stage'] += 1
            real()
        h.sleep = mine
        h.run_fill()
        (ident,) = h.state['identities']
        self.assertEqual(len({a['tx'] for a in ident['attempts']}), 1, 'the same envelope, signed twice')
        self.assertTrue(ident['outcome'].startswith('failed'))
        self.node.base = 15 * 10**8
        self.node.pool.insert(0, st['A'])   # a peer includes it after all
        self.node.mine()
        for run in range(3):
            report = self.harness('fill', '--count', '1').run_fill()
            (ident,) = self.last.state['identities']
            self.assertEqual(ident['outcome'], 'registered')
            self.assertEqual([a['status'] for a in ident['attempts']], [1, 1])
            self.assertEqual(report['failuresByReason'], {}, run)
            self.assertEqual(report['plan']['planned'], 0, 'registered before the plan: no second identity')
            self.assert_follows_the_chain(self.last, report)

    def test_a_fill_holding_more_than_its_count_resumes(self):
        # An identity failed, a second one was registered in its place, then the first one's transaction was included
        # after all: two registered for --count 1. The same command plans nothing more instead of refusing.
        h = self.harness('fill', '--count', '1')
        h.run_fill()
        first = h.state['identities'][0]
        extra = h.new_identity('idle')
        extra['outcome'], extra['registeredAt'] = 'registered', first['registeredAt']
        h.save()
        report = self.harness('fill', '--count', '1').run_fill()
        self.assertEqual((report['plan']['planned'], report['plan']['countExceededBy']), (0, 1))

    def test_a_raised_fee_cap_raises_the_default_funding(self):
        self.node.base = 7 * rf.GWEI
        report = self.harness('fill', '--count', '1', '--fee-cap-wei', str(20 * rf.GWEI)).run_fill()
        self.assertEqual(report['plan']['fundWei'], 2 * rf.REPLACE_GAS * 20 * rf.GWEI)
        self.assertEqual(report['registered'], 1, report['failuresByReason'])

    def test_a_full_pool_is_waited_for_not_an_attempt(self):
        h = self.harness('fill', '--count', '1')
        real = self.node.call
        refused = []

        def call(method, params=()):
            if method == 'eth_sendRawTransaction' and json.loads(bytes.fromhex(params[0][2:]))['to'] == REG and len(refused) < 3:
                refused.append(1)
                raise rf.RpcError(-32016, ['-32016 pool full (gas)', '-32016 pool full (sender gas)', '-32016 pool full'][len(refused) - 1])
            return real(method, params)
        self.node.call = call
        report = h.run_fill()
        self.assertEqual(len(refused), 3)
        self.assertEqual(report['registered'], 1)
        (ident,) = h.state['identities']
        self.assertEqual([a['status'] for a in ident['attempts']], [1], 'the refusals are no attempts')
        self.assertEqual(report['failuresByReason'], {})

    def test_an_operator_funded_to_the_plan_completes_it(self):
        self.cp['M_max'] = 64
        dry = self.harness('fill', '--count', '8', '--fund-batch', '4', '--dry-run').run_fill()['plan']
        self.node.accounts[self.op]['balance'] = dry['operatorTotalWei']
        report = self.harness('fill', '--count', '8', '--fund-batch', '4').run_fill()
        self.assertEqual(report['registered'], 8)

    def test_a_stop_while_the_keys_are_made_stops_there(self):
        self.cp['M_max'] = 64
        h = self.harness('fill', '--count', '20')
        real = self.signer.generate
        made = []

        def generate(path):
            made.append(path)
            if len(made) == 3:
                h.stopping = True   # SIGINT
            return real(path)
        self.signer.generate = generate
        h.run_fill()
        self.assertEqual(len(h.state['identities']), 3)
        self.assertEqual(self.node.pool, [], 'nothing sent')
        self.signer.generate = real
        report = self.harness('fill', '--count', '20').run_fill()
        self.assertEqual((report['identities'], report['registered']), (20, 20))

    # ---- review round 4: the M3 leaver from the chain, and only included transactions decide a check

    def no_m3_result(self, h):
        self.assertNotIn('m3', json.loads((h.dir / 'state.json').read_text()))
        report = h.dir / 'report.json'
        self.assertFalse(report.exists() and 'm3' in json.loads(report.read_text()))

    def sent_kinds(self):
        return [SEL.get(t['data'][2:10]) for b in self.node.blocks.values() for t in b['txs'] if t['to'] == REG] + \
            [SEL.get(t['data'][2:10]) for t in self.node.pool + self.node.parked if t['to'] == REG]

    def test_m3_picks_a_leaver_the_chain_holds_active(self):
        # Another registrant's registerReplacing (the wallet's, at a full set) exits the first idle harness member, which
        # state.json still records as registered: check 3 deregisters a member whose record is ACTIVE instead.
        self.harness('fill', '--count', '11').run_fill()
        first = next(i for i in self.last.state['identities'] if i['role'] == 'idle' and i['outcome'] == 'registered')
        self.node.exit(self.node.reg, first['address'], self.node.height)
        other = '0x' + 'cd' * 20
        self.node.reg['records'][other] = self.node.record(4, self.node.height)
        self.node.reg['active'].append(other)
        self.node.mine()
        h = self.harness('m3')
        result = h.run_m3()
        c = result['checks']['removeInactiveBelowFullSet']
        self.assertTrue(result['pass'], json.dumps(result['checks'], indent=1))
        self.assertEqual((c['status'], c['removalCodeBefore']), (0, 2))
        self.assertLess(c['activeAfterDeregister'], self.cp['M_max'])
        self.assertNotIn('deregister', [a['kind'] for a in h.state['identities'][first['i']]['attempts']])

    def test_m3_a_deregister_that_reverts_sends_no_remove_inactive(self):
        # The leaver is exited by another registrant in the block that carries its deregister (which reverts): the set is
        # still full, so no removeInactive is sent (it would remove a member) and no m3 result is written.
        self.harness('fill', '--count', '11').run_fill()
        h = self.harness('m3')
        real = self.node.mine

        def mine(*a):
            for t in self.node.pool:
                if SEL.get(t['data'][2:10]) == 'deregister()' and self.node.reg['records'][t['from']]['status'] == 1:
                    self.node.exit(self.node.reg, t['from'], self.node.height)
                    self.node.reg['records']['0x' + 'cd' * 20] = self.node.record(4, self.node.height)
                    self.node.reg['active'].append('0x' + 'cd' * 20)
            real()
        h.sleep = mine
        with self.assertRaisesRegex(RuntimeError, 'deregister .* reverted'):
            h.run_m3()
        self.assertIn('deregister()', self.sent_kinds())
        self.assertNotIn('removeInactive(address)', self.sent_kinds())
        self.no_m3_result(h)

    def test_m3_a_replacement_not_included_in_time_writes_no_result(self):
        # Both registerReplacing are held (-32016) past --include-timeout: the path could not be completed (exit 1), not
        # a check that does not hold; they stay in flight and the next run settles them.
        self.harness('fill', '--count', '11').run_fill()
        self.node.park = lambda t: t['to'] == REG and SEL.get(t['data'][2:10]) == 'registerReplacing(address,address)'
        self.node.park_blocks = 8
        h = self.harness('m3', '--include-timeout', '2')
        with self.assertRaisesRegex(RuntimeError, 'registerReplacing was not included'):
            h.run_m3()
        self.no_m3_result(h)
        newcomers = [i for i in h.state['identities'] if i['role'] == 'm3' and i['attempts']]
        self.assertEqual(len(newcomers), 2)
        self.assertTrue(all(i['outcome'] is None and 'status' not in i['attempts'][-1] for i in newcomers), 'no outcome invented')
        self.assertEqual(self.sent_kinds().count('register(address)'), 11, 'only the fill\'s: no later check was run')
        self.node.park = None
        h2 = self.harness('m3')
        result = h2.run_m3()
        by = {i['address']: i for i in h2.state['identities']}
        self.assertTrue(all(by[n['address']]['outcome'] == 'registered' for n in newcomers))
        self.assertTrue(result['pass'], json.dumps(result['checks'], indent=1))

    def test_m3_a_refused_call_writes_no_result(self):
        self.harness('fill', '--count', '11').run_fill()
        h = self.harness('m3')
        real = self.node.call

        def call(method, params=()):
            if method == 'eth_sendRawTransaction' and \
                    SEL.get(json.loads(bytes.fromhex(params[0][2:]))['data'][2:10]) == 'register(address)':
                raise rf.RpcError(-32000, '-32000 insufficient funds')
            return real(method, params)
        self.node.call = call
        with self.assertRaisesRegex(RuntimeError, 'register at the full set was refused'):
            h.run_m3()
        self.no_m3_result(h)
        self.assertNotIn('deregister()', self.sent_kinds())

    def test_stop_miners_survives_a_miner_that_exits_meanwhile(self):
        proc = subprocess.Popen([sys.executable, '-c', 'pass'])
        proc.wait()   # exited and reaped: its pid is gone
        d = self.root / 'h'
        d.mkdir(mode=0o700)
        (d / 'state.json').write_text(json.dumps({'miners': [{'i': 0, 'pid': proc.pid, 'home': str(d / 'miners' / '000000')}]}))
        real = rf.miner_running
        rf.miner_running = lambda m, base: True   # it still ran when checked
        try:
            self.assertEqual(rf.stop_miners(d, wait=0), [proc.pid])
        finally:
            rf.miner_running = real


if __name__ == '__main__':
    unittest.main()
