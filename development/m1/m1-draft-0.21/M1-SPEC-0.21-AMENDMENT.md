# M1 Spec 0.21 Amendment: V3 NetworkProfiles.validate

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Accepted:** 0.20, at 39 Partial / 2 Not started (V3–V4) / 0 Complete.
- Owner decisions U01, U02, U10, U14 and CR-M1-01, CONF_DEPTH, the full gate, the CR-E4 proposals and RF-E6-1 are unchanged. No merge or deploy.

## Adds

1. **`annex/V3-NETWORKPROFILES.md`.** English normative text:
   - the acceptance order and error codes;
   - the full GenesisSpec decode order with widths and bounds;
   - the RecvFit table with exact hand-derived boundaries;
   - trust-pool fetch semantics, node-start policy, deferred experiments and gaps.
2. **`tools/netprofile_ref.py`.** A pure-stdlib validator: framing parser, decoder, `validate`, `may_start_node`, `FakeTransport` and `fetch_and_validate`. It reuses the accepted 0.2 Keccak/RLP read-only.
3. **`vectors/v3-gsv1.json`.**
   - The literal 341-byte GSV1, in flat segments and as an item tree.
   - Hand-decoded values and the K1–K3 vectors.
   - 32 decode negatives: N1–N10 plus supplements for widths, system ids, header/structure and precedence.
4. **`vectors/v3-profiles.json`.**
   - GSV1 trust variants and N1/N6/N11.
   - The chainId case and shape negatives.
   - RF1–RF4 with all exact edges and a multi-violation case.
   - Literal transport scripts.
5. **`tools/run_checks_021.py`.** A standalone runner:
   - input SHAs and preserved-file hashes;
   - a safe positional-only logger that copies diagnostics;
   - provenance literals and a coverage guard.

   It writes only `m1-draft-0.21/results/run-results-0.21.json`.

Source points needing review are DG-V3-1 to DG-V3-9 and P-V3-1 to P-V3-7 (annex §6).
