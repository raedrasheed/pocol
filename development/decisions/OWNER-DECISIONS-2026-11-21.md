# Owner decisions (2026-11-21): the founder servers' disks and their monitoring

Status: **decided by the owner on 2026-11-21; the monitoring was installed and accepted by the owner on 2026-11-21**
("Acceptance of D7's monitoring", below). Approving a decision is not approval to deploy: every step on the founder
servers, the monitor's included, needs the owner's explicit approval, like every launch step on a server.

## The owner's words (verbatim)

> أعتمد رسميًا الخيار (أ) بشأن مساحة التخزين لخوادم PoCol Mainnet الثلاثة.

> الإبقاء على سعة التخزين الحالية البالغة 35 GB لكل خادم، وقبول مخاطر نمو بيانات البلوكتشين، بشرط تنفيذ نظام مراقبة وتنبيهات تلقائية فعّال واختباره قبل إطلاق الشبكة للجمهور.

> عدم حذف أي بيانات بلوكتشين أو تقليم السلسلة أو إيقاف خدمات PoCol تلقائيًا دون قرار منفصل مني.

> لا تنفذ تثبيتًا على السيرفرات قبل موافقتي الصريحة على خطوة النشر.

> لا نؤجل التحضير لإطلاق PoCol Mainnet بسبب سعة الأقراص، لكن تفعيل المراقبة والتأكد من وصول التنبيهات شرطان إلزاميان قبل فتح الشبكة للجمهور.

The same directive lists what the monitoring must do (measurement, a check every 15 minutes, alerts at 70%, 85% and
95%, alerts on accelerating growth and on the approaching full date, an off-host channel, a daily report,
de-duplication with repeats while a critical state lasts, a report when the monitor itself fails, tests that simulate
the thresholds and a real test notification before the system is accepted, and a light footprint for 4 GB / 35 GB
servers). This record states them as decided under D7.

## Decisions

7. **D7 (the owner's option (a)): the founder servers keep 35 GB disks; the growth risk is accepted on the condition of tested monitoring.**
   The owner formally approves option (a), keeping the current disks: the three founder servers, `108.165.128.89`, `108.165.128.90` and `108.165.128.91` (D3 of
   [2026-11-20](OWNER-DECISIONS-2026-11-20.md); Ubuntu Server 24.04 LTS, 4 GB of RAM, D2), keep their current 35 GB
   disk each. The owner accepts the risk of the blockchain data's growth on the condition that an effective automatic
   monitoring and alerting system is implemented and tested before the network is opened to the public:
   - **Before the network is opened to the public** (step 8 of [deploy/mainnet/README.md](../../deploy/mainnet/README.md),
     step 5 of the launch order in [PLAN.md](../public-network/PLAN.md)), the monitor is active on all three servers and
     a real test notification from each has been received (the acceptance test of
     [deploy/monitoring/README.md](../../deploy/monitoring/README.md)). The preparation of the launch (the release
     checks, the hash-rate measurement, the ceremony's preparation) is not postponed because of the disk size; the
     ceremony itself still needs its own approval.
   - **What the monitor does** (`deploy/monitoring`): every 15 minutes (a systemd timer) it measures every real
     filesystem's space and inodes and the PoCol data directory (`/var/lib/pocol-standalone`: the journal,
     history.redb, state.redb, canon.redb and the rest) and their growth over 1 h, 24 h and 7 d; it alerts at 70%, 85%
     and 95% of any disk or its inodes, when growth accelerates and when the estimated time to full falls under 30, 7
     or 2 days; it sends the alerts off the host (Telegram over HTTPS and/or email over SMTP with TLS, certificates
     verified), repeats an alert while it lasts (every 24 h, 6 h or 1 h by level) without flooding, reports recovery,
     sends a daily report (usage, growth, remaining space, estimated days to full) and reports its own failure
     (`OnFailure=`), a gap in its checks and, with an optional heartbeat URL, its silence.
   - **What it never does:** it never deletes blockchain data, prunes the chain, stops or restarts PoCol's services or
     changes the firewall. Any such action, and a disk expansion, needs a separate decision of the owner.
   - **Known risks, recorded truthfully.** The block journal is never compacted: in the worst case (every 10 s block
     body at BODY_MAX) it grows about 1.7 GB a day, about 620 GB a year
     ([REHEARSAL.md](../public-network/REHEARSAL.md), risk 11 and D1). At that rate 35 GB fills in about three weeks.
     The standalone README asks for disk for at least 2 years of the worst case, or the growth measured and alarmed
     ([deploy/standalone/README.md](../../deploy/standalone/README.md)); 35 GB is far below the 2-year worst case, so
     this decision takes the second remedy, the alarm. The R21 retained-state bound at the mainnet caps,
     59,702,459,358 B (the "Accepted risks" of [2026-11-20](OWNER-DECISIONS-2026-11-20.md), `admission-waiver-R21`),
     is itself larger than 35 GB. Idle growth is far lower: the membership part of history.redb was projected at
     8.0 MB a day idle and 12.1 MB a day at \|Δ\| = 24 (REHEARSAL.md, D1). The real growth on the founder servers has
     not been measured yet (D1 is measured on PoCol Mainnet after the launch). The disks may have to be enlarged
     later.

   The finding ledger records D7 as accepted risk F-94 ([FINDINGS.md](../public-network/FINDINGS.md)).

### Acceptance of D7's monitoring (2026-11-21)

The owner's words (verbatim, from the owner's report of 2026-11-21):

> أعتمد اكتمال شرط D7 الخاص بالمراقبة والتنبيهات قبل الإطلاق العام.

> مهم: بعد تثبيت عقد PoCol يجب تغيير `POCOL_DATA_REQUIRED=1` على الخوادم الثلاثة، والتحقق من أن مراقبة مجلد بيانات السلسلة تعمل.

> لا تنفذ أي إجراء على الخوادم دون موافقتي الصريحة.

What the owner reports, recorded as reported:

- **Date and servers.** On 2026-11-21 the owner installed the disk monitor of `deploy/monitoring`, from the version
  merged in `main` (the report names no commit), on the three founder servers: founder-1 `108.165.128.89`, founder-2
  `108.165.128.90` and founder-3 `108.165.128.91`.
- **Channel.** One channel type, Telegram, is configured. Its bot, chat and token are not recorded here (they live
  only in each server's `secret.env`).
- **The seven checks, passed on each of the three servers:**
  1. `--status`: the monitoring runs and the channel is configured;
  2. `--test-alert`: a `[TEST]` message was actually received;
  3. `--simulate-usage 96`: a `[TEST] [EMERGENCY]` message was received, and the real disk was unchanged;
  4. the service run by systemd shows an `ok:` line;
  5. `pocol-disk-monitor.timer` is active;
  6. a forced failure: a `[MONITOR FAILED]` message was received;
  7. after the test mode was unset and the service run again: a `[RECOVERED]` message was received.
- **Afterwards.** All three timers are active, and at the last successful check no message was queued and no delivery
  error was reported.
- **The owner's approval.** The owner approves that D7's condition, the monitoring and alerting before the network
  is opened to the public, is met.
- **Not done (the state of the report).** No PoCol node is installed on the servers, the final genesis has not been
  created and port 30300 has not been opened.
- **What remains of D7 (the owner's requirement).** After the ceremony installs PoCol on the three servers (step 6
  of [deploy/mainnet/README.md](../../deploy/mainnet/README.md)), set `POCOL_DATA_REQUIRED=1` on each of them
  (`sudoedit /etc/pocol-disk-monitor/monitor.conf`) and verify that the monitoring of the chain's data directory
  works: `--status`, and then the daily report, report the PoCol data directory `/var/lib/pocol-standalone` as
  present, with its size.
- **No action on the servers without the owner's explicit approval,** this step included.

What the acceptance does not change: the risk stays an accepted risk (F-94). The disks are still 35 GB; the monitor
warns and does not prevent a full disk, and a disk expansion or any action on the data still needs a separate owner
decision. By the monitor's design ([deploy/monitoring/README.md](../../deploy/monitoring/README.md)) it reports the
PoCol data directory as absent until PoCol is installed, and with `POCOL_DATA_REQUIRED=1` a missing or unreadable
data directory raises a CRITICAL alert.

## Where it stands (2026-11-21)

The monitor, its systemd units, its installer and its tests are in `deploy/monitoring` (CI:
`.github/workflows/founder-disk-monitor.yml`). The owner installed it on the three founder servers on 2026-11-21 and
reports its acceptance test passed on each; the owner approves that D7's condition is met ("Acceptance of D7's
monitoring", above). The risk stays accepted (F-94). What remains is the post-install step: `POCOL_DATA_REQUIRED=1`
on the three servers once the ceremony has installed PoCol, and the PoCol data directory reported present.
