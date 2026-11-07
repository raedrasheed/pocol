"""Public framing and inherited-terminal controls; never use actual credentials."""
import base64
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import unittest
import zlib

import ssh_terminal as terminal


class FrameTests(unittest.TestCase):
    def frame(self):
        self.displayed = []
        return terminal.Frame(b'BEGIN_TOKEN', b'END_TOKEN', self.displayed.append)

    def test_prompt_is_shown_immediately_without_waiting_for_newline(self):
        value = self.frame(); value.feed(b'Sudo password: ')
        self.assertEqual(b''.join(self.displayed), b'Sudo password: ')
        self.assertFalse(value.started)

    def test_split_markers_and_windows_newlines_preserve_only_the_report(self):
        value = self.frame()
        for part in [b'Password: BE', b'GIN_', b'TOKEN\r\n{"result":', b'"PASS"}\r\nEN', b'D_TOKEN\r\n']:
            value.feed(part)
        self.assertEqual(json.loads(value.finish()), {'result': 'PASS'})
        self.assertEqual(b''.join(self.displayed), b'Password: \r\n')

    def test_prefix_and_suffix_are_never_part_of_saved_payload(self):
        value = self.frame(); value.feed(b'PRIVATE_BANNER\nBEGIN_TOKEN\n{}\nEND_TOKEN\nPRIVATE_TRAILER')
        self.assertEqual(value.finish(), b'{}')
        self.assertNotIn(b'PRIVATE', value.finish())

    def test_missing_or_truncated_frame_has_fixed_diagnostic(self):
        for content in [b'PRIVATE_DIAGNOSTIC', b'BEGIN_TOKEN{"private":"not returned"}']:
            value = self.frame(); value.feed(content)
            with self.assertRaises(terminal.TerminalError) as error: value.finish()
            self.assertNotIn('PRIVATE', str(error.exception))
            self.assertNotIn('not returned', str(error.exception))

    def test_oversized_public_payload_is_refused(self):
        value = self.frame()
        with self.assertRaises(terminal.TerminalError):
            value.feed(b'BEGIN_TOKEN' + b'x' * (terminal.MAX_REPORT_BYTES + 1) + b'END_TOKEN')

    def test_runaway_terminal_output_is_bounded(self):
        value = self.frame()
        with self.assertRaises(terminal.TerminalError): value.feed(b'x' * (terminal.MAX_TERMINAL_BYTES + 1))


class Process:
    def __init__(self, content, status=0):
        self.stdout = io.BytesIO(content)
        self.returncode = status
    def wait(self, timeout): return self.returncode
    def poll(self): return self.returncode
    def terminate(self): raise AssertionError('Do not terminate an already completed child')


class ProcessTests(unittest.TestCase):
    def test_timeout_terminates_only_the_owned_ssh_process(self):
        released = threading.Event()
        class Stream:
            def read(self, size):
                released.wait(5)
                return b''
            def close(self): pass
        class Hanging:
            stdout = Stream()
            returncode = None
            terminated = False
            def wait(self, timeout):
                if self.returncode is None: raise subprocess.TimeoutExpired(['ssh'], timeout)
                return self.returncode
            def poll(self): return self.returncode
            def terminate(self):
                self.terminated = True; self.returncode = -15; released.set()
        child = Hanging()
        with self.assertRaisesRegex(terminal.TerminalError, 'timed out'):
            terminal.run(['ssh'], b'BEGIN', b'END', timeout=1, console=lambda data: None,
                         popen=lambda *args, **kwargs: child)
        self.assertTrue(child.terminated)

    def test_invalid_timeout_does_not_create_a_child(self):
        for timeout in [0, 601]:
            with self.assertRaises(terminal.TerminalError):
                terminal.run(['ssh'], b'BEGIN', b'END', timeout=timeout,
                             popen=lambda *args, **kwargs: self.fail('Must not start SSH'))

    def test_stdin_and_stderr_are_inherited_without_password_input_or_capture(self):
        calls = []
        def popen(command, **options):
            calls.append((command, options))
            return Process(b'Sudo password: \nBEGIN{}END')
        displayed = []
        result = terminal.run(['ssh', '-tt'], b'BEGIN', b'END', console=displayed.append, popen=popen)
        self.assertEqual(result.stdout, b'{}')
        self.assertEqual(calls[0][1], {'stdin': None, 'stdout': subprocess.PIPE, 'stderr': None, 'bufsize': 0})
        self.assertEqual(b''.join(displayed), b'Sudo password: \n')

    def test_failed_preflight_exit_two_keeps_public_failure_json(self):
        result = terminal.run(['ssh'], b'BEGIN', b'END', console=lambda data: None,
                              popen=lambda *args, **kwargs: Process(b'BEGIN{"result":"FAIL"}END', 2))
        self.assertEqual(result.returncode, 2)
        self.assertEqual(json.loads(result.stdout), {'result': 'FAIL'})

    def test_public_program_is_transported_byte_for_byte_and_fits_windows(self):
        program = Path(__file__).with_name('preflight.py').read_bytes()
        wrapper, begin, end = terminal.command_program(program, 'a' * 32)
        encoded = re.search(r"b64decode\('([A-Za-z0-9+/=]+)'\)", wrapper).group(1)
        self.assertEqual(zlib.decompress(base64.b64decode(encoded)), program)
        self.assertLess(len(wrapper), 20000)
        self.assertNotEqual(begin, end)
        self.assertIn('scope[\'main\']()', wrapper)

    def test_real_child_executes_public_wrapper_and_returns_only_json(self):
        program = b'def main():\n import json\n print(json.dumps({"result":"PASS"}))\n return 0\n'
        wrapper, begin, end = terminal.command_program(program, 'b' * 32)
        result = terminal.run([sys.executable, '-B', '-c', wrapper], begin, end, console=lambda data: None)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(json.loads(result.stdout), {'result': 'PASS'})

    @unittest.skipIf(os.name == 'nt', 'POSIX pseudo-terminal control; inherited-handle controls run on Windows')
    def test_real_terminal_password_input_bypasses_parent_and_is_not_echoed_or_saved(self):
        import pty
        master, slave = pty.openpty()
        shown, ready = [], threading.Event()
        secret = b'synthetic-test-password'
        child_source = '''import json,sys,termios
fd=sys.stdin.fileno(); saved=termios.tcgetattr(fd); hidden=termios.tcgetattr(fd); hidden[3] &= ~termios.ECHO
termios.tcsetattr(fd,termios.TCSANOW,hidden)
print('Sudo password: ',end='',flush=True)
password=sys.stdin.buffer.readline().strip()
termios.tcsetattr(fd,termios.TCSANOW,saved)
if password != b'synthetic-test-password': raise SystemExit(1)
print('\\nBEGIN'+json.dumps({'result':'PASS'})+'END',flush=True)
'''
        def display(data):
            shown.append(data)
            if b'Sudo password: ' in b''.join(shown): ready.set()
        def owner_input():
            if ready.wait(5): os.write(master, secret + b'\n')
        def popen(command, **options):
            self.assertIsNone(options['stdin'])
            options['stdin'] = slave  # Only the fixture supplies a disposable terminal.
            return subprocess.Popen(command, **options)
        owner = threading.Thread(target=owner_input, daemon=True); owner.start()
        try:
            result = terminal.run([sys.executable, '-c', child_source], b'BEGIN', b'END',
                                  timeout=10, console=display, popen=popen)
            self.assertEqual(json.loads(result.stdout), {'result': 'PASS'})
            self.assertNotIn(secret, result.stdout)
            self.assertNotIn(secret, b''.join(shown))
            self.assertTrue(ready.is_set())
        finally:
            owner.join(timeout=1); os.close(master); os.close(slave)


if __name__ == '__main__':
    unittest.main()
