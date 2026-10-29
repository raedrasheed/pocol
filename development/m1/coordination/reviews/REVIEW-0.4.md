# Codex independent review of author turn 002 / 0.4

Receipt: claude-002.jsonl final result; exit 0, no permission denials. All 15 project files new under 0.4. Claude also reported an automatic memory note outside the project; its contents were not independently reviewed. Review decision: revisions required, not M1 acceptance.

Run in preserved copy: 218 entries, 216 pass, 1 recorded, 1 FAIL, 119.4 seconds. Failure: bound.length rb-2848-does-not-fit, actual length 65561. Do not just mark the expected value passing: explain the length-of-length transitions independently. See rerun-0.4.txt and copy results.

C10 (snapshot model invariant handling) and C11 (phase gate wording) remedies accepted at specification level. C12's structural request bound appears sound but its boundary fixture fails; leave its evidence pending C14 below. No production test has run.

Independent findings (independent-probes-0.4.json; native Node URL oracle):

- C14: invalid expected length in the 2848-reference fixture. At the 65536 length boundary, nested RLP headers grow; actual 65561 exceeds the maximum.
- C15: Python httpsUrl approximation disagrees with WHATWG in 6 of 9 targeted cases. It accepts malformed IPv6, invalid IPv4 and invalid percent-encoded hosts, and rejects valid https://[::1]:. Native Node WHATWG URL is available for a reference oracle now; do not defer schema verification entirely to a production browser experiment.
- C16: Pending.req identity is unspecified. route('sessionA',0), route('sessionB',0), then final_reply(0) decrements B rather than A. If req is an internal opaque handle (not the frame id), state and enforce that requirement and test colliding/repeated frame ids. Do not introduce baseline-unspecified rejection of repeated frame ids silently.
- C17: CR-M1-01 shape/binding has gaps. EIP-1186's required account fields do not include address; make address optional with a match check when present, and always bind proof to requested W. State trie path uses keccak(address20), storage uses keccak(slot32); PR-1 currently describes both as hashing a 32-byte input. Verified account absence must have an explicit branch before comparing a present-account leaf; allow an empty proof only when authenticated empty-root absence is established. The shape's accountProof min=1 and binding's required present leaf otherwise make the claimed noWebsite branch unreachable for valid absence.
- C18: Proof RPC errors cannot all trigger immediate re-anchor: baseline -32021 requires honoring retryAfterMs. Specify bounded delay/restart accounting and strict JSON-RPC envelope/id handling. Keep abstract mock evidence separate from real MPT verification.

Primary sources used for C17: https://eips.ethereum.org/EIPS/eip-1186 (parameters, account/storage fields, absence proof rationale); https://ethereum.org/developers/docs/data-structures-and-encoding/patricia-merkle-trie/ (fixed-length secure-trie keys). These do not establish pocold support.

U34 is not an owner-level discrepancy: BG1 supplies a conservative minimum 61440+256+128=61824, not an assertion that the tight compact-JSON envelope equals 128. A 34-byte margin over a measured 61790 compact message is not a contradiction. Escaping can exceed the outer size guard; document guard precedence rather than change the baseline limit.

Hash evidence updated independently: Rust sha3 0.9.1, @noble/hashes 1.7.1 and pycryptodome 3.24.0 agree on 611 samples, including all 486 positive-file declared content hashes. Initial linker lookup and JS TSV CRLF issues corrected in coordinator tooling; failed receipts retained where present. See hash-triad/*expanded-result.json. No claim that all M0 tasks are complete.

0.3's 18 source/fixture/document files match the saved review copy; protected-0.3-after-002.json. Original baseline/0.1/0.2 protection remains tracked separately. Owner choices/addenda remain unapproved. Missing 28 annex rows are work remaining, not blockers by themselves.
