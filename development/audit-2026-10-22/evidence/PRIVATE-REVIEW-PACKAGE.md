# Private review package (restricted; hashes only here)

These programs are exploit-shaped:
- the adversarial EVM measurements of the execution-memory model;
- the reconstructed audit witnesses;
- the round-2 reviewer's probes.

They are kept out of this public repository. They were delivered to the owner through the session's file
channel, as one archive. Only names and SHA-256 hashes are recorded here, so the delivered copy can be
checked. This page holds no contents, payloads or programs.

Archive `private-review-r2-b132a05.tgz`:
`fc46a03e07bbdb75e54b3bb2e20c64d928d536719a3c554453ace458ddf664e5`

| file in the archive | SHA-256 | covers |
|---|---|---|
| execution/private-adversarial-execution-test.patch | `98c22cbb4b7ced74f3ca6546dc7101cfa49632440d603e5c0bfacefcc0529624` | The private test `private_adversarial_execution_peaks` (A13-DERIVATION §3a, F3). Each case is checked against its own term and against the bound |
| execution/run-b132a05-release.log | `972deec38fd0827e7f7fdb06b696d4db19905641b8f73b52cbe5ca3246b61625` | Its run on b132a05, release: passed |
| witnesses/witnesses-r2.tgz | `06730f27fcf70b69893e1f02162075f25296a171998e9f95caff15f1828bef58` | The reconstructed witnesses with positive controls (WITNESS-CONTROLS.md) |
| review-probes/probe-main.rs | `241a6cf15b00aa4e39836d5c1c8e5d854d5264600528bec8da6549b971974e0c` | Round-2 review probe (F3, F5) |
| review-probes/probe-Cargo.toml | `46d058475494f86976f4df78454a07110e5dcf3e824cb340624132fa4e3baa95` | Its manifest |
| review-probes/race.rs | `7ed9ba795c1b6fe68990c80b4eac639bc66e44942ae7059248e891a8b20847cd` | Isolated replica of the F1 lost wakeup |
| README.md | `60446739d38b5c59b9f59247a9bd65b4dc113ace34bb659617cf334bfba85f8e` | Contents and reproduction instructions |

Durability: the producing container is ephemeral, so the owner's delivered copy is the durable record.
A later package that supersedes this one (for example after the final witness run) gets its own section
here, with its own archive hash.

The public tests do not replace the private measurements. The public
`execution_model_bounds_measured_adversarial_peaks` keeps only cases that were already public.
