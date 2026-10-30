# M1 Draft 0.3 — Dispositions of Coordinator Issues C01–C09

Sources:
- `../coordination/issue-ledger.json` (read only; not edited);
- `../coordination/task-001.md`;
- `../coordination/review-001/independent-probes.json` (the Codex probes);
- `../M1-CLAUDE-REVIEW.md`, `../M1-CODEX-RESPONSE.md`, `../m1-draft-0.2/`.

**No issue is closed by this document.** Closure is for Codex review and for the ledger owner. Each "Proposed status" is the author's view of what review should check.

**Evidence level.** Every check named below is written, but **none has been executed in this turn**: the session had no command-execution tool. Each item is therefore at most "drafted + fixtures (unexecuted)".

| Issue | Ledger decision (summary) | Revision | What changed | Literal evidence | Proposed status |
|---|---|---|---|---|---|
| C01 | Full gate; complete the missing work; no carve-out | R3-07, R3-08 | Inventory of all 41 rows with defining FD lines and dependencies; acceptance separated from experiments without weakening completeness; batch 1 in part (X1 preimage, V1 RW/HC); 10-batch plan; X3 found undefined in the baseline (BLK-02) | `M1-ANNEX-INVENTORY-0.3.md`; `vectors/annex-batch1.json` | Open. 6 partial, 1 blocked, 34 not started, 0 complete |
| C02 | 212 saved checks are not an independent rerun; find an interpreter | R3-10 | Runner made independent of `sys.path` defaults; `rerun_02.py` re-runs the unchanged 0.2 checks into `results/rerun-0.2/` | `tools/run_checks_03.py`, `tools/rerun_02.py` | **Blocked in this turn:** nothing executed. The commands are in the README. The 0.2 author results are still not independent evidence |
| C03 | Deterministic rejection of malformed or deep input without uncaught recursion; verify boundary precedence | R3-01 | Iterative total decoder (P31); no depth rule (P32, U23); child-boundary precedence (P33) | `vectors/rlp-adversarial.json`: 16 fixtures, including 5 literal precedence cases where 0.2 and 0.3 differ or agree, the 1201-level Codex probe size, the 21916-level maximum at 65536 B, and depth with trailing, truncated and non-canonical variants | Drafted + fixtures (unexecuted) |
| C04 | Bounded lexical parsing; overflow always `malformedSelector` | R3-02 | Lexical check before conversion (`selector_value`) | `vectors/selector-cases.json`: 17 cases (u32/u64 wrap traps, 4301 and 5000 digits, non-ASCII digits) plus all 0.2 path cases re-run | Drafted + fixtures (unexecuted) |
| C05 | A unique count is not evidence of at-most-once requests | R3-03 | The provider counts every invocation; a ChunkFetcher model with a session cache keyed by (addr, len), shared by manifest and content; `requests`/`uniqueKeys`/`references` | `vectors/fetch-requests.json`: hand-derived counts for all 11 0.2 positives; 3 constructed fixtures (shared manifest/content cache 4 vs 5 vs 104; same address with a different length, 2; reload, 0); no-cache mutant checks | Drafted + fixtures (unexecuted) |
| C06 | No frozen snapshot guarantee without evidence and baseline reconciliation | R3-04 | Guarantee explicitly **not frozen**. The reconciliation found the baseline conflict FD:L1274/L1262–1273 (BLK-01) and the importer restriction FD:L4934. CR-M1-01 proposed with falsifiable tests; fallback P36 | `vectors/snapshot-mock-scenarios.json`: 14 scenarios, including A → B, A → B → A, and A → B → A under the detector (the mixed render is demonstrated); `tools/snapshot_model.py` (abstract proofs) | Open: BLK-01 + owner Q-C06 + E03 |
| C07 | Owner choices U01/U02/U10/U14/U16 stay proposed | — | Unchanged. Re-listed in `M1-OPEN-0.3.md` §3 | — | Open (owner) |
| C08 | Establish the provenance of the design ID; preserve the reference | R3-09 | Found: the ID sits inside FD:L3, so it cannot equal the raw SHA-256 (that would be a fixed point). `baseline.json` names `state(3).json` as a source, and that file is absent. Probe script for candidate derivations | `tools/design_id_probe.py` (not run) | Open: owner Q-C08. No corruption claim, no match claim |
| C09 | Correct T1-12 AlreadyCurrent interference and T1-10 immutability; specify version width | R3-05, R3-06 | Width table (manifest version u8; IDs uint32; publishedBlock uint64); T1-10 split into four actions with exact permitted bit ranges; T1-12 with non-current targets and the exact error; T1-16 ABI width (P37) | `vectors/version-width.json`: 4 literal manifests, including the 257 mod 256 trap | Drafted + fixtures (unexecuted); T1 needs E01 |
| F01–F26 | No automatic closure | — | Statuses unchanged from `../m1-draft-0.2/M1-DISPOSITIONS-0.2.md`. Touched by 0.3: F07 (R3-04), F13/F14 (R3-01, R3-03), F17 (R3-06), F18 (R3-02), F19 (R3-05), F26 (R3-08) | — | Open |

## Correction to draft 0.2 evidence claims

These 0.2 statements were inaccurate or overstated. The 0.2 files are left unchanged; the corrections apply to them.

1. 0.2 §3.4 and the fixtures: `expected.fetchCalls` was described as "the exact number of distinct `eth_getCode` calls". The model counted distinct addresses but invoked the provider once per reference. So the figure was not a request count (Codex probe C05).
2. 0.2 §3.2 note: "Distinct chunk addresses are fetched at most once per load". The 0.2 model did not implement this, and 0.2 asserted no fixture for it.
3. 0.2 §2 P02: the u8 width of `version` was in the model only, not in the prose (C09).
4. 0.2 `rlp_strict.py`: a recursive decoder with no bound; children were bounded by the input rather than by their parent (C03).
5. 0.2 `m1paths.py`: an unbounded `int(digits)` (C04).
