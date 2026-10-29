# Codex independent review — revision 0.26, author turn 024

Resumed from the saved checkpoint after connectivity returned. The broker confirms the existing author job completed with exit 0 and a successful receipt. No duplicate author job was created.

The preserved-copy runner completed: 377 entries, 302 passed, 75 recorded, zero failures, 25.6 seconds, exit 0. Its consolidated inventory contains 34 CompleteCandidate and seven Partial rows. Existing evidence was preserved rather than rerun or overwritten unnecessarily.

Independent verification completed:

- ASERT: 1027 cases matched a separate native JavaScript BigInt oracle, including negative floor division and saturation. Intermediate widths stayed within 512 bits.
- Hashes: all 347 exported preimages matched independent verification. Keccak was checked with two separate JavaScript libraries in addition to the executed Python reference; SHA-256 was checked with Node's native crypto. Declared address suffixes also matched.
- Signatures: the existing read-only Rust libsecp256k1 0.5.0 / sha3 0.9.1 verifier validated 135 distinct supplied fixture signatures against their expected addresses. No private key was exported, and no transaction was signed or submitted by this verification.
- NX-V3e/f/g: saved actual call counters confirm the future-reference exit before ASERT, the target-ceiling exit before PoW, and the 13-head/4095 message.
- An additional valid header with 256 shares exercised the otherwise-empty share path and passed the RP reference checks.

Two blockers prevent accepting V1:

**C32 — malformed busy replies escape the reference checker.** An error with code -32021 and missing data raises KeyError; null data raises TypeError; an empty data object raises KeyError for retryAfterMs. These must result in a controlled viewIncomplete/no-frame outcome, with validated retry delay and bounded retries. Three independently constructed probes failed. The passing prepared suite does not cover these cases.

**V1-SHA-COST — the literal source bound is not satisfied.** browser.md states at most 13 SHA-256 evaluations. The h=20 transcript actually computes 14 TemplateIDs and 13 PoW hashes, totaling 27. The independent 256-share single-header probe computes one TemplateID, one PoW hash and 256 share hashes, totaling 258. For a maximum window, the structural upper bound is 14 + 13 + 13*256 = 3355 before any implementation-specific reuse. P-V1-3's 27-call explanation omits share verification. A precise, authorized clarification of the cost criterion and corresponding worst-case fixture/accounting is required; this review does not silently reinterpret the source as PoW-only or approve a weaker bound.

The 34 unaffected candidate rows have their consolidated computation/review evidence confirmed at specification scope. Full M1 remains unmet: V1 requires the above changes, and C1–C4/R1/E6 retain their named owner blockers. U01, U02, U10, U14, CR-M1-01, U08 and RF-E6-1 remain unanswered. No owner policy is approved by this report. Deferred production/browser/worker timing outcomes are not fabricated or treated as missing M1 implementation work.

Verdict: **revise** revision 0.26 for V1; acknowledge the unaffected consolidated evidence. The permitted independent review is complete, with incomplete checks and the source conflict explicitly recorded. No new author loop, production implementation, merge or deployment was started. No content restriction blocked a review step in this resumed turn.
