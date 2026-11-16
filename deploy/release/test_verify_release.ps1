# Release signing on Windows (owner decision 2026-11-16): Sign-PoColRelease.ps1 and Verify-PoColRelease.ps1 with
# throwaway ssh-ed25519 keys made in a temporary folder (never committed). Run by five-device-build.yml on Windows.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Here = $PSScriptRoot
$Verify = Join-Path $Here 'Verify-PoColRelease.ps1'
$Sign = Join-Path $Here 'Sign-PoColRelease.ps1'
# The Windows runner's OpenSSH (Win32-OpenSSH 8.x or later) or Git for Windows' ssh-keygen; the verifier's probe refuses
# an ssh-keygen without -Y.
$SshKeygen = (Get-Command ssh-keygen.exe -ErrorAction SilentlyContinue).Source
$GitKeygen = 'C:\Program Files\Git\usr\bin\ssh-keygen.exe'
if (Test-Path -LiteralPath $GitKeygen) { $SshKeygen = $GitKeygen }
if (!$SshKeygen) { throw 'FAIL ssh-keygen.exe was not found' }
$Base = Join-Path ([IO.Path]::GetTempPath()) ('pocol-release-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Base | Out-Null

function Assert([bool]$Condition, [string]$What) { if (!$Condition) { throw "FAIL $What" }; Write-Output "PASS $What" }
function New-Key([string]$Name) {
    $Path = Join-Path $Base $Name
    & $SshKeygen -q -t ed25519 -N '""' -C "$Name@test.invalid" -f $Path
    if ($LASTEXITCODE -ne 0) { throw "FAIL cannot make the throwaway key $Name" }
    return $Path
}
function New-Allowed([string]$Key, [string]$Name, [string]$Principal = 'release@pocol.net') {
    $Pub = ([IO.File]::ReadAllText("$Key.pub")).Trim().Split(' ')
    $Path = Join-Path $Base $Name
    [IO.File]::WriteAllText($Path, "$Principal namespaces=`"pocol-release`" $($Pub[0]) $($Pub[1])`n", (New-Object Text.ASCIIEncoding))
    return $Path
}
function New-Release([string]$Name) {
    $Dir = Join-Path $Base $Name
    New-Item -ItemType Directory -Path $Dir | Out-Null
    foreach ($File in @('PoCol-Mainnet-test-windows-x86_64.zip', 'network.pocol.json')) {
        [IO.File]::WriteAllBytes((Join-Path $Dir $File), [Text.Encoding]::ASCII.GetBytes("$File`n"))
    }
    # Ordinal order, as assemble_release.py (Python sorted) writes it and both verifiers require: Sort-Object compares
    # by culture and ignores case, which puts network.pocol.json before PoCol-... ('P' 0x50 < 'n' 0x6e).
    $Names = [string[]]@(Get-ChildItem -LiteralPath $Dir -File | ForEach-Object Name)
    [Array]::Sort($Names, [StringComparer]::Ordinal)
    Write-Sums $Dir $Names
    return $Dir
}
function Write-Sums([string]$Dir, [string[]]$Names) {
    # A listed name with no file gets a dummy hash (the strict-name cases below).
    $Lines = @($Names | ForEach-Object {
        $Path = Join-Path $Dir $_
        if (Test-Path -LiteralPath $Path -PathType Leaf) { $Hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
        else { $Hash = '0' * 64 }
        $Hash + '  ' + $_ })
    [IO.File]::WriteAllText((Join-Path $Dir 'SHA256SUMS'), (($Lines -join "`n") + "`n"), (New-Object Text.ASCIIEncoding))
}
function Invoke-Verify([string]$Dir, [string]$Allowed) {
    $Out = & powershell -NoProfile -ExecutionPolicy Bypass -File $Verify -Directory $Dir -AllowedSigners $Allowed -SshKeygen $SshKeygen
    return @{ Code = $LASTEXITCODE; Text = ($Out -join "`n") }
}
function Assert-NotVerified([string]$Dir, [string]$Allowed, [string]$Pattern, [string]$What) {
    $R = Invoke-Verify $Dir $Allowed
    Assert ($R.Code -eq 1 -and $R.Text.StartsWith('NOT VERIFIED: ') -and $R.Text -match $Pattern) "$What (got: $($R.Text))"
}

try {
    $Key = New-Key 'owner'
    $Allowed = New-Allowed $Key 'allowed_signers'
    $Dir = New-Release 'release'
    Assert-NotVerified $Dir $Allowed 'not signed' 'an unsigned release does not verify'
    & powershell -NoProfile -ExecutionPolicy Bypass -File $Sign -Key $Key -Directory $Dir -AllowedSigners $Allowed -SshKeygen $SshKeygen | Out-Host
    Assert ($LASTEXITCODE -eq 0) 'the signer signs and verifies'
    $R = Invoke-Verify $Dir $Allowed
    Assert ($R.Code -eq 0 -and $R.Text.StartsWith('VERIFIED: 2 files')) 'a signed release verifies'
    $Fingerprint = (@(& $SshKeygen -lf "$Key.pub") -join ' ').Split(' ')[1]
    Assert ($Fingerprint.StartsWith('SHA256:') -and $R.Text.Contains("key $Fingerprint (ED25519)")) "the verifier prints the key fingerprint $Fingerprint"
    # verify_release.py prints two lines on success: collect them, so Assert gets one bool, not an array.
    $PyOut = @(& python -B (Join-Path $Here 'verify_release.py') $Dir --allowed-signers $Allowed --ssh-keygen $SshKeygen)
    Assert ($LASTEXITCODE -eq 0 -and $PyOut.Count -ge 1 -and ([string]$PyOut[0]).StartsWith('VERIFIED: 2 files')) 'the Python verifier agrees'
    Assert-NotVerified $Dir (Join-Path $Here 'allowed_signers') 'not committed' 'the committed placeholder verifies nothing'
    Assert-NotVerified $Dir (New-Allowed $Key 'other-principal' 'someone@test.invalid') 'principal' 'another principal'
    Assert-NotVerified $Dir (New-Allowed (New-Key 'other') 'other-key') 'does not verify' 'another key'

    $File = Join-Path $Dir 'network.pocol.json'
    $Saved = [IO.File]::ReadAllBytes($File)
    [IO.File]::WriteAllBytes($File, [Text.Encoding]::ASCII.GetBytes("changed`n"))
    Assert-NotVerified $Dir $Allowed 'SHA-256 mismatch' 'a changed file'
    [IO.File]::WriteAllBytes($File, $Saved)
    Remove-Item -LiteralPath "$File.sig"
    Assert-NotVerified $Dir $Allowed 'not signed: network.pocol.json.sig' 'a missing file signature'
    [IO.File]::WriteAllText((Join-Path $Dir 'extra.txt'), 'x')
    Assert-NotVerified $Dir $Allowed 'extra.txt is in the release directory' 'an unlisted file'

    # A second key hidden behind VT (0x0b), NBSP (C2 A0) or a BOM before '#': a comment to .NET Trim, a key line to
    # ssh-keygen. A release signed only by that key never verifies, and the signer refuses such a file.
    $Evil = New-Key 'evil'
    $EvilPub = ([IO.File]::ReadAllText("$Evil.pub")).Trim().Split(' ')
    $EvilLine = [Text.Encoding]::ASCII.GetBytes("#example,release@pocol.net namespaces=`"pocol-release`" $($EvilPub[0]) $($EvilPub[1])`n")
    $DirEvil = New-Release 'release-evil'
    foreach ($Name in @('PoCol-Mainnet-test-windows-x86_64.zip', 'network.pocol.json', 'SHA256SUMS')) {
        & $SshKeygen -q -Y sign -f $Evil -n pocol-release (Join-Path $DirEvil $Name)
        if ($LASTEXITCODE -ne 0) { throw "FAIL cannot sign $Name with the throwaway key" }
    }
    $Index = 0
    foreach ($Prefix in @([byte[]](0x0B), [byte[]](0xC2, 0xA0), [byte[]](0xEF, 0xBB, 0xBF))) {
        $Index += 1
        $Hidden = Join-Path $Base "hidden-allowed-$Index"
        [IO.File]::WriteAllBytes($Hidden, [byte[]]($Prefix + $EvilLine + [IO.File]::ReadAllBytes($Allowed)))
        Assert-NotVerified $DirEvil $Hidden 'not printable ASCII' "a key line hidden behind bytes $(($Prefix | ForEach-Object { '{0:x2}' -f $_ }) -join ' ')"
    }
    $DirHidden = New-Release 'release-hidden'
    $Failed = $false
    try { & $Sign -Key $Key -Directory $DirHidden -AllowedSigners (Join-Path $Base 'hidden-allowed-1') -SshKeygen $SshKeygen } catch { $Failed = $_.Exception.Message -match 'not printable ASCII' }
    Assert ($Failed -and @(Get-ChildItem -LiteralPath $DirHidden -Filter '*.sig').Count -eq 0) 'the signer refuses a hidden key line and writes nothing'

    # The signer refuses before it writes: an existing signature, a key that is not the committed one.
    $Dir2 = New-Release 'release2'
    $Failed = $false
    try { & $Sign -Key (Join-Path $Base 'other') -Directory $Dir2 -AllowedSigners $Allowed -SshKeygen $SshKeygen } catch { $Failed = $_.Exception.Message -match 'not the one in' }
    Assert ($Failed -and @(Get-ChildItem -LiteralPath $Dir2 -Filter '*.sig').Count -eq 0) 'the signer refuses another key and writes nothing'
    $Sums = Join-Path $Dir2 'SHA256SUMS'
    [IO.File]::WriteAllText($Sums, ([IO.File]::ReadAllText($Sums)).Replace("`n", "`r`n"))
    $Failed = $false
    try { & $Sign -Key $Key -Directory $Dir2 -AllowedSigners $Allowed -SshKeygen $SshKeygen } catch { $Failed = $_.Exception.Message -match 'CR line endings' }
    Assert ($Failed -and @(Get-ChildItem -LiteralPath $Dir2 -Filter '*.sig').Count -eq 0) 'the signer refuses CRLF SHA256SUMS'

    # The signer runs the verifier's strict SHA256SUMS rules before it signs: names out of ordinal order (the culture
    # order of Sort-Object), a name listed twice, a name that is not a release file name, a listed directory.
    $Cases = @(
        @{ Names = @('network.pocol.json', 'PoCol-Mainnet-test-windows-x86_64.zip'); Want = 'not sorted'; What = 'unsorted names' },
        @{ Names = @('PoCol-Mainnet-test-windows-x86_64.zip', 'PoCol-Mainnet-test-windows-x86_64.zip', 'network.pocol.json'); Want = 'listed twice'; What = 'a name listed twice' },
        @{ Names = @('.hidden', 'PoCol-Mainnet-test-windows-x86_64.zip', 'network.pocol.json'); Want = 'not a release file name'; What = 'a hidden name' },
        @{ Names = @('PoCol-Mainnet-test-windows-x86_64.zip', 'network.pocol.json', 'notes.sig'); Want = 'not a release file name'; What = 'a .sig name' }
    )
    $Index = 0
    foreach ($Case in $Cases) {
        $Index += 1
        $DirBad = New-Release "release-strict-$Index"
        # Only the two release files exist: a bad name is refused before the signer looks for its file.
        Write-Sums $DirBad ([string[]]$Case.Names)
        $Failed = $false
        try { & $Sign -Key $Key -Directory $DirBad -AllowedSigners $Allowed -SshKeygen $SshKeygen } catch { $Failed = $_.Exception.Message -match $Case.Want }
        Assert ($Failed -and @(Get-ChildItem -LiteralPath $DirBad -Filter '*.sig' -Force).Count -eq 0) "the signer refuses $($Case.What) and writes nothing"
    }
} finally {
    Remove-Item -LiteralPath $Base -Recurse -Force -ErrorAction SilentlyContinue
}
# GitHub's powershell step ends with `exit $LASTEXITCODE`: the last native command above (a verifier run that must
# refuse) left it at 1. Every check passed, so the step exits 0; a FAIL throw has already ended the script non-zero.
exit 0
