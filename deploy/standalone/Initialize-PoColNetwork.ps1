# One-time bootstrap for the owner's three prepared PoCol hosts. Never exports a private key.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$LinuxArchive,
    [string]$SshKey = (Join-Path $env:USERPROFILE '.ssh\id_ed25519_pocol'),
    [ValidatePattern('^[a-z_][a-z0-9_-]*$')][string]$ServerUser = 'pocol-admin'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Servers = @('108.165.128.89', '108.165.128.90', '108.165.128.91')
$Here = $PSScriptRoot
$Node = Join-Path $Here 'pocol.exe'
$Daemon = Join-Path $Here 'pocold.exe'
$Invitation = Join-Path $Here 'network.pocol.json'
if (Test-Path -LiteralPath $Invitation) { throw 'A network invitation already exists here. Refusing to replace an existing network.' }
$LinuxArchive = (Resolve-Path -LiteralPath $LinuxArchive).Path
$SshKey = (Resolve-Path -LiteralPath $SshKey).Path
foreach ($Command in @('ssh.exe', 'scp.exe', 'tar.exe')) { Get-Command $Command -ErrorAction Stop | Out-Null }
foreach ($Name in @('pocol.exe','pocold.exe','pocol-key.exe','pocol-sign.exe','pocol-site.exe','PoCol-Wallet.exe')) {
    if (!(Test-Path -LiteralPath (Join-Path $Here $Name) -PathType Leaf)) { throw "Missing $Name" }
}
foreach ($Line in [IO.File]::ReadAllLines((Join-Path $Here 'SHA256SUMS'), [Text.Encoding]::UTF8)) {
    if ($Line -cnotmatch '^([0-9a-f]{64})  ([A-Za-z0-9._-]+)$') { throw 'Invalid checksum manifest' }
    $Expected = $Matches[1]; $File = Join-Path $Here $Matches[2]
    if ((Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Expected) { throw "Checksum mismatch: $File" }
}
$WindowsBuild = (& $Node build-info | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Windows build identity' }
$LinuxBuild = (& tar.exe -xOf $LinuxArchive build-info.json | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0 -or $LinuxBuild.compatibilitySha256 -cne $WindowsBuild.compatibilitySha256) { throw 'Linux and Windows must come from the same compatible release' }
$ArchiveHash = (Get-FileHash -LiteralPath $LinuxArchive -Algorithm SHA256).Hash.ToLowerInvariant()
$SshArgs = @('-F','none','-o','IdentitiesOnly=yes','-i',$SshKey)
$Tag = 'pocol-setup-' + [Guid]::NewGuid().ToString('N')
$Work = Join-Path $Here $Tag
New-Item -ItemType Directory -Path $Work | Out-Null
$Public = @()
Write-Host 'Experimental test coins only. The new network has no scheduled shutdown height or expiry date.'
Write-Host 'SSH may ask for your local key passphrase and each server sudo password. Enter them only in the terminal.'
Write-Host 'Verify any new SSH host fingerprint through the provider console. No host-key checks are disabled.'
foreach ($Server in $Servers) {
    $Remote = "$ServerUser@$Server"
    & ssh.exe @SshArgs $Remote "umask 077; mkdir ~/$Tag"
    if ($LASTEXITCODE -ne 0) { throw "Cannot create setup directory on $Server; no existing installation was removed" }
    & scp.exe @SshArgs $LinuxArchive "${Remote}:$Tag/release.tar.gz"
    if ($LASTEXITCODE -ne 0) { throw "Upload failed: $Server" }
    $Install = "set -eu; cd ~/$Tag; printf '%s  %s\n' '$ArchiveHash' release.tar.gz | sha256sum -c -; tar -xzf release.tar.gz; sudo bash install-linux.sh"
    & ssh.exe @SshArgs -tt $Remote $Install
    if ($LASTEXITCODE -ne 0) { throw "Installation failed on $Server. Keep $Work and inspect that host; do not delete its identity or rerun over an existing network." }
    $Address = ((& ssh.exe @SshArgs $Remote 'cat /opt/pocol-standalone/identity.address') -join '').Trim()
    if ($LASTEXITCODE -ne 0 -or $Address -cnotmatch '^0x[0-9a-f]{40}$') { throw "Cannot read public identity on $Server" }
    if ($Public -contains $Address) { throw 'Servers unexpectedly share a miner identity' }
    $Public += $Address
    Write-Host "$Server public address: $Address"
}
$Desktop = (& $Node identity | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cannot initialize the Windows identity. Stop any running PoCol instance first.' }
$ChainId = Get-Random -Minimum 777916 -Maximum 2147483000
$GenesisTime = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() + 180
$Miners = @($Public | ForEach-Object { @{ id=$_; reward=$_ } })
$Accounts = @(($Public + @($Desktop.address)) | ForEach-Object { @{ address=$_; wei='1000000000000000000000000' } })
$Config = @{ continuous=$true; chainId=$ChainId; genesisTime=$GenesisTime; registrationPerBlock=1; miners=$Miners; accounts=$Accounts }
$ConfigFile = Join-Path $Work 'public-genesis.json'
[IO.File]::WriteAllText($ConfigFile, ($Config | ConvertTo-Json -Depth 8), (New-Object Text.UTF8Encoding($false)))
$Genesis = Join-Path $Work 'genesis'
& $Daemon init-testnet --config $ConfigFile --out $Genesis
if ($LASTEXITCODE -ne 0) { throw 'Genesis admission failed; no shared network was published' }
$PackArgs = @('network','--from',$Genesis,'--out',$Invitation)
foreach ($Server in $Servers) { $PackArgs += @('--seed', "${Server}:30300") }
& $Node @PackArgs
if ($LASTEXITCODE -ne 0) { throw 'Network invitation creation failed' }
$NetworkHash = (Get-FileHash -LiteralPath $Invitation -Algorithm SHA256).Hash.ToLowerInvariant()
foreach ($Server in $Servers) {
    $Remote = "$ServerUser@$Server"
    & scp.exe @SshArgs $Invitation "${Remote}:$Tag/network.pocol.json"
    if ($LASTEXITCODE -ne 0) { throw "Network file upload failed: $Server. Keep the SAME invitation; do not generate another genesis." }
    $Start = "set -eu; cd ~/$Tag; printf '%s  %s\n' '$NetworkHash' network.pocol.json | sha256sum -c -; sudo test ! -e /opt/pocol-standalone/network.pocol.json; sudo install -m 0644 network.pocol.json /opt/pocol-standalone/network.pocol.json; sudo ufw allow 30300/tcp; sudo systemctl start pocol-standalone.service; sudo systemctl --no-pager status pocol-standalone.service"
    & ssh.exe @SshArgs -tt $Remote $Start
    if ($LASTEXITCODE -ne 0) { throw "Node startup failed: $Server. Inspect its service log and retain the SAME invitation." }
}
Write-Host "Genesis time (UTC): $([DateTimeOffset]::FromUnixTimeSeconds($GenesisTime).UtcDateTime.ToString('u'))"
Write-Host "Import this SAME public file in the Android app: $Invitation"
Write-Host 'The three server identities are initial miners. Windows and Android join as full validating nodes; mining requires the existing on-chain registration.'
Write-Host 'Once installed, operation does not need this script or an assistant. Keep servers running.'
Start-Process -FilePath (Join-Path $Here 'PoCol-Wallet.exe') -WorkingDirectory $Here
