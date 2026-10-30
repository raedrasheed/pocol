"""Re-run the unchanged draft 0.2 checks without writing into m1-draft-0.2 (C02 aid).

Usage: <python> m1-draft-0.3/tools/rerun_02.py
Output: m1-draft-0.3/results/rerun-0.2/results/run-results.json (+ fixture-hashes.json).

Why this wrapper exists: the embedded interpreter in coordination/runtime
uses a ._pth file, so neither the script directory nor the working directory
is on sys.path and `python run_checks.py` cannot import its sibling modules.
This wrapper puts ONLY m1-draft-0.2/tools on sys.path (removing this
directory, whose draft 0.3 modules share names), and redirects the results
directory, so the committed 0.2 results stay untouched. The 0.2 code itself
is not modified.
"""

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TOOLS02 = ROOT / 'm1-draft-0.2' / 'tools'
OUT = HERE.parent / 'results' / 'rerun-0.2'

sys.path[:] = [p for p in sys.path if p and Path(p).resolve() != HERE]
sys.path.insert(0, str(TOOLS02))

import run_checks                                    # noqa: E402  (draft 0.2, unchanged)

if Path(run_checks.__file__).resolve().parent != TOOLS02.resolve():
    sys.exit('refusing to run: run_checks resolved to %s' % run_checks.__file__)
OUT.mkdir(parents=True, exist_ok=True)
run_checks.PKG = OUT                                 # main() writes PKG / 'results'
sys.exit(run_checks.main())
