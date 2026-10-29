"""Read the immutable source behind historical M1 provenance assertions.

English translations replace working-tree reference prose. Existing exact-literal
and line-number assertions describe the pre-translation revision, not a new test
of the translated wording. Reference reads therefore use the explicitly pinned
Git object. Other inputs continue to come from the current working tree. Missing
Git history is an error, never a fallback to mismatched translated source.
"""
from functools import lru_cache
from pathlib import Path
import subprocess

SOURCE_REVISION = '3b861aaafceecdea722a2a262cfe060d04b04f04'
M1_ROOT = Path(__file__).resolve().parent
REPO_ROOT = M1_ROOT.parent.parent


@lru_cache(maxsize=None)
def _reference_bytes(relative):
    return subprocess.check_output(
        ['git', '-C', str(REPO_ROOT), 'show', SOURCE_REVISION + ':' + relative],
        stderr=subprocess.PIPE,
    )


def source_bytes(path):
    path = Path(path).resolve()
    try:
        relative = path.relative_to(M1_ROOT / 'reference')
    except ValueError:
        return path.read_bytes()
    return _reference_bytes('development/m1/reference/' + relative.as_posix())


def source_text(path):
    return source_bytes(path).decode('utf-8')
