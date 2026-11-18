# One-time genesis ceremony of the fresh v3 network (FN-7; development/decisions/FRESH-NETWORK-CP.md): installs PoCol
# on three freshly provisioned Linux servers, collects only their public addresses, creates this PC's own fresh
# identity, fills the committed network profile (v3-rehearsal-profile.json or v3-profile.json, shipped and checksummed
# in this package) and admits its genesis with pocold init-testnet. Never exports, copies or forwards a private key.
#
# The four founders are the three servers (founders 1-3, in the order of -Servers) and this PC (founder 4); each key
# is generated on its own device. Founders 1-3 are paid to the owner-held reward addresses -ServerRewardAddresses
# (owner decision 2026-11-16): made offline with pocol-key on another device, never on a server, they receive the
# 999,980 PCOL allocations and every mining reward; each server keeps only its miner key and its 20 PCOL bond in Escrow.
# Founder 4 (this PC) is its own reward address. The chain id is the profile's (CP section 5): -Network Rehearsal 1656550731, -Network Mainnet
# 2656089850. The rehearsal and the mainnet are separate ceremonies with separate fresh keys: before the mainnet run,
# the servers are re-provisioned (install-linux.sh refuses an existing installation), the package is extracted into a
# new directory, and this PC's rehearsal home (%LOCALAPPDATA%\PoCol) is moved aside after its key is backed up.
#
# -DryRun runs only the local steps, for a test or a review: no SSH, no server, no %LOCALAPPDATA%\PoCol, no wallet.
# Its founders are four throwaway identities in the setup folder, and pocold init-testnet --test-placeholders fills
# the launch second and moves the genesis to the local test chain id, so a dry run is always a TEST GENESIS.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][ValidateSet('Rehearsal','Mainnet')][string]$Network,
    # The three servers' public IPv4 addresses. P2P stays on TCP 30300: install-linux.sh's service listens there.
    [Parameter(Mandatory=$true)][ValidateCount(3,3)][string[]]$Servers,
    # The owner-held reward addresses of founders 1-3, in -Servers order (pocol-key address output, lower-case 0x hex).
    # Required for a real run; a dry run without them uses three throwaway reward identities.
    [ValidateCount(3,3)][string[]]$ServerRewardAddresses,
    # The founders' measured aggregate hash rate in H/s (R25d; the rehearsal's networkHashrateEst feeds the mainnet).
    [Parameter(Mandatory=$true)][uint64]$HashesPerSecond,
    # The synchronized launch second (unix time), or a delay from the moment the genesis is created. Exactly one.
    [uint64]$GenesisTime = 0,
    [ValidateRange(0, 86400)][int]$LaunchDelaySeconds = 0,
    [string]$LinuxArchive,
    [string]$SshKey = (Join-Path $env:USERPROFILE '.ssh\id_ed25519_pocol'),
    [ValidatePattern('^[a-z_][a-z0-9_-]*$')][string]$ServerUser = 'pocol-admin',
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# The committed profiles and the chain ids FRESH-NETWORK-CP.md section 5 drew for them; the profile's own id must match.
$Networks = @{
    Rehearsal = @{ Profile = 'v3-rehearsal-profile.json'; ChainId = [uint64]1656550731; Name = 'PoCol rehearsal' }
    Mainnet   = @{ Profile = 'v3-profile.json';           ChainId = [uint64]2656089850; Name = 'PoCol Mainnet' }
}
# The ceremony inputs of a profile, by JSON pointer: every placeholder it has, and nothing else is filled.
$Ceremony = @('/genesisTime', '/hashRate/hashesPerSecond',
    '/founders/0/id', '/founders/0/reward', '/founders/1/id', '/founders/1/reward',
    '/founders/2/id', '/founders/2/reward', '/founders/3/id', '/founders/3/reward')
# install-linux.sh's service listens on this port on every interface; the invitation's seeds and the firewall rule use it.
$P2pPort = 30300
# init-testnet's summary must report the experimental admission (design C8) and must not report a non-release build.
$RequiredBlockers = @('admission-scope', 'profile-kind')
$RefusedBlockers = @('consensus-unpinned', 'test-features', 'spec-version', 'v3-inactive')
# The genesis is created at least this long before its launch second: the invitation still has to reach three servers
# (each asks for its sudo password) and this PC's wallet has to start. Allow more for a slow operator.
$MinLeadSeconds = 300
$TestChainId = [uint64]777990

$Selected = $Networks[$Network]
$Here = $PSScriptRoot
$Node = Join-Path $Here 'pocol.exe'
$Daemon = Join-Path $Here 'pocold.exe'
$Invitation = Join-Path $Here 'network.pocol.json'
$ProfileFile = Join-Path $Here $Selected.Profile
$DesktopHome = Join-Path $env:LOCALAPPDATA 'PoCol'

function Now { [uint64][DateTimeOffset]::UtcNow.ToUnixTimeSeconds() }
function Get-Pointer($Object, [string]$Pointer) {
    $Value = $Object
    foreach ($Part in $Pointer.TrimStart('/').Split('/')) {
        if ($Part -cmatch '^[0-9]+$') { $Value = @($Value)[[int]$Part] } else { $Value = $Value.$Part }
    }
    return $Value
}
# Fills the given pointers of the profile's text in place, so every other byte stays the committed profile's. Each value
# replaces exactly one occurrence of its placeholder string (JSON numbers bare, addresses quoted).
function Set-Ceremony([string]$Text, $Values) {
    $Parsed = $Text | ConvertFrom-Json
    foreach ($Pointer in @($Values.Keys)) {
        if ($Ceremony -cnotcontains $Pointer) { throw "Not a ceremony input: $Pointer" }
        $Placeholder = Get-Pointer $Parsed $Pointer
        if ($Placeholder -isnot [string] -or !$Placeholder.StartsWith('PLACEHOLDER') -or $Placeholder.IndexOfAny([char[]]'"\') -ge 0) {
            throw "The profile has no plain placeholder at $Pointer"
        }
        $Quoted = '"' + $Placeholder + '"'
        if ([regex]::Matches($Text, [regex]::Escape($Quoted)).Count -ne 1) { throw "The placeholder at $Pointer is not unique" }
        $Text = $Text.Replace($Quoted, [string]$Values[$Pointer])
    }
    return $Text
}
function Assert-Address([string]$Address, [string]$Who) {
    if ($Address -cnotmatch '^0x[0-9a-f]{40}$') { throw "Cannot read the public address of $Who" }
    return $Address
}
# A genesis reward account the node accepts (pocold testnet.rs allowed): not zero, not a system address
# 0x...c0c001-0x...c0c0ff (PAY 0x...c0c0ff and Escrow 0x...c0c002 among them), not 0xff..fe.
function Assert-RewardAddress([string]$Address, [string]$Who) {
    if ($Address -cnotmatch '^0x[0-9a-f]{40}$') { throw "The reward address of $Who is not a lower-case 0x address of 40 hex digits (pocol-key address output): $Address" }
    if ($Address -ceq ('0x' + ('0' * 40)) -or $Address -ceq ('0x' + ('f' * 38) + 'fe')) { throw "The reward address of $Who is a reserved address: $Address" }
    if ($Address.Substring(2, 34) -ceq ('0' * 34)) {
        $Low = [Convert]::ToInt32($Address.Substring(36), 16)
        if ($Low -ge 0xc0c001 -and $Low -le 0xc0c0ff) { throw "The reward address of $Who is a system address: $Address" }
    }
    return $Address
}

# Parameters, before anything is touched.
foreach ($Server in $Servers) {
    $Ip = $null
    if (![Net.IPAddress]::TryParse($Server, [ref]$Ip) -or $Ip.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork -or $Ip.ToString() -cne $Server) {
        throw "Not an IPv4 address in dotted form: $Server"
    }
}
if (@($Servers | Sort-Object -Unique).Count -ne 3) { throw 'The three servers must be distinct' }
if ($HashesPerSecond -lt 1 -or $HashesPerSecond -gt [uint64]9007199254740991) { throw 'HashesPerSecond is the measured aggregate rate in H/s (1 to 2^53-1)' }
if (($GenesisTime -eq 0) -eq ($LaunchDelaySeconds -eq 0)) { throw 'Give exactly one of -GenesisTime (unix seconds) and -LaunchDelaySeconds' }
if ($LaunchDelaySeconds -ne 0 -and $LaunchDelaySeconds -lt $MinLeadSeconds) { throw "LaunchDelaySeconds is at least $MinLeadSeconds" }
if ($GenesisTime -ne 0 -and $GenesisTime -lt (Now) + $MinLeadSeconds) { throw "GenesisTime must be at least $MinLeadSeconds seconds in the future" }
if (!$DryRun -and !$LinuxArchive) { throw 'LinuxArchive is required (the Linux package of this same release)' }
# Reward addresses before any server is touched: install-linux.sh refuses an existing installation, so a refusal from
# init-testnet after provisioning would force a re-provision.
if (!$DryRun -and !$ServerRewardAddresses) { throw 'ServerRewardAddresses is required: the three owner-held reward addresses of founders 1-3 (pocol-key address output), in -Servers order' }
if ($ServerRewardAddresses) {
    for ($i = 0; $i -lt 3; $i++) { Assert-RewardAddress $ServerRewardAddresses[$i] "founder $($i + 1)" | Out-Null }
    if (@($ServerRewardAddresses | Sort-Object -Unique).Count -ne 3) { throw 'The three server reward addresses must be distinct (each founder has its own genesis reward account)' }
}

# This package: checksums (the profiles included), binaries, build identity, profile.
if (Test-Path -LiteralPath $Invitation) { throw 'A network invitation already exists here. Refusing to replace an existing network.' }
foreach ($Name in @('pocol.exe','pocold.exe','pocol-key.exe','pocol-sign.exe','pocol-site.exe','PoCol-Wallet.exe')) {
    if (!(Test-Path -LiteralPath (Join-Path $Here $Name) -PathType Leaf)) { throw "Missing $Name" }
}
$Checked = @()
foreach ($Line in [IO.File]::ReadAllLines((Join-Path $Here 'SHA256SUMS'), [Text.Encoding]::UTF8)) {
    if ($Line -cnotmatch '^([0-9a-f]{64})  ([A-Za-z0-9._-]+)$') { throw 'Invalid checksum manifest' }
    $Expected = $Matches[1]; $Checked += $Matches[2]; $File = Join-Path $Here $Matches[2]
    if ((Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Expected) { throw "Checksum mismatch: $File" }
}
if ($Checked -cnotcontains $Selected.Profile) { throw "$($Selected.Profile) is not covered by SHA256SUMS" }
$ProfileText = [IO.File]::ReadAllText($ProfileFile, [Text.Encoding]::UTF8)
$Parsed = $ProfileText | ConvertFrom-Json
if ([uint64]$Parsed.chainId -ne $Selected.ChainId) { throw "$($Selected.Profile) does not carry the $Network chain id $($Selected.ChainId)" }
if ($Parsed.v3 -ne $true -or [int]$Parsed.registrationPerBlock -ne [int]$Parsed.cp.R_max -or @($Parsed.founders).Count -ne 4) {
    throw "$($Selected.Profile) is not a v3 profile with four founders and registrationPerBlock = R_max"
}
foreach ($Gone in @('miners', 'accounts')) { if ($Parsed.PSObject.Properties.Name -contains $Gone) { throw "A profile has no $Gone" } }
if ([regex]::Matches($ProfileText, '"PLACEHOLDER').Count -ne $Ceremony.Count) { throw 'The profile has placeholders other than the ceremony inputs' }
$AttestationWindow = [uint64]$Parsed.cp.D_att
$WindowsBuild = (& $Node build-info | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Windows build identity' }
if ($WindowsBuild.consensusPinned -ne $true) { throw 'This is not a release build (consensusPinned is not true)' }
if (!$DryRun) {
    $LinuxArchive = (Resolve-Path -LiteralPath $LinuxArchive).Path
    $SshKey = (Resolve-Path -LiteralPath $SshKey).Path
    foreach ($Command in @('ssh.exe', 'scp.exe', 'tar.exe', 'w32tm.exe')) { Get-Command $Command -ErrorAction Stop | Out-Null }
    $LinuxBuild = (& tar.exe -xOf $LinuxArchive build-info.json | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0 -or $LinuxBuild.compatibilitySha256 -cne $WindowsBuild.compatibilitySha256) { throw 'Linux and Windows must come from the same compatible release' }
    $ArchiveHash = (Get-FileHash -LiteralPath $LinuxArchive -Algorithm SHA256).Hash.ToLowerInvariant()
    # Founder 4 is a fresh key in the wallet's fixed home. An existing home holds another key and network binding.
    if (Test-Path -LiteralPath $DesktopHome) {
        throw "$DesktopHome already exists. Back up its key (pocol.exe export-key --out FILE), close PoCol, and move the folder aside (never delete it); then run this again from a freshly extracted package."
    }
    # This PC's clock sets the launch second and the lead and D_att lateness checks, so it must be NTP-synchronized
    # like the servers'. A running Windows Time service is not enough: its source can be the local or a free-running clock.
    $TimeSource = ((& w32tm.exe /query /source) -join '').Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Windows Time is not running. Configure an NTP peer first (every founder within 0.5 s, CP section 4).' }
    if ($TimeSource -eq '' -or $TimeSource -match 'Local CMOS Clock|Free-running System Clock') {
        throw "This PC's clock is not NTP-synchronized (Windows Time source: '$TimeSource'). Configure an NTP peer first (every founder within 0.5 s, CP section 4)."
    }
    $TimePeer = ($TimeSource -split ',')[0].Trim()
    $Chart = (& w32tm.exe /stripchart "/computer:$TimePeer" /samples:1 /dataonly) -join "`n"
    $Offset = [regex]::Match($Chart, '(?m),\s*([+-]\d+[.,]\d+)s\s*$')
    if ($LASTEXITCODE -ne 0 -or !$Offset.Success) { throw "Cannot measure this PC's clock offset against its NTP peer $TimePeer. Make sure the peer answers, then run this again." }
    $OffsetSeconds = [double]::Parse($Offset.Groups[1].Value.Replace(',', '.'), [Globalization.CultureInfo]::InvariantCulture)
    if ([Math]::Abs($OffsetSeconds) -gt 0.5) { throw "This PC's clock is $OffsetSeconds s off its NTP peer $TimePeer (at most 0.5 s, CP section 4). Run w32tm /resync, then run this again." }
    Write-Host "This PC's clock: $OffsetSeconds s off its NTP peer $TimePeer"
}

$Tag = 'pocol-setup-' + [Guid]::NewGuid().ToString('N')
$Work = Join-Path $Here $Tag
New-Item -ItemType Directory -Path $Work | Out-Null
Write-Host "Creates the $($Selected.Name) genesis (chain id $($Selected.ChainId)) with four founders: the three servers and this PC."
Write-Host 'Experimental admission (a Simnet profile in ExperimentalDevnet scope): the network runs publicly and permissionlessly, and reports that it is not public-launch-ready.'
if ($DryRun) {
    Write-Host 'DRY RUN: no server is contacted and no wallet home is created; the genesis is a TEST GENESIS on the local test chain id.'
} else {
    Write-Host 'SSH may ask for your local key passphrase and each server sudo password. Enter them only in the terminal.'
    Write-Host "Reward addresses of founders 1-3 (compare them with your offline pocol-key output): $($ServerRewardAddresses -join ', ')"
    Write-Host 'Verify any new SSH host fingerprint through the provider console. No host-key checks are disabled.'
}

# Founders 1-3: a fresh identity generated on each server by install-linux.sh; only its public address leaves the host.
$Founders = @()
if ($DryRun) {
    for ($i = 1; $i -le 3; $i++) {
        $Identity = (& $Node identity --home (Join-Path $Work "dry-run-founder-$i") | ConvertFrom-Json)
        if ($LASTEXITCODE -ne 0) { throw "Cannot create dry-run founder $i" }
        $Founders += Assert-Address $Identity.address "dry-run founder $i"
    }
} else {
    $SshArgs = @('-F','none','-o','IdentitiesOnly=yes','-i',$SshKey)
    foreach ($Server in $Servers) {
        $Remote = "$ServerUser@$Server"
        & ssh.exe @SshArgs $Remote "umask 077; test `$(timedatectl show -p NTPSynchronized --value) = yes || { echo 'the clock is not NTP-synchronized' >&2; exit 3; }; mkdir ~/$Tag"
        if ($LASTEXITCODE -ne 0) { throw "Cannot prepare $Server (its clock must be NTP-synchronized); no existing installation was removed" }
        & scp.exe @SshArgs $LinuxArchive "${Remote}:$Tag/release.tar.gz"
        if ($LASTEXITCODE -ne 0) { throw "Upload failed: $Server" }
        $Install = "set -eu; cd ~/$Tag; printf '%s  %s\n' '$ArchiveHash' release.tar.gz | sha256sum -c -; tar -xzf release.tar.gz; sudo bash install-linux.sh"
        & ssh.exe @SshArgs -tt $Remote $Install
        if ($LASTEXITCODE -ne 0) { throw "Installation failed on $Server. Keep $Work and inspect that host; do not delete its identity or rerun over an existing network." }
        $Address = ((& ssh.exe @SshArgs $Remote 'cat /opt/pocol-standalone/identity.address') -join '').Trim()
        if ($LASTEXITCODE -ne 0) { throw "Cannot read public identity on $Server" }
        $Founders += Assert-Address $Address $Server
        Write-Host "$Server public address: $Address"
    }
}
# Founder 4: this PC's fresh identity, in the home the wallet opens (a throwaway one in a dry run).
if ($DryRun) {
    $Desktop = (& $Node identity --home (Join-Path $Work 'dry-run-founder-4') | ConvertFrom-Json)
} else {
    $Desktop = (& $Node identity | ConvertFrom-Json)
}
if ($LASTEXITCODE -ne 0) { throw 'Cannot initialize the Windows identity. Stop any running PoCol instance first.' }
$Founders += Assert-Address $Desktop.address 'this PC'
if (@($Founders | Sort-Object -Unique).Count -ne 4) { throw 'Founders unexpectedly share an identity' }
Write-Host "This PC's public address (founder 4): $($Founders[3])"
# The reward addresses: founders 1-3 the owner-held ones (throwaway identities in a dry run without them), founder 4
# this PC's own address.
$Rewards = @()
if ($ServerRewardAddresses) {
    $Rewards = @($ServerRewardAddresses)
} else {
    for ($i = 1; $i -le 3; $i++) {
        $Identity = (& $Node identity --home (Join-Path $Work "dry-run-reward-$i") | ConvertFrom-Json)
        if ($LASTEXITCODE -ne 0) { throw "Cannot create dry-run reward identity $i" }
        $Rewards += Assert-RewardAddress $Identity.address "dry-run founder $i"
    }
}
$Rewards += $Founders[3]
for ($i = 0; $i -lt 3; $i++) {
    if ($Founders -ccontains $Rewards[$i]) { throw "The reward address of founder $($i + 1) is a founder's miner identity: give the owner-held address" }
    Write-Host "Founder $($i + 1) reward address (owner-held): $($Rewards[$i])"
}
if (@($Rewards | Sort-Object -Unique).Count -ne 4) { throw 'The four reward addresses must be distinct' }

# The profile with its ceremony inputs. Founders 1-3 are paid to the owner-held reward addresses (they receive the
# 999,980 PCOL allocations and every mining reward); each server keeps only its key, and its 20 PCOL HELD bond sits in
# Escrow. Founder 4 (this PC) is its own reward address.
if ($GenesisTime -eq 0) { $GenesisTime = (Now) + [uint64]$LaunchDelaySeconds }
if ($GenesisTime -lt (Now) + $MinLeadSeconds) { throw "Less than $MinLeadSeconds seconds remain before the launch second; nothing was published. Re-provision the servers and choose a later -GenesisTime." }
$Values = [ordered]@{ '/hashRate/hashesPerSecond' = $HashesPerSecond }
for ($i = 0; $i -lt 4; $i++) { $Values["/founders/$i/id"] = '"' + $Founders[$i] + '"'; $Values["/founders/$i/reward"] = '"' + $Rewards[$i] + '"' }
# A dry run leaves the launch second to --test-placeholders, which also moves it to the local test chain id.
if (!$DryRun) { $Values['/genesisTime'] = $GenesisTime }
$Config = Set-Ceremony $ProfileText $Values
$Filled = $Config | ConvertFrom-Json
if (!$DryRun -and ($Config.Contains('PLACEHOLDER') -or [uint64]$Filled.genesisTime -ne $GenesisTime)) { throw 'The ceremony config still has a placeholder' }
if ([uint64]$Filled.hashRate.hashesPerSecond -ne $HashesPerSecond -or [uint64]$Filled.chainId -ne $Selected.ChainId) { throw 'The ceremony config was not filled as given' }
for ($i = 0; $i -lt 4; $i++) {
    if ($Filled.founders[$i].id -cne $Founders[$i] -or $Filled.founders[$i].reward -cne $Rewards[$i]) { throw "Founder $($i + 1) was not filled as given" }
}
$ConfigFile = Join-Path $Work $Selected.Profile
[IO.File]::WriteAllText($ConfigFile, $Config, (New-Object Text.UTF8Encoding($false)))

# Genesis admission. The summary on stdout carries the readiness (publicLaunchReady and every launch blocker).
$Genesis = Join-Path $Work 'genesis'
$TestFlag = @(); if ($DryRun) { $TestFlag = @('--test-placeholders') }
$Summary = (& $Daemon init-testnet --config $ConfigFile --out $Genesis @TestFlag) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'Genesis admission failed; no shared network was published' }
[IO.File]::WriteAllText((Join-Path $Work 'init-testnet-summary.json'), $Summary + "`n", (New-Object Text.UTF8Encoding($false)))
$Admitted = $Summary | ConvertFrom-Json
$IsTest = $Admitted.PSObject.Properties.Name -contains 'testPlaceholders'
if ($DryRun) {
    if (!$IsTest -or $Admitted.testPlaceholders -ne $true -or [uint64]$Admitted.chainId -ne $TestChainId -or [uint64]$Admitted.profileChainId -ne $Selected.ChainId) {
        throw 'The dry run did not build a test genesis on the local test chain id'
    }
} elseif ($IsTest -or [uint64]$Admitted.chainId -ne $Selected.ChainId) {
    throw 'pocold did not admit the filled profile under its own chain id'
}
if ($Admitted.scope -cne 'experimental-devnet' -or $Admitted.publicLaunchReady -isnot [bool] -or $Admitted.publicLaunchReady) {
    throw 'Unexpected admission: the fresh network is admitted in experimental-devnet scope and reports publicLaunchReady false'
}
$Blockers = @($Admitted.launchBlockers)
$Ids = @($Blockers | ForEach-Object { $_.id })
foreach ($Id in $RequiredBlockers) { if ($Ids -cnotcontains $Id) { throw "The admission does not report the $Id blocker of the experimental admission" } }
foreach ($Id in $RefusedBlockers) { if ($Ids -ccontains $Id) { throw "Refusing a genesis with the $Id blocker: use the release build of this package" } }
Write-Host "Genesis hash: $($Admitted.genesisHash)"
Write-Host 'Public launch ready: false. Launch blockers, verbatim:'
foreach ($Blocker in $Blockers) { Write-Host "  $($Blocker.id): $($Blocker.detail)" }

# The invitation: the genesis and the three servers as its seeds (the explorer's seed list comes from it, W9).
$Target = $Invitation; if ($DryRun) { $Target = Join-Path $Work 'network.pocol.json' }
$PackArgs = @('network','--from',$Genesis,'--out',$Target)
foreach ($Server in $Servers) { $PackArgs += @('--seed', "${Server}:$P2pPort") }
& $Node @PackArgs | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Network invitation creation failed' }
$NetworkHash = (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "Invitation: $Target (SHA-256 $NetworkHash)"
if ($DryRun) {
    Write-Host "DRY RUN complete. Everything it wrote is in $Work; delete that folder when done."
    return
}

foreach ($Server in $Servers) {
    if ((Now) -gt $GenesisTime + $AttestationWindow) { Write-Warning "$Server starts more than D_att ($AttestationWindow s) after the launch second: its first blocks are fallback-only." }
    $Remote = "$ServerUser@$Server"
    & scp.exe @SshArgs $Invitation "${Remote}:$Tag/network.pocol.json"
    if ($LASTEXITCODE -ne 0) { throw "Network file upload failed: $Server. Keep the SAME invitation; do not generate another genesis." }
    $Start = "set -eu; cd ~/$Tag; printf '%s  %s\n' '$NetworkHash' network.pocol.json | sha256sum -c -; sudo test ! -e /opt/pocol-standalone/network.pocol.json; sudo install -m 0644 network.pocol.json /opt/pocol-standalone/network.pocol.json; sudo ufw allow $P2pPort/tcp; sudo systemctl start pocol-standalone.service; sudo systemctl --no-pager status pocol-standalone.service"
    & ssh.exe @SshArgs -tt $Remote $Start
    if ($LASTEXITCODE -ne 0) { throw "Node startup failed: $Server. Inspect its service log and retain the SAME invitation." }
}
Write-Host "Genesis time (UTC): $([DateTimeOffset]::FromUnixTimeSeconds([int64]$GenesisTime).UtcDateTime.ToString('u'))"
if ((Now) -gt $GenesisTime + $AttestationWindow) { Write-Warning "This PC starts more than D_att ($AttestationWindow s) after the launch second." }
Write-Host 'The four founders (the three servers and this PC) are HELD members from genesis, each with its 20 PCOL bond in Escrow.'
Write-Host "The 999,980 PCOL allocations and the mining rewards of founders 1-3 go to the owner-held addresses $($Rewards[0..2] -join ', '); founder 4's go to this PC's address."
Write-Host 'Each owner-held reward account starts at nonce 1: sign its first offline transaction with pocol-sign --nonce 1.'
Write-Host 'The server keys hold no balance: send PCOL to a server address before it reactivates, deregisters or claims its bond.'
Write-Host 'Mining is off by default on this PC: in PoCol-Wallet choose Settings, Device role, Wallet and miner, turn mining on now, and keep it on.'
Write-Host "The phone is a wallet only: import this SAME public file in the Android app: $Invitation"
Write-Host "Publish the invitation's SHA-256 through a trusted channel. Website switch (in the repository, not from this package; W15/W16):"
Write-Host "  python3 deploy/websites/tools/site_identity.py --to network.pocol.json --commit RELEASE_COMMIT --gateway-host explorer.pocol.net"
Write-Host 'Once installed, operation does not need this script or an assistant. Keep servers running.'
Start-Process -FilePath (Join-Path $Here 'PoCol-Wallet.exe') -WorkingDirectory $Here
