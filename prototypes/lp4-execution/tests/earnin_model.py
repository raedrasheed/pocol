#!/usr/bin/env python3
"""Independent domain model of the earn-in rules (development/decisions/EARN-IN-SPEC.md).

No bytecode, no Rust and no storage encoding: plain records and integers. P2 and P3 must reproduce the
results this model writes to tests/data/earnin-vectors.json. Run with --check to compare against the
committed file, or --write to regenerate it.
"""
import json, sys
from dataclasses import dataclass, field, asdict
from pathlib import Path

PCOL = 10**18
B = 20 * PCOL
P_MAX = 8
T_PROB = 60_480
U = 40
W_ACT = 32
M_MIN = 1
M_MAX = 64
HELD, BURNED, PAID, ACCRUING = 1, 2, 3, 4

class Revert(Exception):
    pass

@dataclass
class Record:
    reward: str
    status: int = 1          # 1 active, 2 exited
    bond_state: int = HELD
    bond: int = 0
    tomb: bool = False
    registered_at: int = 0
    exit: int = 0
    due: int = 0

    def live(self):
        return self.status != 0 and not self.tomb and self.bond_state in (HELD, ACCRUING)

@dataclass
class Registry:
    records: dict = field(default_factory=dict)
    active: list = field(default_factory=list)
    held_total: int = 0
    accruing: int = 0
    durable: set = field(default_factory=set)   # addresses that pass the PAY-02 probe
    paid: dict = field(default_factory=dict)    # reward address -> PCOL paid out of Escrow
    burned: int = 0

    def pay(self, to, amount):
        self.paid[to] = self.paid.get(to, 0) + amount

    # ---- user paths (section 3)
    def register(self, sender, reward, value, h):
        if value > B:
            raise Revert('BadValue')
        if sender in self.records:
            raise Revert('BadState')
        if len(self.active) >= M_MAX:
            raise Revert('Capacity')
        if reward != sender and reward not in self.durable:
            raise Revert('RewardNotDurable')
        if value < B and self.accruing >= P_MAX:
            raise Revert('Capacity')
        state = HELD if value == B else ACCRUING
        self.records[sender] = Record(reward=reward, bond=value, bond_state=state, registered_at=h)
        self.active.append(sender)
        self.held_total += value
        if state == ACCRUING:
            self.accruing += 1

    def top_up(self, sender, value, h):
        if value == 0:
            raise Revert('BadValue')
        r = self.records.get(sender)
        if r is None or not r.live() or r.bond_state != ACCRUING or r.status != 1 or h >= r.registered_at + T_PROB:
            raise Revert('BadState')
        if value > B - r.bond:
            raise Revert('BadValue')
        r.bond += value
        self.held_total += value
        self._promote(r)

    def deregister(self, sender, h):
        r = self.records.get(sender)
        if r is None or not r.live() or r.status != 1 or len(self.active) <= M_MIN:
            raise Revert('BadState')
        self._exit(sender, h)

    def reactivate(self, sender, h):
        r = self.records.get(sender)
        if r is None or not r.live() or r.status != 2 or h < r.exit + W_ACT:
            raise Revert('BadState')
        if r.bond_state == ACCRUING and h >= r.registered_at + T_PROB:
            raise Revert('BadState')
        r.status = 1
        self.active.append(sender)

    # ---- system paths (section 4)
    def sweep(self, h):
        """One beginBlock over every record (the bounded cursor is not modelled)."""
        for ident in list(self.records):
            r = self.records[ident]
            if r.live() and r.bond_state == ACCRUING and r.status == 1 and h >= r.registered_at + T_PROB:
                if len(self.active) > M_MIN:
                    self._exit(ident, h)
            elif r.live() and r.status == 2 and h >= r.due:
                self._refund_erase(ident)

    def punish(self, ident):
        r = self.records[ident]
        if not r.live():
            raise Revert('BadState')
        if r.status == 1:
            self.active.remove(ident)
            r.status = 2
        if r.bond_state == ACCRUING:
            self.accruing -= 1
        self.held_total -= r.bond
        self.burned += r.bond
        r.bond_state, r.tomb = BURNED, True

    def accrue(self, ident, amount):
        r = self.records.get(ident)
        if r is None or not r.live() or r.bond_state != ACCRUING:
            return amount
        add = min(amount, B - r.bond)
        r.bond += add
        self.held_total += add
        self._promote(r)
        return amount - add

    # ---- settlement (section 5)
    def settle_to(self, ident, amount):
        """payTo/distribute of `amount` owed to `ident`. Returns the burned part."""
        if amount == 0:
            return 0
        r = self.records.get(ident)
        if r is None or r.tomb:
            self.burned += amount
            return amount
        excess = self.accrue(ident, amount) if r.bond_state == ACCRUING else amount
        if excess:
            self.pay(r.reward, excess)
        return 0

    # ---- helpers
    def _promote(self, r):
        if r.bond == B:
            r.bond_state = HELD
            self.accruing -= 1

    def _exit(self, ident, h):
        r = self.records[ident]
        self.active.remove(ident)
        r.status, r.exit, r.due = 2, h, h + U

    def _refund_erase(self, ident):
        r = self.records.pop(ident)
        if r.bond:
            self.pay(r.reward, r.bond)
        self.held_total -= r.bond
        if r.bond_state == ACCRUING:
            self.accruing -= 1

def draw_weights(members, accruing, shares):
    """Section 6: candidates and weights. members: ids of A_h; accruing: set; shares: id -> count."""
    candidates = [m for m in members if m not in accruing] or list(members)
    weights = {m: (shares.get(m, 0) if m in candidates else 0) for m in members}
    if sum(weights[m] for m in candidates) == 0:
        weights = {m: (1 if m in candidates else 0) for m in members}
    return weights

def cutover(old_accounts, old_records, miners, system):
    """Section 7. old_accounts: addr -> {nonce,balance,code,storage}; old_records: id -> {reward,bond,
    bondState,tomb}; miners: [(id, reward)]. Returns (alloc, report) or raises Revert."""
    alloc = {a: dict(v) for a, v in old_accounts.items() if a not in system}
    credited = 0
    for ident, r in sorted(old_records.items()):
        if r['bondState'] == HELD and not r['tomb']:
            acct = alloc.setdefault(r['reward'], {'nonce': 0, 'balance': 0, 'code': '0x', 'storage': {}})
            acct['balance'] += r['bond']
            credited += r['bond']
    for ident, reward in miners:
        acct = alloc.get(reward)
        if acct is None or acct['balance'] < B:
            raise Revert('MinerBalanceBelowBond')
        if acct['nonce'] == 0 and acct['code'] == '0x':
            raise Revert('RewardNotDurable')
        acct['balance'] -= B
    copied = sum(v['balance'] for a, v in old_accounts.items() if a not in system)
    report = {'accounts': len(alloc), 'copiedBalances': str(copied), 'bondsCredited': str(credited),
              'minerBondsDeducted': str(B * len(miners)),
              'totalSupply': str(sum(v['balance'] for v in alloc.values()) + B * len(miners))}
    return alloc, report

# ---------------- vectors
def snapshot(reg):
    return {'records': {k: asdict(v) for k, v in sorted(reg.records.items())}, 'active': list(reg.active),
            'heldTotal': str(reg.held_total), 'accruing': reg.accruing, 'burned': str(reg.burned),
            'paid': {k: str(v) for k, v in sorted(reg.paid.items())}}

def run(steps, durable=()):
    reg = Registry(durable=set(durable))
    reg.register('m0', 'm0', B, 0)      # a HELD genesis-like member keeps ActLen > M_min
    out = []
    for step in steps:
        op, args = step[0], step[1:]
        try:
            result = getattr(reg, op)(*args)
            out.append({'op': [op, *map(str, args)], 'ok': True, 'result': None if result is None else str(result)})
        except Revert as e:
            out.append({'op': [op, *map(str, args)], 'ok': False, 'revert': str(e)})
        # Escrow invariant on the modelled quantities: held equals the sum of live bonds.
        assert reg.held_total == sum(r.bond for r in reg.records.values() if r.live()), step
        assert reg.accruing == sum(1 for r in reg.records.values() if r.live() and r.bond_state == ACCRUING), step
        assert 0 <= reg.accruing <= P_MAX
    return {'steps': out, 'final': snapshot(reg)}

def vectors():
    v = {}
    v['register-values'] = run([('register', 'a', 'a', 0, 1), ('register', 'b', 'b', 5 * PCOL, 1),
                                ('register', 'c', 'c', B, 1), ('register', 'd', 'd', B + 1, 1)])
    v['probation-seats'] = run([('register', f'p{i}', f'p{i}', 0, 1) for i in range(P_MAX + 1)] +
                               [('register', 'full', 'full', B, 2)])
    v['durable-reward'] = run([('register', 'a', 'x', 0, 1), ('register', 'b', 'y', 0, 1), ('register', 'c', 'c', 0, 1)],
                              durable=['y'])
    v['top-up'] = run([('register', 'a', 'a', 15 * PCOL, 1), ('top_up', 'a', 6 * PCOL, 2), ('top_up', 'a', 5 * PCOL, 2),
                       ('top_up', 'a', 1, 3), ('register', 'b', 'b', 0, 4), ('top_up', 'b', 0, 5)])
    v['accrual'] = run([('register', 'a', 'ra', 0, 1), ('settle_to', 'a', 7 * PCOL), ('settle_to', 'a', 13 * PCOL),
                        ('settle_to', 'a', 3 * PCOL), ('register', 'b', 'rb', 19 * PCOL, 2), ('settle_to', 'b', 4 * PCOL)],
                       durable=['ra', 'rb'])
    v['deadline'] = run([('register', 'a', 'ra', 2 * PCOL, 10), ('sweep', 10 + T_PROB - 1), ('sweep', 10 + T_PROB),
                         ('reactivate', 'a', 10 + T_PROB + W_ACT), ('sweep', 10 + T_PROB + U),
                         ('register', 'z', 'rz', 0, 20), ('sweep', 20 + T_PROB), ('sweep', 20 + T_PROB + U)],
                        durable=['ra', 'rz'])
    v['exit-and-reactivate-in-probation'] = run([('register', 'a', 'a', 0, 1), ('deregister', 'a', 100),
                                                ('reactivate', 'a', 100 + W_ACT - 1), ('reactivate', 'a', 100 + W_ACT)])
    v['punish'] = run([('register', 'a', 'a', 3 * PCOL, 1), ('settle_to', 'a', 2 * PCOL), ('punish', 'a'),
                       ('settle_to', 'a', PCOL)])
    v['draw'] = {
        'mixed': draw_weights(['a', 'b', 'c'], {'b'}, {'a': 3, 'b': 9, 'c': 1}),
        'mixed-zero-shares': draw_weights(['a', 'b', 'c'], {'b'}, {'b': 9}),
        'all-accruing': draw_weights(['a', 'b'], {'a', 'b'}, {'a': 2}),
        'all-accruing-zero': draw_weights(['a', 'b'], {'a', 'b'}, {}),
    }
    system = {f'0x{0xc0c001 + i:040x}' for i in range(5)}
    old = {'0xs1': {'nonce': 5, 'balance': 30 * PCOL, 'code': '0x', 'storage': {}},
           '0xs2': {'nonce': 2, 'balance': PCOL, 'code': '0x', 'storage': {}},
           '0xu1': {'nonce': 0, 'balance': 7, 'code': '0x6000', 'storage': {'0x1': '0x2'}},
           f'0x{0xc0c002:040x}': {'nonce': 0, 'balance': 900 * PCOL, 'code': '0x00', 'storage': {}}}
    records = {'0xs1': {'reward': '0xs1', 'bond': 100 * PCOL, 'bondState': HELD, 'tomb': False},
               '0xs2': {'reward': '0xs2', 'bond': 100 * PCOL, 'bondState': HELD, 'tomb': False},
               '0xt': {'reward': '0xu1', 'bond': 100 * PCOL, 'bondState': BURNED, 'tomb': True}}
    alloc, report = cutover(old, records, [('0xs1', '0xs1'), ('0xs2', '0xs2')], system)
    v['cutover'] = {'alloc': {k: {**a, 'balance': str(a['balance'])} for k, a in sorted(alloc.items())}, 'report': report}
    try:
        cutover({'0xs': {'nonce': 1, 'balance': PCOL, 'code': '0x', 'storage': {}}}, {}, [('0xs', '0xs')], system)
        v['cutover-low-balance'] = 'accepted'
    except Revert as e:
        v['cutover-low-balance'] = str(e)
    try:
        cutover({'0xs': {'nonce': 0, 'balance': B, 'code': '0x', 'storage': {}}}, {}, [('0xs', '0xs')], system)
        v['cutover-not-durable'] = 'accepted'
    except Revert as e:
        v['cutover-not-durable'] = str(e)
    return v

if __name__ == '__main__':
    path = Path(__file__).with_name('data') / 'earnin-vectors.json'
    data = json.dumps(vectors(), indent=1, sort_keys=True) + '\n'
    if sys.argv[1:] == ['--write']:
        path.write_text(data)
        print('wrote', path)
    else:
        assert path.read_text() == data, 'earnin-vectors.json differs from the model; run --write and review'
        print('PASS earn-in model matches', path.name)
