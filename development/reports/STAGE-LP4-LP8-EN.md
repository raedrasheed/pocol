# LP4–LP8 stage report — 21 October 2026

Branch: `claude/affectionate-pascal-rjhh2k`. Authorization: [OWNER-STANDING-AUTHORIZATION.md](../OWNER-STANDING-AUTHORIZATION.md).

## Completed
**LP4 — Execution and state**
- Merged the P3 line (signed transactions and RPC) with P4 on one branch.
- Adopted LP4-LEDGER-01 (the recommended option) under your authorization.
- Complete v1 system contracts: miner registry, escrow (pending pots, settlement, finalization and burns), share ledger, chain constants and chunk factory.
- Complete four-phase block-application path with escrow invariant checks.
- SV1–SV7s values matched the specification exactly through the compiled contract.
- Closing and the final block preserve total value exactly.
- R18 resource limits: a derivation with a minimum-bound test (one below the bound fails), and a worst-case measurement with 256 identities: 84.2 million gas out of 96 million.
- Two new engineering decisions adopted by recommendation: LP4-FINAL-01 and LP4-GAS-01.
- R21: a state-retention window with verified reconstruction.
- P9: a durable, disk-synchronized journal that recovers from interruption at any byte and excludes a second writer.

**LP5 — Block production, mining and chain selection**
- The pocold node produces real blocks from a fresh genesis: a template signed by the lottery-selected proposer, a fallback template, nonce-range mining with shares, and idling after range exhaustion as in the thesis.
- Full H-pre/H-full checks, the D133 fork-choice rule, and type 1 and type 2 evidence.

**LP6–LP7 — Networking and multiple nodes**
- VA-spec was written before adoption, covering a framed peer protocol and synchronization.
- A test with three real processes passed 14/14 checks:
  - 100 blocks and a late join.
  - Network partition followed by healing onto the heavier branch, forced termination with SIGKILL and recovery.
  - Malicious inputs and transactions through RPC.
  - Matching roots at every height and deterministic replay.

**LP8 — On-chain websites**
- The Website contract uses the specified slots; pocol-site supports publishing and fetching.
- Complete chunk, manifest and content-hash verification, and T16 rejection vectors.
- A live test passed 14/14 checks: publishing two versions, fetching from each node, rejecting unauthorized operations without effects, and restarting.

## Tests performed
- LP4 suite: all library tests pass.
- pocold: 32 unit, 6 consensus, 3 storage and 4 site tests.
- Two live tests using real processes.

## Not yet completed / explicit limits
- Independent P10 review is in progress (a separate reviewer agent).
- Windows checks and GitHub CI were not run on this branch (a PR is needed).
- Sharing trie structure between versions is not implemented; the R21 bound applies to each full snapshot.
- Detailed RpcGuard policies (pricing and reservation) are not fully implemented.

## Next
- **LP9:** a Chrome MV3 extension displaying sites from PoCol state with safeguards, tested in real Chromium.
- **LP10:** an integrated runner and end-to-end test.
