"""The mutation controls compile a copy of this crate in another directory. The crate also reads files outside
itself by relative path (the evm-crypto path dependencies of Cargo.toml, the lp1-node modules src/lib.rs shares,
development vectors that unit tests include); in the copy those paths would point nowhere. anchor_outside() makes
each of them absolute, to the same file the crate itself reads. Paths inside the crate stay relative, so a copy
compiles its own (mutated) sources.
"""
import pathlib, re

FORMS = [re.compile(r'(\bpath\s*=\s*")(\.\./[^"]*)(")'),
         re.compile(r'(#\[path\s*=\s*")(\.\./[^"]*)(")'),
         re.compile(r'(include_(?:str|bytes)!\(\s*")(\.\./[^"]*)(")')]


def anchor_outside(root, copy):
    root = pathlib.Path(root).resolve(); copy = pathlib.Path(copy)
    for f in [copy / 'Cargo.toml', *sorted(copy.rglob('*.rs'))]:
        base = (root / f.relative_to(copy)).parent
        def absolute(m):
            target = (base / m.group(2)).resolve()
            if target == root or root in target.parents: return m.group(0)
            assert target.exists(), (str(f), m.group(2), 'outside reference does not exist')
            return m.group(1) + target.as_posix() + m.group(3)
        text = f.read_text(encoding='utf-8'); new = text
        for form in FORMS: new = form.sub(absolute, new)
        if new != text: f.write_text(new, encoding='utf-8')
