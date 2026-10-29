# M1 consolidated specification audit (draft 0.25)

**For root review. Not approved. The author ran nothing.** The machine-readable sources are:
- `row-inventory.json` (41 rows);
- `findings-trace.json` (F01–F26);
- `decision-register.json`;
- `gap-registry.json`;
- `../hash/hash-freeze-plan.json`.

`tools/run_checks_025.py` computes each row's R3-08 criteria from these files and from the saved, reviewed evidence. The tables below are the values the author expects that run to produce. They are not results.

## 1. How evidence is reused

- **Saved evidence.** Each row cites saved review results by file and check-ID pattern, for example `R02 ^positive ` in `coordination/review-001/m1-draft-0.2/results/run-results.json`.
- **Binding to the current files.** The runner requires every reviewed copy under `coordination/review-001/m1-draft-0.X/` to be byte-equal to the current package. The saved results then still describe the current fixtures, and no earlier runner is re-run.
- **Old failures.** The three saved FAIL entries that the cited patterns reach are each tied to the later evidence that closed them:
  - `bound.length rb-2848-does-not-fit` (0.4) and `c14.fixture rb-2848` (0.5): C14, closed by `c14.*` in 0.6.
  - `br19a.a7 …` (0.6): closed by `br19a.*` in 0.8.
- **What is not counted.** Production TypeScript, Chrome and EVM runs (Phase A) do not count against a row (R3-08, R4-02). A missing experiment *definition* does count.

## 2. Row criteria (expected on root's run)

Legend:
- **S**: satisfied.
- **P**: pending root review of 0.25 material.
- **B**: blocked.

| Rows | c1 spec + fixtures | c2 executed | c3 reviewed | c4 decisions | c5 blockers | Blocked by |
|---|---|---|---|---|---|---|
| C1, C2, C4 | S | S | S | B | B | owner U01 (C4 also U10); reviewer items |
| C3 | P | P | P | B | B | owner CR-M1-01, U02, U10, U14; reviewer items; E07 table new in 0.25 |
| R1 | S | S | S | B | B | owner CR-M1-01 |
| E6 | S | S | S | B | B | owner/root RF-E6-1 |
| V1 | B | S | S | S | B | missing definitions V1-WINDOW, V1-TEXTS, V1-NX |
| X1, B4 | S | S | S | B | B | the netKey hash (reviewer P-X1) |
| B8 | S | S | S | B | B | the txA hash (reviewer TK-1) |
| E3, E4, V2, V3 | P | P | P | B | P | reviewer items; 0.25 supplements or representations |
| X2, X3, B1–B3, B5–B7, B9–B11, R2–R4, S1–S4, Q1–Q4, E1, E2, E5, E7, V4 | S | S | S | B | S | reviewer items only |

Summary:
- **Complete:** no row.
- **Blocked only by reviewer decisions:** 34 rows (including E3, E4, V2 and V3 once their 0.25 material is reviewed).
- **Owner-blocked:** C1–C4, R1 and E6.
- **Blocked by missing definitions:** V1.

V1 is the one row with genuinely unauthored source-defined content:
- the HeaderNetCheck window vectors;
- the HC message texts;
- NX-V3e/f/g.

NX-V3e/f/g also need the M3-TS/ASERT model (FD:L4951).

## 3. Supplements in 0.25 (each labelled; earlier failed cases preserved)

| ID | Closes | What runs |
|---|---|---|
| S25-SWEEPMAX | 0.16 gap `BR22a-sweepMax` | sweepMax as a World subclass. The source names it but does not spell it out, so it is read as a remove issued with a set and held with it. The runner checks: equivalence with the accepted World on all 520 combinations; that the control cell (P1, none, no, A = L0, remove = L1) fails c1 and c7a; that the same cell passes without the fault; and it records the full grid |
| S25-LCLIT | DG-V2-2 / P-V2-10 | 45 cases and 9 controls exported as literal request→reply transcripts. Replaying them through a server that serves only those literals reproduces every fixture cell |
| S25-RG3B | DG-V2-1 | The hook protocol (sink-blocking hook, control-connection adoption signal, 120 s limit, preconditions) and three scripts with pass criteria. Four model-analogue runs check the trace, the requests, the result, the totalRequests count and the strict SinkChecker |
| S25-E07 | F04 (no viewer fixture) | Version-selection table T43-1…9. Both U02 columns are written; neither is chosen |
| S25-EXPSPEC | E01–E07 definitions | Exact inputs and pass criteria. The E06 site construction is derived from `path-cases.json`. The T12 site bodies are M2 content |

The d15c boundary cells and the d13 scope are not new work:
- **d15c (10100, 60300):** decided by saved E5 evidence (`e5.case BR22d-V-d/e`).
- **d13:** the source states properties, not a literal table.

## 4. Original findings F01–F26

- **Propose closure at spec scope:** F01, F03, F13, F14, F15, F20, F22, F26. Each has spec text, fixtures, executed evidence and root review, or is a contract-only behaviour with a complete definition whose outcome is E01/E04.
- **Propose closure once root records reviewer decisions:**
  - F02 (U22), F05 (U04), F08 (U06, U22), F09 (U07 rule, U08), F10 (U09);
  - F12 (U11, U22), F16 (U12, U17), F18 (U13), F19 (U20), F21 (U21), F24 (U16).
- **Open on owner decisions:** F04 (U02), F06 and F23 (U01), F07 (CR-M1-01), F11 and F17 (U10), F25 (U14).

The eight Codex corrections (errata 1–8 of `M1-DISPOSITIONS-0.2.md`) are each tied to their adopting text and evidence. The ledger entry "F01-F26 open" is replaced by these individual proposals. Nothing is closed by this file.

## 5. Hash-fixture freeze

**Existing evidence is reused.** The runner rebuilds all 611 triad preimages from the current fixtures by the generator's rules. It requires byte equality with `inputs-expanded.tsv`, and it requires the TSV and result SHA-256 values to equal root's `e05-historical-evidence-inventory.json`. The Rust result is read as UTF-16LE. K1–K3 and GSV1 are bound to `e05-keccak-three-library.json`.

**New preimages never checked by three libraries** are exported for root:
- the ABI selectors and topics;
- the Website slot keys;
- the EIP-1186 slot keys 0–2;
- the C14 content hash;
- the txA hash;
- the unadopted alternative-E selector.

**Blocked, never frozen:**
- every CREATE2 preimage and every manifest that lists a chunk address (owner U01);
- the selectors and slots (reviewer P-ABI/P13; E04 confirms them in Phase A);
- the netKey (P-X1);
- txA (TK-1).

**Freezable now:** K1–K3, GSV1, the 486 file contents, and the chunk data, runtime and initcode.

**Not frozen, by class:**
- Synthetic labels (the LogClient branch hashes, the GSV1 placeholder roots, the uniform test hashes) keep their literal values.
- Private-source SHA-256 values are not exported.
- Key-derived addresses are excluded (HF-BLK-KEYS).
- V3 network-profile digests over placeholders are blocked (HF-BLK-DGV3).

The runner also classifies every 64-hex token in every earlier JSON fixture. One that fits no class fails the run.

## 6. Representation alternatives (not applied, not owner-approved)

**CR-E4-01.**
- **The problem:** a JSON-number epoch read in JS collides at 2^53 + 1 (it reads as 2^53), and 2^64 − 1 reads as 2^64, outside the domain.
- **Recommended (A):** epoch as the name's hex16 string.
- **Cost:** 2 bytes. The maximum model record becomes 1442009 ≤ 1442048, and the netKey bound becomes 105 characters.
- **The u64 domain is unchanged.**

**CR-E4-02.**
- **New finding F25-SUR-1:** the baseline already rejects lone surrogates in storage_set keys and values at B3, through `str(n)` (browser.md:200, 225). The 0.4 model and its case BR17d-lone-surrogate implement this.
- **Consequence:** the 0.13 claim that this "needs a baseline change" is incorrect. The collision is unreachable through the bridge.
- **Recommended (R):** no baseline change. Relabel P-Q1-1 as defensive and keep `unpairedSurrogate` in the codec.
- **WTF-8 alternative:** written, with fixtures, only for the case where the owner wanted lone surrogates accepted.

**CID-1.** The profile chainId shows the same JS-number boundary:
- 9007199254740993 reads as 9007199254740992;
- 18446744073709551615 reads as 2^64.

The effect is a wrong step-3 match and a wrong netKey. **Recommended (A):** exact integer reading. The networks.json format and the domain are unchanged. Every existing fixture chainId is ≤ 2^53 − 1.
