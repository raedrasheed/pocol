# Rehearsal T2 on the PC (development/public-network/REHEARSAL.md, section 8): the PC runs the wallet engine without an
# RPC port, so collect_measures.py cannot sample it; this logs the PC's clock offset against its NTP peer instead, one
# JSON line per sample in the clock schema collect_measures.py merges (kind "clock", label, unixMs, clock.offsetMs).
# It measures as Initialize-PoColNetwork.ps1 does: w32tm /stripchart against the first entry of w32tm /query /source.
# It reads no key, contacts nothing but that NTP peer and writes only its own new file in -Out. Ctrl-C stops it; every
# line already written is complete.
[CmdletBinding()]
param(
    # The folder for clock-LABEL-<startUTC>.jsonl (created if missing).
    [Parameter(Mandatory=$true)][string]$Out,
    [ValidatePattern('^[A-Za-z0-9_-]{1,32}$')][string]$Label = 'PC',
    [ValidateRange(1, 86400)][int]$Every = 60,
    # The NTP peer; by default the first comma-separated entry of w32tm /query /source.
    [string]$TimePeer,
    # Samples before it stops; 0 runs until Ctrl-C.
    [ValidateRange(0, 10000000)][int]$Count = 0,
    # The w32tm command (a test substitutes a fake).
    [string]$W32tm = 'w32tm.exe'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (!$TimePeer) {
    $TimeSource = ((& $W32tm /query /source) -join '').Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Windows Time is not running. Configure an NTP peer first.' }
    if ($TimeSource -eq '' -or $TimeSource -match 'Local CMOS Clock|Free-running System Clock') {
        throw "This PC's clock is not NTP-synchronized (Windows Time source: '$TimeSource'). Configure an NTP peer first."
    }
    $TimePeer = ($TimeSource -split ',')[0].Trim()
}
# One absolute path for PowerShell's cmdlets and .NET's AppendAllText alike: .NET resolves a relative path against
# the process's start directory, not the session's location after cd.
$Out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Start = [DateTimeOffset]::UtcNow
$File = Join-Path $Out ("clock-$Label-" + $Start.UtcDateTime.ToString("yyyyMMdd'T'HHmmss'Z'") + '.jsonl')
if (Test-Path -LiteralPath $File) { throw "$File exists; an existing file is never replaced" }
$Utf8 = New-Object Text.UTF8Encoding($false)
Write-Host "Logging this PC's clock offset against $TimePeer every $Every s to $File (Ctrl-C stops)."
$Taken = 0
while ($true) {
    $Now = [DateTimeOffset]::UtcNow
    $Chart = (& $W32tm /stripchart "/computer:$TimePeer" /samples:1 /dataonly) -join "`n"
    $Measured = $LASTEXITCODE -eq 0
    $Offset = [regex]::Match($Chart, '(?m),\s*([+-]\d+[.,]\d+)s\s*$')
    $Clock = [ordered]@{ source = 'w32tm'; reference = $TimePeer; stratum = $null; offsetMs = $null }
    $Line = [ordered]@{ v = 1; kind = 'clock'; label = $Label; utc = $Now.UtcDateTime.ToString("yyyy-MM-dd'T'HH:mm:ss.fff'Z'"); unixMs = $Now.ToUnixTimeMilliseconds(); clock = $Clock }
    if ($Measured -and $Offset.Success) {
        $Clock.offsetMs = 1000 * [double]::Parse($Offset.Groups[1].Value.Replace(',', '.'), [Globalization.CultureInfo]::InvariantCulture)
    } else {
        $Line.kind = 'error'
        $Line.error = "no offset from the NTP peer $TimePeer"
    }
    [IO.File]::AppendAllText($File, (ConvertTo-Json -InputObject $Line -Compress -Depth 4) + "`n", $Utf8)
    $Taken++
    if ($Count -gt 0 -and $Taken -ge $Count) { break }
    Start-Sleep -Seconds $Every
}
