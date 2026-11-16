# Verify a PoCol release directory on Windows: every file, SHA256SUMS and their ssh-ed25519 signatures (owner decision
# 2026-11-16). Windows PowerShell 5.1 or later. The same rules as verify_release.py:
#   - SHA256SUMS: one '<sha256>  <name>' line per file, LF only, sorted, distinct bare names;
#   - SHA256SUMS.sig and '<name>.sig' beside every listed file, each verified with
#     ssh-keygen -Y verify -f allowed_signers -I release@pocol.net -n pocol-release -s FILE.sig < FILE;
#   - nothing else in the directory; the allowed_signers file holds exactly one ssh-ed25519 line, not the placeholder.
# Prints 'VERIFIED: ...' only when every signature verifies; otherwise 'NOT VERIFIED: <reason>' and exit code 1. There is
# no checksum-only success: a SHA-256 match alone is integrity, not the publisher's authentication.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File Verify-PoColRelease.ps1 -Directory DIR [-AllowedSigners FILE] [-SshKeygen PATH]
#
# ssh-keygen reads each file on its standard input as raw bytes (Start-Process -RedirectStandardInput): a PowerShell
# pipe would re-encode the bytes. The Windows 10 built-in OpenSSH 7.7 has no -Y; use Git for Windows'
# usr\bin\ssh-keygen.exe or a current Win32-OpenSSH (8.2 or later).
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Directory,
    [string]$AllowedSigners = (Join-Path $PSScriptRoot 'allowed_signers'),
    [string]$SshKeygen = 'ssh-keygen.exe',
    [string]$Principal = 'release@pocol.net'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Namespace = 'pocol-release'
$Sums = 'SHA256SUMS'
$Probe = Join-Path $PSScriptRoot 'testdata'
$ProbePrincipal = 'probe@pocol.invalid'

function Fail([string]$Reason) { Write-Output "NOT VERIFIED: $Reason"; exit 1 }
function Invoke-Verify([string]$Allowed, [string]$Who, [string]$Signature, [string]$Message) {
    $Out = [IO.Path]::GetTempFileName(); $Err = [IO.Path]::GetTempFileName()
    try {
        $KeygenArgs = @('-Y', 'verify', '-f', ('"' + $Allowed + '"'), '-I', $Who, '-n', $Namespace, '-s', ('"' + $Signature + '"'))
        $P = Start-Process -FilePath $SshKeygen -ArgumentList $KeygenArgs -RedirectStandardInput $Message -RedirectStandardOutput $Out -RedirectStandardError $Err -NoNewWindow -Wait -PassThru
        return ($P.ExitCode -eq 0)
    } catch {
        Fail "cannot run $SshKeygen ($($_.Exception.Message))"
    } finally {
        Remove-Item -LiteralPath $Out, $Err -Force -ErrorAction SilentlyContinue
    }
}
function Test-Armor([string]$Path) {
    if (!(Test-Path -LiteralPath $Path -PathType Leaf)) { Fail "not signed: $(Split-Path -Leaf $Path) is missing" }
    $Text = [IO.File]::ReadAllText($Path)
    if ($Text.Length -gt 4096 -or $Text -cnotmatch '\A-----BEGIN SSH SIGNATURE-----\n[A-Za-z0-9+/=\n]+-----END SSH SIGNATURE-----\n?\z') {
        Fail "$(Split-Path -Leaf $Path) is not an SSH signature"
    }
}

# The allowed signers file: exactly one key line, the release principal, the namespace, an ed25519 key.
if (!(Test-Path -LiteralPath $AllowedSigners -PathType Leaf)) { Fail "cannot read the allowed signers file $AllowedSigners" }
$AllowedText = [IO.File]::ReadAllText($AllowedSigners)
if ($AllowedText.Contains("`r")) { Fail "${AllowedSigners}: CR line endings" }
$KeyLines = @($AllowedText.Split("`n") | Where-Object { $_.Trim() -ne '' -and !$_.TrimStart().StartsWith('#') })
if ($KeyLines.Count -ne 1) { Fail "${AllowedSigners}: exactly one key line is allowed, found $($KeyLines.Count)" }
if ($KeyLines[0].Contains('PLACEHOLDER')) { Fail "the owner release key is not committed ($AllowedSigners holds the PLACEHOLDER line)" }
$KeyMatch = [regex]::Match($KeyLines[0].Trim(), '\A(\S+) namespaces="([^"]+)" (\S+) (\S+)(?: .*)?\z')
if (!$KeyMatch.Success) { Fail "${AllowedSigners}: the key line is not '$Principal namespaces=`"$Namespace`" ssh-ed25519 KEY'" }
if ($KeyMatch.Groups[1].Value -cne $Principal) { Fail "${AllowedSigners}: the principal is not $Principal" }
if ($KeyMatch.Groups[2].Value -cne $Namespace) { Fail "${AllowedSigners}: the namespace is not $Namespace" }
if ($KeyMatch.Groups[3].Value -cne 'ssh-ed25519' -or $KeyMatch.Groups[4].Value -cnotmatch '\AAAAAC3NzaC1lZDI1NTE5[A-Za-z0-9+/]{48}\z') {
    Fail "${AllowedSigners}: the key is not an ssh-ed25519 public key"
}

# ssh-keygen must implement -Y verify: the committed known-good vector verifies, and the same signature over other
# bytes does not.
if (!(Get-Command $SshKeygen -ErrorAction SilentlyContinue)) { Fail "ssh-keygen was not found (pass -SshKeygen; OpenSSH 8.2 or later)" }
$ProbeMessage = Join-Path $Probe 'probe.txt'
$ProbeAllowed = Join-Path $Probe 'probe.allowed_signers'
$ProbeSig = Join-Path $Probe 'probe.txt.sig'
$Changed = [IO.Path]::GetTempFileName()
try {
    [IO.File]::WriteAllBytes($Changed, [byte[]]([IO.File]::ReadAllBytes($ProbeMessage) + [byte[]](0x78)))
    if (!(Invoke-Verify $ProbeAllowed $ProbePrincipal $ProbeSig $ProbeMessage) -or (Invoke-Verify $ProbeAllowed $ProbePrincipal $ProbeSig $Changed)) {
        Fail "$SshKeygen cannot check SSH signatures (ssh-keygen -Y verify needs OpenSSH 8.2 or later; use Git for Windows usr\bin\ssh-keygen.exe or a current Win32-OpenSSH)"
    }
} finally { Remove-Item -LiteralPath $Changed -Force -ErrorAction SilentlyContinue }

# SHA256SUMS: strict lines, sorted distinct names, nothing else in the directory.
if (!(Test-Path -LiteralPath $Directory -PathType Container)) { Fail "$Directory is not a directory" }
$Directory = (Resolve-Path -LiteralPath $Directory).Path
$SumsPath = Join-Path $Directory $Sums
if (!(Test-Path -LiteralPath $SumsPath -PathType Leaf)) { Fail "no $Sums in $Directory" }
$SumsBytes = [IO.File]::ReadAllBytes($SumsPath)
$SumsText = [Text.Encoding]::ASCII.GetString($SumsBytes)
if ($SumsText.Contains("`r")) { Fail "$Sums has CR line endings (LF only)" }
if ($SumsText.Length -eq 0 -or !$SumsText.EndsWith("`n")) { Fail "$Sums is empty or does not end with a newline" }
$Rows = @()
foreach ($Line in $SumsText.Substring(0, $SumsText.Length - 1).Split("`n")) {
    if ($Line -cnotmatch '\A([0-9a-f]{64})  ([A-Za-z0-9._-]+)\z') { Fail "${Sums}: not a '<sha256>  <name>' line: $Line" }
    $Name = $Matches[2]
    if ($Name.StartsWith('.') -or $Name -ceq $Sums -or $Name.EndsWith('.sig')) { Fail "${Sums}: '$Name' is not a release file name" }
    $Rows += ,@($Matches[1], $Name)
}
$Names = @($Rows | ForEach-Object { $_[1] })
if (@($Names | Sort-Object -Unique -CaseSensitive).Count -ne $Names.Count) { Fail "${Sums}: a name is listed twice" }
$Sorted = [string[]]$Names.Clone(); [Array]::Sort($Sorted, [StringComparer]::Ordinal)
if (($Sorted -join "`n") -cne ($Names -join "`n")) { Fail "${Sums}: the names are not sorted" }
$Expected = @($Sums, "$Sums.sig") + $Names + @($Names | ForEach-Object { "$_.sig" })
foreach ($Entry in @(Get-ChildItem -LiteralPath $Directory -Force)) {
    if ($Expected -cnotcontains $Entry.Name) { Fail "$($Entry.Name) is in the release directory but not in $Sums" }
    if ($Entry.PSIsContainer -or ($Entry.Attributes -band [IO.FileAttributes]::ReparsePoint)) { Fail "$($Entry.Name) is not a regular file" }
}
Test-Armor "$SumsPath.sig"
if (!(Invoke-Verify $AllowedSigners $Principal "$SumsPath.sig" $SumsPath)) { Fail "$Sums.sig does not verify for $Principal in namespace $Namespace" }
foreach ($Row in $Rows) {
    $File = Join-Path $Directory $Row[1]
    if (!(Test-Path -LiteralPath $File -PathType Leaf)) { Fail "$($Row[1]) is listed in $Sums but missing" }
    if ((Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Row[0]) { Fail "$($Row[1]): SHA-256 mismatch" }
    Test-Armor "$File.sig"
    if (!(Invoke-Verify $AllowedSigners $Principal "$File.sig" $File)) { Fail "$($Row[1]).sig does not verify for $Principal in namespace $Namespace" }
}
Write-Output "VERIFIED: $($Rows.Count) files, $Sums and every signature ($Principal, namespace $Namespace, $AllowedSigners)"
exit 0
