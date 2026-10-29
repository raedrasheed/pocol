"""Draft 0.7 signEligible: a pure predicate over the request's captured epoch (R7-03, C21).

U42 (re-stamping) is WITHDRAWN. Specification tooling only. Subclasses the 0.6
World (m1-draft-0.6/tools/profile_race_ref.py) and replaces only sign_eligible:

- it never writes to the request;
- condition 6 (FD:L1323, eligibilityEpoch == req.epoch) is checked as equality;
- the first failing condition, in order 1..7, gives the code. A request whose
  only failing condition is 6 is stale: it is cancelled and a fresh request
  (fresh approval) is required. P: code 4901 (U43), because the request's
  frozen context is no longer the current one.

Every commit re-evaluates pending requests (unchanged from 0.6), so any
committed mutation cancels every pending request: those with a substantive
failure get that code (conditions 1-5, 7 are checked before 6), all others
get the stale code.
"""

import profile_race_ref as P06

C_STALE = 4901


class World(P06.World):
    def sign_eligible(self, req, view=None):
        view = self.cache if view is None else view
        prof = view.get(req['pid'])
        if prof is None or any(prof[f] != req['frozen'][f] for f in P06.FIELDS):
            return P06.C_IDENTITY                                   # 1
        if self._conflict(prof, view):
            return P06.C_CONFLICT                                   # 2
        nk = P06.net_key(prof['chainId'], prof['genesisHash'])
        if nk in self.revoked:
            return P06.C_CONFLICT                                   # 3
        if (nk, self.site) not in self.connections:
            return P06.C_CONFLICT                                   # 4
        if self.locked:
            return P06.C_CONFLICT                                   # 5
        if req['epoch'] != self.epoch:
            return C_STALE                                          # 6 (pure equality; no rewrite)
        if prof['trustLevel'] == 'RP':
            st = self.rp.get(req['pid'], {})
            if not (st.get('h', 0) >= 1 and st.get('windowOk')):
                return P06.C_IDENTITY                               # 7
        return None


net_key = P06.net_key
conflicting = P06.conflicting
