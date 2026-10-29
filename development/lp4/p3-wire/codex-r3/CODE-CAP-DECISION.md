# Consolidated code-limit semantics decision — NOT APPROVED

No M1 text, CP value, approved Registry limit, runtime limit or acceptance criterion
has been changed. Boot remains disabled. This proposal concerns new explicit LP4
consensus semantics, not a routine library configuration or a missing test label.

## Evidence and ambiguity

Preserved M1 governance.md:13 gives B_code_max=32768 and CODE_CAP=2^30;
governance.md:84 R9 requires B_code_max>=24577. execution.md:81 says both caps
make code deposit fail. execution.md requires Cancun plus declared deviations.
M1 storage.md:16–21 gives ChunkFactory data<=24575, runtime0x00||data<=24576.
None of the searched preserved M1 reference/draft sources explicitly says whether
B_code_max replaces EIP170's24576 limit or is an additional bound, or whether a
changed runtime limit changes EIP3860's49152 initcode limit. Nor do those passages
specify whether CODE_CAP charges repeated code per account or only unique code blobs.
Both accounting choices can produce different accepted transactions/state roots.
The approved PAY/CREATE/Registry amendments do not answer these questions.

The current engine retains standard Cancun limits. Reproducible diagnostic:
`cargo +1.99.1 run --offline --locked --manifest-path prototypes/lp4-execution/Cargo.toml --example code_caps_decision`.
See code-caps-diagnostic.json; all rows explicitly have acceptedRule=false.
Each runtime constructor is five bytes RETURNing zero-filled runtime, with8M gas.
No alternative is installed in native execution; the override is diagnostic-only.

| Case | Current native, CP B_code_max32768 | Diagnostic revm limit_contract_code_size32768 |
|---|---|---|
| runtime24576 | success, gas4971731 | success |
| runtime24577 | CreateContractSizeLimit halt | success, gas4971949 |
| runtime32768 | CreateContractSizeLimit halt | success, gas6611795 |
| runtime32769 | halt | halt |
| initcode49152 | success | success |
| initcode49153 | CreateInitCodeSizeLimit | success |
| initcode65536 | rejected | success |
| initcode65537 | rejected | rejected |

revm19.7.0's pinned primitives/interpreter derive max initcode as2*configured
code limit. Simply assigning the CP field therefore also changes EIP3860 to65536
at the default CP. That second consensus change cannot be inferred or installed.
A CP schema/hash identity does not authorize either interpretation or boot.

## Option1 — proposed explicit additive LP4 contract

1. For supported LP4 profiles, replace the EIP170 per-account deposited runtime
   size ceiling with the exact CP.B_code_max value. Equality succeeds if every
   other requirement/gas condition succeeds; one byte above fails code deposit.
   This includes top-level CREATE and nested CREATE/CREATE2. No runtime code bytes
   or new account state may remain after a failed deposit; standard gas/creator
   nonce/enclosing rollback behavior remains Cancun.
2. Preserve the standard EIP3860 maximum initcode length49152 independently of
   B_code_max, for transactions and both CREATE opcodes. Preserve its word gas,
   error/exceptional-halt behavior and standard evaluation order. Do not let the
   revm configuration coupling raise or lower this limit. EIP3541 and every other
   non-amended Cancun rule remain unchanged.
3. Define CODE_CAP as the logical sum of runtime-code lengths over all existing
   accounts in the current complete state, including the five system accounts
   and user accounts; native PAY contributes zero. Charge identical code at two
   addresses twice, even if physical storage shares a code hash. Zero/absent code
   contributes zero. Physical deduplication belongs to the separate R21 model.
   Use actual source-bound code bytes/state, not caller success/count flags.
4. GenesisBuilder enforces both code limits over the actual complete allocation,
   without changing GenesisSpec syntax/error ordering or initializing accounts
   implicitly. Every deposit checks the proposed resulting logical total; equality
   is permitted and CODE_CAP+1 fails deposit. Actual Cancun deletions release the
   corresponding logical charge; ordinary post-Cancun SELFDESTRUCT that retains
   code releases none. Reverts restore code, accounts and accounting together.
   Missing code bytes, mismatched code hash, incomplete state or overflow cannot
   be accepted as a valid numeric resource fact.
5. Preserve all CP values, R9, gas/transaction/system/write/slot/account/log caps,
   PAY/CREATE/Registry approvals, M1/LP3 and prior evidence unchanged. This is a new
   LP4 native execution rule and must be linked wherever its semantics are used.
   No main/deploy/production activation/billing change or boot authorization.

## Option2 — explicit alternative

Keep EIP17024576 and EIP386049152 unchanged; B_code_max is an additional per-account
code bound, never permission to exceed EIP170 on creation. Keep the same explicit
logical per-account CODE_CAP accounting/genesis/rollback clauses3–5 above. Report
runtime creation's effective limit=min(24576,B_code_max); the larger CP equality
case must not be falsely reported as a successful deposit. Test CP/genesis bounds
separately. This interpretation also needs explicit approval; it is not assumed.
The owner may instead provide a complete different accounting/limit contract.

## Acceptance after the decision (no case is PASS now)

- Source-bound independently generated allocation/code inventories and profile IDs;
  runtime limits equality/+1, multiple CP values, available gas equality/minus1,
  code0/0xef, code-hash sharing at multiple addresses and logical total cap±1.
- Actual signed top-level creation and compiled nested CREATE/CREATE2; initcode
  49151/49152/49153 with valid minimum/default CP code ceilings; below-R9
  profile settings remain invalid and must not be treated as supported profiles;
  preserve error order, nonce/gas effects and nested rollback.
- Actual GenesisBuilder positive/negative code-bound/missing-code/hash/overflow
  cases, system code and native PAY treatment. Do not pass GenesisBuilder from
  a root primitive or opaque fixture.
- Full original20661 upstream Cancun controls remain mandatory and unmodified.
  Enumerate every changed PoCol code-size case individually, with its actual
  replacement execution proof. No blanket exception, exclusion or fake baseline PASS.
- Independent execution/root/balance/code/gas comparison, compiled cap/initcode/
  accounting/rollback mutations, state import and crash/reorg proof; bind the
  selected semantic version and all affected general R18/R21/caps64 resource facts.
  T2c/full integration criteria remain mandatory, never weakened.

Decision needed before implementing the selected limit/accounting semantics or
claiming P2/P3 acceptance. Unaffected work can continue: signed/RPC/profile controls,
PAY/Registry/forced-transfer tests, source/license audits and evidence preservation.
