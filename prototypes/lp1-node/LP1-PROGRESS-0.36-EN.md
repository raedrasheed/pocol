# LP1 progress — Author revision 3 (handoff 0.36)

**Status:** focused correction, not compiled or tested by the author; no completion claim.

| Item | Correction |
|---|---|
| LP1-I04 | Depth 16 is now a hard ceiling in `rlp::decode_with_depth`. A higher requested limit is explicitly rejected with `decode.depthLimit` before reading input, keeping recursion within 17 frames. Removed the stack-overflowing test and added tests for the default limit, above-ceiling requests on 2000 nested lists, and limits 0..16. Stack size was not increased and no test was skipped. |
| LP1-I05 | The adversarial genesis corpus (5000 deterministic cases) separates byte-identical mutations: these must return the same identity and hash; their count is pinned at 2 according to independent replay. Actually changed inputs still must differ from the frozen hash. No change to genesis hashing. |
| README | Two new sections on depth and the adversarial corpus; A1 and A2 criteria updated. |

**Awaiting the root:** full offline cargo test with captured exit code, rustfmt, oracles and real HTTP checks. M1 0.32 remains unchanged.
