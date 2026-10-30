# SYS-01 resource revalidation — NOT PASSED

Semantic version: LP4-SYS-01. Measurement model: sys-frame-measurement-v1.
Fixtures are SYNTHETIC, not supported-profile acceptance or proven global bounds.
Full identity is the complete GenesisSpec in vectors, its recomputed genesisHash,
chain, CP and synthetic/simnet fork schedule [(1,0)]; boot=false. Source/artifact
hashes are recorded in the adjacent evidence manifest. This does not replace the
required complete CP/profile/resource-model facts or caller-independent derivation.

For mandatory call i let B=CP.G_SYS and U_0=0. Require U_i<=B, set
A_i=min(30000000,B-U_i), reject A_i=0, and execute the actual frame. On success
set U_(i+1)=checked_add(U_i,raw_spent_i). Reject any error or U_(i+1)>B.
Refund counters do not change A, U, fees or balances. Each frame's raw gas includes
nested EVM/native costs; no intrinsic/user gas is added. This is a checked meter,
not a proof that the prescribed workload completes within B over all valid CP.

For EIP150 after ordinary CALL overhead, memory expansion and value costs, n gas
can forward at most n-floor(n/64). To forward g>0 the minimum such n is
`g+floor((g-1)/63)`; g=0 needs n=0. Proof: write g-1=63q+r,
0<=r<63; n=64q+r+1 forwards exactly g, and n-1 forwards at most g-1.
The caller must additionally retain all costs after the child, opcode overhead,
actual dynamic access/memory costs and every remaining prescribed call. Derive
these from the actual compiled program and available remaining SYS allocation.
No total-of-optimistic-child-gas argument passes general R18.

Fresh epochs remove reuse of warm accounts/slots, original storage and transient
storage. Persisted SSTORE values do survive into the next epoch. Measured compiled
witness examples are two independent cold reads at2323 each, clearSlot raw5158
with4800 refund (no SYS credit), followed by restoreSlot22281 with no refund.
Independent execution records actual opcode/SLOAD/SSTORE traces, raw gas and
full roots for the real compiled Registry/bond bridge, including closing PAY.
Those traces are measurements for these exact states, not worst-case bounds.

Current host implementation owns a block candidate clone and builds each frame
with another database clone plus NativeContext's complete epoch-start PAY clone.
It scans complete actual code before candidate construction and after each call.
This costs O(actual complete state/code) host work and memory; EVM gas alone does
not bound it. Physical code sharing, retained/history trie nodes and these clones
must be included in general R21. No RAM bound or caps64 minimum is established.

Still mandatory and NOT PASSED: validated current UT and stored-parent execution,
five actual complete programs, complete GenesisBuilder, real phase0-3/user atomic
integration and disk/crash/reorg, finalization/SV6, original/amended Cancun controls
on this new source, full CP-domain gross gas/EIP150/work/write/slot/account/log/code
bounds, general R18/R21 and SYS_SLOT_MAX/caps64 minimum. Old unsigned transaction
measurements remain historical and are not acceptance evidence for SYS-01.

Current exact-profile measurements: Registry markClose raw30831/SSTORE1/SLOAD3;
beginBlock raw123767/SSTORE2/SLOAD197 (SCAN64); closing markClose raw60894,
refund4800, SSTORE3/SLOAD11; closing beginBlock raw5813/SSTORE1/SLOAD1. All emit
zero logs. All52 stored ChainCaps values are independently checked against the
full synthetic identity. The initialized record/escrow/aggregate bond now derives
from CP.B_reg. These are NOT GenesisBuilder or supported-profile admission proofs.
