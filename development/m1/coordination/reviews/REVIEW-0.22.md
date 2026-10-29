# Codex independent review — revision 0.22, author turn 020

Preserved-copy execution on Python 3.11.1: 248 entries, 196 passed, 52 recorded, zero failures, 13.0 seconds, exit 0. All 139 entries of the old V3 suite ran again. The new parser uses an explicit stack, retains wrapped-single-byte metadata and existing L0 details, and introduces no recursion-limit change or input restriction. Library provenance now checks protected source contents while allowing preserved review paths; the old copy-path mismatch remains recorded.

Twenty-three root probes passed against the actual repaired parser. They cover nested lists at depths 1, 10, 1500 and 5000 in the top field, CP leaf and member identity, truncated deep inputs, gsInt for a wrapped version integer, valid GSV1 after rejections, and exact byte round-trip. The root probe's initial expectation for the one-level list containing version zero was corrected to gsVersion according to the documented precedence; that initial expectation result is preserved separately. It was not an author defect or a dropped assertion. Deep malformed shapes reject with gsStructure, and framing failures remain L0.

C30 is closed as a reference parser defect. The verified E05 GSV1 hash is recorded in the new supplement; its 341 bytes are unchanged. Root evidence from three independent libraries remains valid for K1–K3 and that exact preimage. Other hash-bearing fixtures still require the final freeze audit.

Verdict: accept the scoped V3 specification/reference-fixture batch. Coverage is 40 Partial, 1 Not started (V4), 0 Complete. Recorded model conventions, trust/transport supplements and genuine experiment-definition gaps remain distinguishable from baseline authority. Actual node start, TS/Chrome execution and node-side alloc-root validation are implementation experiments, not fabricated evidence or new work in this task.

Owner decisions, CR-E4 representation proposals, RF-E6-1, CONF_DEPTH and the full specification gate are unchanged. Continue X8 fixture definitions and then the consolidated completeness audit. No merge, deployment or production implementation.
