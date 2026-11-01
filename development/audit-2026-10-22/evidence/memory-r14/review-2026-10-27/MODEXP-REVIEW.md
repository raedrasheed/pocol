# Independent adversarial review of the MODEXP precompile (2026-10-27)

- **Scope:** `lp4-execution/src/modexp.rs` (`0c2ad75`), its wiring in `native.rs`, and its tests.
- **Reviewer:** a separate agent instance. It was given the code and the reference crates (revm-precompile
  16.2.0, aurora-engine-modexp 1.3.0), not this work's conclusions. Its task was to find any input where the
  output differs from revm's `berlin_run`, any panic, and any input much slower than revm.
- **Environment:** a scratch copy outside the repository. Its test programs stay outside the repository.

## What it ran

- About 1.5 million random inputs over four seeds, compared with revm (gas, bytes and `Err`). About 280,000
  took the one-word path and about 440,000 the wide path.
- **Moduli:**
  - zero-padded one-word values;
  - q·2^t with t from 1 to 865, including 64, 65, 127–129, 191–193 and every multiple of 64;
  - q of 1 to 40 words, including 3, 2^64 − 1, 2^64 + 1 and all-ones;
  - powers of two, and random values.
- **Bases:** m, m − 1, m with junk appended, multiples of q, values with many low zero bits, bases longer than
  m, and leading zeros.
- **Exponents:** 0 to 3, values up to 2,000, 2^64 − 1, 2^64, 2^72, 2^80, high bytes followed by zeros, 32–256
  bytes of all ones, and leading zeros.
- **Inputs and gas:** inputs cut short; gas limits at the cost, at the cost − 1, at 200, at 30M and at u64::MAX.
- **Structured sets:** 16 odd q × t = 0..63 × bases near 2^(t±1), q·2^k and m ± 1 × 150 exponents. For the wide
  path, 7 shapes of q × 14 values of t, and 60 large wide moduli (q up to 300 words, t up to 5,065).
- The existing tests and the review's own, rebuilt with `overflow-checks` and `debug-assertions` on.

## Result

- **No divergence from revm** in gas, bytes or errors on any input.
- **No reachable panic.** None from slicing, shifts, `split_at` or `unwrap`, and no overflow even with overflow
  checks on.
- **Areas reported correct:**
  - parsing and its order of checks, so the `Err`s are revm's;
  - the output encoding, including modulus 0 and 1, leading zeros and a zero result;
  - the one-word Montgomery (carry handling, `r2`, `inv_word`) and the 2^t part;
  - the multi-word helpers and the Newton inverse;
  - the inner revm calls, and the wiring at address 5;
  - aurora itself on the inputs the fast paths take: its results are the true values, so matching the true
    value matches revm.

## Findings and what was done

| # | finding | severity | done |
|---|---|---|---|
| 1 | The wide path ran the Newton inverse and the CRT multiplications (Θ(n²) in the words of 2^t) where aurora answers at once: a zero or empty exponent, or base^e smaller than the modulus. For 3·2^606656 (75,833 bytes) with an empty exponent this took ~200 ms against revm's ~0.02 ms, about 7 ns per gas at most, so it does not raise the worst case | low | `revm_is_quick` hands those inputs to revm: a zero exponent, and e of at most 8 bytes with base words · e < modulus words (aurora's own condition, with words counted as aurora keeps them). Measured on the same inputs: ~49 µs against revm's ~10 µs (wrapper-overhead-2.log); the rest is the linear pre-parse of the 75 KB modulus. Test `the_wide_path_leaves_trivial_powers_to_revm`; the wide differential test now also counts the inputs handed over. Faults WX12–WX14 (mutants-wide-2.log): all caught |
| 2 | `modpow_word` reduced the base byte by byte before checking whether the exponent is empty: 76,000 bytes of base with an empty exponent took 2.1 ms against revm's 0.011 ms, about 0.07 ns per gas | negligible | The empty-exponent check now comes first. The result is the same (1 for m > 1) |
| 3 | The first differential test's positive control also counted trivial cases (m = 0, m = 1), so it would not notice the nontrivial one-word path being switched off. That would cost speed, not correctness | test gap | A second control: the fast path computed a power, with m > 1 and a nonzero exponent, in more than an eighth of the cases |

lp4 release suite with these changes: 209 passed, 0 failed, 1 ignored (lp4-release-quick.log). The results of
every input are unchanged: the changes only decide which code computes them.
