# Owner decisions (2026-11-21): the founder servers' disks and their monitoring

Status: **decided by the owner on 2026-11-21.** Approving a decision is not approval to deploy: installing the
monitor on the founder servers is a deployment step that needs the owner's explicit approval, like every launch step
on a server.

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

7. **D7: the founder servers keep 35 GB disks; the growth risk is accepted on the condition of tested monitoring.**
   The three founder servers, `108.165.128.89`, `108.165.128.90` and `108.165.128.91` (D3 of
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

## Where it stands (2026-11-21)

The monitor, its systemd units, its installer and its tests are in `deploy/monitoring` (CI:
`.github/workflows/founder-disk-monitor.yml`). They were tested locally only. Nothing is installed on the founder
servers and no test notification has been received yet, so D7's condition is not met yet; meeting it is the owner's
acceptance test on each server, after the owner approves the install.
