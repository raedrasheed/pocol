# PoCol P2P automatic outbound (PR2b) — decisions taken at their recommended defaults

Status: **the four owner decisions of the PR2b plan are implemented at the plan's recommended defaults.** All four
are node-local (no wire, consensus or `content_key` change), the class the owner's rule of 2026-11-08 approves
without reservation ([V3-OWNER-DECISIONS-2026-11-08.md](V3-OWNER-DECISIONS-2026-11-08.md) §1). Each names its
fallback, so the owner can still choose another option before the release; nothing here is deployed.

| # | Decision | Default taken | Fallback if declined |
|---|---|---|---|
| D14′ | The 50-peer rule, outbound side | development/m1/reference/network.md:157 reads "(32 established inbound; ≤ 18 outbound: configured peers, ≤ 8 automatic and 1 spare; the 8 handshaking places are not peers)". `auto` is clamped to fit; an explicit `--outbound N` past it is refused at boot. | Clamp `auto` to `min(4, 50 − inbound_max − peers − 1)` under the old text. |
| D7′a | Non-listening nodes and the seeds | A node without a listener keeps one seed wanted while the seeds answer (a dead seed is held 1 min … 6 h, so this costs almost nothing once they stop). | Release the seed on health alone, as listening nodes do. |
| D17 | Bootstrap without the seeds | An optional v2 invitation field `bootstrap` (≤ 16 community listeners that agreed to be listed), outside `content_key`, refreshable by `pocol upgrade`, and `pocold run --bootstrap`. | A `bootstrap.pocol.txt` beside the invitation; a "reseed" upgrade path; documented `addrbook` copying. |
| D23 | Retiring the seeds | An optional v2 invitation flag `seedsRetired`, outside `content_key`: when true the launcher passes no seeds. | Rely on dial backoff and seed holds alone. |

VA-SPEC §1a.7-§1a.9 states the behaviour these defaults give.
