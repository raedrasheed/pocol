Owner decisions pending after review 0.28 — Current coordinator guide
These are recommendations, not approvals. All nine decisions below await your answer. This message does not amend specifications or launch an author task.

1) U01 — Factory address
Problem: the source abbreviates the address, but tests need the complete address from which content-chunk addresses are derived.
Options: confirm 0x0000000000000000000000000000000000c0c005, or specify another full address.
Recommendation: confirm this expansion if it is the intended address. Tests built around it do not establish that it was your choice.
Effect: confirmation preserves current values; another address requires regeneration and verification of addresses, CREATE2 inputs and affected vectors. No actual deployment now.

2) U02 — Explicit request for a revoked version
Problem: default viewing of a revoked version is prohibited, but the text does not settle an explicit request through @v<n>.
Options: allow after a warning page and confirmation click with a persistent banner, or always prohibit viewing.
Recommendation: allow subject to warning and confirmation, so a historical version can deliberately be inspected.
Effect: default viewing remains prohibited; users can intentionally open a revoked version. A permanent prohibition is stricter and changes the viewer acceptance table.

3) U10 — Previous owner's publishing permission
Problem: ownership transfer does not specify whether the previous owner's publishing permission is removed.
Options: retain it until the new owner explicitly removes it, or revoke it automatically on transfer.
Recommendation: retain it with a clear transfer warning and a permissions list visible to the new owner.
Effect: the former publisher can continue publishing until removed. Automatic revocation grants less authority on handover but changes contract behavior and T1-12. The choice depends on your intended meaning of ownership transfer.

4) U14 — Scope of the ten-second timeout
Problem: is the timeout per request or for the entire operation? Responses taking 1.6 seconds with three waits can finish at 14.05 seconds even though each individual request is fast.
Options: a separate timeout per request; one timeout until the last response; or one timeout until the final result, including responses, waits and computation. Content loading and header checking may have different policies if explicitly intended.
Current recommendation: one timeout from the first request to the final result, including computation and waits. Also specify whether it applies to both site loading and header verification; this is not assumed implicitly.
Effect: prevents a remote operation from holding resources for too long, but may reject a large site or a legitimate slow operation exceeding ten seconds. Per-request timeouts permit those loads without bounding total duration. Ending at the last response leaves computation without a total bound.
Note: Claude's earlier recommendation was per request; Codex's current recommendation differs after timing analysis. Neither is a decision. Six seconds of total retry waits alone does not establish an overall timeout.

5) CR-M1-01 — Proof-backed site-state reads
Problem: rereading state through eth_call alone does not prove a single snapshot when state changes and returns to its previous value, especially in RP. The proposal binds reads to a state root committed in a specific header.
Options: adopt the amended change request; request changes; or reject it and retain the weaker guarantee with clear disclosure.
Recommendation: adopt subject to size/retry limits and the later E03 experiment verifying trie-proof support.
Effect: adds eth_getProof to the internal client only, with a 524288-byte response bound and a 1 MiB STATE_POOL. Sites remain prohibited from calling it. Rejection retains change-detection reads without allowing a claim of an authenticated snapshot. Implementation and measurement come later; a specification decision does not establish their success. The proof-key count depends on U08; timeouts require explicit alignment with U14.

6) U08 — Manifest chunking
Problem: the baseline permits multiple chunks; the draft proposes 1–3 chunks of prescribed sizes to fix write/proof costs.
Options: A) canonical chunking enforced by contract and viewer; B) at most three chunks with flexible sizes; C) retain all chunkings in both contract and viewer and redesign proof limits; D) leave the contract unchanged and have the viewer reject noncanonical chunking.
Recommendation: A. If you do not want to narrow contract inputs, D is an alternative, with disclosure that some stored versions cannot be viewed.
Effect: A keeps all valid content publishable but prevents alternative chunking. B retains the proof bound with less deterministic chunking. C preserves chunking freedom and potentially increases proof requests and memory. D permits storing an unviewable version. The six-key proof bound depends on A, B or D.

7) RF-E6-1 — Stale-request count in x12b
Problem: the text says one stale request was dropped, while the complete timeline with cancellation disabled in both operations yields two.
Options: A) add cell 5001 and correct the total to 2, one per operation; B) restrict the fault to the first operation, retaining total 1; C) clarify that 1 refers only to the first operation, with total 2.
Recommendation: A, matching the general fault and computed timeline.
Effect: corrects the count and vector description; stale requests still cause no allocation, ticket or write. B changes the fault scope; C preserves the number while clarifying its meaning. Your previous question remains open; no choice will be made on your behalf.

8) V1-SHA-COST — SHA-256 operation bound
Problem: the source says 13 operations, but all required checks need up to 14 template hashes + 13 proof-of-work hashes + 3328 share hashes = 3355. The operations and distinct inputs were independently verified. Caching reduces repetition but cannot reduce this bound to 13.
Options: A) correct the bound to 3355 with component bounds and retain every check; B) make 13 refer only to proof of work, without a written total bound; C) remove share checking, leaving 27 and possibly accepting a header with invalid shares; D) reduce accepted shares, narrowing the valid headers that can be displayed; E) retain literal 13 and leave V1 blocked or redesign it.
Recommendation: A. It corrects the cost description without weakening checks.
Effect: only the written cost bound changes; acceptance rules remain. These calculations do not establish device timing. The source has not yet changed.

9) P-C32-2 — Waiting after a rate response
Problem: busy has a bounded duration, but the source does not define the rate response's format or maximum wait; a remote server can request a long wait.
Options: A) accept an integer from 0 to 2000 ms and reject larger values without waiting; B) specify the server format first, then derive a tighter bound; C) accept any nonnegative integer duration and rely on an overall timeout; D) use 250–2000 as for busy; E) do not retry rate responses.
Recommendation: A as an explicit client policy, with U14 settled. The ceiling is not treated as already present in the source.
Effect: total retry waits are at most 6 seconds, excluding response and computation time. A server requesting more than two seconds immediately loses the display. B requires another source definition; C may greatly extend an operation if timeouts are per request; D rejects valid short waits such as 50 ms; E rejects transient congestion and changes the retry rule.

Operational status: malformed-response defects were fixed and verified. Corrections and calculations are complete within this scope. C1–C4, R1, E6 and V1 remain blocked on owner choices; reviewed evidence for other rows is preserved. You can answer with a decision ID and option, or describe another option in detail. This message records no answer from you.
