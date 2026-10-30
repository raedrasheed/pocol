# LP4 P1 separate sequential review

Scope: raw/ordered/secure-storage/account MPT root primitives only. No EVM, node
persistence, GenesisBuilder allocation semantics, system execution or boot path.
Single author; this is a distinct review pass, not an external reviewer identity.

Compared canonical hex-prefix flags, odd/even paths, prefix terminal branch values,
root hashing, child encodings strictly shorter than32 bytes, secure hashed keys,
RLP minimal integer values, zero-slot omission and explicit-account encoding. Entry
budget is a caller-supplied allocation guard, not a new consensus cap. Raw API keys
are <=64 bytes; secure addresses/slots and RLP(u64) indices fit. State-account deletion
and EIP161 belong to future execution; this root helper retains explicit accounts.
No R18/R21 bound is inferred from the key-depth or input guard.

Independent Python oracle inserts uncompressed nibble nodes, then compresses paths;
it does not import Rust or any M1 trie implementation. Frozen M1 pure Keccak is
checked against K1–K3; RLP here is independent. The oracle verifies1218 cases with
zero mismatches, including exact31/32 reference boundaries, empty/prefix/64-byte
keys, randomized/permuted trees, zero storage, ordered roots and1024 accounts.
Nineteen successful roots are frozen as a byte-pinned test fixture.

Author tests precede this review. Reproduction runner passed release build, format,
134 portable Rust tests, probe compilation, independent oracle/frozen fixture hash,
and eight isolated source controls. Every accepted control compiled twice and
failed with an explicit `assertion failed`; import/compile errors never pass.
Positive root behavior is exercised on the unmodified library. The maximum prefix
corpus also passes on a512KiB thread stack. This is not a general trie memory proof.

Retained failures/findings: initial oracle iterated non-vector metadata in the
Keccak JSON dictionary and failed before testing roots; corrected to exact K1–K3.
An initial duplicate-guard mutant produced a bounds panic. Its raw result is retained
but excluded from the eight assertion-based controls; replaced with a compilable
budget-guard mutant. No original-code panic or root mismatch was observed.

Immutable M1, LP3 and previous fixtures/dependency locks are checked against041061a.
No new Rust or Python dependencies were installed; pure Python Keccak source/license
provenance is inherited from frozen M1. Cached toolchains and binaries stay outside
the published tree. Source/input/output hashes and exact reproduction commands are
preserved. Current-head GitHub requirements and supported Windows regression must
pass before foundation merge. This local receipt is not a Windows pass.

All 23 LP4 execution/resource obligations remain NOT PASSED. General R18/R21,
caps64 minimum, actual GenesisBuilder/syscode, finalization/cursor/pending[F]/final
subsidy, SV6 and durable execution acceptance require later increments. The pending
owner decision concerns new named acceptance bindings, not permission for P1.
