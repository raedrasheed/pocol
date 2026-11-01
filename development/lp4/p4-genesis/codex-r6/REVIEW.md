# Separate D27 component review

Checked the literal initcode prefix against M1 D27 (10-byte constructor followed
by00||data), independently computed CREATE2 preimages and actual deployed code.
No data is run as constructor instructions. Length checking precedes uint16
conversion, and repeat calls verify the exact existing runtime hash. A failed
CREATE2 or mismatched runtime cannot return a successful new deployment.

Independent execution exposed a harness root mismatch: direct InMemoryDB.commit
retained an empty, zero-fee COINBASE account. The component test now uses the
existing canonical epoch commit helper, which removes touched empty accounts.
The old failing vectors and comparison log remain preserved; this was a test
commit-path fix, not an engine/consensus rule change. Complete public apply and
durable commit acceptance are still absent and remain NOT PASSED.

Reviewed D27's24575 data limit independently of approved CODE-01's larger runtime
cap. Added a profile-bound negative execution that allows runtime32768, so an
incorrect data24576 contract cannot hide behind EIP170. The first mutation run
really compiled and failed this assertion but its log classifier expected the
literal word assertion. Preserved that non-passing runner result; added the word
to the diagnostic message without changing the test predicate, then reran all
eight compiled mutations into a fresh evidence directory. No assertion was
removed or weakened and no build error was classified as a kill.

No SSTORE is allowed in this program: real opcode counters as well as changed-
storage assertions cover the component executions. Creation still changes the
factory nonce, creates an account and stores code. Those host/disk costs are
mandatory resource obligations, not zero merely because SSTORE is absent.
