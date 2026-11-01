"""SR-model for M1 draft 0.11: C24 repair of m1-draft-0.10/tools/sr_ref.py.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

Drop-in module: `import sr_ref` from this directory gives the 0.10 model with the C24
repairs applied, and the World runtime itself uses them.

How:
1. The 0.10 source file is loaded read-only under the private module name
   'sr_ref_010_base'. Nothing in m1-draft-0.10 is written or edited.
2. The strict helpers below replace the module-level names in that loaded module.
   World methods resolve these names through the module globals, so World.epochs,
   lookup, _name/record_name and _observe all use the repaired helpers.
3. World.epochs is replaced, because it also reads `bootMs` (the strict value rule).
4. Every public name of the repaired module is re-exported from this module.

C24 (coordination/review-001/REVIEW-0.10.md):
  (a) `re.match(...$)` accepted a name followed by '\\n'. Every name is now checked with
      re.fullmatch on the exact string: no trimming, no normalization.
  (b) bool passed `isinstance(x, int)` and `True == 1`. Every numeric field now requires
      `type(x) is int`.

Policy for integral floats (P-C24-2): REJECTED everywhere. Rationale: the baseline
declares u64/u32 integer ranges (browser.md:539), which a JS Number carries exactly only up
to 2^53-1, and the fmt 2 codec that fixes the stored numeric representation is row E4
(pending). Accepting 6.0 would rely on Python float==int equality, which is the accidental
behaviour C24 rejects. Nothing else changes: not the gates, scheduler, retries,
checkpoints, adapters, sequence allocation or any owner policy.
"""

import importlib.util
import json
import re
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_ROOT = _HERE.parent.parent
SOURCE_010 = _ROOT / 'm1-draft-0.10' / 'tools' / 'sr_ref.py'

MAX_SAFE_INTEGER = 2 ** 53 - 1        # JS Number.MAX_SAFE_INTEGER, used for bootMs (P-C24-4)


def load_010(module_name):
    """Execute the 0.10 source as a fresh, independent module object (read-only)."""
    spec = importlib.util.spec_from_file_location(module_name, str(SOURCE_010))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def load_pristine_010():
    """An UNPATCHED copy of 0.10, used only to reproduce and preserve the C24 failures."""
    return load_010('sr_ref_010_pristine')


_base = load_010('sr_ref_010_base')

EPOCH_PREFIX = _base.EPOCH_PREFIX
EPOCH_MAX = _base.EPOCH_MAX
SEQ_MAX = _base.SEQ_MAX
EPOCH_BYTES_MAX = _base.EPOCH_BYTES_MAX

_EPOCH_FULL = re.compile(r'pocol:epoch:([0-9a-f]{16})')
_REC_TAIL_FULL = re.compile(r'([0-9a-f]{16}):([0-9a-f]{8})')


class FieldTypeError(TypeError, ValueError):
    """A numeric field of the wrong type. It subclasses ValueError, so callers written for
    0.10 (which catch ValueError) still see a refusal."""


def is_int(x):
    """JSON/JS integer for this reference: exactly int. Not bool, not float (P-C24-1/2)."""
    return type(x) is int


def _u(x, lo, hi, what):
    if not is_int(x):
        raise FieldTypeError('%s must be an integer (not bool/float/str): %r' % (what, x))
    if not lo <= x <= hi:
        raise ValueError('%s out of range: %r' % (what, x))
    return x


# ------------------------------------------------------------------ strict E1 helpers

def epoch_name(E):
    _u(E, 1, EPOCH_MAX, 'epoch')
    return EPOCH_PREFIX + format(E, '016x')


def parse_epoch_name(name):
    if type(name) is not str:
        return None
    m = _EPOCH_FULL.fullmatch(name)
    if not m:
        return None
    E = int(m.group(1), 16)
    return E if E >= 1 else None


def record_name(net, addr, E, seq):
    _u(E, 1, EPOCH_MAX, 'epoch')
    _u(seq, 0, SEQ_MAX, 'seq')
    return _base.key_prefix(net, addr) + format(E, '016x') + ':' + format(seq, '08x')


def parse_record_name(name, net, addr):
    if type(name) is not str:
        return None
    p = _base.key_prefix(net, addr)
    if not name.startswith(p):
        return None
    m = _REC_TAIL_FULL.fullmatch(name[len(p):])
    if not m:
        return None
    E, seq = int(m.group(1), 16), int(m.group(2), 16)
    return (E, seq) if E >= 1 else None


def valid_record(ver, v):
    """Abstract fmt-2 value check (the byte codec is still E4). Numeric fields are exact
    ints within range and equal to the name's version; tomb is exactly bool; dict str->str."""
    if type(v) is not dict:
        return False
    fmt, ep, sq, tomb, d = v.get('fmt'), v.get('epoch'), v.get('seq'), v.get('tomb'), v.get('dict')
    if not (is_int(fmt) and fmt == 2):
        return False
    if not (is_int(ep) and 1 <= ep <= EPOCH_MAX and ep == ver[0]):
        return False
    if not (is_int(sq) and 0 <= sq <= SEQ_MAX and sq == ver[1]):
        return False
    if type(tomb) is not bool:
        return False
    if type(d) is not dict or not all(type(k) is str and type(x) is str for k, x in d.items()):
        return False
    return not (tomb and d)


def epoch_value_ok(v):
    return (type(v) is dict and set(v) == {'bootMs'} and is_int(v['bootMs'])
            and -MAX_SAFE_INTEGER <= v['bootMs'] <= MAX_SAFE_INTEGER
            and len(json.dumps(v, separators=(',', ':'))) <= EPOCH_BYTES_MAX)


def strict_boot_ms(v):
    """bootMs as used by the window and lateGens, or None if the stored value is not a valid
    {bootMs}. Such an item still counts in EN and M (its NAME is valid) but contributes no
    window time. This is the 0.10 treatment of non-dict values, extended to bool, float and
    out-of-range values (P-C24-3; finding RF-8)."""
    if type(v) is dict and 'bootMs' in v and is_int(v['bootMs']) and -MAX_SAFE_INTEGER <= v['bootMs'] <= MAX_SAFE_INTEGER:
        return v['bootMs']
    return None


def _epochs_strict(self):
    out = []
    for n, v in self.be.items.items():
        if type(n) is not str or not n.startswith(EPOCH_PREFIX):
            continue
        E = parse_epoch_name(n)
        if E is None and 'epochNonceNames' in self.faults:          # fault-only form, unchanged from 0.10
            E = parse_epoch_name(n[:len(EPOCH_PREFIX) + 16])
        if E is None:
            continue
        out.append((E, n, strict_boot_ms(v)))
    return out


# ------------------------------------------------------------------ install into the loaded 0.10 module

_base.epoch_name = epoch_name
_base.parse_epoch_name = parse_epoch_name
_base.record_name = record_name
_base.parse_record_name = parse_record_name
_base.valid_record = valid_record
_base.epoch_value_ok = epoch_value_ok
_base.World.epochs = _epochs_strict
REPAIRED_HELPERS = ('epoch_name', 'parse_epoch_name', 'record_name', 'parse_record_name', 'valid_record', 'epoch_value_ok', 'World.epochs')

# Re-export everything public from the repaired module (lookup, World, Backend, constants,
# late_gens, site_admission, FAULTS, ...). lookup/World resolve the helpers above via _base globals.
for _k in dir(_base):
    if not _k.startswith('__') and _k not in globals():
        globals()[_k] = getattr(_base, _k)
World = _base.World
lookup = _base.lookup
BASE_MODULE = _base


def self_check():
    """Proof that the runtime is wired to the repaired helpers (not just wrapper functions)."""
    return {
        'worldEpochsIsStrict': _base.World.epochs is _epochs_strict,
        'baseParseEpochIsStrict': _base.parse_epoch_name is parse_epoch_name,
        'baseParseRecordIsStrict': _base.parse_record_name is parse_record_name,
        'baseValidRecordIsStrict': _base.valid_record is valid_record,
        'baseRecordNameIsStrict': _base.record_name is record_name,
        'baseEpochNameIsStrict': _base.epoch_name is epoch_name,
        'lookupGlobalsPatched': _base.lookup.__globals__['parse_record_name'] is parse_record_name
                                and _base.lookup.__globals__['valid_record'] is valid_record,
        'sourceFile': str(SOURCE_010),
    }


if __name__ == '__main__':      # not used by the runner
    print(json.dumps(self_check(), indent=1))
    sys.exit(0)
