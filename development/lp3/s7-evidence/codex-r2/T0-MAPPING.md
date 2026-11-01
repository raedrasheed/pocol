# T0 semantic core evidence and exclusions

M1 validation describes V1–V26 behaviors but supplies no literal V1–V26 wire corpus.
The new tests bind those **semantic core cases** to generated inputs and preserved
public RP headers; they are not claimed byte-identical to missing literal vectors.

- L0: public canonical headers; trailing bytes, nonminimal/leading-zero height,
  missing UT element, zero target, 7-byte nonce, 19-byte proposer, high-s/out-of-range
  signatures and v=27 in both signatures, duplicate/unsorted/winner shares.
- L1: netChain, netGenesis, unsupported/inactive fork; signed-attempt/missing
  proposer/missing signature/wrong recovery. Combined bad chain plus Rule 3 yields
  ordered {netChain,3}, first netChain. Successful shape checks explicitly report
  parent/state/execution rules not evaluable.
- L2: recompute actual TemplateID/PoW for all 20 public fixture headers and check
  nonce<2^28. A synthetic supported-version=1 envelope has exactly 3067 bytes,
  256 explicit shares, canonical signature *shapes*, target MAX and nonce=0, whose
  PoW meets the target. Its template authentication/parent/state validity is not
  claimed. It is not called literal V6 or an accepted real block.
- L3: CF6 verifies the pure no-child-of-final and H_END predicates. Actual V20–V22
  finalization/state execution remains a mandatory LP4 obligation in the separate
  owner approval; no supplied success boolean or synthetic full-valid certificate.

The old RP codec and frozen RP rule priority are preserved. `envelope::decode` is
an additive M3 stateless entry point which adds consensus Rule 1 signature-shape
and explicit-share canonicality checks. `diagnose` evaluates only Rule 1, network
identity and signed-template Rule 3. It does not evaluate Draw, state assignment,
winner ownership, parent-dependent validity, or H-full.

An initial maximum-shape test used unsupported version 65535 and produced 3069
bytes; that failed test is retained. The corrected supported-version=1 fixture
produces 3067, without weakening the fixed header bound or old decoder.
