# Bounded decoder fuzzing

Run from the repository root with `cargo-fuzz 0.13.2`, nightly Rust and `rust-src`.
The recorded run used `nightly-2026-11-04` (`708c94ef0`) and AddressSanitizer.
The separate lockfile preserves the application dependency versions; no production
manifest or lockfile is changed. Build and corpus directories can stay outside the checkout.

```bash
python3 development/security-audit-2026-11-04/fuzz/seed.py /tmp/pocol-fuzz-corpus
cargo +nightly-2026-11-04 fuzz run \
  --fuzz-dir development/security-audit-2026-11-04/fuzz \
  --target-dir /tmp/pocol-fuzz-build -j 4 parsers /tmp/pocol-fuzz-corpus -- \
  -max_total_time=180 -max_len=16384 -rss_limit_mb=2048 -timeout=3 -seed=777900
```

The selector dispatches to P2P, RLP, headers, bodies, genesis, signed transactions,
invitation JSON and hexadecimal decoding. Accepted canonical encodings must round-trip.
An empty corpus also works; the generator supplies public signed test vectors, the
peer-discovery frames (the 11-item Hello, GetAddr, Addr) and minimal structural inputs.
It does not reproduce libFuzzer's exact evolving corpus.

The initial run before moving this harness into the repository completed 25,050,824
executions in 183 seconds, coverage counters 2,590 / features 6,061, and reported
OOM/timeout/crash 0/0/0. `-j 4` becomes `-fork=4`; the RSS limit applies to a worker,
not a container-wide aggregate. These are execution counts, not unique inputs or
proof that all parser branches were reached. Fuzzing does not validate consensus,
execution, reorganization, signatures' timing behavior or a live network's capacity.
