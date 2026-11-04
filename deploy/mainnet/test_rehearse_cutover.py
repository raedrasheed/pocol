#!/usr/bin/env python3
"""rehearse_cutover.cutover refuses a pocold that exits 0 without writing the cutover report.

The deployed pocold predates the cutover command and answers it with its usage and exit status 0,
so a rehearsal pointed at it (a wrong POCOLD_BIN_DIR) must fail with that reason. No binary runs here.
"""
import json, shutil, subprocess, tempfile, unittest
from pathlib import Path
from unittest import mock
import rehearse_cutover as rc

class Cutover(unittest.TestCase):
    def setUp(self):
        self.d = Path(tempfile.mkdtemp()); self.addCleanup(shutil.rmtree, self.d)
        self.out = self.d / 'new'

    def run_with(self, returncode, stderr='', report=None):
        def fake(argv, **kw):
            self.assertEqual(argv[1:3], ['cutover', '--network'])
            if report is not None:
                self.out.mkdir(); (self.out / 'cutover-report.json').write_text(json.dumps(report), encoding='utf-8')
            return subprocess.CompletedProcess(argv, returncode, '', stderr)
        with mock.patch.object(rc.subprocess, 'run', fake):
            return rc.cutover(self.d / 'old', self.d / 'data', 5, self.d / 'new.json', self.out, ['--state-store'])

    def test_a_build_without_the_cutover_command_is_named(self):
        with self.assertRaises(AssertionError) as e: self.run_with(0, 'usage:\n  pocold init-devnet ...')
        self.assertIn('without the cutover command', str(e.exception))
        self.assertIn('POCOLD_BIN_DIR', str(e.exception))

    def test_a_failed_cutover_reports_its_error(self):
        with self.assertRaises(AssertionError) as e: self.run_with(1, 'error: --height S')
        self.assertEqual(str(e.exception), 'cutover failed: error: --height S')

    def test_a_cutover_past_the_time_limit_is_named(self):
        def slow(argv, **kw): raise subprocess.TimeoutExpired(argv, kw['timeout'], output=b'', stderr=b'rebuilding')
        with mock.patch.object(rc.subprocess, 'run', slow), self.assertRaises(AssertionError) as e:
            rc.cutover(self.d / 'old', self.d / 'data', 5, self.d / 'new.json', self.out, [], timeout=7)
        self.assertIn('did not finish within 7 s', str(e.exception))
        self.assertIn(str(self.d / 'new.log'), str(e.exception))
        self.assertIn('rebuilding', (self.d / 'new.log').read_text(encoding='utf-8'))

    def test_each_run_keeps_its_output_next_to_its_directory(self):
        with self.assertRaises(AssertionError): self.run_with(1, 'error: --height S')
        self.assertIn('error: --height S', (self.d / 'new.log').read_text(encoding='utf-8'))
        self.assertFalse(self.out.exists())

    def test_the_report_is_returned(self):
        self.assertEqual(self.run_with(0, report={'allocTotal': '1'}), {'allocTotal': '1'})

if __name__ == '__main__':
    unittest.main()
