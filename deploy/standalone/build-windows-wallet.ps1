param([switch]$VerifyRegression)
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$fx=Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
$wpf=Join-Path $fx 'WPF'
$out=Join-Path $root 'prototypes\pocold\target\release\PoCol-Wallet.exe'
New-Item -ItemType Directory -Path (Split-Path $out -Parent) -Force | Out-Null
$References=@('/nologo','/platform:x64','/reference:System.dll','/reference:System.Core.dll','/reference:System.Web.Extensions.dll','/reference:System.Xaml.dll',"/reference:$wpf\WindowsBase.dll","/reference:$wpf\PresentationCore.dll","/reference:$wpf\PresentationFramework.dll","/resource:$root\prototypes\pocol-windows\Wallet.xaml,Wallet.xaml")
$References+=@("/resource:$root\prototypes\pocol-windows\Assets\pocol-logo.png,pocol-logo.png","/resource:$root\prototypes\pocol-windows\Assets\pocol-icon.png,pocol-icon.png")
$source=Join-Path $root 'prototypes\pocol-windows\Wallet.cs'
$References+=@("/win32icon:$root\prototypes\pocol-windows\Assets\pocol-icon.ico")
$tests=Join-Path $root 'prototypes\pocol-windows\WalletUiTests.cs'
& "$fx\csc.exe" @References /target:winexe "/out:$out" $source
if($LASTEXITCODE -ne 0){throw 'Wallet compilation failed'}
$testDir=Join-Path $root 'wallet-ui-tests'
New-Item -ItemType Directory -Path $testDir -Force | Out-Null
if($VerifyRegression){
    # Prove that the same UI test catches the released history-rendering bug.
    $oldSource=Join-Path $testDir 'Wallet-before.cs'
    $oldText=& git -C $root show '50c2f27ee67961dec7aaf0eb735c0b641810e0ea:prototypes/pocol-windows/Wallet.cs'
    if($LASTEXITCODE -ne 0){throw 'Could not load released wallet source'}
    [IO.File]::WriteAllText($oldSource,($oldText -join "`n"),[Text.UTF8Encoding]::new($false))
    $oldExe=Join-Path $testDir 'WalletUiTests-before.exe'
    & "$fx\csc.exe" @References /target:exe /main:WalletUiTests "/out:$oldExe" $oldSource $tests
    if($LASTEXITCODE -ne 0){throw 'Regression baseline compilation failed'}
    $before=@(& $oldExe)
    $beforeExit=$LASTEXITCODE
    $before | Tee-Object -FilePath (Join-Path $testDir 'before.log')
    if($beforeExit -ne 1 -or ($before -join "`n") -notmatch 'FAIL The populated history was not rendered\.'){
        throw 'The released source did not reproduce the expected regression'
    }
    Write-Output 'PASS regression reproduced in the released Windows UI'
}
$testExe=Join-Path $testDir 'WalletUiTests.exe'
& "$fx\csc.exe" @References /target:exe /main:WalletUiTests "/out:$testExe" $source $tests
if($LASTEXITCODE -ne 0){throw 'UI test compilation failed'}
& $testExe (Join-Path $testDir 'activity-fixture.png') | Tee-Object -FilePath (Join-Path $testDir 'after.log')
if($LASTEXITCODE -ne 0){throw 'Windows wallet UI regression test failed'}
$preview=Join-Path $root 'wallet-preview.png'
$process=Start-Process -FilePath $out -ArgumentList @('--preview',('"'+$preview+'"')) -Wait -PassThru
if($process.ExitCode -ne 0 -or !(Test-Path $preview)){throw 'Wallet preview rendering failed'}
