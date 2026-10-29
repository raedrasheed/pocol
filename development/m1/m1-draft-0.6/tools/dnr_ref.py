"""DnrGuard reference model for the restored د1-د5 scenarios (R6-04). Specification tooling only.

Models chrome.declarativeNetRequest session rules as a dict and the documented
matching of these two rule shapes (tabIds, resourceTypes, urlFilter with a
leading '|' anchor, priority). It does not reproduce Chrome; the browser runs
are Phase A. Labels: B = FD:L1349-1352; H = historical D37 text (round 15).
"""

EXT_ID = 'abcdefghijklmnopabcdefghijklmnop'           # P: placeholder extension id (32 chars a-p)
RT = ['sub_frame', 'stylesheet', 'script', 'image', 'font', 'object', 'xmlhttprequest', 'ping',
      'csp_report', 'media', 'websocket', 'webtransport', 'webbundle', 'other']   # H: all types except main_frame


def rules_for(tab):
    """B: ids 2*tabId / 2*tabId+1, priorities 1 / 2, allow urlFilter '|chrome-extension://<ID>/'.
    H: block is scoped by tabIds [tabId]; both rules use resourceTypes RT."""
    return [
        {'id': 2 * tab, 'priority': 1, 'action': {'type': 'block'},
         'condition': {'tabIds': [tab], 'resourceTypes': list(RT)}},
        {'id': 2 * tab + 1, 'priority': 2, 'action': {'type': 'allow'},
         'condition': {'urlFilter': '|chrome-extension://%s/' % EXT_ID, 'resourceTypes': list(RT)}},
    ]


class FakeDnr:
    def __init__(self):
        self.rules = {}
        self.fail_update = False
        self.corrupt_read = False

    def update_session_rules(self, add=(), remove_ids=()):
        if self.fail_update:
            raise RuntimeError('injected updateSessionRules failure')
        for i in remove_ids:
            self.rules.pop(i, None)
        for r in add:
            self.rules[r['id']] = r

    def get_session_rules(self):
        rs = [dict(r) for r in self.rules.values()]
        if self.corrupt_read and rs:
            rs[0] = dict(rs[0], priority=99)
        return sorted(rs, key=lambda r: r['id'])

    def evaluate(self, tab, url, rtype):
        best = None
        for r in self.rules.values():
            c = r['condition']
            if 'tabIds' in c and tab not in c['tabIds']:
                continue
            if rtype not in c['resourceTypes']:
                continue
            uf = c.get('urlFilter')
            if uf and not (uf.startswith('|') and url.startswith(uf[1:])):
                continue
            if best is None or r['priority'] > best['priority']:
                best = r
        return best['action']['type'] if best else 'noRule'


class Viewer:
    """Fail-closed installation (B L1352; H: update, read back, verify, then {installed: true})."""

    def __init__(self, dnr):
        self.dnr, self.frames, self.events = dnr, set(), []

    def open(self, tab):
        want = rules_for(tab)
        try:
            self.dnr.update_session_rules(add=want, remove_ids=[2 * tab, 2 * tab + 1])
        except RuntimeError:
            self.events.append(('installFailed', tab))
            return {'installed': False, 'frame': False}
        got = [r for r in self.dnr.get_session_rules() if r['id'] in (2 * tab, 2 * tab + 1)]
        if got != want:
            self.events.append(('verifyFailed', tab))
            try:
                self.dnr.update_session_rules(remove_ids=[2 * tab, 2 * tab + 1])
            except RuntimeError:
                pass
            return {'installed': False, 'frame': False}
        self.events.append(('installedVerified', tab))
        self.frames.add(tab)
        self.events.append(('frameCreated', tab))
        return {'installed': True, 'frame': True}

    def remove(self, tab, reason):
        """H: on tabs.onRemoved, pagehide or a failed ping."""
        self.frames.discard(tab)
        self.dnr.update_session_rules(remove_ids=[2 * tab, 2 * tab + 1])
        self.events.append(('removed', tab, reason))

    def worker_restart_ping_failed(self, tab):
        """H د5: after a worker restart the viewer's ping fails -> tear the frame down;
        the new worker verifies leftover session rules for tabs without a live frame and removes them."""
        self.frames.discard(tab)
        self.events.append(('frameTornDown', tab, 'pingFailed'))
        leftovers = [r['id'] for r in self.dnr.get_session_rules() if r['id'] // 2 not in self.frames]
        self.dnr.update_session_rules(remove_ids=leftovers)
        self.events.append(('leftoversRemoved', sorted(leftovers)))
