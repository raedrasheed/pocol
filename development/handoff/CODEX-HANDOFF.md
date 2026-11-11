# Handoff to Codex: finish the open PRs, then implement PoCol v3

Written 2026-11-11 by the previous agent (Claude Code), whose session is ending. This is your complete brief. The
repository is **raedrasheed/pocol** (the older name `raedrasheed/pocol-import` redirects to it). The default branch is
`main`; at this writing it was at `40b0428` ("Merge pull request #8").

Read this whole file first. After it, read in order:

1. `development/handoff/V3-ROADMAP.md`: the v3 implementation plan (PR sequence, consensus-lock strategy, ground truth).
2. `development/decisions/V3-DESIGN.md` and `development/decisions/V3-OWNER-DECISIONS-2026-11-08.md`: the approved v3
   design and the owner's answers.
3. `development/VA-SPEC.md` §2.3–2.4: the consensus boundary, lock kinds and re-pin rules.
4. `development/handoff/V3-DESIGN-WORKING-NOTES.md`: the longer working notes behind V3-DESIGN.md (background only).

---

## 1. The owner's rules (binding; do not reinterpret)

The owner wrote this in Arabic:

> «أوافق على تعديل التي لا تغيير خوارزمية الاجماع بدون اي تحفظ. أما التي تحتاج تغيير في خوارزمية الاجماع فأوافق على
> التغيير الذي لا يغيير فكرة الاجماع بالتعاون ولا يغير فكرة الكتلة النموذج أو تغيير أي مبدأ من المبادئ الاساسية ل PoCol.»

Meaning:
- **Approved without reservation:** changes that do not change the consensus algorithm (node-local code, tests, CI,
  tooling, clients, docs).
- **Approved only within limits:** consensus changes are approved only if they keep collaborative consensus (the PoCol
  idea), the template block (الكتلة النموذج) and every basic PoCol principle. If a consensus change would alter one of
  these, **stop and ask the owner** with a concrete proposal. Do not merge it.

The owner also said: «ادمج PR #1 والـ PRs القادمة عند نجاحها», meaning "merge PR #1 and the coming PRs when they
pass". This is a **standing merge approval**: merge a PR yourself once all of these hold:
- CI is fully green on its current head;
- it has no merge conflict;
- every valid review finding (Codex review bot, humans) is fixed or answered with a reason.

Owner decisions already taken:
- **v3 options:** "accept the recommendations": O-0=a O-6=b O-10=b O-11=b O-12=a O-13=b O-14=b O-15=b O-17=b O-18=b
  O-19=M3 O-20=b O-21=b O-32=b O-33=b.
- **Seeds:** the 3 seed nodes will be stopped once peer discovery works and enough independent nodes exist.
- **The 4M allocation** is the owner's personal matter, meant to fund network development. Do not touch it.
- **Android signing:** the owner will create the real Android signing key.
- **Audit:** a human security audit happens after the real launch.

### Hard security constraints (never violate)
- Live network deploy or cutover is **on hold**. Do not deploy, do not re-initialise the network, never run
  `Initialize-PoColNetwork.ps1`.
- Never display, copy or publish `/var/backups/pocol-pre-0.4.0.*`, `/etc/pocol-testnet`, miner key files or
  `api/config.php`. Preserve `api/config.php` and existing deploy data. Never delete wallet data.
- Never disable TLS verification.
- The branch `docs/claude-handoff-2026-11-01` is sensitive. It stays off `main`; never merge or quote it.
- No AI model names or identifiers in commits, PR titles or PR bodies.
- Never skip, disable, `#[ignore]` or quarantine a test to get CI green. Never push an empty commit to re-trigger
  CI. Never rewrite history on a shared branch: merge `main` into PR branches, do not rebase or force-push them.

---

## 2. What is done (merged into main)

| GitHub PR | Content |
|---|---|
| #1 | Gossip of locked templates and valid shares, per-address-group inbound cap |
| #2 | v3 design and owner decisions (docs) |
| #3 | Pull-only peer discovery (Hello 11 items, GetAddr/Addr, address book, persistence) |
| #4 | Pre-authentication DoS checks: no state rebuild before cheap checks; bounded rebuilds; verdict cache |
| #5 | 32 inbound slots, netgroup eviction |
| #6 | Consensus identity, consensus boundary and append-only lock (`consensus-boundary.txt`, `consensus-sources.lock`), launcher upgrade path |
| #7 | Pool and block builder count gas *used*, not declared |
| #8 | Bounded memory for the continuous chain (`canon.redb`, Canon window, checkpoint format 2) |

---

## 3. Open work. Do these first, in this order.

### 3.1 PR #9: automatic outbound connections (branch `claude/p2p-outbound`)
- **What it is:** the node dials peers by itself (outbound manager, feelers, anchors, seed policy, bootstrap list,
  `seedsRetired`). It is what lets the 3 seeds be retired. Decisions are in
  `development/decisions/P2P-OUTBOUND-DECISIONS.md`.
- **State (updated):** `main` (with #8) is already merged into this branch. The new head is `02386f5`, a merge commit
  with parents 4d41a0a and 40b0428.
  - All A13 pins were re-derived by running the tests. Each moved by exactly #9's +27,556,914 B; for example the
    `a13_bound_breakdown` total went from 3,357,540,586 to 3,385,097,500. The commit body has the full arithmetic.
  - The local full suite passed: 561 passed, 0 failed.
  - CI was running on `02386f5` when the session ended. **Skip steps 1–4 below unless `main` moved again; go to step 5
    (watch CI, then merge).**
- **What to do:**
  1. `git fetch origin && git checkout claude/p2p-outbound && git log --oneline -5`. If the last commit is a "Merge
     main (memory bounds) into automatic outbound; re-derive A13 pins" commit, the merge was pushed: go to step 4.
  2. Otherwise run `git merge origin/main`. Resolve conflicts keeping **both** sides:
     - `prototypes/pocold/src/resources.rs`: A13 terms. #8 added the canon / Canon-window terms; #9 added the outbound
       and address-book terms.
     - `prototypes/pocold/tests/consensus.rs`: `a13_bound_breakdown` pinned totals and the comment clauses.
     - `prototypes/pocold/tests/network_budget.rs`: its pins.
     - `development/A13-DERIVATION.md` (or the derivation doc the tests cite).
     - `prototypes/pocold/consensus-sources.lock`: **append-only**. Keep every line of both sides; main's history must
       be a prefix.
     - CI workflow files: keep both sides' added test targets.
  3. Re-derive every A13 pin by **running** the tests, never by hand:
     `cargo test --release --locked --test consensus a13 -- --test-threads=1` and `--test network_budget`. Update the
     pins and comment clauses to the printed values. Check that each pin moved by the sum of what #8 and #9 each added,
     and write that arithmetic in the merge commit body.
  4. Validate (section 6 lists all commands). Push with `git push origin claude/p2p-outbound`.
  5. Wait for CI. Known Windows-only causes we already fixed (do not regress them):
     - A test that types socket addresses only under `#[cfg(unix)]`.
     - The route-probe test assumed Windows refuses UDP connect to `255.255.255.255`. It does not; the test now picks a
       refused address per platform.
     - `deploy/mainnet/collect_seeds.py` `PREFLIGHT_SHA256` must equal the hash of `deploy/mainnet/preflight.py`. If
       you edit `preflight.py`, re-pin that hash after reviewing the change; the hash is computed on LF bytes.
  6. Merge when green and clean (merge commit, not squash).

### 3.2 PR #10: encrypted key backup, new Android application id, SECURITY.md (branch `claude/client-key-backup`)
- **What it is:**
  - `pocol export-key`, `inspect-key-backup` and `import-key`: a passphrase-encrypted backup with PBKDF2 at 600k
    iterations and AES-256-GCM.
  - Android backup and restore UI; `-PpocolApplicationId=org.pocol.wallet`.
  - Windows wallet backup and restore.
  - `SECURITY.md`.
  - All of it is node-local (`=` in `consensus-boundary.txt`).
- **State:** head `a24f61d`. All checks were green except `native (windows-2022)`. That job failed in
  `tests/cutover.rs::a_store_mode_cutover_70_blocks_below_the_head_rebuilds_the_state_and_matches_one_at_the_head`
  with `-32018 ParamGate rejected [R14, R19]`.
  - This failure is **not caused by #10**: main's own Windows run 42863094336 failed the same way. It is intermittent
    and Windows-only.
  - The failed job was re-run once. A comment on the PR explains this.
  - The one Codex finding ("Recover the activity-file move along with the restored key") was fixed in `a24f61d` and
    answered on the thread. The thread may still show "unresolved": resolve it in the GitHub UI.
- **What to do:**
  - If the re-run is green, merge.
  - If not, finish 3.3 first. Then port its fix into this branch (merge `main` after the fix lands, or cherry-pick
    it), push, and merge when green.

### 3.3 Intermittent Windows cutover failure on main (branch `claude/cutover-windows-flake`, if it exists)
- **The failure:** `tests/cutover.rs:188` gets `ParamGate rejected [R14, R19]` on Windows only, intermittently.
- **State (updated):** the root cause is found and the fix is open as **PR #11** (branch
  `claude/cutover-windows-flake`, commit `d57599f`). It changes test files only.
  - Cause: R14 and R19 are boot timing rules. Every calibration block, including its fsync to a scratch `state.redb`,
    must finish within 1 s. The cutover test binary ran four disk-heavy tests at once, so the failing test's
    calibration write waited behind the others' and went over 1 s.
  - Fix: the six node-booting tests run one at a time, and the error now reports the measured times.
  - Merge #11 when green, then merge `main` into PR #10 and merge #10 when green. Steps 1–5 below are already done.
  - Left for the owner: the patch to print R14/R19 timings needs a change in `pocold/src/lib.rs` (marked `+`), and
    whether one slow disk write should be able to refuse a boot at all is a rule question.
- **What to do:**
  1. Find which ParamGate checks R14 and R19 are: `grep -rn "R14\|R19\|ParamGate\|-32018" prototypes/`.
  2. Explain why they can reject on Windows: clock resolution (about 15.6 ms), equal timestamps, slow runners,
     deadlines in the test, or a race during the state rebuild.
  3. If the root cause is in the test or in node-local code, fix it with a test that fails without the fix. Open a PR
     to `main` and merge it when green.
  4. **If the root cause is in consensus code** (any file marked `+` in `prototypes/pocold/consensus-boundary.txt`, or
     ParamGate rules that are consensus), do **not** change it. Write up the root cause and a proposed patch for the
     owner.
  5. Do not "fix" it with retries or longer sleeps that have no root cause.

### 3.4 Smaller follow-ups
- On main, PR #8 made the Windows `pocold` CI job take about 69 minutes against a 90-minute step timeout. If it nears
  the limit, raise the timeout in the workflow or make the slowest consensus tests cheaper. Never skip tests.
- `net::rpc_work_deadline_skips_expired_work_and_releases_resources` can fail with "threads N vs N" when many tests run
  in parallel (a `/proc` thread count). It passes alone. Make it robust: measure its own threads, not the process
  total.

---

## 4. Then: implement v3 completely

Follow `development/handoff/V3-ROADMAP.md`. This is a summary of how; the roadmap gives the what, file by file.

### 4.1 The central constraint (roadmap §1)
- The live chain (H15000, spec 1, schedule `[(1,0),(2,15000)]`) must keep **byte-identical** verdicts from every main
  build until the v3 genesis.
- The pinned H15000 identity
  `bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110` must not change.
- **Never `bump` `CONSENSUS_RULES_VERSION` on main.**
- Every v3 rule is keyed on `v3_active(h) = active_version(h) >= 3 || genesis.spec_version >= 2`.
- Keep `SPEC_VERSION = 1`, `MAX_FORK_VERSION = 2` and `CONSENSUS_RULES_VERSION = 1` until the activation PR
  (V3-PR-I).
- A Cargo feature `v3-preview` lets tests decode spec-2 genesis files. A preview build carries identity flag bit 2 and
  is refused by admission and packaging for anything except simnet.
- **Re-pinning the lock** for a dormant v3 PR:
  ```
  POCOL_REPIN_CONSENSUS=neutral POCOL_REPIN_GATE=spec:2 POCOL_REPIN_REF=<PR number> \
  POCOL_REPIN_NOTE="dormant v3: <items>; reachable only on genesis specVersion 2, which SPEC_VERSION=1 builds refuse" \
  cargo test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test consensus_guard
  ```
  - A PR that changes pinned vectors (`lp4-execution/tests/data/`, `lp1-node/tests/data/`) uses
    `POCOL_REPIN_CONSENSUS=neutral-vector`, with no gate, and its note says "spec:2-dormant".
  - The lock is append-only. If another boundary PR merges first, merge `main` in and re-pin; the new line gets the next
    `n`.
  - `POCOL_RELEASE=1` builds refuse unpinned `+` edits; never work around that.
  - Before pushing, run `python3 deploy/ci/check_lock_append_only.py` (or the script the CI workflow names) against
    `origin/main`.

### 4.2 PR sequence (one GitHub PR each, merged in this order; details per PR in roadmap §4)
1. **V3-PR-A:** decision record `development/decisions/V3-DECISIONS.md` (the owner rule verbatim, the O-answers,
   supersessions) and the normative spec amendments. Docs only.
2. **V3-PR-B:** scaffold:
   - `spec_version` in `GenesisSpec`, CP schema v2, `v3_active`;
   - the `v3-preview` feature and identity bit;
   - the program-set registry (replacing `Spec::earnin()`, removing the `EARNIN_*` mirrors);
   - testnet and cutover genesis tooling with a `v3` switch;
   - a CI job running the v3 suites with `--features v3-preview`.
3. **V3-PR-C1** (membership and draw; removes the `active.is_empty() → live` fallback), **C2** (fork choice: timely
   signed precedence) and **C3** (execution caps and fees, log caps from CP). These can run in parallel after B.
4. **V3-PR-E0:** the v3 system-program set. Reproducible build (`compile_system_v3.cjs`, solc 0.8.28 from npm, pinned),
   a CI job that recompiles and compares bytes, constants read from CP.
5. **V3-PR-E1** registry budgets → **E2b** evidence penalties → **E3** late-share credit (evidence kind 3) → **E2a**
   issuance, carry, tip split and the supply invariant with a carry term → **E4** contestable seats and probation pool.
6. **V3-PR-F:** the v3 profile values (draft CP in roadmap §5, T_blk = 30 s per O-6=b) and resource proofs (A13).
7. **V3-PR-G:** off-node verifiers and clients: extension `rp.js`, `deploy/rewards/pocol_rewards.py`, wallets.
8. **V3-PR-H:** simulation harness (SimNet) and test vectors: adversarial share withholding, late shares, seat
   contests, reorgs.
9. **V3-PR-I:** activation. `SPEC_VERSION` becomes 2 in both copies, the preview restriction is removed, re-pin
   `neutral gate=spec:2` (this spends the gate). Do this **last**, only when A–H are green.
10. **V3-PR-J:** cutover rehearsal and WAN soak, on a private simnet or testnet only, never the live network.
11. **Parallel tracks** (node-local, any time; roadmap "Parallel tracks"):
    - N1: Δ_rel telemetry. Land it early; it feeds F.
    - N2: persisted finalized height and retention floor.
    - N3: `--mine-threads` and a local getWork.
    - N4: fee-ordered pool and replace-by-fee.
    - N5: computed `launch_ready`.
    - N6: builder flag "no user transactions after height S". It must ship in a release running on H15000 **before**
      the cutover.
    - K: in-place code/CP swap machinery, gated `fork:3`.

### 4.3 How to work each PR
- **Branches:** one branch per PR, from current `main`. Name it after the PR (for example `v3/pr-b-scaffold`).
- **Tests:**
  - Tests are written as sentences. Each new test must fail without the change: check this by reverting the source
    change once.
  - Every consensus change gets an old-vs-new differential test showing spec-1 verdicts are unchanged.
- **Memory pins:** keep the A13 pins exact. Any new memory term gets a pinned term, a CONDITIONS entry and a row in the
  derivation doc.
- **Commits:** small, with an imperative subject under 72 characters and a body that says why.
- **Before each push:** run the commands in section 6. Re-read your diff and ask what would make CI reject it.
- **After CI is green:** read the Codex review on the PR, fix valid findings, answer the rest with reasons, then merge
  (standing approval) unless the change touches a PoCol principle (section 1).

---

## 5. Things only the owner can do (remind the owner; do not do them)
- In GitHub → Settings → Advanced Security, turn on Dependency graph, Dependabot **alerts**, Secret scanning and
  **Push protection**. Leave Dependabot *security/version updates* off, because pinned consensus dependencies must not
  change automatically. Private vulnerability reporting is already enabled.
- Move the seeds' hot-key funds to an offline wallet.
- Create the real Android release signing key.
- Before launch, decide the re-genesis/cutover date and height S, and run 5–10 independent nodes and miners before
  stopping the 3 seeds (runbook in `development/public-network/OPERATIONS.md`).
- Signed releases, monitoring, a bug bounty and a "beta" label at launch.

---

## 6. Build and test commands (run from the repository root unless noted)

```
cd prototypes/pocold
cargo test --release --locked --lib
cargo test --release --locked --test consensus -- --test-threads=1
cargo test --release --locked --test caps -- --test-threads=1
cargo test --release --locked --test consensus_guard
cargo test --release --locked --test net
cargo test --release --locked --test relay
cargo test --release --locked --test discovery
cargo test --release --locked --test network_budget
cargo test --release --locked --test cutover
cargo test --release --locked --no-fail-fast          # the full suite (long)
cargo test --release --locked --features io-fault --test consensus -- --test-threads=1   # the io-fault group, as CI runs it
cd ../..
cargo test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml --lib
python3 -B deploy/standalone/test_launcher.py
python3 -B deploy/standalone/test_android_jni.py
python3 -B deploy/standalone/test_release_package.py
python3 -B -m unittest discover -s deploy/mainnet -p 'test_*.py'
```

- **Release only:** build and test with `--release`. Debug builds are very slow and fill the disk.
- **Windows:** check Windows compile errors locally with `cargo check --release --locked --target x86_64-pc-windows-gnu
  --tests --bins`. Remember that Windows locks open files (redb databases), has a coarser clock, and behaves
  differently for UDP connect to broadcast.
- **The CI jobs to watch:**
  - `consensus (ubuntu-24.04 / windows-2022)`;
  - `native (...)`;
  - `Windows Server 2022 (pocold, pocold-io-fault, lp1, lp4, extension)`. These evidence jobs only log
    `pocold exit 101`; download their evidence artifact for the test output;
  - `operations`, `tools`, `windows-collector`, `android`, `csharp`, `browser*`, `rehearsal`, `keys-and-enrollment`.

---

## 7. Definition of done
Every item in V3-ROADMAP.md is merged, every CI job is green on `main`, and the rehearsal (V3-PR-J) passes on a
private network. The live network is still untouched: deploy and cutover remain the owner's decision, using the
runbook.
