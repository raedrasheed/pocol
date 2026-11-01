#Requires -Version 5.1
<#
.SYNOPSIS
  LP3 S1 (PR #10) Windows regression checks, run by .github/workflows/lp3-s1-windows-verify.yml.

.DESCRIPTION
  Tests the checked-out commit of this repository with Rust 1.58.1 (the LP1/LP2 accepted version):
  build, rustfmt, the full Rust suite, the LP2 storage journal and real-process crash-recovery tests,
  the portable allocation test (debug, and release with a full-size gsVersion), fixture verification,
  the HTTP and store end-to-end scripts, the GenesisSpec differential against the accepted M1
  reference, and the genesis-decode CLI harness. Optional slow checks run with -IncludeSlow (or
  INCLUDE_SLOW=true).

  Statuses (results.json, summary.md):
    PASS                 ran, exit 0, and at least one test actually ran where tests are expected
    FAIL                 ran and failed (non-zero exit, timeout, or no output where output is required)
    BLOCKED BY PLATFORM  cannot run on this platform (e.g. RLIMIT_AS checks on Windows; Windows-only
                         storage checks when this script is run elsewhere for a smoke test)
    NOT RUN              not executed: optional, skipped after an earlier failure, or zero tests
                         compiled for this platform
  The script exits 1 if any check FAILED, else 0. Evidence: -Evidence directory (logs\, commands.json,
  results.json, environment.json, source-hashes.json, summary.md, harness JSON files).

  Only child processes get the toolchain environment (RUSTUP_TOOLCHAIN); nothing global is changed
  beyond installing the 1.58.1 toolchain into the runner's own rustup.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Evidence,
    [switch]$IncludeSlow
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$RustVersion = '1.58.1'
$IsWin = ($env:OS -eq 'Windows_NT')
$Exe = $(if ($IsWin) { '.exe' } else { '' })
if (-not $IncludeSlow -and $env:INCLUDE_SLOW -eq 'true') { $IncludeSlow = [switch]$true }

$Repo = (& git rev-parse --show-toplevel).Trim()
$Commit = (& git -C $Repo rev-parse HEAD).Trim()
$Dirty = @(& git -C $Repo status --porcelain --untracked-files=no).Count -gt 0
$Crate = Join-Path $Repo 'prototypes/lp1-node'
New-Item -ItemType Directory -Force -Path $Evidence | Out-Null
$Evidence = (Resolve-Path -LiteralPath $Evidence).Path
$Logs = Join-Path $Evidence 'logs'
$Target = Join-Path (Split-Path -Parent $Evidence) 'lp3-target'
foreach ($d in @($Logs, $Target)) { New-Item -ItemType Directory -Force -Path $d | Out-Null }
if ($env:GITHUB_OUTPUT) { Add-Content -LiteralPath $env:GITHUB_OUTPUT -Value ('commit=' + $Commit) }

$Utf8 = New-Object System.Text.UTF8Encoding($false)
$Steps = New-Object System.Collections.Generic.List[object]
$Results = New-Object System.Collections.Generic.List[object]
$ChildEnv = @{}
$ChildEnvRemove = @('RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER')

function ConvertTo-ArgString([string[]]$Items) {
    $quoted = foreach ($a in $Items) {
        if ($a -eq '') { '""' }
        elseif ($a -notmatch '[\s"]') { $a }
        else { '"' + (($a -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"' }
    }
    return ($quoted -join ' ')
}

# Runs a program with child-only environment changes; writes its output to logs\<Name>.log; never
# throws on a non-zero exit. Returns @{ ExitCode; Text; TimedOut }.
function Invoke-Step {
    param([string]$Name, [string]$Program, [string[]]$ArgList, [string]$Cwd = $Crate, [hashtable]$ExtraEnv = @{}, [int]$TimeoutMinutes = 60)
    $log = Join-Path $Logs ($Name + '.log')
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Program
    $psi.Arguments = ConvertTo-ArgString $ArgList
    $psi.WorkingDirectory = $Cwd
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = $Utf8
    $psi.StandardErrorEncoding = $Utf8
    foreach ($k in $ChildEnvRemove) { if ($psi.EnvironmentVariables.ContainsKey($k)) { $psi.EnvironmentVariables.Remove($k) } }
    foreach ($k in $ChildEnv.Keys) { $psi.EnvironmentVariables[$k] = $ChildEnv[$k] }
    foreach ($k in $ExtraEnv.Keys) { $psi.EnvironmentVariables[$k] = $ExtraEnv[$k] }
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $text = ''
    $code = $null
    $timedOut = $false
    try {
        $p = [System.Diagnostics.Process]::Start($psi)
        $o = $p.StandardOutput.ReadToEndAsync()
        $e = $p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit($TimeoutMinutes * 60000)) {
            $timedOut = $true
            try { $p.Kill() } catch { }
        }
        $p.WaitForExit()
        $text = $o.Result + $e.Result
        if (-not $timedOut) { $code = $p.ExitCode }
    }
    catch {
        $text = 'could not start: ' + $_.Exception.Message
    }
    $sw.Stop()
    [System.IO.File]::WriteAllText($log, $text, $Utf8)
    $rec = [ordered]@{
        name = $Name; exitCode = $code; timedOut = $timedOut
        seconds = [math]::Round($sw.Elapsed.TotalSeconds, 2)
        command = ($Program + ' ' + $psi.Arguments); cwd = $Cwd; log = ('logs/' + $Name + '.log')
    }
    if ($null -ne $code -and ($code -lt 0 -or $code -gt 255)) { $rec.exitCodeHex = ('0x{0:X8}' -f $code) }
    if ($ExtraEnv.Count -gt 0) { $rec.env = ($ExtraEnv.Keys | ForEach-Object { $_ + '=' + $ExtraEnv[$_] }) -join ',' }
    $Steps.Add([pscustomobject]$rec)
    Write-Host ('{0,-44} exit {1,-12} {2,8}s' -f $Name, $(if ($timedOut) { 'TIMEOUT' } else { $code }), $rec.seconds)
    return [pscustomobject]@{ ExitCode = $code; Text = $text; TimedOut = $timedOut }
}

function Add-Result([string]$Check, [string]$Status, [string]$Detail, $Step = $null) {
    $Results.Add([pscustomobject][ordered]@{
            check = $Check; status = $Status
            exitCode = $(if ($Step) { $Step.ExitCode } else { $null })
            detail = $Detail
        })
}

# "test result:" lines of a cargo test run, and the total of tests that actually ran.
function Get-TestSummary([string]$Text) {
    $lines = @(($Text -split "`r?`n") | Where-Object { $_ -match '^\s*Running |^test result:|^\{"(test|case|corpus|e2e|check)"' })
    $ran = 0
    foreach ($l in $lines) {
        if ($l -match '^test result: \w+\. (\d+) passed; (\d+) failed') { $ran += [int]$Matches[1] + [int]$Matches[2] }
        if ($l -match '^\{"test":"genesis_batch_alloc".*"cases":(\d+)') { $ran += [int]$Matches[1] }
        if ($l -match '^\{"test":"asert_alloc"') { $ran += 1 }
    }
    return [pscustomobject]@{ Lines = ($lines -join "`n"); Ran = $ran }
}

# Names listed under libtest's "failures:" headers (stdout; cargo's "Running" lines are on stderr,
# so failures are matched by test name, not by target).
function Get-FailedTests([string]$Text) {
    $names = @()
    $in = $false
    foreach ($l in ($Text -split "`r?`n")) {
        if ($l -eq 'failures:') { $in = $true; continue }
        if ($in -and $l -match '^    (\S+)$') { $names += $Matches[1]; continue }
        if ($in -and $l -ne '') { $in = $false }
    }
    return @($names | Sort-Object -Unique)
}

$stop = ''
$windowsOnly = @('store-journal', 'store-process-crash-recovery', 'e2e-store')
$blockedJournalTests = @()

function Test-Cargo([string]$Check, [string[]]$CargoArgs, [hashtable]$ExtraEnv = @{}, [switch]$ExpectTests) {
    if ($stop) { Add-Result $Check 'NOT RUN' ('skipped: ' + $stop); return $null }
    $r = Invoke-Step $Check 'cargo' $CargoArgs $Crate $ExtraEnv
    $s = Get-TestSummary $r.Text
    if ($r.ExitCode -eq 0 -and $ExpectTests -and $s.Ran -eq 0) {
        Add-Result $Check 'NOT RUN' ("exit 0 but no test ran (none compiled for this platform)`n" + $s.Lines) $r
    }
    elseif ($r.ExitCode -eq 0) {
        Add-Result $Check 'PASS' $(if ($s.Lines) { $s.Lines } else { 'exit 0' }) $r
    }
    elseif (-not $IsWin -and $windowsOnly -contains $Check -and $r.Text -match 'UnsupportedPlatform|unsupportedPlatform') {
        if ($Check -eq 'store-journal') { $script:blockedJournalTests = Get-FailedTests $r.Text }
        Add-Result $Check 'BLOCKED BY PLATFORM' ("Windows-only storage writer: UnsupportedPlatform on this host`n" + $s.Lines) $r
    }
    elseif (-not $IsWin -and $Check -eq 'cargo-test-full' -and @(Get-FailedTests $r.Text).Count -gt 0 -and
        @(Get-FailedTests $r.Text | Where-Object { $blockedJournalTests -notcontains $_ }).Count -eq 0) {
        Add-Result $Check 'BLOCKED BY PLATFORM' ("every failing test is one of the " + $blockedJournalTests.Count + " Windows-only store_journal writer tests blocked above; all other tests passed`n" + $s.Lines) $r
    }
    else {
        Add-Result $Check 'FAIL' ($(if ($r.TimedOut) { 'timeout' } else { 'exit ' + $r.ExitCode }) + "`n" + $s.Lines) $r
    }
    return $r
}

function Test-Program([string]$Check, [string]$Program, [string[]]$ArgList) {
    if ($stop) { Add-Result $Check 'NOT RUN' ('skipped: ' + $stop); return $null }
    $r = Invoke-Step $Check $Program $ArgList $Crate
    $tail = (($r.Text -split "`r?`n") | Where-Object { $_ -match '^\{' } | Select-Object -Last 3) -join "`n"
    if ($r.ExitCode -eq 0) { Add-Result $Check 'PASS' $tail $r }
    elseif (-not $IsWin -and $windowsOnly -contains $Check -and $r.Text -match 'unsupportedPlatform|UnsupportedPlatform') {
        Add-Result $Check 'BLOCKED BY PLATFORM' 'Windows-only storage writer: unsupportedPlatform on this host' $r
    }
    else { Add-Result $Check 'FAIL' ($(if ($r.TimedOut) { 'timeout' } else { 'exit ' + $r.ExitCode }) + "`n" + $tail) $r }
    return $r
}

# The CLI harness reports each of its checks; OS memory-limit checks are BLOCKED BY PLATFORM where
# RLIMIT_AS does not exist (Windows).
function Test-Cli([string]$Check, [string]$Bin, [string[]]$Extra) {
    $json = Join-Path $Evidence ($Check + '.json')
    $r = Test-Program $Check 'python' (@('tests/lp3_genesis_cli.py', '--bin', $Bin, '--m1', '../../development/m1', '--out', $json) + $Extra)
    if ($r -and (Test-Path -LiteralPath $json)) {
        $d = Get-Content -Raw -LiteralPath $json | ConvertFrom-Json
        foreach ($x in $d.results) {
            $detail = $(if ($x.status -eq 'BLOCKED BY PLATFORM') { $x.reason } else { 'exit ' + $x.exit + ', ' + $x.seconds + ' s' })
            Add-Result ($Check + ': ' + $x.check) $x.status $detail
        }
    }
}

# ------------------------------------------------------------------ environment and toolchain

$envInfo = [ordered]@{
    commit = $Commit
    trackedFilesModified = $Dirty
    started = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    githubRun = $(if ($env:GITHUB_RUN_ID) { $env:GITHUB_SERVER_URL + '/' + $env:GITHUB_REPOSITORY + '/actions/runs/' + $env:GITHUB_RUN_ID + ' attempt ' + $env:GITHUB_RUN_ATTEMPT } else { $null })
    workflowRef = $env:GITHUB_WORKFLOW_REF
    workflowSha = $env:GITHUB_WORKFLOW_SHA
    eventName = $env:GITHUB_EVENT_NAME
    runnerImage = $(if ($env:ImageOS) { $env:ImageOS + ' ' + $env:ImageVersion } else { $null })
    runnerArch = $env:RUNNER_ARCH
    os = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    powershell = $PSVersionTable.PSVersion.ToString()
    scriptSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath).Hash.ToLower()
    includeSlow = [bool]$IncludeSlow
}
$envInfo.git = (Invoke-Step 'git-version' 'git' @('--version') $Repo).Text.Trim()
$envInfo.python = (Invoke-Step 'python-version' 'python' @('-c', 'import sys; print(sys.version)') $Repo).Text.Trim()
$envInfo.rustup = (Invoke-Step 'rustup-version' 'rustup' @('--version') $Repo).Text.Trim()

$tc = $RustVersion
if ($IsWin) { $tc = $RustVersion + '-x86_64-pc-windows-msvc' }
$r = Invoke-Step 'rustup-toolchain-install' 'rustup' @('toolchain', 'install', $tc, '--profile', 'minimal', '-c', 'rustfmt', '--no-self-update') $Repo
if ($r.ExitCode -ne 0) { $stop = 'toolchain install failed' }
$ChildEnv['RUSTUP_TOOLCHAIN'] = $tc
$envInfo.rustToolchain = $tc
$envInfo.rustc = (Invoke-Step 'rustc-version' 'rustc' @('-vV') $Crate).Text.Trim()
$envInfo.cargo = (Invoke-Step 'cargo-version' 'cargo' @('-V') $Crate).Text.Trim()
$envInfo.rustfmt = (Invoke-Step 'rustfmt-version' 'rustfmt' @('--version') $Crate).Text.Trim()
if (-not $stop -and $envInfo.rustc -notmatch ('release: ' + [regex]::Escape($RustVersion))) { $stop = 'rustc is not ' + $RustVersion }
if ($stop) { Add-Result 'toolchain' 'FAIL' $stop } else { Add-Result 'toolchain' 'PASS' ($tc + '; ' + $envInfo.cargo) }

# Identity of the tested bytes, in addition to the commit.
$hashFiles = @(Get-ChildItem -LiteralPath $Crate -Recurse -File | Where-Object { $_.FullName -notmatch '[\\/]target[\\/]' }) +
@(Get-ChildItem -LiteralPath (Join-Path $Repo '.github') -Recurse -File)
$hashes = foreach ($f in ($hashFiles | Sort-Object FullName)) {
    [ordered]@{ path = ($f.FullName.Substring($Repo.Length + 1) -replace '\\', '/'); sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $f.FullName).Hash.ToLower() }
}
[System.IO.File]::WriteAllText((Join-Path $Evidence 'source-hashes.json'), ((@{ commit = $Commit; files = @($hashes) } | ConvertTo-Json -Depth 4)), $Utf8)

# ------------------------------------------------------------------ checks

if (-not $stop) {
    $f = Invoke-Step 'cargo-fetch' 'cargo' @('fetch', '--locked') $Crate
    if ($f.ExitCode -ne 0) { Add-Result 'cargo-fetch' 'FAIL' ('exit ' + $f.ExitCode) $f; $stop = 'cargo fetch --locked failed' }
}
$common = @('--offline', '--locked', '--target-dir', $Target)
$b = Test-Cargo 'cargo-build' (@('build') + $common)
if ($b -and $b.ExitCode -ne 0) { $stop = 'debug build failed' }
$null = Test-Cargo 'cargo-build-release' (@('build', '--release') + $common)
$null = Test-Cargo 'rustfmt-check' @('fmt', '--', '--check')
$null = Test-Cargo 'store-journal' (@('test') + $common + @('--test', 'store_journal', '--', '--test-threads=1', '--nocapture')) -ExpectTests
$null = Test-Cargo 'store-process-crash-recovery' (@('test') + $common + @('--test', 'store_process', '--', '--test-threads=1', '--nocapture')) -ExpectTests
$null = Test-Cargo 'cargo-test-full' (@('test') + $common + @('--no-fail-fast', '--', '--test-threads=1', '--nocapture')) -ExpectTests
$null = Test-Cargo 'genesis-batch-alloc-debug' (@('test') + $common + @('--test', 'genesis_batch_alloc')) -ExpectTests
$null = Test-Cargo 'genesis-batch-alloc-release-max' (@('test', '--release') + $common + @('--test', 'genesis_batch_alloc')) @{ LP3_ALLOC_MAX_VERSION = '1' } -ExpectTests

$bin = Join-Path $Target ('debug/lp1-node' + $Exe)
$binRel = Join-Path $Target ('release/lp1-node' + $Exe)
$null = Test-Program 'verify-fixtures' $bin @('verify-fixtures', '--fixtures', 'fixtures')
$null = Test-Program 'e2e-http' 'python' @('tests/e2e_http.py', '--bin', $bin, '--fixtures', 'fixtures')
if (-not $IsWin -and -not $stop) {
    # Off Windows the LP2 writer refuses to open (by design); e2e_store.py then aborts. Probe first.
    $probe = Invoke-Step 'store-init-probe' $bin @('store', 'init', '--store', (Join-Path $Target 'probe-store'), '--fixtures', 'fixtures')
    if ($probe.Text -match 'unsupportedPlatform') { Add-Result 'e2e-store' 'BLOCKED BY PLATFORM' 'store init reports unsupportedPlatform: the LP2 writer is Windows-only' $probe }
    else { $null = Test-Program 'e2e-store' 'python' @('tests/e2e_store.py', '--bin', $bin, '--fixtures', 'fixtures') }
}
else {
    $null = Test-Program 'e2e-store' 'python' @('tests/e2e_store.py', '--bin', $bin, '--fixtures', 'fixtures')
}
$null = Test-Program 'lp3-genesis-diff' 'python' @('tests/lp3_genesis_diff.py', '--bin', $bin, '--m1', '../../development/m1', '--out', (Join-Path $Evidence 'lp3-genesis-diff.json'))
Test-Cli 'lp3-genesis-cli' $bin @()
if ($IncludeSlow) {
    $null = Test-Program 'lp3-genesis-diff-release-max' 'python' @('tests/lp3_genesis_diff.py', '--bin', $binRel, '--m1', '../../development/m1', '--max-version', '--out', (Join-Path $Evidence 'lp3-genesis-diff-release-max.json'))
    Test-Cli 'lp3-genesis-cli-release-max' $binRel @('--max-version')
}
else {
    Add-Result 'lp3-genesis-diff-release-max' 'NOT RUN' 'optional; dispatch with include_slow'
    Add-Result 'lp3-genesis-cli-release-max' 'NOT RUN' 'optional; dispatch with include_slow'
}
if ($IsWin) {
    Add-Result 'RLIMIT_AS address-space limit checks' 'BLOCKED BY PLATFORM' 'Windows has no RLIMIT_AS; bounded memory is covered here by genesis-batch-alloc (counting allocator)'
}

# ------------------------------------------------------------------ evidence

$envInfo.finished = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
[System.IO.File]::WriteAllText((Join-Path $Evidence 'environment.json'), ($envInfo | ConvertTo-Json -Depth 4), $Utf8)
[System.IO.File]::WriteAllText((Join-Path $Evidence 'commands.json'), (ConvertTo-Json -InputObject $Steps.ToArray() -Depth 4), $Utf8)
$counts = [ordered]@{}
foreach ($s in @('PASS', 'FAIL', 'BLOCKED BY PLATFORM', 'NOT RUN')) { $counts[$s] = @($Results | Where-Object { $_.status -eq $s }).Count }
$summary = [ordered]@{ commit = $Commit; trackedFilesModified = $Dirty; toolchain = $tc; os = $envInfo.os; githubRun = $envInfo.githubRun; counts = $counts; results = $Results.ToArray() }
[System.IO.File]::WriteAllText((Join-Path $Evidence 'results.json'), ($summary | ConvertTo-Json -Depth 5), $Utf8)

$md = New-Object System.Text.StringBuilder
[void]$md.AppendLine('# LP3 S1 Windows verification')
[void]$md.AppendLine('')
[void]$md.AppendLine('Commit `' + $Commit + '`' + $(if ($Dirty) { ' (WITH UNCOMMITTED CHANGES)' } else { '' }) + ', ' + $tc + ', ' + $envInfo.os + $(if ($envInfo.runnerImage) { ', image ' + $envInfo.runnerImage } else { '' }) + '.')
[void]$md.AppendLine('')
[void]$md.AppendLine('PASS ' + $counts['PASS'] + ', FAIL ' + $counts['FAIL'] + ', BLOCKED BY PLATFORM ' + $counts['BLOCKED BY PLATFORM'] + ', NOT RUN ' + $counts['NOT RUN'] + '.')
[void]$md.AppendLine('')
[void]$md.AppendLine('| Check | Status | Exit | Detail |')
[void]$md.AppendLine('|---|---|---|---|')
foreach ($x in $Results) {
    $detail = (($x.detail -replace '\|', '/') -replace "`r?`n", '<br>')
    if ($detail.Length -gt 600) { $detail = $detail.Substring(0, 600) + '...' }
    [void]$md.AppendLine('| ' + $x.check + ' | ' + $x.status + ' | ' + $x.exitCode + ' | ' + $detail + ' |')
}
[System.IO.File]::WriteAllText((Join-Path $Evidence 'summary.md'), $md.ToString(), $Utf8)
if ($env:GITHUB_STEP_SUMMARY) { Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $md.ToString() }

Write-Host ''
Write-Host ('PASS ' + $counts['PASS'] + ', FAIL ' + $counts['FAIL'] + ', BLOCKED BY PLATFORM ' + $counts['BLOCKED BY PLATFORM'] + ', NOT RUN ' + $counts['NOT RUN'])
if ($counts['FAIL'] -gt 0) { exit 1 }
exit 0
