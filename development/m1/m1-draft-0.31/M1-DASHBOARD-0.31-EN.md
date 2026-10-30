# M1 summary — Draft 0.31

**The author ran nothing. M1 is not complete.** Counts below are expected and computed by the run in `dashboard-0.31-en.json`.

## C35: error-data size

Measurement in 0.30 used Python and differed from browser JSON encoding:
- A lone surrogate counted as 3 bytes; the browser emits 6.
- 1100 surrogates counted as 3347 bytes; the correct size is 6647.

Consequently, data larger than 4 KiB was retained, the checker waited and then displayed the page.

Measurement now uses native JSON encoding in the locally installed Node:
- Data exceeding 4096 bytes is dropped.
- The busy response then becomes invalid, stopping display without waiting.

Strict checks always precede the adapter: size, UTF-8, depth, duplicate keys and request ID.

## C36: the six failures in 0.30

These were evidence-linking errors, not behavioral defects.
- Links are now read from the root's machine-readable files.
- Failures are preserved unchanged and linked to their repair checks.

## Runtime

Python standard library plus locally installed Node v22.13.1, not Python alone. Missing Node causes checks to fail; no silent fallback.

## Expected status

- **38 candidate-complete rows.**
- **3 rows awaiting root review:** C3, C4 and V1.
- No row is complete before root review. No production code, deployment or transactions.
