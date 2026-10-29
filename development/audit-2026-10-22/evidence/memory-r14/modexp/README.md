# MODEXP: two exact fast paths and the shape sweep (2026-10-27)

The precompile at address 5 is `lp4-execution/src/modexp.rs`. Its results, gas and errors are revm 19.7's
`berlin_run`'s for every input. The analysis is in A13-DERIVATION §3b-2, and the decision in
proposals/OPEN-DECISIONS rows 1–2.

The commits:
- `b46658a`: the one-word path;
- `b66ba65`: the test of moduli of more than one word (kills MX8);
- `0c2ad75`: the wide even path, its tests, and `jm_modexp_sweep`;
- `d30dc71`: `jm_store_scale` (no change to the measured code).

Host: 4 vCPU KVM; rustc 1.99.1; release builds, with `POCOL_ROOT_CHECK=0` for every timing.

## Files, by the tree they ran on

| file | tree | what | result |
|---|---|---|---|
| exec-cost-per-gas.log | `1babbc7` + modexp.rs before it was committed as `b46658a` | `jm_exec_cost_per_gas`: each precompile per call; JM-PRECOMPILE-POCOL beside revm's line for MODEXP | one-word shapes 4–7× faster (b8_e1024_m8_even 512 → 86 µs); others revm's |
| r14-gap.log, r14-gap-with-b16.log | `563a0c3` + the same | `jm_r14_workload_gap` with the one-word path | one-word MODEXP blocks 236–252 ms (were 957–1,023) |
| lp4-release.log | `563a0c3` + the same | lp4 release suite | 205 passed, 0 failed |
| mutants.log | `b46658a` and `b66ba65` (separate worktree) | 11 seeded faults of the one-word path and its wiring | 11 caught |
| pocold-release-b66ba65.log | `b66ba65` | pocold release suite | 238 passed, 0 failed, 24 ignored |
| sweep-before-wide.log | `2840571` + the sweep test (no wide path) | `jm_modexp_sweep`, four modulus kinds | the slowest real block 622 ms: a 256-byte modulus with its low half zero, exponent 3 |
| sweep-evenmax-before-wide.log | the same, plus the 3·2^(8m-2) kind | the sweep on that kind only | **1.46–1.56 s per full block, ~190 ns per gas**: aurora's bit-by-bit inverse mod 2^t |
| lp4-release-wide.log | `0c2ad75`'s code before its commit (one comment differs) | lp4 release suite | 208 passed, 0 failed |
| mutants-wide.log | the same (separate worktree) | 11 seeded faults of the wide path | 11 caught; WX11 changes routing only (results stay revm's) and the routing assertion catches it |
| pocold-release-0c2ad75.log | `0c2ad75` | pocold release suite | 238 passed, 0 failed, 25 ignored |
| sweep-0c2ad75.log | `0c2ad75` | `jm_modexp_sweep`, all five kinds, 384 shapes, the six slowest built as blocks | slowest real block 455 ms (16-byte modulus, revm's path); 3·2^8190: 1,062 → 156 µs per call |
| r14-gap-d30dc71.log | `d30dc71` | `jm_r14_workload_gap` with the slowest shapes, both modes, genesis and grown state | every kind under t_exec = 1 s: at most 474 ms validate, 510 ms import |
| wrapper-overhead.log | `d30dc71` + `wrapper_overhead_interleaved` (committed with this file) | interleaved per-call timing on inputs handed to revm | pocol = revm within noise; its own checks 0.07–0.56 µs |

## Two corrections

- **An earlier conclusion was wrong.** After `b46658a` this work reported that D-2's premise no longer held on
  this host. The sweep then found the second class (3·2^(8m-2) with a small exponent) at 1.46–1.56 s. The
  premise held until `0c2ad75`.
- **One-shot per-call timings vary up to about 2× on this host.** For example, in sweep-0c2ad75.log the shapes
  on revm's path show pocol up to 1.8× slower than revm. Interleaved (wrapper-overhead.log), the two are equal
  within noise. The full-block validations (median of 3) are the figures to use.

## Not established here

- That no shape outside the grid is slower. The sweep is a grid, not a proof of the maximum.
- Any decision: D-2 and the calibration workload are rows 1–2 of proposals/OPEN-DECISIONS.md.
