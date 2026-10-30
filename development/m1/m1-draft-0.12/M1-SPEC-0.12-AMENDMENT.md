# M1 — Draft 0.12: Amendment R12-01 (C25 repair only)

**Status:** review draft, **NOT approved**. Phase S only.

- **Nothing in 0.12 was executed by the author.** The session had file tools only.
- Only new files under `m1-draft-0.12/` were written. 0.10 and 0.11, their results, the review evidence (including the failed C24 and C25 proofs), the ledger, the GUI and the checkouts are unchanged.
- The last accepted batch is still 0.9.

**Task.** `coordination/task-010.md`.

**Review input.** `coordination/review-001/REVIEW-0.11.md` and `namespace-independent-probe-0.11.{py,json}`.
- The probe started with F5 plus 7 distinct LF-tailed `pocol:epoch:` names: 8 physical names.
- 0.11 confirmed a new F6, which gave **9 physical names**, state `confirmed`, and violations `[]`.

## C25: the defect

The baseline (browser.md:509–510) lists **all** `pocol:epoch:*` items with `getKeys`. EN is their count, and EN + 1 ≤ 8 is checked **before** issuing anything.

0.11 used the number of *canonical* items as EN, so malformed names fell out of the physical name budget. The author had disclosed this as RF-9; the review rejected it as a default.

## R12-01 repair (`tools/sr_ref.py`, drop-in)

**Mechanism.** It loads the 0.11 module read-only, which in turn loads 0.10 read-only and applies C24. Then it rebinds `World.__init__`, `_attempt`, `_observe` and `gen_summary` in memory, and adds `raw_epoch_names`.

**Two counts, kept apart:**

| Count | What it covers | What uses it |
|---|---|---|
| **Raw namespace** | every `str` name starting with `pocol:epoch:`, canonical or not | the EN gate (before any `set`, data or snapshot); the name-budget invariant; `maxEpochNames` |
| **Canonical epochs** | names passing the strict C24 parser; invalid names never contribute an epoch number and are never normalized | M and E = M + 1; the generation window; `lateGens`; the sweep; record lookup |

**Sweep: unchanged.**
- Only canonical names below the canonical maximum are removed, under the existing T_LATE rule.
- **Malformed items are preserved.** There is no approved cleanup policy, so no automatic deletion was added.
- The raw EN is re-read after the sweep. If it is still 8: no issue, then the persistent `epochNames` failure with action 25 (existing rule).

**Invariant.**
- The raw count going above 8 is a violation **when it increases**.
- An overfull initial namespace is reported (`initialNamespaceOverfull`, `namespaceOverfullNotWorsened`) and never made worse.
- The negative controls `epochNonceNames` and `epochNoGate` still grow the namespace to 41, so they are still detected.

**Field meaning.**
- `gen_summary().EN` keeps its 0.10/0.11 meaning, the canonical count at issue. It is report-only.
- The new `ENraw` is the gate value.
- For namespaces without malformed names the two are equal, which is the case in all baseline vectors.

**Unchanged:**
- clock, scheduler, retry caps, checkpoints, `seq`;
- disk and site adapters;
- the invalid-bootMs choice (P-C24-3) and canonical ranges;
- owner policy.

No E4–E7 work.

## Fixtures (`vectors/c25-namespace.json`)

**Review probe.**
- Pristine 0.11 must reproduce 9 physical names, `confirmed`, `[]`.
- The repaired model must stay `gateBlocked`, with 0 fence sets.

**New cases.**

| Case | Namespace | Repaired outcome (hand derived) | Pristine 0.11 |
|---|---|---|---|
| N1 | F5 + 6 LF (raw 7) | issues **one**: E6, ENraw 7, raw maximum 8; after the sweep raw 7; 6 malformed kept | same |
| N2 | F5 + 7 LF (raw 8) | blocked before any set. Sweep at 60000 removes nothing. At 60001 still 8, so disabled `epochNames`, action 25. The write gets −32603 `{storeRecovery, epochNames}`. 0 snapshots. Raw 8 throughout | confirmed E6, raw 9 (**C25**) |
| N3 | F5 + 8 LF (raw 9, initially overfull) | blocked, then disabled. Reported, never worsened (raw stays 9). No violation | raw 10 |
| N4 | F5, F6, LF, CRLF, nonce, uppercase, bare-prefix names (raw 7) and 5 lookalikes outside the namespace | E **7**: malformed names never move the maximum, and lookalikes are not counted. Raw maximum 8. The sweep removes F5 and F6 only | same |
| N5 | F5..F8 + 4 LF (raw 8), removes fail | blocked. The sweep tries F5..F7 and fails. Physical count stays 8, so disabled | confirmed E9, raw 9 |
| N5b | as N5, removes succeed | the sweep removes F5..F7, raw is 5, E9 confirms at 60002. Malformed names stay | confirmed E9 at 1, raw 9 |

The raw count is measured by the runner's own instrumentation subclass, not taken from the model's statistic.

## Amended inherited expectation (exactly one)

**Fixture:** `m1-draft-0.11/vectors/c24-regressions.json`, `world[1]`, "W-A2 malformed names do not fill the name gate" (F5 + 7 LF names, run until 100).

| | Expectation |
|---|---|
| **Old (0.11)** | `gen1 {E: 6, EN: 1, state: confirmed}`, `epochGateBlocks 0`. This encoded C25 |
| **New (0.12)** | `gen1 {E: null, EN: null, state: gateBlocked}`, `epochGateBlocks 1` |

- The fixture is kept, and its pristine-0.10 expectation is unchanged.
- The runner asserts three things:
  - the old expectation text matches the 0.11 file;
  - the repaired model now **rejects** the old expectation;
  - the amended expectation passes.
- The continuation past the sweep (disabled, `epochNames`, action 25) is case N2.

All other inherited tests and controls keep their expectations unchanged, including W-A1, W-B and W-C. In particular, W-A1 (F5 + one LF name, raw 2) still issues E6, legitimately.

## Re-run

`tools/run_checks_012.py` re-runs in memory:
- the full 0.11 suite: the 24 C24 probes, the preserved pristine-0.10 failures, names, generators, values, and the World regressions with the single amendment;
- through it, the entire 0.10 suite: E1, BR22h h1–h6 with 2 × 729 h5, BR22b, retry/sweep, 520 BR22a combinations, all legacy controls.

Results are prefixed `inherited011.` and go only to `results/run-results-0.12.json`.
