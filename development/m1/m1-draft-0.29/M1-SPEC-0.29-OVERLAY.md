# M1 Draft 0.29: effective-specification overlay (delegated technical decisions)

**For root review. The author ran nothing. M1 is not complete.**

## 0. Authority and limits

- **Source of the nine choices.** `coordination/DELEGATED-M1-DECISIONS-029.json`. The ledger's `standingAuthorization` records:
  - `delegatedTechnicalDecisions: true`;
  - `personalApproval: false`;
  - `noProduction: true`.
- **What they are.** These are **coordinator delegated technical decisions**, not personal owner approvals.
- **Owner questions.** The saved owner questions stay **unanswered**. This package neither reads nor writes the owner-answer record.
- **The named-decider gate (c4)** is met for a delegated item only by all four of:
  - the recorded user delegation;
  - the specific coordinator decision;
  - this package's checks for the item passing (`decision.<id>.applied` in `tools/run_checks_029.py`);
  - root's review.
- **Scope.** The overlay applies only in this new revision.
  - Every earlier baseline, draft, result, review, GUI file and checkout stays unchanged and readable as history.
  - Another selection needs a new revision and a re-run of the affected fixtures.
- **Not done.** No production code, deployment, transaction or real ownership transfer is made or implied.

**Effective M1 profile** = `reference/` baseline, then the accepted drafts 0.2–0.28, then this overlay.

Where the overlay supersedes a sentence, that sentence is cited below by file and line. The sentence itself is not edited.

## 1. The nine decisions

Each entry gives:
- the effective text, verbatim from `decisions/adopted-decisions-0.29.json`;
- the original source;
- the alternatives, the rationale and the consequences (from the register);
- the gates and hash groups it affects.

### 1.1 U01 — reference factory address (`reference-address`)

> The M1 reference factory address is 0x0000000000000000000000000000000000c0c005 (the 0.2 P28 expansion of the elided FD:L777 value). It binds every M1 fixture. It is NOT a deployment address and confirms no deployment; choosing another address later regenerates every address-dependent fixture.

- **Source.**
  - FD:L777 `ChunkFactory: 0x…C0C005` (elided).
  - m1-draft-0.2 M1-SPEC-0.2.md:57 (P28).
- **Alternatives.**
  - Use the existing full 20-byte fixture address (chosen).
  - Choose another address and regenerate every dependent value.
- **Rationale.** The address is fully specified and reversible, and it matches the vectors already verified independently. No real deployment address is inferred.
- **Consequences.** M1 fixtures bind to this address. A later deployment address requires regeneration, not personal confirmation.
- **Gates.**
  - c4 and c5 of C1–C4 (owner blocker U01);
  - findings F06 and F23.
- **Hash groups.** Every 0.25 export entry with owner parameter U01, which is checked by `U01.hashEntriesUsingTheAddress`:
  - `triad.codeTable`, `triad.manifests`;
  - the aliases `alias.versionRecords` and `alias.t1_02.manifestHash`.
- **Checks.** `U01.*`: the 0.2 model constant, the code-table and draft01 address evidence, and the not-a-deployment flag.

### 1.2 U02 — explicit view of a revoked version (`interstitial`)

> Default display of a revoked version stays refused. An explicit @v<n> of a revoked version shows a warning page that names the version as revoked, fetches no chunk, and loads only after a deliberate click; the loaded version then carries a persistent 'revoked' banner on every page of the session.

- **Source.**
  - FD:L914 ("revoke prevents the default display");
  - M1-SPEC-0.2.md:221 (the U02 row);
  - 0.25 S25-E07 T43-9.
- **Alternatives.**
  - Warning, explicit confirmation and persistent banner (chosen).
  - Always refuse explicitly selected revoked versions.
- **Rationale.** The default revocation ban is kept, and deliberate historical inspection stays possible. The baseline does not forbid explicit selection.
- **Consequences.** Viewing a revoked version needs a warning and a deliberate click, and default display stays refused.
- **Gates.** c4 and c5 of C3; finding F04; E07 definition completed.
- **Hash groups.** None.
- **New conventions** (§5): P-U02-1, P-U02-2.
- **Fixtures.** `vectors/u02-revoked-viewer-0.29.json`. It has 13 rows, among them:
  - the default view, including a non-conforming state whose current version is revoked;
  - explicit view without a click, with a click, after dismissal, after navigation and after a reload;
  - the rejected `refuse` column, kept for comparison;
  - the P-U02-2 timing case.

### 1.3 U10 — previous owner's publisher rights (`keep-with-warning`)

> acceptOwnership leaves the publisher set unchanged. The publisher tool warns that the previous owner keeps createVersion, publish and setCurrent until the new owner calls setPublisher(previous, false). No real transfer is made or implied.

- **Source.** M1-SPEC-0.2.md:235 (P09), 252 (`acceptOwnership`), 426 (T1-12).
- **Alternatives.**
  - Keep the old publisher until it is explicitly removed (chosen, with a warning).
  - Revoke the old publisher automatically on transfer. This is executed only as a comparison, in `U10.rejectedAlternative.P-1`.
- **Rationale.** The owner and publisher roles stay separate, T1-12 is unchanged, and the residual authority becomes visible.
- **Consequences.** The warning must disclose the retained rights. The new owner must explicitly remove the previous publisher.
- **Gates.** c4 and c5 of C3 and C4; findings F11 and F17; E01 (T1-12).
- **Convention.** P-U10-1.
- **Fixtures.** `vectors/u10-publisher-transfer-0.29.json`, cases P-1 to P-6:
  - transfer with the retained right, then removal;
  - no owner rights for the old owner;
  - self-removal before transfer;
  - a no-op second removal;
  - removal between nomination and acceptance;
  - a cancelled nomination.

### 1.4 U14 — the 10-second deadline (`whole-load-through-verdict`)

> Each covered operation has one 10000 ms budget from the send of its first request through its final verdict, including reply waits, retry sleeps and computation. An event at elapsed >= 10000 is expiry; nothing succeeds at or after it. Request timeout = min(10000, remaining budget).

- **Covered operations:**
  - website content retrieval and load;
  - the HeaderNetCheck RP initial load, including `eth_blockNumber`, the `pocol_getHeaders` retries and verification.
- **Source.** FD:L905 and network.md:374 ("timeout 10s").
- **Superseded.** M1-SPEC-0.2.md:159 (the per-request preference and the ban on a whole-load deadline).
- **Not changed.** implementation.md:203 (the per-request 10 s of RpcReadClient for site frame reads) is not a covered operation.
- **Alternatives.**
  - Per-request 10 s only.
  - Whole load until the last reply.
  - Whole load through the verdict, including computation (chosen).
- **Rationale.** Root's independent timing cases show that the narrower scopes let 14.05 s loads and computation escape the deadline. One end-to-end budget bounds remote stalls.
- **Consequences.** Slow legitimate loads can fail. CR-M1-01's per-request and cumulative-wait limits are amended explicitly in §2.
- **Gates.** c4 and c5 of C3 and V1; finding F25; new Phase A definition X-U14.
- **Conventions.** P-D29-1, P-D29-2, P-D29-3.

### 1.5 CR-M1-01 — proof-bound website-state reads (`adopt-rev2-with-explicit-deadline-amendment`)

> Proof-bound website-state reads of CR-M1-01 rev 2 are adopted for M1 with: internal-only eth_getProof, reply limit 524288 bytes, STATE_POOL 1 MiB, sites still forbidden, 6-key proof 2 under the U08 canonical split. Amendment: per-request deadline = min(10000, remaining whole-load budget); -32021 delays follow busy [250, 2000] / rate [0, 2000] with at most 3 retries; the 0..10000 single wait and the 20000 ms cumulative wait are superseded; no wait or success beyond the whole-load deadline. Support of eth_getProof by pocold is NOT claimed: E03 stays a later experiment.

- **Source.**
  - m1-draft-0.4/CR-M1-01-STATE-PROOF-READS.md:9 (site ban) and 54–58 (524288, STATE_POOL);
  - m1-draft-0.5/CR-M1-01-REV2.md.
- **Superseded** (the text stays in the file as history):
  - REV2.md:24 "the 10 s per-request deadline";
  - :41 "integer in 0–10000 (P)";
  - :85 single wait 0–10000;
  - :86 cumulative wait ≤ 20000.
- **Unchanged.**
  - Attempts ≤ 4, so sends ≤ 48.
  - The worst-case bytes bound of 19464192.
  - The MPT checks, the site ban, the 524288 reply limit, the 1 MiB pool and the 6-key proof.
- **Alternatives.**
  - Proof-bound internal reads (chosen, with the amendment).
  - Amend the mechanism.
  - Keep the reread detector with a weaker guarantee.
- **Rationale.** Proof-bound consistency avoids an unsupported snapshot claim under A→B→A. The reference fixtures support the definitions, not node compatibility.
- **Consequences.** E03 stays a precisely defined later experiment, amended in `vectors/experiment-amendments-0.29.json` (MPT-15 replaced). No measured node support is claimed.
- **Gates.** c4 and c5 of C3 and R1; finding F07.
- **Fixtures.**
  - The literal proof fixtures of 0.5 are retained.
  - The seven BUD budget fixtures are re-expressed under the overlay. Only BUD-3 changes, and it is marked SUPERSEDED.

### 1.6 U08 — canonical manifest split (`A-canonical-contract-and-viewer`)

> The contract (createVersion: ManifestSplit) AND the viewer (rule manifest.split) require the canonical split: exactly ceil(len/24575) chunks, all 24575 bytes except the last. The baseline's any-split input domain is preserved as history only.

- **Source.**
  - storage.md:32–36 (the baseline, with no split rule);
  - M1-SPEC-0.2.md:278 (P22), 245 (`createVersion` precedence), 428 (T1-14), 141 (viewer rule);
  - m1-draft-0.26/owner/U08-CHANGE-REQUEST.md.
- **Alternatives:**
  - A: canonical in contract and viewer (chosen);
  - B: at most 3 chunks, flexible sizes;
  - C: unrestricted split with larger proof budgets;
  - D: unrestricted contract with viewer rejection.
- **Rationale.** All valid content stays representable. The deterministic split bounds writes and the 6-key proof. The narrowing is explicit.
- **Consequences.** Only the publisher's choice of split is removed. Stored versions are canonical, so there is no "stored but unviewable" class.
- **Gates.** c4 and c5 of C3 and C4 (ledger token U08-CR); findings F09 and F17.
- **Hash groups.** None directly. The 0.25 freeze plan names U08 as a parameter, but no entry of the 0.25 export carries it. The runner resolves U08 only if such an entry ever appears.
- **Convention.** P-U08-1.
- **Fixtures.** `vectors/u08-canonical-split-0.29.json`:
  - boundary lengths 1, 24575, 24576, 49150, 49151 and 65536, run through the real 0.2 viewer rule, a full manifest-stage pass with real chunks, and the contract arguments;
  - lengths 0 and 65537;
  - the array mismatch;
  - 12 noncanonical splits, up to 65536 one-byte chunks;
  - the 6-key proof checked for every length 1..65536.
- **The 6-key proof and its maximal assumptions.**
  - The proof assumes PR-1 (hexary MPT with 64-nibble paths) and PR-2 (a node is at most 564 bytes).
  - It assumes a canonical split for every stored and displayed version, and zero-proved unused element slots.
  - Under these assumptions the response is (6+1)·65·1133 + 4096 = 519611 bytes, within 524288.

### 1.7 RF-E6-1 — the x12b total (`A-total-two`)

> x12b gains the derived cell 5001 (T2a deadline, T2b queued); the effective criterion is adminStaleDropped = 2 (op1 1, op2 1) with zero alloc, ticket and set for both stale requests. The source literal 1 stays preserved history.

- **Source.** validation.md:738 ("adminStaleDropped = 1"); m1-draft-0.17/annex/C27-RFE61.md option A.
- **Alternatives:**
  - add the 5001 cell and state the total 2 (chosen);
  - scope the fault to op1;
  - read 1 as op1's subtotal.
- **Rationale.** The full executed global `adminNoCancel` timeline gives one stale request from each of the two operations.
- **Gates.** c4 and c5 of E6.
- **Evidence.** `vectors/rfe61-x12b-binding-0.29.json` binds the total exactly to the root-executed 0.18 results (the 0.17 suite):
  - total 2, with op1 = 1 and op2 = 1 by both the FIFO log and the trace;
  - the 5001 request;
  - two stale drops at 7000;
  - zero allocation, ticket and set;
  - the traced engine equal to the untraced one.

  The executed vector must be byte-identical to the current 0.17 file, using the sha256 recorded in that run. Both historical "recorded" literal-conflict entries are rebound to this overlay. They are not deleted.

### 1.8 V1-SHA-COST — the SHA-256 cost sentence (`A-explicit-total3355`)

> Client cost: at most 14 header decodes, at most 13 ASERT with operands of at most 512 bits, and at most 3355 SHA-256 evaluations: at most 14 TemplateID + 13 PoW + 3328 share hashes, with each header's TemplateID computed once and cached. Every check of items 1-9 is retained. No device performance is claimed.

- **Superseded.** browser.md:58 "≤ 13 SHA-256". The literal is preserved as history and checked to be unchanged.
- **Alternatives:**
  - explicit category bound 3355 (chosen);
  - PoW-only 13 without an overall bound;
  - drop the share checks;
  - narrow the share domain;
  - keep 13 and block.
- **Rationale.** Direct interception confirms:
  - 3355 distinct inputs;
  - 6721 calls without caching;
  - every required check.

  Caching cannot reach 13.
- **Consequences.** TemplateID caching is **required** to stay within the stated operation budget. No check is omitted.
- **Gates.** c4 and c5 of V1.
- **Worst-case binding** (in the runner):
  - The independently verified 0.27 full-window fixture SHA-W is replayed from its reviewed transcript, with no rebuild. The transcript's sha256 and each reply's sha256 are recorded.
  - Expected counts: TemplateID 14, PoW 13 and share hashes 3328 (3355 in total), with 3355 unique preimages.
  - Every distinct preimage must equal an exported freeze entry whose id root verified independently:
    - 13 TemplateIDs, 13 PoW and 3328 shares in the 0.27 freeze;
    - the reference header's TemplateID H:7 in the 0.26 freeze.
  - The run without caching gives 6721 calls.
  - Root's 0.26 and 0.27 evidence is re-bound by summary and sha256:
    - 18 SHA probes;
    - 3677 hashes;
    - 31 signatures;
    - 1027 ASERT cases;
    - 347 hashes from 0.26;
    - the 258-hash probe.

### 1.9 P-C32-2 — rate retry delay (`A-rate-delay-cap`)

> A rate delay is retried only if it is an integer in [0, 2000] ms; busy keeps [250, 2000]; at most 3 retries; a delay out of range, or a malformed error, ends in a controlled outcome without sleeping; a valid delay must also end before the whole-load deadline. The source defines no rate range: this is an adopted client policy, not a source range, and no claim is made that every honest server stays inside it.

- **Source.**
  - network.md:234 (the busy range);
  - browser.md:41 (3 retries);
  - m1-draft-0.28/owner/P-C32-2-CHANGE-REQUEST.md.
- **Alternatives:**
  - client cap 0..2000 (chosen);
  - define a server formula;
  - uncapped delay under the whole-load deadline;
  - use the busy interval for rate;
  - no rate retry.
- **Consequences.** A 6000 ms total sleep is a sleep bound, not a load bound. The load bound is U14.
- **Gates.** c4 and c5 of V1. The same rule applies to the content-load retries of C3.

## 2. U14 and CR-M1-01 reconciled

| Rule | Effective value | Replaces |
|---|---|---|
| Deadline | One 10000 ms budget per covered operation, from the first request's send through the final verdict | 0.2:159 per-request preference |
| Tie | An event at elapsed ≥ 10000 is expiry. No reply, sleep end or verdict at or after 10000 counts | — |
| Request timeout | min(10000, remaining). Since remaining ≤ 10000, it always ends at the deadline | REV2:24 fixed 10 s per request |
| −32021 delay | busy [250, 2000] and rate [0, 2000], integer; at most 3 retries; the 4th valid −32021 gets no sleep | REV2:41 and :85 (0..10000) |
| Delay that does not fit | t + delay ≥ deadline: never slept (P-D29-1). HeaderNetCheck → viewIncomplete at once. Content state read → the attempt fails (REV2 §3.3) and may restart within the remaining budget | — |
| Cumulative wait | None. The whole-load budget bounds every wait | REV2:86 (≤ 20000) |
| Expiry outcome | HeaderNetCheck: viewIncomplete, no frame, 4901. Content: `unavailable`, zero frames (P-D29-2). Attempts exhausted stays `inconsistent` | — |
| Restarts and sends | ≤ 4 attempts, ≤ 48 sends, 19464192 bytes | unchanged |
| Limits | eth_getProof reply 524288; STATE_POOL 1 MiB; sites forbidden; 6-key proof | unchanged |
| Node support | Not claimed. E03 runs later | unchanged |

**Open reading (P-D29-3).** In RP, the HeaderNetCheck load runs first and the content load follows. The delegated text does not say whether the two operations have **separate** 10 s budgets or **share** one. Fixture RP-COMB gives both answers:
- separate budgets: the content load renders at 10500;
- one shared budget: the content load is `unavailable` at 10000.

Neither reading is adopted. Root decides.

**Virtual time versus browser enforcement.** Every outcome in `vectors/deadline-cases-0.29.json` is a **pure virtual-time reference outcome**.
- Latencies and computation times are hypothetical inputs.
- The HeaderNetCheck cases run the unchanged 0.27 checker on reviewed literal reply texts.
- Browser enforcement is the separate Phase A definition X-U14. Its real-clock times are recorded as measurements, not asserted, and they are not an M1 gate condition.

**Cases.** The fixtures cover these classes:
- positive;
- exact deadline (a reply or computation at 10000);
- just before (9999) and just after (10000/10001, or a send at 9999);
- retries that fit and retries that expire;
- a 4th −32021 with no sleep;
- a delay that does not fit;
- a delay that fits, followed by a resend that does not;
- a retry that succeeds at 9999;
- out-of-range busy or rate delays, a malformed reply and other errors;
- no reply;
- computation that crosses the deadline;
- h = 0;
- eight parallel getCode requests, plus the guard against a ninth;
- four failed attempts;
- verified absence;
- the RP pair.

The seven 0.28 timelines are re-run as well:
- Six equal root's independent WL_a column.
- U14-T6C used the unadopted rate policy C, so it now behaves like U14-T6A.

**Whole-deadline rerun of V1 acceptance.** Every reviewed 0.27 V1 transcript is replayed through the deadline:
- 20 carried 0.26 cases;
- 41 C32 cases;
- 3 SHA cases.

The replay uses three profiles:
- **Zero latency.** The verdict, the request list and the send times must equal the saved values.
- **700 ms per request with 100 ms of computation.** Every saved verdict must be kept.
- **1600 ms with 50 ms.** The long retry chains expire. The expected value is computed by independent arithmetic over the saved send times.

**Honest servers.** No statement about the rate delays of every honest server, and no source range for `rate`, is made.

## 3. Conventions pending root decision (exact)

None of these is assumed accepted. Each blocks c4 of the listed rows until root records a decision.

| ID | Convention | Rows |
|---|---|---|
| P-V1-1 | wire form | V1 |
| P-V1-2 | undecodable element → rule 1 | V1 |
| P-V1-4 | Arabic count phrase and warning join | V1 |
| P-V1-5 | signature byte layout r‖s‖v | V1 |
| P-V1-6 | fixture clock C = ts(head) + 5 | V1 |
| P-V1-7 | gasLimit/gasUsed u64, shares be64 | V1 |
| P-V1-8 | V1NET genesis (chainId 777002 placeholder network) | V1 |
| P-V1-9 | other RPC errors → viewIncomplete | V1 |
| P-V1-10 | the retry waits exactly retryAfterMs | V1 |
| P-D29-1 | a valid delay with t + delay ≥ deadline is never slept: HeaderNetCheck ends with viewIncomplete; a content state read fails the attempt and may restart | V1, C3 |
| P-D29-2 | content-load expiry → `unavailable`, zero frames; exhausted attempts stay `inconsistent` | C3 |
| P-D29-3 | RP: separate or shared budgets for HeaderNetCheck and the content load (both are fixtures) | V1, C3 |
| P-U08-1 | `ManifestSplit(i)`: the first position that differs from the canonical split; a missing element counts as a difference | C4 |
| P-U02-1 | consent to view a revoked version covers one load; a reload shows the interstitial again | C3 |
| P-U02-2 | the interstitial is the verdict of the first covered load; user think time is outside every budget; the click starts a new covered load | C3 |
| P-U10-1 | the warning appears at nomination and after acceptance; it names the previous owner, its retained calls and the removal call; it stays until removal; a cancelled nomination clears it | C3, C4 |

**Recorded status of related items.**
- P-V1-3 is withdrawn.
- P-C32-1, P-C32-3 and P-C32-4 were accepted in REVIEW-0.27, line 15.
- P-C32-2 is the delegated decision in §1.9.

## 4. Expected gate effect (computed by the runner, not asserted)

| Row | c1 | c2 | c3 | c4 | c5 | Expected status |
|---|---|---|---|---|---|---|
| C1, C2 | pending root review | pending root review | pending root review | satisfied (delegated U01) | pending root review | PendingRootReview |
| R1 | pending | pending | pending | satisfied (delegated CR-M1-01) | pending | PendingRootReview |
| E6 | pending | pending | pending | satisfied (delegated RF-E6-1) | pending | PendingRootReview |
| C3 | pending | pending | pending | **pending reviewer**: P-D29-1, -2, -3, P-U02-1, -2, P-U10-1 | pending | Partial |
| C4 | pending | pending | pending | **pending reviewer**: P-U08-1, P-U10-1 | pending | Partial |
| V1 | pending | pending | pending | **pending reviewer**: P-V1-1, -2, -4…-10, P-D29-1, P-D29-3 | pending | Partial |
| The other 34 rows | satisfied | satisfied | satisfied | satisfied | satisfied | CompleteCandidate (unchanged) |

**Findings.**
- **Closable at specification scope** (18): F01–F03, F05, F08, F10, F12–F16, F18–F22, F24 and F26.
  - F26 is the gate text. The gate itself stays unmet until every row is Complete.
- **Closable after root review of the overlay:** F06, F09 and F23.
- **Waiting for a root convention:** F04, F07, F11, F17 and F25.

**Experiment definitions.**
- E01, E03 and E07 are amended.
- X-U14 is new.
- E02, E04, E05 and E06 are unchanged.
- No definition is missing. The Phase A outcomes are not gate conditions.

## 5. Not claimed

- No deployment, and no confirmed factory deployment address.
- No measured `eth_getProof` support.
- No browser timing.
- No device SHA-256 performance.
- No universal honest-server delay range.
- No owner answer.
- No Complete row.
- No production code.
