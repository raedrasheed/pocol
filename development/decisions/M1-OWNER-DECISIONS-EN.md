# M1 decisions requiring Raed

Status: open. These are proposals, not approved decisions.

| ID | Required decision | Why it is needed |
|---|---|---|
| U01 | Confirm factory address `0x0000000000000000000000000000000000c0c005` | The source abbreviates the address; current vectors assume this expansion |
| U02 | Allow an explicitly selected revoked version after a warning and confirmation, with a persistent banner | Defines revoked-content viewing; default viewing remains prohibited |
| U10 | Retain previous publishers' permissions after ownership transfer until the new owner removes them | Determines who remains able to publish |
| U14 | Apply the ten-second timeout per request, rather than to the entire load | Affects large-site loading and delay handling |
| CR-M1-01 | Adopt state-proof reads and amend receive resources, or retain the baseline and redesign reading | The method is absent from the extension's current closed method list |

CR-M1-01 proposal: Ethereum-format account/storage proofs, up to six slots per request, a 512 KiB receive bound and a new 1 MiB STATE_POOL. It also proposes retry limits: up to 48 sends, waiting up to ten seconds per attempt and twenty seconds cumulatively. Sites themselves remain prohibited from calling eth_getProof.

Technical text: [CR rev 2](../m1/m1-draft-0.5/CR-M1-01-REV2.md), with size/pool limits in the [0.4 proposal](../m1/m1-draft-0.4/CR-M1-01-STATE-PROOF-READS.md). Actual node support and real proof verification have not been implemented.

The single-eth_call alternative may provide a consistent snapshot from an honest local node, but does not prove that a malicious RP server's response is bound to the state root. The model comparison in 0.8, after correcting C23, confirms this distinction; the alternative is not adopted as equivalent.

Test-tool choices and work-batch ordering do not require new owner approval. Publisher transaction-confirmation depth is a required operational input with no invented default. The full-specification gate remains in force; there is no early implementation permission.
