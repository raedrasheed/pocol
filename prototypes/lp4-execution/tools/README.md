# Locked Solidity compiler tools

The compiler stays at **solc 0.8.28**. Its `tmp` dependency is overridden to
**0.2.7** to address GHSA-52f5-9888-hmc6 and GHSA-ph9p-34f9-6g65, including
the later non-string option hardening. Keep the override until a deliberately
validated compiler update makes it unnecessary.

Use npm **8.3 or later** so overrides are honored. The updated dependency needs
Node **14.14 or later**; the regression runner below needs Node **18 or later**.
Validation was performed on Node 24.22.1 / npm 11.9.0 under Linux.

From the repository root:

```bash
npm ci --prefix prototypes/lp4-execution/tools --ignore-scripts --no-audit --no-fund
node --test --test-reporter=tap prototypes/lp4-execution/tools/test_tmp_security.cjs
npm audit --prefix prototypes/lp4-execution/tools --package-lock-only --ignore-scripts
```

The tests resolve `tmp` from solc's own module context. They use only newly
created test files and a local SMT solver fixture, not a real solver or network
service. The solver fixture test is POSIX-specific and skips on Windows.

Compiler upgrades require fixture comparison: changes to emitted contracts can
affect the accepted system programs. See the [security follow-up](../../../development/security-dependency-followup-2026-11-05/REPORT-AR.md)
for byte-for-byte comparison of all 13 outputs and the remaining EVM dependency work.
