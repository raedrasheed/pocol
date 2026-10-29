# Audit repair baseline (A01–A14)

| Item | Value |
|---|---|
| Audited snapshot | `090eae4dc059d1a7df330d9fe674c873ac515564` (branch `claude/affectionate-pascal-rjhh2k`) |
| Current state of the audited branch at repair start | unchanged: `origin/claude/affectionate-pascal-rjhh2k` = `090eae4` (fetched 2026-10-22) |
| Integrated elsewhere? | no: `090eae4` is not an ancestor of `origin/main` (`15a3877`); no other branch contains it |
| Repair branch | `claude/audit-repair-a01-a14`, created from `090eae4` |
| Audit report received | `REPORT.md`, SHA-256 `b4862d72262ef05741f68c1817ad536458f876a96fff96ba853700e31f4e9769` |
| Audit attachments FINDINGS.json, COVERAGE.md, REPRODUCE.md, overlays, SHA256SUMS | **NOT PROVIDED** (only REPORT.md was attached; none are in the repository on any branch). Verification against SHA256SUMS is **BLOCKED**. Every finding was therefore reproduced independently from REPORT.md |
| Toolchain used here | rustc/cargo 1.97.0 (the repository pins 1.99.1 in rust-toolchain.toml; rustup is not available in this container, so the pin is not applied), Node 22.22.0, Python 3, Playwright Chromium build 1194 |
| Repository visibility | public. Exploit-shaped witness programs are kept local (archive SHA-256 `547202e3520b72348e0ade040fe492673fdce37c313b04de668e3e149f3d69a7`); only their observation log is committed: `evidence/baseline-090eae4.log` |

## Reproduction on the audited snapshot

Witnesses ran against an isolated `git worktree` of `090eae4` with separate Cargo target directories.
A witness prints `REPRODUCED` when the defect is present; it is an observation, never an acceptance pass.

| Finding | Witness | Result on 090eae4 |
|---|---|---|
| A01 | valid fallback header + body `txs=[empty]` through `Node::import`, then the legitimate body | REPRODUCED: `Block("15 empty body item")`, then `Block("known invalid: …")` |
| A02 | DevnetConfig S_max=1 (R9); H_CLOSE=H_END (R13); GenesisSpec with SYS_SLOT_MAX=1 and re-derived allocRoot | REPRODUCED ×3: builds succeed; 69 system slots admitted while standalone r18 rejects |
| A03 | signed block with a CREATE emitting LOG0 of 32769 bytes | REPRODUCED: block valid, receipt status 1 with the log |
| A04 | three committed records, one bit flipped in the first length field | REPRODUCED: `open` → Ok, 0 payloads, `torn_tail=true` |
| A05 | live node: 48 P2P sockets without Hello; 64 idle RPC connections | REPRODUCED: all 48 kept after 12 s; node threads 54 → 118 |
| A06 | real service worker on the Chrome mock: conflicting profile; confirmation pending while the endpoint genesis changes and the wallet locks | REPRODUCED ×3: accounts granted on a conflict profile; signed raw present before the click; transaction submitted after lock + identity change |
| A07 | `decodeSpec` with T_blk=0, nonceMode=2, empty allocRoot, scalar M0List, empty sysCodeHash | REPRODUCED: all five accepted |
| A08 | synthetic valid RP window, then the last header with a 31-byte stateRoot, empty logsBloom, list gasLimit (re-mined) | REPRODUCED: `verifyWindow` ok |
| A09 | three concurrent 8 MiB `pocol_getLogs` reservations in one session | REPRODUCED: peak 25 165 824 > 10 485 760 |
| A10 | Origin-bearing `pocol_getTemplate`: singleton vs one-element batch | REPRODUCED: singleton −32601, batch member executed |
| A11 | submit → included in a signed block → heavier competing branch | REPRODUCED: pending nonce back to 0, transaction gone |
| A12 | service worker, first record write never settles, then 8 more writes | REPRODUCED: storeTimeout reply released admission; 0 of 8 refused |
| A13 | source review (derivation completeness) | not a runtime witness; see REPAIR-PLAN |
| A14 | request id 123 answered by `{"jsonrpc":"1.0","id":999,"result":…}` | REPRODUCED: result returned |
