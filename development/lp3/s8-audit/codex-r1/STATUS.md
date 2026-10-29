# S8 review checkpoint — NOT ACCEPTED

Adds 12,000 seeded strict Rust M3 envelope cases and 12,000 seeded TS Genesis
framing/canonicality cases. Author TS run: eleven groups PASS after correcting a
fixture field lookup and an explicit Uint8Array type. Separate sequential source
review confirms no production code, M1, old evidence, lockfile or boot change.

Integrated review mutation/oracle receipts are being collected. Local full-target
Rust execution reached Windows-only journal operations: those FAILED on Linux with
explicit UnsupportedPlatform (never counted passing). Initial loopback failure was
a sandbox network restriction; granted test-network rerun passed both HTTP tests.
The added Windows CI invokes the existing pinned-toolchain harness to execute real
journal/crash/process regressions on their supported OS. Merge requires its success,
actual artifact inspection, both current-head GitHub checks and remaining review.

S7 merged at 5a04bdf; CF approval is separately recorded. Automatic PR17 review was
still running at its merge and was not counted as a passing review.
