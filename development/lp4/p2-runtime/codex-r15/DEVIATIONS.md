# Execution adaptations and remaining replacement evidence

Authority: unchanged [M1 execution](../../../m1/reference/execution.md), the
accepted LP4 milestone and the separately recorded owner amendments below. This is
a traceability inventory, not new consensus rules or permission to exclude tests.
All 20,661 pinned Cancun variants remain mandatory controls. The upstream runner
uses Ethereum chain/context/transaction types; it does not stand in for PoCol apply.

| Area | Approved authority and implemented control | Remaining mandatory evidence |
| --- | --- | --- |
| Native PAY | LP4-PAY-APPROVAL.md and LP4-PAY-LIFECYCLE-APPROVAL.md; actual Registry/Escrow identity, exact64 input,5000 native gas, source debit, no callback/account creation, original recipient durability, nested rollback; existing44 boundary+2 lifetime and compiled controls | Full source-built systems, GenesisBuilder eligibility, SV5, actual settlement/finalization and profile-bound resources |
| CREATE storage collision | LP4-COLLISION-APPROVAL.md; additive EIP-7610 in actual native and upstream control paths, complete original/journal storage and rollback | Persisted backing-state integration and bounded resource derivation; no hidden-slot assumption for a future database |
| Registry transitions/write budgets | LP4-REGISTRY-APPROVAL.md and LP4-REGISTRY-WRITE-APPROVAL.md; actual user/bond bridge, six-call budgets, sweep and closing cursor; only LP4 register10/reward2/claim10 amendment | Complete system contracts/phase driver and all admitted CP/profile bounds; original9/1/9 remain unpassed |
| CHAINID and transaction admission | M1:777902; current native config and wrong-chain controls | P3 actual signed legacy155/2930/1559, pool−32015/block-rule17; reject unprotected legacy and types3/4. GST's unsigned chain1 adapter proves none of these |
| NUMBER/TIMESTAMP/COINBASE/GASLIMIT/BASEFEE | M1 execution fields and London fee rules; explicit immutable native execution context, fixed Escrow coinbase; R15 actual opcode/output/gas/root comparisons and gas/basefee equality/negative controls | P3/P5 header-bound gas/basefee validation. Historical wrappers retain their30M/zero-fee **synthetic control** context, not accepted network parameters |
| PREVRANDAO | M1 SHA256('PoColRandao' concatenated with parent seal); R15 executes explicitly supplied value | P5 derive from validated stored parent seal and prove wrong-seal/current-result leak controls. Supplying a synthetic value is not this derivation |
| BLOCKHASH | M1 exact256-block window including genesis; R15 checks(10,0),(257,0),(257,1),(300,44),(300,43),current,future,height0 with an independently ordered ancestor engine | P5/P9 complete validated HeaderStore ancestry and reorg provenance. An absent cache entry returning zero is not evidence of complete ancestry |
| BLOBHASH/BLOBBASEFEE, no EIP-4788 | M1 zero/one/no beacon system call; R15 ordinary nonblob transaction reads0/1; no implicit beacon call in this transaction entry point | P3 reject actual signed type3/4 and P5 prove full phase list. Current low-level TxEnv is not signed-wire admission and must not be described as universally enforcing BLOBHASH0 |
| Final tips burn and forced-transfer locking | M1 declared deviations, unchanged | P6 actual finalization, all reward/subsidy/bond pots, pending[F] absence, zero final subsidy, cursor mutation and SV6. NOT PASSED |
| Execution/resource caps | M1 cap/failure classes; LP3 carried generalR18/R21 and caps64 minimum obligations | P4–P9 actual systems, complete CP/profile/model derivations, cap equality/minus1/overrun and compiled controls. No synthetic fixture is an admitted resource fact |
| Parent result/system ordering | M1 D141/D142 and approved T24 mapping | P5/P6 stored parent/grandparent derivation, h1empty, mismatch stop, atomic whole-block phases and final child rejection. NOT PASSED |

Ethereum validation repairs (nonce maximum and unsigned comparison-adapter admission,
MODEXP zero edge, optimized Blake2) are separately identified in R12/R14. They are
not blanket PoCol exclusions or authority to invent transaction rules. Engine expected
rejection presence alone is weaker than complete reason/order/wire equivalence:
1,172 primary rejection categories have independent numeric checks, while one bigint
fixture is an adapter overflow control. Keep those scopes explicit.

All26 acceptance-ledger obligations, P2 and LP4 remain NOT PASSED; boot remains false.
The external Geth CLI is test-only, GPL3+/LGPL3+ as recorded in R14, never PoCol runtime
code or a distributed binary. It supplies independent actual execution, not activation.
