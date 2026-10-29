# M1 Draft 0.31: C35 native error-data bound and C36 evidence bindings

**For root review. The author ran nothing. M1 is not complete.**

## 1. C35: the 4 KiB error-data bound is measured natively

**Baseline.** browser.md:283–285: an error reply's `message` is cut to 256 UTF-16 units, and `data` is passed only if its encoding is ≤ 4 KiB. Otherwise `data` is dropped.

**The 0.30 defect.** 0.30 measured `data` with a Python serializer. Python's `surrogatepass` wrote each lone surrogate as 3 illegal UTF-8 bytes, where well-formed `JSON.stringify` writes the 6-byte escape `\ud800`. Python also printed numbers its own way (`1.0`, `-0.0`, `Infinity`).

Root's native oracle gave 6647 bytes for 1100 lone surrogates; the model gave 3347. So the guarded HeaderNetCheck:
- kept the oversized data;
- slept 250 ms;
- rendered.

**The decision.** REVIEW-DECISIONS-0.30 `sourceEncodingDecision`: match well-formed native `JSON.stringify` UTF-8 bytes, and prefer the native Node codec to a new serializer.

**The repair.** `tools/guard_ref_031.py` keeps every 0.30 rule and its order, and adds one step:

| Step | Rule | Where |
|---|---|---|
| 1–5 | declared length, received length (4096 / 98304), fatal UTF-8 (one BOM dropped), depth ≤ 16, parseStrict (duplicates at any depth, NaN/Infinity) | Python, unchanged |
| 6 | envelope: object, `id` present, C19 value equal to the request id (2.0 and 2e0 still match 2) | Python, unchanged |
| 7 | only if `error` is an object: the native codec reads the **original raw bytes** | Node `json_codec_031.mjs` |

Step 7 in detail:
- `message.slice(0, 256)` is applied if the message is longer than 256 UTF-16 units.
- `data` is deleted if `Buffer.byteLength(JSON.stringify(data))` > 4096.
- Only when one of these changes the reply is it re-serialized with native `JSON.stringify`. Lone surrogates are escaped, so the result is safe JSON. Otherwise the original text reaches the unchanged 0.27 checker byte for byte.

**Ordering.** The codec never sees a reply that failed steps 1–6. A codec failure (missing Node, timeout, oversized output, refusal, malformed report) FAILs the run. There is no Python fallback.

**Consequence of a dropped `data`.** The busy reply has no delay, so the outcome is:
- controlled malformed;
- no sleep;
- no frame, 4901.

**Unchanged.**
- The 4 KiB bound, the 256-unit cap, both receive limits and depth 16.
- The declared-length order.
- The rate [0, 2000] and busy [250, 2000] delays, and at most 3 retries.
- The shared 10 s RP budget.
- Every valid reply's bytes, hash inputs and signatures.

Valid short Unicode and normal numbers are not forbidden.

**Fixtures** (`vectors/c35-native-codec-cases.json`):
- **All 17 root oracle rows** (3 from `error-data-native-oracle-030.json`, 14 from `error-data-native-expanded-031.json`). Both files are read with explicit UTF-8 readers and their sha256 values are recorded. The rows cover:
  - lone surrogates (6647 → drop);
  - paired emoji;
  - 1900 × `1.0` (3846 natively, where Python's form is about 7600);
  - 1100 × `-0.0`;
  - 750 × `1e400`, which becomes `null`;
  - the exponent thresholds 1e20/1e21/1e-6/1e-7;
  - the IEEE 754 rounding of 1000000000000000100 and 9007199254740993;
  - `-0.0`, `2.0` and `1e400`.

  Each row is checked twice:
  - **Directly:** the native `dataBytes` must equal the oracle's `expectedJsonUtf8Bytes`, and `dataDropped` must equal `shouldDrop`.
  - **Integrated:** through the guarded HeaderNetCheck, a drop gives a controlled no-frame with no sleep; a kept row gives a 250 ms retry and then ok.

  The 0.30 Python count is recorded beside each row as defect evidence only. `C35.defectWitness.loneSurrogate` shows the 0.30 model would keep the data that native drops.
- **Exact boundaries.** 4096 is kept and 4097 is dropped, built three ways:
  - 675 escaped lone surrogates plus ASCII;
  - 1012 raw U+1F600 plus ASCII;
  - ASCII only.
- **Clipping.** 256 units passes unchanged; 257 units is clipped. A message whose 256th unit splits a surrogate pair becomes a valid escaped `\ud83d` after `JSON.stringify`, and the retry proceeds.
- **Ordering.** Invalid UTF-8, depth 17, duplicate `retryAfterMs`, NaN, id 9999, 98305 received bytes and a declared Content-Length of 98305 all fail at the guard with **zero codec calls**. A result reply makes no codec call, and an unchanged small error reply makes one call and reaches the checker as its original text.
- **Reruns through 0.31.** All 60 C33 cases of 0.30, the 24 HeaderNetCheck deadline cases of 0.29 and every reviewed 0.27 transcript, plus the shared-budget RP-COMB.

**Loader history.** Root's first native comparison (`error-data-native-comparison-030.json`, 2 failures) used the platform default encoding. It is preserved as a corrected harness issue, not a source finding (HF-13). The remaining genuine lone-surrogate failure is HF-14, now bound to the C35 checks.

## 2. C36: the six 0.30 FAILs

These were evidence-binding defects. They stay FAIL in root's saved 0.30 results. `vectors/c36-binding-repairs.json` maps each one to its repair:

| 0.30 FAIL | Repair in 0.31 |
|---|---|
| `bind029.reviewAcceptsUnaffected` | `C36.reviewDecisions030.acceptedUnaffected`, which parses `REVIEW-DECISIONS-0.30.json`: `acceptedUnaffectedRows` = C1, C2, R1, E6; zero personal owner answers |
| `C33.source.scopeProbes.provenance` | `C36.scopeProbes.parsedLiterals`, which parses the probe JSON and compares each `literal` field with the exact C33 fixture text (duplicate `error.code` against G-DUP-errorCode, also against root's corpus; id 9999 against G-SCOPE-busyWrongId) |
| `convention.P-V1-11.decided` | re-evaluated on this run's C33, C35 and C36 checks |
| `consolidation.rowsMatchProposal` | recomputed |
| `coverage030.required` | `coverage031.required` (full set, no waiver) |
| `history.HF-8` | `history.HF-8`: the 0.29 failures stay FAIL; three are repaired by 0.30 passes, and `coverage029` by `coverage031.required` |

No earlier review was edited, and no test was rewritten to match an invented phrase.

The original C33 and C34 defects stay closed in the ledger and are not reopened; the runner checks this (`bind030.ledgerNotReopened`). C35 and C36 are open, with the repairs proposed here.

## 3. What is bound rather than recomputed

The following are bound to root's 0.30 results by status, and the outputs by sha256 (`bind030.*`):
- the C34 repair: N05 and the eight mutants;
- `decision.U08.applied`;
- the freeze outputs (668 legacy entries and 4024 V1 entries, with opaque IDs labelled, not frozen);
- the delegated-decision bindings;
- histories HF-9 to HF-11;
- the experiment-definition gate.

Root's independent evidence is bound by summary:
- 23/23 guard checks;
- 1120 deadline cases;
- 668 triad preimages;
- 3677 hashes;
- 31 signatures;
- 1027 ASERT cases.

No crypto fixture is rebuilt, and no freeze hash is recomputed.

## 4. Gate

All 41 rows and F01–F26 are recomputed from root's 0.30 matrix, root's 0.29 row evidence, `REVIEW-DECISIONS-0.30.json` and this run.

**c4.** It uses the user's standing delegation and the specific decisions: the nine selections and the 17 conventions. P-U08-1 is bound to R30, and P-V1-11 to this run.

**Expected statuses** (computed, not asserted):

| Status | Rows | Why |
|---|---|---|
| CompleteCandidate (38) | the 34 earlier candidates, plus C1, C2, R1 and E6 | the last four were accepted as unaffected by REVIEW-DECISIONS-0.30 |
| PendingRootReview (3) | C3, C4 | final bindings of the 0.30 repairs, per the REVIEW-DECISIONS-0.30 conditions |
| | V1 | C35 and C36 |

**Findings:**
- 23 closable at specification scope;
- F09, F17 and F25 closable after root review of 0.31.

**Phase A.** X-C33, X-U14 and the E-experiments are definition gates only. Their outcomes come later.

**Not Complete.** No row is Complete before root executes this run with Node and reviews it.
