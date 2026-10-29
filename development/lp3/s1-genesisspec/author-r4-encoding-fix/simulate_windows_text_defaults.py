"""Runs a harness script with Python's Windows defaults for text files, on Linux: a text-mode open()
without an explicit encoding uses the ANSI code page (cp1252 on the GitHub windows-2022 runner, as
its traceback shows), and text-mode writes translate '\\n' to '\\r\\n'. Explicit encodings, explicit
newline arguments and binary mode are untouched. subprocess text mode without an encoding also uses
the locale encoding on Windows; it is patched the same way. Simulation only, not a Windows run.
Python's UTF-8 mode default is also undone (see below).
Usage: simulate_windows_text_defaults.py HARNESS ARGS..."""
import builtins, io, runpy, subprocess, sys

ANSI = 'cp1252'
orig_open = io.open

def win_open(file, mode='r', buffering=-1, encoding=None, errors=None, newline=None, closefd=True, opener=None):
    if 'b' not in mode:
        if encoding is None or encoding == 'locale':
            encoding = ANSI
        if any(m in mode for m in 'wax') and newline is None:
            newline = '\r\n'
    return orig_open(file, mode, buffering, encoding, errors, newline, closefd, opener)

builtins.open = win_open
io.open = win_open
# Without UTF-8 mode (the Windows default) io.text_encoding(None) is 'locale'; this Linux Python may
# run in UTF-8 mode, where it would return 'utf-8' and hide the locale default from pathlib.
orig_text_encoding = io.text_encoding
io.text_encoding = lambda encoding, stacklevel=2: 'locale' if encoding is None else encoding
orig_popen_init = subprocess.Popen.__init__

def popen_init(self, *a, **k):
    if (k.get('text') or k.get('universal_newlines')) and k.get('encoding') is None:
        k['encoding'] = ANSI
    return orig_popen_init(self, *a, **k)

subprocess.Popen.__init__ = popen_init
if sys.argv[1:2] == ['--no-resource']:
    # Windows has no 'resource' module: the CLI harness then takes its no-RLIMIT_AS code path.
    sys.modules['resource'] = None
    sys.argv = sys.argv[1:]
sys.argv = sys.argv[1:]
runpy.run_path(sys.argv[0], run_name='__main__')
