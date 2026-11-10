"""The append-only rule of the consensus lock. No network or git remote is used.
Run: python3 -m unittest discover -s deploy/ci -v
"""
import unittest

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


if __name__ == '__main__':
    unittest.main()
