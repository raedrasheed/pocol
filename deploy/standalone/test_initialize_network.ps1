# FN-7: runs Initialize-PoColNetwork.ps1 -DryRun in a package-like temporary folder with this tree's release binaries
# (prototypes\pocold\target\release, after build-windows-wallet.ps1 and the release cargo build) and the committed
# profiles, so pocold init-testnet --test-placeholders admits the script's filled profile; then checks the refusals.
# Contacts no server and never touches %LOCALAPPDATA%\PoCol. Run by five-device-build.yml on Windows.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$Release = Join-Path $Root 'prototypes\pocold\target\release'
# RFC 5737 documentation addresses: only written into the dry run's invitation, never contacted.
$Servers = @('192.0.2.1', '192.0.2.2', '192.0.2.3')
$Profiles = @{ Rehearsal = @('v3-rehearsal-profile.json', 1656550731); Mainnet = @('v3-profile.json', 2656089850) }
$DesktopHome = Join-Path $env:LOCALAPPDATA 'PoCol'
$DesktopHomeBefore = Test-Path -LiteralPath $DesktopHome
$Folders = @()

function Write-Sums([string]$Dir) {
    $Lines = @(Get-ChildItem -LiteralPath $Dir -File | Where-Object { $_.Name -cne 'SHA256SUMS' } | Sort-Object Name |
        ForEach-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + $_.Name })
    [IO.File]::WriteAllText((Join-Path $Dir 'SHA256SUMS'), (($Lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false)))
}
function New-Package {
    $Dir = Join-Path ([IO.Path]::GetTempPath()) ('pocol-fn7-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $Dir | Out-Null
    $script:Folders += $Dir
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Initialize-PoColNetwork.ps1') -Destination $Dir
    foreach ($Name in @('v3-profile.json', 'v3-rehearsal-profile.json')) { Copy-Item -LiteralPath (Join-Path $Root "deploy\mainnet\$Name") -Destination $Dir }
    foreach ($Name in @('pocol.exe','pocold.exe','pocol-key.exe','pocol-sign.exe','pocol-site.exe','PoCol-Wallet.exe')) {
        Copy-Item -LiteralPath (Join-Path $Release $Name) -Destination $Dir
    }
    Write-Sums $Dir
    return $Dir
}
function Invoke-DryRun([string]$Dir, [hashtable]$Arguments) {
    $Script = Join-Path $Dir 'Initialize-PoColNetwork.ps1'
    & $Script @Arguments
}
function Assert([bool]$Condition, [string]$What) { if (!$Condition) { throw "FAIL $What" } ; Write-Output "PASS $What" }
function Assert-Refused([string]$Dir, [hashtable]$Arguments, [string]$Pattern, [string]$What) {
    $Before = @(Get-ChildItem -LiteralPath $Dir -Directory).Count
    $Message = $null
    try { Invoke-DryRun $Dir $Arguments | Out-Null } catch { $Message = $_.Exception.Message }
    Assert ($null -ne $Message -and $Message -match $Pattern) "$What (refused: $Message)"
    Assert (@(Get-ChildItem -LiteralPath $Dir -Directory).Count -eq $Before) "$What leaves no setup folder"
}
function Base([string]$Network) {
    return @{ DryRun = $true; Network = $Network; Servers = $Servers; HashesPerSecond = [uint64]1000000; LaunchDelaySeconds = 900 }
}
# Three fresh throwaway reward addresses (owner decision 2026-11-16: founders 1-3 are paid to owner-held addresses); no
# real address is ever used here.
function New-RewardAddresses {
    $Dir = Join-Path ([IO.Path]::GetTempPath()) ('pocol-fn7-rewards-' + [Guid]::NewGuid().ToString('N'))
    $script:Folders += $Dir
    $List = @()
    for ($i = 1; $i -le 3; $i++) {
        $List += ((& (Join-Path $Release 'pocol.exe') identity --home (Join-Path $Dir "reward-$i")) | ConvertFrom-Json).address
    }
    return $List
}

try {
    foreach ($Run in @(@('Rehearsal', $true), @('Mainnet', $true), @('Mainnet', $false))) {
        $Network = $Run[0]; $GiveRewards = $Run[1]
        $ProfileName = $Profiles[$Network][0]; $ChainId = [uint64]$Profiles[$Network][1]
        $Dir = New-Package
        $Arguments = Base $Network
        $Given = @()
        if ($GiveRewards) { $Given = New-RewardAddresses; $Arguments.ServerRewardAddresses = $Given }
        Invoke-DryRun $Dir $Arguments | Out-Host
        $Work = @(Get-ChildItem -LiteralPath $Dir -Directory -Filter 'pocol-setup-*')
        Assert ($Work.Count -eq 1) "$Network dry run writes one setup folder"
        $Work = $Work[0].FullName
        Assert (!(Test-Path -LiteralPath (Join-Path $Dir 'network.pocol.json'))) "$Network dry run publishes no invitation beside the package"
        $Summary = [IO.File]::ReadAllText((Join-Path $Work 'init-testnet-summary.json')) | ConvertFrom-Json
        Assert ($Summary.testPlaceholders -eq $true -and [uint64]$Summary.chainId -eq 777990 -and [uint64]$Summary.profileChainId -eq $ChainId) "$Network dry run is a test genesis on the local test id, from the $ChainId profile"
        Assert ($Summary.scope -ceq 'experimental-devnet' -and $Summary.publicLaunchReady -eq $false) "$Network admission is experimental and not public-launch-ready"
        $Ids = @($Summary.launchBlockers | ForEach-Object { $_.id })
        Assert (($Ids -ccontains 'admission-scope') -and ($Ids -ccontains 'profile-kind') -and !($Ids -ccontains 'consensus-unpinned')) "$Network readiness blockers"
        $Source = [IO.File]::ReadAllText((Join-Path $Root "deploy\mainnet\$ProfileName")) | ConvertFrom-Json
        $Config = [IO.File]::ReadAllText((Join-Path $Work $ProfileName)) | ConvertFrom-Json
        Assert ([string]$Config.genesisTime -clike 'PLACEHOLDER*' -and [uint64]$Config.hashRate.hashesPerSecond -eq 1000000) "$Network dry run fills the hash rate and leaves the launch second to the test flag"
        Assert (($Config.cp | ConvertTo-Json -Compress) -ceq ($Source.cp | ConvertTo-Json -Compress) -and [uint64]$Config.chainId -eq $ChainId -and $Config.registrationPerBlock -eq $Source.cp.R_max) "$Network config keeps the committed CP, chain id and registrationPerBlock = R_max"
        $Devnet = [IO.File]::ReadAllText((Join-Path $Work 'genesis\devnet.json')) | ConvertFrom-Json
        $Founders = @($Config.founders | ForEach-Object { $_.id })
        $Rewards = @($Config.founders | ForEach-Object { $_.reward })
        Assert ($Founders.Count -eq 4 -and @($Founders | Sort-Object -Unique).Count -eq 4 -and @($Founders | Where-Object { $_ -cnotmatch '^0x[0-9a-f]{40}$' }).Count -eq 0) "$Network four distinct founders"
        Assert (@($Rewards | Sort-Object -Unique).Count -eq 4 -and $Rewards[3] -ceq $Founders[3] -and @(0..2 | Where-Object { $Founders -ccontains $Rewards[$_] }).Count -eq 0) "$Network founders 1-3 are paid to separate reward addresses, founder 4 to its own"
        if ($GiveRewards) { Assert ((@($Rewards[0..2]) -join ',') -ceq ($Given -join ',')) "$Network founders 1-3 rewards are the given owner-held addresses" }
        Assert ((@($Devnet.founders | ForEach-Object { $_.id }) -join ',') -ceq ($Founders -join ',') -and $Devnet.testPlaceholders -eq $true) "$Network genesis founders are the script's"
        Assert ((@($Devnet.founders | ForEach-Object { $_.reward }) -join ',') -ceq ($Rewards -join ',')) "$Network genesis reward addresses are the script's"
        $Invitation = [IO.File]::ReadAllText((Join-Path $Work 'network.pocol.json')) | ConvertFrom-Json
        Assert ((@($Invitation.seeds) -join ',') -ceq (($Servers | ForEach-Object { "${_}:30300" }) -join ',')) "$Network invitation seeds are the servers on 30300"
    }

    $Dir = New-Package
    $Given = Base 'Mainnet'; $Given.Servers = @('192.0.2.1', '192.0.2.2')
    Assert-Refused $Dir $Given 'Servers|argument' 'two servers'
    $Given = Base 'Mainnet'; $Given.Servers = @('192.0.2.1', '192.0.2.1', '192.0.2.3')
    Assert-Refused $Dir $Given 'distinct' 'a repeated server'
    $Given = Base 'Mainnet'; $Given.Servers = @('192.0.2.1', '2001:db8::1', '192.0.2.3')
    Assert-Refused $Dir $Given 'IPv4' 'an IPv6 server'
    $Given = Base 'Mainnet'; $Given.Servers = @('192.0.2.1', '1', '192.0.2.3')
    Assert-Refused $Dir $Given 'IPv4' 'a non-dotted server'
    $Given = Base 'Mainnet'; $Given.HashesPerSecond = [uint64]0
    Assert-Refused $Dir $Given 'HashesPerSecond' 'a zero hash rate'
    $Given = Base 'Mainnet'; $Given.GenesisTime = [uint64]([DateTimeOffset]::UtcNow.ToUnixTimeSeconds() + 3600)
    Assert-Refused $Dir $Given 'exactly one' 'both a launch second and a delay'
    $Given = Base 'Mainnet'; $Given.Remove('LaunchDelaySeconds')
    Assert-Refused $Dir $Given 'exactly one' 'neither a launch second nor a delay'
    $Given = Base 'Mainnet'; $Given.LaunchDelaySeconds = 60
    Assert-Refused $Dir $Given 'at least' 'a delay below the minimum lead'
    $Given = Base 'Mainnet'; $Given.Remove('LaunchDelaySeconds'); $Given.GenesisTime = [uint64]([DateTimeOffset]::UtcNow.ToUnixTimeSeconds() + 60)
    Assert-Refused $Dir $Given 'future' 'a launch second too close'
    $Given = Base 'Mainnet'; $Given.Network = 'Testnet'
    Assert-Refused $Dir $Given 'Network|argument' 'an unknown network'
    $Given = Base 'Mainnet'; $Given.Remove('DryRun')
    Assert-Refused $Dir $Given 'LinuxArchive' 'a real run without the Linux package'
    $Given = Base 'Mainnet'; $Given.Remove('DryRun'); $Given.LinuxArchive = (Join-Path $Dir 'missing.tar.gz')
    Assert-Refused $Dir $Given 'ServerRewardAddresses is required' 'a real run without the reward addresses'
    $Good = New-RewardAddresses
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], $Good[1])
    Assert-Refused $Dir $Given 'ServerRewardAddresses|argument' 'two reward addresses'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], $Good[1], $Good[2], $Good[0])
    Assert-Refused $Dir $Given 'ServerRewardAddresses|argument' 'four reward addresses'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], $Good[0], $Good[2])
    Assert-Refused $Dir $Given 'distinct' 'a repeated reward address'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], $Good[1].ToUpperInvariant().Replace('0X', '0x'), $Good[2])
    Assert-Refused $Dir $Given 'lower-case' 'an upper-case reward address'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], '0x12345', $Good[2])
    Assert-Refused $Dir $Given 'lower-case' 'a short reward address'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], ('0x' + ('0' * 40)), $Good[2])
    Assert-Refused $Dir $Given 'reserved' 'the zero reward address'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], ('0x' + ('0' * 34) + 'c0c0ff'), $Good[2])
    Assert-Refused $Dir $Given 'system address' 'the PAY address as a reward address'
    $Given = Base 'Mainnet'; $Given.ServerRewardAddresses = @($Good[0], ('0x' + ('0' * 34) + 'c0c002'), $Good[2])
    Assert-Refused $Dir $Given 'system address' 'the Escrow address as a reward address'

    # The profile: tampered, uncovered, or with another network's chain id.
    $File = Join-Path $Dir 'v3-profile.json'
    $Committed = [IO.File]::ReadAllText($File)
    [IO.File]::WriteAllText($File, $Committed.Replace('"D_att": 90', '"D_att": 60'))
    Assert-Refused $Dir (Base 'Mainnet') 'Checksum mismatch' 'a profile changed after packaging'
    [IO.File]::WriteAllText($File, $Committed.Replace('2656089850', '1656550731'))
    Write-Sums $Dir
    Assert-Refused $Dir (Base 'Mainnet') 'chain id' 'a mainnet profile with the rehearsal chain id'
    [IO.File]::WriteAllText($File, $Committed.Replace('"registrationPerBlock": 4', '"registrationPerBlock": 1'))
    Write-Sums $Dir
    Assert-Refused $Dir (Base 'Mainnet') 'R_max' 'registrationPerBlock other than R_max'
    [IO.File]::WriteAllText($File, $Committed)
    Write-Sums $Dir
    $Sums = Join-Path $Dir 'SHA256SUMS'
    [IO.File]::WriteAllLines($Sums, @([IO.File]::ReadAllLines($Sums) | Where-Object { $_ -notlike '*  v3-profile.json' }))
    Assert-Refused $Dir (Base 'Mainnet') 'not covered' 'a profile outside SHA256SUMS'
    Write-Sums $Dir
    New-Item -ItemType File -Path (Join-Path $Dir 'network.pocol.json') | Out-Null
    Assert-Refused $Dir (Base 'Mainnet') 'already exists' 'an existing invitation'

    Assert ((Test-Path -LiteralPath $DesktopHome) -eq $DesktopHomeBefore) 'no dry run touched the wallet home'
} finally {
    foreach ($Dir in $Folders) { Remove-Item -LiteralPath $Dir -Recurse -Force -ErrorAction SilentlyContinue }
}
