# Collect-PoColClock.ps1 against a fake w32tm (no NTP peer is contacted): the lines it writes, the comma decimal mark,
# a failed measurement and the refusals. Run by full-nonce-upgrade.yml on Windows; it also runs under pwsh elsewhere.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Script = Join-Path $PSScriptRoot 'Collect-PoColClock.ps1'
$Dir = Join-Path ([IO.Path]::GetTempPath()) ('pocol-clock-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Dir | Out-Null
function Assert([bool]$Condition, [string]$What) { if (!$Condition) { throw "FAIL $What" } ; Write-Output "PASS $What" }
function New-Fake([string]$Name, [string]$Source, [string]$Chart, [int]$Code) {
    $Path = Join-Path $Dir "$Name.ps1"
    $Body = "if (`$args[0] -eq '/query') { Write-Output '$Source'; exit 0 }`n" +
            "if (`$args[0] -eq '/stripchart') { if (`$args[1] -cne '/computer:time.example') { exit 9 }; Write-Output 'Tracking time.example [192.0.2.10:123].'; Write-Output '$Chart'; exit $Code }`n" +
            "exit 8`n"
    [IO.File]::WriteAllText($Path, $Body, (New-Object Text.UTF8Encoding($false)))
    return $Path
}
try {
    $Good = New-Fake 'good' 'time.example,0x8' '12:00:00, +00.1234567s' 0
    $Out = Join-Path $Dir 'good'
    & $Script -Out $Out -Every 1 -Count 2 -W32tm $Good | Out-Null
    $Files = @(Get-ChildItem -LiteralPath $Out -Filter 'clock-PC-*.jsonl')
    Assert ($Files.Count -eq 1) 'one new file'
    $Lines = @(Get-Content -LiteralPath $Files[0].FullName | ForEach-Object { $_ | ConvertFrom-Json })
    Assert ($Lines.Count -eq 2) 'two samples'
    foreach ($L in $Lines) {
        Assert ($L.v -eq 1 -and $L.kind -ceq 'clock' -and $L.label -ceq 'PC' -and $L.unixMs -gt 1600000000000) 'the clock schema'
        Assert ([Math]::Abs($L.clock.offsetMs - 123.4567) -lt 0.0001 -and $L.clock.source -ceq 'w32tm' -and $L.clock.reference -ceq 'time.example') 'the offset in ms'
    }
    Assert ($Lines[1].unixMs -ge $Lines[0].unixMs + 900) 'samples -Every apart'

    $Comma = New-Fake 'comma' 'time.example' '12:00:00, -00,0500000s' 0
    $Out = Join-Path $Dir 'comma'
    & $Script -Out $Out -Count 1 -Label 'home-pc' -W32tm $Comma | Out-Null
    $L = Get-Content -LiteralPath (@(Get-ChildItem -LiteralPath $Out)[0].FullName) | ConvertFrom-Json
    Assert ([Math]::Abs($L.clock.offsetMs + 50) -lt 0.0001 -and $L.label -ceq 'home-pc') 'a comma decimal mark and a negative offset'

    # A relative -Out after cd in the same session (the README's `-Out .\measures`): the file lands in the folder the
    # script created under the PowerShell location, not under the process's start directory.
    $Here = Join-Path $Dir 'here'
    New-Item -ItemType Directory -Path $Here | Out-Null
    Push-Location -LiteralPath $Here
    try { & $Script -Out (Join-Path '.' 'measures') -Count 1 -W32tm $Good | Out-Null } finally { Pop-Location }
    $Files = @(Get-ChildItem -LiteralPath (Join-Path $Here 'measures') -Filter 'clock-PC-*.jsonl')
    Assert ($Files.Count -eq 1 -and (Get-Content -LiteralPath $Files[0].FullName | ConvertFrom-Json).kind -ceq 'clock') 'a relative -Out resolves against the PowerShell location'

    $Failed = New-Fake 'failed' 'time.example' 'error: 0x800705B4' 1
    $Out = Join-Path $Dir 'failed'
    & $Script -Out $Out -Count 1 -W32tm $Failed | Out-Null
    $L = Get-Content -LiteralPath (@(Get-ChildItem -LiteralPath $Out)[0].FullName) | ConvertFrom-Json
    Assert ($L.kind -ceq 'error' -and $null -eq $L.clock.offsetMs) 'a failed measurement is an error line without an offset'

    foreach ($Case in @(@('cmos', 'Local CMOS Clock'), @('free', 'Free-running System Clock'), @('empty', ''))) {
        $Fake = New-Fake $Case[0] $Case[1] '12:00:00, +00.0000000s' 0
        $Out = Join-Path $Dir $Case[0]
        $Message = $null
        try { & $Script -Out $Out -Count 1 -W32tm $Fake | Out-Null } catch { $Message = $_.Exception.Message }
        Assert ($Message -match 'not NTP-synchronized' -and !(Test-Path -LiteralPath $Out)) "refuses the source '$($Case[1])' before writing"
    }
    $Message = $null
    try { & $Script -Out (Join-Path $Dir 'label') -Count 1 -Label 'a b' -W32tm $Good | Out-Null } catch { $Message = $_.Exception.Message }
    Assert ($null -ne $Message) 'refuses a label outside [A-Za-z0-9_-]'
} finally {
    Remove-Item -LiteralPath $Dir -Recurse -Force
}
