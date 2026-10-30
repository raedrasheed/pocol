# PoCol Wallet and continuous five-device network

Run three Linux server miners, one native Windows wallet, and one native Android
wallet. Both wallets contain the same full validating Rust/P2P engine and sign
locally. Their mining switch starts/stops actual block production while validation
and synchronization continue. No assistant or central signing/control service is
needed after the one-time network setup.

This is an experimental network with valueless test currency, not a qualified
financial mainnet. The wallet is functional, not a balance simulation or an RPC-only
mockup. Its primary color is #1aa9e4; it has Wallet, Activity, Mining and Settings
screens, account transfers, receive/copy/share (Android), transaction review, and
explicit bonded miner registration. No invented fiat price, reward yield or
external assets are displayed. Activity lists transactions submitted by this
installation, with inclusion/revert status and confirmation counts.

## Continuous protocol

The new bootstrap sets `continuous: true`. Its genesis explicitly encodes all four
lifetime fields (`H_END`, `T_END`, `H_CLOSE`, `T_CLOSE`) as zero, meaning **no
scheduled closing block, final block or expiration date**. This is not a larger
hidden end height. A separate compiled system-program set keeps the registry open;
finite profiles retain the original program bytes and behavior.

Continuous profiles apply TX_CAP and TXBYTES_CAP per block, and reset the historical
log allowance at each block. Per-transaction, per-block, live-state, gas, peer and
queue protections remain. Old cumulative transaction/log limits cannot gradually
exhaust the ability to send. Existing finite networks are not silently converted.
A new shared genesis and the same updated binaries on every device are required;
never erase an existing identity or regenerate different genesis files per device.

No fixed machine can promise infinite storage. History, replay indexes and some
metadata still grow. The local memory model explicitly projects from observed
journal records plus 2048 records of headroom, rechecks as records grow, and refuses
inadequate local capacity without marking valid blocks invalid. It does not claim
an infinite-history resource bound. `POCOL_MEMORY_BUDGET_BYTES` can set the launcher's
local declared budget after provisioning resources; it does not change consensus.
The default model uses 6 GiB; a 4 GiB server with a 3 GiB service cgroup remains a
small pilot, not a worst-case capacity proof. Disk sizing, compaction and long-term
load qualification remain operational work. Numeric heights retain the format's
u64 representation; zero disables scheduled expiry, not hardware arithmetic.

## One-time setup from Windows

Extract the five-device ZIP. Review `Initialize-PoColNetwork.ps1`, open PowerShell
inside that directory, and run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Initialize-PoColNetwork.ps1 -LinuxArchive .\PoCol-linux-x86_64.tar.gz
```

The process-scoped policy exception is for this script only. It uses the existing
`pocol-admin` account, local `%USERPROFILE%\.ssh\id_ed25519_pocol`, and `-F none`
to avoid the previously broken SSH config permissions. Enter passphrases and sudo
passwords only into your local terminal. Host-key verification remains enabled;
verify new fingerprints through the provider console. No key is forwarded/copied.

The only allowed targets are the three prepared PoCol hosts:
`108.165.128.89`, `108.165.128.90`, `108.165.128.91`. The unrelated `.31` host is never
contacted. The script verifies Linux/Windows checksums and source compatibility,
creates independent miner keys on their servers, collects only public addresses,
creates one admitted continuous genesis, distributes its invitation, opens P2P TCP
30300, starts server services and opens **PoCol-Wallet.exe** on Windows. Server RPC
remains loopback-only. Existing state/identities are never overwritten. On a partial
failure, retain the setup folder and the SAME genesis; do not blindly rerun or reset.
The remote deployment path has not been exercised by CI.

Transfer the generated **network.pocol.json** to the phone. Install the ARM64 APK,
open **PoCol Wallet → Settings → Import network file**, select that file, and
connect. Windows imports the adjacent shared file automatically; Settings also
supports explicit import while disconnected. All devices use the same invitation.
Publish the invitation fingerprint through a trusted channel; a checksum is not
publisher authentication. The old f1ad7e3 bundle is superseded for new deployments.

## Everyday use

- Windows 10 22H2 x64 / Windows 11: open **PoCol-Wallet.exe** or **Start-PoCol.cmd**.
  Native WPF uses Windows' .NET Framework 4.8; no WSL, Docker, browser or Python.
  The wallet starts the engine through private anonymous process pipes, not an
  HTTP admin endpoint. Closing the application checkpoints and stops the engine.
  Automatic startup after Windows reboot is not configured.
- Android 8+ ARM64: open **PoCol Wallet** and connect. The full engine runs in a
  separate foreground-service process with a notification and Stop control. Android
  may suspend/kill it because of battery, heat or memory policy. Keep server miners
  online; mobile 24/7 availability is not promised. After a reboot, reconnect manually.
- Linux: `pocol-standalone.service` starts on reboot and restarts after failure. Its
  installer is fresh-install only and preserves Unix executable modes in the tarball.

**Send:** enter the full recipient and a decimal PCOL amount, review network,
recipient, amount and maximum fee, then explicitly confirm/sign. The Rust engine
parses up to 18 decimal places without floats. Quotes expire after 60 seconds and
can be consumed once. A quote does not broadcast. Zero/system/precompile recipients,
contract calls from the simple send screen, stale/offline states and insufficient
available balance are refused. Inclusion is not advertised as irreversible finality.

**Mining:** off by default on both wallets, including after restart. The three
server keys are genesis miners. The PC/phone must hold an eligible registered
identity to enable mining. A new identity can use **Review mining registration**,
review the current bond and maximum fee, and explicitly confirm its transaction.
Registration does not automatically enable mining. It needs funds, available
registry capacity and network inclusion. The bootstrap funds the Windows identity
with test coins; send test coins to the phone's public address to fund its bond and
fees. Turning mining off does not deregister, release the bond or stop the wallet.
Inactive/ejected identities follow existing registry rules; registration is never
silently repeated and a toggle does not bypass eligibility.

## Keys and updates

Windows uses authenticated user-bound DPAPI ciphertext and rejects raw Unix keys,
reparse points, hard links and altered ciphertext. Linux/Android retain strict
0600/0400, same-user, single-link key checks. Keys never cross the wallet UI bridge,
HTTP RPC or P2P. A home lock prevents concurrent engines. Missing, corrupt or
substituted keys fail instead of silently rotating identity.

This test build has **no portable encrypted recovery export**. Keep suitable
private device/profile backups. Android backup is disabled; uninstalling erases
identity and state. CI APKs are debug-signed for testing and a clean CI run may use
a different debug certificate. Do not uninstall an existing key-bearing app to
install a new certificate. Durable updates require an operator-retained private
release signing key; the unsigned release APK is also built. No keystore/private
key belongs in the release bundle or repository.

## Validation and remaining qualification

CI builds native Linux/Windows and the real ARM64 Android engine/JNI/APK. It runs
portable identity/invitation controls, real-socket tests, five local validating
processes, and a five-process wallet test with real transfer, one-shot review,
bonded registration, mining on/off, continued synchronization and identity/history
recovery. A compiled-contract execution test applies 20,002 blocks with real signed
transfers beyond the old closing/ending heights and old time expiry. A continuous
consensus test retains per-block caps and checks that local resource projections
grow with history. Finite-profile regression tests remain required.

The Windows build renders the actual WPF interface to a preview image with no
mock account balance. Android physical lifecycle/UX/performance and a real five-host
cross-platform soak remain unproven. Existing admission/RpcGuard/calibration/review
conditions remain in `development/public-network/PLAN.md`; no audit or financial
mainnet readiness is claimed.

```sh
cargo build --locked --release --bins --manifest-path prototypes/pocold/Cargo.toml
python deploy/standalone/test_launcher.py -v
python deploy/standalone/test_launcher.py --integration
python deploy/standalone/test_wallet.py
cargo test --locked --release --test consensus continuous_admission --manifest-path prototypes/pocold/Cargo.toml
cargo test --locked --release --lib continuous_executes --manifest-path prototypes/lp4-execution/Cargo.toml
```

Android: SDK35 / NDK27.2.12479018 / JDK17 / Gradle8.11.1,
`bash deploy/standalone/build-android.sh`. Windows wallet:
`powershell -File deploy/standalone/build-windows-wallet.ps1`.
Reproduce continuous contracts with solc **0.8.28** and
`node prototypes/lp4-execution/tests/compile_system_v1.cjs prototypes/lp4-execution/tests/data/system-continuous.json --continuous`.
The script applies a checked, explicit continuous-mode branch to the original
registry source; its original v1 artifact is unchanged.

Platform references:
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/overview/
- https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
- https://developer.android.com/develop/background-work/services/fgs/service-types
