# M1 Draft 0.31: native error-data codec (C35) and evidence bindings (C36), author turn 029

**For root review. Not approved. Nothing was executed by the author.**

Only new files under `m1-draft-0.31/` were written. Earlier tools are loaded unchanged by path: the 0.30 runner helpers and guard, the 0.29 overlay models, the 0.27 checker and the 0.4 strict parser.

## Runtime provenance (changed from 0.30)

0.31 is **not** pure Python. It needs two things:

- **Python 3.11 standard library.** It runs the runner, the receive guard and the bindings.
- **Node.js, installed locally** (expected **v22.13.1**, the version of root's native oracles, already used by M1 tooling). It runs only the fixed reference codec `tools/json_codec_031.mjs`, which uses the engine's own `JSON.parse` / `JSON.stringify` for error clipping and the error-data byte size.

This is a reference-fixture runtime, not a production dependency. The codec:
- uses only built-ins (`node:process`, `node:buffer`, `TextDecoder`);
- takes no arguments and reads no files or environment;
- makes no network calls and starts no child processes.

The runner checks all of this statically (`codec.staticShape`).

**How Python calls it** (`tools/guard_ref_031.py`):
- with `subprocess.run([node, json_codec_031.mjs], input=<raw reply>, shell=False, timeout=10)`;
- the reply goes only through stdin;
- output is bounded;
- `NODE_OPTIONS` and `NODE_PATH` are removed from the child environment.

**Finding Node.** The runner looks for Node in `--node <path>`, then `POCOL_NODE`, then `PATH`.

**If Node or the codec is missing or misbehaves,** every dependent check FAILs. There is no Python fallback.

| File | Content |
|---|---|
| `M1-SPEC-0.31-AMENDMENT.md` | C35 and C36, ordering, bindings, gate |
| `M1-STATUS-0.31.md` | rows, exact remaining root work, rebound failures |
| `M1-DASHBOARD-0.31-AR.md` | plain Arabic summary |
| `tools/json_codec_031.mjs` | native Node reference codec |
| `tools/guard_ref_031.py` | 0.30 receive guard, plus the native codec for error replies |
| `tools/run_checks_031.py` | the runner (root only) |
| `vectors/c35-native-codec-cases.json` | oracle rows, exact boundaries, clipping, ordering |
| `vectors/c36-binding-repairs.json` | the six 0.30 FAILs mapped to their repair checks |
| `audit/acceptance-inventory-0.31.json` | 0.30 binding, changed rows, history, proposed statuses |

## Command (root only)

```
coordination\runtime\python311\python.exe m1-draft-0.31\tools\run_checks_031.py --root <tree> --out <dir> [--node <path to node.exe>]
```

The runner writes only into `--out`, and an earlier file is never overwritten. It writes:
- `run-results-0.31.json`;
- `acceptance-matrix-0.31.json`;
- `bindings-0.31.json`;
- `status-0.31.json`;
- `dashboard-0.31-ar.json`.
