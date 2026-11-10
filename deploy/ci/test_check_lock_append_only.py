"""The append-only rule of the consensus lock. No network or git remote is used.
Run: python3 -m unittest discover -s deploy/ci -v
"""
import unittest

import check_lock_append_only as c

ONE = 'history n=1 kind=base rules=1 sources=a note="base"\nfile aa x.rs\n'
TWO = ONE + 'history n=2 kind=neutral rules=1 sources=b note="refactor"\nfile bb x.rs\n'


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

    def test_the_tree_passes_against_its_own_head(self):
        self.assertEqual(c.main(['--base', 'HEAD']), 0)
        self.assertEqual(c.main(['--base', 'refs/heads/no-such-branch-for-this-test']), 2)


if __name__ == '__main__':
    unittest.main()
