"""Alternative E: one eth_call to a Website view getter returning the whole tuple (R7-06).

Specification tooling only. Abstract model, like the 0.3/0.4 snapshot models:
a getter response is either served from one real state (honest node: eth_call
executes on one snapshot, FD:L838) or fabricated by a dishonest endpoint.
The client can verify nothing about the response's origin: there is no proof.
The decoded tuple goes through the same R4-01 invariant checks as P20.
"""

import snapshot_model_04 as S4

GETTER_SIGNATURE = 'websiteSnapshot(uint32)'
GETTER_RETURNS = '(uint32 versionCount, uint32 currentVersion, uint32 selected, bytes32 manifestHash, uint32 manifestLen, uint8 status, uint64 publishedBlock, address[] chunks, uint32[] lengths)'


def tuple_from_state(st, n):
    """What an honest node returns for websiteSnapshot(n); n = 0 selects the current version."""
    s2 = S4._slot2(st)
    sel = s2['currentVersion'] if n == 0 else n
    rec = S4._record(st, sel) if 1 <= sel <= s2['versionCount'] else None
    return {'slot2': s2, 'selected': sel, 'record': rec}


def load_e(states, response, request, anchor_number=100):
    """response: {'from': stateName} (honest) or {'fabricated': tuple}."""
    tup = tuple_from_state(states[response['from']], 0 if request['kind'] == 'default' else request['version']) \
        if 'from' in response else response['fabricated']
    inv = S4.slot2_invariant(tup['slot2'])
    if inv:
        return {'result': 'stateInvariant', 'invariant': inv, 'frames': 0}
    if request['kind'] == 'default' and tup['slot2']['currentVersion'] == 0:
        return {'result': 'noSite', 'frames': 0}
    if tup['record'] is None:
        return {'result': 'versionNotFound', 'frames': 0}
    inv = S4.record_invariant(tup['record'], anchor_number)
    if inv:
        return {'result': 'stateInvariant', 'invariant': inv, 'frames': 0}
    st = tup['record']['status']
    if request['kind'] == 'default':
        if st != S4.PUBLISHED:
            return {'result': 'stateInvariant', 'invariant': 'state.currentNotPublished', 'frames': 0}
        return {'result': 'render', 'frames': 1, 'manifestHash': tup['record']['manifestHash'], 'mixed': False}
    if st == S4.DRAFT:
        return {'result': 'refuseDraft', 'frames': 0}
    if st == S4.REVOKED:
        return {'result': 'revokedInterstitial', 'frames': 0}
    return {'result': 'render', 'frames': 1, 'manifestHash': tup['record']['manifestHash'], 'mixed': False}
