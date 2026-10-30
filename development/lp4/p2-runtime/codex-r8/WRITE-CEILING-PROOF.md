# Source-bound successful-user-write upper bounds

Authority: explicit LP4-REGISTRY-02, not original M1 ceiling acceptance. Applies to
RegistryPayment.sol's actual MinerRegistry + RewardEscrow + ChainCaps artifacts and
approved native PAY-01/02/CREATE-01. Hashes in source-sha256.json bind exact bytes;
compiler artifact binds solc0.8.28/Cancun/optimizer200/noCBOR/nohash and ABI/layout.
Any code/model change requires re-verification. No caller-provided resource PASS flag.

Assumptions: closed source-built system code at prescribed addresses, valid canonical
state/layout and validated CP widths/relations, correct host phases, native PAY no
callback/no account creation and journal rollback. Synthetic fixture measurements
alone do not establish those whole-profile/host/resource assumptions. No full LP4
or R18/R21/caps64 acceptance follows from this component proof.

Counting SSTORE instructions is stronger than counting distinct changed storage keys:
zero/same-value stores still count. No user-path loop; append/removal touches at most
one moved element. View calls to Caps/Registry have no SSTORE. Authenticated Escrow
bridge has no SSTORE; PAY transfers balances, never enters arbitrary recipient code.
No hidden Solidity state variables (compiler storageLayout.storage is empty); all
writes appear as the explicit put/SSTORE statements in the source.

| Successful user path | Maximum SSTORE decomposition | Bound |
|---|---|---:|
| register | Rec.w0..w3:4; RecArr/ActList append:2; RecLen/ActLen:2; heldTotal:1; shared counters:1 | 10 |
| setRewardAddress | reward word:1; counters:1 (even no-op same recipient) | 2 |
| claimBond middle | Escrow PAY:0 storage; heldTotal:1; clear4 record words; replace/clear2 RecArr slots; moved record position:1; RecLen:1; counters:1 | 10 |
| claimBond tail/single | heldTotal1, record4, RecArr tail1, RecLen1, counters1 | 8 |
| deregister/removeInactive middle | ActList replace/tail2; moved record position1; ActLen1; own status/position word1; own clock word1; counters1 | 7 |
| deregister/removeInactive tail | ActList tail1; ActLen1; own word1; clock1; counters1 | 5 |
| reactivate | ActList append1; ActLen1; own status/position1; clock1; counters1 | 5 |

The moved member differs from the removed member on a middle removal; own word is
written once (removeActive returns the value instead of storing an intermediate).
Escrow reads the actual HELD/non-tomb Registry amount and recipient; no arbitrary-debit
parameters or authority flag. All payments/status/deletion/heldTotal/counters revert
in one EVM journal on failure. Mandatory multi-call block rollback/crash/reorg remains
an unverified host obligation; ordinary EVM nonce charging is not claimed block rollback.

Branch choices, any admitted CP value and arbitrary existing array lengths cannot
increase these finite user-path counts. Failed calls cannot commit partial writes.
The boundary/negative tests and independent SSTORE traces exercise these source paths,
including actual10/2/10 attainment on positive-bond/middle cases and unchanged7/5 ceilings.
Compiled controls detect budget exemption, extra write, boundary/status/route/cursor
faults. See independent results for observed gas/storage/access lists, not a generalized
gas upper bound. Synthetic CP52 snapshots are fingerprintable execution inputs, not
admitted resource facts, measurements of all supported profiles or proofs of R16/LAG.

Only the three owner-approved ceilings changed. Original9/1/9 remain not satisfied;
all other caps/formulas are unchanged. Full-profile R18/SYS accesses/gas, general R21,
caps64, GenesisBuilder and all execution obligations remain NOT PASSED before merge.
