#!/usr/bin/env python3
"""Audit-repair integration run: the LP10 suites (prototypes/lp10/run_lp10.py SUITES) plus lp1-node and
the extension release build, on the checked-out commit. Writes a summary with commit, toolchain,
per-suite exit status, counts and timing to development/audit-2026-10-22/evidence/integration.json and
full logs to the directory given by --logs (default: a temporary directory; logs are not committed).

usage: python3 development/audit-2026-10-22/tools/run_integration.py [--logs DIR] [--only a,b] [--skip a,b]
lp1-node's store_journal tests run and fail on Linux (UnsupportedPlatform: the journal is Windows-only).
That outcome is recorded as it is (exit code and failed count, suite pass=false); whether every failure is
that expected platform limitation is classified separately and never turns a failure into a pass."""
import importlib.util, json, os, re, subprocess, sys, tempfile, time
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
spec = importlib.util.spec_from_file_location("run_lp10", os.path.join(ROOT, "prototypes", "lp10", "run_lp10.py"))
lp10 = importlib.util.module_from_spec(spec); spec.loader.exec_module(lp10)
P = lambda *x: os.path.join(ROOT, "prototypes", *x)
# lp1-node runs with its accepted toolchain, Rust 1.58.1 (rustup); the other crates pin 1.99.1 in rust-toolchain.toml.
SUITES = [("lp1-node-tests", P("lp1-node"), ["cargo", "+1.58.1", "test", "--offline", "--locked", "--no-fail-fast"], 1800),
          ("ext-build", P("pocol-extension"), ["node", "build.mjs"], 300)] + lp10.SUITES

def main():
    a = sys.argv[1:]
    logs = a[a.index("--logs") + 1] if "--logs" in a else tempfile.mkdtemp(prefix="pocol-integration-")
    only = set(a[a.index("--only") + 1].split(",")) if "--only" in a else None
    skip = set(a[a.index("--skip") + 1].split(",")) if "--skip" in a else set()
    os.makedirs(logs, exist_ok=True)
    git = lambda *x: subprocess.run(["git", "-C", ROOT, *x], capture_output=True, text=True).stdout.strip()
    ev = {"commit": git("rev-parse", "HEAD"), "dirty": bool(git("status", "--porcelain")), "host": lp10.platform.platform(),
          "toolchain": {"cargo": lp10.version(["cargo", "--version"]), "rustc": lp10.version(["rustc", "--version"]), "rustcLp1": lp10.version(["rustc", "+1.58.1", "--version"]),
                        "rustcPinned": subprocess.run(["rustc", "--version"], cwd=P("pocold"), capture_output=True, text=True).stdout.strip(), "node": lp10.version(["node", "--version"]), "python": lp10.platform.python_version()},
          "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "logs": logs, "suites": []}
    ok_all = True
    for name, cwd, cmd, tmo in SUITES:
        if (only and name not in only) or name in skip: continue
        t = time.time(); print(f"== {name}: {' '.join(cmd)}", flush=True)
        # stderr merged into stdout: cargo's "Running <binary>" lines must stay next to their results.
        try: r = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=tmo); code, out = r.returncode, r.stdout
        except subprocess.TimeoutExpired as e: code, out = -1, (e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")) + "\nTIMEOUT"
        open(os.path.join(logs, f"{name}.log"), "w").write(out)
        s = {"suite": name, "cmd": " ".join(cmd), "exit": code, "seconds": round(time.time() - t, 1), "summary": lp10.summarize(name, out)}
        s["pass"] = code == 0
        if name == "lp1-node-tests":
            # The actual outcome stays as it is. Classified separately: did every failure come from the
            # Windows-only journal binary failing with UnsupportedPlatform (the expected limitation)?
            blocks = re.split(r"\n\s+Running ", out); journal = [b for b in blocks if b.startswith("tests/store_journal.rs")]
            jr = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", journal[0]) if journal else []
            others = [b for b in blocks if not b.startswith("tests/store_journal.rs")]
            rs = [m for b in others for m in re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed", b)]
            s["executedAndFailed"] = {"binary": "tests/store_journal.rs", "executed": bool(jr), "passed": int(jr[0][0]) if jr else None, "failed": int(jr[0][1]) if jr else None,
                                      "unsupportedPlatformErrors": journal[0].count("UnsupportedPlatform(") if journal else 0}
            s["otherBinaries"] = {"passed": sum(int(x[1]) for x in rs), "failed": sum(int(x[2]) for x in rs)}
            s["onlyExpectedPlatformLimitation"] = (code != 0 and bool(jr) and int(jr[0][1]) > 0 and s["otherBinaries"]["failed"] == 0 and len(rs) > 0
                                                   and s["executedAndFailed"]["unsupportedPlatformErrors"] >= int(jr[0][1]))
            s["platformNote"] = "store_journal uses the Windows-only atomic journal; on this host its tests execute and fail with UnsupportedPlatform, identically on the audited 090eae4. Windows verification is required to pass them."
        ev["suites"].append(s); ok_all &= s["pass"]
        print(f"   exit={code} pass={s['pass']} {s['seconds']}s {json.dumps(s.get('executedAndFailed', s['summary']))[:300]}", flush=True)
    ev["result"] = "PASS" if ok_all else "FAIL"; ev["finished"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    failing = [s for s in ev["suites"] if not s["pass"]]
    ev["failuresOnlyExpectedPlatformLimitation"] = bool(failing) and all(s.get("onlyExpectedPlatformLimitation") for s in failing)
    out = os.path.join(ROOT, "development", "audit-2026-10-22", "evidence", "integration.json")
    prev = json.load(open(out)) if os.path.exists(out) else {"runs": []}
    prev["runs"].append(ev); json.dump(prev, open(out, "w"), indent=1)
    print(json.dumps({"result": ev["result"], "suites": [(s["suite"], s["exit"], s["pass"]) for s in ev["suites"]]}))
    return 0 if ok_all else 1

if __name__ == "__main__":
    sys.exit(main())
