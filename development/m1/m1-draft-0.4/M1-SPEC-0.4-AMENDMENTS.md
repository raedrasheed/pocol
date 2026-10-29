# M1 — Specification Draft 0.4: Amendments to Drafts 0.2 and 0.3

Status: **review draft, NOT approved. Specifications only.** No production contract, node or extension code exists. Nothing was installed or deployed, and no transaction was sent.
Baseline: `../reference/FINAL_DESIGN.md`, unchanged and authoritative. Its approved design ID `e8a19ecb…a851aa0` is approval provenance (R4-08).

**Inheritance.** Draft 0.4 = draft 0.2 (`../m1-draft-0.2/M1-SPEC-0.2.md`), plus the 0.3 amendments (`../m1-draft-0.3/M1-SPEC-0.3-AMENDMENTS.md`), plus the amendments below. Unchanged text, tools and fixtures are inherited **by reference**; they are not copied. The 0.3 tools `rlp_strict.py`, `m1model.py` and `m1paths.py` are used unchanged by `tools/run_checks_04.py`.

| ID | Issue | Subject | Replaces or extends |
|---|---|---|---|
| R4-01 | C10 | State-invariant checks and precedence; undefined status never renders | 0.3 R3-04 model; 0.2 §4.3 |
| R4-02 | C11 | Three-phase gate: spec acceptance, drafting candidate code, accepting candidate code | 0.3 R3-08 table |
| R4-03 | C12 | Viewer request bound of 2850 derived from the RLP size; non-canonical splits stay valid | 0.3 R3-04 budget figure "426" |
| R4-04 | C13 | X3 count corrected to 17; unsigned addendum proposal ADD-M1-01 | 0.3 BLK-02 |
| R4-05 | C06 | CR-M1-01 as a concrete addendum: `CR-M1-01-STATE-PROOF-READS.md` | 0.3 BLK-01 |
| R4-06 | C01 | Annex batch 2: B1, B2, B3, B5, B7 | new |
| R4-07 | C01 | Annex row R1 brought forward (partial) | new |
| R4-08 | C08, evidence | Design-ID wording; record of executed evidence | 0.3 R3-09, R3-10 |

---

## R4-01 — State invariants (C10)

**Defect.** The 0.3 model rendered an explicitly selected version whose status was 3 (Codex probe `C10-undefined-status-explicit`). The version-selection rules (0.2 §4.3) authorize only two kinds of display: published content, and revoked content after an explicit confirmation (U02).

**P38 (new).** After a proof verifies against the anchored root, the client validates the decoded Website words in this order. The first violation ends the load with `stateInvariant`. A `stateInvariant` outcome means zero frames, no chunk fetch and **no restart**: the state was read consistently, but no conforming contract can produce it.

| Step | When | Rule ID | Violation |
|---|---|---|---|
| S1 | After proof 1 | `state.slot2Bits` | Any of bits 64–255 of slot 2 is set |
| S2 | After proof 1 | `state.countRange` | `versionCount > 1024` |
| S3 | After proof 1 | `state.currentRange` | `currentVersion > versionCount` |
| — | Selection | — | Default request with `currentVersion = 0`: `noSite`. `n > versionCount`: `versionNotFound`, and proof 2 is not sent |
| V1 | After proof 2 | `state.recordBits` | Any of bits 104–255 of base+1 is set |
| V2 | After proof 2 | `state.recordAbsent` | `manifestLen = 0`, `manifestHash = 0` and chunk count 0 for an id ≤ `versionCount`. This is checked before status, so an all-zero record is never read as a draft |
| V3 | After proof 2 | `state.statusDomain` | Status not in {0, 1, 2} |
| V4 | After proof 2 | `state.publishedBlock` | A draft with publishedBlock ≠ 0, or a published or revoked version with publishedBlock outside 1..N |
| — | Outcome | `state.currentNotPublished` | Default request whose current version is not published |

Outcomes for an explicit `@v<n>`: status 0 gives `refuseDraft`; status 2 gives `revokedInterstitial` (zero frames until confirmed, U02); status 1 renders, with a banner if n is not current. **Only status 1 can produce a frame directly.** The runner checks this as a property over statuses 0–7, for both request kinds.

The record-content rules of 0.2 §3.2 (`manifest.length`, `manifest.split`, `mfetch.*`) are unchanged. They run during manifest retrieval, after these state checks.

Fixtures: `vectors/snapshot-invariants.json` has 15 scenarios, SNAP-20 to SNAP-34:
- explicit and default status 3, including a non-current one;
- current above count, also when the explicitly requested id is itself valid;
- count above the limit, which takes precedence over current above count;
- slot 2 high bits;
- an absent current record, and an absent explicitly requested record (`recordAbsent`, not `refuseDraft`);
- publishedBlock 0, a non-zero publishedBlock on a draft, and a publishedBlock after the anchor;
- record high bits, which take precedence over the status domain;
- the revoked interstitial;
- an invariant found after a restart.

All 13 P20 scenarios of 0.3 are re-run under the corrected model `tools/snapshot_model_04.py`, after an explicit state transform. The transform adds the fields that a conforming record always has; the expected results are unchanged. The 0.3 detector scenario SNAP-03D is not affected.

## R4-02 — Gate phases (C11; replaces the R3-08 table)

The 0.3 table required EVM, compiler and gas evidence "before implementing the unit". That evidence needs candidate code, so the gate was circular. It is replaced by three phases. **This task, and every author turn so far, is in Phase S only.**

| Phase | Allowed work | Entry conditions | Exit / acceptance evidence |
|---|---|---|---|
| **S: specification** (current) | Specifications, literal fixtures, reference-only Python checks, reviews | — | M1-spec acceptance: every inventory row Complete under the R3-08 conditions 1–5 (full gate, no carve-out); E05 K1–K3 agreement for every frozen fixture hash; U05 decided (CR-M1-01 approved or P36 chosen); ADD-M1-01 signed, or the original definitions supplied |
| **D: drafting candidate code** (a later, separately authorized task) | Candidate contracts, extension modules and publisher, as review candidates, not production | M1-spec accepted, or an owner-recorded U15 decision; every U item affecting the unit decided; M0 prerequisites: tool versions and licences pinned, anvil available, K1–K3 triad | — |
| **A: accepting candidate code as the production implementation** | Running experiments against the candidate | The candidate code exists (Phase D) | ChunkFactory and Website: E01 (T1 suite incl. T1-16), E02 (gas), E04 (ABI, `storageLayout`, build record P27). Viewer and extension: E03 (pocold proofs, MPT-1…12, if CR-M1-01), E06 (browser), E07 (viewer table), T12 including ADD-M1-01, BR, MR. Codex review of the code. Failures may reopen the spec |

Experiments are not entry conditions for Phase D. They are acceptance conditions of Phase A. Completeness of the annexes stays a Phase S condition and is not weakened.

## R4-03 — Viewer request bound (C12; corrects 0.3 R3-04)

**Correction.** The 0.3 figure of "up to 426 content-chunk requests" applies only to manifests that use the canonical publisher split (P04). It is **not** a viewer bound. B07 accepts any chunk lengths in 1..24575 that sum to the file size, and this draft does not restrict valid non-canonical splits. Codex built a valid 23131-byte manifest that causes 1001 actual requests for 2013 logical bytes; it is reproduced as fixture `rb-codex-1001`.

**P39 (new, derived).** Any manifest of at most 65536 bytes that passes the decode, structure and semantic stages has **at most 2847 content chunk references**:
- Each reference costs at least 23 bytes: a 21-byte address string and a 1-byte length, plus a 1-byte list header.
- A manifest with 2847 or more references has at least 54 bytes of fixed overhead.
- floor((65536 − 54)/23) = 2847, and 54 + 23·2848 = 65558 > 65536.

So a load makes **at most 3 + 2847 = 2850 `eth_getCode` requests**, and fewer with cache hits.

Fixtures (`vectors/request-bound.json`):
- `rb-max-2847` attains the bound: a valid 65535-byte manifest with literal head bytes, giving 2847 content requests and 2850 requests with the manifest chunks;
- `rb-2848-does-not-fit`: 65558 bytes, `decode.oversize`;
- `rb-codex-1001`;
- two non-canonical splits of the 111-byte HTML file, both valid: [100, 11], and 111 one-byte chunks with requests equal to the number of distinct bytes.

The CR-M1-01 budget (§6) uses 2850.

## R4-04 — X3 definitions (C13)

**Correction.** The 0.3 BLK-02 count of 22 was wrong. FD:L4959 requires **17** items: Q8–Q12 (5), X1–X7 (7) and د1–د5 (5). Q8–Q12 and X1–X7 appear in the baseline only as references. BR23 (d1)–(d5) (FD:L3066ff) are possible namesakes of د1–د5. A mapping between them is **unestablished**, not proven absent.

`annex/addendum-x3-proposal.json` (**ADD-M1-01, UNSIGNED PROPOSAL**) does **not** supply the original definitions. It proposes substitutes for owner decision:
- **Mapping decisions:**
  - Q_i is the acceptance check of T12 site i, by the order of FD:L2644: Q8 woff2, Q9 images, Q10 form, Q11 SPA, Q12 contract read/write;
  - X1–X7 are isolation attempts in the X8 namespace;
  - د1–د5 are the unsupported features of FD:L1345 other than WebRTC;
  - the BR23 mapping is rejected, with three stated reasons.
- **For each of the 17 items:** the baseline-derived purpose (with FD lines), literal inputs (HTML, CSS and JS text, and a generated PNG), and a pass criterion.
- **Assets still pending:** a WOFF2 font, a JPEG and a WebP. Their generation tools and licences are M0 items.

BLK-02 stays open until the owner signs ADD-M1-01 (CR-M1-02) or supplies the originals. T12 also references Q1–Q7, which are undefined; FD:L4959 does not require them, so this is recorded as an observation.

## R4-05 — CR-M1-01 (C06)

See `CR-M1-01-STATE-PROOF-READS.md`. It specifies:
- request bodies and the exact response-acceptance order;
- a **conditional** byte bound of 524288, resting on two stated premises that the baseline does not give; FD:L935 is shown to be unusable as a proof-size source;
- the new `STATE_POOL` (1 MiB) and the memory delta;
- the unchanged importer set and site ban;
- the restart and request budget (≤ 4 anchors + 8 proofs, then ≤ 2850 chunk reads);
- authenticity assumptions;
- future MPT tests MPT-1…12;
- the proposed baseline text changes.

The detector design stays a documented counterexample (SNAP-03D). The C06 snapshot guarantee is not frozen until the owner decides.

## R4-06 — Annex batch 2: bridge envelope, bucket, matrix, deny list, dependency rules (C01)

The machine-readable sources are `annex/bridge-matrix.json` and `annex/dependency-rules.json`. The executable reading is `tools/bridge_ref.py`. The fixtures are `vectors/bridge-check-cases.json` and `vectors/dependency-graphs.json`. Labels: B means transcribed from the baseline, P means a proposal where the baseline leaves a choice.

**B1 — Envelope and check order** (FD:L1108–1144):
- **Envelope.** The message is JSON text `{id, kind, payload}` with exactly these fields.
  - `id` is an integer in [0, 2³²−1].
  - `kind` is one of `rpc_read`, `wallet_req`, `storage_set`, `nav`.
  - `payload` is `{method, params}` with exactly these fields, and `params` is always an array.
  - The reply is `{id, result}` or `{id, error: {code, message, data?}}`. There is no partial success.
- **Processing.** `arrivalMs = floor(performance.now())`, stamped in the worker before any check. Messages are processed one at a time, in arrival order.
- **Order: Bpre, B0, B1, B2, B3, B4.** Processing stops at the first violation. The order is the same for every kind.

| Stage | Violation | Error | Returned id | Token |
|---|---|---|---|---|
| Bpre size | UTF-8 length > 65536 (P: unit is UTF-8 bytes, U35) | −32600 `{reason:'size'}` | null | Not touched |
| Bpre rate | `tokens_mt` < 1000 after refill | −32005 `{reason:'rate'}` | null | Refilled, not deducted |
| B0 | Invalid JSON; NaN or Infinity; duplicate key at any depth; depth > 8 (P: the top-level container counts as depth 1); outer fields ≠ {id, kind, payload}; id is not an integer token `0` or `[1-9][0-9]*` of at most 2³²−1 (P: `1.0`, `-0` and `true` are invalid); kind not a string; payload ≠ {method, params}; method not a string; params not an array | −32600, no `data` (P) | The id, if the parsed top-level object has a valid id; otherwise null (P: a parse failure always gives null) | Consumed |
| B1 | kind not in the set | −32600 `{reason:'kind'}` | id | Consumed |
| B2 | (kind, method) not in the matrix (byte-exact lookup in a frozen Map; prototype names never match) | 4200 `{method}`, method cut to its first 64 UTF-16 units (P: this may split a surrogate pair, as JavaScript `slice` does) | id | Consumed |
| B3 | params do not match the method schema | −32602 `{path}` | id | Consumed |
| B4 | Routing, then the target's state constraints (FD:L1138–1144): pending → −32005 `{reason:'pending'}`; reserve > 2 s → `{busy}`; `{store}`; quota → 4300; nav confirmation → `{pending}` | as given | id | Consumed |

Error `message` text is not normative. The baseline criterion compares the code, the data and the returned id (FD:L2662).

**B2 — Integer bucket** (FD:L1129–1133). These are formulas, not rates.
- Constants: `BUCKET_CAP_MT = 50000`, `REFILL = 50` mt per ms, `MSG_COST_MT = 1000`.
- State `{tokens_mt, last_ms}` starts at {50000, session creation}. Navigation does not refill it.
- For each message that passes the size check:
  1. `tokens_mt = min(50000, tokens_mt + 50·(arrivalMs − last_ms))`;
  2. `last_ms = arrivalMs`;
  3. if `tokens_mt ≥ 1000`, deduct 1000 and pass; otherwise reject with `rate` and deduct nothing.
- Size-rejected messages leave the state untouched.
- Consequence: at most 50 + 50·t messages in t seconds (FD:L1315).

**pendingReads** (FD:L1146–1155): per session, starting at 0.
- **+1** when RpcReadClient or LogClient accepts a message, before the first connection. P: acceptance means passing the pending check; a later `busy` is a final reply and gives −1.
- **−1** at the final reply to the frame: success, remote error, transport error, or timeout (10 s, 30 s for `fetchAll`).
- Frame teardown does not reset the counter. Requests of the torn-down frame become orphans: they are aborted without a reply and give −1 only when they settle.
- LogClient's internal retries do not count.
- Limits: 4 per session, 16 overall.
- The reset happens only at the end of the session.

**B3 — Matrix and schemas** (FD:L1157–1198): `annex/bridge-matrix.json`, complete for all four kinds:
- 18 `rpc_read` methods, 4 `wallet_req`, 2 `storage_set` and 2 `nav`;
- the primitive types with their regexes: addr, hash32, slot32, qty, tag, data, str(n), pathStr, httpsUrl;
- the composites: callObj, txObj, filter, topics, percentiles;
- the path rule and the field-evaluation order.

Proposals and reconciliations:
- **U32:** BR8 (FD:L2685) gives `params[2]` for an extra element, while BR17d (FD:L2710) gives `params` for an extra element. Resolved by a per-method `thirdParamPath` for `eth_call` and `eth_estimateGas` only.
- **U31 (observation):** `value` is typed qty, which FD:L1160 limits to u64.
- **httpsUrl:** the reference model approximates the WHATWG URL parser for the stated properties only. Browser confirmation is E06.

**B5 — Deny list** (FD:L1202–1205; BR1–BR4, BR7, BR11): `denyList` in the matrix file.
- 39 names that are forbidden for every kind, with wildcard families expanded to concrete members (P);
- 17 kind-crossing pairs;
- 9 malformed or prototype names.
- The runner checks every (kind, name) pair for 4200 at B2.

**B7 — Dependency rules** (FD:L1218, FD:L3262–3264, FD:L4934, FD:L4943): rule text is in `annex/dependency-rules.json`:
- `bridge-no-walletsubmit-no-transport`;
- `bridge-no-malicioushttp`;
- `httptransport-importers-closed` (exactly the six importers, plus tests);
- `src-no-test-imports`.

The rules apply to direct edges only, because Bridge → RpcReadClient → HttpTransport is the intended design. **U33:** FD:L3264 asks for "no module calls fetch except HttpTransport" as a dependency-cruiser rule. `fetch` is a global, not an import, so dependency-cruiser cannot express it. Proposed: an ESLint `no-restricted-globals` / `no-restricted-properties` rule, with its text given. The residual (computed property access such as `globalThis['fe'+'tch']`) is recorded. Module paths are a P convention.

**Literal cases** (`vectors/bridge-check-cases.json`):
- 18 positive messages and the BR1–BR13 rejections;
- BR17d and BR18c/d/g schema cases;
- edge cases: id `1.0`, `-0`, `true`; NaN; depth 8 vs 9; 65536 vs 65537 bytes; 256/257-byte keys; 61440/61441-byte values; 2048/2049-character paths; a split surrogate pair in 4200 data;
- 5 bucket sequences and 5 pendingReads sequences.

**Observation (U34):** BG1 (FD:L2608) makes `BRIDGE_MSG_MAX = 61823` fail the build, which implies a derived minimum of 61824. The largest compact `storage_set` message without escaping (id 4294967295, a 256-byte key, a 61440-byte value) counts 61790 bytes by this draft's count. The 34-byte difference is unexplained. Also, values with characters that JSON must escape can exceed 64 KiB even though B3 would accept them. Neither changes the literal limit of 65536.

## R4-07 — Annex row R1, brought forward (partial)

`annex/recv-limits.json`:
- the whole table of FD:L1262–1273, with WorstLegit formulas recomputed from the network CP values (FD:L2246–2247): 66560, 87124, 98304, 163958, 168015, 264192, 347136;
- the closed capped list;
- the D100 build vectors (FD:L2610–2616);
- RF3 and RF4;
- the conditional `eth_getProof` row of CR-M1-01.

Remaining work: RecvFit for the other profiles needs their CP files, and BG1 is batch 3/4.

## R4-08 — Design ID and evidence record

- **Design ID.** The design ID `e8a19ecb…` embedded at FD:L3 and recorded in `reference/baseline.json` is the **approval provenance identifier**. No equality with the raw-file bytes is claimed or needed. The 0.3 probe script is not an acceptance item.
- **Evidence executed by Codex (not by this author):**
  - 0.2: 212/212 checks passed;
  - 0.3: 333 passed, 2 recorded, 0 failed;
  - independent pycryptodome checks: 17/17 hash and address checks passed, and netKey = `0xacfd846cb4e99df47de0b04af39136202c3d466fdcdd2b1ad7a6a8586b01d044` for the 51-byte X1 preimage.
- **Not executed:** the full M0 triad (Rust sha3, TS noble, Python).
- **This turn:** the author session has file tools only. **Nothing in 0.4 has been executed by the author.**
