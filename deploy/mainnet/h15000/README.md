# PoCol Mainnet: the H15000 upgrade files

These are the public invitation and the trusted fork schedules of the upgrade that activated protocol version 2 at block
15000 on the existing chain. No genesis was rewritten, no balances moved and no chain data was reset.

| File | SHA-256 |
|---|---|
| `network.pocol.json` (served at https://pocol.net/downloads/network.pocol.json) | `f4ed1373019563106a4ba0af9f063c874c667a5be50a2b42f32f16e2dcbeaf3b` |
| `server-fork-schedule.json`: `[[1,0],[2,15000]]`, for gateways and nodes | `a5b51cebebdfcc8b51cf2d82afd3498d314a20adf6b5cabe2d5f2e7cf72fb0d3` |
| `browser-fork-schedule.json`: `[[1,"0"],[2,"15000"]]`, for the extension's trusted local schedule | `653d6a795cae1ca3bfd365a71afe348541ec23c9e011554db23aaccc7c4ad01e` |

The chain identity is:
- chainId `1599846715`;
- genesisHash `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca`;
- protocolVersion `2`;
- compatibilitySha256 `6cf38a14aa01ca3d824b0309564942cc926ca7232e2a5b2672a17e8c99e35476`.

Genesis keeps `nonceMode: 0`; after activation, the mining mode is the RPC's `effectiveNonceMode`. The display name is
PoCol Mainnet, but `profileKind`/`nodeKind` still read Simnet, and `publicLaunchReady:false` was not changed: branding
is not a launch-readiness approval.

## Distributions (release assets, not in Git)

The packages are inside `PoCol-Complete-Handoff-2026-11-01.zip` in the release
[`handoff-2026-11-01`](https://github.com/palcertcom/pocol/releases/tag/handoff-2026-11-01).

| Package | Bytes | SHA-256 |
|---|---:|---|
| `PoCol-Mainnet-0.4.0-Windows.zip` | 12,040,656 | `df9b3880a64be8bf537c7ae630e7a799741662821b0998f0bdfd20aebf6d6ff7` |
| `PoCol-Mainnet-0.4.0-Linux.zip` | 53,759,738 | `71c7ff9dbadc5021aa84a9f2c9e89f7a8de606f4f41d04ec0a8dea0c92b97170` |
| `PoCol-Mainnet-0.4.0-Android-Unsigned.zip` (not an installable update: unsigned) | 14,391,549 | `3d999074d5884871a1d1383895716ce29b87b9853ec66ee2658049785e7c8a67` |
| `PoCol-Studio-IDE-Viewer-0.4.1-English.zip` (the extension as published) | 332,570 | `85e6ebb6a0e3056a22c9c92090ed278f4eb9dcf6fb63631cefb5e6db3cbc5c1b` |

The Windows, Linux and 0.4.1 extension files served by pocol.net matched these hashes on 2026-11-01.
