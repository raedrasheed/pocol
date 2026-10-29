"""signEligible / ProfileMutex / eligibilityEpoch reference model (R6-03 X2, R6-04 Q8-Q12).

Specification tooling only: a deterministic discrete-event model used to make the
restored Q8-Q12 scenarios falsifiable. It is not the extension's Wallet or
NetworkProfiles module. Labels in comments:
  B = current baseline (FD:L1321-1325, FD:L982-983, FD:L1001, FD:L1744-1751)
  H = historical round-15 source (D58 text), carried as a proposal
  P = this draft's reconciliation choice
"""

from keccak import keccak256
import rlp_strict as R

C_CONFLICT, C_IDENTITY, C_REJECT = 4100, 4901, 4001


def net_key(chain_id, genesis_hash):
    """B (FD:L981): keccak256(RLP(['PoCol-net-v1', chainId, genesisHash]))."""
    return '0x' + keccak256(R.encode([b'PoCol-net-v1', R.uint(chain_id), bytes.fromhex(genesis_hash[2:])])).hex()


FIELDS = ('chainId', 'genesisHash', 'endpoint', 'trustLevel')


class World:
    def __init__(self, profiles, site, rp=None, node_genesis=None):
        self.cache = {k: dict(v) for k, v in profiles.items()}      # worker state (committed)
        self.storage = {k: dict(v) for k, v in profiles.items()}    # chrome.storage 'profiles:' keys
        self.revoked, self.connections, self.locked, self.epoch = set(), set(), False, 0
        self.rp = rp or {}                                          # pid -> {'h': int, 'windowOk': bool}
        self.node_genesis = node_genesis or {}                      # endpoint -> genesis actually served
        self.site = site
        self.pending, self.signs, self.sends, self.log = {}, 0, 0, []
        self.mutex, self.queue, self.on_changed = None, [], []

    # -- eligibility (B: seven conditions of FD:L1323; codes B where stated, else H) --
    def _conflict(self, prof, view):
        return any(p['chainId'] == prof['chainId'] and p['genesisHash'] != prof['genesisHash']
                   for p in view.values() if p is not prof)

    def sign_eligible(self, req, view=None):
        view = self.cache if view is None else view
        prof = view.get(req['pid'])
        if prof is None or any(prof[f] != req['frozen'][f] for f in FIELDS):
            return C_IDENTITY                                       # 1: profile changed (B L982 -> 4901)
        if self._conflict(prof, view):
            return C_CONFLICT                                       # 2: conflict (B L983 -> 4100)
        nk = net_key(prof['chainId'], prof['genesisHash'])
        if nk in self.revoked:
            return C_CONFLICT                                       # 3: revoked (H -> 4100)
        if (nk, self.site) not in self.connections:
            return C_CONFLICT                                       # 4: connection withdrawn (H -> 4100)
        if self.locked:
            return C_CONFLICT                                       # 5: wallet locked (H -> 4100)
        if prof['trustLevel'] == 'RP':
            st = self.rp.get(req['pid'], {})
            if not (st.get('h', 0) >= 1 and st.get('windowOk')):
                return C_IDENTITY                                   # 7: RP without h>=1 and a window (B L1001 -> 4901)
        if req['epoch'] != self.epoch:
            req['epoch'] = self.epoch                               # 6: P (U42) re-stamp when 1-5 and 7 hold
        return None

    # -- requests ----------------------------------------------------------------
    def request(self, rid, pid):
        prof = self.cache[pid]
        req = {'pid': pid, 'frozen': {f: prof[f] for f in FIELDS}, 'epoch': self.epoch, 'state': 'pending', 'code': None}
        code = self.sign_eligible(req)
        if code:
            req.update(state='rejected', code=code)
        self.pending[rid] = req
        return req

    # -- mutations under ProfileMutex (B L1322; FIFO H) --------------------------
    def _apply(self, op):
        kind = op[0]
        if kind == 'addProfile':
            self.cache[op[1]] = dict(op[2])
            self.storage[op[1]] = dict(op[2])
        elif kind == 'setField':
            self.cache[op[1]][op[2]] = op[3]
            self.storage[op[1]][op[2]] = op[3]
        elif kind == 'revoke':
            self.revoked.add(op[1])
        elif kind == 'disconnect':
            self.connections.discard((op[1], self.site))
        elif kind == 'connect':
            self.connections.add((op[1], self.site))
        elif kind == 'lock':
            self.locked = True
        elif kind == 'adoptStorage':                                # onChanged detection (H)
            self.cache = {k: dict(v) for k, v in self.storage.items()}
        self.epoch += 1
        for rid, req in self.pending.items():                       # re-evaluate inside the lock (H)
            if req['state'] == 'pending':
                code = self.sign_eligible(req)
                if code:
                    req.update(state='cancelled', code=code)
                    self.log.append(('cancelled', rid, code, 'atCommit'))

    def submit(self, op):
        if self.mutex is None:
            self.mutex = 'mutation'
            self._apply(op)
            self.mutex = None
            self._drain()
        else:
            self.queue.append(op)                                   # waits: cannot commit between accept steps

    def raw_write(self, pid, field=None, value=None, profile=None):
        """Direct write to chrome.storage 'profiles:' from a non-worker context (H)."""
        if profile is not None:
            self.storage[pid] = dict(profile)
        else:
            self.storage[pid][field] = value
        self.on_changed.append(pid)
        if self.mutex is None:
            self._drain()

    def _drain(self):
        while self.mutex is None and (self.queue or self.on_changed):
            if self.queue:
                op = self.queue.pop(0)
            else:
                self.on_changed.pop(0)
                op = ('adoptStorage',)
            self.mutex = 'mutation'
            self._apply(op)
            self.mutex = None

    # -- acceptance (B L1324: eligibility, identity, second check, sign, send, release) --
    def accept(self, rid, during=None, raw_at=None, raw=None):
        req = self.pending.get(rid)
        if req is None or req['state'] != 'pending':
            self.log.append(('acceptNoEffect', rid))
            return None
        self.mutex = 'accept:' + rid
        code = self.sign_eligible(req)                              # step 1 (worker state)
        if not code:
            for op in during or []:                                 # submitted while the lock is held: queued
                self.submit(op)
            if raw_at == 'beforeStep3':
                self.raw_write(**raw)
            served = self.node_genesis.get(req['frozen']['endpoint'], req['frozen']['genesisHash'])
            if served != req['frozen']['genesisHash']:
                code = C_IDENTITY                                   # step 2: identity on the frozen endpoint
            else:
                code = self.sign_eligible(req, view=self.storage)   # step 3: P re-reads storage
            if not code and raw_at == 'afterStep3':
                self.raw_write(**raw)
                self.log.append(('declaredLimit', 'rawWriteAfterStep3', rid))
        if code:
            req.update(state='cancelled', code=code)
            self.log.append(('cancelled', rid, code, 'atAccept'))
        else:
            self.signs += 1                                         # step 4: sign the displayed bytes
            self.sends += 1                                         # step 5: WalletSubmit
            req.update(state='sent')
            if self.queue:
                self.log.append(('sentBefore', rid, [op[0] for op in self.queue]))
        self.mutex = None                                           # step 6: release
        self._drain()
        return req


def conflicting(world):
    """Profiles labelled 'conflicting' (B L983)."""
    out = set()
    for a, pa in world.cache.items():
        for b, pb in world.cache.items():
            if a != b and pa['chainId'] == pb['chainId'] and pa['genesisHash'] != pb['genesisHash']:
                out.add(a)
    return sorted(out)
