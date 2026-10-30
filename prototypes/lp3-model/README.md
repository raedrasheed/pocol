# LP3 TypeScript reference model (S6)

Pure reference arithmetic, timing, membership, nonce assignment and proposer Draw;
bounded iterative GenesisSpec decoding, canonical encoding and Keccak identity.
No parent-state authentication, signing, private keys, EVM, production node or boot.
All inputs use bigint where integers can exceed JavaScript's exact-number range.
The line adapter quotes integer tokens before JSON parsing to preserve u64 values.

Local execution uses Node 24.19 native type stripping, **not type checking**:

    node --experimental-strip-types tests.ts

Required proper type checking and Node 22 execution are a separate PR workflow with
exact TypeScript 5.9.3, @types/node 22.18.6 and undici-types 6.21.0 dependencies.
From this directory after installing those exact packages:

    npm run typecheck
    npm test

Proper TypeScript checking and Node 22 execution have passed in GitHub Actions;
source-bound run IDs and the initial corrected type-check failure are preserved under
development/lp3/s6-model/codex-r1. Final-head requirements must still pass before merge.
The first local `node --test tests.ts` invocation reported only a file-level result
under this environment; the corrected direct invocation verifies all seven named tests.
Eight named tests now include every accepted L4n/order supplement and the 1500-level
deep/shallow corpus. Separate Python review covers 12,000 arithmetic cases against
Rust and TS, 10,000 timing cases, 1,200 Genesis cases against Rust and TS, and the
697-scenario/7977-sampler assignment corpus. This does not complete S7 or LP3.
