# Registry payment layout v1 (LP4 engineering implementation)

Authority: LP4-PAY-APPROVAL.md, LP4-PAY-LIFECYCLE-APPROVAL.md,
LP4-REGISTRY-APPROVAL.md and LP4-REGISTRY-WRITE-APPROVAL.md in development/.
New implementation encoding, not recovered M1 bytecode. M1 field locations unchanged.
No complete GenesisBuilder/apply/settlement/finalization acceptance is implied.

MinerRegistry: slot0 RecLen,1 ActLen,2 sweepCur,3 packed counters/phase metadata,
4 heldTotal; RecArr at Keccak(be256(5))+i; ActList at Keccak(be256(6))+i;
Rec[id] four words at Keccak(pad32(id)||be256(7))+k.

| Word | Bit ranges, low first |
|---|---|
| w0 | status0..7 (none0/active1/exited2), tomb8..15, bondState16..23 (HELD1/BURNED2/PAID3), recPos24..55, actPos56..87, lastShare88..151; remaining zero/reserved |
| w1 | reward address0..159; remaining zero |
| w2 | bond u256 |
| w3 | exitHeight0..63, dueHeight64..127, burnHeight128..191, graceStart192..255 |
| slot3 | userOps0..15, shared register/reactivate16..23, lastBegin24..87, beginInitialized88, Closing89, closeCursor90..121, closeInitialized122, lastMark123..186, closeH187..250; remaining zero/reserved |

Both counter fields update in one SSTORE. Phase/cursor metadata also shares slot3;
there is no hidden persistent counter, unjournaled success flag or extra user write.
markClose runs once per height, including pre-closing; first Closing mark initializes
and processes up to RET immediately (M1 cursorDone bound starts at closeH). beginBlock
requires that height's mark and runs once, resetting budgets then ordinary sweep.
Full host phase ordering/invalid-block atomicity remains a separate apply obligation.

ChainCaps: complete CP52 fields at slots0..51 in accepted schema order, immutable
through this runtime ABI; real GenesisBuilder must validate and populate them.
No constructor/root/ParamGate/resource/admission proof is implied by installing
synthetic test storage. RewardEscrow releaseBond has no storage writes; it authenticates
Registry, reads the actual HELD non-tomb record and calls native PAY from Escrow.
Pending pots, burns, settlement/finalization are not implemented by this bond module.

IRegistryUser is the canonical user ABI. Runtime fallback validates exact selector,
length, upper address padding and BadCalldata→BadValue→Closing→RegOpsLimit prefix.
All six ABI entries are payable so custom BadValue is returned by the strict dispatcher.
Other semantic errors follow the prefix. SYSTEM-only markClose/beginBlock are distinct
from Registry-only Escrow.releaseBond; no generic arbitrary-debit forwarder is exposed.

Bounds for successful user execution (SSTORE instructions, including unchanged writes):
register10; reward2; claim middle10/tail8; deactivate middle7/tail5; reactivate5.
No loop in user paths; swap removal touches at most one moved record. Only constant
views, authenticated bridge and native PAY are called; recipient code is never called.
These structural write ceilings do not prove SYS gas/access, R18/R21, caps64, R16/LAG,
GenesisBuilder, SV5/SV6, finalization, storage durability or an admitted profile.
Re-measure all of those with complete CP/profile/model identities before merge.
