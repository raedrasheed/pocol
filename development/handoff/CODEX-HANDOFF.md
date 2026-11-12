# Handoff to Codex: finish the open PRs, then implement PoCol v3

Written 2026-11-11 by the previous agent (Claude Code); last updated 2026-11-12 ~17:40 UTC. This is your complete
brief. The repository is **raedrasheed/pocol** (it was renamed from `raedrasheed/pocol-import`, which redirects; some
tools still list it as `pocol-import`). The default branch is `main`; at this writing it was at `a664276` ("Merge pull
request #17").

## 0. START HERE: when the previous agent stopped, find the current state with this checklist

The previous agent may have stopped in the middle of a step. Before doing anything, establish the real state from
GitHub; trust the repository over this file.

1. `git fetch origin --prune` and `git log --oneline -15 origin/main`. Note which PRs are merged (expected: up to #20
   and #17; see section 2).
2. List the open PRs on GitHub. Expected open work (details in section 3):
   - **PR #21** V3-PR-E0 (branch `claude/v3-pr-e0-programs`): the dormant v3 program set.
   - **PR #22** N1 V3-05 (branch `claude/v3-n1-template-latency`): Δ_rel derivation and template-latency telemetry.
   - **PR #23** earn-in SYS gas envelope (branch `claude/earnin-gas-envelope`): node-local admission.
   - For each: if CI is green on its current head, there is no conflict and every valid review finding is fixed or
     answered, merge it (merge commit; standing approval, section 1). If CI is red, root-cause and fix it on the branch.
     If `main` moved and there is a conflict, `git merge origin/main` into the branch, keep both sides, re-pin the lock
     if the branch has a lock line (section 4.1), re-run the tests in section 6, push.
   - **PR events do not reach the session** (the Claude GitHub App is not installed on the repository). Poll the check
     runs of each head commit instead of waiting for a notification.
3. Then the stacked v3 branches, which are **not yet PRs**, in this order after #21 merges:
   `claude/v3-pr-e1-registry-budgets` (V3-PR-E1) → `claude/v3-pr-e2b-evidence-penalties` (V3-PR-E2b) →
   `claude/v3-pr-e3-late-shares` (V3-PR-E3, in progress). Follow the stacked-branch procedure in section 3.2.
4. Then continue with the rest of section 4 (E2a, E4, F, G, H, I, J), one PR at a time, each from the newest `main`.
5. Rules for every step:
   - Never deploy to the live network.
   - Never `bump` `CONSENSUS_RULES_VERSION`.
   - Never skip or ignore tests.
   - Never force-push a shared branch.
   - Stop and ask the owner if a change would alter collaborative consensus, the template block or a PoCol principle.

### Owner decisions recorded since the v3 options (all on `main`)
- **ParamGate boot timing (2026-11-11):** "yes" to printing the measured times in an R14/R19 refusal, "no" to one slow
  disk write refusing a boot. Recorded in `development/decisions/PARAMGATE-BOOT-TIMING-DECISIONS-2026-11-11.md`.
  Implemented by #12 and refined by #19 (store mode counts durable commit latency again, as a median that tolerates one
  stalled write).
- **O-13 follow-up (2026-11-12):** the owner confirmed option (a): keep the all-probationer exception below B_reg/4.
  Verbatim: «أوافق على (أ)، أبقِ الاستثناء». Recorded in `development/decisions/V3-OWNER-DECISIONS-2026-11-12.md`. The
  dormant v3 code already implements it; no code change.

### Questions waiting for the owner (do not decide them yourself)
- **D-V3-21 creation-burn user bound** (V3-DECISIONS.md, D-V3-21): option (a) node-local quote and refusal in the pool,
  `eth_estimateGas` and wallets, or (b) a consensus sender-signed cap. It must be decided before V3-PR-I. Until then it
  is the enforced launch blocker in `V3_LAUNCH_BLOCKERS` (`lp4-execution/src/admission.rs`, checked in `admit()`).
- Anything a later PR review flags as needing the owner. Read each PR description and its review threads.

Read this whole file first. After it, read in order:

1. `development/handoff/V3-ROADMAP.md`: the v3 implementation plan (PR sequence, consensus-lock strategy, ground truth).
2. `development/decisions/V3-DECISIONS.md` (D-V3-01..34), `V3-DESIGN.md`, `V3-OWNER-DECISIONS-2026-11-08.md` and
   `V3-OWNER-DECISIONS-2026-11-12.md`: the approved v3 design and the owner's answers.
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
- **O-13 follow-up (2026-11-12):** option (a), keep the all-probationer exception (section 0).
- **ParamGate boot timing (2026-11-11):** PG-1 yes, PG-2 no (section 0).
- **Seeds:** the 3 seed nodes will be stopped once peer discovery works and enough independent nodes exist.
- **The 4M allocation** is the owner's personal matter, meant to fund network development. Do not touch it.
- **Android signing:** the owner will create the real Android signing key.
- **Audit:** a human security audit happens after the real launch.

### Hard security constraints (never violate)
- Live network deploy or cutover is **on hold**. Do not deploy, do not re-initialise the network, never run
  `Initialize-PoColNetwork.ps1`.
- Never display, copy or publish operator secrets or server-side deploy data (the owner knows where they are).
  Preserve existing deploy data. Never delete wallet data.
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
| #10 | Encrypted wallet key backup |
| #11 | Cutover tests boot nodes one at a time (fixes the intermittent Windows `ParamGate rejected [R14, R19]`) |
| #12 | Robust ParamGate boot timing (measured times in R14/R19 refusals; robust statistic; one re-measurement) |
| #13 | V3-PR-A: v3 decision record (`V3-DECISIONS.md`, D-V3-01..34) and reference amendments |
| #14 | V3-PR-B: v3 genesis and `v3-preview` scaffolding |
| #15 | V3-PR-C1: v3 membership and proposer eligibility |
| #16 | V3-PR-C2: v3 timely signed sibling precedence |
| #17 | V3-PR-C3: execution caps and fees (see below) |
| #18 | Audit records for #12–#17 (A13 spec-1 impact of the C2 eviction batch recorded, V3-DECISIONS provenance, Python identity bit 2) |
| #19 | Store-mode R14/R19 count durable commit latency again (median, one stalled write tolerated); the Windows CI job prints failing test lines |
| #20 | V3-PR-C2b: v3 signed timeliness measured from the template lock time (poisoning resistant, bounded map) |

The audit of the earlier PRs #12–#17 is done; its fixes are #18, #19, #20 and the fixes merged with #17:
- the legacy receipt-scratch A13 under-count (+878,624 B on spec 1) is recorded;
- v3 receipts are counted in the heavy lane, and ParamGate R26 is added;
- the D-V3-21 creation-burn user bound is an enforced launch blocker (`V3_LAUNCH_BLOCKERS`, checked in `admit()`);
- the discovery test retries an empty status read.

---

## 3. Open work. Do these first, in this order.

**Update (2026-11-12 ~17:40 UTC).**

### 3.1 Open PRs (repository raedrasheed/pocol)
| PR | Branch | Content |
|---|---|---|
| #21 | `claude/v3-pr-e0-programs` | V3-PR-E0: dormant v3 program set (`PoColSystemV3.sol`, `system-v3.json`), reproducible solc CI workflow `system-programs.yml`, P_MAX/T_PROB read from CP |
| #22 | `claude/v3-n1-template-latency` | N1 V3-05: v3 Δ_rel derivation, per-proposer template latency telemetry (`pocol_templateLatency`) |
| #23 | `claude/earnin-gas-envelope` | Earn-in SYS gas envelope (node-local admission) |

Merge each when it meets the standing approval (section 1). #22 and #23 are independent of the v3 stack.

### 3.2 Stacked branches (not yet PRs; merge in order after #21)
1. `claude/v3-pr-e1-registry-budgets` (V3-PR-E1). Already merged onto the updated E0 branch.
2. `claude/v3-pr-e2b-evidence-penalties` (V3-PR-E2b):
   - offender-only type-2 burn and the reporter's cut ρ;
   - the whole pot burns only when the accused is the winner that was ACCRUING at `recordResult` (this interpretation
     is documented in D-V3-16);
   - v3 phase-0 reorder;
   - `GAS_ENVELOPE_V3`.
3. `claude/v3-pr-e3-late-shares` (V3-PR-E3, late share credit): **in progress**. Finish it per roadmap §4.

Procedure for each stacked branch, once its parent has merged or moved:
- Merge the updated parent (or `origin/main` once the parent has merged) into the branch. Never rebase.
- Resolve `prototypes/pocold/consensus-sources.lock` by taking the parent's version, then re-pin with
  `POCOL_REPIN_CONSENSUS=neutral-vector` (no gate: the guard refuses a gate on `neutral-vector`). The note starts with
  `spec:2-dormant`.
- `prototypes/pocold/tests/v3_membership.rs` "pr15-2" pins (`UNBOUNDED_PERIOD` / `UNBOUNDED_RECOVERY`) move with every
  program-byte change: 16/19 on main, 15/18 on E0, 14/17 on E1. Update them with a comment saying why. Never loosen a
  behavioural assertion.
- Open the PR to `main` only after its parent has merged, so the diff shows only its own change.

### 3.3 Follow-ups (add a PR for each; none is a reason to hold the open PRs)
- **V1 `GAS_ENVELOPE` settleFinal is under-measured for a tomb-free final block** (4,306,697 measured vs 4,230,000).
  No outcome change at G_SYS 96M, but fixing it changes the H15000 admission facts. Separate node-local PR.
- **A13 transactions-root clone** (`ordered_root` of the raw transactions) is not modelled. Separate node-local A13 PR.
- **Spec-1 earn-in ACC_COUNT slot** is not counted in `resources::derive`.
- **REG_OPS > REG_MAINT_MAX** gate rule: add in V3-PR-F.
- **`GAS_ENVELOPE_V3`**: re-measure at the v3 profile in V3-PR-F.
- **Wallet `rejoin_step`** must read the `C_react` clock: V3-PR-G.
- **D-V3-21 creation-burn user bound**: owner decision needed (section 0); must be resolved before V3-PR-I.
- **Off-node reward mirrors** (`deploy/rewards/pocol_rewards.py`, `refdec.js`, `rp.js`) must follow every E change:
  V3-PR-G.
- The first Windows `pocold` failure on PR #19 (2026-11-12 ~12:33 UTC, exit 101) did not reproduce on rerun and showed
  no test name. The job log now prints the failure lines; if it recurs, root-cause it from the log.

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
  - A PR that changes pinned vectors (`lp4-execution/tests/data/`, `lp1-node/tests/data/`, including every
    `system-v3.json` change in E0–E4) uses `POCOL_REPIN_CONSENSUS=neutral-vector`, with no gate (the guard refuses
    one), and its note starts with "spec:2-dormant".
  - The lock is append-only. If another boundary PR merges first, merge `main` in and re-pin; the new line gets the next
    `n`.
  - `POCOL_RELEASE=1` builds refuse unpinned `+` edits; never work around that.
  - Before pushing, run `python3 deploy/ci/check_lock_append_only.py` (or the script the CI workflow names) against
    `origin/main`.

### 4.2 PR sequence (one GitHub PR each, merged in this order; details per PR in roadmap §4)
Status as of 2026-11-12: A, B, C1, C2 (with C2b), C3 **merged**; E0 and N1 **in review** (#21, #22); E1 and E2b
**ready** (stacked, not yet PRs); E3 **in progress**.

1. **V3-PR-A:** decision record and normative spec amendments. Docs only. **Merged (#13).**
2. **V3-PR-B:** scaffold (`spec_version`, CP schema v2, `v3_active`, `v3-preview` feature and identity bit, program-set
   registry, genesis tooling with a `v3` switch, v3 CI job). **Merged (#14).**
3. **V3-PR-C1** (membership and draw), **C2** (fork choice: timely signed precedence; C2b timeliness from the template
   lock time) and **C3** (execution caps and fees, log caps, base-fee floor, creation fee). **Merged (#15, #16, #20,
   #17).**
4. **V3-PR-E0:** the v3 system-program set, reproducible build, CI recompile-and-compare job, constants read from CP.
   **In review (#21).**
5. **V3-PR-E1** registry budgets (ready) → **E2b** evidence penalties (ready) → **E3** late-share credit (evidence
   kind 3; in progress) → **E2a** issuance, carry, tip split and the supply invariant with a carry term → **E4**
   contestable seats and probation pool.
6. **V3-PR-F:** the v3 profile values (draft CP in roadmap §5, T_blk = 30 s per O-6=b) and resource proofs (A13). Also:
   the REG_OPS > REG_MAINT_MAX gate rule and the `GAS_ENVELOPE_V3` re-measure at the v3 profile.
7. **V3-PR-G:** off-node verifiers and clients: extension `rp.js`, `refdec.js`, `deploy/rewards/pocol_rewards.py`,
   wallets. Also: the client part of the D-V3-21 creation-burn user bound and wallet `rejoin_step` reading `C_react`.
8. **V3-PR-H:** simulation harness (SimNet) and test vectors: adversarial share withholding, late shares, seat
   contests, reorgs.
9. **V3-PR-I:** activation. `SPEC_VERSION` becomes 2 in both copies, the preview restriction is removed, re-pin
   `neutral gate=spec:2` (this spends the gate). Gate: `V3_LAUNCH_BLOCKERS` is empty and its source check is replaced by
   a test through `admit()`. Do this **last**, only when A–H are green.
10. **V3-PR-J:** cutover rehearsal and WAN soak, on a private simnet or testnet only, never the live network.
11. **Parallel tracks** (node-local, any time; roadmap "Parallel tracks"):
    - N1: Δ_rel telemetry. In review (#22); it feeds F.
    - N2: persisted finalized height and retention floor.
    - N3: `--mine-threads` and a local getWork.
    - N4: fee-ordered pool and replace-by-fee.
    - N5: computed `launch_ready`.
    - N6: builder flag "no user transactions after height S". It must ship in a release running on H15000 **before**
      the cutover.
    - K: in-place code/CP swap machinery, gated `fork:3`.

### 4.3 How to work each PR
- **Branches:** one branch per PR, from current `main` (or from its parent branch for a stacked PR, section 3.2). Name
  it after the PR (for example `v3/pr-b-scaffold`).
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
- Decide the D-V3-21 creation-burn user bound, option (a) or (b), before V3-PR-I (section 0).
- Optional: install the Claude GitHub App on `raedrasheed/pocol`, so PR events (CI results, reviews) reach the
  session.
- Start a session with `raedrasheed/pocol` as its source, so the agent can download CI artifacts.

---

## 6. Build and test commands (run from the repository root unless noted)

```
touch prototypes/pocold/build.rs                      # always, before building (see notes below)
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
cargo test --release --locked --features v3-preview --lib --test v3_scaffold --test v3_membership --test v3_fork_choice --test v3_execution --test cutover --test consensus_guard   # the v3-preview job (full-nonce-upgrade.yml)
cargo test --release --locked --no-fail-fast          # the full suite (long)
cargo test --release --locked --features io-fault --test consensus -- --test-threads=1   # the io-fault group, as CI runs it
cd ../..
cargo test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml --lib
python3 -B deploy/standalone/test_launcher.py
python3 -B deploy/standalone/test_android_jni.py
python3 -B deploy/standalone/test_release_package.py
python3 -B -m unittest discover -s deploy/mainnet -p 'test_*.py'
```

- **Release only:** build and test with `--release`. Never build debug: it is very slow and fills the disk.
- **Shared target dir:** sharing one `CARGO_TARGET_DIR` across worktrees of the same crates can reuse a stale `pocold`
  build script output. Always `touch prototypes/pocold/build.rs` before building.
- **Disk is tight:** delete old test executables in `target/release/deps` between branches.
- **Windows:** check Windows compile errors locally with `cargo check --release --locked --target x86_64-pc-windows-gnu
  --tests --bins`. Remember that Windows locks open files (redb databases), has a coarser clock, and behaves
  differently for UDP connect to broadcast.
- **The CI jobs to watch:**
  - `consensus (ubuntu-24.04 / windows-2022)`;
  - `native (...)`;
  - `Windows Server 2022 (pocold, pocold-io-fault, lp1, lp4, extension)`. Since #19 the job log prints the failing
    test lines. CI artifacts cannot be downloaded through the session proxy;
  - `v3-preview` (in `full-nonce-upgrade.yml`; also runs the lp1-node and lp4-execution `--lib` suites with the feature);
  - `System programs reproduction` (`system-programs.yml`, from #21: recompiles every `system-*.json`, compares bytes,
    runs the earn-in and v3 contract and chain suites);
  - `operations`, `tools`, `windows-collector`, `android`, `csharp`, `browser*`, `rehearsal`, `keys-and-enrollment`.
- **GitHub access:** PR events do not arrive in the session; poll check runs for the head commit.

---

## 7. Definition of done
Every item in V3-ROADMAP.md is merged, every CI job is green on `main`, and the rehearsal (V3-PR-J) passes on a
private network. The live network is still untouched: deploy and cutover remain the owner's decision, using the
runbook.
