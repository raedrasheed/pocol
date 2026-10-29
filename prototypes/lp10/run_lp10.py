#!/usr/bin/env python3
"""LP10 runner: builds everything and runs every suite of the local PoCol devnet in
order, then writes an evidence summary (commit, toolchain, per-suite result and
timing) to development/lp10/LP10-EVIDENCE.json.

usage: python3 prototypes/lp10/run_lp10.py [--skip SUITE[,SUITE]] [--only SUITE[,SUITE]]
Suites: lp4-tests, pocold-tests, ext-unit, e2e-devnet, e2e-site, e2e-browser, e2e-integrated.
Local devnet only (simnet profiles, loopback ports 30300-30302 / 8540-8542 / 8600-8602).
"""
import json, os, platform, re, subprocess, sys, time
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
P = lambda *x: os.path.join(ROOT, "prototypes", *x)
SUITES = [
    ("lp4-tests", P("lp4-execution"), ["cargo", "test", "--release", "--offline"], 1800),
    ("pocold-build", P("pocold"), ["cargo", "build", "--release", "--offline"], 1800),
    ("pocold-tests", P("pocold"), ["cargo", "test", "--release", "--offline"], 1800),
    ("ext-unit", P("pocol-extension"), ["node", "tests/unit.mjs"], 600),
    ("e2e-devnet", P("pocold"), ["python3", "tests/e2e_devnet.py"], 1800),
    ("e2e-site", P("pocold"), ["python3", "tests/e2e_site.py"], 1800),
    ("e2e-browser", P("pocol-extension"), ["node", "tests/e2e_browser.mjs"], 2700),
    ("e2e-integrated", P("pocol-extension"), ["node", "tests/e2e_integrated.mjs"], 3600),
]

def version(cmd):
    try: return subprocess.run(cmd, capture_output=True, text=True, timeout=30).stdout.strip().splitlines()[0]
    except Exception as e: return f"unavailable ({e})"

def summarize(name, out):
    if name.endswith("tests"):
        rs = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed", out)
        return {"passed": sum(int(r[1]) for r in rs), "failed": sum(int(r[2]) for r in rs)}
    lines = out.strip().splitlines()
    for i in range(len(lines) - 1, -1, -1):  # last JSON object, one line or pretty-printed
        if lines[i].startswith("{"):
            try: v = json.loads("\n".join(lines[i:])); return {k: v[k] for k in ("result", "checks", "passed", "failed") if k in v}
            except Exception: continue
    return {"lines": lines[-3:]}

def main():
    args = sys.argv[1:]
    only = set(args[args.index("--only") + 1].split(",")) if "--only" in args else None
    skip = set(args[args.index("--skip") + 1].split(",")) if "--skip" in args else set()
    logdir = os.path.join(ROOT, "development", "lp10", "logs"); os.makedirs(logdir, exist_ok=True)
    ev = {"commit": subprocess.run(["git", "-C", ROOT, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip(),
          "dirty": bool(subprocess.run(["git", "-C", ROOT, "status", "--porcelain"], capture_output=True, text=True).stdout.strip()),
          "host": platform.platform(), "toolchain": {"cargo": version(["cargo", "--version"]), "rustc": version(["rustc", "--version"]), "node": version(["node", "--version"]), "python": platform.python_version()},
          "started": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "suites": []}
    ok_all = True
    for name, cwd, cmd, tmo in SUITES:
        if (only and name not in only) or name in skip: continue
        t = time.time(); print(f"== {name}: {' '.join(cmd)}", flush=True)
        try: r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=tmo); code, out = r.returncode, r.stdout + r.stderr
        except subprocess.TimeoutExpired as e: code, out = -1, (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or "") + "\nTIMEOUT"
        open(os.path.join(logdir, f"{name}.log"), "w").write(out)
        s = {"suite": name, "exit": code, "seconds": round(time.time() - t, 1), "summary": summarize(name, out)}
        ev["suites"].append(s); ok_all &= code == 0
        print(f"   exit={code} {s['seconds']}s {json.dumps(s['summary'])[:300]}", flush=True)
    ev["result"] = "PASS" if ok_all else "FAIL"; ev["finished"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    json.dump(ev, open(os.path.join(ROOT, "development", "lp10", "LP10-EVIDENCE.json"), "w"), indent=1)
    print(json.dumps({"result": ev["result"], "suites": [(s["suite"], s["exit"]) for s in ev["suites"]]}))
    return 0 if ok_all else 1

if __name__ == "__main__":
    sys.exit(main())
