"""The append-only rule of the consensus lock. No network or git remote is used.
Run: python3 -m unittest discover -s deploy/ci -v
"""
import hashlib
import unittest
from unittest import mock

import check_lock_append_only as c

ONE = 'history n=1 kind=base rules=1 protocol=2 protocolMin=2 sources=a note="base"\nfile aa x.rs\nwire 01 v2/hello\n'
TWO = ONE + 'history n=2 kind=neutral rules=1 protocol=2 protocolMin=2 sources=b note="refactor"\nfile bb x.rs\nwire 01 v2/hello\n'


class AppendOnly(unittest.TestCase):
    def test_a_new_lock_and_an_appended_line_pass(self):
        self.assertIsNone(c.check(None, ONE))
        self.assertIsNone(c.check(ONE, ONE))
        self.assertIsNone(c.check(ONE, TWO))
        self.assertIsNone(c.check(ONE, ONE.replace('file aa', 'file cc')), 'file lines are rewritten by re-pin')

    def test_an_edited_removed_or_reordered_history_line_fails(self):
        self.assertIn('line 1 differs', c.check(ONE, ONE.replace('note="base"', 'note="edited"')))
        self.assertIn('line 2 of the base was removed', c.check(TWO, ONE))
        self.assertIn('removed it', c.check(ONE, None))
        swapped = TWO.replace('history n=1', 'history n=X').replace('history n=2', 'history n=1').replace('history n=X', 'history n=2')
        self.assertIsNotNone(c.check(TWO, swapped))
        self.assertIsNotNone(c.check('# no history\n', ONE))

    def test_wire_lines_follow_the_golden_rules_across_the_base(self):
        line = 'history n=2 kind=wire rules=1 protocol={p} protocolMin={m} sources=a note="wire"\n'
        def head(p, m, wires):
            return ONE.replace('wire 01 v2/hello\n', '') + line.format(p=p, m=m) + ''.join(f'wire {sha} {key}\n' for key, sha in wires)
        self.assertIsNone(c.check(ONE, head(2, 2, [('v2/hello', '01')])))
        self.assertIn('v2/hello (v2 >= protocolMin=2) changed', c.check(ONE, head(2, 2, [('v2/hello', '02')])))
        self.assertIn('changed or was removed', c.check(ONE, head(2, 2, [])))
        self.assertIn('new wire key v2/ping at v2', c.check(ONE, head(2, 2, [('v2/hello', '01'), ('v2/ping', '03')])))
        self.assertIn('new wire key v3/ping at v3', c.check(ONE, head(2, 2, [('v2/hello', '01'), ('v3/ping', '03')])))
        self.assertIsNone(c.check(ONE, head(3, 2, [('v2/hello', '01'), ('v3/ping', '03')])))
        self.assertIsNone(c.check(ONE, head(3, 3, [('v3/ping', '03')])), 'raising protocolMin retires v2')
        self.assertIn('malformed wire line', c.check(ONE, head(2, 2, [('hello', '01')])))

    def test_the_tree_passes_against_its_own_head(self):
        self.assertEqual(c.main(['--base', 'HEAD']), 0)
        self.assertEqual(c.main(['--base', 'refs/heads/no-such-branch-for-this-test']), 2)



# The one-time reset (removal plan §3) of a retired lock: full lines like re-pin writes, and a note with a
# non-ASCII character so that the exemption is armed by the raw bytes, not by locale-decoded text.
K = 'rules=2 maxFork=3 spec=2 protocol=2 protocolMin=2'
SVD = 'sources=S0 vectors=V0 deps=D0'
WIRE_LINES = 'wire 0a v2/get-range\nwire 0b v2/hello\nwire 0c v2/tx\n'
MANIFEST = 'file f1 a/x.rs\nfile f2 b/y.rs\ndep d 1.0 registry c0\n' + WIRE_LINES
RETIRED = ('# header line with \u00a7 2.3\n'
           'history n=1 kind=base rules=1 maxFork=2 spec=1 protocol=2 protocolMin=2 sources=S1 vectors=V0 deps=D0 wire=W0 ref=PR5 note="old base"\n'
           f'history n=2 kind=bump {K} sources=S2 vectors=V0 deps=D0 wire=W0 ref=PR6 note="the bump"\n'
           f'history n=3 kind=neutral {K} {SVD} wire=W0 gate=fork:3 ref=PR7 note="cap raise \u00a7 1.4"\n' + MANIFEST)
RESET_LINE = f'history n=1 kind=base {K} {SVD} wire=W1 ref=Phase-8 note="fresh v3 network genesis"\n'
RESET = ('# header line with \u00a7 2.3\n' + RESET_LINE + MANIFEST).replace('wire 0b v2/hello', 'wire 1b v2/hello').replace('wire 0a v2/get-range', 'wire 1a v2/get-range')
APPENDED = RESET.replace(RESET_LINE, RESET_LINE + f'history n=2 kind=neutral {K} sources=S9 vectors=V0 deps=D0 wire=W1 ref=Phase-10 note="refactor"\n')


def raw(text):
    return text.encode('utf-8')


def sha(text):
    return hashlib.sha256(raw(text)).hexdigest()


class OneTimeReset(unittest.TestCase):
    def check(self, base, head):
        return c.check(base, head, None if base is None else raw(base))

    def armed(self):
        return mock.patch.object(c, 'RETIRED_BASE_LOCK_SHA256', sha(RETIRED))

    def test_the_reset_of_the_retired_lock_passes_only_while_armed(self):
        with self.armed():
            self.assertIsNone(self.check(RETIRED, RESET))
            self.assertTrue(c.resets(RETIRED, RESET, raw(RETIRED)))
            self.assertIn('line 1 differs', c.check(RETIRED, RESET), 'no raw bytes: not armed')
            other = RETIRED.replace('old base', 'old bass')
            self.assertIn('line 1 differs', self.check(other, RESET.replace('old base', 'old bass')), 'one byte differs: not armed')
            self.assertIn('line 1 differs', c.check(RETIRED, RESET, raw(RETIRED) + b'\n'))
        self.assertIn('line 1 differs', self.check(RETIRED, RESET), 'the real constant does not match the fixture')

    def test_the_exemption_hashes_raw_bytes(self):
        # The fixture's note is non-ASCII: a Latin-1 re-encoding of the same text does not arm it.
        with self.armed():
            self.assertIsNotNone(c.check(RETIRED, RESET, RETIRED.encode('latin-1')))
            self.assertIsNone(c.check(RETIRED, RESET, raw(RETIRED)))

    def test_anything_but_a_neutral_single_base_line_is_refused(self):
        def refused(head, why):
            with self.armed():
                got = self.check(RETIRED, head)
            self.assertIsNotNone(got, why)
            self.assertIn('not the one-time reset of the retired lock', got, why)
            return got
        line = RESET_LINE.rstrip('\n')
        two = RESET.replace(RESET_LINE, RESET_LINE + RESET_LINE.replace('n=1 kind=base', 'n=2 kind=neutral').replace('S0', 'S8'))
        self.assertIn('exactly one history line', refused(two, 'two lines'))
        self.assertIn('exactly one history line', refused(RESET.replace(RESET_LINE, ''), 'no line'))
        self.assertIn('n=1 kind=base', refused(RESET.replace('kind=base', 'kind=neutral'), 'kind=neutral'))
        self.assertIn('n=1 kind=base', refused(RESET.replace('history n=1', 'history n=2'), 'n=2'))
        self.assertIn('rules=3', refused(RESET.replace(line, line.replace('rules=2', 'rules=3')), 'rules'))
        self.assertIn('spec=1', refused(RESET.replace(line, line.replace('spec=2', 'spec=1')), 'spec'))
        self.assertIn('maxFork=2', refused(RESET.replace(line, line.replace('maxFork=3', 'maxFork=2')), 'maxFork'))
        self.assertIn('protocolMin=3', refused(RESET.replace(line, line.replace('protocolMin=2', 'protocolMin=3')), 'protocolMin'))
        self.assertIn('protocol=3', refused(RESET.replace(line, line.replace(' protocol=2', ' protocol=3')), 'protocol'))
        self.assertIn('no gate', refused(RESET.replace(line, line.replace('wire=W1', 'wire=W1 gate=fork:4')), 'gate'))
        self.assertIn('sources=S7', refused(RESET.replace('sources=S0', 'sources=S7'), 'sources'))
        self.assertIn('vectors=V7', refused(RESET.replace('vectors=V0', 'vectors=V7'), 'vectors'))
        self.assertIn('deps=D7', refused(RESET.replace('deps=D0', 'deps=D7'), 'deps'))
        self.assertIn('sources=None', refused(RESET.replace(' sources=S0', ''), 'no sources field'))
        self.assertIn('file lines', refused(RESET.replace('file f1 a/x.rs', 'file f9 a/x.rs'), 'a changed file line'))
        self.assertIn('file lines', refused(RESET.replace('file f2 b/y.rs\n', 'file f2 b/y.rs\nfile f3 c/z.rs\n'), 'an added file line'))
        self.assertIn('file lines', refused(RESET.replace('file f2 b/y.rs\n', ''), 'a removed file line'))
        self.assertIn('wire keys', refused(RESET.replace('wire 0c v2/tx\n', 'wire 0c v2/tx\nwire 0d v2/ping\n'), 'a wire key added'))
        self.assertIn('wire keys', refused(RESET.replace('wire 0c v2/tx\n', ''), 'a wire key removed'))
        self.assertIn('wire key v2/tx changed', refused(RESET.replace('wire 0c v2/tx', 'wire 1c v2/tx'), 'another key\'s sha moved'))
        self.assertIn('malformed wire line', refused(RESET.replace('wire 0c v2/tx', 'wire 0c tx'), 'malformed wire'))
        # A note that only mentions a field does not count as that field.
        with self.armed():
            self.assertIsNone(self.check(RETIRED, RESET.replace('fresh v3 network genesis', 'fresh v3 genesis, no gate=fork:9 sources=S7')))
            self.assertIn('removed it', self.check(RETIRED, None), 'a deleted lock is still refused')

    def test_an_ordinary_append_to_the_retired_lock_still_passes_while_armed(self):
        line = f'history n=4 kind=neutral {K} sources=S5 vectors=V0 deps=D0 wire=W0 ref=PR8 note="refactor"\n'
        appended = RETIRED.replace(MANIFEST, line + MANIFEST)
        with self.armed():
            self.assertIsNone(self.check(RETIRED, appended))
            self.assertFalse(c.resets(RETIRED, appended, raw(RETIRED)))
            self.assertIsNone(self.check(RETIRED, RETIRED))
            self.assertIn('v2/hello (v2 >= protocolMin=2) changed', self.check(RETIRED, appended.replace('wire 0b v2/hello', 'wire 1b v2/hello')),
                          'an append keeps the wire rules')

    def test_after_the_reset_the_ordinary_rule_applies(self):
        with self.armed():
            self.assertIsNone(self.check(RESET, RESET))
            self.assertIsNone(self.check(RESET, APPENDED))
            self.assertIn('line 1 differs', self.check(RESET, RETIRED))
            self.assertIn('line 1 differs', self.check(RESET, RESET.replace('fresh v3', 'edited')))

    def test_the_constant_is_a_sha256(self):
        self.assertRegex(c.RETIRED_BASE_LOCK_SHA256, r'^[0-9a-f]{64}$')


if __name__ == '__main__':
    unittest.main()
