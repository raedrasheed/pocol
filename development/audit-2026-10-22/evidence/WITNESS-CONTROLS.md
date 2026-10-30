# Witnesses: provenance and positive controls (round 2)

## Provenance
- Every witness was **reconstructed independently** in this repair from the findings described in
  REPORT.md.
- The audit's own reproduction programs and attachments were not received: FINDINGS.json,
  REPRODUCE.md, overlays and SHA256SUMS. So none of these witnesses is the audit's program, and
  agreement with the audit's observations rests on the REPORT.md descriptions.
- The programs are exploit-shaped and are **not committed** to this public repository. They were
  delivered to the owner through the session's file channel, as an archive (see below). Only their
  observation lines are committed.

## What a NOT_REPRODUCED must mean
A witness prints `WITNESS <id> REPRODUCED|NOT_REPRODUCED`. That is an observation, never an
acceptance pass.

Since round 2, every witness also prints `CONTROL <id> OK|MISSING`. A control shows, on the same
build and through the same route, that the scenario was built and the vulnerable path executed. So
NOT_REPRODUCED cannot mean that:
- the node refused to start;
- an API changed under the witness;
- the witness never reached the path.

On the audited snapshot the positive control is the REPRODUCED observation itself: see
`baseline-090eae4.log`, and `witness-a06v2-baseline.log` for the A06 variant.

## Variant note
- The Rust witnesses v2 (A01, A02, A03, A04, A11) differ from v1 only in how the devnet profile is
  built: through the repaired admission API, since `Genesis::build` is now crate-private. The observed
  operation is the same.
- v1 cannot compile against the repaired API. v2 cannot compile against the baseline, which lacks the
  admission module. Each variant therefore runs only on its side, and the controls below cover the
  repaired side.
- The A06 v2 variant runs on both sides and reproduces on 090eae4.

## Record on `df637252fd0691dc5891295271d164bc132d04fe` ([witnesses-r2.log](witnesses-r2.log))

| witness | observation on the repair | positive control (repaired build) |
|---|---|---|
| A01 | malformed copy → envelope error; legitimate body → imported | OK: both copies reached import, and the legitimate body is head |
| A02-R9, -R13, -SYS_SLOT_MAX | each refused with its rule (-32018 [R9], [R13], [R14, R18, R19]) | OK: the unmodified profile is admitted through the same route |
| A03 | the over-limit LOG0 (32,769 B) is included, status 0, no logs | OK: a within-limit LOG0 (32,732 B, charge 32,768) in the same position has status 1 with one log, so the status 0 is the limit, not gas or decoding |
| A04 | the flipped journal fails closed (Corrupt(0)) | OK: the unflipped journal reopens with its 3 records |
| A05 | idle P2P sockets reach EOF at the Hello deadline; RPC threads capped | OK: the flood reached the node (RPC threads 7 → 39, up to the cap), and it serves afterwards |
| A06-conflict | eth_requestAccounts on the conflicting profile → 4100 | OK: the conflicting pair exists |
| A06-sign-before-approval | the dialog carries no signed bytes | OK: the transaction's confirmation dialog was shown |
| A06-lock-and-endpoint-change | 4100, nothing sent | OK: without the transitions, the same session submits (sendRaw 0 → 1) |
| A07 | no invalid spec accepted | OK: the valid spec is accepted by the same decoder |
| A08 | the malformed RP window fails | OK: the valid window verifies |
| A09 | peak session reservation 8 MiB ≤ 10 MiB | OK: one 8 MiB request is served through the same pool and session |
| A10 | singleton and batch member → -32601 | OK: an allowed batch member from the same origin is answered |
| A11 | pending nonce 1 after the reorg | OK: the transaction was accepted, and the reorg to the heavier branch happened |
| A12 | the unsettled write keeps its slot: 1 of 8 further writes refused | OK: admission engaged (1 refused, 7 admitted and queued) |
| A14 | the foreign-id / jsonrpc 1.0 reply → recvParse | OK: a correct reply to the same request is accepted |
| A13 | source review (no runtime witness) | n/a: see A13-DERIVATION.md and the execution-model measurement |

Corrections:
- The first A12 control counted settled writes and printed MISSING. Writes of one site settle in
  order, so the later ones wait behind the deliberately unsettled first write. The control was
  replaced by the admission check above, and the extension group was rerun. Both versions are
  recorded in the log.
- Round 1's dashboard counted "18/18" witnesses. Its log, like this one, has **17** observations
  (A02 has three); A13 has none. The figure is corrected here.

## Delivery of the programs
- The archive `witnesses-r2.tgz` was sent to the owner through the session's file channel, not
  committed. It holds the witness programs, the runner and the specs they use.
- Its SHA-256 is recorded in [EVIDENCE-RECONCILIATION.md](../EVIDENCE-RECONCILIATION.md).
