# M1 — Specification Draft 0.7: Amendments

Status: **review draft, NOT approved. Phase S (specifications and reference tooling) only.**
- No production code, installs, deployments, real transactions, subagents or memory notes.
- **Nothing in 0.7 was executed by the author.**

Inheritance: 0.7 = 0.2 + the amendments of 0.3 to 0.6 + the amendments below. Unchanged material is inherited by reference.

| ID | Issue / row | Subject |
|---|---|---|
| R7-01 | BR19 checker | Snapshot-field collision fixed in the checker; bucket semantics unchanged |
| R7-02 | C20 | Faithful single-rule disabling for path rules; isolation of neg-path-empty corrected; mutant chunks never cached |
| R7-03 | C21 | U42 withdrawn: signEligible is a pure predicate; stale requests are cancelled and need a fresh request |
| R7-04 | R1–R4 (D98) | Receive scopes connected to the pools; read-step and pool model; MaliciousHttp literal scripts; MR10 |
| R7-05 | S1–S4 (D99) | SiteSession lifecycle and orphan states, nav bucket, BR20a/b/c/d, BridgeRef session extension |
| R7-06 | C06 context | Alternative E (single-call `eth_call` getter): consistent against an honest node, NOT equivalent in RP; CR-M1-01 unchanged |
| R7-07 | B7 | Global-fetch policy enforced by a sufficient criterion; the alias bypass is closed |

## R7-01 — BR19 checker

The 0.6 checker stored `{'tokensBefore': snapshot, **r}`. For a size rejection, `r['tokensBefore']` is `None`, and the unpacked `r` overwrote the snapshot. The 0.7 checker keeps the snapshot in a separate field (`snapshotTokensBefore`).

The bucket model, BridgeRef's own `tokensBefore_mt` and the size-rejection semantics are unchanged. Check: `br19a.a7` over all 265 lines.

## R7-02 — C20 rule-disable instrumentation and the cache

- **Path rules.** `path_violations()` lists every violation in precedence order: `path.length`, the leading-slash `path.grammar`, then for each segment from left to right `path.dotSegment` or `path.grammar`.
  - With all rules enabled, the first violation is reported exactly as in 0.3.
  - When the harness disables one rule, only that rule is skipped, and the next violation is reported.
  - Fixtures (`vectors/c20-cases.json`): `/a//..` → `path.dotSegment`; `/./%` → `path.grammar`; `''` → `path.grammar`; plus 5 more multi-fault and control cases. The runner also shows the 0.3 behaviour (accept) for the three Codex probe paths.
- **Isolation correction.** 0.2 `neg-path-empty` was labelled `full`. It is now `next` → `path.grammar`, because without the length rule the missing leading slash still fails.
- **Regression.** Every 0.2 positive, negative and version-record fixture is re-run under the corrected instrumentation, with every isolation claim (only the one correction above).
- **Cache.** A chunk is inserted into the session ChunkCache only if none of its stage's four fetch rules was disabled while it was checked. A mutant-accepted response therefore cannot be served later to a stage whose rule is enabled.
  - `CACHE-1` covers the cross-stage case: a forged chunk accepted with `mfetch.origin` disabled is not reused, and `fetch.origin` rejects it. Under 0.3 it was served from the cache (bypass), and the runner demonstrates that.
  - `CACHE-2`: honest sharing is unchanged (1 request).
  - `CACHE-3`: a content-stage mutant is not cached.
  - Normal (unmutated) behaviour and request counts are unchanged.

## R7-03 — C21 epoch predicate (U42 withdrawn)

`signEligible` is a **pure predicate**. Condition 6 is `eligibilityEpoch == req.epoch` (FD:L1323), and the request is never rewritten.

- Every commit re-evaluates pending requests. A request that fails only condition 6 is **stale**: it is cancelled with 4901 (P, U43) and needs a fresh request and a fresh approval.
- Substantive conditions (1–5) are reported before staleness. For example, a revocation still gives 4100.

Fixtures (`vectors/epoch-cases.json`):
- **Purity:** the Codex probe case (captured 0, world 1) now gives 4901 twice, with the request unchanged.
- **Stale cases:** acceptance after a commit; a fresh request succeeds; substantive before stale; a mutation queued during an acceptance commits after it; an out-of-worker write detected later.
- **SE-benign-mutation, replaced:** r1 is cancelled with 4901 at the commit, its later acceptance has no effect, and a fresh r2 is sent.
- All other 0.6 race scenarios are re-run unchanged.

## R7-04 — D98 (R1–R4)

- **R1 scopes** (`annex/recv-scopes.json`): every caller has a pool, a session/in-flight counting rule, a deadline and a recvLimit.
  - SITE_POOL: bridge reads and fetchAll; session- and in-flight-counted.
  - CONTENT_POOL: ChunkFetcher; in-flight-counted. P: not session-counted.
  - TRUST_POOL: HeaderNetCheck and NetworkProfiles; outside the in-flight count.
  - WalletSubmit → TRUST_POOL (P: the baseline assigns no pool).
  - STATE_POOL: only if CR-M1-01 is approved.
- **R3 read steps and pools** (`tools/recvguard_ref.py`):
  - steps 2–6: Content-Length pre-check; count after decompression; drop the piece that crosses the limit; fatal UTF-8; parseStrict with depth ≤ 16 and no duplicates; error-message truncation to 256 UTF-16 units; `data` dropped above 4 KiB;
  - step 1: pools with FIFO and a 2 s busy timeout, SESSION_RECV_MAX and RECV_INFLIGHT_MAX;
  - P: strict FIFO per pool, so the waiting head blocks later requests in that pool.
- **R2/R4 MaliciousHttp scripts** (`annex/malicioushttp-scripts.json`): byte-exact wire format (P). Large repeats are expressed by count. MR4 is a deterministic gzip of 2³⁰ zero bytes; the runner records its length and sha256.
  - Model-checked: MR1–MR5, MR6/6b/6c/6d (accept at the limit, reject one byte over; MR6d has the n=128, p=16 feeHistory shape at exactly 168015 bytes), MR8, MR9, MR11, MR12, and MR-neg.
  - MR10a/b/c/f and BR20d are model-checked with literal timings (`vectors/mr10-cases.json`).
  - Future (Phase A): CAP1, MR7 timing, the browser consequences of MR8 and MR9, MR10d memory, and the common C criteria.

## R7-05 — D99 (S1–S4)

- **S1 lifecycle and orphan states.** In `vectors/br20-cases.json`, `S1lifecycle`:
  - the session key is (tabId, netKey, siteAddress), created only by a user action;
  - the session ends when the tab closes or on navigation to another address or network;
  - the session owns its budgets; frames join without renewal;
  - one LoadJob per session; blob URLs are revoked on teardown;
  - messages from a closed port are dropped and consume no token;
  - orphans go live → orphan → settled.
- **S2 nav bucket:** 6000 capacity, refill 1 per ms, cost 2000 (FD:L1196). BR20b is executed.
- **S3 BR20a:** every cell of the table is executed (`tools/session_ref.py`), with the P extra row "late message from the closed frame F1 is dropped". The negative control `frameBudget` must differ at m51 (pass / forwarded).
  - BR20c: the G bounds are checked on a model run (P: retry every 100 ms, zero load latency, 7 site chunks). The browser run and the C measurements are future.
  - BR20d is model-checked with the pools (16 forwarded, 4 pending; the trust check succeeds).
- **S4 BridgeRef session extension:** the session model emits BridgeTrace-format lines with shared budgets across frames, orphan settlement and nav decisions.

## R7-06 — Alternative E (evaluation)

See `annex/snapshot-alternative-E.md`.
- One `eth_call` to a P getter `websiteSnapshot(uint32)` returns the whole tuple from one node snapshot.
- It is baseline-compatible in transport (`eth_call` is listed), and it runs the same R4-01 invariant checks.
- Against an honest LN or DEV node it is single-state, consistent with D13. **It does not bind the state to an authenticated root:** a dishonest RP endpoint can fabricate a published tuple, and E renders it (fixture E-2), whereas P20 rejects forged proofs.
- Therefore E is not equivalent, and the requirement is not weakened.
- CR-M1-01 stays the pending owner decision. E appears only as an LN/DEV option inside it, and no new question is raised.

## R7-07 — Global-fetch criterion

`annex/fetch-rule-criterion.json`.
- **C1 (static):** no free reference to the network globals, nor to `globalThis`, `self`, `window`, `frames`, `parent`, `top`, `opener`, `Reflect`, `Function` or `eval`, outside HttpTransport. Any alias of the global object therefore needs a flagged reference, so the 0.5 residual `const g = globalThis; g['fe'+'tch']` is now a violation.
- **C2 (runtime):** MV3 forbids `unsafe-eval`, so constructor-chain code construction throws, which closes the only unnamed path to the global object.
- **C3 (optional):** a throwing `fetch` stub installed after HttpTransport captures `fetch`.
- C1 and C2 together are sufficient. The import rules stay in dependency-cruiser.
- No fixture is labelled as a passing bypass. The fixtures are expected CI verdicts; nothing is installed or run.
