# LP9 stage report — Chrome extension for on-chain sites

Branch: `claude/affectionate-pascal-rjhh2k`. Source: `prototypes/pocol-extension`. Design and coverage: [LP9-EXTENSION.md](../lp9/LP9-EXTENSION.md).

## Completed
**Extension (MV3)**
- Network profiles: genesisPre is decoded and hashed; a profile is rejected if it disagrees with genesisHash or chainId. It is then bound to netKey.
- Before any fetch, the extension checks that the node belongs to the same network (chainId and genesis); otherwise it returns code 4901.
- RP trust level: a 13-header window is checked against items 1–9 and the viewTargetCeil target ceiling; the trust statement appears outside the site frame.
- Fully verified site fetching: each chunk's CREATE2 address, manifest hash and every file hash.
- Three-layer network isolation:
  - Strict sandbox-page CSP; the loader hash identifies the only permitted script.
  - Per-tab DNR rules.
  - RtcLockdown and prevention of nested-frame creation through all DOM interfaces.
- BridgeAuth: a size limit, integer token bucket, strict linear JSON parsing, a frozen allowlist and exact schemas.
- Routing limits: 4 pending reads, LogClient through pocol_getLogs with a 1024-block bound, navigation bucket, one pending confirmation and wallet limits.
- RecvGuard: per-method receive bounds, streaming reads cut off at the limit, strict UTF-8 decoding, depth ≤ 16 and reservation pools.
- Site storage (localStorage):
  - Literal quota accounting (1 MiB) and a 4096-pair limit.
  - Immutable versioned records (epoch, seq).
  - The D103 worker-generation fence prevents a late write from a dead generation from superseding a newer write.
- Local wallet:
  - Seed encryption with PBKDF2 (600000 iterations) and AES-GCM; a key derived per network.
  - Displays raw bytes and hash before signing; no signing or sending without a click.

**New engineering decision (adopted by recommendation under the authorization)**
- LP9-DNR-01: Chrome tab IDs can reach 1.4 billion, so 2·tabId exceeds rule-ID bounds.
- A small slot k is used per tab, with rules {2k, 2k+1}, retaining the same tab condition.

**Operational observation**
- With T_blk = 1 the chain falls behind schedule, because the fallback block requires at least 1+dFbWait seconds.
- ASERT eases the target until the RP client rejects the window at viewTargetCeil; this is correct client behavior.
- The adopted test setting is T_blk = 2, and the rule was recorded in the runbook.

## Tests
- Unit tests: **171/171** passed, covering:
  - BR1–BR19a tables and the literal BR17e quota table.
  - Fuzzing with ten thousand messages.
  - RecvGuard, T16 manifest vectors and wallet.
  - The I57 generation scenario.
  - Defective builds that must fail (noRefill, quotaNoSubtract and allowAll).
- Real Chrome test with a live three-node network: **66/66** passed.
  - Rendering HTML, CSS (including @import), JS and PNG from verified chunks.
  - HTTP network counter and UDP (STUN) counter both zero.
  - Each isolation layer alone blocks networking. Positive controls establish that the probes are real: 3 requests with both layers disabled, and 5 STUN packets without RtcLockdown.
  - Full BR14 wallet path: the transaction was included with a matching receipt on all three nodes; rejection returns 4001 and an ungranted account returns 4100.
  - Storage and isolation between sites.
  - Stopping the extension worker and recovering data in a new generation.
  - Navigation, site links, a 404 page and external links with confirmation.
  - Rejecting another network and reading update v2 through all three nodes.
- The JS RP tool accepted real headers produced by Rust nodes and rejected every mutation under the correct item. This establishes agreement for ASERT and items 1–9 between the two languages.

## Independent review
- An independent agent reviewed the entire extension and found no critical or high-severity vulnerability.
- It confirmed these aspects:
  - Isolation: no path to the network through all layers.
  - BridgeAuth, wallet and storage.
  - Content and header verification.
- It raised four findings, all repaired; details are in [REVIEW-RESOLUTION.md](../lp9/REVIEW-RESOLUTION.md):
  - Medium: show transaction fees and maximum cost before signing.
  - Low: the site must not extend the wallet unlock period.
  - Low: store `__proto__` like any other key.
  - Low: concurrent disk reservation D105.
- After repair: unit tests 171/171 and Chrome test 66/66.

## Explicit limits (not claimed)
- BR24: administrative deletion and tombstones.
- ProfileMutex.
- Remote endpoints (disabled; local only).
- Part of T12: woff2, forms and modules through importmap.
- Complete BR20a and BR19b tables.
- An independent BridgeRef for fuzzing.
- Details are in the coverage table.

## Next stage: LP10
- A unified runner for all suites: `prototypes/lp10/run_lp10.py`.
- An integrated scenario:
  - Partition the network and publish an update on the lesser-work branch.
  - View it in Chrome, then heal and reorganize.
  - Recover the correct content in Chrome.
  - SIGKILL followed by a full restart.
  - ≥ 100 blocks with matching roots, then journal replay.
- Resource measurement, runbook and final report.
