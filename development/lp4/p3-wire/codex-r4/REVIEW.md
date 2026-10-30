# Separate sequential author review — no GitHub approval implied

Review followed implementation and scoped test execution. No Claude, coordinator,
subagent or delegated reviewer was used. GitHub protection/review requirements
remain independently mandatory; no merge or acceptance is requested here.

Reviewed native/profile entry, deposit wrapper, inventory resolution/checked sums,
complete base/journal union, actual deletion timing, cfg restoration on errors,
upstream EIP3541/initcode ordering, fixtures, both independent comparator adapters,
compiled controls and workflow. The unchanged Cancun control binary remains the
baseline; the new delta binary is explicitly unsigned/synthetic and isolates code
semantics, not the complete PoCol PAY/header/chain admission contract.

Findings resolved:

1. revm's code-size configuration also changes initcode. Keep it unchanged until
   deposit return. Actual49153 negatives and a compiled coupling mutation verify
   the fix; no transaction validator or opcode was patched.
2. Fixture source's chain ID differs from synthetic777902. Encode actual chain ID
   into each complete profile before identity/hash binding. Initial failures are
   preserved, not success. Signed execution now verifies all76 cases.
3. py-evm's oversized top-level initcode exception can leave its isolated candidate
   mutated. Preserve the discarded root and rebuild/discard the entire candidate;
   do not claim unmodified engine atomicity. Rust input snapshots remain immutable.
4. Fresh selfdestruct is only scheduled before transaction end. Prevent early
   credit; verify next-transaction release versus retained-code denial independently.
   Initial follow-up gas1M did not fit remaining8M block gas; use a valid100k
   transaction, preserving both block and transaction gas rules and the failed log.
5. Six frozen corpus pre-states have50000-byte code. Retain all six original Cancun
   controls and enumerate each new snapshot rejection. Independent actual VM
   inventory verifies rejection; full GenesisBuilder remains NOT PASSED. Do not
   label all14 deltas as successful amended transaction execution.
6. First compiled-control detector missed two assertion forms despite real test
   failures. Require explicit assertion diagnostics; preserve that failed detector
   attempt and rerun final controls. Build failures never count as detection.

Limits retained explicitly: source/profile/model resource binding and complete
ParamGate, actual genesis/system allocation, P2 integration, T2c, overflow proof,
persistence/crash/reorg, real system phases/finalization/SV6 and generalR18/R21/
caps64 are unverified. Complete-state scans and DB cloning need general host-work
and memory accounting; no performance or memory bound is inferred from CODE_CAP.

Source bindings verify all changed implementation/test/CI bytes. Review found no
remaining local defect in the scoped tested paths; this is not a full P2/P3/LP4
acceptance review. Boot remains false; all27 ledger gates remain NOT PASSED.

The raw Cargo log retains its original blank final line. The complete staged
whitespace check reports that evidence-only EOF warning; source/documentation
checks excluding raw logs pass. Evidence bytes and their digests are preserved,
not normalized, and no completely clean full-tree whitespace check is claimed.
