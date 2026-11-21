"""The mutation controls compile a copy of this crate in another directory. The crate also reads files outside
itself by relative path (the evm-crypto path dependencies of Cargo.toml, the lp1-node modules src/lib.rs shares,
development vectors that unit tests include); in the copy those paths would point nowhere. anchor_outside() makes
each of them absolute, to the same file the crate itself reads. Paths inside the crate stay relative, so a copy
compiles its own (mutated) sources.

Each copy is its own package, so cargo builds its library, binaries and test binaries anew (several hundred MB
per copy) and nothing ever reuses them. outputs_before() and release_outputs() delete exactly those outputs once
the copy's result is recorded; dependency builds stay for the next copy. They change no build, run or kill
condition.
"""
import json, pathlib, re, shutil, subprocess

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


SUBDIRS = ('deps', 'examples', 'incremental', '.fingerprint', 'build')


def _target(env, copy):
    given = env.get('CARGO_TARGET_DIR')
    return pathlib.Path(given).resolve() if given else pathlib.Path(copy).resolve() / 'target'


def _entries(target):
    if not target.is_dir(): return set()
    found = set()
    for profile in target.iterdir():
        if not (profile / 'deps').is_dir(): continue
        for folder in (profile, *(profile / s for s in SUBDIRS)):
            if folder.is_dir(): found.update(str(p) for p in folder.iterdir())
    return found


def outputs_before(env, copy):
    """Call before the copy is first built, with the environment cargo gets: what release_outputs() keeps."""
    target = _target(env, copy)
    return target, _entries(target)


def release_outputs(env, copy, before):
    """Call after the copy's logs and result are written. A target directory inside the copy is removed whole.
    In a shared target directory only entries that are new since outputs_before() and named after this copy's
    own targets (library, binaries, tests, examples) or its package are removed; everything that existed before,
    including the original crate's outputs, and every dependency stays."""
    target, kept = before; copy = pathlib.Path(copy).resolve()
    if target == copy / 'target' or copy in target.parents:
        if target.exists(): shutil.rmtree(target)
        return
    meta = subprocess.run(['cargo', 'metadata', '--offline', '--locked', '--no-deps', '--format-version', '1',
                           '--manifest-path', str(copy / 'Cargo.toml')], env=env, capture_output=True, check=True)
    package = json.loads(meta.stdout)['packages'][0]
    locked = {n.replace('-', '_') for n in re.findall(r'^name = "([^"]*)"$', (copy / 'Cargo.lock').read_text(), re.M)}
    names = set()
    for t in package['targets']:
        if t['name'].replace('-', '_') not in locked: names |= {t['name'], t['name'].replace('-', '_')}
    assert names, package['name']
    own = re.compile(r'^(?:lib)?(?:' + '|'.join(map(re.escape, sorted(names))) + r')(?:-[0-9a-z]+)?(?:\.[0-9A-Za-z_]+)*$')
    unit = re.compile('^' + re.escape(package['name']) + r'-[0-9a-z]+$')
    for entry in sorted(_entries(target) - kept):
        p = pathlib.Path(entry)
        if not (own.match(p.name) or (p.parent.name in ('.fingerprint', 'build') and unit.match(p.name))): continue
        if p.is_dir() and not p.is_symlink(): shutil.rmtree(p)
        else: p.unlink()
