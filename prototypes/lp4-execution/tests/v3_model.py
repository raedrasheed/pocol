#!/usr/bin/env python3
"""Independent model of the v3 registry budgets (V3-08, V3-09; V3-DESIGN.md, V3-DECISIONS.md D-V3-08/09),
extending the earn-in model (earnin_model.py).

No bytecode, no Rust and no storage encoding: plain records, lists and integers. Unlike earnin_model.py it
models every block start the way the chain runs it (each new height first runs beginBlock: the per-block
budgets restart and the bounded refund sweep advances its cursor over the record array) and the per-block
budgets of every user path:

- V3-08: setRewardAddress and topUp draw on a maintenance budget of REG_MAINT_MAX = 8 per block, and an
  identity performs at most one maintenance operation per block. Register, deregister, claimBond and
  removeInactive share REG_OPS less the maintenance used in the block; register also keeps R_max.
- V3-09: reactivate draws on its own per-block budget R_react (not R_max) and an identity reactivates at most
  once per C_react blocks, counted from its previous reactivation. A reactivation is valid after the W_act
  cooldown and strictly before dueHeight = exitHeight + U, the height at which the refund sweep erases it.

tests/v3_contracts.rs replays tests/v3/registry-budget-vectors.json and tests/v3/earnin-on-v3-vectors.json (the
earn-in model's scenarios re-run here), written by this script, on the v3 programs and checks every step's
result, every record (including the new clocks), the record array order, the sweep cursor, the counters,
ACC_COUNT and the Escrow accounting. Run with --check (default) to compare against the committed files, or
--write to regenerate them.
"""
import json, random, sys
from dataclasses import dataclass, asdict
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import earnin_model as em

PCOL = em.PCOL
B = em.B
HELD, BURNED, PAID, ACCRUING = em.HELD, em.BURNED, em.PAID, em.ACCRUING
REG_MAINT_MAX = 8          # V3-08: a program constant, not a CP field
MAINT_MOD = 1 << 40        # V3-08: lastMaintHeight is kept modulo 2^40 (40 spare record bits)
Revert = em.Revert

# The CP the contract harness installs (tests/v3_contracts.rs, as earnin_contracts.rs) by schema name.
CP_INDEX = {'D': 11, 'R_max': 17, 'M_max': 18, 'M_min': 19, 'SWEEP_MAX': 21, 'SWEEP_SCAN': 22, 'REG_OPS': 23,
            'W_act': 24, 'Theta_inact': 25, 'E_win': 26, 'U': 27, 'P_MAX': 54, 'T_PROB': 55, 'R_react': 61,
            'C_react': 62}
DEFAULT_CP = {'D': 4, 'R_max': 16, 'M_max': 64, 'M_min': 1, 'SWEEP_MAX': 16, 'SWEEP_SCAN': 64, 'REG_OPS': 64,
              'W_act': 32, 'Theta_inact': 64, 'E_win': 16, 'U': 40, 'P_MAX': 8, 'T_PROB': 60_480, 'R_react': 16,
              'C_react': 40}

@dataclass
class Record(em.Record):
    grace: int = 0           # graceStart: registration or last reactivation
    burn: int = 0            # burnHeight (punish)
    last_share: int = 0      # set only by recordShares, which these scenarios do not run
    last_maint: int = 0      # V3-08 lastMaintHeight modulo 2^40, 0 = never
    last_react: int = 0      # V3-09 lastReactHeight, 0 = never

class Registry(em.Registry):
    """The earn-in Registry with v3 budgets, a block clock and the bounded sweep. Inherits the earn-in
    settlement (accrue, settle_to, _promote) unchanged."""

    def __init__(self, cp, durable=()):
        super().__init__(durable=set(durable))
        self.cp = dict(DEFAULT_CP, **cp)
        self.height = 0
        self.order = []                      # RecArr: insertion order, swap-removal on erase
        self.cursor = 0                      # sweepCur
        self.ops = self.regs = self.maint = self.react = 0
        # m0: the genesis-like HELD member every scenario starts with (as the contract harness installs it).
        self.records['m0'] = Record(reward='m0', bond=B, bond_state=HELD, registered_at=0)
        self.order.append('m0')
        self.active.append('m0')
        self.held_total = B

    # ---- block start (markClose is a no-op on a continuous network; beginBlock follows)
    def block(self, h):
        if h <= self.height:
            return
        self.height = h
        self.ops = self.regs = self.maint = self.react = 0
        n = len(self.order)
        if n == 0:
            self.cursor = 0
            return
        cur, removed = self.cursor, 0
        scans, deletes = min(self.cp['SWEEP_SCAN'], 64), min(self.cp['SWEEP_MAX'], 16)
        i = 0
        while i < scans and removed < deletes and n != 0:
            if cur >= n:
                cur = 0
            if self._sweep_one(self.order[cur], h):
                removed += 1
                n -= 1
            else:
                cur += 1
            i += 1
        self.cursor = 0 if n == 0 else cur % n

    def _sweep_one(self, ident, h):
        r = self.records[ident]
        if r.live() and r.status == 2 and h >= r.due:
            self._refund(ident)
            self._erase(ident)
            return True
        if r.live() and r.status == 1 and r.bond_state == ACCRUING and h >= r.registered_at + self.cp['T_PROB']:
            if len(self.active) > self.cp['M_min']:
                self._exit(ident, h)
            return False
        if r.tomb and r.bond_state != HELD and h >= r.burn + self.cp['E_win'] + self.cp['D'] + 1:
            self._erase(ident)
            return True
        return False

    # ---- budgets (checked before any record state, as the contract's RegOpsLimit prefix)
    def _budget(self, kind):
        if kind == 'maint':
            if self.maint >= REG_MAINT_MAX:
                raise Revert('RegOpsLimit')
        elif kind == 'react':
            if self.react >= self.cp['R_react']:
                raise Revert('RegOpsLimit')
        else:
            if self.ops + self.maint >= self.cp['REG_OPS'] or (kind == 'register' and self.regs >= self.cp['R_max']):
                raise Revert('RegOpsLimit')

    def _spend(self, kind):
        if kind == 'maint':
            self.maint += 1
        elif kind == 'react':
            self.react += 1
        else:
            self.ops += 1
            if kind == 'register':
                self.regs += 1

    def _tx(self, sender, kind, body):
        """One user transaction: budget, body, spend. The sender's nonce rises whatever the outcome, so it
        is durable (PAY-02 judges the pre-transaction state) from its next transaction on."""
        try:
            self._budget(kind)
            result = body()
            self._spend(kind)
            return result
        finally:
            self.durable.add(sender)

    def _live(self, ident):
        r = self.records.get(ident)
        if r is None or not r.live():
            raise Revert('BadState')
        return r

    def _maintain(self, r, h):
        # One maintenance operation per identity per block; heights compare modulo 2^40, so the clock can only
        # refuse (k*2^40 blocks apart), never admit a second operation in one block.
        if r.last_maint == h % MAINT_MOD:
            raise Revert('RegOpsLimit')

    # ---- user paths
    def register(self, sender, reward, value, h):
        self.block(h)
        if value > B:
            self.durable.add(sender)
            raise Revert('BadValue')
        def body():
            if sender in self.records:
                raise Revert('BadState')
            if len(self.active) >= self.cp['M_max']:
                raise Revert('Capacity')
            if reward != sender and reward not in self.durable:
                raise Revert('RewardNotDurable')
            probation = value < B
            if probation and self.accruing >= self.cp['P_MAX']:
                raise Revert('Capacity')
            self.records[sender] = Record(reward=reward, bond=value, bond_state=ACCRUING if probation else HELD,
                                          registered_at=h, grace=h)
            self.order.append(sender)
            self.active.append(sender)
            self.held_total += value
            self.spent[sender] = self.spent.get(sender, 0) + value
            if probation:
                self.accruing += 1
        return self._tx(sender, 'register', body)

    def top_up(self, sender, value, h):
        self.block(h)
        if value == 0:
            self.durable.add(sender)
            raise Revert('BadValue')
        def body():
            r = self._live(sender)
            self._maintain(r, h)
            if r.bond_state != ACCRUING or r.status != 1 or h >= r.registered_at + self.cp['T_PROB']:
                raise Revert('BadState')
            add = min(value, B - r.bond)
            r.bond += add
            self.held_total += add
            self.spent[sender] = self.spent.get(sender, 0) + add
            self._promote(r)
            r.last_maint = h % MAINT_MOD
            return value - add
        return self._tx(sender, 'maint', body)

    def set_reward(self, sender, reward, h):
        self.block(h)
        def body():
            r = self._live(sender)
            self._maintain(r, h)
            if reward not in self.durable:
                raise Revert('RewardNotDurable')
            r.reward = reward
            r.last_maint = h % MAINT_MOD
        return self._tx(sender, 'maint', body)

    def deregister(self, sender, h):
        self.block(h)
        def body():
            r = self._live(sender)
            if r.status != 1 or len(self.active) <= self.cp['M_min']:
                raise Revert('BadState')
            self._exit(sender, h)
        return self._tx(sender, 'other', body)

    def remove_inactive(self, sender, target, h):
        self.block(h)
        def body():
            r = self._live(target)
            if r.status != 1 or len(self.active) <= self.cp['M_min']:
                raise Revert('BadState')
            if h < max(r.last_share, r.grace) + self.cp['Theta_inact']:
                raise Revert('BadState')
            self._exit(target, h)
        return self._tx(sender, 'other', body)

    def claim_bond(self, sender, h):
        self.block(h)
        def body():
            r = self._live(sender)
            if r.status != 2 or h < r.due:
                raise Revert('BadState')
            self._refund(sender)
            self._erase(sender)
        return self._tx(sender, 'other', body)

    def reactivate(self, sender, h):
        self.block(h)
        def body():
            r = self._live(sender)
            # V3-09: after the W_act cooldown and strictly before the refund sweep's height.
            if r.status != 2 or h < r.exit + self.cp['W_act'] or h >= r.due:
                raise Revert('BadState')
            if r.bond_state == ACCRUING and h >= r.registered_at + self.cp['T_PROB']:
                raise Revert('BadState')
            if r.last_react != 0 and h < r.last_react + self.cp['C_react']:
                raise Revert('BadState')
            if len(self.active) >= self.cp['M_max']:
                raise Revert('Capacity')
            r.status, r.exit, r.due, r.grace, r.last_react = 1, 0, 0, h, h
            self.active.append(sender)
        return self._tx(sender, 'react', body)

    # ---- system paths
    def sweep(self, h):
        self.block(h)

    def punish(self, ident, h):
        self.block(h)
        r = self.records[ident]
        if not r.live():
            raise Revert('BadState')
        if r.bond_state == ACCRUING:
            self.accruing -= 1
        if r.status == 1:
            self.active.remove(ident)
            r.status, r.exit, r.due = 2, h, h
        self.held_total -= r.bond
        self.burned += r.bond
        r.bond_state, r.tomb, r.burn = BURNED, True, h

    # ---- helpers
    def _exit(self, ident, h):
        r = self.records[ident]
        self.active.remove(ident)
        r.status, r.exit, r.due = 2, h, h + self.cp['U']

    def _refund(self, ident):
        r = self.records[ident]
        if r.bond:
            self.pay(r.reward, r.bond)
        self.held_total -= r.bond
        if r.bond_state == ACCRUING:
            self.accruing -= 1

    def _erase(self, ident):
        pos = self.order.index(ident)
        last = self.order.pop()
        if last != ident:
            self.order[pos] = last
        del self.records[ident]

# ---------------- vectors
def snapshot(reg):
    return {'records': {k: {**asdict(v), 'bond': str(v.bond)} for k, v in sorted(reg.records.items())},
            'order': list(reg.order), 'cursor': reg.cursor, 'active': sorted(reg.active),
            'heldTotal': str(reg.held_total), 'accruing': reg.accruing, 'burned': str(reg.burned),
            'counters': {'ops': reg.ops, 'regs': reg.regs, 'maint': reg.maint, 'react': reg.react},
            'paid': {k: str(v) for k, v in sorted(reg.paid.items())},
            'spent': {k: str(v) for k, v in sorted(reg.spent.items())}}

def check_invariants(reg, step):
    live = [r for r in reg.records.values() if r.live()]
    assert reg.held_total == sum(r.bond for r in live), step
    # ACC_COUNT conservation: the counter is exactly the number of live ACCRUING records.
    assert reg.accruing == sum(1 for r in live if r.bond_state == ACCRUING), step
    assert 0 <= reg.accruing <= max(reg.cp['P_MAX'], 0), step
    assert sorted(reg.active) == sorted(k for k, r in reg.records.items() if r.live() and r.status == 1), step
    assert sorted(reg.order) == sorted(reg.records), step
    # The three budgets are independent (D-V3-08/09): each counter stays within its own cap. ops + maint can
    # exceed REG_OPS (REG_OPS users first, maintenance after), so a block admits at most
    # REG_OPS + REG_MAINT_MAX + R_react registry user operations; each REG_OPS user saw ops + maint < REG_OPS
    # at its own check (_budget), which run() asserts per step.
    assert reg.maint <= REG_MAINT_MAX and reg.react <= reg.cp['R_react'] and reg.regs <= reg.cp['R_max'], step
    assert reg.ops <= reg.cp['REG_OPS'], step

def run(steps, cp=None, durable=()):
    """Heights never decrease, so the contract harness replays the same steps block by block."""
    cp = cp or {}
    reg = Registry(cp, durable)
    out = []
    users = {'register', 'deregister', 'claim_bond', 'remove_inactive'}
    for step in steps:
        op, args = step[0], step[1:]
        if op in users:
            reg.block(args[-1])
            before = (reg.ops, reg.maint)
        try:
            result = getattr(reg, op)(*args)
            out.append({'op': [op, *map(str, args)], 'ok': True, 'result': None if result is None else str(result)})
            if op in users:
                assert before[0] + before[1] < reg.cp['REG_OPS'], step
        except Revert as e:
            out.append({'op': [op, *map(str, args)], 'ok': False, 'revert': str(e)})
        check_invariants(reg, step)
    return {'cp': {k: cp[k] for k in sorted(cp)}, 'durable': sorted(durable), 'steps': out, 'final': snapshot(reg)}

def attacker_cycle(records=33, start=40, attack=40):
    """V3-09 attack (V3-DESIGN.md): with R_max = 1 an attacker holding W_act+1 = 33 records deregisters one
    and reactivates another every block, its transactions first; a newcomer then tries to register."""
    steps = [('register', f'a{i}', f'a{i}', B, 1 + i) for i in range(records)]
    first = start + records - 1                        # every attacker record exited once before the cycle
    for t in range(start, first):
        steps.append(('deregister', f'a{(t - start) % records}', t))
    for t in range(first, first + attack):
        steps.append(('deregister', f'a{(t - start) % records}', t))
        steps.append(('reactivate', f'a{(t - start - (records - 1)) % records}', t))
        if t == first:
            steps.append(('register', 'newcomer', 'newcomer', B, t))
    return steps

def run_random(seed, n=700):
    """A seeded random walk at small windows so that every budget, clock, window and the bounded sweep
    cursor are exercised. Each step usually picks an actor whose record suits the operation (a probe copy of
    the model is consulted; it never decides a result), otherwise any actor, so reverts stay frequent.
    punish is emitted only for a live record (the host never punishes otherwise)."""
    cp = {'D': 2, 'E_win': 2, 'W_act': 2, 'U': 6, 'C_react': 6, 'R_react': 2, 'Theta_inact': 4, 'R_max': 2,
          'REG_OPS': 4, 'T_PROB': 30, 'P_MAX': 3, 'M_max': 8, 'SWEEP_MAX': 1, 'SWEEP_SCAN': 2}
    actors, rewards = [f'r{i}' for i in range(10)], ['rw1', 'rw2']
    rng, probe, steps, h = random.Random(seed), Registry(cp, rewards), [], 1
    values = [0, 3 * PCOL, 11 * PCOL, B, B + 1]
    def pick(pred):
        fits = [a for a in actors if pred(probe.records.get(a))]
        return rng.choice(fits) if fits and rng.random() < 0.8 else rng.choice(actors)
    live = lambda r: r is not None and r.live()
    active = lambda r: live(r) and r.status == 1
    exited = lambda r: live(r) and r.status == 2
    while len(steps) < n:
        h += rng.choice([0, 0, 0, 1, 1, 2])
        if h > probe.height:          # start the block explicitly, so that the replay starts it at the same step
            steps.append(('sweep', h))
            probe.sweep(h)
        k = rng.random()
        if k < 0.15:
            a = pick(lambda r: r is None)
            s = ('register', a, rng.choice([a, a, *rewards]), rng.choice(values), h)
        elif k < 0.24:
            s = ('top_up', pick(lambda r: active(r) and r.bond_state == ACCRUING), rng.choice([1, 4 * PCOL, 9 * PCOL, B]), h)
        elif k < 0.33:
            a = pick(live)
            s = ('set_reward', a, rng.choice([a, *rewards, 'nowhere']), h)
        elif k < 0.47:
            s = ('deregister', pick(active), h)
        elif k < 0.63:
            s = ('reactivate', pick(lambda r: exited(r) and h >= r.exit + cp['W_act']), h)
        elif k < 0.71:
            s = ('claim_bond', pick(lambda r: exited(r) and h >= r.due), h)
        elif k < 0.80:
            s = ('remove_inactive', rng.choice(actors), rng.choice(['m0', pick(active)]), h)
        elif k < 0.97:
            s = ('settle_to', rng.choice(actors), rng.choice([PCOL, 6 * PCOL, 25 * PCOL]))
        else:
            a = pick(live)
            if not live(probe.records.get(a)):
                continue
            s = ('punish', a, h)
        steps.append(s)
        try:
            getattr(probe, s[0])(*s[1:])
        except Revert:
            pass
    return run(steps, cp, rewards)

def vectors():
    v = {}
    live = {'REG_OPS': 16, 'R_max': 1}
    # V3-08: one HELD identity spends REG_OPS on setRewardAddress (it needs only a live record). On the
    # earn-in programs every join, exit and eviction then reverts for the block; here only its first call
    # succeeds, the newcomer registers and the spammer, inactive since its registration, is evicted.
    v['maintenance-spam-one-identity'] = run(
        [('register', 's', 's', B, 1)] + [('set_reward', 's', 'x', 100)] * 16 +
        [('register', 'n', 'n', B, 100), ('remove_inactive', 'n', 's', 100), ('set_reward', 's', 'x', 101)],
        live, durable=['x'])
    # V3-08: eight identities exhaust REG_MAINT_MAX; joins keep REG_OPS - 8; the next block starts afresh.
    ids = [f'k{i}' for i in range(10)]
    v['maintenance-budget-many-identities'] = run(
        [('register', i, i, B, 1) for i in ids] +
        [('set_reward', i, i, 2) for i in ids[:9]] +
        [('register', f'j{i}', f'j{i}', B, 2) for i in range(9)] +
        [('deregister', 'k0', 2), ('set_reward', 'k8', 'k8', 3), ('register', 'j8', 'j8', B, 3)],
        {'REG_OPS': 16, 'R_max': 16})
    # V3-08: topUp is maintenance; one per identity per block, shared with setRewardAddress.
    v['maintenance-top-up'] = run(
        [('register', 'p', 'p', 2 * PCOL, 1), ('register', 'q', 'q', 0, 1), ('top_up', 'p', PCOL, 2),
         ('top_up', 'p', PCOL, 2), ('set_reward', 'p', 'p', 2), ('top_up', 'q', B, 2), ('set_reward', 'q', 'q', 2),
         ('set_reward', 'p', 'p', 3), ('top_up', 'p', PCOL, 3), ('top_up', 'p', B, 4)],
        {'REG_OPS': 16})
    # V3-09: R_react per block (and not R_max: a registration still lands after two reactivations), the
    # C_react clock from the previous reactivation (exactly last + C_react succeeds) and the window.
    v['react-budget-and-cooldown'] = run(
        [('register', x, x, B, 1 + i) for i, x in enumerate('abcd')] + [('deregister', x, 5) for x in 'abcd'] +
        [('reactivate', 'a', 36), ('reactivate', 'a', 37), ('reactivate', 'b', 37), ('reactivate', 'c', 37),
         ('register', 'e', 'e', B, 37), ('reactivate', 'c', 38), ('reactivate', 'd', 38),
         ('deregister', 'a', 39), ('deregister', 'b', 39), ('reactivate', 'a', 76), ('reactivate', 'a', 77),
         ('reactivate', 'b', 78), ('deregister', 'c', 80), ('reactivate', 'c', 112), ('reactivate', 'c', 119)],
        {'R_react': 2, 'R_max': 1, 'REG_OPS': 16})
    # V3-09: the window closes at dueHeight even where the refund sweep has not reached the record
    # (SWEEP_MAX 0: only claimBond refunds, including an ACCRUING record's).
    v['react-window-unswept'] = run(
        [('register', x, x, B, 1) for x in 'ab'] + [('register', 'z', 'z', 0, 1)] +
        [('deregister', x, 2) for x in 'abz'] +
        [('reactivate', 'a', 33), ('claim_bond', 'a', 41), ('reactivate', 'b', 41), ('reactivate', 'a', 42),
         ('claim_bond', 'a', 42), ('claim_bond', 'z', 42), ('deregister', 'b', 43), ('reactivate', 'b', 80),
         ('reactivate', 'b', 82)],
        {'SWEEP_MAX': 0})
    # ACC_COUNT through every path: probation register, top-up and accrual promotion, exit and reactivation,
    # eviction, punish and the tomb erase, sweep refunds and the probation deadline exit (claimBond above).
    v['acc-count-paths'] = run(
        [('register', f'p{i}', f'p{i}', 5 * PCOL if i == 3 else 0, 1) for i in range(1, 7)] +
        [('top_up', 'p1', B, 2), ('settle_to', 'p2', 25 * PCOL), ('deregister', 'p3', 3), ('reactivate', 'p3', 35),
         ('remove_inactive', 'p1', 'p4', 65), ('punish', 'p5', 66), ('deregister', 'p3', 67), ('sweep', 87),
         ('sweep', 105), ('sweep', 107), ('sweep', 160), ('register', 'p7', 'p7', 0, 161), ('sweep', 200)],
        {'T_PROB': 150, 'P_MAX': 6, 'REG_OPS': 16})
    # V3-09 attack: 33 attacker records cycling deregister/reactivate. The newcomer's registration lands in the
    # first block of the cycle; each attacker record's second reactivation inside C_react = U reverts.
    v['attacker-cycle'] = run(attacker_cycle(), live)
    # D-V3-08/09: the three budgets are independent. Once register/deregister/claimBond/removeInactive have used up
    # REG_OPS, setRewardAddress and topUp (REG_MAINT_MAX) and reactivate (R_react) still succeed in the same block;
    # maintenance used first still shrinks REG_OPS for the others (REG_OPS minus maintenance used).
    v['reg-ops-exhausted-maint-and-react'] = run(
        [('register', x, x, B, 1) for x in 'abc'] + [('register', 'p', 'p', 0, 1), ('register', 'd', 'd', B, 1),
         ('deregister', 'c', 1), ('set_reward', 'a', 'a', 1), ('top_up', 'p', PCOL, 1), ('set_reward', 'b', 'b', 1),
         ('register', 'd', 'd', B, 2), ('register', 'e', 'e', B, 2), ('deregister', 'a', 2), ('deregister', 'b', 2),
         ('remove_inactive', 'e', 'c', 2), ('set_reward', 'c', 'c', 2), ('top_up', 'p', PCOL, 2),
         ('set_reward', 'd', 'd', 3), ('set_reward', 'e', 'e', 3), ('top_up', 'p', PCOL, 3),
         ('deregister', 'c', 3), ('register', 'f', 'f', B, 3), ('deregister', 'd', 3),
         ('deregister', 'd', 4), ('register', 'f', 'f', B, 4), ('register', 'g', 'g', B, 4), ('deregister', 'e', 4),
         ('deregister', 'f', 36), ('deregister', 'g', 36), ('register', 'h', 'h', B, 36),
         ('register', 'i', 'i', B, 36), ('register', 'j', 'j', B, 36), ('reactivate', 'a', 36),
         ('reactivate', 'b', 36), ('set_reward', 'p', 'p', 36), ('deregister', 'h', 36)],
        {'REG_OPS': 4, 'R_max': 4})
    # D-V3-08: the per-identity maintenance clock lives in the record (w0), so it lasts the record's lifetime. An
    # exited identity whose dueHeight block the sweep has not reached (SWEEP_MAX 0) can maintain, claimBond,
    # register again and maintain once more in one block: two REG_OPS and an R_max entry for the second
    # maintenance operation, so no amplification of REG_OPS (REGISTRY-LAYOUT.md).
    v['maintenance-clock-per-record'] = run(
        [('register', 'x', 'x', B, 1), ('deregister', 'x', 2), ('set_reward', 'x', 'x', 42),
         ('set_reward', 'x', 'x', 42), ('claim_bond', 'x', 42), ('register', 'x', 'x', B, 42),
         ('set_reward', 'x', 'x', 42), ('set_reward', 'x', 'x', 42), ('set_reward', 'x', 'x', 43)],
        {'SWEEP_MAX': 0})
    for seed in (1, 2, 3):
        v[f'random-{seed}'] = run_random(seed)
    return v

def earnin_on_v3():
    """The earn-in model's scenarios (earnin_model.py at P_MAX 8 / T_PROB 60,480, and at the O-14=b values of
    v3_model_vectors.py) re-run through this model. Outside V3-08/V3-09 the two models agree step for step; the
    first difference in any scenario must be a V3-08/V3-09 refusal (RegOpsLimit), and it occurs in 'top-up' only,
    whose second topUp in one block from one identity is the one maintenance operation too many."""
    out = {}
    for label, (p_max, t_prob, m_max), cp in (
            ('earnin', (8, 60_480, 64), {'P_MAX': 8, 'T_PROB': 60_480}),
            ('earnin-p64', (64, 86_400, 256), {'P_MAX': 64, 'T_PROB': 86_400, 'M_max': 256, 'R_max': 255, 'REG_OPS': 1024})):
        saved = (em.P_MAX, em.T_PROB, em.M_MAX)
        em.P_MAX, em.T_PROB, em.M_MAX = p_max, t_prob, m_max
        try:
            base = em.vectors()
        finally:
            em.P_MAX, em.T_PROB, em.M_MAX = saved
        differ = []
        for name, body in sorted(base.items()):
            if 'steps' not in body:
                continue
            steps = [tuple(int(x) if x.isdigit() else x for x in st['op']) for st in body['steps']]
            v = run(steps, cp, body['durable'])
            key = lambda st: (st['ok'], st.get('revert'))
            first = next((i for i, (a, b) in enumerate(zip(body['steps'], v['steps'])) if key(a) != key(b)), None)
            if first is not None:
                assert v['steps'][first].get('revert') == 'RegOpsLimit', (label, name, first)
                differ.append(name)
            out[f'{label}/{name}'] = v
        assert differ == ['top-up'], (label, differ)
    return out

if __name__ == '__main__':
    files = {'registry-budget-vectors.json': vectors, 'earnin-on-v3-vectors.json': earnin_on_v3}
    for name, make in files.items():
        path = Path(__file__).with_name('v3') / name
        data = json.dumps(make(), indent=1, sort_keys=True) + '\n'
        if sys.argv[1:] == ['--write']:
            path.write_text(data)
            print('wrote', path)
        else:
            assert path.read_text() == data, f'{name} differs from the model; run --write and review'
            print('PASS v3 registry-budget model matches', name)
