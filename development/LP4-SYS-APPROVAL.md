# LP4-SYS-01 — approved additive system execution contract

The owner explicitly approved the complete option 1 at:
https://github.com/raedrasheed/Proof-of-Collaboration/blob/b1f25e0613fbeb3832dfb20252f76b92ef72b90d/development/lp4/p4-genesis/codex-r1/SYS-EXECUTION-DECISION.md

Recorded approval (English translation of the original Arabic):

> I approve option one in full in SYS-EXECUTION-DECISION.md at commit `b1f25e0613fbeb3832dfb20252f76b92ef72b90d` as the explicit LP4-SYS-01 addition. Record approval while preserving M1 and previous evidence, then implement the complete contract and specified acceptance tests. In particular, verify that the new ORIGIN semantics are confined to SYSTEM, permissions rely on CALLER, proposer data is correct, gas is counted before refunds, and the whole block rolls back on error. Rederive affected resource measurements and continue independent work automatically. This approves the design, not P2, P3 or LP4 acceptance; boot remains disabled.

All seven clauses and every acceptance requirement in that immutable option are
binding. This is an additive LP4 decision, not recovered original M1 semantics.
The historical proposal, diagnostics, M1 and LP3 evidence remain unchanged.

- Mandatory messages have CALLER=SYSTEM, value/price zero and no transaction
  intrinsic gas, nonce, fee, burn or USER receipt. USER execution is unchanged.
- SYSTEM-only ORIGIN is the validated signed UT proposer (nonzero), or zero for
  fallback. Authorization depends on CALLER, never ORIGIN. Exact tips calldata
  remains 36 bytes. Pending base is keccak256(be256(j)||be256(3)); amount word0,
  kind (0 signed/1 fallback) bits0..7 and proposer bits8..167 in word1, winner
  low160 in word2, initially zero. All remaining bits are zero.
- Every call uses a fresh epoch over the actual current private candidate,
  resetting warmth, original storage, transient storage and refunds. Warm SYSTEM,
  nonzero ORIGIN, callee, Escrow and standard Cancun precompiles; no warm slots.
  PAY takes the complete epoch-start snapshot and retains its approved rules.
- Allocate min(30000000, remaining CP.G_SYS); zero is invalid. Debit checked raw
  gas before refunds including nested/native execution. No refund credit/fees.
  EIP150 applies; block GASLIMIT remains CP.GAS_LIMIT and context stays validated.
- System effects enter world state, not USER gas/tips/receipts. The five actual v1
  programs must emit no logs; logs cannot be silently discarded or exempted.
- Enforce prescribed phases and stored parent/grandparent derivation (h1 empty).
  Any mandatory error/cap/context/budget failure rejects the entire block including
  earlier USER/system effects. Epoch cleanup and enclosing rollback are mandatory.
  Actual disk atomicity/crash/reorg remains a separate required execution gate.
- All CP values and numeric caps remain unchanged, including Registry 10/2/10,
  CODE-01 runtime limits and independent initcode49152. Bind affected measurements
  to full profile/CP, sources/artifacts, SYS-01 and resource-model version.

Actual compiled programs, GenesisBuilder, pending encoding, independent complete
roots/gas/state, metadata/authorization negatives, epoch/nested/late rollback,
30M/G_SYS boundaries, EIP150, successful-build semantic mutations, all original
20661 controls and amended replacements, full phases/finality/SV6, general
R18/R21/caps64 and import/crash/reorg remain mandatory. No unrun, blocked,
synthetic or diagnostic case passes acceptance. All full gates remain NOT PASSED.
Boot remains disabled. No main merge, activation, deployment or billing change.
