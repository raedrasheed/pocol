# R4 measured resources — general R18/R21 and caps64 NOT PASSED

These are synthetic-profile measurements, not proven bounds for admitted CPs.
Full CP/profile identities, code, initial states and actual post-states appear in
the hash-bound complete vector file. The independent result records opcode and
actual address/key SLOAD/SSTORE traces, exact gas and complete state roots.

M0=4 constructs75 nonzero storage slots; M0=1024 constructs5175. Both use8787
logical code bytes, counting each account separately. These counts are complete
for these fixtures, not a prediction for the final five system programs.

Each normal closeBlock performs two SSTOREs (amount/meta); winner stays zero and
is not written. The independent engine observes four SLOAD instructions across
Escrow and ChainCaps. Fallback raw closeBlock gas52696; signed52711. The measured
normal phase0+phase2 raw totals are224958/224973. USER gas43452 yields actual
tips130356 at effectiveTip3; the native credit80000 adds no EVM gas or receipt.
These numbers are measurements only. Refund counters never extend G_SYS.

The host credit dirties Escrow's account balance. Repeated full code/account/slot/
supply scans, state clones, USER candidate, fresh PAY epoch, genesis construction
and complete trie hashing consume CPU/RAM even though credit has no EVM gas.
Logical CODE_CAP does not bound orphaned physical code or histories. None may be
omitted from R21 or converted into a caller-supplied success flag.

R18 must include EIP150 continuation/allocation headroom and EIP2200 SSTORE sentry,
not just the sum of observed raw gas. For every real supported program/path it
must prove a_i<=30M and G_SYS>=max_i(prefixRaw_i+a_i), with checked arithmetic,
alongside every dirty-account/key/log/storage/account/code/body cap. Post-state
checks tested here do not prove immediate cap enforcement at every operation.

Real full-five-program measurement, general derivations for every supported CP,
caps64 minimum SYS_SLOT_MAX, complete physical R21, real settlement/finality,
validated phases and durable rollback remain mandatory NOT PASSED. No cap value
or acceptance criterion changes; boot=false.
