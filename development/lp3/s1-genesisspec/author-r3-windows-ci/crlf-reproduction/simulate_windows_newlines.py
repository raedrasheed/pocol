"""Runs a harness script as on Windows for text-mode file writes: Python's default newline handling
there translates '\\n' to os.linesep ('\\r\\n') when writing text. Here, on Linux, every text-mode
open for writing gets newline='\\r\\n', which is exactly that translation. Binary writes are untouched.
With --keep DIR, temporary directories are created under DIR and not deleted, so the generated inputs
can be hashed. Usage: simulate_windows_newlines.py [--native] [--keep DIR] HARNESS ARGS..."""
import builtins, io, os, runpy, sys, tempfile

args = sys.argv[1:]
native = args[:1] == ['--native']
if native:
    args = args[1:]
keep = None
if args[:1] == ['--keep']:
    keep, args = args[1], args[2:]
orig_open = io.open

def win_open(file, mode='r', buffering=-1, encoding=None, errors=None, newline=None, closefd=True, opener=None):
    if 'b' not in mode and any(m in mode for m in 'wax') and newline is None:
        newline = '\r\n'
    return orig_open(file, mode, buffering, encoding, errors, newline, closefd, opener)

if not native:
    builtins.open = win_open
    io.open = win_open
if keep:
    os.makedirs(keep, exist_ok=True)
    real_mkdtemp = tempfile.mkdtemp
    tempfile.mkdtemp = lambda *a, **k: real_mkdtemp(dir=keep, prefix=k.get('prefix', 'tmp'))
    tempfile.TemporaryDirectory.cleanup = lambda self: None
    tempfile.TemporaryDirectory._cleanup = classmethod(lambda cls, *a, **k: None)
    import shutil
    shutil.rmtree = lambda *a, **k: None
    real_remove = os.remove
    os.remove = lambda p, *a, **k: None if not str(p).endswith('.out') else real_remove(p)
    os.rmdir = lambda *a, **k: None
sys.argv = args
runpy.run_path(args[0], run_name='__main__')
