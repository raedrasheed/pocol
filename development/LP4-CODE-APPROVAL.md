# LP4-CODE-01 — approved additive code-limit contract

The owner explicitly approved option 1 in the immutable proposal:
[CODE-CAP-DECISION.md at 3bdbd149](https://github.com/raedrasheed/Proof-of-Collaboration/blob/3bdbd149ccabfe21ef6cdd7fbaaa2afdabb1f59b/development/lp4/p3-wire/codex-r3/CODE-CAP-DECISION.md).
Recorded answer: «اعتماد الخيار الأول كاملًا (الموصى به)».
This records a new additive LP4 consensus rule, not an original M1 definition.
The historical proposal and all M1/LP3 evidence remain unchanged.

All five clauses and every acceptance requirement of option 1 are binding:

- Deposited runtime limit is exactly CP.B_code_max, including signed creation,
  CREATE and CREATE2. Equality is allowed; exceeding it fails deposit with Cancun
  gas, nonce, checkpoint and enclosing rollback semantics.
- EIP3860 initcode remains independently limited to 49152 bytes, with its existing
  word gas, transaction/opcode errors and evaluation order. EIP3541 is unchanged.
- CODE_CAP charges actual runtime bytes per existing account, including system
  accounts and duplicate code at different addresses. Native PAY contributes zero.
  Physical deduplication is a separate R21 obligation.
- Actual GenesisBuilder and each deposit enforce both caps from complete state
  and code bytes. Actual deletion releases charge; retained Cancun code does not.
  Missing bytes, hash mismatch, incomplete facts and overflow cannot pass.
  State, code and accounting must revert together.
- CP values, R9, every other ceiling and the PAY/CREATE/Registry decisions are
  unchanged. No approval of results or authorization to boot is implied.

Mandatory evidence includes independent inventories and identities, multiple valid
CP values and R9 rejection, signed and compiled nested creation boundaries,
49151/49152/49153 initcode, gas boundaries, EIP3541, logical sum/deduplication,
actual GenesisBuilder, deletion/rollback, all 20661 original Cancun controls plus
individual changed-case replacement proofs, independent roots/balances/code/gas,
compiled mutation controls, import/crash/reorg and updated general R18/R21/caps64
facts bound to the full profile and semantic/model version. No narrower primitive
test passes T2c or full integration. Every unverified obligation remains NOT PASSED.

Boot stays disabled. No main merge, production activation, deployment or billing.
