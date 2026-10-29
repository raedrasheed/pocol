# Annex C32-POLICY: corrections to the 0.27 retry-policy claims (normative text, draft 0.28)

**For root review. Not approved. The author ran nothing.**
- This file supersedes the sentences listed in section 1.
- It changes no reference behaviour: `m1-draft-0.27/tools/v1_ref_027.py` and the 0.27 vectors are reused byte-identically.
- It adopts no owner policy and changes no source text.

## 0. Status after REVIEW-0.27

| Item | Status |
|---|---|
| C32 malformed-reply crashes | Closed at reference scope (root: 26/26 independent malformed replies, plus the 41 prepared cases) |
| P-C32-1 envelope | Accepted qualified technical convention |
| P-C32-3 C19 integral values | Accepted qualified technical convention |
| P-C32-4 strict JSON constants | Accepted qualified technical convention |
| **P-C32-2 rate-delay cap [0, 2000]** | **Unapproved owner client policy.** Routed in `../owner/P-C32-2-CHANGE-REQUEST.md` |
| busy-delay range [250, 2000] | Source fact: network.md:234 |
| V1-SHA-COST | Owner change request pending (`m1-draft-0.27/owner/V1-SHA-COST-CHANGE-REQUEST.md`, recommendation A ≤ 3355 by category). The literal «≤ 13 SHA-256» (browser.md:58) is untouched and unresolved |
| P-V1-3 | Withdrawn |
| U14 | Unanswered. Neither branch is selected (section 3) |

## 1. Superseded 0.27 sentences

| 0.27 location | 0.27 text | Correction |
|---|---|---|
| `annex/C32-RPC-ERROR-ENVELOPE.md` | «the cap reuses the only approved −32021 range» | The rate interval is a **proposed client policy** (P-C32-2). network.md gives the busy range only. Reusing it for rate is a policy choice, not a source fact |
| `annex/C32-RPC-ERROR-ENVELOPE.md` | «An RpcGuard-conforming server (network.md:217, 234) only sends delays inside these ranges, so its behaviour is unchanged» | **Withdrawn for rate.** network.md:215–218 gives a 20 tokens/s bucket of capacity 20 and the reply shape `{rate, retryAfterMs}`, but **no formula and no range** for the rate delay. Compatibility holds only under the conditional assumptions of section 2. A remote RP may also not apply D88–D94 at all (network.md:373). For busy, the statement stays true by network.md:234 |
| `annex/C32-RPC-ERROR-ENVELOPE.md` | «It is at most 3 · 2000 = 6000 ms, inside the 10 s RP protection of network.md:374» | 6000 ms bounds **retry sleep only**, and only **under the proposed P-C32-2 cap** (with busy bounded by the source). It does not bound response latency, worker computation or the total load time. It proves nothing about the 10 s protection of network.md:374. What that 10 s covers depends on U14 (section 3) |
| `vectors/c32-error-cases.json` (case C32-exhausted-max) | «inside the 10 s RP protection of network.md:374» | As above: sleep-only arithmetic, not a deadline proof |
| `tools/run_checks_027.py` (check C32.maxTotalWait) | «inside the 10 s RP protection» | As above. The check's arithmetic (3 × 2000) remains a correct sleep sum |
| `M1-SPEC-0.27-AMENDMENT.md` | «total wait ≤ 6000 ms» | Read as: total **retry sleep** ≤ 6000 ms under the proposed rate cap |

## 2. Rate delay: what the source gives, and conditional derivations

The source facts (network.md:215–218, 353):
- a per-connection bucket of 20 tokens/s with capacity 20;
- a single request consumes 1 token; a batch of k consumes k tokens, with batches ≤ 20;
- a shortfall gives −32021 `{rate, retryAfterMs}` without execution, and there is no waiting for tokens inside a request.

The delay value is not defined. Each derivation below is **conditional** on an assumed server formula:

| Assumption | Formula for retryAfterMs | Bound for a single request (k = 1) | Bound for a batch (k ≤ 20) | Inside the proposed [0, 2000]? |
|---|---|---|---|---|
| F1 | time until enough tokens for this request | ≤ ceil(1000/20) = 50 ms | ≤ ceil(k · 1000/20) ≤ 1000 ms | yes |
| F2 | time until the bucket is full | ≤ 1000 ms | ≤ 1000 ms | yes |
| F3 | F1 or F2 plus implementation jitter or backoff | not bounded by the source | not bounded | unknown |
| F4 | a remote RP that does not implement RpcGuard (network.md:373) | not bounded by the source | not bounded | unknown |

HeaderNetCheck sends single requests (P-V1-1), so under F1 a conforming server sends ≤ 50 ms. Nothing in the source excludes F3 or F4, so **no statement that "every honest server satisfies the cap" is made**.

## 3. U14 and the 10 s protection: both branches, conditional

**The sources.**
- FD:L905 lists «مهلة 10s» among the content-retrieval client limits and «ردود −32021 تُعاد بعد retryAfterMs».
- network.md:374 says the extension protects itself in RP with, among other things, «مهلة 10s».
- U14 (open owner item) asks whether such a 10 s timeout is **per request** or **whole load**.
- Whether the HeaderNetCheck requests fall under the same 10 s rule is not stated. This annex models both readings and adopts neither.

**Branch PR (per request).** Every HTTP request has its own 10 s deadline from its send time. Retry sleep and worker computation are not counted.

**Branch WL (whole load).** The check as a whole has one 10 s deadline from the first request. Retry sleep counts.
- **WL-a:** worker computation up to the verdict counts.
- **WL-b:** the deadline ends at the last reply; computation after it does not count.

The cases below state WL-a. WL-b differs only in U14-T5.

**The cases.** They are literal timelines in `../vectors/u14-timing-cases.json`. Each request latency and computation time is a hypothetical input, not a measurement; no browser result is claimed. Each expected outcome is conditional on the branch named:

| Case | Inputs (ms) | Sleep | Elapsed | PR | WL-a |
|---|---|---|---|---|---|
| U14-T1 | all latencies 100; 3 busy replies at 2000, then success; compute 50 | 6000 | 6550 | ok | ok |
| U14-T2 | all latencies 1600; 3 busy replies at 2000, then success; compute 50 | 6000 | 14050 | ok | deadline at 10000, while request 4 (third getHeaders, sent at 8800) is in flight → viewIncomplete |
| U14-T3 | eth_blockNumber 4000; getHeaders 7000 (no retry); compute 50 | 0 | 11050 | ok | deadline at 10000, while getHeaders is in flight → viewIncomplete |
| U14-T4 | eth_blockNumber 100; getHeaders 10001 | 0 | — | that request's deadline fires at 100 + 10000 = 10100 → viewIncomplete | deadline at 10000 → viewIncomplete (100 ms earlier) |
| U14-T5 | latencies 100; compute 9850 | 0 | 10050 | ok | WL-a: deadline at 10000 during computation → viewIncomplete; WL-b: ok (last reply at 200) |
| U14-T6 | latencies 100; 3 rate replies at 5000, then success; compute 50 | — | — | P-C32-2 alternative A: viewIncomplete at the first rate reply (t = 200, no sleep). Alternative C (no cap): sleep 15000, elapsed 15550, ok | alternative A: viewIncomplete at t = 200. Alternative C: deadline at 10000 during the second sleep → viewIncomplete |

**What the cases show.**
- Retry-sleep arithmetic alone cannot establish any 10 s property.
- Under PR, a check can take far longer than 10 s (T2, T3, T6-C).
- Under WL, the verdict changes with latency and computation that ScriptedRpc does not model.

The P-C32-2 choice matters most under PR, where the cap is the only bound on attacker-chosen sleep (T6).
