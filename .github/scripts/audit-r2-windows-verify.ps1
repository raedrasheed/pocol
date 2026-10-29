#Requires -Version 7.0
<#
.SYNOPSIS
  Audit repair (A01-A14, round 2) Windows checks, run by .github/workflows/audit-r2-windows-verify.yml.

.DESCRIPTION
  Tests the checked-out commit, which must equal -ExpectedCommit (a full SHA), with explicit toolchains:
    lp1-node          Rust 1.58.1: the full suite, and store_journal on its own (the Windows-only journal
                      that fails with UnsupportedPlatform elsewhere)
    lp4-execution     Rust 1.99.1, release: execution, admission scope, A13 execution model, proposal vectors
    pocold            Rust 1.99.1, release: journal v2 (store), P2P and RPC sockets (net), the RPC work
                      deadline and light lane, admission, consensus; and with --features io-fault the deterministic
                      storage fault tests (state_store, consensus)
    extension         Node 22.22.0 (official zip, SHA-256 pinned below): npm ci, build, unit suite
  Not included: the real-Chrome and live-devnet suites (Chromium/Playwright and Python devnets are not
  prepared for Windows). pocold's thread-count assertions read /proc and are reported as not measured on
  Windows (THREADS-NOT-MEASURED in the pocold log).

  Statuses (results.json, summary.md): PASS (ran, exit 0, tests ran), FAIL (ran and failed, or no test ran
  where tests are expected), NOT RUN (skipped after a setup failure). The script exits 1 if any check is
  not PASS. Evidence: -Evidence directory (logs\, results.json, environment.json, summary.md).
  Only child processes get toolchain variables (RUSTUP_TOOLCHAIN, PATH for Node); nothing global changes
  beyond installing the two Rust toolchains into the runner's own rustup.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Evidence,
    [Parameter(Mandatory = $true)][string]$ExpectedCommit
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$RustLp1 = '1.58.1'
$RustMain = '1.99.1'
$NodeVersion = '22.22.0'
# https://nodejs.org/dist/v22.22.0/SHASUMS256.txt, node-v22.22.0-win-x64.zip
$NodeSha256 = 'c97fa376d2becdc8863fcd3ca2dd9a83a9f3468ee7ccf7a6d076ec66a645c77a'

if ($ExpectedCommit -cnotmatch '^[0-9a-f]{40}$') { throw 'ExpectedCommit must be a full 40-character lowercase hex SHA' }
$Repo = (& git rev-parse --show-toplevel).Trim()
$Commit = (& git -C $Repo rev-parse HEAD).Trim()
if ($Commit -ne $ExpectedCommit) { throw "checked-out commit $Commit is not the requested $ExpectedCommit" }
$Dirty = @(& git -C $Repo status --porcelain --untracked-files=no).Count -gt 0
New-Item -ItemType Directory -Force -Path $Evidence | Out-Null
$Evidence = (Resolve-Path -LiteralPath $Evidence).Path
$Logs = Join-Path $Evidence 'logs'
New-Item -ItemType Directory -Force -Path $Logs | Out-Null
$P = { param($x) Join-Path $Repo ('prototypes/' + $x) }

$Utf8 = New-Object System.Text.UTF8Encoding($false)
$Results = New-Object System.Collections.Generic.List[object]
$ChildEnvRemove = @('RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER')

function ConvertTo-ArgString([string[]]$Items) {
    $quoted = foreach ($a in $Items) {
        if ($a -eq '') { '""' }
        elseif ($a -notmatch '[\s"]') { $a }
        else { '"' + (($a -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"' }
    }
    return ($quoted -join ' ')
}

# Runs a program with child-only environment changes, writes its output to logs\<Name>.log, never throws
# on a non-zero exit. Returns @{ ExitCode; Text; TimedOut; Seconds }.
function Invoke-Step {
    param([string]$Name, [string]$Program, [string[]]$ArgList, [string]$Cwd, [hashtable]$ExtraEnv = @{}, [int]$TimeoutMinutes = 60)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Program; $psi.Arguments = ConvertTo-ArgString $ArgList; $psi.WorkingDirectory = $Cwd
    $psi.UseShellExecute = $false; $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = $Utf8; $psi.StandardErrorEncoding = $Utf8
    foreach ($k in $ChildEnvRemove) { if ($psi.EnvironmentVariables.ContainsKey($k)) { $psi.EnvironmentVariables.Remove($k) } }
    foreach ($k in $ExtraEnv.Keys) { $psi.EnvironmentVariables[$k] = $ExtraEnv[$k] }
    $sw = [System.Diagnostics.Stopwatch]::StartNew(); $text = ''; $code = $null; $timedOut = $false
    try {
        $p = [System.Diagnostics.Process]::Start($psi)
        $o = $p.StandardOutput.ReadToEndAsync(); $e = $p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit($TimeoutMinutes * 60000)) { $timedOut = $true; try { $p.Kill($true) } catch { } }
        $p.WaitForExit(); $text = $o.Result + "`n" + $e.Result
        if (-not $timedOut) { $code = $p.ExitCode }
    }
    catch { $text = 'could not start: ' + $_.Exception.Message }
    $sw.Stop()
    [System.IO.File]::WriteAllText((Join-Path $Logs ($Name + '.log')), $text, $Utf8)
    Write-Host ('{0,-28} exit {1,-8} {2,8}s' -f $Name, $(if ($timedOut) { 'TIMEOUT' } else { $code }), [math]::Round($sw.Elapsed.TotalSeconds, 1))
    return [pscustomobject]@{ ExitCode = $code; Text = $text; TimedOut = $timedOut; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1); Command = ($Program + ' ' + $psi.Arguments); Cwd = $Cwd }
}

# Totals of cargo's "test result:" lines.
function Get-CargoTotals([string]$Text) {
    $passed = 0; $failed = 0; $n = 0
    foreach ($l in ($Text -split "`r?`n")) { if ($l -match '^test result: \w+\. (\d+) passed; (\d+) failed') { $passed += [int]$Matches[1]; $failed += [int]$Matches[2]; $n++ } }
    return [pscustomobject]@{ Passed = $passed; Failed = $failed; Binaries = $n }
}

$setupFailed = ''
function Add-Check([string]$Check, $Step, [string]$Status, [string]$Detail) {
    $Results.Add([pscustomobject][ordered]@{
            check = $Check; status = $Status; detail = $Detail
            exitCode = $(if ($Step) { $Step.ExitCode } else { $null }); seconds = $(if ($Step) { $Step.Seconds } else { $null })
            command = $(if ($Step) { $Step.Command } else { $null }); cwd = $(if ($Step) { $Step.Cwd } else { $null })
        })
}
function Test-Cargo([string]$Check, [string]$Crate, [string]$Toolchain, [string[]]$CargoArgs, [int]$TimeoutMinutes = 60) {
    if ($setupFailed) { Add-Check $Check $null 'NOT RUN' ('setup failed: ' + $setupFailed); return $null }
    $r = Invoke-Step $Check 'cargo' $CargoArgs (& $P $Crate) @{ RUSTUP_TOOLCHAIN = $Toolchain } $TimeoutMinutes
    $t = Get-CargoTotals $r.Text
    $detail = "Rust $Toolchain; $($t.Passed) passed, $($t.Failed) failed in $($t.Binaries) test binaries"
    if ($r.Text -match 'THREADS-NOT-MEASURED') { $detail += '; thread-count assertions not measured on this platform (THREADS-NOT-MEASURED)' }
    if ($r.TimedOut) { Add-Check $Check $r 'FAIL' ('timeout; ' + $detail) }
    elseif ($r.ExitCode -eq 0 -and $t.Passed -gt 0) { Add-Check $Check $r 'PASS' $detail }
    elseif ($r.ExitCode -eq 0) { Add-Check $Check $r 'FAIL' ('exit 0 but no test ran; ' + $detail) }
    else { Add-Check $Check $r 'FAIL' ("exit $($r.ExitCode); " + $detail) }
    return $r
}
function Test-Program([string]$Check, [string]$Crate, [string]$Program, [string[]]$ArgList, [hashtable]$Env, [int]$TimeoutMinutes = 30) {
    if ($setupFailed) { Add-Check $Check $null 'NOT RUN' ('setup failed: ' + $setupFailed); return $null }
    $r = Invoke-Step $Check $Program $ArgList (& $P $Crate) $Env $TimeoutMinutes
    $tail = (($r.Text -split "`r?`n") | Where-Object { $_ -match '"result"|"passed"|"failed"' } | Select-Object -Last 3) -join ' '
    if ($r.ExitCode -eq 0 -and -not $r.TimedOut) { Add-Check $Check $r 'PASS' $tail } else { Add-Check $Check $r 'FAIL' ("exit $($r.ExitCode); " + $tail) }
    return $r
}

# ---- toolchains ----
$Tools = Join-Path (Split-Path -Parent $Evidence) 'audit-r2-tools'
New-Item -ItemType Directory -Force -Path $Tools | Out-Null
$env0 = [ordered]@{ commit = $Commit; expectedCommit = $ExpectedCommit; dirty = $Dirty; os = [System.Environment]::OSVersion.VersionString; ps = $PSVersionTable.PSVersion.ToString()
    runner = $env:RUNNER_NAME; runnerImage = $env:ImageOS + ' ' + $env:ImageVersion; started = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ') }
foreach ($tc in @($RustLp1, $RustMain)) {
    $r = Invoke-Step ('rustup-install-' + $tc) 'rustup' @('toolchain', 'install', $tc, '--profile', 'minimal') $Repo
    if ($r.ExitCode -ne 0) { $setupFailed = "rustup toolchain install $tc" }
}
$env0.rustc = @{}
foreach ($tc in @($RustLp1, $RustMain)) { $env0.rustc[$tc] = (& rustc "+$tc" --version 2>&1 | Out-String).Trim() }
$NodeDir = $null
try {
    $zip = Join-Path $Tools "node-v$NodeVersion-win-x64.zip"
    Invoke-WebRequest -Uri "https://nodejs.org/dist/v$NodeVersion/node-v$NodeVersion-win-x64.zip" -OutFile $zip
    $h = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLowerInvariant()
    if ($h -ne $NodeSha256) { throw "node zip SHA-256 $h is not the pinned $NodeSha256" }
    Expand-Archive -LiteralPath $zip -DestinationPath $Tools -Force
    $NodeDir = Join-Path $Tools "node-v$NodeVersion-win-x64"
    $env0.node = (& (Join-Path $NodeDir 'node.exe') --version).Trim(); $env0.nodeSha256 = $h
}
catch { $setupFailed = 'node: ' + $_.Exception.Message; $env0.node = 'unavailable: ' + $_.Exception.Message }
$NodeEnv = $(if ($NodeDir) { @{ PATH = ($NodeDir + ';' + $env:PATH) } } else { @{} })

# ---- checks ----
foreach ($c in @(@('lp1-node', $RustLp1), @('lp4-execution', $RustMain), @('pocold', $RustMain))) {
    $r = Invoke-Step ('fetch-' + $c[0]) 'cargo' @('fetch', '--locked') (& $P $c[0]) @{ RUSTUP_TOOLCHAIN = $c[1] }
    if ($r.ExitCode -ne 0 -and -not $setupFailed) { $setupFailed = 'cargo fetch ' + $c[0] }
}
[void](Test-Cargo 'lp1-store-journal' 'lp1-node' $RustLp1 @('test', '--locked', '--test', 'store_journal'))
[void](Test-Cargo 'lp1-node-all' 'lp1-node' $RustLp1 @('test', '--locked', '--no-fail-fast') 90)
[void](Test-Cargo 'lp4-execution' 'lp4-execution' $RustMain @('test', '--release', '--locked', '--no-fail-fast') 90)
[void](Test-Cargo 'pocold' 'pocold' $RustMain @('test', '--release', '--locked', '--no-fail-fast') 90)
# Deterministic storage faults (--features io-fault, absent from the release): the I/O fault tests of state_store and consensus.
[void](Test-Cargo 'pocold-io-fault' 'pocold' $RustMain @('test', '--release', '--locked', '--no-fail-fast', '--features', 'io-fault', '--test', 'state_store', '--test', 'consensus', '--test', 'checkpoint_source', '--test', 'eviction_memory') 90)
$npm = $(if ($NodeDir) { Join-Path $NodeDir 'npm.cmd' } else { 'npm' }); $node = $(if ($NodeDir) { Join-Path $NodeDir 'node.exe' } else { 'node' })
[void](Test-Program 'extension-npm-ci' 'pocol-extension' $npm @('ci', '--ignore-scripts', '--no-audit', '--no-fund') $NodeEnv)
[void](Test-Program 'extension-build' 'pocol-extension' $node @('build.mjs') $NodeEnv)
[void](Test-Program 'extension-unit' 'pocol-extension' $node @('tests/unit.mjs') $NodeEnv)

# ---- evidence ----
$env0.finished = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
$env0 | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Evidence 'environment.json') -Encoding utf8
$Results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Evidence 'results.json') -Encoding utf8
$bad = @($Results | Where-Object { $_.status -ne 'PASS' })
$md = @("# Audit repair r2 Windows verification", "", "- commit: ``$Commit`` (requested ``$ExpectedCommit``), dirty: $Dirty", "- OS: $($env0.os); PowerShell $($env0.ps)",
    "- Rust: $($env0.rustc[$RustLp1]) / $($env0.rustc[$RustMain]); Node: $($env0.node)", "", "| check | status | detail |", "|---|---|---|")
foreach ($x in $Results) { $md += "| $($x.check) | $($x.status) | $(($x.detail -replace '\|', '/') -replace "`r?`n", ' ') |" }
$md += ''; $md += $(if ($bad.Count -eq 0) { 'Result: PASS' } else { "Result: FAIL ($($bad.Count) checks not PASS)" })
$md -join "`n" | Set-Content -LiteralPath (Join-Path $Evidence 'summary.md') -Encoding utf8
if ($env:GITHUB_STEP_SUMMARY) { $md -join "`n" | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding utf8 }
if ($bad.Count -gt 0) { exit 1 }
exit 0
