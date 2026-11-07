"""Run one SSH sudo terminal while retaining only its framed public report.

stdin belongs to OpenSSH and the owner's terminal. Python never asks for, reads
or stores a password. Sudo controls terminal echo during its password prompt.
"""
import base64
import queue
import secrets
import subprocess
import sys
import threading
import time
import zlib

MAX_REPORT_BYTES = 256 * 1024
MAX_TERMINAL_BYTES = 16 * 1024


class TerminalError(ValueError):
    pass


def command_program(program, token):
    """Compress public source to fit Windows command-line limits; no credentials."""
    packed = base64.b64encode(zlib.compress(program, 9)).decode('ascii')
    begin, end = f'POCOL_REPORT_{token}_BEGIN', f'POCOL_REPORT_{token}_END'
    wrapper = (
        'import base64,zlib,sys;'
        "scope={'__name__':'pocol_readonly_preflight'};"
        f"exec(compile(zlib.decompress(base64.b64decode('{packed}')),"
        "'<pocol-readonly-preflight>','exec'),scope);"
        f"print('\\n{begin}',flush=True);"
        "status=scope['main']();"
        f"print('\\n{end}',flush=True);sys.exit(status)"
    )
    return wrapper, begin.encode('ascii'), end.encode('ascii')


def console_write(data):
    # Never save terminal output in an application report or a log.
    binary = getattr(sys.stderr, 'buffer', None)
    if binary is not None:
        binary.write(data); binary.flush()
    else:
        sys.stderr.write(data.decode('utf-8', errors='replace')); sys.stderr.flush()


class Frame:
    def __init__(self, begin, end, console):
        self.begin, self.end, self.console = begin, end, console
        self.pending = bytearray()
        self.report = bytearray()
        self.started = self.complete = False
        self.console_bytes = 0

    def display(self, data):
        if not data:
            return
        self.console_bytes += len(data)
        if self.console_bytes > MAX_TERMINAL_BYTES:
            raise TerminalError('Interactive terminal output exceeded its limit')
        self.console(bytes(data))

    def feed(self, chunk):
        if self.complete:
            self.display(chunk)
            return
        if not self.started:
            self.pending.extend(chunk)
            start = self.pending.find(self.begin)
            if start < 0:
                # Keep only a possible partial marker; show prompts immediately.
                keep = 0
                for size in range(min(len(self.pending), len(self.begin) - 1), 0, -1):
                    if self.pending.endswith(self.begin[:size]):
                        keep = size
                        break
                count = len(self.pending) - keep
                self.display(self.pending[:count])
                del self.pending[:count]
                return
            self.display(self.pending[:start])
            chunk = bytes(self.pending[start + len(self.begin):])
            self.pending.clear()
            self.started = True
        self.report.extend(chunk)
        stop = self.report.find(self.end)
        if stop >= 0:
            if stop > MAX_REPORT_BYTES:
                raise TerminalError('Interactive public report exceeded its size limit')
            remainder = bytes(self.report[stop + len(self.end):])
            del self.report[stop:]
            self.complete = True
            self.display(remainder)
        elif len(self.report) > MAX_REPORT_BYTES + len(self.end):
            raise TerminalError('Interactive public report exceeded its size limit')

    def finish(self):
        if not self.complete:
            self.display(self.pending)
            raise TerminalError('No complete public preflight report returned; inspect the SSH/sudo message in the terminal')
        return bytes(self.report).strip()


def run(command, begin, end, *, timeout=180, console=console_write, popen=subprocess.Popen):
    """Keep password interaction in SSH, show prompts, and bound report/elapsed time."""
    if not 1 <= timeout <= 600:
        raise TerminalError('Invalid interactive SSH timeout')
    # Inherited stdin/stderr are essential. Do not replace stdin with PIPE,
    # request sudo -S, or capture stderr/password prompts into an audit log.
    child = popen(command, stdin=None, stdout=subprocess.PIPE, stderr=None, bufsize=0)
    messages = queue.Queue(maxsize=8)
    stopped = threading.Event()
    deadline = time.monotonic() + timeout
    frame = Frame(begin, end, console)

    def deliver(item):
        while not stopped.is_set():
            try:
                messages.put(item, timeout=0.1)
                return
            except queue.Full:
                pass

    def read_output():
        try:
            while not stopped.is_set():
                content = child.stdout.read(4096)
                deliver(content)
                if not content:
                    break
        except (OSError, ValueError):
            deliver(None)

    reader = threading.Thread(target=read_output, daemon=True)
    reader.start()
    try:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise queue.Empty
            chunk = messages.get(timeout=remaining)
            if chunk is None:
                raise TerminalError('The interactive SSH output could not be read')
            if not chunk:
                break
            frame.feed(chunk)
        status = child.wait(timeout=max(0.01, deadline - time.monotonic()))
        return subprocess.CompletedProcess(command, status, frame.finish(), b'')
    except (queue.Empty, subprocess.TimeoutExpired):
        raise TerminalError('Interactive SSH/sudo timed out; no deployment was performed') from None
    finally:
        stopped.set()
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill(); child.wait(timeout=3)
        child.stdout.close()
        reader.join(timeout=1)


def markers():
    return secrets.token_hex(16)
