# LP1 progress — Author revision 2 (handoff 0.35)

**Status:** focused correction, not compiled or tested. The author ran no commands; no completion claim.

| Item | Correction |
|---|---|
| LP1-I01 | Removed non-ASCII text from a raw byte string in `src/json.rs` tests. The positive UTF-8 test now uses an ordinary string with `as_bytes()`, and escaped inputs are constructed at runtime. |
| LP1-I02 | Request IDs follow accepted C19 value semantics (binary64, as in JSON.parse): finite integer in 0..2^32-1, with -0 normalized to 0. Added tests for all sixteen original oracle rows, directly and through RPC. |
| README | Question 1 is resolved as baseline conformance, rather than a nonblocking question. No M1 specification change. |

**Awaiting the root:** offline build/test, rustfmt, then A2–A7, E2E script and Codex review. Further compilation errors may appear because the previous compilation stopped at the lexer.
