# LP4-ISSUANCE-01 — approved additive native subsidy contract

The owner explicitly approved option1 in full: «اعتماد الخيار الأول كاملًا».
Immutable proposal:
https://github.com/raedrasheed/Proof-of-Collaboration/blob/a67eb9d7ce39e7fdb7c0a53e3dee85de73de61e8/development/lp4/p4-genesis/codex-r3/ISSUANCE-DECISION.md

All eight clauses and every listed acceptance requirement are binding. This is a
new explicit LP4 consensus addition; M1, LP3, the historical proposal and all
previous approvals/evidence remain unchanged. No old decision is reopened.

- Credit exactly CP.subsidy ONCE in phase2 after authenticated USER execution/tips,
  immediately before closeBlock, for signed and fallback nonfinal blocks alike.
  No funding before/during USER or phase0. At finalF credit ZERO and finalize(tips).
- Restricted host transition only, not PAY/CALL/value/transaction/precompile or
  SYSTEM debit. No account creation, nonce, callback, log/USER receipt/tip or
  additional system call/calldata; exact36-byte tips ABI and SYS-01 stay intact.
- Require actual validated complete GenesisBuilder/system manifest and existing
  Escrow/code/profile identity. Checked u256 balance/supply/R23 and every original
  account/code/dirty/resource cap; missing/corrupt/overflow facts reject the block.
- Only validated executor phase2 with single-use state, no caller flag, USER/RPC/
  EVM authorization or replay. Wrong final/nonfinal context or duplicate rejects.
- SYS charges raw actual frame gas before refunds. No EVM gas for the host credit;
  host CPU/memory/dirty-account work MUST enter full R18/R21 and existing caps,
  bound to complete CP/profile/phase/semantics/source/artifact/resource model.
- closeBlock/PAY epoch sees actual candidate AFTER funding. Pot is checked subsidy
  plus authenticated tips, never balance delta/SELFBALANCE/transient/success flags.
  Bond/pending/burned/surplus liabilities stay distinct.
- Issuance, all USER/SYS/pending/resource/supply effects share one enclosing block
  candidate. Any late error/cap/commitment/persistence failure rejects/restores all.
  Actual disk/crash/reorg and inverse state are separately mandatory; RAM clones
  alone do not pass. No canonical intermediate credit.

All immutable acceptance cases, independent real full-state/gas comparisons,
successful-build mutations, actual five programs/GenesisBuilder/phase0-3, signed/
fallback/F0/pending[F] absence/finalization/SV6, supply/basefee/forced-surplus/bond
invariants, general R18/R21/caps64 and20661 original/amended controls remain
mandatory and NOT PASSED until verified. Both old timing countermodels remain
historical diagnostics. Design approval accepts no result or increment.
Boot stays disabled; no main merge, deployment, activation or billing change.
