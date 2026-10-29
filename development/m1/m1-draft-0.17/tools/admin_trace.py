"""AdminFIFO guard trace for M1 draft 0.17 (C27 and RF-E6-1 fixtures).
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

TracedAdminEngine subclasses the 0.16 AdminEngine (m1-draft-0.16/tools/admin_ref.py, imported
read-only) and records every AdminFIFO step in `fifo_log`:
  ['request', t, op, n]               attempt n requested (before any grant in the same step)
  ['grant', t, op, n, set#]           guard passed; reserve + alloc + issue in one step
  ['grantUnguarded', t, op, n, set#]  only under the fault adminStaleIssue (guard skipped)
  ['diskRejected', t, op, n]          reserve refused (no alloc, no set)
  ['withdraw', t, op, n]              withdrawn at its operation's resolution
  ['skipWithdrawn', t, op, n]         a withdrawn request reaches the FIFO head and is discarded
  ['staleDrop', t, op, n]             guard failed: slot returned at once, no reserve/alloc/set
  ['waitExpired', t, op, n]           ADMIN_SLOT_WAIT ran out while still queued

_pump_admin is a line-for-line copy of AdminEngine._pump_admin with log calls added; the runner
checks that the traced and untraced engines produce identical sets, stats and traces.

Analysis mode (NOT a source fault): no_cancel_ops = {'op1'} disables withdrawal for the listed
operations only. It is used to show the reading of x12b under which the literal total
adminStaleDropped = 1 holds (annex C27-RFE61 section 3, option B).
"""

import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_ROOT = _HERE.parent.parent
sys.path.insert(0, str(_ROOT / 'm1-draft-0.16' / 'tools'))
import admin_ref as A                  # noqa: E402  (0.16, read-only)

R13 = A.R13


class TracedAdminEngine(A.AdminEngine):
    def __init__(self, *a, no_cancel_ops=(), **kw):
        super().__init__(*a, **kw)
        self.fifo_log = []
        self._withdraw_logged = set()
        self.no_cancel_ops = frozenset(no_cancel_ops)

    def _log(self, kind, req, *extra):
        self.fifo_log.append([kind, self.t, req['op'], req['n']] + list(extra))

    def _log_withdrawals(self):
        for r in self.admin_fifo2:
            if r['withdrawn'] and id(r) not in self._withdraw_logged:
                self._withdraw_logged.add(id(r))
                self._log('withdraw', r)

    def _start_attempt(self, op_id, n):
        self.fifo_log.append(['request', self.t, op_id, n])
        super()._start_attempt(op_id, n)

    def _admin_wait_expired(self, req):
        if not req['done'] and not req['withdrawn'] and self.dops[req['op']]['state'] == 'running':
            self._log('waitExpired', req)            # only when the expiry has an effect
        super()._admin_wait_expired(req)

    def _admin_resolve(self, dop, outcome):
        if dop['id'] in self.no_cancel_ops:
            self.admin_fifo2 = [A._Sticky(r) if r['op'] == dop['id'] else r for r in self.admin_fifo2]
        super()._admin_resolve(dop, outcome)

    def _pump_admin(self):
        self._log_withdrawals()
        while len(self.admin_slots) < R13.ADMIN_SLOTS and self.admin_fifo2:
            req = self.admin_fifo2.pop(0)
            if req['withdrawn']:
                self._log('skipWithdrawn', req)
                continue
            dop = self.dops[req['op']]
            guard_ok = dop['state'] == 'running' and self.deleting.get(dop['key']) == dop['id']
            if not guard_ok and 'adminStaleIssue' not in self.faults:
                self.admin_stats['adminStaleDropped'] += 1
                req['done'] = True
                self._tr('adminStaleDropped', op=dop['id'], n=req['n'])
                self._log('staleDrop', req)
                continue
            req['done'] = True
            self._issue_tomb(dop, req['n'])
            att = dop['attempts'][req['n']]
            if att['state'] == 'diskRejected':
                self._log('diskRejected', req)
            else:
                self._log('grant' if guard_ok else 'grantUnguarded', req, att['set'])
