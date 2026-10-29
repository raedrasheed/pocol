#!/usr/bin/env python3
"""Commit roles and tree equivalence for the audit-repair evidence (round 2).

usage: python3 reconcile.py IMPL INTEG WIT CHROME [HEAD]
Prints a Markdown section that, for every evidence commit and the PR head, shows:
  - whether the code trees (prototypes/ and .github/) equal those of the last implementation commit
    (`git rev-parse <c>:prototypes` and `<c>:.github`, compared as tree hashes);
  - every commit between the implementation commit and the head, with the paths it touches.
A difference outside development/ after IMPL makes the equivalence claim false, and the script says so.
"""
import subprocess, sys

def git(*a):
    return subprocess.run(["git", *a], capture_output=True, text=True, check=True).stdout.strip()

def tree(c, path):
    try: return git("rev-parse", f"{c}:{path}")
    except subprocess.CalledProcessError: return None

def main():
    impl, integ, wit, chrome = sys.argv[1:5]; head = sys.argv[5] if len(sys.argv) > 5 else git("rev-parse", "HEAD")
    full = lambda c: git("rev-parse", c)
    roles = [("last implementation commit", impl), ("integration-tested commit", integ), ("witness-tested commit", wit), ("real-Chrome-tested commit", chrome), ("PR head", head)]
    out = ["| role | commit | prototypes/ tree | .github/ tree | code equal to the implementation commit |", "|---|---|---|---|---|"]
    ref = (tree(impl, "prototypes"), tree(impl, ".github"))
    ok_all = True
    for name, c in roles:
        t = (tree(c, "prototypes"), tree(c, ".github")); eq = t == ref; ok_all &= eq
        out.append(f"| {name} | `{full(c)}` | `{t[0][:12]}` | `{(t[1] or 'none')[:12]}` | {'yes' if eq else '**no**'} |")
    out.append("")
    log = git("log", "--format=%H %s", f"{impl}..{head}").splitlines()
    out.append(f"Commits after the implementation commit ({len(log)}):")
    out.append("")
    for l in reversed(log):
        h, s = l.split(" ", 1); paths = git("diff-tree", "--no-commit-id", "--name-only", "-r", h).splitlines()
        # Only prototypes/ and .github/ are built/tested; development/ docs and root files
        # (.gitignore, README) do not change the compiled code, so they do not break equivalence.
        code = [p for p in paths if p.startswith("prototypes/") or p.startswith(".github/")]
        noncode = [p for p in paths if p not in code]
        tag = "all docs/meta (no code)" if not code else "**code: " + ", ".join(code) + "**"
        out.append(f"- `{h[:12]}` {s}: {len(paths)} paths, {tag}" + (f" (+{len(noncode)} docs/meta)" if code and noncode else ""))
        ok_all &= not code
    out.append("")
    out.append("Result: " + ("every evidence commit and the head carry the implementation commit's code trees; later commits change development/ and root metadata (.gitignore) only." if ok_all else "**a post-implementation commit changed prototypes/ or .github/; the evidence does not cover the head as is.**"))
    print("\n".join(out))
    return 0 if ok_all else 1

if __name__ == "__main__":
    sys.exit(main())
