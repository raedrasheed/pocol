# Provenance of the integrated e2e_devnet 3/3 runs (review finding E2E-R3-9)

This note adds what `../e2e-integration/e2e-devnet-3x-integrated.log` does not record. That log is
not edited. Its only header line gives the binary sha256.

The facts below come from the session transcript: the tool calls at 2026-10-23T18:58:16Z,
18:58:22Z, 18:58:40Z and 19:13:51Z. The transcript is not in the repository. Each fact that can
still be checked was re-checked on 2026-10-23, as noted, and again on 2026-10-24 after a container
restart: the four tool calls were read back from the transcript, and every check below was
repeated with the same result. The checks of the next section were repeated once more on
2026-10-24 around 06:50Z, after a session limit, in the worktree `wf_86607a23-c25-4`: same results
(four sha256 `9d73605e…`, `git diff --quiet` exit 0, binary `145ae16c…`, 81 PASS lines and three
PASS result lines in the log).

## What ran

| Item | Value |
|---|---|
| Runs | 3, back to back, started at 18:58:41Z, 19:03:03Z and 19:08:02Z; each printed 27 PASS lines and `{"result": "PASS", "checks": 27, ...}` |
| Checkout | the main checkout `/home/user/Proof-of-Collaboration` at `e35f165`, just after the merge of `wf/e2e-r2` (18:58:16Z) |
| Script | `prototypes/pocold/tests/e2e_devnet.py`, sha256 `9d73605edf1898816da59c5417183be41fd718860836695585d5858fa82c1c15` |
| Binary | pocold sha256 `145ae16ca3c9f9377cce836546debcb90c6fa1a5501b151d5b4bd1605c8ed653`, built at 18:58:22Z in `prototypes/pocold` with `timeout 1200 flock <scratch>/cpu.lock cargo build --release --locked --bins` (default target directory), then copied to `<scratch>/e2e-int/bin/` with pocol-sign (sha256 `50416e36…c7f414e`) |
| Locks | `<scratch>/e2e.lock`, then `<scratch>/cpu.lock` (taken in that order by nested `flock`), for each run |

`<scratch>` is the session scratchpad
`/tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad`.

The exact command, with the working directory `/home/user/Proof-of-Collaboration/prototypes/pocold`:

```sh
for i in 1 2 3; do echo "=== run $i $(date -u +%FT%TZ)"; POCOLD=$S/e2e-int/bin/pocold timeout 1800 flock $S/e2e.lock flock $S/cpu.lock python3 -I tests/e2e_devnet.py 2>&1 | tail -40; echo "exit=$?"; done > $S/e2e-int/e2e-3x.log 2>&1
```

Here `$S` is `<scratch>`. At 19:13:51Z the log was copied into the repository with the binary line
prepended.

## Checks made for this note

- The script's sha256 is `9d73605e…c1c15` at `f583c7f`, `e35f165`, `d40fd91` and `fa97594`
  (`git show <commit>:prototypes/pocold/tests/e2e_devnet.py | sha256sum`). The merge that produced
  `e35f165` rewrote this file. `git merge` refuses to run over local changes to a file it rewrites,
  so the file that ran was the committed one.
- `git diff --quiet e35f165 fa97594 -- prototypes` exits 0: the integrated runs hold for the code at
  `fa97594`.
- `<scratch>/e2e-int/bin/pocold` still hashes to `145ae16c…ed653`.
- Each run block in the log has 28 lines (27 PASS lines and the JSON line), so `tail -40` dropped
  nothing.

## A caveat on the recorded exit codes

The `exit=0` lines in that log are **not** the test's exit status. `$?` after `… | tail -40` is
the status of `tail`. The evidence that each run passed is its final line,
`{"result": "PASS", "checks": 27, …}`. The test prints that line only on its success path, right
before `return 0`; any failure prints `"result": "FAIL"` and returns 1. stderr went through the
same pipe, and no traceback follows the line in any of the three blocks. The round-3 runs record
the exit status of the test process itself (`README.md` §4).
