# LP2 progress — Durable candidate-header admission (author revision 1, handoff 0.37)

**Status:** code and tests written, not compiled or run by the author. No acceptance claim until the root executes P0–P8 and Codex reviews them.

**Design behavior:** an experimental local candidate-header journal.

- A header is accepted after strict decoding and the specified H-pre checks (NetID, items 2–9, height, parent and H_END), with a `U256::MAX` ceiling and no wall clock.
- Metadata binds the SHA-256 digest of the exact profile-file bytes, together with genesisHash and chainId.
- Records are hash-linked, with chained checksums, a length guard and a commit marker; strict bounds are checked before any memory allocation.
- The writer is exclusive on Windows through `share_mode(0)`; the OS releases it on process crash, and there are no lockfiles.
- The whole batch is validated before any write, then each record is written and synchronized separately. Atomicity is per record, not per batch; any write error disables the writer until reopening.
- A torn tail is reported as requiring recovery. Recovery copies the verified prefix into a new directory only and never touches the source.

**Always-reported status:** `candidate`, `awaiting: H-full`, `executedHeight = 0`, `consensus = false`. No execution, canonical chain or live consensus.

**Limits:**

- The writer is supported only on Windows; portable locking elsewhere is not claimed.
- No directory-entry synchronization.
- The journal is linear and rejects branches. Branch rejection is not a judgment that a block is invalid.
- The format is experimental and may change without migration; it is not the final redb/RetentionStore design.
- Fixture headers added in tests are not live block production.

**Next:** LP3 (bootable genesis and ParamGate), then LP4 (EVM execution and H-full), follow LP2 acceptance. Execution, live consensus, production and synchronization remain for later stages. M1 0.32 is unchanged.
