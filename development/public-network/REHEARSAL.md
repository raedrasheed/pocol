# Rehearsal: the launch-evidence plan

This is the plan for the closed rehearsal of the fresh v3 network: what it measures, the threshold each measure is
read against, and the remedy when a measure misses its threshold. The thresholds T1-T8, M1-M4, R-off, D1, S1 and S2
and the risks they measure are the owner-approved fresh-network design's §4b and §4a (FRESH-NETWORK-DESIGN.md, kept
outside this repository), committed here with the same numbers. Sections 4 and 5 add the drills and the resource bound
that gate L1 lists ([PLAN.md](PLAN.md)).

This plan runs nothing and marks no gate closed. A gate closes only with its implementation and evidence
([PLAN.md](PLAN.md)). The gates are reported, and the node's readiness report (`publicLaunchReady`, `launchBlockers`)
is node-local and refuses nothing: it never stops a node from running, mining or serving sites.

**Remedies.** Each measure names a remedy, and the affected measures are run again after it. A remedy that changes a
profile value (for example GAS_LIMIT, BODY_MAX, D_att, τ, S_max, κm or, as a last resort, T_blk) changes the
consensus parameters before the genesis: it needs the owner's approval, is recorded in
[FRESH-NETWORK-CP.md](../decisions/FRESH-NETWORK-CP.md), and both profiles (`deploy/mainnet/v3-profile.json` and
`v3-rehearsal-profile.json`) change together. A remedy in code (the relay, the wallet, a program, the gateway) is an
ordinary reviewed change. A node-local declaration (Δ2 instead of Δ1) changes no CP field.

## 1. Run conditions

- **Profile and build.** The final CP ([FRESH-NETWORK-CP.md](../decisions/FRESH-NETWORK-CP.md)) with the rehearsal
  chain id **1656550731** (`deploy/mainnet/v3-rehearsal-profile.json`), on the release candidate built with
  `POCOL_RELEASE=1` from a pinned tree ([PLAN.md](PLAN.md), "Launch order"), with fresh keys on every founder's device.
- **Duration.** At least 24 h, plus the M1 fill (about 2,500 blocks, about 7 h).
- **Founders.** The design runs the rehearsal with all 4 founders mining: the three servers and the owner's home PC.
  [deploy/mainnet/README.md](../../deploy/mainnet/README.md) (step 6) describes the closed rehearsal on the three US
  servers. The host set is the owner's choice, made when the owner approves the step.
- **Home-PC steps.** A home-PC off/on step of 10 min, and a near-BODY_MAX site publish while the PC is the proposer.
- **If the PC cannot join,** use `tc netem` with 150 ± 30 ms and a 1-5 Mbit/s upload limit.
- **After the rehearsal** the devices are re-provisioned and every founder key is made fresh again for the mainnet
  ceremony ([deploy/standalone/README.md](../../deploy/standalone/README.md), "Rehearsal, then mainnet").

## 2. Thresholds (design §4b)

| # | Measure | Pass | Remedy |
|---|---|---|---|
| T1 | `pocol_templateLatency` per pair | late 0; sinceParentMs p99 ≤ 1,000 ms (Δ1) or ≤ 2,000 ms (Δ2); max < Δ_rel − 2 s, including full-body templates from the PC | Peer the PC to the US servers only, declare Δ2 (same CP), require ≥ 5 Mbit/s upload, or run the PC as a non-mining node (N3). Last resort: T_blk 20-30 s before genesis. |
| T2 | Clock offsets | pairwise ≤ 500 ms; zero future-queued templates | Fix NTP. φ = 2 still passes at dFbWait 8. |
| T3 | t_exec | worst block (GAS_LIMIT, site publish): validate+commit p99 ≤ 1 s on every founder; R14/R19 admitted | Lower GAS_LIMIT or BODY_MAX, or replace the slow founder. |
| T4 | Empty blocks | a=0 ≤ 0.5 %, a≥1 ≤ 0.2 % (outside off/on windows) | Raise D_att. |
| T5 | Forks | siblings ≤ 2 %, max reorg ≤ 2, `reorgs_ge_k_safe` 0 at `safe` 6, no alarm at 60 | Fix relay, then T_blk 20-30 s. |
| T6 | Intervals | 24 h mean 10 s ± 5 %; difficulty noise ≤ ±10 %; re-converges ≤ 150 blocks after a 25 % step; first 100 blocks' mean in 5-20 s | Re-measure hashRate; τ 1,200-1,800. |
| T7 | Home-PC shares | ≥ 99 % credited; credited fraction ≥ 0.95 × hashrate fraction | As T1. |
| T8 | Launch drill | all founders mining within D_att of g_ts | Scripted start. |
| M1 | **Full set:** a registration harness registers up to 10,000 identities (about 2,500 blocks ≈ 7 h), mostly idle, with some ACCRUING miners | child_ctx p99 < 100 ms; RSS within the A13 bound; history growth per day within C6's target; gateway reward computation < 1 s per block; T4 and T5 unchanged | Finish or repair FN-3 or the child-context caching (FN-1, FN-1b), then run M1 again. |
| M2 | SYS gas at the full set | settle (all-ACCRUING payees), recordShares, recordLate, beginBlock and the phase-0 total within `GAS_ENVELOPE_V3` and G_SYS; no SYS call grows with M | Re-measure; lower S_max or κm only through ParamGate. |
| M3 | Capacity path | at a full set, two registrants in the same block with `registerReplacing`: both succeed on distinct victims; `register` alone gets `Capacity`; `removeInactive` reverts below a full set; a victim re-registers after U | Fix the wallet or program. |
| M4 | ACCRUING exposure | fraction of pots won and blocks proposed by ACCRUING members is recorded; any type-2 INVALID pot is logged with its burned amount | Disclose; owner review if material. |
| R-off | **Founder offline > Θ + U (about 2 h) at the full set** | the home PC is evicted by `registerReplacing`; it is refunded at U; it registers again (with 20 PCOL → HELD, and once with 0 → ACCRUING, re-earning the bond) | Fix the wallet or program. |
| D1 | Disk per day | journal + history.redb growth recorded; projected 2-year worst case fits each server's disk | Size the disks or add an alarm. |
| S1 | Sites | **the real explorer bundle** and a synthetic 4 MiB site published (≤ 70 + 10 and ≤ 175 + 10 blocks); base-fee drift recorded; `pocol-site fetch` digests identical on all 4 nodes before and after restart; the Chrome viewer, Studio and IDE open both through the gateway within the rate budget; `pocol_getCaps` matches the state; the publisher refuses before signing at a near-full cap (cap-test build) | Fix the gateway caching or publisher fees. |
| S2 | Cap raise (cap-test build) | fill ACCT_CAP; `register` and new accounts fail; activate C10 at a height; both succeed; replay and reorg across the height are consistent | Fix C10 and run S2 again. |

Reading the measures on the node:
- **T5.** The design's `reorgs_ge_k_safe` counter was renamed: since owner decision 12 the node counts
  reorganizations of `k_alarm` (default 60) blocks or more as `reorgsGeKAlarm`, and reports the deepest one as
  `maxReorgDepth` (`pocol_finality`, and `pocol_status.retention`; `development/m1/reference/network.md`, the
  `pocol_finality` entry). "`reorgs_ge_k_safe` 0 at `safe` 6" is read as `maxReorgDepth` below `kSafe` 6, and "no alarm
  at 60" as `reorgsGeKAlarm` 0.
  These counters, and `reorgs`, count since the node's start: a restart sets them to 0, and `maxReorgDepth` keeps the
  deepest reorganization until the next start. So T5 is read in intervals: read `reorgs`, `maxReorgDepth` and
  `reorgsGeKAlarm` on every node at the start of the run, just before and just after each drill window (section 4),
  before every restart (planned or not; an unplanned one is found from the service's restart count), and at the end.
  An interval outside the drill windows passes when its `reorgsGeKAlarm` increase is 0 and, if `reorgs` rose,
  `maxReorgDepth` at its end is at most 2. A drill that raises a node's `maxReorgDepth` (the partitioned minority,
  section 4.1) hides any later shallower reorganization on that node, so that node is restarted cleanly right after
  the drill window, which starts its counters again (the restart can serve as the section 4.2 drill).
- **M1.** C6's history target is 100 MB a day for the membership part of history.redb
  ([FN-3C-C6-EVIDENCE](../decisions/FN-3C-C6-EVIDENCE-2026-11-14.md)). The A13 bound is in section 5.
- **S2** follows [OPERATIONS.md](OPERATIONS.md), "Activating the cap raise (FN-5, C10; rehearsal S2)", step 7.

## 3. The risks these measures read (design §4a)

| Risk | Design figures | Read by |
|---|---|---|
| 1. Node memory and history at 10,000 members | Corrected in [FN-3C-C6-EVIDENCE](../decisions/FN-3C-C6-EVIDENCE-2026-11-14.md) §1: closed by FN-3 (evidence FN-3c) and FN-1b. A13 admits the node at the design's §1a CP at 5,485,767,089 B, 151,377,487 B (144 MiB) inside the usable 5.25 GiB; the membership part of history.redb projects to 8.0 MB a day idle and 12.1 MB a day with every block at \|Δ\| = 24. | M1, section 5 |
| 2. Cap fill with no creation fee | The account cap fills first: 10^6 new-account transfers at 21,000 gas = 2.1·10^10 gas, about 0.21 PCOL at the 10^7 wei floor, about 14.6 h at about 190 per block. After that a newcomer without an account cannot register. All three caps fill for about 3.2-3.3 PCOL over about 9.5 days; consensus and mining continue. The design's mitigations: C10 (the cap-raise path, FN-5), C9's occupancy report (`pocol_getCaps`), Q1(b). Drafted for the launch disclosures ([deploy/standalone/README.md](../../deploy/standalone/README.md), "Launch disclosures", State caps; wording awaiting owner review). | S1 (near-full cap), S2 |
| 3. 10 s latency | Needs measured Δ ≤ 2 s and φ ≤ 1 s. The withholding gain is 1.6× the 30 s value (+0.105 at f = 0.2); natural forks about 0.8-1.4 %; the home PC's latency tax about 1-2.5 %. | T1, T2, T5, T7 |
| 4. Calibration | A wrong target_g gives a burst of 1 s blocks or slow blocks for 75+ blocks. Measure the founders' aggregate hashrate. | T6 |
| 5. Launch synchronization | A founder starting more than D_att (90 s) after g_ts produces fallback-only blocks. | T8 |
| 6. Sybil seats and eviction at a full set | A zero-work seat costs only gas (about 2·10^-6 PCOL); M_max fills in about 6.9 h at R_max 4. At a full set of equal small miners (11,520 shares/h over 10,000 members, about 1.15 each) about 31.6 % are removable at any moment; newcomers can evict up to 1,440 per hour. Below a full set nobody can be removed. | M1, M3 |
| 7. Founders' seats | At a full set a founder offline for more than 1 h can lose its seat to `registerReplacing`; it registers again with 20 PCOL (HELD at once) after U, if a removable seat exists. | R-off |
| 8. V3-16 exposure (C2) | ACCRUING winners can make a whole pot INVALID under type-2 evidence, and low-bond (≥ 5 PCOL) members can propose. | M4 |
| 9. Gas envelope basis | settle has up to 259 payees per pot; at about 32.7k gas per promoted payee that is about 8.5-13M, to be re-measured below the 30M per-call cap; R18 re-verified against the V3-only gas domain. | M2 |
| 10. Site throughput and lifetime | One full chunk per block is about 2.4 KiB/s. A 4 MiB site takes about 29 min (175 blocks) and pushes the base fee from 0.01 to about 15 gwei (about 1.9 PCOL burned). The explorer bundle (1.66 MB) takes about 70 blocks. The IDE bundle (20.6 MB, one 10.8 MB file) stays hosted. CODE_CAP is the lifetime limit. | S1 |
| 11. Disk | The journal is never compacted: worst case BODY_MAX × 8,640 ≈ 1.7 GB per day (about 620 GB per year). Size the servers' disks for at least 2 years of worst case, or measure and alarm. | D1, section 5 |
| 13. R_KEEP 5.7 h | A longer partition forces a resync. | section 4.1 |
| 14. Admission label | The release reports `publicLaunchReady: false` with its launch blockers (Q3). The report is node-local and refuses nothing. Owner decision 2026-11-18: the launch disclosures carry no "not public-launch-ready" item. | (reported, not measured) |

Risk 12 (process: FN-2b's same-PR rule and the one-shot lock reset) is a development rule, not a rehearsal measure.

## 4. Operations drills (gate L1)

Each drill runs on the rehearsal network and records its timestamps; its window is left out of T4 and T5, like the
home-PC off/on window. The
server commands use the paths of `deploy/standalone/install-linux.sh` (`/opt/pocol-standalone`, the
`pocol-standalone` user and home `/var/lib/pocol-standalone`, RPC on `127.0.0.1:8545`). A node's state is read with
RPC on the host, for example:

```sh
curl -s -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}' http://127.0.0.1:8545
```

### 4.1 Partition and heal

1. Cut one server's P2P (TCP 30300) from the other founders with the host firewall, for a set time well below R_KEEP
   (2,048 blocks, about 5.7 h): for example 30 minutes.
2. Heal it, and record the time until every founder reports the same block hash at a common height at least `kSafe`
   (6) below the lowest head.
3. Record `maxReorgDepth` and `reorgsGeKAlarm` on each side. The partitioned minority abandons its own branch when it
   heals; the majority's counters should not move. Then restart the minority cleanly, so its counters start again
   for T5 (section 2, "Reading the measures on the node").
4. Optionally, a partition longer than R_KEEP: record the resync the node then needs (risk 13).

Pass: every founder converges on one chain without a resync after the short partition, and the majority shows no
reorganization. Remedy: fix the relay or sync code, then repeat the drill.

### 4.2 Restart and replay

1. Restart one founder cleanly (`sudo systemctl restart pocol-standalone`), and once after an unclean stop
   (`sudo systemctl kill --signal=SIGKILL pocol-standalone`; the unit's `Restart=on-failure` starts it again).
2. Record the time from the start to the head, and the journal replay at open (the node's log).

Pass: the node reopens without a `LOCAL CAPACITY` refusal at open ([OPERATIONS.md](OPERATIONS.md)), reports
`evictStalls` 0, rejoins mining, and reports the same block hash as its peers at a common confirmed height; the
`pocol-site fetch` digests are unchanged (S1). Remedy: fix the journal, replay or restore code, then repeat the drill.

### 4.3 Backup and restore

1. On one server, back up the miner key encrypted and check the backup, as the `pocol-standalone` user
   ([RECOVERY.md](RECOVERY.md), section 1: `pocol-key export`, then `pocol-sign --chain-id 1656550731 --address
   --backup BACKUP`).
2. Restore it as in [RECOVERY.md](RECOVERY.md), section 2 (`pocol-key import` for a missing key file; `pocol
   import-key` into a re-provisioned home), with the server's node stopped. Never run two nodes with one miner key.
3. On the PC, back up and restore the wallet key (Settings → Key backup, or `pocol.exe export-key` and `import-key`;
   [deploy/standalone/README.md](../../deploy/standalone/README.md), "Encrypted key backup").
4. Optionally, the lost-key path on one founder ([RECOVERY.md](RECOVERY.md), section 4): a new identity registers
   again; the old record stays active until it is removable at a full set (R-off exercises that refund path).

Pass: the restored node reports `miner` equal to the address in `identity.address` and mines (`stats.mined` grows); a
fresh home syncs to the head (record the time). Remedy: fix the key tools or the runbook, then repeat the drill.

### 4.4 Release and rollback

The drill follows [OPERATIONS.md](OPERATIONS.md), "Upgrades (G2, G3)": a release that changes only the build digest
(no consensus rule, no schedule) interoperates with the previous one, so founders may take it in any order.

1. Build a second release candidate from the same consensus sources that differs only in its build digest; check its
   `pocold build-info` (the same `consensusRulesVersion`) and its package's `SHA256SUMS`.
2. On one server, stop the service, keep the current binaries aside, install the candidate's `pocol`, `pocold`,
   `pocol-key`, `pocol-sign` and `pocol-site` in `/opt/pocol-standalone` (mode 0755), and start it:

   ```sh
   sudo systemctl stop pocol-standalone
   sudo install -d -m 0755 /opt/pocol-standalone.previous
   for name in pocol pocold pocol-key pocol-sign pocol-site; do sudo cp -p "/opt/pocol-standalone/$name" /opt/pocol-standalone.previous/; done
   for name in pocol pocold pocol-key pocol-sign pocol-site; do sudo install -m 0755 "CANDIDATE_DIR/$name" "/opt/pocol-standalone/$name"; done
   sudo systemctl start pocol-standalone
   ```

3. Roll back the same way from `/opt/pocol-standalone.previous`.
4. On the PC, `pocol inspect-upgrade --network FILE` shows what an invitation change would do and writes nothing.

Pass: after each move the node keeps compatible peers (`compatiblePeers`), reports the same `consensusIdentity` as
the others, and follows and extends the head. G3 (a fork activation) is not drilled here: seeds have no fork
migration tool yet (the G3 TODO in OPERATIONS.md). Remedy: fix the build identity or the launcher, then repeat.

## 5. Resource-growth bound

RSS and disk are recorded on every founder over the whole run, M1 included, and read against the A13 store-mode bound
([A13-DERIVATION](../audit-2026-10-22/A13-DERIVATION.md), sections "FN-3c" and "FN-1b";
[FN-3C-C6-EVIDENCE](../decisions/FN-3C-C6-EVIDENCE-2026-11-14.md)).

- **Memory.** The A13 model admits the node at the design's §1a CP at 5,485,767,089 B (5.11 GiB) after FN-1b, under
  the usable 5,637,144,576 B (7/8 of the 6 GiB budget). Sample the node's RSS, for example
  `ps -o rss= -p "$(systemctl show -p MainPID --value pocol-standalone)"`. Pass: peak RSS stays within that bound and
  does not grow with height across the run (A13 §2g, windowed history). The server unit also sets `MemoryHigh=2560M`
  and `MemoryMax=3G` (`install-linux.sh`, a bounded pilot on a 4 GiB host), so record RSS against those limits too; a
  node stopped by `MemoryMax` shows as a service restart. Remedy: size the host's memory and the unit's limits (owner
  approval), or reduce the node's memory use in code with A13 re-derived.
- **Disk (D1).** Record the daily growth of the journal, history.redb and state.redb, and the node's `diskObserved` and
  `resourceDiskBytes` ([OPERATIONS.md](OPERATIONS.md)). The membership part of history.redb was projected at 8.0 MB a
  day idle and 12.1 MB a day at \|Δ\| = 24, against C6's 100 MB; the journal's worst case is about 1.7 GB a day. At the
  §1a CP, after FN-1b, the A13 test (`prototypes/pocold/tests/fn3_scale.rs`,
  `a13_fresh_network_headroom_against_the_observed_files`) finds the node admitted while state.redb stays below
  1,221,539,659,776 B (1,137.6 GiB, history.redb empty) or history.redb below 620,041,863,168 B (577.5 GiB,
  state.redb empty) (`development/audit-2026-10-22/evidence/fn1b/a13-and-genesis.log`; FN-3C-C6-EVIDENCE §5 and §6).
  Before FN-1b the figures were 1,317.6 and 668.3 GiB, the ones in A13-DERIVATION's FN-3c section. Pass (D1): the
  projected 2-year worst case fits each server's disk. Record also the projected 2-year state.redb and history.redb
  sizes against these A13 file bounds. Remedy: size the disks or add an alarm.

## 6. Recording

Record each measure with its value, its threshold, pass or miss, and any remedy applied, under
[evidence/](evidence/) for the rehearsal run. The evidence holds only public data: addresses, hashes, heights,
timings and counters, never a key file, a backup or a passphrase. A gate in [PLAN.md](PLAN.md) changes its status only
with that evidence.
