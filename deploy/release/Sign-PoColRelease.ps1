# Owner-run: sign a PoCol release directory with the owner's ssh-ed25519 release key (owner decision 2026-11-16), on the
# owner's Windows ceremony PC. deploy/release/README.md is the procedure; sign-release.sh is the Linux/macOS twin.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\release\Sign-PoColRelease.ps1 -Key <offline path>\pocol-release -Directory DIR
#
# -Key is the private key file (ssh-keygen asks for its passphrase), or its .pub when ssh-agent holds the key.
# -Directory comes from assemble_release.py: the release files and SHA256SUMS. This script
#   - refuses a key inside this repository, a public key that is not the committed allowed_signers key, an existing .sig
#     in the directory, a SHA256SUMS that is not strict or does not match the files, and any file it does not list;
#   - signs each listed file, then SHA256SUMS, with ssh-keygen -Y sign -n pocol-release (writing FILE.sig);
#   - runs Verify-PoColRelease.ps1 on the directory against the committed allowed_signers and prints the key's
#     fingerprint, to compare with the one published on pocol.net.
# It never copies, prints, converts or generates a key. -AllowedSigners is for the tests only.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Key,
    [Parameter(Mandatory=$true)][string]$Directory,
    [string]$AllowedSigners = (Join-Path $PSScriptRoot 'allowed_signers'),
    [string]$SshKeygen = 'ssh-keygen.exe'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Namespace = 'pocol-release'
$Sums = 'SHA256SUMS'
function Refuse([string]$Reason) { throw "REFUSED: $Reason" }

if (!(Get-Command $SshKeygen -ErrorAction SilentlyContinue)) { Refuse 'ssh-keygen (OpenSSH 8.2 or later) is required: pass -SshKeygen' }
if (!(Test-Path -LiteralPath $Key -PathType Leaf)) { Refuse "no key file at $Key" }
if (!(Test-Path -LiteralPath $Directory -PathType Container)) { Refuse "no release directory at $Directory" }
$Key = (Resolve-Path -LiteralPath $Key).Path
$Directory = (Resolve-Path -LiteralPath $Directory).Path
$Repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path.TrimEnd('\') + '\'
if (($Key + '\').StartsWith($Repo, [StringComparison]::OrdinalIgnoreCase)) { Refuse "the key $Key is inside the repository; keep it offline, outside any checkout" }
if ($Key.EndsWith('.pub')) { $Pub = $Key } else { $Pub = "$Key.pub" }
if (!(Test-Path -LiteralPath $Pub -PathType Leaf)) { Refuse "no public key at $Pub (keep the .pub beside the key)" }

# The key must be the committed release key, before anything is written.
# Printable ASCII and LF only, as Verify-PoColRelease.ps1 reads it: a line hidden by VT, NEL, NBSP or a BOM would be
# a comment to .NET Trim and a second key line to ssh-keygen.
$AllowedBytes = [IO.File]::ReadAllBytes($AllowedSigners)
foreach ($Byte in $AllowedBytes) {
    if ($Byte -ne 10 -and ($Byte -lt 32 -or $Byte -gt 126)) { Refuse "$AllowedSigners holds a byte that is not printable ASCII or LF" }
}
$AllowedText = [Text.Encoding]::ASCII.GetString($AllowedBytes)
$KeyLines = @($AllowedText.Split("`n") | Where-Object { $_.Trim([char]32) -ne '' -and !$_.TrimStart([char]32).StartsWith('#') })
if ($KeyLines.Count -ne 1 -or $KeyLines[0].Contains('PLACEHOLDER')) { Refuse "the owner release key is not committed in $AllowedSigners (commit its public line first, by a reviewed PR)" }
$Committed = $KeyLines[0].Trim([char]32).Split(' ')
$Given = ([IO.File]::ReadAllText($Pub)).Trim().Split(' ')
if ($Committed.Count -lt 4 -or $Given.Count -lt 2 -or $Committed[2] -cne $Given[0] -or $Committed[3] -cne $Given[1]) {
    Refuse "the key is not the one in $AllowedSigners"
}

$SumsPath = Join-Path $Directory $Sums
if (!(Test-Path -LiteralPath $SumsPath -PathType Leaf)) { Refuse 'no SHA256SUMS (run assemble_release.py first)' }
if (@(Get-ChildItem -LiteralPath $Directory -Filter '*.sig' -Force).Count -ne 0) { Refuse 'a .sig already exists in the release directory; never overwrite a signature' }
$SumsText = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($SumsPath))
if ($SumsText.Contains("`r")) { Refuse 'SHA256SUMS has CR line endings' }
if ($SumsText.Length -eq 0 -or !$SumsText.EndsWith("`n")) { Refuse 'SHA256SUMS is empty or does not end with a newline' }
# The strict rules of Verify-PoColRelease.ps1 (and sign-release.sh's parse_sums), all before the first signature:
# release file names only, each once, in ordinal order; nothing unlisted; regular files only; matching hashes.
$Rows = @()
foreach ($Line in $SumsText.Substring(0, $SumsText.Length - 1).Split("`n")) {
    if ($Line -cnotmatch '\A([0-9a-f]{64})  ([A-Za-z0-9._-]+)\z') { Refuse "SHA256SUMS has a line that is not <sha256>  <name>: $Line" }
    $Name = $Matches[2]
    if ($Name.StartsWith('.') -or $Name -ceq $Sums -or $Name.EndsWith('.sig')) { Refuse "SHA256SUMS: '$Name' is not a release file name" }
    $Rows += ,@($Matches[1], $Name)
}
$Names = @($Rows | ForEach-Object { $_[1] })
if (@($Names | Sort-Object -Unique -CaseSensitive).Count -ne $Names.Count) { Refuse 'SHA256SUMS: a name is listed twice' }
$Sorted = [string[]]$Names.Clone(); [Array]::Sort($Sorted, [StringComparer]::Ordinal)
if (($Sorted -join "`n") -cne ($Names -join "`n")) { Refuse 'SHA256SUMS: the names are not sorted (ordinal order, as assemble_release.py writes them)' }
foreach ($Entry in @(Get-ChildItem -LiteralPath $Directory -Force)) {
    if ($Entry.Name -cne $Sums -and $Names -cnotcontains $Entry.Name) { Refuse "$($Entry.Name) is not listed in SHA256SUMS" }
    if ($Entry.PSIsContainer -or ($Entry.Attributes -band [IO.FileAttributes]::ReparsePoint)) { Refuse "$($Entry.Name) is not a regular file" }
}
foreach ($Row in $Rows) {
    $File = Join-Path $Directory $Row[1]
    if (!(Test-Path -LiteralPath $File -PathType Leaf)) { Refuse "$($Row[1]) is listed in SHA256SUMS but missing" }
    if ((Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Row[0]) { Refuse 'SHA256SUMS does not match the files' }
}
foreach ($Name in @($Names + $Sums)) {
    & $SshKeygen -q -Y sign -f $Key -n $Namespace (Join-Path $Directory $Name)
    if ($LASTEXITCODE -ne 0) { Refuse "ssh-keygen could not sign $Name" }
}
& (Join-Path $PSScriptRoot 'Verify-PoColRelease.ps1') -Directory $Directory -AllowedSigners $AllowedSigners -SshKeygen $SshKeygen
if ($LASTEXITCODE -ne 0) { Refuse 'the signed release does not verify' }
Write-Output 'Release key fingerprint (compare with the one published on pocol.net):'
& $SshKeygen -lf $Pub
