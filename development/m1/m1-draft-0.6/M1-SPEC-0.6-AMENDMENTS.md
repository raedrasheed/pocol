# M1 — Specification Draft 0.6: Amendments

Status: **review draft, NOT approved. Phase S (specifications and reference tooling) only.**
- No MV3 extension, no production module, no installs, no deployments, no transactions, no subagents, no memory notes.
- **Nothing in 0.6 was executed by the author.**

Inheritance: 0.6 = 0.2 + the amendments of 0.3, 0.4 and 0.5 + the amendments below. Unchanged material is inherited by reference.

| ID | Issue / row | Subject |
|---|---|---|
| R6-01 | C14 | Corrected 24-byte head of the 2848-reference manifest (the contentHash was missing) |
| R6-02 | C19 | Integer value semantics for the bridge id, the JSON-RPC response id, `error.code` and `retryAfterMs` |
| R6-03 | X2 | signEligible (7 conditions), ProfileMutex serialization, eligibilityEpoch: executable model |
| R6-04 | C13 / X3 | Original Q8–Q12 (with Q9 a/b/c), X1–X7 and د1–د5 restored in English with traceability; ADD-M1-01 withdrawn |
| R6-05 | B10 | BR19a/BR19b literal tables, the BR19c script and decision rule, BridgeRef |
| R6-06 | B11 | BR16 generator (seed b₀, Python and JS implementations), SchemaRef, BridgeRef, SiteStorageRef, BridgeTrace format |
| R6-07 | status | Decisions reclassified per review; CONF_DEPTH; gate; evidence record |

## R6-01 — C14 head

After the size field `0x821640` comes the contentHash, written as `a0` followed by 32 bytes. The 0.5 expectation skipped it.

- Corrected head: `0xfa0100150180fa01000ffa01000b822f6101821640a02fdc`.
- contentHash: `0x2fdc37b7…b88b`. Both come from Codex's independent `c14-independent-head.json`.
- The chunks-list header `f9ffe0` starts at offset 54.
- The length (65561) and the level table were already correct in the 0.5 run.

`vectors/c14-head.json`; the runner checks the head, the content hash and the offset.

## R6-02 — C19 integer ids

FD:L1110 requires an integer in [0, 2³²−1] and gives no lexical grammar. **The adopted reading (B) is value semantics.** The id is any JSON number whose IEEE-754 double value (as `JSON.parse` yields it) is finite, integral and in range. The returned reply id is that value, with −0 normalized to 0.

- **Accepted, with the returned id:** `1`, `1.0`, `1e0`, `1E0` and `10e-1` return 1. `-0`, `-0.0`, `0` and `1e-400` (underflow) return 0. `4294967295`, `4294967295.0` and `4.294967295e9` return 4294967295.
- **Rejected:** `4294967296`; `4.294967296e9`; `4294967295.99999999999`, which rounds to 2³²; `-1`; `1.5`; `1e400` (Infinity); `true`; `null`; `"1"`; `[1]`.
- The 0.4/0.5 lexical rule is withdrawn; it was not baseline text. No narrower lexical rule is proposed.
- **The same semantics apply to:**
  - the `eth_getProof` response `id`, compared by value with the request id: `5.0` and `5e0` match 5, `-0` matches 0, `5.5` and `true` fail;
  - `error.code`: `-32021.0` is accepted, `-32021.5` gives `errorShape`;
  - `retryAfterMs`: `500.0` is accepted, `500.5` fails.
- **Superseded expectations:** 0.4 `B0-id-1.0` and `B0-id-minus-zero` (now accepted, ids 1 and 0) and 0.5 `ENV-id-float` (now accepted as a result).

Consistency: `tools/id_oracle_06.cjs` evaluates every token with native `JSON.parse` and `Number.isInteger`. The runner requires the Python and Node verdicts and returned ids to agree, and all 0.4 bridge cases are re-run under the new rule.

## R6-03 / R6-04 — X2 and restored X3

See `annex/X3-RESTORED.md` (English, with traceability) and `annex/x3-restored.json`. Summary:
- The 17 required IDs are restored from the recovered round-15 source (message 31), with Q9 split into a/b/c. They concern:
  - **Q:** signing and profile races under ProfileMutex;
  - **X:** malicious-site isolation;
  - **د:** the DNR rule lifecycle.

  ADD-M1-01's mappings (T12 site ordinals, isolation substitutes, unsupported features) are **withdrawn**.
- **Executable models (decision logic only):**
  - `tools/profile_race_ref.py` covers Q8, Q9a, Q9b, both Q9c variants, Q10, both Q11 variants, Q12, and 7 supporting signEligible scenarios;
  - `tools/dnr_ref.py` covers د1, د2 (two variants), د3, د4 (resource-type matrix) and د5.
- **X1–X7** are given as literal site files with expected outcomes for the browser run (Phase A).
- **Reconciliation:** I1–I8. There is no outright contradiction. The gaps (out-of-worker write detection; DNR resource types, scoping and removal) are filled with labelled proposals taken from the historical D58/D37 text.

## R6-05 — B10: BR19 and BridgeRef

`vectors/br19-tables.json`:
- **BR19a (a1)–(a7):** literal arrival times, message literals, results and `tokens_mt` after each step. The runner replays all 265 messages through the integer bucket.
  - Proposed details: the invalid-JSON and oversize literals, and the definition of `tokensBefore_mt` as the stored value before the refill, so that a size-rejected line and the next line share it.
  - Negative control: `noRefill` fails (a3).
- **BR19b (b1)–(b5):** the RpcReadClient/LogClient pending model (`tools/bridgeref_06.ReadClient`) with FakeTransport and FakeClock. It covers the 4-pending limit, release, the 10 s timeout, one pending for `fetchAll` however many internal requests it makes, and orphans with no replies.
- **BR19c:** the script, the G matching and the decision rule are transcribed. A synthetic reference trace exercises BridgeRef: 60 arrivals at 1 ms spacing give 52 passes and 8 rate rejections; of the 5 `eth_call`, 4 are forwarded and the 5th gets pending; the result is decisive.
- **BridgeRef** (`bridge_ref()` in `tools/bridgeref_06.py`):
  - Inputs: the messages with their recorded arrivalMs, and the recorded reply times.
  - Output: the expected BridgeTrace line per message: Bpre, B0–B3 via SchemaRef, B4 pending, and SiteStorageRef totals.
  - Replies due at time t are processed before arrivals at t (P).

## R6-06 — B11: BR16, Refs and BridgeTrace

- **BR16 generator** (`annex/br16-generator.json`). The seed is b₀ = SHA256('PoColSeed'‖be32(0)), from FD:L2522. The algorithm is P and fully specified:
  - a SHA-256 counter-mode PRNG and a self-contained serializer;
  - name lists, mutations, valid and invalid kinds, random params;
  - 12 structural templates (invalid JSON, payload array, extra fields, missing params, duplicate keys, id variants, depth 9, 65537 bytes);
  - every 50th message is a near-quota storage message (key 1/255/256/257 bytes, value 0/1/61439/61440/61441 bytes).

  It is implemented twice: `tools/br16_gen.py` and `tools/br16_gen.cjs`. The runner requires equal stream hashes for the 10⁴ messages. The JS generator also evaluates httpsUrl natively for every `site_openExternal` value it produces, so the Python harness never meets a value missing from the oracle.
- **SchemaRef** = `m1-draft-0.4/annex/bridge-matrix.json` with `bridge_ref.py` B0–B3, under R6-02 semantics and the 0.6 URL oracle.
- **SiteStorageRef** = `m1-draft-0.5/tools/sitestorage_ref.py`.
- **BridgeTrace format** (B, FD:L1221): fields `frame, seq, id, arrivalMs, tokensBefore_mt, bpre, stage, code, route, pendingBefore, storeTotalBefore, storeTotalAfter, forwardedSeq, replyMs`, in this order. P: `stage` is null after a Bpre rejection, and B4 for routed rejections. `validate_trace_line` checks the format on every line of the BR19c and BR16 traces.
- **BR16 run:** all 10⁴ messages are decided by BridgeRef. The runner checks:
  - the trace format;
  - no forwarded request for a rejected message;
  - storage totals never above 1048576.

  The stage/code histogram and the hash of the decisions are recorded, not asserted.

## R6-07 — Decisions and gate

- **Reviewer decisions, not owner decisions** (per REVIEW-0.5):
  - the anvil test-account derivation (TK-1) and the txA gas/fee tuple: independently verified by Codex, with no transaction submitted;
  - batch order and interpreter choice;
  - U31–U42.
- **CONF_DEPTH** stays an explicit, required runtime input of the publisher, with no protocol default. Its value is not selected in this specification.
- **The full gate stays in force.** No carve-out is requested.
- **Real decisions that remain:**
  - CR-M1-01: the proof format premise, the new `STATE_POOL`, and the retry caps of 10 s per wait and 20 s per load;
  - U01 (factory address), U02 (revoked interstitial), U10 (publisher rights after an ownership transfer) and U14 (meaning of the 10 s timeout).
- **C08 is closed by provenance:** `e8a19ecb` = SHA256(JSON.stringify({sections, decisions, risks})), reproduced by Codex.
- **Executed evidence (Codex, not the author):**
  - 0.5: URL oracle with 33 values; Python 347 passed, 3 recorded, 1 failed (C14 prefix, fixed here);
  - txA cross-checks: Node passed; Rust libsecp256k1 0.5.0 verified the signature and recovery; pycryptodome verified the hashes and the tuple;
  - the hash triad agrees on 611 samples.
