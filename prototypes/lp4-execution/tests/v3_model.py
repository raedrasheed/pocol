#!/usr/bin/env python3
"""Independent model of the v3 registry budgets (V3-08, V3-09; V3-DESIGN.md, V3-DECISIONS.md D-V3-08/09) and of
the v3 evidence penalties (V3-16, V3-17; D-V3-16/17), extending the earn-in model (earnin_model.py).

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

- V3-16: type-2 evidence settles pot j normally after punish (the tombed offender's winner part, proposer part and
  shares burn; honest contributors are paid), except that the whole pot burns when the accused is pot j's winner
  and that winner was ACCRUING (bond below B_reg) when the result of block j was recorded. The v3 host records the
  parent's result before the block's evidence, so that fact is the state at block j, never read later.
- V3-17: punish pays rho_bp/10^4 of the burned bond to the reward address of the carrying block's signed proposer
  (the reporter; a self-report included) and burns the rest; a fallback block has no reporter and burns it all.
  The reporter passed with the evidence must be the block's proposer (zero on a fallback), else the block is invalid.

The evidence scenarios (class Chain) run whole blocks: phase 0 in the v3 order (recordShares and recordResult of
h-1, evidence, markClose, pruneShares, settle(h-D), beginBlock), the block's user transactions, phase 2 (issuance
and closeBlock). Settlement follows M1 incentives.md (winnerPart, propPart, R split over Q = max(C, kappa*m), payBurn,
penaltyBurn, baseBurn, roundBurn) with the earn-in rule that a payment to an ACCRUING record fills its bond first.

- V3-02 (late share credit): kind-3 evidence of block h credits valid shares of an ancestor template j in
  [max(1, h-D), h-1] that its header omitted, through recordLate(j, owners) after recordShares/recordResult of h-1
  and before settle(h-D) (class Chain); the program refuses j outside the window, an unrecorded j and a credit past
  the no-dilution cap total[j] + n <= kappa*m, and sets lastShare = max(lastShare, j). The host's validity rules
  (pocold chain.rs late_check) are modelled by late_verdict over concrete TemplateIDs, targets and nonce ranges, with
  17 planted mutants (LATE_MUTANTS) that each fail one of tests/v3/late-share-validity-vectors.json.

- V3-19 (issuance M3), V3-15 (carry) and V3-20 (tip split), V3-PR-E2a: phase 2 credits issuance(h) = h == 0 ? 0 :
  max(k >= 256 ? 0 : S0 >> k, S_tail), k = h // H_half, and closeBlock(tips, issued) records pot h = issued +
  (1 - gamma_tip) * tips + carry into pot h, zeroing that carry; gamma_tip_bp/10^4 of the tips (floor) goes to the
  signed proposer as the proposer part does (payTo: an ACCRUING record fills its bond first, a tomb burns), a fallback
  block leaves all tips in the pot. Settlement sends the base burn R - A to the carry (roundBurn, payBurn, penaltyBurn
  and a whole-pot burn still burn), lagged by one block (D-V3-15): settle(h - D) in block h adds to the carry into pot
  h + 1, so pot h receives the base burn of settle(h - 1 - D), computed after every late credit to h - 1 - D, and block
  h, the last carrier of late shares of h - D, has no claim on what leaving them out would carry
  (check_last_carrier_gains_nothing). The Escrow equality gains the carry term (every outstanding carry); after phase 2
  of block h only the carry into pot h + 1 may remain, and none after a final block.
  tests/v3/issuance-carry-vectors.json; ECON_MUTANTS are eight planted model mutants that each fail it, and
  check_pot_fixed_by_its_block fails on base-burn-into-closed-pot, check_last_carrier_gains_nothing on carry-not-lagged.

- V3-10 (contestable seats) and V3-14 (the probation-pool contest), V3-PR-E4: recordShares and recordLate keep per
  record epoch counters (the template epoch j // E_epoch; countCur for the epoch of lastShare, countPrev for the one
  before, each saturating at 2^20 - 1) and the epoch totals of every credited share. registerContest(reward, victim)
  takes two REG_OPS units and one R_max entry, only with the set full (active == M_max); the victim exits as by
  deregister (refund after U, payable and punishable until then). A probationer newcomer facing a full pool (ACC_COUNT >=
  P_MAX) displaces an ACCRUING victim, which also gives up its pool seat at once (owner decision 2026-11-13, option a:
  pool_out, ACC_COUNT down at the contest and not again at its refund, punish or promotion; reactivate takes a pool
  seat again). The victim must be live and active, past grace
  (graceStart + Theta_inact), registered or reactivated at or before the first block of the measured epoch (the previous
  one), judged only once that epoch's late window has closed (h mod E_epoch >= D - 1), and below the threshold:
  countPrev < E - k_sigma*sqrt(E), E = theta*Tprev/active, decided exactly (threshold, below_exact).
  tests/v3/contest-vectors.json (with the threshold table).

tests/v3_contracts.rs replays tests/v3/registry-budget-vectors.json, tests/v3/earnin-on-v3-vectors.json (the earn-in
model's scenarios re-run here), tests/v3/evidence-penalty-vectors.json, tests/v3/late-share-vectors.json,
tests/v3/issuance-carry-vectors.json and tests/v3/contest-vectors.json, written by
this script, on the v3 programs and checks every step's result, every record (including the new clocks), the record array order, the sweep
cursor, the counters, ACC_COUNT, every pending pot and the Escrow accounting. Run with --check (default) to compare
against the committed files, or --write to regenerate them.
"""
import copy, json, math, random, sys
from dataclasses import dataclass, asdict
from pathlib import Path
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import earnin_model as em

PCOL = em.PCOL
B = em.B
HELD, BURNED, PAID, ACCRUING = em.HELD, em.BURNED, em.PAID, em.ACCRUING
REG_MAINT_MAX = 8          # V3-08: a program constant, not a CP field
MAINT_MOD = 1 << 32        # V3-08: lastMaintHeight is kept modulo 2^32 (w1's 32 spare bits since V3-PR-E4)
CNT_SAT = (1 << 20) - 1    # V3-10: each epoch counter saturates at 2^20 - 1 (20 spare w0 bits); a saturated count is
                           # never contestable
Revert = em.Revert

# The CP the contract harness installs (tests/v3_contracts.rs, as earnin_contracts.rs) by schema name.
CP_INDEX = {'D': 11, 'R_max': 17, 'M_max': 18, 'M_min': 19, 'SWEEP_MAX': 21, 'SWEEP_SCAN': 22, 'REG_OPS': 23,
            'W_act': 24, 'Theta_inact': 25, 'E_win': 26, 'U': 27, 'P_MAX': 54, 'T_PROB': 55, 'R_react': 61,
            'C_react': 62, 'E_epoch': 63, 'theta_bp': 64, 'k_sigma': 65}
# V3-10 (O-10 = b): E_epoch one day at T_blk 30 s, theta = 1/8, k_sigma = 3 (V3-ROADMAP §5); scenarios override them.
DEFAULT_CP = {'D': 4, 'R_max': 16, 'M_max': 64, 'M_min': 1, 'SWEEP_MAX': 16, 'SWEEP_SCAN': 64, 'REG_OPS': 64,
              'W_act': 32, 'Theta_inact': 64, 'E_win': 16, 'U': 40, 'P_MAX': 8, 'T_PROB': 60_480, 'R_react': 16,
              'C_react': 40, 'E_epoch': 2880, 'theta_bp': 1250, 'k_sigma': 3}

@dataclass
class Record(em.Record):
    grace: int = 0           # graceStart: registration or last reactivation
    burn: int = 0            # burnHeight (punish)
    last_share: int = 0      # raised by recordShares and recordLate (the registry-only scenarios run neither)
    last_maint: int = 0      # V3-08 lastMaintHeight modulo 2^32, 0 = never
    last_react: int = 0      # V3-09 lastReactHeight, 0 = never
    cnt_cur: int = 0         # V3-10 shares credited in epoch last_share // E_epoch (saturating at CNT_SAT)
    cnt_prev: int = 0        # V3-10 shares credited in the epoch before it
    pool_out: bool = False   # V3-14 (owner decision 2026-11-13): a pool-contest victim that gave up its pool seat at the
                             # contest (POOL_OUT, w0 bit 87); cleared by reactivate. Snapshots name it only when set.

    def pooled(self):
        """An ACCRUING record that holds a pool seat: counted in ACC_COUNT."""
        return self.bond_state == ACCRUING and not self.pool_out

def threshold(tprev, active, theta_bp, k_sigma):
    """V3-10: the least integer count that is not below E - k_sigma*sqrt(E), E = theta*Tprev/active, theta = theta_bp/10^4:
    with N = theta_bp*Tprev and Dn = 10^4*active the real threshold is (N - sqrt(k^2*N*Dn))/Dn, so the integer one is
    ceil((N - isqrt(k^2*N*Dn))/Dn) (ceil(N - x) = N - floor(x)), 0 when that is not positive. A count c is contestable
    iff c < threshold (checked exactly by below_exact)."""
    n, d = theta_bp * tprev, 10_000 * active
    s = math.isqrt(k_sigma * k_sigma * n * d)
    return -((s - n) // d) if n > s else 0

def below_exact(c, tprev, active, theta_bp, k_sigma):
    """c < E - k*sqrt(E) over the reals, without a square root: E - c > 0 and k^2*E < (E - c)^2, scaled by Dn."""
    n, d = theta_bp * tprev, 10_000 * active
    return n - c * d > 0 and k_sigma * k_sigma * n * d < (n - c * d) ** 2

class Registry(em.Registry):
    """The earn-in Registry with v3 budgets, a block clock and the bounded sweep. Inherits the earn-in
    settlement (accrue, settle_to) unchanged; _promote leaves ACC_COUNT alone for a pool-contest victim (V3-14)."""

    def __init__(self, cp, durable=()):
        super().__init__(durable=set(durable))
        self.cp = dict(DEFAULT_CP, **cp)
        self.height = 0
        self.order = []                      # RecArr: insertion order, swap-removal on erase
        self.cursor = 0                      # sweepCur
        self.ops = self.regs = self.maint = self.react = 0
        self.totals = {'epoch': 0, 'cur': 0, 'prev': 0}   # V3-10 slot 10: every credited share, by template epoch
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
        elif kind == 'contest':
            # V3-10: a removal and a registration, two REG_OPS units, and one R_max entry.
            if self.ops + self.maint + 1 >= self.cp['REG_OPS'] or self.regs >= self.cp['R_max']:
                raise Revert('RegOpsLimit')
        else:
            if self.ops + self.maint >= self.cp['REG_OPS'] or (kind == 'register' and self.regs >= self.cp['R_max']):
                raise Revert('RegOpsLimit')

    def _spend(self, kind):
        if kind == 'maint':
            self.maint += 1
        elif kind == 'react':
            self.react += 1
        elif kind == 'contest':
            self.ops += 2
            self.regs += 1
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
            self._enroll(sender, reward, value, h)
        return self._tx(sender, 'register', body)

    def _enroll(self, sender, reward, value, h):
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

    # ---- V3-10 / V3-14: contestable seats and the probation-pool contest
    def credit(self, ident, j, n):
        """n shares of template j to an existing record: lastShare = max(lastShare, j) and its epoch counters. The
        counters describe epoch last_share // E_epoch (cnt_cur) and the one before (cnt_prev); a credit to a later epoch
        rolls them forward (cnt_prev = cnt_cur when it is the next epoch, else 0), one to the previous epoch (a late
        share older than lastShare's epoch) raises cnt_prev, one to an older epoch only keeps lastShare's max."""
        r = self.records.get(ident)
        if r is None:
            return
        e = self.cp['E_epoch']
        el, ej = r.last_share // e, j // e
        cur, prev = r.cnt_cur, r.cnt_prev
        if ej == el:
            cur += n
        elif ej > el:
            prev, cur = (cur if ej == el + 1 else 0), n
        elif ej + 1 == el:
            prev += n
        r.cnt_cur, r.cnt_prev = min(cur, CNT_SAT), min(prev, CNT_SAT)
        r.last_share = max(r.last_share, j)

    def credit_total(self, j, n):
        """Every credited share (header or late, recorded owner or not) into the epoch totals, rolled as a record."""
        t, e = self.totals, self.cp['E_epoch']
        ej = j // e
        if ej == t['epoch']:
            t['cur'] += n
        elif ej > t['epoch']:
            t['prev'], t['cur'], t['epoch'] = (t['cur'] if ej == t['epoch'] + 1 else 0), n, ej
        elif ej + 1 == t['epoch']:
            t['prev'] += n

    def contest_check(self, victim, probation, h):
        """The contest rule (codes as PoColSystemV3.sol contestCheck): 0 contestable, 1 no live active victim, 2 nothing
        to contest (the set not full, for a pool contest too), 3 pool full and the victim not ACCRUING, 4 no measured epoch or its late window open, 5 in grace,
        6 registered or reactivated after the first block of the measured epoch, 7 not below the threshold."""
        r = self.records.get(victim)
        if r is None or not r.live() or r.status != 1:
            return 1, 0, 0
        pool = probation and self.accruing >= self.cp['P_MAX']
        if len(self.active) < self.cp['M_max']:
            return 2, 0, 0
        if pool and r.bond_state != ACCRUING:
            return 3, 0, 0
        e = self.cp['E_epoch']
        cur = h // e
        if cur == 0 or h % e + 1 < self.cp['D']:
            return 4, 0, 0
        if h < r.grace + self.cp['Theta_inact']:
            return 5, 0, 0
        if r.grace > (cur - 1) * e:
            return 6, 0, 0
        p = cur - 1
        el = r.last_share // e
        count = r.cnt_cur if el == p else r.cnt_prev if el == p + 1 else 0
        t = self.totals
        tp = t['cur'] if t['epoch'] == p else t['prev'] if t['epoch'] == p + 1 else 0
        thr = threshold(tp, len(self.active), self.cp['theta_bp'], self.cp['k_sigma'])
        if count >= thr or count >= CNT_SAT:
            return 7, count, thr
        return 0, count, thr

    def contest_status(self, victim, probation, h):
        """The contestStatus(victim, probation) view at the current block: 'code,count,threshold'."""
        self.block(h)
        return ','.join(map(str, self.contest_check(victim, probation in (True, 'true', 1), h)))

    def register_contest(self, sender, reward, victim, value, h):
        """registerContest(reward, victim) with bond `value`, only with the set full (active == M_max, the pool contest
        included): the victim exits as by deregister (refund after U, no burn; it stays payable, so its parts of pending pots
        fill its accrued bond, and punishable until dueHeight). A probationer newcomer facing a full pool displaces an
        ACCRUING victim from the pool: owner decision 2026-11-13 (option a), the victim gives up its pool seat at once
        (pool_out, ACC_COUNT down here and never again for it). Then the sender registers as register(reward)."""
        self.block(h)
        if value > B:
            self.durable.add(sender)
            raise Revert('BadValue')
        def body():
            if sender in self.records:
                raise Revert('BadState')
            probation = value < B
            code = self.contest_check(victim, probation, h)[0]
            if code in (1, 3):
                raise Revert('BadState')
            if code:
                raise Revert('NotContestable')
            pool = probation and self.accruing >= self.cp['P_MAX']
            self._exit(victim, h)
            if pool:
                self.records[victim].pool_out = True
                self.accruing -= 1
            self._enroll(sender, reward, value, h)
        return self._tx(sender, 'contest', body)

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
            if r.bond_state == ACCRUING and r.pool_out:       # V3-14: a pool-contest victim takes a pool seat again
                if self.accruing >= self.cp['P_MAX']:
                    raise Revert('Capacity')
                self.accruing += 1
            r.status, r.exit, r.due, r.grace, r.last_react, r.pool_out = 1, 0, 0, h, h, False
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
        if r.pooled():
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
        if r.pooled():                 # V3-14: a pool-contest victim left ACC_COUNT at the contest
            self.accruing -= 1

    def _promote(self, r):
        if r.bond == B:
            if r.pooled():
                self.accruing -= 1
            r.bond_state = HELD

    def _erase(self, ident):
        pos = self.order.index(ident)
        last = self.order.pop()
        if last != ident:
            self.order[pos] = last
        del self.records[ident]

# ---------------- vectors
def record_json(r):
    """A record as the vectors carry it; pool_out (V3-14) only when set, so scenarios without a pool contest are unchanged."""
    out = {**asdict(r), 'bond': str(r.bond)}
    if not out['pool_out']:
        del out['pool_out']
    return out

def snapshot(reg):
    return {'records': {k: record_json(v) for k, v in sorted(reg.records.items())},
            'order': list(reg.order), 'cursor': reg.cursor, 'active': sorted(reg.active),
            'heldTotal': str(reg.held_total), 'accruing': reg.accruing, 'burned': str(reg.burned),
            'counters': {'ops': reg.ops, 'regs': reg.regs, 'maint': reg.maint, 'react': reg.react},
            'epochTotals': dict(reg.totals),
            'paid': {k: str(v) for k, v in sorted(reg.paid.items())},
            'spent': {k: str(v) for k, v in sorted(reg.spent.items())}}

def check_invariants(reg, step):
    live = [r for r in reg.records.values() if r.live()]
    assert reg.held_total == sum(r.bond for r in live), step
    # ACC_COUNT conservation: the counter is exactly the number of live ACCRUING records that hold a pool seat (V3-14:
    # all of them but pool-contest victims, which gave theirs up at the contest).
    assert reg.accruing == sum(1 for r in live if r.pooled()), step
    assert all(r.status == 2 or not r.pool_out for r in reg.records.values()), step
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
    users = {'register', 'deregister', 'claim_bond', 'remove_inactive', 'register_contest'}
    for step in steps:
        op, args = step[0], step[1:]
        if op in users:
            reg.block(args[-1])
            before = (reg.ops, reg.maint)
        try:
            result = getattr(reg, op)(*args)
            out.append({'op': [op, *map(str, args)], 'ok': True, 'result': None if result is None else str(result)})
            if op in users:
                assert before[0] + before[1] + (op == 'register_contest') < reg.cp['REG_OPS'], step
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

# ---------------- V3-16 / V3-17: evidence penalties (V3-PR-E2b)
# Settlement CP fields the contract harness installs for the chain scenarios (CP schema v2 indices as CP_INDEX).
CHAIN_CP_INDEX = {'m': 8, 'kappa': 9, 'S_max': 10, 'W_w': 12, 'alpha_bp': 13, 'gamma_bp': 14, 'S0': 15, 'H_half': 52,
                  'S_tail': 53, 'gamma_tip_bp': 59, 'rho_bp': 66}
# V3-19/V3-20 (O-19 = M3, O-20 = b): H_half 4,207,680 blocks (four years at 30 s), S_tail = S0/32, gamma_tip 50%.
CHAIN_DEFAULT = {'m': 32, 'kappa': 2, 'S_max': 256, 'W_w': 8, 'alpha_bp': 1000, 'gamma_bp': 500, 'S0': 2 * PCOL,
                 'H_half': 4_207_680, 'S_tail': 2 * PCOL // 32, 'gamma_tip_bp': 5000, 'rho_bp': 2500}

# V3-PR-E2a planted model mutants (econ_mutants_killed): each must make tests/v3/issuance-carry-vectors.json differ from
# the model or fail one of its Escrow checks.
# base-burn-into-closed-pot is the design V3-15 rejects (safety finding 8): half of settle(j)'s base burn is added to the
# oldest pot still pending, already closed by its block; check_pot_fixed_by_its_block must also fail on it
# (fixedness_mutant_killed). carry-not-lagged is the previous rule (settle(h - D)'s base burn into pot h, the pot of the
# last carrier of h - D); check_last_carrier_gains_nothing must also fail on it (lag_mutant_killed). final-keeps-own-carry:
# finalize burns only the carry into pot F and keeps the carry of F's own settle(F - D).
ECON_MUTANTS = ['carry-not-added', 'tip-share-to-pot', 'base-burn-still-burned', 'invariant-missing-carry',
                'issuance-off-by-one-at-H_half', 'base-burn-into-closed-pot', 'carry-not-lagged', 'final-keeps-own-carry']
MUTANT = None

def issuance(h, cp):
    """V3-19 M3: issuance(h) = h == 0 ? 0 : max(k >= 256 ? 0 : S0 >> k, S_tail), k = h // H_half."""
    if h == 0:
        return 0
    k = (h - 1) // cp['H_half'] if MUTANT == 'issuance-off-by-one-at-H_half' else h // cp['H_half']
    return max(0 if k >= 256 else cp['S0'] >> k, cp['S_tail'])

class Chain:
    """Whole blocks over the v3 registry model: the v3 phase 0, user transactions, phase 2, and RewardEscrow's pending
    pots and settlement. The Escrow balance is never stored: it is the genesis bond plus what senders moved into the
    registry and the issuance, less every payment out of Escrow, and check() requires it to equal the pending pots
    plus the held bonds plus everything burned (no surplus: the scenarios make no forced transfers)."""

    def __init__(self, cp, durable=()):
        self.cp = dict(DEFAULT_CP, **{**CHAIN_DEFAULT, **cp})
        self.reg = Registry(self.cp, durable)
        self.reg.durable.add('m0')           # the genesis member's reward account exists with a nonce
        self.pots = {}                       # j -> {amount, proposer ('' = fallback), winner, accruing, invalid}
        self.ledger = {}                     # j -> {owner: count}, owners in first-seen order
        self.issued = 0
        self.tips = 0                        # V3-20: tips the block's transactions paid to Escrow (the coinbase)
        self.carry = {}                      # V3-15 (lagged): t -> the carry into pot t, the base burn of the settlement
                                             # in block t - 1, until closeBlock(t) (or finalize burns it)
        self.final = False                   # set by finalize: the final block's settleFinal burns its base burns
        self.settled = {}                    # j -> pot j's amount when settle(j) or settleFinal(j) distributed it
        self.height = 0
        self.proposer = ''

    # ---- payments out of Escrow
    def _pay(self, to, amount):
        """PAY to a reward address: a recipient that is not durable (PAY-02) fails, and the amount burns."""
        if to in self.reg.durable:
            self.reg.pay(to, amount)
        else:
            self.reg.burned += amount

    def _pay_to(self, ident, amount):
        """payTo and distribute: an absent record (payBurn) or a tomb (penaltyBurn) burns; an ACCRUING record fills
        its bond first (EARN-IN P1); the rest is paid to the reward address."""
        if amount == 0:
            return
        r = self.reg.records.get(ident)
        if r is None or r.tomb:
            self.reg.burned += amount
            return
        out = self.reg.accrue(ident, amount) if r.bond_state == ACCRUING else amount
        if out:
            self._pay(r.reward, out)

    def _accruing(self, ident):
        r = self.reg.records.get(ident)
        return r is not None and r.live() and r.bond_state == ACCRUING

    # ---- phase 0
    def begin(self, h, proposer, winner, shares, evidence):
        if h != self.height + 1:
            raise AssertionError(('heights are contiguous', h, self.height))
        self.proposer = proposer
        if h >= 2:
            counts = {}
            for o in list(shares) + [winner]:           # recordShares: the header shares, then the implicit winner
                counts[o] = counts.get(o, 0) + 1
            self.ledger[h - 1] = counts
            for o, n in counts.items():                 # lastShare = h - 1; V3-10 epoch counters and totals
                self.reg.credit(o, h - 1, n)
            self.reg.credit_total(h - 1, sum(counts.values()))
            pot = self.pots[h - 1]                      # recordResult: the winner and whether it is ACCRUING now
            pot['winner'], pot['accruing'] = winner, self._accruing(winner)
        for e in evidence:
            if e[0] == 'late':                          # V3-02: recordLate(j, owners), in body order
                self._record_late(e[1], e[2], h)
                continue
            accused, j, reporter = e
            if j is not None and j in self.pots:        # the host invalidates only a pending pot
                pot = self.pots[j]
                if pot['winner'] == accused and pot['accruing']:
                    pot['invalid'] = True
            self._punish(accused, reporter, h)
        k = max(self.cp['D'], self.cp['W_w'])            # pruneShares (markClose: nothing on a continuous network)
        if h > k + 1:
            del self.ledger[h - k - 1]
        if h > self.cp['D']:
            self._settle(h - self.cp['D'], h)
        self.reg.block(h)                                # beginBlock: budgets and the refund sweep
        self.height = h

    def _record_late(self, j, owners, h):
        """V3-02 recordLate (MinerRegistry, nested ShareLedger): one owner per late share of template j, which the host
        validated. The program checks the call shape, the window j in [max(1, h-D), h-1], that j's shares are recorded
        and the no-dilution cap total[j] + n <= kappa*m; then count and total rise and lastShare = max(lastShare, j)
        for every existing record (never a revert when a later share already moved it past j)."""
        if not owners or len(owners) > self.cp['S_max']:
            raise Revert('BadCalldata')
        if h < 2 or j == 0 or j >= h or j + self.cp['D'] < h:
            raise Revert('BadPhase')
        counts = self.ledger.get(j)
        total = sum(counts.values()) if counts else 0
        if total == 0 or total + len(owners) > self.cp['kappa'] * self.cp['m']:
            raise Revert('BadState')
        for o in owners:
            counts[o] = counts.get(o, 0) + 1
        for o in owners:                                # lastShare = max(lastShare, j); V3-10 epoch counters
            self.reg.credit(o, j, 1)
        self.reg.credit_total(j, len(owners))

    def _punish(self, ident, reporter, h):
        if reporter != self.proposer:
            raise Revert('BadCalldata')                  # the reporter is the block's signed proposer, none on a fallback
        reg = self.reg
        r = reg.records.get(ident)
        if r is None or r.tomb:
            raise Revert('BadState')                     # a host error: the block is invalid
        if r.bond_state == PAID:
            r.tomb, r.burn = True, h
            return
        if r.pooled():
            reg.accruing -= 1
        if r.status == 1:
            reg.active.remove(ident)
            r.status, r.exit, r.due = 2, h, h
        reg.held_total -= r.bond
        r.bond_state, r.tomb, r.burn = BURNED, True, h
        rep = reg.records.get(reporter) if reporter else None
        cut = r.bond * self.cp['rho_bp'] // 10_000 if rep is not None else 0
        if cut:
            self._pay(rep.reward, cut)
        reg.burned += r.bond - cut

    def _settle(self, j, h=None):
        """settle(j) in block h (settleFinal at a final block: h is not needed, the base burn burns)."""
        pot = self.pots.pop(j)
        amount = pot['amount']
        self.settled[j] = amount
        if pot['invalid']:
            self.reg.burned += amount
            return
        wp = amount * self.cp['alpha_bp'] // 10_000
        pp = amount * self.cp['gamma_bp'] // 10_000
        self._pay_to(pot['winner'], wp)
        self._pay_to(pot['proposer'] or pot['winner'], pp)
        r = amount - wp - pp
        counts = self.ledger[j]
        c = sum(counts.values())
        q = max(c, self.cp['kappa'] * self.cp['m'])
        paid = 0
        for o, n in counts.items():
            p = r * n // q
            paid += p
            self._pay_to(o, p)
        a = r * c // q
        self.reg.burned += a - paid                      # roundBurn
        if r == a:
            return
        if MUTANT == 'base-burn-still-burned' or self.final:
            self.reg.burned += r - a
            return
        t = h if MUTANT == 'carry-not-lagged' else h + 1  # V3-15, lagged: into pot h + 1, not the pot block h closes
        x = 0
        if MUTANT == 'base-burn-into-closed-pot' and self.pots:
            x = (r - a) // 2
            self.pots[min(self.pots)]['amount'] += x
        self.carry[t] = self.carry.get(t, 0) + r - a - x

    # ---- phase 2
    def close(self, h, tips=0):
        """V3-19/V3-15/V3-20 closeBlock(tips, issued): the host credits issued = issuance(h); the tips are already in
        Escrow (the coinbase). Returns (issued, pot amount)."""
        if h != self.height:
            raise AssertionError(('close the current block', h, self.height))
        issued = issuance(h, self.cp)
        self.issued += issued
        self.tips += tips
        share = 0 if not self.proposer or MUTANT == 'tip-share-to-pot' else tips * self.cp['gamma_tip_bp'] // 10_000
        carry = self.carry.pop(h, 0)                      # V3-15: the carry into pot h (block h - 1's settlement)
        amount = issued + (tips - share) + (0 if MUTANT == 'carry-not-added' else carry)
        self.pots[h] = {'amount': amount, 'proposer': self.proposer, 'winner': None, 'accruing': False, 'invalid': False}
        self._pay_to(self.proposer, share)               # V3-20: as the proposer part (payTo)
        return issued, amount

    def finalize(self, h, tips=0):
        """Phase 2 of a final block F: finalize(tips) burns the tips and (V3-15) every outstanding carry: the carry into
        pot F (block F - 1's settlement; no pot F exists) and the carry into F + 1 of F's own settle(F - D); then
        settleFinal(j) settles every remaining pot, whose base burn now burns (no later pot to carry it into)."""
        if h != self.height:
            raise AssertionError(('finalize the current block', h, self.height))
        self.tips += tips
        kept = {h + 1: self.carry[h + 1]} if MUTANT == 'final-keeps-own-carry' and h + 1 in self.carry else {}
        self.reg.burned += tips + sum(self.carry.values()) - sum(kept.values())
        self.carry, self.final = kept, True
        for j in sorted(self.pots):
            self._settle(j)
        return 0, 0

    def escrow_balance(self):
        reg = self.reg
        return B + sum(reg.spent.values()) + self.issued + self.tips - sum(reg.paid.values())

    def check(self, step):
        check_invariants(self.reg, step)
        reg = self.reg
        # T17 with equality: every wei Escrow holds is a pending pot, a held bond, burned, or an outstanding carry. Inside
        # block h the carries are into pots h (from block h - 1) and h + 1 (this block's settlement); after closeBlock(h)
        # only the one into h + 1, which exists only once a settlement ran (h > D); after a final block none.
        carry = 0 if MUTANT == 'invariant-missing-carry' else sum(self.carry.values())
        assert self.escrow_balance() == sum(p['amount'] for p in self.pots.values()) + reg.held_total + reg.burned + carry, step
        assert all(v > 0 for v in self.carry.values()), step
        assert set(self.carry) <= {self.height, self.height + 1}, step
        assert step[0] != 'close' or set(self.carry) <= ({self.height + 1} if self.height > self.cp['D'] else set()), step
        assert step[0] != 'final' or not self.carry, step
        assert step[0] != 'final' or not self.pots, step
        # The pending window: contiguous heights within [h-D+1, h] (pot h exists once the block is closed).
        ks = sorted(self.pots)
        assert ks == list(range(ks[0], ks[-1] + 1)) if ks else True, step
        assert all(self.height - self.cp['D'] < j <= self.height for j in ks), step

def chain_snapshot(ch):
    out = snapshot(ch.reg)
    out['pots'] = {str(j): {'amount': str(p['amount']), 'proposer': p['proposer'], 'winner': p['winner'] or '',
                            'accruing': p['accruing'], 'invalid': p['invalid']} for j, p in sorted(ch.pots.items())}
    out['escrowBalance'] = str(ch.escrow_balance())
    out['issued'] = str(ch.issued)
    out['tips'] = str(ch.tips)
    out['carry'] = {str(t): str(a) for t, a in sorted(ch.carry.items())}
    return out

def run_chain(steps, cp=None, durable=(), ledger=False):
    """Steps: ('begin', h, proposer, winner, shares, evidence), registry user operations at h, ('close', h). A begin
    that reverts invalidates the block: the state is the one before it, and the script produces height h again."""
    cp = cp or {}
    ch = Chain(cp, durable)
    out = []
    for step in steps:
        op, args = step[0], step[1:]
        if op == 'begin':
            h, proposer, winner, shares, evidence = args
            rec = {'op': ['begin', str(h), proposer], 'winner': winner or '', 'shares': list(shares),
                   'evidence': [[a, j, list(rep) if a == 'late' else rep] for a, j, rep in evidence]}
            saved = copy.deepcopy(ch)
            try:
                ch.begin(h, proposer, winner, shares, evidence)
                rec.update(ok=True, result=None)
            except Revert as e:
                ch = saved
                rec.update(ok=False, revert=str(e))
            out.append(rec)
        elif op in ('close', 'final'):
            issued, amount = (ch.close if op == 'close' else ch.finalize)(*args)
            tips = args[1] if len(args) > 1 else 0
            out.append({'op': [op, str(args[0])], 'ok': True, 'result': None, 'tips': str(tips), 'issued': str(issued),
                        'amount': str(amount)})
        else:
            if args[-1] != ch.height:
                raise AssertionError(('a user operation runs in the current block', step))
            try:
                result = getattr(ch.reg, op)(*args)
                out.append({'op': [op, *map(str, args)], 'ok': True, 'result': None if result is None else str(result)})
            except Revert as e:
                out.append({'op': [op, *map(str, args)], 'ok': False, 'revert': str(e)})
        ch.check(step)
    final = chain_snapshot(ch)
    if ledger:          # V3-02 scenarios also compare the retained ShareLedger heights (counts and totals)
        final['ledger'] = {str(j): {'total': sum(c.values()), 'counts': dict(sorted(c.items()))} for j, c in sorted(ch.ledger.items())}
    return {'cp': dict(sorted({**CHAIN_DEFAULT, **cp}.items())), 'durable': sorted(durable), 'steps': out,
            'final': final}

class Script:
    """Builds chain steps: block(h, ...) emits begin, the block's user operations and close."""
    def __init__(self):
        self.steps = []

    def block(self, h, proposer='m0', winner='m0', shares=(), evidence=(), ops=(), tips=0):
        self.invalid(h, proposer, winner, shares, evidence)
        self.steps.extend(ops)
        self.steps.append(('close', h, tips) if tips else ('close', h))
        return self

    def final(self, h, proposer='m0', winner='m0', shares=(), evidence=(), tips=0):
        """A final block: phase 0 as any block, then finalize(tips) and settleFinal of every remaining pot."""
        self.invalid(h, proposer, winner, shares, evidence)
        self.steps.append(('final', h, tips))
        return self

    def invalid(self, h, proposer='m0', winner='m0', shares=(), evidence=()):
        """A begin alone (the scenario expects it to revert, or block() completes it)."""
        ev = [(e[0], e[1], e[2] if len(e) > 2 else proposer) for e in evidence]
        self.steps.append(('begin', h, proposer, winner if h >= 2 else '', list(shares), ev))
        return self

    def quiet(self, start, end, proposer='m0'):
        for h in range(start, end + 1):
            self.block(h, proposer)
        return self

def joined(*members, h=1):
    """Block-1 registrations: (id, bond) with a self reward, or (id, bond, reward)."""
    return [('register', m[0], m[2] if len(m) > 2 else m[0], m[1], h) for m in members]

def run_chain_random(seed, blocks=64):
    """A seeded random chain: signed and fallback blocks, random winners and shares among members and strangers,
    registrations (full, partial and zero bonds), top-ups, exits, and type-1 and type-2 evidence against live
    records (never m0, so that a proposer remains), aimed at pending pots' winners often enough to reach the whole-pot
    burn. About one block in twelve first
    tries a block the program refuses (a reporter that is not the proposer, or evidence against a tomb), then makes
    the height again. Every step is checked by Chain.check."""
    rng = random.Random(seed)
    actors = [f'r{i}' for i in range(8)]
    probe = Chain({}, ['rw'])
    sc = Script()
    def play(step):
        nonlocal probe
        saved = copy.deepcopy(probe)
        op, args = step[0], step[1:]
        try:
            if op == 'begin':
                probe.begin(*args)
            elif op == 'close':
                probe.close(*args)
            else:
                getattr(probe.reg, op)(*args)
        except Revert:
            if op == 'begin':
                probe = saved
        sc.steps.append(step)
    for h in range(1, blocks + 1):
        live = [k for k, r in probe.reg.records.items() if r.live() and k != 'm0']   # m0 stays, so a proposer remains
        active = [k for k in probe.reg.active]
        proposer = '' if not active or rng.random() < 0.3 else rng.choice(active)
        pool = actors + ['m0']
        winner = rng.choice(pool) if h >= 2 else ''
        shares = [rng.choice(pool) for _ in range(rng.randint(0, 6))] if h >= 2 else []
        if h >= 3 and live and rng.random() < 0.08:
            # A block the program refuses: a mismatched reporter, or evidence against a tomb.
            tombs = [k for k, r in probe.reg.records.items() if r.tomb]
            if tombs and rng.random() < 0.5:
                bad = [(rng.choice(tombs), None, proposer)]
            else:
                bad = [(rng.choice(live), None, rng.choice([x for x in ['m0', 'r0', 'r1', ''] if x != proposer]))]
            play(('begin', h, proposer, winner, shares, bad))
        evidence = []
        if h >= 2 and rng.random() < 0.25:
            pending = [j for j in probe.pots if j < h]
            for _ in range(rng.randint(1, 2)):
                if pending and rng.random() < 0.65:
                    j = rng.choice(pending)
                    w = probe.pots[j]['winner'] if j < h - 1 else winner
                    cands = [w] if (w in live and rng.random() < 0.6) else live
                else:
                    j, cands = None, live
                cands = [c for c in cands if c not in [e[0] for e in evidence]]
                if cands:
                    evidence.append((rng.choice(cands), j, proposer))
        play(('begin', h, proposer, winner, shares, evidence))
        for _ in range(rng.randint(0, 3)):
            k = rng.random()
            a = rng.choice(actors)
            if k < 0.45:
                s = ('register', a, rng.choice([a, a, 'rw']), rng.choice([0, 0, 5 * PCOL, 15 * PCOL, B]), h)
            elif k < 0.75:
                s = ('top_up', a, rng.choice([PCOL, 6 * PCOL, B]), h)
            else:
                s = ('deregister', a, h)
            play(s)
        play(('close', h))
    return run_chain(sc.steps, {}, ['rw'])

def evidence_vectors():
    v = {}
    # Fails on the original: type-2 evidence against a HELD winner before settle(j) (at j+1 and at j+2). Pot 3 and pot 5
    # settle normally: the tombed winner's winner part (and own shares) burn, the signed proposer of pot 3 and the
    # contributors are paid; the reporter (the carrying block's proposer) receives rho = 25% of the 20 PCOL bond.
    sc = Script().block(1, ops=joined(('w', B), ('c', B), ('r', B), ('p', B), ('v', B)))
    sc.block(2).block(3, proposer='p')
    sc.block(4, proposer='r', winner='w', shares=['c', 'c', 'r', 'w'], evidence=[('w', 3)])
    sc.block(5, proposer='p')
    sc.block(6, proposer='m0', winner='v', shares=['c', 'v', 'r'])
    sc.block(7, proposer='m0', evidence=[('v', 5)])
    sc.quiet(8, 10)
    v['type2-held-winner-pays-contributors'] = run_chain(sc.steps)
    # The whole pot burns when the accused winner was ACCRUING when its result was recorded: a zero bond (evidence in
    # j+1, before the same block's punish could change the record) and a partial bond (evidence in j+2).
    sc = Script().block(1, ops=joined(('a', 0), ('q', 5 * PCOL), ('c', B)))
    sc.block(2).block(3, proposer='c')
    sc.block(4, winner='a', shares=['c', 'c', 'a'], evidence=[('a', 3)])
    sc.block(5, proposer='c')
    sc.block(6, winner='q', shares=['c', 'q'])
    sc.block(7, proposer='c', evidence=[('q', 5)])
    sc.quiet(8, 10)
    v['type2-accruing-winner-burns-whole-pot'] = run_chain(sc.steps)
    # The ACCRUING fact is the one committed by recordResult, never read later: q is ACCRUING when pot 3's result is
    # recorded (phase 0 of block 4), tops up to B in block 4 (HELD), and is accused in block 5; the whole pot burns.
    sc = Script().block(1, ops=joined(('q', 15 * PCOL), ('c', B)))
    sc.block(2).block(3)
    sc.block(4, winner='q', shares=['c'], ops=[('top_up', 'q', 5 * PCOL, 4)])
    sc.block(5, proposer='c', evidence=[('q', 3)])
    sc.quiet(6, 9)
    v['accruing-fact-committed-at-record-result'] = run_chain(sc.steps)
    # P7: evidence against members of pot 3 other than its winner settles it normally. The ACCRUING share owner a and
    # the signed proposer p of pot 3 are both accused (two pieces of evidence for one pot in one block): a's shares and
    # p's proposer part burn, the HELD winner w and the contributor c are paid.
    sc = Script().block(1, ops=joined(('w', B), ('c', B), ('p', B), ('a', 0)))
    sc.block(2).block(3, proposer='p')
    sc.block(4, winner='w', shares=['a', 'a', 'c'], evidence=[('a', 3), ('p', 3)])
    sc.quiet(5, 8)
    v['non-winner-accused-settles-normally'] = run_chain(sc.steps)
    # The settlement boundary: evidence in block j+D still precedes settle(j) in that phase 0 (the ACCRUING winner's
    # whole pot burns); in block j+D+1 the pot is already settled (the winner part accrued into the winner's bond), so
    # only the bond burns, with the accrued part, and rho of it goes to the reporter.
    sc = Script().block(1, ops=joined(('q', 5 * PCOL), ('y', 5 * PCOL), ('c', B)))
    sc.block(2).block(3).block(4, winner='q', shares=['c']).block(5, winner='y', shares=['c', 'y'])
    sc.block(6).block(7, proposer='c', evidence=[('q', 3)])
    sc.block(8).block(9, proposer='c', evidence=[('y', 4)])
    sc.quiet(10, 12)
    v['evidence-at-the-settlement-boundary'] = run_chain(sc.steps)
    # V3-17 self-report: proposer r reports itself (type 1) and c in the same block. r loses its bond but receives rho of
    # it and of c's at its reward address rr: a self-report loses (1 - rho) * B.
    sc = Script().block(1, ops=joined(('r', B, 'rr'), ('c', B)))
    sc.quiet(2, 3)
    sc.block(4, proposer='r', evidence=[('r', None), ('c', None)])
    sc.quiet(5, 8)
    v['self-report-loses-three-quarters'] = run_chain(sc.steps, durable=['rr'])
    # V3-17 on a fallback block (no reporter): the whole bond burns. A reporter on a fallback block, a reporter that is
    # not the signed proposer and evidence against a tomb are refused (the block is invalid; the height is made again).
    sc = Script().block(1, ops=joined(('w', B), ('r', B), ('c', B)))
    sc.block(2).block(3)
    sc.invalid(4, proposer='', winner='w', shares=['c'], evidence=[('w', 3, 'r')])
    sc.invalid(4, proposer='c', winner='w', shares=['c'], evidence=[('w', 3, 'r')])
    sc.block(4, proposer='', winner='w', shares=['c'], evidence=[('w', 3)])
    sc.invalid(5, proposer='r', evidence=[('w', None)])
    sc.block(5, proposer='r')
    sc.quiet(6, 8)
    v['fallback-has-no-reporter'] = run_chain(sc.steps)
    # V3-17: the cut is paid to the reporter's reward address directly, also when the reporter is ACCRUING (its bond is
    # not filled from it): q (5 PCOL on probation) reports w, then m0 reports c.
    sc = Script().block(1, ops=joined(('q', 5 * PCOL), ('w', B), ('c', B)))
    sc.block(2, proposer='q', evidence=[('w', None)])
    sc.block(3, evidence=[('c', None)])
    sc.quiet(4, 7)
    v['reporter-cut-paid-directly'] = run_chain(sc.steps)
    for seed in (1, 2):
        v[f'random-{seed}'] = run_chain_random(seed)
    return v

# ---------------- V3-02: late share credit (V3-PR-E3)
def late(j, *owners):
    """A kind-3 evidence effect as the host passes it: recordLate(j, owners), one owner per late share."""
    return ('late', j, list(owners))

def run_late_random(seed, blocks=48):
    """A seeded random chain with late credit: random winners and header shares among members and strangers, and in
    about one block in three late credits for random ancestors in [max(1, h-D), h-1], within the no-dilution cap
    (kappa*m = 8 here), sometimes several items for one template in one block, mixed with type-1 evidence; about one
    block in ten first tries a late credit the program refuses (past the cap, outside the window), then makes the
    height again. Every step is checked by Chain.check (the Escrow invariant with equality)."""
    rng = random.Random(seed)
    cp = {'m': 4}
    pool = ['m0', 'w', 'b', 'c', 'a', 'x', 'stranger']
    sc = Script().block(1, ops=joined(('w', B), ('b', B), ('c', B), ('a', 0), ('x', 5 * PCOL)))
    probe = Chain(cp)
    for step in sc.steps:
        op, args = step[0], step[1:]
        if op == 'begin':
            probe.begin(*args)
        elif op == 'close':
            probe.close(*args)
        else:
            getattr(probe.reg, op)(*args)
    km = probe.cp['kappa'] * probe.cp['m']
    tombed = set()
    for h in range(2, blocks + 1):
        winner = rng.choice(pool[:5])
        shares = [rng.choice(pool) for _ in range(rng.randint(0, 9))]
        lo = max(1, h - probe.cp['D'])
        evidence = []
        if h >= 3 and rng.random() < 0.1:
            j = rng.choice([lo - 1, h]) if lo > 1 else h
            bad = [late(max(j, 1) if j >= 1 else h, rng.choice(pool))] if rng.random() < 0.5 else []
            if not bad:
                # past the cap on a recorded height
                jj = rng.randrange(lo, h)
                room = km - (sum(probe.ledger[jj].values()) if jj in probe.ledger else len(shares) + 1)
                bad = [late(jj, *[rng.choice(pool) for _ in range(max(room, 0) + 1)])]
            sc.invalid(h, winner=winner, shares=shares, evidence=bad)
            saved = copy.deepcopy(probe)
            try:
                probe.begin(h, 'm0', winner, shares, [(e[0], e[1], e[2]) for e in bad])
                raise AssertionError(('a refused late credit was accepted', seed, h, bad))
            except Revert:
                probe = saved
        if h >= 3 and rng.random() < 0.35:
            for _ in range(rng.randint(1, 3)):
                j = rng.randrange(lo, h)
                used = (sum(probe.ledger[j].values()) if j in probe.ledger else len(shares) + 1) if j < h - 1 else len(shares) + 1
                used += sum(len(e[2]) for e in evidence if e[0] == 'late' and e[1] == j)
                room = km - used
                if room > 0:
                    evidence.append(late(j, *[rng.choice(pool) for _ in range(rng.randint(1, room))]))
        if h >= 4 and rng.random() < 0.08:
            live = [k for k, r in probe.reg.records.items() if r.live() and k not in ('m0',) and k not in tombed]
            if live:
                t = rng.choice(live)
                tombed.add(t)
                evidence.insert(rng.randint(0, len(evidence)), (t, None, 'm0'))
        sc.block(h, winner=winner, shares=shares, evidence=evidence)
        ev = [(e[0], e[1], e[2] if len(e) > 2 else 'm0') for e in evidence]
        probe.begin(h, 'm0', winner, shares, ev)
        probe.close(h)
    return run_chain(sc.steps, cp, ledger=True)

def late_vectors():
    """Script.block(h, winner, shares) names block h-1's result (recorded in phase 0 of h, before h's evidence)."""
    v = {}
    # Fails on the original: the winner w of block 4 omits b's share (header shares: c only). Block 5 carries LS(4)
    # crediting b; settle(4) at h = 8 pays b its share of the pool, and b's lastShare is 4.
    sc = Script().block(1, ops=joined(('w', B), ('b', B), ('c', B)))
    sc.quiet(2, 4)
    sc.block(5, winner='w', shares=['c'], evidence=[late(4, 'b')])
    sc.quiet(6, 9)
    v['late-share-pays-the-omitted-contributor'] = run_chain(sc.steps, ledger=True)
    # The settlement boundary: a carrier at h = j+D still credits j (recordLate runs before settle(h-D) in the same
    # phase 0); at h = j+D+1 the pot is settled and the window refuses it (BadPhase), as it does j = h and j = 0.
    sc = Script().block(1, ops=joined(('w', B), ('b', B), ('c', B)))
    sc.quiet(2, 4)
    sc.block(5, winner='w', shares=['c'])
    sc.quiet(6, 7)
    sc.block(8, evidence=[late(4, 'b', 'b')])
    sc.invalid(9, evidence=[late(4, 'b')])
    sc.invalid(9, evidence=[late(9, 'b')])
    sc.invalid(9, evidence=[late(0, 'b')])
    sc.block(9)
    sc.quiet(10, 13)
    v['late-share-at-the-settlement-boundary'] = run_chain(sc.steps, ledger=True)
    # lastShare = max(lastShare, j), never a revert: b won block 5 (recorded in block 6's phase 0, lastShare 5) before
    # block 6 credits b's late share of block 4; d had no share, so its lastShare becomes 4. Likewise c (header share
    # of block 6) keeps lastShare 6 when block 8 credits c's late share of block 5.
    sc = Script().block(1, ops=joined(('w', B), ('b', B), ('c', B), ('d', B)))
    sc.quiet(2, 4)
    sc.block(5, winner='w', shares=['c'])
    sc.block(6, winner='b', evidence=[late(4, 'b', 'd')])
    sc.block(7, winner='c', shares=['c'])
    sc.block(8, evidence=[late(5, 'c')])
    sc.quiet(9, 12)
    v['late-last-share-is-monotone'] = run_chain(sc.steps, ledger=True)
    # The no-dilution cap at kappa*m = 8: block 4's header holds 6 shares (5 + the winner), two items in block 5 fill
    # it exactly (q stays kappa*m, so no header share loses value); a third credit in block 6 is refused (BadState).
    # Block 6's header alone reaches the cap, so no late credit for it fits.
    sc = Script().block(1, ops=joined(('w', B), ('b', B), ('c', B), ('d', B)))
    sc.quiet(2, 4)
    sc.block(5, winner='w', shares=['c'] * 5, evidence=[late(4, 'b'), late(4, 'd')])
    sc.invalid(6, evidence=[late(4, 'd')])
    sc.block(6)
    sc.invalid(7, winner='w', shares=['c'] * 7, evidence=[late(6, 'b')])
    sc.block(7, winner='w', shares=['c'] * 7)
    sc.quiet(8, 12)
    v['late-share-cap-fills-only-the-uncovered-pool'] = run_chain(sc.steps, {'m': 4}, ledger=True)
    # Owners as recordShares treats them: a stranger without a record is counted and its part burns (payBurn); an owner
    # punished earlier in the carrying block is counted and its part burns (penaltyBurn); a probationer's part fills
    # its bond (EARN-IN P1). Several late items for different templates in one block.
    sc = Script().block(1, ops=joined(('w', B), ('t', B), ('a', 0), ('c', B)))
    sc.quiet(2, 4)
    sc.block(5, winner='w', shares=['c'])
    sc.block(6, winner='w', shares=['c'], evidence=[late(4, 'stranger', 'a'), ('t', None), late(5, 't', 'a', 'a')])
    sc.quiet(7, 10)
    v['late-share-owners-absent-tomb-and-accruing'] = run_chain(sc.steps, ledger=True)
    for seed in (1, 2):
        v[f'late-random-{seed}'] = run_late_random(seed)
    return v

# ---------------- V3-19 / V3-15 / V3-20: issuance, carry and tip split (V3-PR-E2a)
OWNER_S0, OWNER_H_HALF = PCOL, 4_207_680             # V3-ROADMAP §5: S0 = 1 PCOL, H_half = 4 years at 30 s, tail S0/32

def schedule_cases():
    """issuance(h) at the era boundaries for the owner's values, a tail-free schedule, a U256-sized S0 and a compressed
    H_half (the host's issuance.rs and issuance_oracle.py --v3 replay this table)."""
    rows = []
    def add(label, s0, hh, tail, hs):
        cp = {'S0': s0, 'H_half': hh, 'S_tail': tail}
        for h in hs:
            rows.append({'label': label, 'h': str(h), 'S0': str(s0), 'H_half': str(hh), 'S_tail': str(tail),
                         'issued': str(issuance(h, cp))})
    H = OWNER_H_HALF
    add('owner', OWNER_S0, H, OWNER_S0 // 32, [0, 1, H - 1, H, H + 1, 2 * H - 1, 2 * H, 4 * H, 5 * H - 1, 5 * H, 6 * H,
                                                255 * H, 256 * H - 1, 256 * H, 256 * H + 1, 2**64 - 1])
    add('no-tail', OWNER_S0, H, 0, [1, H, 59 * H, 60 * H, 256 * H])
    add('u256-s0', 2**256 - 1, 1, 0, [1, 2, 255, 256, 257, 2**64 - 1])
    add('compressed', 2 * PCOL, 4, 2 * PCOL // 32, list(range(0, 30)))
    return rows

def run_issuance_chain(seed, blocks, cp, late_p=0.3, tip_p=0.4, c_cycle=None):
    """A seeded chain for the carry: header share counts (C, winner included), late credits within the cap, tips in
    signed and fallback blocks, sometimes a probationer or a tombed proposer. c_cycle gives C(h-1) per block."""
    rng = random.Random(seed)
    pool = ['m0', 'w', 'b', 'c', 'a', 'stranger']
    sc = Script().block(1, ops=joined(('w', B), ('b', B), ('c', B), ('a', 5 * PCOL)))
    probe = Chain(cp)
    for st in sc.steps:
        op, args = st[0], st[1:]
        (probe.begin if op == 'begin' else probe.close if op == 'close' else getattr(probe.reg, op))(*args)
    km = probe.cp['kappa'] * probe.cp['m']
    for h in range(2, blocks + 1):
        c = c_cycle[(h - 2) % len(c_cycle)] if c_cycle else rng.randint(1, probe.cp['S_max'] + 1)
        winner = rng.choice(pool[:5])
        shares = [rng.choice(pool) for _ in range(c - 1)]
        proposer = '' if rng.random() < 0.3 else rng.choice(['m0', 'w', 'a'])
        lo = max(1, h - probe.cp['D'])
        evidence = []
        if h >= 3 and proposer and rng.random() < late_p:
            for _ in range(rng.randint(1, 2)):
                j = rng.randrange(lo, h)
                used = sum(probe.ledger[j].values()) if j < h - 1 else len(shares) + 1
                used += sum(len(e[2]) for e in evidence if e[0] == 'late' and e[1] == j)
                if km - used > 0:
                    evidence.append(late(j, *[rng.choice(pool) for _ in range(rng.randint(1, km - used))]))
        tips = rng.choice([1, 3, 12_345, 10**15 + 1, 7 * PCOL]) if rng.random() < tip_p else 0
        sc.block(h, proposer=proposer, winner=winner, shares=shares, evidence=evidence, tips=tips)
        probe.begin(h, proposer, winner, shares, [(e[0], e[1], e[2] if len(e) > 2 else proposer) for e in evidence])
        probe.close(h, tips)
    return sc.steps

def pots_at_close(steps, cp):
    """For every h: (pot h's amount as closeBlock(h) recorded it, the amount settle(h) distributed, or the amount still
    pending at the chain's end when settle(h) has not run). The second is what V3-15 fixes: a design that adds value to
    an already-closed pot later (safety finding 8's rejected shape) changes it, never the first."""
    ch, out = Chain(cp), {}
    for st in steps:
        op, args = st[0], st[1:]
        if op == 'begin':
            ch.begin(*args)
        elif op == 'close':
            out[args[0]] = ch.close(*args)[1]
        else:
            getattr(ch.reg, op)(*args)
    return {h: (a, ch.settled[h] if h in ch.settled else ch.pots[h]['amount'], h in ch.settled) for h, a in out.items()}

def late_inclusion_variant(steps, after, rng):
    """The same chain with the late credits of every block after `after` changed: each late item dropped with
    probability 0.7 (fewer credits never makes a block invalid)."""
    out = []
    for st in steps:
        if st[0] == 'begin' and st[1] > after and any(e[0] == 'late' for e in st[5]):
            ev = [e for e in st[5] if e[0] != 'late' or rng.random() < 0.3]
            st = st[:5] + (ev,)
        out.append(st)
    return out

def check_pot_fixed_by_its_block(seed=7, blocks=40):
    """V3-15 test (V3-DESIGN.md): changing LS inclusion after block j+1 (block j+1 fixed) never changes pot j+1, nor any
    pot up to j+1: pot h depends only on blocks 1..h (its carry, lagged by one block, on blocks 1..h-1). Compared is both
    the amount closeBlock(h) recorded and the amount
    settle(h) distributed (or still pending at the end), so value added to a closed pot after its block fails the check.
    Positive controls: some later pot does change (the carry of settle(j') follows the late credits to j', which its
    carriers up to the settling block commit), and the compared pots include settled ones."""
    cp = {'m': 4, 'S_max': 16}
    base = run_issuance_chain(seed, blocks, cp, late_p=0.8)
    pots = pots_at_close(base, cp)
    rng = random.Random(seed)
    changed_later = settled_compared = 0
    for j in range(1, blocks - 1):
        variant = late_inclusion_variant(base, j + 1, rng)
        if variant == base:
            continue
        other = pots_at_close(variant, cp)
        assert all(other[h] == pots[h] for h in range(1, j + 2)), ('pot fixed by its block', j)
        settled_compared += sum(pots[h][2] for h in range(1, j + 2))
        changed_later += any(other[h][0] != pots[h][0] for h in range(j + 2, blocks + 1))
    assert changed_later > 0, 'the late credits never moved a later pot: the check tests nothing'
    assert settled_compared > 0, 'no compared pot was settled: the settle-time amount is not tested'
    return changed_later

def fixedness_mutant_killed():
    """check_pot_fixed_by_its_block fails on base-burn-into-closed-pot (value moved into an already-closed pot at settle
    time, which a close-time comparison alone cannot see)."""
    global MUTANT
    MUTANT = 'base-burn-into-closed-pot'
    try:
        check_pot_fixed_by_its_block()
    except AssertionError as e:
        return str(e.args[0] if e.args else e)
    finally:
        MUTANT = None
    raise AssertionError('base-burn-into-closed-pot survives check_pot_fixed_by_its_block')

LAST_J = 2                                           # the template of the last-carrier scenario (D = 4: h = 6)

def last_carrier_steps(include, blocks=12):
    """D-V3-15 incentive scenario: template j = 2 has C = 2 (winner w and one header share of w), and x holds eight valid
    shares of it that the header left out. Block h = j + D = 6, signed by a, is the last block whose window [h - D, h - 1]
    still holds j; it either carries late(2, x * 8) (include) or omits them. a has no other part in any pot (it is no
    winner and has no share), so everything a is paid comes from pot 6's proposer part."""
    sc = Script().block(1, ops=joined(('a', B), ('x', B), ('w', B)))
    for h in range(2, blocks + 1):
        ev = [late(LAST_J, *['x'] * 8)] if include and h == LAST_J + 4 else []
        sc.block(h, proposer='a' if h == LAST_J + 4 else 'm0', winner='w', shares=['w'] if h == LAST_J + 1 else [],
                 evidence=ev)
    return sc.steps

def play(steps, cp=None):
    """Runs chain steps (as run_chain, without a revert); returns the Chain and pot h as closeBlock(h) recorded it."""
    ch, closed = Chain(cp or {}), {}
    for st in steps:
        op, args = st[0], st[1:]
        if op == 'begin':
            ch.begin(*args)
        elif op == 'close':
            closed[args[0]] = ch.close(*args)[1]
        else:
            getattr(ch.reg, op)(*args)
        ch.check(st)
    return ch, closed

def base_burn_of(amount, c, cp):
    """settle's base burn R - floor(R*C/Q) of a pot of `amount` whose template holds C shares (Q = max(C, kappa*m))."""
    r = amount - amount * cp['alpha_bp'] // 10_000 - amount * cp['gamma_bp'] // 10_000
    q = max(c, cp['kappa'] * cp['m'])
    return r - r * c // q

def check_last_carrier_gains_nothing():
    """D-V3-15 (lagged carry) test, fails on the previous rule: the signed proposer of h = j + D, the last carrier of
    late shares of j, gains nothing in its own pot by omitting LS(j). Pot h is the same with and without the omission,
    as closeBlock(h) recorded it and as settle(h) distributed it, and so is everything a is paid; the omission moves
    exactly the extra base burn of settle(j) into pot h + 1 (pot h + 1 = issued + tips - share + base burn of settle(j)
    in both runs), whose template can no longer credit j, while x loses its eight late shares."""
    h = LAST_J + 4
    runs = {}
    for include in (True, False):
        ch, closed = play(last_carrier_steps(include))
        cp = ch.cp
        assert cp['D'] == 4 and LAST_J + cp['D'] == h, 'block h is the last carrier of j'
        runs[include] = (closed, ch.settled, ch.reg.paid.get('a', 0), ch.reg.paid.get('x', 0))
    (ci, si, ai, xi), (co, so, ao, xo) = runs[True], runs[False]
    assert ci[h] == co[h], ('pot h as closeBlock(h) recorded it', ci[h], co[h])
    assert si[h] == so[h], ('pot h as settle(h) distributed it', si[h], so[h])
    assert ai == ao, ("the last carrier's payments", ai, ao)
    cp = Chain({}).cp
    a2 = si[LAST_J]
    assert a2 == so[LAST_J], 'pot j is the same in both runs'
    bi, bo = base_burn_of(a2, 2 + 8, cp), base_burn_of(a2, 2, cp)   # C = 2 header (w twice) + 8 late, or 2
    assert bo > bi > 0, ('the omission raises the base burn of settle(j)', bi, bo)
    tips_share_issued = issuance(h + 1, cp)              # block h + 1 has no tips
    assert ci[h + 1] == tips_share_issued + bi and co[h + 1] == tips_share_issued + bo, ('the carry of settle(j) is in pot h + 1', ci[h + 1], co[h + 1])
    assert all(ci[k] == co[k] and si[k] == so[k] for k in range(1, h + 1)), 'no pot up to h differs'
    assert xi > xo, 'x is paid its late shares only when they are carried'
    return {'pot h': ci[h], 'pot h+1 (include, omit)': (ci[h + 1], co[h + 1]), 'a paid': ai, 'x loses': xi - xo}

def lag_mutant_killed():
    """check_last_carrier_gains_nothing fails on carry-not-lagged (the previous rule: the omitted value enters pot h, of
    which h's signed proposer takes gamma_bp/10^4)."""
    global MUTANT
    MUTANT = 'carry-not-lagged'
    try:
        check_last_carrier_gains_nothing()
    except AssertionError as e:
        return str(e.args[0] if e.args else e)
    finally:
        MUTANT = None
    raise AssertionError('carry-not-lagged survives check_last_carrier_gains_nothing')

def issuance_vectors():
    v = {'schedule': {'cases': schedule_cases()}}
    # Compressed fixture (H_half = 4, S_tail = S0/32): blocks over 6·H_half reach the tail; host credit = recorded pot
    # every block (tests/v3_contracts.rs replays the chain and the preview test the real host).
    cp = {'H_half': 4}
    sc = Script().block(1, ops=joined(('w', B), ('c', B)))
    for h in range(2, 27):
        sc.block(h, proposer='' if h % 5 == 0 else 'm0', winner='w' if h % 2 else 'm0', shares=['c'] * (h % 4),
                 tips=12_345 if h % 3 == 0 else 0)
    v['issuance-halving-compressed'] = run_chain(sc.steps, cp)
    # V3-20 tip split: a HELD signed proposer (odd tips: floor for the proposer, the rest to the pot), a fallback block
    # (all tips to the pot), a probationer proposer (its share fills its bond first; a large tip promotes it), and a
    # proposer that reports itself in its own block (a tomb: its share burns, penaltyBurn).
    sc = Script().block(1, ops=joined(('p', B, 'pr'), ('q', 15 * PCOL), ('s', B), ('c', B)))
    sc.block(2, proposer='p', tips=12_345)
    sc.block(3, proposer='', tips=10**15 + 1)
    sc.block(4, proposer='q', winner='c', tips=2 * PCOL + 1)
    sc.block(5, proposer='q', winner='c', shares=['c'], tips=9 * PCOL)
    sc.block(6, proposer='s', evidence=[('s', None)], tips=4 * PCOL)
    sc.block(7, proposer='p', tips=1)
    sc.quiet(8, 12)
    v['tip-split-signed-fallback-accruing-tomb'] = run_chain(sc.steps, durable=['pr'])
    # Fuzz C from 1 to S_max + 1 (S_max 16, kappa*m 8: Q = max(C, 8), so C >= 8 leaves no base burn) with late credits
    # filling the uncovered pool, and tips.
    cp = {'m': 4, 'S_max': 16}
    v['carry-fuzz-c-1-to-smax-plus-1'] = run_chain(run_issuance_chain(3, 40, cp, c_cycle=list(range(1, 18))), cp, ledger=True)
    v['carry-random-late-and-tips'] = run_chain(run_issuance_chain(5, 40, cp), cp, ledger=True)
    # Pot j+1 is fixed by block j+1: the same chain with the late credits of blocks after j+1 dropped (j = 6) records
    # the same pots up to j+1, and a different one at a later height (tests/v3_contracts.rs compares both runs).
    base = run_issuance_chain(11, 20, cp, late_p=0.9)
    v['late-inclusion-base'] = run_chain(base, cp, ledger=True)
    v['late-inclusion-after-7-dropped'] = run_chain(late_inclusion_variant(base, 7, random.Random(0)), cp, ledger=True)
    # A final block F = 9: the carry into pot 9 (settle(4) in block 8) is outstanding, phase 0's settle(5) adds the carry
    # into pot 10, finalize burns both with the tips (no tip share at F), and settleFinal(6..8) burns their base burns
    # directly; nothing is left pending or carried.
    sc = Script().block(1, ops=joined(('w', B), ('c', B)))
    for h in range(2, 9):
        sc.block(h, proposer='m0' if h % 2 else '', winner='w', shares=['c'] * (h % 3), tips=5 * PCOL if h == 6 else 0)
    sc.final(9, winner='w', shares=['c'], tips=3 * PCOL + 1)
    v['final-block-burns-the-carry'] = run_chain(sc.steps)
    # D-V3-15: the last carrier of template 2 (block 6) carries x's eight late shares of it, or omits them
    # (tests/v3_contracts.rs last_carrier_gains_nothing_by_omitting_late_shares compares pots 6 and 7 of both runs).
    v['last-carrier-includes-late'] = run_chain(last_carrier_steps(True))
    v['last-carrier-omits-late'] = run_chain(last_carrier_steps(False))
    return v

def econ_mutants_killed():
    """Each ECON_MUTANT makes issuance_vectors() raise (an Escrow check) or differ from the unmutated vectors."""
    global MUTANT
    want = json.dumps(issuance_vectors(), sort_keys=True)
    out = {}
    for m in ECON_MUTANTS:
        MUTANT = m
        try:
            got = json.dumps(issuance_vectors(), sort_keys=True)
            out[m] = 'differs' if got != want else None
        except AssertionError:
            out[m] = 'escrow check'
        finally:
            MUTANT = None
        assert out[m], f'economic mutant {m} survives tests/v3/issuance-carry-vectors.json'
    return out

# ---------------- V3-10 / V3-14: contestable seats and the probation-pool contest (V3-PR-E4)
# The contest scenarios' CP: an 8-block epoch (D = 4: contests open at h mod 8 >= 3), four seats, two probation slots,
# theta = 1 and k_sigma = 1 (E = Tprev/active, threshold ceil(E - sqrt(E))), short grace; scenarios override these.
CONTEST_CP = {'E_epoch': 8, 'Theta_inact': 4, 'M_max': 4, 'P_MAX': 2, 'theta_bp': 10_000, 'k_sigma': 1}
WORKERS = ('m0', 'a', 'b')

def contest_status(victim, h, probation=False):
    return ('contest_status', victim, 'true' if probation else 'false', h)

def contest(sender, victim, h, value=B, reward=None):
    return ('register_contest', sender, reward or sender, victim, value, h)

def busy(h, extra=()):
    """Block h's parent result: the workers take turns winning, each with one header share, plus `extra` shares."""
    return {'winner': WORKERS[h % 3], 'shares': list(WORKERS) + list(extra)}

def contest_chain(last, ops=None, members=(('a', B), ('b', B), ('c', B)), late=None, extra=None, cp=None, proposer='m0',
                  durable=(), joins=None, winners=None):
    """Blocks 1..last: `members` join at block 1 (and `joins[h]` at h), every block names busy(h) (plus extra(h) shares;
    winners[h], if given, as the winner of template h - 1), with `late(h)` evidence and `ops[h]` user operations."""
    ops, joins, winners = ops or {}, joins or {}, winners or {}
    sc = Script().block(1, ops=joined(*members) + list(ops.get(1, [])))
    for h in range(2, last + 1):
        r = busy(h, extra(h) if extra else ())
        sc.block(h, proposer=proposer, winner=winners.get(h, r['winner']), shares=r['shares'], evidence=late(h) if late else (),
                 ops=list(joined(*joins[h], h=h) if h in joins else []) + list(ops.get(h, [])))
    return run_chain(sc.steps, {**CONTEST_CP, **(cp or {})}, durable=durable, ledger=False)

def threshold_table():
    """contestThreshold(Tprev, active) at chosen theta_bp and k_sigma: the integer threshold and isqrt(k^2*N*Dn). Rows
    cover the owner's values (E = 45 at 256 seats: about 25), k_sigma 0 and theta 0, a single seat, large Tprev, and
    k^2*N*Dn a perfect square, one below one (where an unfloored Newton step returns the root plus one) and huge. Each row is
    checked exactly against the reals (below_exact)."""
    rows = []
    def add(label, tprev, active, theta_bp, k_sigma):
        thr = threshold(tprev, active, theta_bp, k_sigma)
        s = math.isqrt(k_sigma * k_sigma * theta_bp * tprev * 10_000 * active)
        assert thr == 0 or below_exact(thr - 1, tprev, active, theta_bp, k_sigma), label
        assert not below_exact(thr, tprev, active, theta_bp, k_sigma), label
        rows.append({'label': label, 'tprev': str(tprev), 'active': active, 'theta_bp': theta_bp, 'k_sigma': k_sigma,
                     'threshold': str(thr), 'root': str(s)})
    add('owner-e45-256-seats', 45 * 256 * 8, 256, 1250, 3)        # V3-DESIGN.md: E = 45, threshold about 25
    add('owner-e44-256-seats', 44 * 256 * 8, 256, 1250, 3)
    add('owner-quiet-day', 2880 * 64, 256, 1250, 3)                # kappa*m shares per template for a day
    add('owner-full-day', 2880 * 257, 256, 1250, 3)                # S_max + 1 shares per template for a day
    add('owner-small-e', 9 * 256 * 8, 256, 1250, 3)                # E = 9 = (3*sqrt(9))... threshold 0
    add('owner-e-just-above-9', 9 * 256 * 8 + 1, 256, 1250, 3)
    add('no-margin', 1000, 7, 10_000, 0)                           # k_sigma 0: ceil(E)
    add('no-margin-exact', 700, 7, 10_000, 0)
    add('theta-zero', 10**9, 4, 0, 3)
    add('tprev-zero', 0, 4, 1250, 3)
    add('one-seat', 1000, 1, 10_000, 1)
    add('contest-cp-38', 38, 4, 10_000, 1)                         # the threshold-boundary scenarios
    add('contest-cp-39', 39, 4, 10_000, 1)
    for q in (1, 7, 2**20 + 3, 2**40 + 1):
        # theta_bp 1, one seat, k_sigma 1: k^2*N*Dn = 10^4*Tprev. Tprev = 10^4*q^2 makes it the square of 10^4*q, and
        # Tprev = 10^4*q^2 - 2q makes it (10^4*q - 1)^2 - 1, one below a square.
        add(f'square-{q}', 10_000 * q * q, 1, 1, 1)
        add(f'square-minus-one-{q}', 10_000 * q * q - 2 * q, 1, 1, 1)
        add(f'square-plus-{q}', 10_000 * q * q + 1, 1, 1, 1)
    add('huge', 2**90 + 12345, 65535, 10_000, 65535)
    add('huge-square-minus-one', 10_000 * (2**43) ** 2 - 2 * 2**43, 1, 1, 1)
    # Sensitivity: an isqrt one off moves the threshold in some row (beyond the root the table states).
    moved = [r['label'] for r in rows for dlt in (-1, 1) if int(r['root']) + dlt >= 0 and int(r['threshold']) > 0 and
             -((int(r['root']) + dlt - r['theta_bp'] * int(r['tprev'])) // (10_000 * r['active'])) != int(r['threshold'])]
    assert moved, 'no row is sensitive to the integer square root'
    return {'rows': rows, 'sensitive': sorted(set(moved))}

def contest_vectors():
    v = {'threshold-table': threshold_table()}
    # V3-10, the main test: a full set (m0, a, b, c; M_max 4), c idle. Epoch 1 (templates 8..15) holds Tprev = 8*4 = 32
    # shares, so E = 32/4 = 8 and the threshold is ceil(8 - sqrt(8)) = 6; c has 0. Contests open at h = 19 (19 mod 8 = 3 =
    # D - 1: the late window of template 15 closed). register reverts Capacity; registerContest(n, c) with the full bond
    # exits c (refund after U = 40, by the sweep at h = 59) and registers n.
    ops = {5: [contest_status('c', 5), contest('n', 'c', 5)],
           16: [contest_status('c', 16)], 18: [contest_status('c', 18), contest('n', 'c', 18)],
           19: [contest_status('c', 19), ('register', 'n', 'n', B, 19), contest('n', 'c', 19), contest_status('c', 19),
                contest_status('n', 19)],
           58: [('claim_bond', 'c', 58)], 60: [('claim_bond', 'c', 60)]}
    v['seat-contest-below-threshold-refund-after-u'] = contest_chain(61, ops)
    # Reverts: a member above the threshold (a), the sender already registered (a), an exited victim, the set not full
    # (b exits: active 3 < M_max), then x fills it and y's contest of c lands.
    ops = {19: [contest_status('a', 19), contest('n', 'a', 19), contest('a', 'c', 19), ('deregister', 'b', 19),
                contest_status('c', 19), contest('n', 'c', 19), contest('n', 'b', 19), ('register', 'x', 'x', B, 19),
                contest('y', 'b', 19), contest('y', 'c', 19)]}
    v['seat-contest-refusals'] = contest_chain(24, ops)
    # The threshold, off by one: the victim v holds one share in each of templates 8..13 (6 shares; Tprev 38, threshold
    # ceil(9.5 - sqrt(9.5)) = 7: contestable) or 8..14 (7 shares; Tprev 39, threshold 7: not contestable).
    for n_ in (6, 7):
        ops = {19: [contest_status('v', 19), contest('n', 'v', 19)]}
        v[f'threshold-boundary-{n_}-shares'] = contest_chain(
            22, ops, members=(('a', B), ('b', B), ('v', B)), extra=lambda h, n_=n_: ['v'] if 9 <= h <= 8 + n_ else [])
    # In grace: c joins at block 8 (the first block of epoch 1, allowed) with Theta_inact 16, so it is in grace until 24;
    # at 19..23 the contest reverts (code 5) although c's count is 0; at 27 (epoch 2 measured) it lands.
    ops = {19: [contest_status('c', 19), contest('n', 'c', 19)], 23: [contest('n', 'c', 23)],
           27: [contest_status('c', 27), contest('n', 'c', 27)]}
    v['contest-in-grace'] = contest_chain(30, ops, members=(('a', B), ('b', B)), joins={8: [('c', B)]},
                                          cp={'Theta_inact': 16})
    # Joined or reactivated after the first block of the measured epoch (code 6): five seats; c joins at 8 (allowed), y at
    # 9 (one block late); r exits at 9 and reactivates at 11 (W_act 2, U 6, C_react 6), so its graceStart is 11. At 19
    # (epoch 1 measured) c is contested, y and r are not; at 27 (epoch 2 measured: y and r idle since joining) both are.
    ops = {9: [('deregister', 'r', 9)], 11: [('reactivate', 'r', 11)],
           19: [contest('n1', 'c', 19), contest_status('y', 19), contest('n2', 'y', 19), contest('n2', 'r', 19)],
           27: [contest_status('y', 27), contest('n2', 'y', 27), contest('n3', 'r', 27)]}
    v['contest-joined-after-the-measured-epoch-start'] = contest_chain(
        30, ops, members=(('a', B), ('b', B), ('r', B)), joins={8: [('c', B)], 9: [('y', B)]},
        cp={'M_max': 6, 'W_act': 2, 'U': 6, 'C_react': 6})
    # Budgets (E1 interplay): a contest takes two REG_OPS units and one R_max entry (REG_OPS 3, R_max 4). Block 19: b's exit
    # (1) and x's registration (2) leave one unit, too few; block 20: the contest (2), then a deregister (3); a second
    # contest is refused. Block 21: two maintenance operations leave one unit (REG_OPS less maintenance), too few.
    ops = {19: [('deregister', 'b', 19), ('register', 'x', 'x', B, 19), contest('n', 'c', 19)],
           20: [contest('n', 'c', 20), ('deregister', 'a', 20), contest('n2', 'x', 20)],
           21: [('set_reward', 'm0', 'm0', 21), ('set_reward', 'n', 'n', 21), contest('n2', 'x', 21)],
           22: [contest_status('x', 22), contest('n2', 'x', 22)]}
    v['contest-budget'] = contest_chain(24, ops, cp={'REG_OPS': 3, 'R_max': 4})
    # V3-14, the pool contest: six seats, two probation slots held by p1 (mining, zero bond) and p2 (idle, 5 PCOL). At 19
    # q's probation register reverts Capacity. The set is not full (four of six seats), so nothing is contestable (code 2:
    # a pool contest needs a full set, as every contest, O-10=b), with a probation bond or the full one. x and y fill the
    # set; then contesting p1 (above the threshold) and a (HELD: the pool contest takes only ACCRUING victims) revert, and
    # q's contest with bond 0 displaces p2: p2 exits as by deregister, keeping its 5 PCOL until dueHeight 59 (owner decision
    # 2026-11-13, option a), and gives up its pool seat at once (pool_out), so ACC_COUNT 2 -> 1 -> 2 with q seated.
    ops = {19: [('register', 'q', 'q', 0, 19), contest_status('p2', 19, True), contest('q', 'p2', 19, 0),
                contest('q', 'p2', 19, B), ('register', 'x', 'x', B, 19), ('register', 'y', 'y', B, 19),
                contest_status('p2', 19, True), contest('q', 'p1', 19, 0), contest('q', 'a', 19, 0), contest('q', 'p2', 19, 0),
                contest_status('q', 19, True)]}
    v['pool-contest'] = contest_chain(24, ops, members=(('a', B), ('p1', 0), ('p2', 5 * PCOL)),
                                      extra=lambda h: ['p1'], cp={'M_max': 6})
    # Exits before a pool contest cannot raise the threshold (review finding: with a pool contest on a set that is not
    # full, E's denominator was the active count at the contest). Six seats full (m0, a, c, d, p1, p2), p2 with 5 shares in
    # epoch 1: Tprev = 40 + 5 = 45, threshold at six seats ceil(7.5 - sqrt(7.5)) = 5, so p2 is not contestable (code 7);
    # on the copy, a and c deregister first (active 4): nothing is contestable (code 2), where the first draft's rule (E
    # over the active count now) would give ceil(11.25 - sqrt(11.25)) = 8 and displace p2.
    work = lambda h: ['p1'] + (['p2'] if 9 <= h <= 13 else [])
    four = (('a', B), ('c', B), ('d', B), ('p1', 0), ('p2', 5 * PCOL))
    v['pool-contest-exits-cannot-raise-the-threshold'] = contest_chain(
        22, {19: [('deregister', 'a', 19), ('deregister', 'c', 19), contest_status('p2', 19, True), contest('q', 'p2', 19, 0)]},
        members=four, extra=work, cp={'M_max': 6})
    v['pool-contest-above-the-threshold-at-the-full-set'] = contest_chain(
        22, {19: [contest_status('p2', 19, True), contest('q', 'p2', 19, 0)]}, members=four, extra=work, cp={'M_max': 6})
    # The pool victim's pending pots (D-V3-14; owner decision 2026-11-13, option a): p2 is idle in epoch 1 and mines
    # templates 16..18 of epoch 2; displaced at 19, it stays an exited record, so its shares of pots 16..18 (settled at
    # 20..22) fill its accrued bond as for any exited ACCRUING record. Nothing burns that the same chain without the
    # contest does not burn, and p2's bond is the same (the first draft erased p2 at once and burned exactly that part).
    pots = dict(members=(('a', B), ('p1', 0), ('p2', 5 * PCOL)), extra=lambda h: ['p1'] + (['p2'] if h in (17, 18, 19) else []))
    ops = {19: [contest_status('p2', 19, True), contest('q', 'p2', 19, 0)]}
    v['pool-victim-pending-pots-fill-its-bond'] = contest_chain(24, ops, **pots)
    v['pool-victim-pending-pots-without-a-contest'] = contest_chain(24, {}, **pots)
    out, keep = v['pool-victim-pending-pots-fill-its-bond']['final'], v['pool-victim-pending-pots-without-a-contest']['final']
    assert out['burned'] == keep['burned'] and 'p2' not in out['paid'] and \
        out['records']['p2']['bond'] == keep['records']['p2']['bond'] != str(5 * PCOL) and out['records']['p2']['pool_out'], \
        'the pool victim keeps what its pending pots accrue; no burn beyond the chain without the contest'
    # Its refund at dueHeight (19 + U = 59, by the sweep): ACC_COUNT fell once, at the contest (2 -> 1 -> 2 with q), and the
    # refund leaves it at 2; held_total falls by the bond the pots filled, which reaches p2's reward address in full.
    v['pool-victim-refunded-at-due'] = contest_chain(60, ops, **pots)
    fin = v['pool-victim-refunded-at-due']['final']
    assert 'p2' not in fin['records'] and fin['accruing'] == 2 and fin['paid']['p2'] == keep['records']['p2']['bond'], \
        'refunded at dueHeight with what its pots accrued; ACC_COUNT not lowered again'
    # Pending pots that fill the bond: p2 holds B_reg less 0.01 PCOL, so its first settled share promotes it to HELD while
    # exited (the rest of its parts is paid to its reward address); ACC_COUNT stays (p2 left it at the contest), and the
    # sweep refunds B_reg at 59.
    near = dict(members=(('a', B), ('p1', 0), ('p2', B - PCOL // 100)), extra=pots['extra'])
    v['pool-victim-promoted-by-its-pending-pots'] = contest_chain(60, ops, **near)
    mid = contest_chain(24, ops, **near)['final']
    fin = v['pool-victim-promoted-by-its-pending-pots']['final']
    assert mid['records']['p2']['bond_state'] == HELD and mid['records']['p2']['status'] == 2 and mid['accruing'] == 2, \
        'promoted while exited, counted once'
    assert int(fin['paid']['p2']) - int(mid['paid']['p2']) == B and fin['accruing'] == 2, 'refunded B_reg at 59'
    # A pool victim stays punishable after the contest (the first draft erased it, so evidence carried later found no
    # record): p2 wins template 18 (recorded ACCRUING at block 19) and is displaced at 19; block 21 carries type-2 evidence
    # against it for pot 18, still pending: p2 is tombed, its bond burned less the reporter's cut, the whole pot 18 burns
    # (V3-16), ACC_COUNT does not fall again (p2 left it at the contest), held_total falls by p2's bond; the tomb is erased
    # at 21 + E_win + D + 1 = 42, with no refund.
    punished = dict(pots, late=lambda h: [('p2', 18)] if h == 21 else [], winners={19: 'p2'})
    v['pool-victim-punished-after-the-contest'] = contest_chain(44, ops, **punished)
    fin = v['pool-victim-punished-after-the-contest']['final']
    assert 'p2' not in fin['records'] and 'p2' not in fin['paid'] and fin['accruing'] == 2
    # The newcomer is seated at once: in the contest block q is a live active ACCRUING record (its view: code 5, in grace),
    # tops up its bond, and its shares count from the next template; p2, exited, holds neither seat (a second probationer's
    # registration in that block reverts Capacity: the set and the pool are full again).
    seated = {19: ops[19] + [contest_status('q', 19, True), ('top_up', 'q', PCOL, 19), ('register', 'r', 'r', 0, 19)]}
    v['pool-newcomer-seated-at-once'] = contest_chain(
        22, seated, members=pots['members'], extra=lambda h: pots['extra'](h) + (['q'] if h >= 20 else []))
    # A pool victim reactivates only into a free pool seat (W_act 2, U 6, C_react 6): a exits at 21 (a free active seat), but
    # the pool is full (p1, q), so p2's reactivate reverts Capacity; p1 promotes by topUp, freeing a pool seat, and p2
    # reactivates in the same block: pool_out cleared, ACC_COUNT 1 -> 2, graceStart 21.
    react = {21: [('deregister', 'a', 21), ('reactivate', 'p2', 21), ('top_up', 'p1', B, 21), ('reactivate', 'p2', 21)]}
    v['pool-victim-reactivates-into-a-free-pool-seat'] = contest_chain(
        24, {**ops, **react}, **pots, cp={'W_act': 2, 'U': 6, 'C_react': 6})
    fin = v['pool-victim-reactivates-into-a-free-pool-seat']['final']
    assert fin['records']['p2']['status'] == 1 and 'pool_out' not in fin['records']['p2'] and fin['accruing'] == 2
    # A seat-contest victim stays punishable until its due height: c (HELD) and p (ACCRUING, 5 PCOL), both idle, are
    # displaced at 19 by full-bond newcomers; block 21 carries type-1 evidence against both (reporter m0): each is tombed,
    # its bond burned less the reporter's cut, ACC_COUNT falls once (at the punish, not again at the sweep), held_total by
    # both bonds. The first scenario ends at 30 (both tombs held, before the due height 59); the second runs on to 64: the
    # sweep refunds neither, and erases both tombs at 21 + E_win + D + 1 = 42.
    ops = {19: [contest('n1', 'c', 19), contest('n2', 'p', 19)]}
    for name_, last in (('seat-victims-punished-before-due', 30), ('seat-victims-punished-then-swept', 64)):
        v[name_] = contest_chain(last, ops, members=(('a', B), ('c', B), ('p', 5 * PCOL)),
                                 late=lambda h: [('c', None), ('p', None)] if h == 21 else [])
    # A probationer newcomer with room in the pool contests a full set: the seat contest. Its victim p is ACCRUING (idle,
    # zero bond): p exits and keeps its slot (ACC_COUNT 2 with q) until the sweep refunds it at 19 + U = 59 (ACC_COUNT 1).
    ops = {19: [contest_status('p', 19), contest('q', 'p', 19, 0)]}
    v['seat-contest-probation-newcomer-accruing-victim'] = contest_chain(60, ops, members=(('a', B), ('b', B), ('p', 0)))
    # Both full (four seats, two slots): q's probation contest of the idle p2 frees its seat and its slot at once (p2 exits,
    # refunded after U).
    ops = {19: [contest_status('p2', 19, True), contest('q', 'p2', 19, 0)]}
    v['pool-and-seats-full'] = contest_chain(22, ops, members=(('p1', 0), ('p2', 0), ('a', B)), extra=lambda h: ['p1'])
    # A rotation race in the pool (four seats, full while s holds one; SWEEP_MAX 0, so only claimBond refunds; W_act 2,
    # U 6): s exits at 9 and, at its due
    # height 15, claims its bond and registers again in the same block before q, so q's register reverts Capacity and its
    # contest of the fresh record reverts (in grace, code 5; past it, code 6 until its registration precedes the measured
    # epoch); at 27 (epoch 2 measured) s, idle, is displaced. Then p1 exits at 28: at 34 q2 registers too early (the exited p1 still holds its
    # slot), p1 claims its bond, q2 takes the slot and p1's registration reverts Capacity. In the same block a victim that
    # exits first cannot be contested (BadState).
    ops = {9: [('deregister', 's', 9)],
           15: [('claim_bond', 's', 15), ('register', 's', 's', 0, 15), ('register', 'q', 'q', 0, 15), contest_status('s', 15, True),
                contest('q', 's', 15, 0)],
           20: [contest_status('s', 20, True), contest('q', 's', 20, 0)],
           27: [contest_status('s', 27, True), contest('q', 's', 27, 0)],
           28: [('deregister', 'p1', 28)],
           34: [('register', 'q2', 'q2', 0, 34), ('claim_bond', 'p1', 34), ('register', 'q2', 'q2', 0, 34), ('register', 'p1', 'p1', 0, 34)],
           35: [('deregister', 'a', 35), contest('z', 'a', 35)]}
    v['pool-rotation-race'] = contest_chain(37, ops, members=(('a', B), ('p1', 0), ('s', 0)),
                                            cp={'M_max': 4, 'W_act': 2, 'U': 6, 'C_react': 6, 'SWEEP_MAX': 0})
    # ACC_COUNT through promote, exit, displace and punish (P_MAX 3, eight seats): p3 promotes by topUp, p4 takes the slot,
    # p1 exits (still counted until its refund), p2 is punished (type-1 evidence, reporter m0), q takes that slot; at 19 the
    # pool (p1 exited, p4, q) is full, x, y and w fill the set (a pool contest needs a full set) and z displaces the idle
    # p4 (exited, out of the pool: refunded at 59, after the scenario); the sweep refunds p1 at 45. The model checks
    # ACC_COUNT = the live ACCRUING records that hold a pool seat after every step.
    sc_ops = {2: [('top_up', 'p3', 5 * PCOL, 2), ('register', 'p4', 'p4', 0, 2), ('register', 'p5', 'p5', 0, 2)],
              5: [('deregister', 'p1', 5)], 7: [('register', 'q', 'q', 0, 7)],
              19: [('register', 'z', 'z', 0, 19), contest_status('p4', 19, True), ('register', 'x', 'x', B, 19),
                   ('register', 'y', 'y', B, 19), ('register', 'w', 'w', B, 19), contest_status('p4', 19, True),
                   contest('z', 'p4', 19, 0)]}
    sc = Script().block(1, ops=joined(('a', B), ('p1', 0), ('p2', 0), ('p3', 15 * PCOL)))
    for h in range(2, 47):
        r = busy(h, ['q'] if h > 8 else [])
        sc.block(h, winner=r['winner'], shares=r['shares'], evidence=[('p2', None)] if h == 6 else (), ops=sc_ops.get(h, []))
    v['acc-count-promote-exit-displace-punish'] = run_chain(sc.steps, {**CONTEST_CP, 'P_MAX': 3, 'M_max': 8})
    # V3-02 is counted (V3-PR-E3 late credit): the winners leave c's three shares of each template 8..15 out of their
    # headers; carriers credit them late (template t at t + 1 .. t + 3, templates 14 and 15 at heights 17 and 18 of epoch 2,
    # after c's late share of template 16, so countPrev takes them). Before 19 the contest is not judged (code 4: the late
    # window of template 15 is open); at 19 c has 24 > the threshold and cannot be contested. Without the late credit c
    # has 0 and is displaced.
    def late_of(h):
        out = [late(t, 'c', 'c', 'c') for t in range(8, 16) if h == t + 1 + (t >= 14) * 2 and t < 14] + \
              [late(t, 'c', 'c', 'c') for t in (14, 15) if h == t + 3]
        if h == 17:
            out.insert(0, late(16, 'c'))
        return out
    ops = {17: [contest_status('c', 17)], 18: [contest_status('c', 18)], 19: [contest_status('c', 19), contest('n', 'c', 19)]}
    v['late-credit-counts-in-the-statistic'] = contest_chain(22, ops, late=late_of)
    v['omitted-shares-without-late-credit'] = contest_chain(22, ops)
    # The counters roll at the epoch boundary: c works in epochs 0 and 1, rests in epoch 2 and works again from template
    # 26; the views show epoch 1's count at 19 (countCur, until c's first share of epoch 2... which never comes), 0 for
    # epoch 2 at 27 and epoch 3's at 35, and a works throughout.
    work = lambda h: ['c'] if (2 <= h <= 16 or h >= 27) else []
    ops = {11: [contest_status('c', 11), contest_status('a', 11)], 19: [contest_status('c', 19), contest_status('a', 19)],
           27: [contest_status('c', 27)], 35: [contest_status('c', 35), contest_status('a', 35)]}
    v['counters-roll-at-the-epoch-boundary'] = contest_chain(36, ops, extra=work)
    # E_epoch below D: the late window of an epoch's last template closes in a later epoch, so nothing is ever contestable
    # (code 4); a late share two epochs older than the owner's lastShare only keeps lastShare's max.
    ops = {h: [contest_status('c', h)] for h in range(9, 15)}
    v['tiny-epoch-never-contestable'] = contest_chain(
        16, ops, extra=lambda h: ['c'], late=lambda h: [late(h - 4, 'c')] if h >= 6 else [], cp={'E_epoch': 2})
    return v

# ---------------- V3-02: the host's kind-3 validity (pocold chain.rs late_fields / late_window / late_check)
import hashlib

def rlp_bytes(b):
    if len(b) == 1 and b[0] < 0x80:
        return bytes(b)
    return rlp_header(len(b), 0x80) + bytes(b)

def rlp_header(n, base):
    if n <= 55:
        return bytes([base + n])
    ln = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([base + 55 + len(ln)]) + ln

def rlp_list(*encoded):
    payload = b''.join(encoded)
    return rlp_header(len(payload), 0xc0) + payload

def rlp_decode(b):
    """A strict decoder for the items the vectors carry (canonical RLP, as pocold's)."""
    def item(i):
        x = b[i]
        if x < 0x80:
            return b[i:i + 1], i + 1
        if x <= 0xb7:
            n = x - 0x80
            out = b[i + 1:i + 1 + n]
            assert len(out) == n and not (n == 1 and out[0] < 0x80)
            return out, i + 1 + n
        if x <= 0xbf:
            k = x - 0xb7
            n = int.from_bytes(b[i + 1:i + 1 + k], 'big')
            assert n > 55
            return b[i + 1 + k:i + 1 + k + n], i + 1 + k + n
        if x <= 0xf7:
            n, start = x - 0xc0, i + 1
        else:
            k = x - 0xf7
            n, start = int.from_bytes(b[i + 1:i + 1 + k], 'big'), i + 1 + k
            assert n > 55
        out, j = [], start
        while j < start + n:
            it, j = item(j)
            out.append(it)
        assert j == start + n
        return out, j
    it, end = item(0)
    assert end == len(b)
    return it

def ls_item(j, nonces, jlen=8, nlen=8, kind=3, extra=()):
    return rlp_list(rlp_bytes(bytes([kind])), rlp_bytes(j.to_bytes(jlen, 'big')),
                    rlp_list(*[rlp_bytes(n.to_bytes(nlen, 'big')) for n in nonces]), *extra).hex()

def pow_ok(tid, n, t_share):
    return int.from_bytes(hashlib.sha256(bytes.fromhex(tid) + n.to_bytes(8, 'big')).digest(), 'big') <= t_share

LATE_MUTANTS = ['form-short-j', 'count-allows-empty', 'count-ignores-s-max', 'order-allows-equal', 'window-below-open',
                'window-low-plus-one', 'window-allows-current', 'cap-ignores-block', 'cap-strict', 'cap-removed',
                'nmax-inclusive-end', 'owner-from-carrier', 'winning-nonce-allowed', 'header-share-allowed',
                'branch-credit-allowed', 'block-repeat-allowed', 'target-ignored']

def late_verdict(case, mutant=None):
    """The kind-3 rules in pocold's order (chain.rs late_fields, late_window, late_check) over one carrying block's items
    and the branch facts of each template j. Returns {'ok': True, 'effects': [[j, owners]]} or {'ok': False, 'at': i,
    'reason': r}. `mutant` plants one wrong rule (LATE_MUTANTS); each must fail some vector (late_validity_vectors)."""
    r = case['rules']
    seen = {}
    effects = []
    for i, raw in enumerate(case['items']):
        def no(reason):
            return {'ok': False, 'at': i, 'reason': reason}
        f = rlp_decode(bytes.fromhex(raw))
        if not isinstance(f, list) or len(f) != 3 or f[0] != b'\x03':
            return no('evidence form')
        if not isinstance(f[1], bytes) or (len(f[1]) != 8 and not (mutant == 'form-short-j' and 1 <= len(f[1]) <= 8)):
            return no('ls form')
        if not isinstance(f[2], list) or any(not isinstance(x, bytes) or len(x) != 8 for x in f[2]):
            return no('ls form')
        j = int.from_bytes(f[1], 'big')
        nonces = [int.from_bytes(x, 'big') for x in f[2]]
        if (not nonces and mutant != 'count-allows-empty') or (len(nonces) > r['S_max'] and mutant != 'count-ignores-s-max'):
            return no('ls count')
        if any(a > b or (a == b and mutant != 'order-allows-equal') for a, b in zip(nonces, nonces[1:])):
            return no('ls order')
        lo = max(1, r['h'] - r['D'])
        if mutant == 'window-below-open':
            lo = 1
        if mutant == 'window-low-plus-one':
            lo = max(1, r['h'] - r['D'] + 1)
        hi = r['h'] if mutant == 'window-allows-current' else r['h'] - 1
        if j < lo or j > hi:
            return no('ls window')
        b = case['bases'][str(j)]
        s = seen.setdefault(j, set())
        used = b['total'] + (0 if mutant == 'cap-ignores-block' else len(s)) + len(nonces)
        if mutant != 'cap-removed' and (used > r['kappa_m'] or (mutant == 'cap-strict' and used == r['kappa_m'])):
            return no('ls cap')
        ranges = case['carrierRanges'] if mutant == 'owner-from-carrier' else b['ranges']
        owners = []
        for n in nonces:
            owner = next((o for o, lo_, hi_ in ranges if lo_ <= n < hi_ or (mutant == 'nmax-inclusive-end' and n == hi_)), None)
            if owner is None:
                return no('ls nonce outside nMax')
            if n == b['nonce'] and mutant != 'winning-nonce-allowed':
                return no('ls winning nonce')
            if n in b['header'] and mutant != 'header-share-allowed':
                return no('ls header share')
            if n in b['credited'] and mutant != 'branch-credit-allowed':
                return no('ls credited on the branch')
            if n in s and mutant != 'block-repeat-allowed':
                return no('ls repeated in the block')
            if not pow_ok(b['tid'], n, int(b['tShare'], 16)) and mutant != 'target-ignored':
                return no('ls share target')
            owners.append(owner)
        s.update(nonces)
        effects.append([j, owners])
    return {'ok': True, 'effects': effects}

def late_validity_vectors():
    """Concrete kind-3 cases: TemplateIDs, share targets (about half of all nonces meet 2^255), stored nonce ranges of
    three members partitioning [0, nMax_j = 3000), header shares, branch credits and ledger totals. Each case's verdict
    comes from late_verdict; pocold replays them through chain.rs late_parse/late_window/late_check
    (tests/v3_late_shares.rs), and every planted mutant of LATE_MUTANTS fails at least one."""
    T = (1 << 255) - 1
    def tid(label):
        return hashlib.sha256(b'pocol-v3-late:' + label.encode()).hexdigest()
    ranges = [['a', 0, 1000], ['b', 1000, 2000], ['c', 2000, 3000]]
    other = [['c', 0, 1500], ['a', 1500, 3000]]          # the carrier's own assignment (members changed since j)
    def passing(t, lo, hi, k, skip=()):
        out = [n for n in range(lo, hi) if pow_ok(t, n, T) and n not in skip]
        return out[:k]
    def failing(t, lo, hi):
        return next(n for n in range(lo, hi) if not pow_ok(t, n, T))
    rules = {'h': 10, 'D': 4, 'S_max': 8, 'kappa_m': 8}
    t6, t8, t9 = tid('j6'), tid('j8'), tid('j9')
    win = passing(t9, 2000, 3000, 1)[0]
    hdr = passing(t9, 0, 1000, 2)
    cred = passing(t9, 1000, 2000, 1, skip=hdr)
    good = passing(t9, 1000, 2000, 6, skip=cred)
    def base(t, total=3, header=(), credited=(), nonce=None):
        return {'tid': t, 'nonce': win if nonce is None else nonce, 'tShare': f'{T:064x}', 'ranges': ranges,
                'header': list(header), 'credited': list(credited), 'total': total}
    bases = {'9': base(t9, 3, hdr, cred), '8': base(t8, 1, nonce=5), '6': base(t6, 1, nonce=5),
             '10': base(tid('j10'), 1, nonce=5), '5': base(tid('j5'), 1, nonce=5)}
    cases = {}
    def case(name, items, **kw):
        c = {'rules': dict(rules, **kw.get('rules', {})), 'bases': bases, 'carrierRanges': other, 'items': items}
        c['expect'] = late_verdict(c)
        cases[name] = c
    g8 = passing(t8, 0, 3000, 4)
    g6 = passing(t6, 0, 3000, 2)
    case('ok-one-item', [ls_item(9, good[:2])])
    case('ok-owner-from-block-j-ranges', [ls_item(9, [good[0]] + passing(t9, 2000, 3000, 1, skip=[win]))])
    case('ok-two-templates', [ls_item(9, good[:1]), ls_item(8, g8[:2])])
    case('ok-two-items-one-template-fill-the-cap', [ls_item(9, good[:2]), ls_item(9, good[2:5])])
    case('ok-window-low-boundary-j-equals-h-minus-d', [ls_item(6, g6)])
    case('form-four-fields', [ls_item(9, good[:1], extra=(rlp_bytes(b'x'),))])
    case('form-j-seven-bytes', [ls_item(9, good[:1], jlen=7)])
    case('form-nonce-nine-bytes', [ls_item(9, good[:1], nlen=9)])
    case('form-nonces-not-a-list', [rlp_list(rlp_bytes(b'\x03'), rlp_bytes((9).to_bytes(8, 'big')), rlp_bytes(good[0].to_bytes(8, 'big'))).hex()])
    case('count-empty', [ls_item(9, [])])
    case('count-above-s-max', [ls_item(8, passing(t8, 0, 3000, 9))], rules={'kappa_m': 64})
    case('duplicate-in-the-list', [ls_item(9, [good[0], good[0]])])
    case('unsorted-list', [ls_item(9, [good[1], good[0]])])
    case('window-below-h-minus-d', [ls_item(5, [7])])
    case('window-current-height', [ls_item(10, [7])])
    case('window-zero', [ls_item(0, [7])], rules={'h': 3})
    case('cap-already-full', [ls_item(9, good[:1])], rules={'kappa_m': 3})
    case('cap-exceeded-by-the-item', [ls_item(9, good[:6])])
    case('cap-exceeded-across-items', [ls_item(9, good[:3]), ls_item(9, good[3:6])])
    case('nonce-at-nmax', [ls_item(9, [3000])])
    case('nonce-above-nmax', [ls_item(9, [good[0], 1 << 40])])
    case('winning-nonce', [ls_item(9, [win])])
    case('header-share', [ls_item(9, [hdr[0]])])
    case('duplicate-across-carriers', [ls_item(9, [cred[0]])])
    case('duplicate-within-the-block', [ls_item(9, good[:2]), ls_item(9, good[1:3])])
    case('target-miss', [ls_item(9, [failing(t9, 1000, 2000)])])
    # A share mined on a sibling template of height 9 (not the branch's ancestor): it meets the sibling's target and
    # not the ancestor's, so it is refused as a target miss on the branch's TemplateID.
    sib = tid('j9-sibling')
    case('non-ancestor-template', [ls_item(9, [next(n for n in range(1000, 2000) if pow_ok(sib, n, T) and not pow_ok(t9, n, T))])])
    # A share in another member's range is that member's: owners come from block j's stored ranges, never from the
    # carrier (the carrier's assignment differs here).
    case('another-members-range-is-that-members', [ls_item(9, passing(t9, 0, 1000, 1, skip=hdr) + good[:1])])
    return cases

def late_mutants_killed(cases):
    out = {}
    for m in LATE_MUTANTS:
        killed = sorted(n for n, c in cases.items() if late_verdict(c, m) != c['expect'])
        assert killed, f'late validity mutant {m} survives every vector'
        out[m] = killed
    return out

if __name__ == '__main__':
    files = {'registry-budget-vectors.json': vectors, 'earnin-on-v3-vectors.json': earnin_on_v3,
             'evidence-penalty-vectors.json': evidence_vectors, 'late-share-vectors.json': late_vectors,
             'late-share-validity-vectors.json': late_validity_vectors, 'issuance-carry-vectors.json': issuance_vectors,
             'contest-vectors.json': contest_vectors}
    for name, make in files.items():
        path = Path(__file__).with_name('v3') / name
        data = json.dumps(make(), indent=1, sort_keys=True) + '\n'
        if sys.argv[1:] == ['--write']:
            path.write_text(data)
            print('wrote', path)
        else:
            assert path.read_text() == data, f'{name} differs from the model; run --write and review'
            print('PASS v3 model matches', name)
    print('PASS pot h is fixed by block h: dropping later late credits moved a later pot in',
          check_pot_fixed_by_its_block(), 'variants and never an earlier one')
    print('PASS the fixedness check fails on base-burn-into-closed-pot:', fixedness_mutant_killed())
    print('PASS the last carrier of j gains nothing by omitting LS(j) (D-V3-15, lagged carry):', check_last_carrier_gains_nothing())
    print('PASS the last-carrier check fails on carry-not-lagged (the previous rule):', lag_mutant_killed())
    econ = econ_mutants_killed()
    print(f'PASS every one of {len(econ)} planted issuance/carry/tip mutants fails the issuance-carry vectors:',
          ', '.join(f'{m} ({k})' for m, k in econ.items()))
    killed = late_mutants_killed(late_validity_vectors())
    print(f'PASS every one of {len(killed)} planted late-share validity mutants fails a vector:',
          ', '.join(f'{m} ({len(k)})' for m, k in killed.items()))
