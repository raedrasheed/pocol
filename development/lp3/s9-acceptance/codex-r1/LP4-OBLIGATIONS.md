# LP4 execution handoff — not LP3 passes

Standing authorization allowed completion of amended LP3. The following original
obligations remain mandatory and are not silently satisfied by its pure models:

1. Implement real GenesisBuilder allocation-state MPT and system code; independently
   derive allocRoot/sysCodeHash and compare actual outputs before any future boot.
2. Generalize/prove R18 SYS resource derivations and R21 trie memory bounds; version
   and bind complete inputs, compiled code/artifact and model. Distinguish measured
   facts, synthetic fixtures and mathematical bounds. Missing/incomplete/mismatched
   provenance must reject acceptance. No caller-supplied success flags.
3. Derive the caps/64 minimum SYS_SLOT_MAX, including minimum/minimum-1 and cap-family
   boundary controls. See the five explicit tests in LP3-AMENDMENT-01.md.
4. Real closing/finalization/cursor/state execution: <=RET records/block and exact
   persistence; absence of pending[F]; zero final subsidy from actual state effects;
   actual no-valid-child-of-final transitions. See four explicit tests in the CF
   approval. Core supplied-parent predicates are not state validation.
5. Real Escrow/SV6 and all balance/payment/burn/account effects; no pure arithmetic
   conservation result may substitute for an execution transition.
6. Full H validation and actual block acceptance must precede any future production
   boot. LP3 leaves boot disabled. Physical scheduling/callback integration is LP6.

The concrete LP3 starting state is tested head 56cda6f on foundation merge c054dad,
followed by the S9 acceptance documentation merge. All accepted evidence and original
failures are retained. No live network, deploy, main merge or billing action is
part of this handoff, and no LP4 implementation criterion is marked passed here.
