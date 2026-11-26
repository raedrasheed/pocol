"""An in-process fake PoCol v3 node for the holders index tests (test_pocol_holders.py): a chain of blocks with
branches, the state of every block (balances, nonces, code, the registry records, heldTotal, burnedTotal), served only
for heights in the 32-block state window as pocold does (-32017 below it), pocol_getMiners and pocol_getCaps at the
head, receipts of contract creations, and the executed reward flows of each block (the `rewards` callable). The fake
moves value exactly as the chain does: issuance credited to RewardEscrow, the sender pays gas x (base fee + tip), the
tip goes to Escrow, the base fee leaves the state; rewards are paid out of Escrow (or accrue into a bond) and protocol
burns stay in Escrow's balance. Loopback-free: no socket, no process."""
import copy
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pocol_rewards as R  # noqa: E402
import pocol_holders as H  # noqa: E402

PCOL = 10 ** 18
TIME0 = 1_795_292_456
CHAIN_ID = 777964
B_REG = 20 * PCOL
FOUNDERS_BALANCE = 999_980 * PCOL
# The Mainnet founders' pattern (site-identity.json genesisSpec): founder 0's ID is its own reward address, founders 1-3
# have a distinct reward address; each reward account holds 999,980 PCOL and each record a HELD bond of 20 PCOL.
FOUNDERS = [('0x' + '15' * 20, '0x' + '15' * 20), ('0x' + '44' * 20, '0x' + 'bf' * 20),
            ('0x' + 'c3' * 20, '0x' + '56' * 20), ('0x' + 'e4' * 20, '0x' + '4c' * 20)]
DEFAULT_CP = {4: 1, 5: 1, 24: 1, 8: 32, 9: 2, 10: 256, 11: 4, 26: 16, 13: 1000, 14: 500, 15: 4 * PCOL, 16: B_REG,
              18: 10000, 19: 1, 25: 360, 52: 12_623_040, 53: PCOL // 8, 54: 5000, 58: 2500}


def uint(n):
    return b'' if n == 0 else n.to_bytes((n.bit_length() + 7) // 8, 'big')


def make_spec(chain_id=CHAIN_ID, members=FOUNDERS, cp=None):
    """A canonical v3 GenesisSpec [2, chainId, CP (59 items), allocRoot, M_0List, sysCodeHash]."""
    values = {**DEFAULT_CP, **(cp or {})}
    return R.rlp_encode([bytes([2]), uint(chain_id), [uint(values.get(i, 1)) for i in range(59)], b'\x33' * 32,
                         [[bytes.fromhex(i[2:]), bytes.fromhex(r[2:])] for i, r in members],
                         bytes.fromhex(R.V3_SYS_CODE_HASH[2:])])


def network_of(spec, chain_id=CHAIN_ID):
    return {'chainId': chain_id, 'genesisHash': '0x' + R.keccak256(spec).hex(), 'specVersion': 2, 'v3': True}


def fake_issuance(h, s0, h_half, tail):
    """The per-block issuance, written independently of pocol_holders.issuance (V3-19 M3)."""
    if h == 0:
        return 0
    era = h // h_half
    return max(s0 // (2 ** era) if era < 256 else 0, tail)


def bhash(branch, n):
    return '0x' + f'{branch:02x}' + f'{n:062x}'


def word(value):
    return '0x' + value.to_bytes(32, 'big').hex()


class State:
    def __init__(self):
        self.accounts = {}     # address -> [balance, nonce, code hex]
        self.records = {}      # id -> {status, tomb, bondState, lastShare, bond, registeredAt, reward, graceStart}
        self.burned = 0

    def account(self, address):
        return self.accounts.setdefault(address, [0, 0, '0x'])


class FakeHolderChain:
    """`mine(...)` appends a block; `reorg(fork)` drops the blocks above `fork` (a new branch is then mined). Reads:
    `query(method, params)` (Gateway.query's envelopes), `batch(calls)`, `rewards(height)`. `static` holds balances
    that never change (the large synthetic sets), `caps_extra` is added to pocol_getCaps' account count, `on_read`
    runs before every read, `fail` makes every read raise, `calls` counts reads by method."""

    def __init__(self, members=FOUNDERS, cp=None, chain_id=CHAIN_ID, static=None):
        self.spec = make_spec(chain_id, members, cp)
        self.network = network_of(self.spec, chain_id)
        self.cp = {**DEFAULT_CP, **(cp or {})}
        self.chain_id = chain_id
        self.branch = 1
        self.blocks, self.states, self.flows = [], [], []
        self.extra = {}
        self.receipts = {}
        self.static = static or {}
        self.caps_extra = 0
        self.on_read = None
        self.fail = False
        self.rewards_fail = False
        self.calls = {}
        self.slots = {}
        genesis = State()
        for program in sorted(H.PROGRAMS):
            genesis.accounts[program] = [0, 0, '0x60']
        for i, (miner, reward) in enumerate(members):
            genesis.account(reward)[:2] = [FOUNDERS_BALANCE, 1]
            genesis.records[miner] = {'status': 1, 'tomb': False, 'bondState': 1, 'lastShare': 0, 'bond': B_REG,
                                      'registeredAt': 0, 'reward': reward, 'graceStart': 0}
            self.slots[H.record_slot(miner) + 3] = miner
        genesis.account(H.ESCROW)[0] = B_REG * len(members)
        self.states.append(genesis)
        self.flows.append([])
        self._block(0, [], None, 0, 0, TIME0)

    # ---------------- building ----------------
    @property
    def head(self):
        return len(self.blocks) - 1

    def _block(self, n, txs, miner, gas, base, ts):
        h = bhash(self.branch, n)
        parent = self.blocks[-1]['hash'] if self.blocks else '0x' + '00' * 32
        full = []
        for i, tx in enumerate(txs):
            item = {'hash': tx['hash'], 'blockHash': h, 'blockNumber': hex(n), 'transactionIndex': hex(i)}
            if tx.get('undecoded'):
                item['raw'] = '0x02'
            else:
                item.update({'from': tx['from'], 'to': tx.get('to'), 'value': hex(tx.get('value', 0)),
                             'input': tx.get('input', '0x'), 'nonce': '0x0', 'gas': hex(tx.get('gas', 21000))})
            full.append(item)
            if tx.get('to') is None and not tx.get('undecoded'):
                self.receipts[tx['hash']] = {'transactionHash': tx['hash'], 'blockHash': h, 'blockNumber': hex(n),
                                             'contractAddress': tx.get('creates'), 'status': '0x1'}
        block = {'number': hex(n), 'hash': h, 'parentHash': parent, 'timestamp': hex(ts),
                 'miner': miner or '0x' + '00' * 20, 'gasUsed': hex(gas), 'baseFeePerGas': hex(base),
                 'transactions': [t['hash'] for t in full], 'pocol': {'final': False, 'signed': True}, 'full': full}
        self.blocks.append(block)
        return block

    def mine(self, txs=(), miner=None, base_fee=10 ** 9, flows=None, internal=(), burn=0, mutate=None, ts=None):
        """Block n = head + 1: issuance to Escrow; the transactions ({from, to|None, value, gas, tip, creates,
        undecoded}); the reward flows [(minerId, rewardAddress, paid, accrued)] (default: 90 % of the issuance to the
        winner's reward address); `internal` transfers (from, to, amount) no transaction shows; `burn` added to
        burnedTotal inside Escrow; `mutate(state, n)` for registry changes."""
        n = len(self.blocks)
        state = copy.deepcopy(self.states[-1])
        escrow = state.account(H.ESCROW)
        issued = fake_issuance(n, self.cp[15], self.cp[52], self.cp[53])
        escrow[0] += issued
        members = sorted(r for r in state.records if state.records[r]['status'] == 1)
        miner = miner or members[n % len(members)]
        used, full = 0, []
        for i, tx in enumerate(txs):
            tx = dict(tx)
            tx.setdefault('hash', '0x' + f'{self.branch:02x}{n:030x}{i:032x}')
            gas, tip, value = tx.get('gas', 21000), tx.get('tip', 1), tx.get('value', 0)
            sender = state.account(tx['from'])
            fee = gas * (base_fee + tip)
            assert sender[0] >= fee + value, 'fake: sender cannot pay'
            sender[0] -= fee + value
            sender[1] += 1
            escrow[0] += gas * tip
            used += gas
            target = tx.get('creates') if tx.get('to') is None else tx['to']
            if target is not None:
                account = state.account(target)
                account[0] += value
                if tx.get('to') is None:
                    account[1], account[2] = 1, '0x6080'
            full.append(tx)
        if flows is None:
            flows = [(miner, state.records[miner]['reward'], issued * 9 // 10, 0)] if issued else []
        for flow_miner, reward, paid, accrued in flows:
            assert escrow[0] >= paid, 'fake: Escrow cannot pay'
            escrow[0] -= paid
            state.account(reward)[0] += paid
            if accrued:
                state.records[flow_miner]['bond'] += accrued
        for source, target, amount in internal:
            assert state.account(source)[0] >= amount
            state.account(source)[0] -= amount
            state.account(target)[0] += amount
        state.burned += burn
        if mutate:
            mutate(state, n)
        for miner_id in state.records:
            self.slots[H.record_slot(miner_id) + 3] = miner_id
        self.states.append(state)
        self.flows.append(list(flows))
        last = int(self.blocks[-1]['timestamp'], 16)
        return self._block(n, full, miner, used, base_fee, ts if ts is not None else last + 10)

    def mine_many(self, count, **kw):
        for _ in range(count):
            self.mine(**kw)

    def reorg(self, fork):
        """Drop every block above `fork`; the next mined blocks are a new branch."""
        for b in self.blocks[fork + 1:]:
            self.extra[b['hash']] = b
        del self.blocks[fork + 1:]
        del self.states[fork + 1:]
        del self.flows[fork + 1:]
        self.branch += 1

    # ---------------- accounting ----------------
    def held_total(self, state):
        return sum(r['bond'] for r in state.records.values() if H.held(r))

    def balances_sum(self, n=None):
        state = self.states[self.head if n is None else n]
        return sum(a[0] for a in state.accounts.values()) + sum(self.static.values())

    def existing_accounts(self, state):
        count = sum(1 for a, v in state.accounts.items() if a not in H.PROGRAMS and (v[0] or v[1] or v[2] != '0x'))
        return count + sum(1 for a, v in self.static.items() if v and a not in state.accounts)

    # ---------------- reads ----------------
    def _state(self, tag):
        n = self.head if tag == 'latest' else int(tag, 16)
        if n > self.head:
            return None, {'code': -32001, 'message': 'unknown block'}
        if n < self.head - (H.STATE_WINDOW - 1):
            return None, {'code': -32017, 'message': 'state outside the retained trie window (K_hist)'}
        return self.states[n], None

    def _account(self, state, address):
        if address in state.accounts:
            return state.accounts[address]
        return [self.static.get(address, 0), 0, '0x']

    def batch(self, calls):
        return [self.query(m, p) for m, p in calls]

    def query(self, method, params, deadline=None):
        self.calls[method] = self.calls.get(method, 0) + 1
        if self.on_read:
            self.on_read(method, params)
        if self.fail:
            raise OSError('node down')
        ok = lambda result: {'jsonrpc': '2.0', 'id': 1, 'result': result}
        if method == 'pocol_getParams':
            return ok({'chainId': self.chain_id, 'genesisHash': self.network['genesisHash'],
                       'genesisPre': '0x' + self.spec.hex()})
        if method == 'eth_getBlockByNumber':
            n = self.head if params[0] == 'latest' else int(params[0], 16)
            if not 0 <= n <= self.head:
                return ok(None)
            return ok({k: v for k, v in self.blocks[n].items() if k != 'full'})
        if method == 'eth_getBlockByHash':
            b = next((b for b in self.blocks if b['hash'] == params[0]), self.extra.get(params[0]))
            if b is None:
                return ok(None)
            return ok({**{k: v for k, v in b.items() if k != 'full'}, 'transactions': b['full']})
        if method == 'eth_getTransactionReceipt':
            return ok(self.receipts.get(params[0]))
        if method == 'pocol_getMiners':
            state = self.states[self.head]
            return ok([{'id': i, 'status': r['status'], 'tomb': r['tomb'], 'bondState': r['bondState'],
                        'lastShare': r['lastShare'], 'bond': hex(r['bond']), 'registeredAt': r['registeredAt'],
                        'reward': r['reward'], 'rewardBalance': hex(self._account(state, r['reward'])[0])}
                       for i, r in sorted(state.records.items())])
        if method == 'pocol_getCaps':
            state = self.states[self.head]
            return ok({'number': hex(self.head), 'hash': self.blocks[-1]['hash'],
                       'accounts': hex(self.existing_accounts(state) + self.caps_extra), 'accountsCap': hex(10 ** 6)})
        if method in ('eth_getBalance', 'eth_getTransactionCount', 'eth_getCode', 'eth_getStorageAt'):
            state, error = self._state(params[-1])
            if error:
                return {'jsonrpc': '2.0', 'id': 1, 'error': error}
            address = params[0].lower()
            if method == 'eth_getStorageAt':
                slot = int(params[1], 16)
                if address == H.REGISTRY and slot == 4:
                    return ok(word(self.held_total(state)))
                if address == H.ESCROW and slot == 0:
                    return ok(word(state.burned))
                miner = self.slots.get(slot) if address == H.REGISTRY else None
                if miner is not None and miner in state.records:
                    return ok(word(state.records[miner]['graceStart'] << 192))
                return ok(word(0))
            account = self._account(state, address)
            if method == 'eth_getBalance':
                return ok(hex(account[0]))
            if method == 'eth_getTransactionCount':
                return ok(hex(account[1]))
            return ok(account[2])
        return {'jsonrpc': '2.0', 'id': 1, 'error': {'code': -32601, 'message': 'Method not found'}}

    def rewards(self, height):
        """Block `height`'s executed reward flows as pocol-block-reward-v3's block.beneficiaries; the states h - 1 and
        h must be in the window, as for snapshot_v3."""
        self.calls['rewards'] = self.calls.get('rewards', 0) + 1
        if self.rewards_fail or not 1 <= height <= self.head or height - 1 < self.head - (H.STATE_WINDOW - 1):
            raise RuntimeError('state outside the retained trie window (K_hist)')
        merged = {}
        for miner, reward, paid, accrued in self.flows[height]:
            entry = merged.setdefault((miner, reward), [0, 0])
            entry[0] += paid
            entry[1] += accrued
        beneficiaries = [dict({'minerId': m, 'rewardAddress': r, 'amountWei': str(p)},
                              **({'bondAccruedWei': str(a)} if a else {})) for (m, r), (p, a) in sorted(merged.items())]
        return {'schema': 'pocol-block-reward-v3', 'blockHeight': height, 'blockHash': self.blocks[height]['hash'],
                'block': {'beneficiaries': beneficiaries}}
