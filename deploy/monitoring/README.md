# deploy/monitoring: the founder servers' disk monitor

The three founder servers, `108.165.128.89`, `108.165.128.90` and `108.165.128.91` (Ubuntu Server 24.04 LTS, 4 GB of
RAM, a 35 GB disk each), keep their 35 GB disks by owner decision of 2026-11-21 (D7,
[OWNER-DECISIONS-2026-11-21.md](../../development/decisions/OWNER-DECISIONS-2026-11-21.md)). The owner accepts the
risk of the chain's growth on the condition that an automatic monitoring and alerting system runs on all three
servers, and that a real test notification from each has been received, before the network is opened to the public
(step 8 of [deploy/mainnet/README.md](../mainnet/README.md)). This directory is that system. Nothing here has been
installed on a server: installing it is a deployment step that needs the owner's approval.

Why it matters: PoCol's block journal is never compacted. In the worst case (every 10 s block body at BODY_MAX) it
grows about 1.7 GB a day, about 620 GB a year
([REHEARSAL.md](../../development/public-network/REHEARSAL.md), risk 11 and D1;
[FINDINGS.md](../../development/public-network/FINDINGS.md) F-26), so at that rate a 35 GB disk fills in about three
weeks. Idle growth is far lower (the membership part of history.redb was projected at 8.0 MB a day idle). The disk may
have to be enlarged later; the monitor gives the warning, the owner decides.

## What it does

`pocol_disk_monitor.py` (Python 3 standard library only, nothing to install with pip) runs every 15 minutes from a
systemd timer and:

- measures every real mounted filesystem (ext4, xfs, vfat, btrfs and the like; tmpfs, devtmpfs, overlay, squashfs,
  proc, sysfs and other pseudo filesystems are skipped, and bind mounts of the same device are counted once): size,
  used, free (the space available without the root reserve, as `df` reports), percent used and inode use;
- measures the PoCol data directory, `/var/lib/pocol-standalone` (`deploy/standalone/install-linux.sh`): its real disk
  use (allocated blocks, so sparse files count what they use) split into the journal (`state/blocks.journal`),
  history.redb, state.redb, canon.redb and the rest. It lists directory entries and reads their metadata only; it never
  opens a PoCol file. Before PoCol is installed the directory is reported as absent;
- keeps samples (every 15 minutes for 26 h, then one an hour for 8 days, at most 400) and computes the growth over
  1 h, 24 h and 7 d, for every filesystem and for the PoCol data, and the estimated days until each filesystem is full
  at the faster of the 24 h and 7 d rates (the 1 h rate is shown but not used for the estimate: a store file can grow
  in one step);
- sends alerts, a daily report and failure notices off the host, by Telegram and/or email.

### Alerts

| Condition | WARNING | CRITICAL | EMERGENCY | Clears |
| --- | --- | --- | --- | --- |
| Disk space used, any filesystem | 70% | 85% | 95% | 2 points below the level |
| Inodes used, any filesystem | 70% | 85% | 95% | 2 points below the level |
| Estimated time until full | under 30 days | under 7 days | under 2 days | above the level x 1.2 |
| Accelerating growth (disk and PoCol data) | 24 h rate at least 3 x the rate of the days before and at least 500 MB a day, sustained: each 12 h half of the last 24 h grew at least 250 MB a day | | | when the 24 h rate falls below 80% of either limit (0.8 x 3 x the earlier rate, or 400 MB a day), or one 12 h half grew under 200 MB a day |
| PoCol data directory missing (with `POCOL_DATA_REQUIRED=1`) | | yes | | when present |
| Founder peer port unreachable (optional, `PEER_CHECK=1`) | | N consecutive checks (3) | | when reachable |
| Monitoring gap: no successful check for over 45 minutes | once | | | |
| System clock behind the recorded history | yes | | | when the clock is right again |

- An alert is sent when a level is entered; while it persists it is sent again every 24 h (WARNING), 6 h (CRITICAL)
  or 1 h (EMERGENCY); when it eases or clears a RECOVERED or EASED message is sent. Every value is configurable in
  `monitor.conf`.
- All events of one check go in one message. Each channel sends at most 12 messages an hour; the rest wait.
- A failed delivery is retried at every following check, the most severe message first. While a channel is down, a
  newer alert message replaces the undelivered one (and a newer daily report the undelivered report, a newer failure
  notice the undelivered one), so when the channel comes back the owner gets the current state at once, not a backlog:
  that message says how many it replaced, since when, and the most severe of them, and lists every alert active now.
  A message that one channel keeps refusing (for example a mail rejected as spam) no longer holds back the others
  after 4 failed attempts. Undelivered messages are kept per channel (at most 50; when full, the lowest severity is
  dropped first and the next message says how many were dropped). The alert level itself is kept in the state
  regardless, so an active EMERGENCY is sent again within the hour.
- A one-off step in usage (a package upgrade, a download) is not reported as accelerating growth: the growth must
  show in both halves of the last 24 h. It still counts in the usage levels and the time-to-full estimate.
- Messages are short, in English with a severity tag (`[EMERGENCY]`, `[CRITICAL]`, `[WARNING]`, `[RECOVERED]`,
  `[INFO]`, `[REPORT]`, `[MONITOR FAILED]`, `[TEST]`) and one Arabic line, and name the server (`SERVER_NAME`, the
  hostname and `SERVER_IP`). They never contain a secret. Example:

  ```text
  [CRITICAL] PoCol disk monitor: founder-1 [srv1] (108.165.128.89)
  - CRITICAL: disk / 86.2% used, 4.7 GB free of 34.3 GB
  - CRITICAL: disk / full in about 2.8 days at +1.7 GB/day
  /: 86.2% used, 4.7 GB free, growth +1.7 GB/day (24 h), full in about 2.8 days; PoCol data 27.9 GB
  The monitor deletes nothing and stops nothing; disk expansion or any data action needs a separate owner decision (D7).
  Time: 2026-12-10 14:15 UTC
  PoCol: حرج - founder-1 [srv1] (108.165.128.89)
  ```

  Sizes are decimal (1 GB = 10^9 bytes).

### The daily report

Once a day, at the first check at or after `DAILY_REPORT_HOUR` (08:00 by default) in `DAILY_REPORT_TZ` (`UTC` by
default; `local` uses the server's time zone), each server sends a `[REPORT]` with, for every filesystem, its use,
free space, inodes, growth over 1 h, 24 h and 7 d and the estimated days to full; the PoCol data size by part and its
growth; the active alerts; and the monitor's own health (last check, messages waiting, configuration warnings). If the
host was off at 08:00 the report follows at its first check that day. **A missing daily report means the monitor or
the host is down.**

### When the monitor itself fails

- **The unit fails** (an error in a check): `OnFailure=` starts `pocol-disk-monitor-failure.service`, which sends a
  `[MONITOR FAILED]` alert (queued for the next successful check if no channel answers). While the check keeps
  failing it is repeated every `REALERT_MONITOR_FAILED_HOURS` (1 h by default) as `STILL FAILING` with the count of
  failed checks, not at every failed check; the next successful check sends `MONITOR RECOVERED`. A byte in
  `monitor.conf` or `secret.env` that is not UTF-8 drops only that line (reported as a warning, without its content).
- **The timer was stopped or the host was down**: the next check reports a `MONITORING GAP` with the time since the
  last successful check. `Persistent=true` runs a missed check at boot.
- **The host or its network is down**: nothing on the host can report that. Use the optional dead-man heartbeat:
  every successful check requests the owner's `HEARTBEAT_URL` (a healthchecks.io-compatible ping URL; with messages
  still undelivered it requests `URL/fail`), and the heartbeat service alerts the owner when the pings stop. Without
  it, the missing daily report is the signal.
- **The clock steps** (a wrong clock before NTP sync at boot; the units start after `time-sync.target`): stored
  times later than now are reset, so re-alerts and the gap check keep working. If only the newest samples lie in the
  future they are dropped with a `CLOCK STEPPED BACK` warning; if most of the history does, the clock is taken to be
  behind: the history is kept, checks are not recorded, and a WARNING stays active until the clock is right. A clock
  that jumps far ahead for one check looks like a long gap and is reported as one.
- **The disk is so full that a new state file cannot be written**: the monitor overwrites a 256 KiB reserve copy that
  it allocated earlier, in place, so its alert history, hourly limit and report schedule survive and it does not
  repeat itself every 15 minutes. A write that comes up short never replaces the good state file.
- **After the launch** each founder can also check the other two founders' TCP port 30300 (`PEER_CHECK=1`, `PEERS`,
  off by default) and alert when a peer is unreachable for 3 consecutive checks.

## What it never does

It only reads and notifies. It never deletes, prunes, compacts, truncates or moves any data (blockchain data
included), never starts, stops or restarts PoCol or any other service, never changes a firewall rule and runs no
external command. It writes only in its own state directory, `/var/lib/pocol-disk-monitor`: the state file
`state.json`, its temporary file `state.json.tmp`, the lock `lock`, the fixed-size reserve copy `state.reserve`, and,
when the state file is corrupt, the set-aside `state.json.corrupt`. Any action on the data or on the disk needs a separate owner decision (D7).
`test_disk_monitor.py` (class `NothingDestructive`) checks this on the source (imports, calls, the only write paths)
and by running every command against a temporary tree that must stay unchanged; the unit's sandbox
(`ProtectSystem=strict` with `ReadWritePaths` only on the state directory) enforces it on the server.

## Resource use

A check is a short Python process at the lowest priority: `Nice=19`, `IOSchedulingClass=idle`, `CPUWeight=20`,
`CPUQuota=20%`, `MemoryMax=64M`, `TasksMax=16`. Measured on a development host with `/usr/bin/time -v` and
`/usr/bin/python3.12` (Ubuntu 24.04's interpreter) on 2026-11-21: one `--once` with no message to send took about
0.1 s of wall time (0.07 s of CPU) with a maximum resident set of about 23 MB; one with a full 8-day history (270
samples) and two Telegram deliveries over TLS to a local test server took 0.11 to 0.13 s and about 26 MB. The state file is
about 37 KB with a full history; the reserve copy takes a fixed 256 KiB. These are local measurements, not measurements on the founder servers.

## Setting up the channels

Configure at least one channel; both are better. Secrets live only in `/etc/pocol-disk-monitor/secret.env`
(root:root, mode 0600), written by the owner on each server with `sudoedit`; never in Git, a chat or this repository.
The repository ships only `secret.env.example` with `REPLACE_ME` placeholders.

### Telegram

1. In Telegram, open **@BotFather**, send `/newbot`, choose a name and a user name. BotFather replies with the bot
   token (`123456789:AA...`). Keep it secret.
2. Open a chat with the new bot and send it any message (a bot cannot write to someone who has not written to it).
3. Find the chat id: on the owner's own computer, open `https://api.telegram.org/bot<TOKEN>/getUpdates` in a private
   browser window and read `"chat":{"id":...}` (a number; negative for a group). Close the window afterwards: the URL
   holds the token.
4. On each server: `sudoedit /etc/pocol-disk-monitor/secret.env` and set `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID`.

One bot and one chat can serve all three servers; every message names its server. The monitor calls
`https://api.telegram.org/bot<TOKEN>/sendMessage` with certificate verification always on.

### Email

Use an SMTP account that can send mail (a provider's SMTP relay or an app password of a mailbox). In
`/etc/pocol-disk-monitor/monitor.conf` set `SMTP_HOST`, `SMTP_PORT` (587 with `SMTP_SECURITY=starttls`, or 465 with
`SMTP_SECURITY=ssl`), `SMTP_USER`, `SMTP_FROM` and `SMTP_TO` (comma separated); put `SMTP_PASSWORD` in `secret.env`.
The connection is always encrypted and the server's certificate verified; a server that offers no STARTTLS is refused,
never used in plain text.

### Heartbeat (optional, recommended)

Create a check at a healthchecks.io-compatible service with a 15-minute period and a grace time of about 30 minutes,
and put its ping URL (https only) in `secret.env` as `HEARTBEAT_URL`. Use one check per server.

## Install (the owner, on each founder server)

Approval first: this is a deployment step on the founder servers and needs the owner's explicit approval. On each of
`108.165.128.89`, `108.165.128.90` and `108.165.128.91`, as the admin user `orvix47` with sudo:

1. Copy this directory from a checkout of the reviewed commit to the server, for example from the owner's PC:
   `scp -r deploy/monitoring orvix47@108.165.128.89:pocol-disk-monitor` (it holds no secret). `.gitattributes`
   keeps its files LF on every checkout, Windows included.
2. On the server: `cd ~/pocol-disk-monitor && sudo bash ./install-disk-monitor.sh` (`bash ./...` also runs a copy
   that lost its executable bit on the way from a Windows PC). The installer refuses to run without sudo, checks
   `/usr/bin/python3` (the interpreter the units run), installs the script to `/opt/pocol-disk-monitor/`, the units to `/etc/systemd/system/`, creates
   `/var/lib/pocol-disk-monitor` (0700) and, only if they do not exist yet, `/etc/pocol-disk-monitor/monitor.conf`
   (0644) and `/etc/pocol-disk-monitor/secret.env` (0600, placeholders); it enables and starts the timer. Run again,
   it upgrades the script and the units and keeps both files. It does not touch PoCol's install, data, unit or the
   firewall.
3. `sudoedit /etc/pocol-disk-monitor/monitor.conf`: set `SERVER_NAME` (`founder-1`, `founder-2`, `founder-3`) and
   `SERVER_IP` (the server's address), and the SMTP settings if email is used. Once PoCol is installed, set
   `POCOL_DATA_REQUIRED=1`.
4. `sudoedit /etc/pocol-disk-monitor/secret.env`: the channel secrets (above).
5. Run the acceptance test below.

`sudo python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --status` prints the current measurements and which
channels are configured, without secrets and without sending. `--report` sends the daily report now (`--report
--print-only` only prints it).

To remove it: `sudo /opt/pocol-disk-monitor/uninstall-disk-monitor.sh` (keeps the configuration and the
measurements), or `... --purge` (removes them too). It removes only the monitor's own files.

## Acceptance test (before the network is opened to the public)

On each of the three servers, and recorded with the date and the server:

1. `sudo python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --test-alert` exits 0, prints `delivered` for
   every configured channel (and `heartbeat: ok` if one is set), and the `[TEST]` notification is **received** in
   Telegram and/or the mailbox. A non-zero exit names the channel that failed.
2. `sudo python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --simulate-usage 96` exits 0 and a
   `[TEST] [EMERGENCY]` message for 96% is received. It runs the whole alert path with the simulated value and leaves
   the real state unchanged.
3. `systemctl list-timers pocol-disk-monitor.timer` shows the next run within 15 minutes, and
   `journalctl -u pocol-disk-monitor.service -n 5` shows `ok:` lines from the timer's checks.
4. A forced failure triggers the failure alert:
   `sudo systemctl set-environment POCOL_DISK_MONITOR_FORCE_FAILURE=1`,
   `sudo systemctl start pocol-disk-monitor.service` (it fails), then
   `sudo systemctl unset-environment POCOL_DISK_MONITOR_FORCE_FAILURE` and
   `sudo systemctl reset-failed pocol-disk-monitor.service`. A `[MONITOR FAILED]` alert is received. The forced
   failure measures and writes nothing.
5. With a heartbeat: the heartbeat service shows the server's pings.

The network is opened to the public only after all three servers pass (D7). The daily report of the next morning is a
further check that the timer runs.

## Files

| File | Installed as |
| --- | --- |
| `pocol_disk_monitor.py` | `/opt/pocol-disk-monitor/pocol_disk_monitor.py` |
| `systemd/pocol-disk-monitor.service` | `/etc/systemd/system/` (one check, hardened, low priority) |
| `systemd/pocol-disk-monitor.timer` | `/etc/systemd/system/` (`OnCalendar=*:0/15`, `Persistent=true`) |
| `systemd/pocol-disk-monitor-failure.service` | `/etc/systemd/system/` (the `OnFailure=` notifier) |
| `monitor.conf.example`, `secret.env.example` | `/etc/pocol-disk-monitor/` (and the first `monitor.conf`, `secret.env`) |
| `install-disk-monitor.sh`, `uninstall-disk-monitor.sh` | the uninstaller in `/opt/pocol-disk-monitor/` |

The service runs as root with a single capability, `CAP_DAC_READ_SEARCH`, which it needs only to list and stat the
PoCol data directory (0700, owned by the `pocol-standalone` user; the installer's permissions are left as they are).
Its own `secret.env` (root, 0600) needs no capability: the service runs as its owner. The capability bypasses read
permission on any file, so the units also mask what is known to hold key material, as defense in depth (the code
opens none of it): the live and the staged miner key (`identity.pocol-key`, `identity.pocol-key.restoring`),
`/etc/credstore.encrypted`, `/etc/credstore`, the host credential key `/var/lib/systemd/credential.secret`,
`/var/lib/pocol-faucet`, `/etc/pocol-testnet`, `/var/backups` and `/root`. Old keys kept as
`identity.pocol-key.replaced-<address>` have no fixed name, cannot be masked by the unit and stay readable to this
process, as do the home directories under `/home` (masking `/home` would hide a separate `/home` filesystem from the
measurement): move such files off the server, as KEYS.md and `encrypt-server-key.sh` advise. Everything else is sandboxed: the file system is read-only except its state directory
(`ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp`), no new privileges, private devices, no kernel or
control-group changes, a system-call filter and IPv4/IPv6/Unix/netlink sockets only (`systemd-analyze security
--offline` scores it 2.0, "OK").

## Local checks

```sh
python3 -B -m unittest discover -s deploy/monitoring -v
```

`test_disk_monitor.py` covers measurement, thresholds, hysteresis, re-alerts, recovery, growth, acceleration, the
estimate, the daily report, gap detection, the failure path and its repeat interval, the queue and retries (an outage
backlog, a message a channel keeps refusing), the state file (bounded, corrupt or damaged, short writes, a disk too
full for a new file), clock steps, configuration files that are not UTF-8, the missing PoCol directory, secrets and the commands, with fakes for the host and local Telegram,
heartbeat and SMTP servers on 127.0.0.1 (a throw-away CA made with `openssl`); no real message is sent.
`test_disk_monitor_install.py` checks the units (and runs `systemd-analyze verify` and `security --offline`), and the
installer and uninstaller (`bash -n`, `shellcheck`, and runs in a temporary root with a fake `systemctl`). A test whose
tool is missing is skipped and says so; with `POCOL_MONITOR_REQUIRE_TOOLS=1` it fails instead, as in CI
(`.github/workflows/founder-disk-monitor.yml`).
