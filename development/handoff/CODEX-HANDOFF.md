# Handoff to Codex: finish the open PRs, then implement PoCol v3

Written 2026-11-11 by the previous agent (Claude Code), whose session is ending. This is your complete brief. The
repository is **raedrasheed/pocol** (the older name `raedrasheed/pocol-import` redirects to it). The default branch is
`main`; at this writing it was at `40b0428` ("Merge pull request #8").

## 0. START HERE: when the previous agent stopped, find the current state with this checklist

The previous agent may have stopped in the middle of a step. Before doing anything, establish the real state from
GitHub; trust the repository over this file.

1. `git fetch origin --prune` and `git log --oneline -15 origin/main`. Note which PRs are merged.
2. List the open PRs on GitHub. Expected work, in this order:
   - **PR #10** (`claude/client-key-backup`, encrypted key backup):
     - Head `e78aed0` has `main` merged in. Its CI was running.
     - If it is still open: when CI is green, merge it (merge commit).
     - If CI is red: root-cause the failure and fix it on that branch.
     - If `main` moved and the PR has a conflict: `git merge origin/main` into the branch, keep both sides, re-run the
       tests in section 6, push.
   - **V3-PR-A** (branch `claude/v3-pr-a-decisions`, docs only):
     - Contents: `development/decisions/V3-DECISIONS.md` (the owner rule, the O-answers, decisions D-V3-01..34, the lock
       strategy), `decisions.json` entries D-V3-01..34, and the v3-scoped amendments to `development/m1/reference/`
       `consensus.md`, `threat.md`, `economics.md`, `incentives.md` and `governance.md`.
     - It was being written and reviewed when the agent stopped.
     - Run `git log --oneline origin/main..origin/claude/v3-pr-a-decisions`. If the branch does not exist or is
       incomplete, write it yourself from section "V3-PR-A" of `development/handoff/V3-ROADMAP.md`.
     - Check it against the owner rule (section 1). Validate `decisions.json` with
       `python3 -c "import json;json.load(open('development/m1/reference/decisions.json'))"`.
     - Open a PR to `main` titled "v3 decision record and normative amendments (docs)". Merge it when CI is green.
       It must merge **before** any v3 code.
3. Then continue with V3-PR-B and the rest of section 4, one PR at a time. Each starts from the newest `main`.
4. Rules for every step:
   - Never deploy to the live network.
   - Never `bump` `CONSENSUS_RULES_VERSION`.
   - Never skip or ignore tests.
   - Never force-push a shared branch.
   - Stop and ask the owner if a change would alter collaborative consensus, the template block or a PoCol principle.

### Owner decisions on the two PR #11 questions (decided 2026-11-11; implement as one PR after PR #10)
The owner answered both questions: **"yes" to question 1, "no" to question 2.** Implement them as one PR from the newest
`main`, before V3-PR-B, titled "ParamGate boot timing: report measured times, robust calibration".

1. **Yes: report the measured times.** A ParamGate R14/R19 refusal must state:
   - the per-block execution ms;
   - the store/fsync ms;
   - the sample count;
   - the `t_exec` limit.

   This is diagnostics only and changes no verdict. Because `pocold/src/lib.rs` is marked `+`, re-pin the lock with a
   `neutral` line and a note.
2. **No: one slow disk write must not refuse a boot.**
   - Keep R14 and R19, and keep `t_exec` at 1 s.
   - Take several samples per calibration block, and compare a robust statistic (median or p90) with `t_exec`.
   - Measure execution time and store/fsync time separately. `t_exec` applies to execution; report store latency,
     with its own wider bound if needed.
   - On a refusal, re-run the calibration once before refusing. A host that is really too slow must still be refused.
   - Add tests:
     - one injected slow commit is admitted;
     - a sustained slowdown is still refused with `[R14, R19]` and the times.
   - Use a `neutral` lock line with a note. There is no change to block validity, so this is within the owner rule.

### Questions waiting for the owner (do not decide them yourself)
- The PR #11 questions are decided (see above).
- Anything the V3-PR-A reviewers flagged as needing the owner. Read the PR description and its review threads.

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
| #9 | Automatic outbound connections (outbound manager, feelers, anchors, bootstrap list, `seedsRetired`) |
| #11 | Cutover tests boot nodes one at a time (fixes the intermittent Windows `ParamGate rejected [R14, R19]`) |

---

## 3. Open work. Do these first, in this order.

**Update (2026-11-11 08:50 UTC):**
- PR #9 (automatic outbound) and PR #11 (Windows cutover test fix) are **merged**. `main` is now at `2a4ba1c` or later.
- PR #10 (encrypted key backup, branch `claude/client-key-backup`) has the new `main` merged in at `e78aed0`. Its one
  conflict was the CLI flag table and usage line in `src/bin/pocol.rs`, kept from both sides. lib, cli,
  consensus_guard and the Python suites pass locally.
- **To do:** wait for PR #10's CI on `e78aed0`. If it is green and clean, merge it (merge commit).
  - Its only review thread, the Codex "activity-file move" finding, was fixed in `a24f61d` and answered. Resolve the
    thread in the GitHub UI if it still shows unresolved.
  - If CI fails, root-cause the failure and fix it on the branch.
- **Owner's open questions from PR #11** (do not change these without the owner):
  1. Should the ParamGate R14/R19 error print the measured times? That needs a change in `pocold/src/lib.rs`, which is
     marked `+` and needs a lock line.
  2. Should one slow disk write be able to refuse a boot at all?

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
