#!/usr/bin/env python3
"""Native double build (gate I1): build the Linux release binaries and the Linux package twice, under different
checkout paths, umasks, TZ, locales, cargo homes and source mtimes, and fail unless every byte is the same.
Standard library only; Linux x86_64; needs git, rustup with the 1.99.1 toolchain and a C compiler (cc).

    python3 deploy/release/double_build.py [--worktree] [--work DIR] [--keep]
        both legs on this machine, one after the other, from fresh copies of this repository's HEAD
        (--worktree: of the working tree, uncommitted and untracked files included), then the comparison
    python3 deploy/release/double_build.py build --leg a|b --out DIR
        one leg in this checkout (CI runs one per job): cargo +1.99.1 build --locked --release --bins with
        POCOL_RELEASE=1, then package.py --platform Linux, then the SHA-256 lists in DIR
    python3 deploy/release/double_build.py compare DIR_A DIR_B
        exit 0 only when both legs' lists are equal; otherwise a diff summary (sizes, first differing offset and,
        when diffoscope is on PATH, its report)

What the recipe fixes (everything else may vary between the legs):
  - the toolchain: rustc and cargo 1.99.1, --locked, the release profile of prototypes/pocold/Cargo.toml;
  - the build path cargo sees: /tmp/pocol-native-build/src, a symlink to the checkout, held under an exclusive lock
    (/tmp/pocol-native-build/lock: one build at a time per machine, a second one is refused). The checkout itself can be
    anywhere: cargo derives the crates' -C metadata (and so every mangled symbol) from the absolute path of the path
    dependencies outside prototypes/pocold (lp4-execution and the third-party/evm-crypto patches), so the path
    cargo is given must be the same in every build. Changing the manifests would change the consensus-pinned
    prototypes/pocold/Cargo.toml, so the recipe fixes the path instead;
  - every path the compilers record: --remap-path-prefix (Rust) and -ffile-prefix-map (C and assembly compiled by the
    cc crate: blst, c-kzg, secp256k1-sys) map the build path, the real checkout path, the cargo home and the rustup
    home to /pocol/src, /pocol/cargo-home and /pocol/rustup;
  - SOURCE_DATE_EPOCH: the commit's committer time (package.py stamps the archive entries with it);
  - the cargo configuration: cargo runs in the checkout (as the native CI job does) and reads only the checkout's
    committed .cargo/config(.toml), the cargo home's and the user's own (~/.cargo); a .cargo/config(.toml) in any
    other directory above the checkout (such as /tmp/.cargo, which any local user can create) is refused;
  - the package: package.py's Linux archive (entries in name order, that one mtime, 0:0 without names, modes 0755 or
    0644, ustar, a gzip header without name or time).

It builds no Windows wallet and no APK (both are outside this check), signs nothing and never reads a key. A
successful double build shows that two builds of one commit agree; it is not the independent rebuild of the signed
release that gate I1 still requires.
"""
import argparse, contextlib, fcntl, hashlib, io, os, platform, re, shutil, subprocess, sys, tarfile, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
TOOLCHAIN = '1.99.1'
BINARIES = ('pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site')
BUILD_LINK = Path('/tmp/pocol-native-build/src')
# The legs of the local run; CI's two jobs use the same values (five-device-build.yml, native-reproducible).
LEGS = {
    'a': {'path': 'a/src', 'umask': 0o022, 'tz': 'UTC', 'lc_all': 'C'},
    'b': {'path': 'b/other/deeper/checkout-b', 'umask': 0o077, 'tz': 'Pacific/Kiritimati', 'lc_all': 'C.UTF-8'},
}
LISTS = ('binaries.sha256', 'package.sha256', 'members.txt')


class Refused(Exception):
    pass


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, **kw)


def out(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw).stdout


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def version():
    text = (ROOT / 'deploy/standalone/package.py').read_text(encoding='utf-8')
    return re.search(r"(?m)^version='([0-9]+\.[0-9]+\.[0-9]+)'$", text)[1]


def checked_path(p):
    s = str(p)
    # "=" would split a remap flag; whitespace would split CFLAGS (the cc crate splits on it); \x1f the rustc flags.
    if any(c in s for c in '=\x1f') or any(c.isspace() for c in s) or not s.startswith('/'):
        raise Refused(f'{s!r}: a build path must be absolute and contain no "=", whitespace or unit separator')
    return s


def cargo_home(env):
    return Path(env.get('CARGO_HOME') or Path.home() / '.cargo').absolute()


def rustup_home(env):
    return Path(env.get('RUSTUP_HOME') or Path.home() / '.rustup').absolute()


def prefix_maps(checkout, env):
    """(from, to) pairs, both spellings of every directory (as given and with symlinks resolved), shortest first so
    the more specific mapping comes last (the one rustc and gcc apply)."""
    pairs = {checked_path(BUILD_LINK): '/pocol/src'}  # as given: the link is (re)pointed only when the build starts
    for given, to in ((checkout, '/pocol/src'), (cargo_home(env), '/pocol/cargo-home'),
                      (cargo_home(env) / 'registry', '/pocol/cargo-home/registry'), (rustup_home(env), '/pocol/rustup')):
        for spelling in {Path(given), Path(given).resolve()}:
            pairs[checked_path(spelling)] = to
    return sorted(pairs.items(), key=lambda kv: (len(kv[0]), kv[0]))


def reproducible_env(checkout, base=None):
    """The build environment of the recipe for a checkout (its real path)."""
    env = dict(os.environ if base is None else base)
    # A flag from the caller's environment would make this build another build.
    for key in ('RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER',
                'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET_DIR', 'CARGO_INCREMENTAL', 'CFLAGS', 'CXXFLAGS', 'ASMFLAGS', 'CC', 'CXX',
                'POCOL_RELEASE', 'SOURCE_DATE_EPOCH'):
        env.pop(key, None)
    for key in [k for k in env if k.startswith(('CARGO_PROFILE_', 'CFLAGS_', 'CXXFLAGS_', 'CC_', 'CXX_', 'POCOL_REPIN_'))]:
        env.pop(key)
    maps = prefix_maps(checkout, env)
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(f'--remap-path-prefix={a}={b}' for a, b in maps)
    env['CFLAGS'] = env['CXXFLAGS'] = env['ASMFLAGS'] = ' '.join(f'-ffile-prefix-map={a}={b}' for a, b in maps)
    env['CARGO_TARGET_DIR'] = str(BUILD_LINK / 'prototypes/pocold/target')
    env['CARGO_INCREMENTAL'] = '0'
    env['POCOL_RELEASE'] = '1'
    env['SOURCE_DATE_EPOCH'] = out(['git', 'log', '-1', '--format=%ct', 'HEAD'], cwd=checkout).strip()
    return env


def build_dir():
    """/tmp/pocol-native-build, created if missing. Refuses a directory that is not this user's own or that others can
    write to."""
    parent = BUILD_LINK.parent
    try:
        parent.mkdir(mode=0o755)
    except FileExistsError:
        pass
    st = os.lstat(parent)
    if not os.path.isdir(parent) or os.path.islink(parent) or st.st_uid != os.getuid() or st.st_mode & 0o022:
        raise Refused(f'{parent}: must be a directory of this user, not writable by others (remove it and retry)')
    return parent


@contextlib.contextmanager
def build_link(target):
    """Holds the exclusive lock of the build path and points BUILD_LINK at the checkout for the duration. A second build
    on the same machine (another checkout, another run) is refused while the lock is held, never switched in midway.
    A link left by a build that died is replaced: with the lock held, no other build is using it."""
    parent = build_dir()
    fd = os.open(parent / 'lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    try:
        if os.fstat(fd).st_uid != os.getuid():
            raise Refused(f'{parent / "lock"}: not owned by this user')
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise Refused(f'{parent / "lock"}: another native build is running on this machine (one at a time); '
                          'wait for it to finish') from None
        if BUILD_LINK.exists() and not BUILD_LINK.is_symlink():
            raise Refused(f'{BUILD_LINK}: exists and is not a symlink')
        tmp = parent / f'.src-{os.getpid()}'
        if tmp.is_symlink():
            tmp.unlink()
        os.symlink(target, tmp)
        os.replace(tmp, BUILD_LINK)
        try:
            yield
        finally:
            # Only this build's link: never one another build put there.
            if BUILD_LINK.is_symlink() and os.readlink(BUILD_LINK) == str(target):
                BUILD_LINK.unlink()
    finally:
        os.close(fd)  # releases the lock; the lock file stays (removing it would race a waiting build)


def check_cargo_config(checkout, env):
    """Refuses a cargo configuration the commit does not carry. Cargo reads .cargo/config(.toml) in its working
    directory (here the checkout) and in every directory above it: a file there (such as /tmp/.cargo/config.toml, which
    any local user can create) could set a rustc wrapper, a linker or [env] for the build. Allowed: the checkout's own
    committed files, the cargo home's and the user's own ~/.cargo (cargo reads both in every build of this user)."""
    allowed = set()
    for home in (cargo_home(env), Path.home() / '.cargo'):
        for name in ('config.toml', 'config'):
            allowed.add(os.path.realpath(home / name))
    tracked = set(out(['git', 'ls-files', '-z', '--', '.cargo/config.toml', '.cargo/config'], cwd=checkout).split('\0'))
    found = []
    for d in (Path(checkout), *Path(checkout).parents):
        for name in ('config.toml', 'config'):
            p = d / '.cargo' / name
            if not (p.exists() or p.is_symlink()):
                continue
            if d == Path(checkout) and f'.cargo/{name}' in tracked:
                continue
            if os.path.realpath(p) in allowed and d != Path(checkout):
                continue
            found.append(str(p))
    if found:
        raise Refused('cargo would read a configuration this commit does not carry (remove it, or build elsewhere): '
                      + ', '.join(found))


def tool_versions(env):
    lines = []
    for cmd in (['rustc', f'+{TOOLCHAIN}', '-vV'], ['cargo', f'+{TOOLCHAIN}', '-V'], ['cc', '--version']):
        try:
            text = out(cmd, env=env).splitlines()
            lines.append(' '.join(cmd) + ': ' + ' | '.join(text[:1] if cmd[0] == 'cc' else [l for l in text if l]))
        except (OSError, subprocess.CalledProcessError) as e:
            lines.append(' '.join(cmd) + f': unavailable ({e})')
    import zlib
    lines.append(f'python {platform.python_version()} zlib {zlib.ZLIB_RUNTIME_VERSION}')
    return lines


def build(leg, dest):
    checkout = ROOT
    dest = Path(dest).absolute()
    if dest.exists() and any(dest.iterdir()):
        raise Refused(f'{dest}: exists and is not empty')
    dirty = out(['git', 'status', '--porcelain'], cwd=checkout)
    if dirty.strip():
        raise Refused(f'{checkout}: the tree has uncommitted changes; a release is built from a commit '
                      '(the local run takes them with --worktree)\n' + dirty)
    commit = out(['git', 'rev-parse', 'HEAD'], cwd=checkout).strip()
    env = reproducible_env(checkout)
    dest.mkdir(parents=True, exist_ok=True)
    umask = os.umask(0); os.umask(umask)
    env_lines = [f'leg {leg}', f'commit {commit}', f'checkout {checkout}', f'build path {BUILD_LINK}',
                 f'cargo home {cargo_home(env)}', f'umask {umask:04o}', f'TZ {env.get("TZ", "")}',
                 f'LC_ALL {env.get("LC_ALL", "")}', f'SOURCE_DATE_EPOCH {env["SOURCE_DATE_EPOCH"]}',
                 f'image {env.get("ImageOS", "?")} {env.get("ImageVersion", "?")}', *tool_versions(env)]
    (dest / 'env.txt').write_text('\n'.join(env_lines) + '\n', encoding='utf-8')
    print('\n'.join(env_lines), flush=True)
    check_cargo_config(checkout, env)
    dist = dest / 'dist'
    with build_link(checkout):
        manifest = BUILD_LINK / 'prototypes/pocold/Cargo.toml'
        # In the checkout, as the native job runs it: cargo finds its configuration from its working directory.
        run(['cargo', f'+{TOOLCHAIN}', 'build', '--locked', '--release', '--bins', '--manifest-path', str(manifest)],
            env=env, cwd=checkout)
        run([sys.executable, '-B', str(BUILD_LINK / 'deploy/standalone/package.py'), '--platform', 'Linux', '--out', str(dist)],
            env=env, cwd=checkout)
    release = checkout / 'prototypes/pocold/target/release'
    sums = {name: sha256(release / name) for name in BINARIES}
    (dest / 'binaries.sha256').write_text(''.join(f'{sums[n]}  {n}\n' for n in BINARIES), encoding='utf-8')
    archive = dist / f'PoCol-Mainnet-{version()}-linux-x86_64.tar.gz'
    write_package_lists(archive, dest)
    members = members_of(archive)
    for name in BINARIES:
        if members.get(name, ('',))[0] != sums[name]:
            raise Refused(f'{name}: the packaged binary is not the one cargo built')
    print((dest / 'binaries.sha256').read_text(encoding='utf-8') + (dest / 'package.sha256').read_text(encoding='utf-8'),
          end='', flush=True)


def members_of(archive):
    """{name: (sha256, mode, mtime, uid, gid)} of the archive's members."""
    found = {}
    with tarfile.open(archive) as tar:
        for m in tar.getmembers():
            data = tar.extractfile(m).read() if m.isfile() else b''
            found[m.name] = (hashlib.sha256(data).hexdigest(), f'{m.mode:04o}', m.mtime, m.uid, m.gid)
    return found


def write_package_lists(archive, dest):
    import gzip
    with gzip.open(archive, 'rb') as f:
        tar_sha = hashlib.sha256(f.read()).hexdigest()
    (dest / 'package.sha256').write_text(f'{sha256(archive)}  {archive.name}\n{tar_sha}  {archive.name[:-3]} (uncompressed)\n',
                                         encoding='utf-8')
    rows = members_of(archive)
    (dest / 'members.txt').write_text(''.join(f'{v[0]}  {v[1]} {v[2]} {v[3]}:{v[4]}  {k}\n' for k, v in sorted(rows.items())),
                                      encoding='utf-8')


def first_difference(a, b):
    with open(a, 'rb') as fa, open(b, 'rb') as fb:
        offset = 0
        while True:
            ca, cb = fa.read(1 << 16), fb.read(1 << 16)
            if ca != cb:
                n = min(len(ca), len(cb))
                i = next((j for j in range(n) if ca[j] != cb[j]), n)
                return offset + i
            if not ca:
                return None
            offset += len(ca)


def explain(a_dir, b_dir, use_diffoscope=True):
    """Prints a summary of how the two legs' packages differ: per member size and first differing offset."""
    archives = [next((Path(d) / 'dist').glob('*.tar.gz'), None) for d in (a_dir, b_dir)]
    if not all(archives):
        print('  (a leg has no package to compare)')
        return
    work = Path(tempfile.mkdtemp(prefix='pocol-double-build-diff-'))
    try:
        trees = []
        for leg, archive in zip('ab', archives):
            tree = work / leg
            tree.mkdir()
            with tarfile.open(archive) as tar:
                for m in tar.getmembers():
                    if m.isfile() and '/' not in m.name and m.name not in ('.', '..'):
                        (tree / m.name).write_bytes(tar.extractfile(m).read())
            trees.append(tree)
        names = sorted({p.name for t in trees for p in t.iterdir()})
        for name in names:
            pa, pb = trees[0] / name, trees[1] / name
            if not pa.exists() or not pb.exists():
                print(f'  {name}: only in leg {"a" if pa.exists() else "b"}')
                continue
            off = first_difference(pa, pb)
            if off is not None:
                print(f'  {name}: sizes {pa.stat().st_size} / {pb.stat().st_size}, first differing byte at offset {off} (0x{off:x})')
        if use_diffoscope and shutil.which('diffoscope'):
            r = subprocess.run(['diffoscope', '--no-progress', '--max-report-size', '2000000', '--text', '-',
                                str(archives[0]), str(archives[1])], capture_output=True, text=True, timeout=1800)
            print('diffoscope report (first 300 lines):')
            print('\n'.join(r.stdout.splitlines()[:300]))
        elif use_diffoscope:
            print('  (diffoscope is not installed: only sizes and first differing offsets are shown)')
    finally:
        shutil.rmtree(work, ignore_errors=True)


def package_problem(d):
    """None when the leg's dist/ holds exactly what package.py writes for Linux, the archive its package.sha256 names
    with that SHA-256 and the SHA256SUMS line of that archive (the bytes CI uploads for signing are then the bytes the
    comparison is about); otherwise what is wrong."""
    first = (d / 'package.sha256').read_text(encoding='utf-8').splitlines()[0].split('  ', 1)
    if len(first) != 2:
        return f'{d / "package.sha256"}: malformed first line'
    want, name = first
    dist = d / 'dist'
    files = sorted(p.name for p in dist.iterdir()) if dist.is_dir() else []
    if files != sorted([name, 'SHA256SUMS']):
        return f'{dist} must hold exactly {name} and SHA256SUMS, holds {files}'
    got = sha256(dist / name)
    if got != want:
        return f'{dist / name} has SHA-256 {got}, its package.sha256 lists {want}'
    if (dist / 'SHA256SUMS').read_bytes() != f'{want}  {name}\n'.encode():
        return f'{dist / "SHA256SUMS"} is not the one line of {name}'
    return None


def compare(a_dir, b_dir, use_diffoscope=True):
    a_dir, b_dir = Path(a_dir), Path(b_dir)
    for d in (a_dir, b_dir):
        for name in LISTS:
            if not (d / name).is_file() or not (d / name).read_text(encoding='utf-8').strip():
                print(f'DIFFERENT: {d / name} is missing or empty (the leg did not finish)', file=sys.stderr)
                return 1
    for leg, d in (('a', a_dir), ('b', b_dir)):
        print(f'leg {leg}:')
        env_txt = d / 'env.txt'
        print('  ' + env_txt.read_text(encoding='utf-8').strip().replace('\n', '\n  ') if env_txt.is_file() else '  (no env.txt)')
    for leg, d in (('a', a_dir), ('b', b_dir)):
        problem = package_problem(d)
        if problem:
            print(f'DIFFERENT: leg {leg}: {problem}', file=sys.stderr)
            return 1
    differing = [n for n in LISTS if (a_dir / n).read_bytes() != (b_dir / n).read_bytes()]
    if not differing:
        print('IDENTICAL: both builds produced the same bytes')
        print((a_dir / 'binaries.sha256').read_text(encoding='utf-8') + (a_dir / 'package.sha256').read_text(encoding='utf-8'), end='')
        return 0
    print('DIFFERENT: the two builds differ (if the tool lines above differ too, the legs ran different toolchains or images)',
          file=sys.stderr)
    for name in differing:
        la = (a_dir / name).read_text(encoding='utf-8').splitlines()
        lb = (b_dir / name).read_text(encoding='utf-8').splitlines()
        print(f'{name}:')
        for line in sorted(set(la) ^ set(lb), key=lambda l: (l.split()[-1], l in lb)):
            print(f'  {"a" if line in la else "b"}: {line}')
    explain(a_dir, b_dir, use_diffoscope)
    return 1


def snapshot(dest, commit_time):
    """A throwaway repository with the working tree's files (tracked, modified and untracked, not ignored) and one
    commit with fixed author, committer and dates: both legs get the same synthetic commit id."""
    names = [n for n in out(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT).split('\0') if n]
    dest.mkdir(parents=True)
    for name in names:
        src = ROOT / name
        if src.is_symlink() or not src.is_file():
            if src.is_symlink():
                raise Refused(f'{name}: symlinks are not supported in a snapshot')
            continue  # a tracked file deleted in the working tree
        target = dest / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(src, target)
        os.chmod(target, (0o777 if os.stat(src).st_mode & 0o100 else 0o666) & ~current_umask())
    git = ['git', '-c', 'init.defaultBranch=snapshot', '-c', 'core.autocrlf=false', '-c', 'commit.gpgsign=false',
           '-c', 'user.name=double-build', '-c', 'user.email=double-build@invalid']
    run(git + ['init', '-q'], cwd=dest)
    run(git + ['add', '-A'], cwd=dest)
    stamp = f'@{commit_time} +0000'
    run(git + ['commit', '-q', '--no-verify', '-m', 'double-build snapshot of a working tree (not a commit of the repository)'],
        cwd=dest, env={**os.environ, 'GIT_AUTHOR_DATE': stamp, 'GIT_COMMITTER_DATE': stamp})


def current_umask():
    u = os.umask(0); os.umask(u)
    return u


def local(args):
    commit = out(['git', 'rev-parse', 'HEAD'], cwd=ROOT).strip()
    commit_time = out(['git', 'log', '-1', '--format=%ct', 'HEAD'], cwd=ROOT).strip()
    if not args.worktree and out(['git', 'status', '--porcelain'], cwd=ROOT).strip():
        print(f'note: building commit {commit}; uncommitted changes are not included (--worktree includes them)', flush=True)
    work = Path(args.work).absolute() if args.work else Path(tempfile.mkdtemp(prefix='pocol-double-build-'))
    work.mkdir(parents=True, exist_ok=True)
    if any(work.iterdir()):
        raise Refused(f'{work}: exists and is not empty')
    print(f'work directory {work}', flush=True)
    status = 1
    try:
        base_home = cargo_home(os.environ)
        envs = {}
        # Both checkouts first, so both legs build the same source even if the working tree changes meanwhile.
        for leg, spec in LEGS.items():
            checkout = work / spec['path']
            env = {**os.environ, 'TZ': spec['tz'], 'LC_ALL': spec['lc_all']}
            old = os.umask(spec['umask'])
            try:
                if args.worktree:
                    snapshot(checkout, commit_time)
                else:
                    checkout.parent.mkdir(parents=True, exist_ok=True)
                    run(['git', 'clone', '-q', '--shared', '--no-checkout', str(ROOT), str(checkout)])
                    run(['git', '-c', 'core.autocrlf=false', 'checkout', '-q', '--detach', commit], cwd=checkout)
                if leg == 'b':
                    # Other source mtimes (set in reverse name order) and another cargo home path over the shared registry.
                    files = sorted((p for p in checkout.rglob('*') if p.is_file() and '.git' not in p.relative_to(checkout).parts),
                                   reverse=True)
                    for p in files:
                        os.utime(p, (981173106, 981173106))
                    home = work / 'b/cargo-home'
                    home.mkdir(parents=True)
                    (home / 'registry').symlink_to(base_home / 'registry')
                    for name in ('config.toml', 'config'):
                        if (base_home / name).is_file():
                            (home / name).symlink_to(base_home / name)
                    env['CARGO_HOME'] = str(home)
            finally:
                os.umask(old)
            envs[leg] = (checkout, env)
        heads = {out(['git', 'rev-parse', 'HEAD'], cwd=c).strip() for c, _ in envs.values()}
        if len(heads) != 1:
            raise Refused('the working tree changed while the two snapshots were taken; run again')
        if args.worktree:
            print(f'worktree snapshot {heads.pop()} (a commit of a throwaway repository, not of this one)', flush=True)
        for leg, spec in LEGS.items():
            checkout, env = envs[leg]
            old = os.umask(spec['umask'])
            try:
                print(f'== leg {leg}: {checkout} (umask {spec["umask"]:04o}, TZ {spec["tz"]}, LC_ALL {spec["lc_all"]})', flush=True)
                run([sys.executable, '-B', str(checkout / 'deploy/release/double_build.py'), 'build', '--leg', leg,
                     '--out', str(work / leg / 'out')], cwd=checkout, env=env)
            finally:
                os.umask(old)
                if not args.keep:
                    shutil.rmtree(checkout / 'prototypes/pocold/target', ignore_errors=True)
        status = compare(work / 'a/out', work / 'b/out')
    finally:
        if args.keep:
            print(f'kept {work}')
        elif status == 0 and not args.work:
            shutil.rmtree(work, ignore_errors=True)
        else:
            # Keep only the legs' outputs (lists, env.txt, packages) for inspection.
            for leftover in (work / 'a/src', work / 'b/other', work / 'b/cargo-home'):
                shutil.rmtree(leftover, ignore_errors=True)
            print(f'kept the legs\' outputs in {work}/a/out and {work}/b/out')
    return status


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest='command')
    b = sub.add_parser('build', help='one leg in this checkout')
    b.add_argument('--leg', required=True, choices=sorted(LEGS))
    b.add_argument('--out', required=True, type=Path)
    c = sub.add_parser('compare', help='compare two legs\' output directories')
    c.add_argument('a', type=Path)
    c.add_argument('b', type=Path)
    c.add_argument('--no-diffoscope', action='store_true')
    p.add_argument('--worktree', action='store_true', help='build the working tree (a snapshot), not HEAD')
    p.add_argument('--work', help='work directory (empty or new; default: a new temporary directory)')
    p.add_argument('--keep', action='store_true', help='keep the checkouts and their target directories')
    a = p.parse_args(argv)
    if sys.platform != 'linux' or platform.machine() not in ('x86_64', 'AMD64'):
        print('REFUSED: the native double build is for the Linux x86_64 package', file=sys.stderr)
        return 2
    try:
        if a.command == 'build':
            build(a.leg, a.out)
            return 0
        if a.command == 'compare':
            return compare(a.a, a.b, not a.no_diffoscope)
        return local(a)
    except (Refused, subprocess.CalledProcessError, OSError) as e:
        print(f'REFUSED: {e}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
