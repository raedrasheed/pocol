# Run on the same Windows account that owns the PoCol desktop wallet.
# No private bytes are printed or written. Output is password-encrypted JSON.
[CmdletBinding()]
param(
    [string]$KeyPath = (Join-Path $env:LOCALAPPDATA 'PoCol\identity.pocol-key'),
    [string]$OutputPath = (Join-Path ([Environment]::GetFolderPath('Desktop')) 'PoCol-Studio-Wallet.json')
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
if (Test-Path -LiteralPath $OutputPath) { throw 'Output already exists. Choose a new -OutputPath; existing files are never overwritten.' }
$keyInfo = Get-Item -LiteralPath $KeyPath
if ($keyInfo.PSIsContainer -or ($keyInfo.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Select the original regular PoCol wallet file.' }
if ($keyInfo.Length -gt 4096) { throw 'Invalid PoCol key file size.' }
$encoded = [IO.File]::ReadAllBytes($keyInfo.FullName)
$magic = [Text.Encoding]::ASCII.GetBytes("POCOL-DPAPI-USER-V1`0")
if ($encoded.Length -le $magic.Length) { throw 'Expected a Windows PoCol DPAPI wallet.' }
for ($i = 0; $i -lt $magic.Length; $i++) { if ($encoded[$i] -ne $magic[$i]) { throw 'Expected a Windows PoCol DPAPI wallet.' } }
$cipher = New-Object byte[] ($encoded.Length - $magic.Length)
[Array]::Copy($encoded, $magic.Length, $cipher, 0, $cipher.Length)
$secret = $null; $derived = $null; $plainPassword = $null; $passwordPtr = [IntPtr]::Zero; $confirmPtr = [IntPtr]::Zero; $aes = $null; $kdf = $null; $mac = $null
try {
    $secret = [Security.Cryptography.ProtectedData]::Unprotect($cipher, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    if ($secret.Length -ne 32) { throw 'Unexpected decrypted wallet size.' }
    $password = Read-Host 'Choose an export password (at least 10 characters)' -AsSecureString
    $confirmation = Read-Host 'Repeat the export password' -AsSecureString
    $passwordPtr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($password)
    $confirmPtr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($confirmation)
    $plainPassword = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPtr)
    if ($plainPassword.Length -lt 10) { throw 'Use at least 10 characters.' }
    if ($plainPassword -cne [Runtime.InteropServices.Marshal]::PtrToStringBSTR($confirmPtr)) { throw 'Passwords do not match.' }
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    $salt = New-Object byte[] 16; $iv = New-Object byte[] 16
    $rng.GetBytes($salt); $rng.GetBytes($iv); $rng.Dispose()
    $iterations = 600000
    $kdf = [Security.Cryptography.Rfc2898DeriveBytes]::new($plainPassword, $salt, $iterations, [Security.Cryptography.HashAlgorithmName]::SHA256)
    $derived = $kdf.GetBytes(64)
    $aesKey = New-Object byte[] 32; $macKey = New-Object byte[] 32
    [Array]::Copy($derived, 0, $aesKey, 0, 32); [Array]::Copy($derived, 32, $macKey, 0, 32)
    $aes = [Security.Cryptography.Aes]::Create(); $aes.KeySize = 256; $aes.Mode = 'CBC'; $aes.Padding = 'PKCS7'; $aes.Key = $aesKey; $aes.IV = $iv
    $encryptor = $aes.CreateEncryptor(); $ct = $encryptor.TransformFinalBlock($secret, 0, $secret.Length); $encryptor.Dispose()
    $prefix = [Text.Encoding]::UTF8.GetBytes("PoCol-Studio-export-v1`0")
    $auth = New-Object byte[] ($prefix.Length + $salt.Length + $iv.Length + $ct.Length)
    $offset = 0
    foreach ($part in @($prefix, $salt, $iv, $ct)) { [Array]::Copy($part, 0, $auth, $offset, $part.Length); $offset += $part.Length }
    $mac = [Security.Cryptography.HMACSHA256]::new($macKey); $tag = $mac.ComputeHash($auth)
    $export = [ordered]@{schema='pocol-studio-key-v1'; kdf='PBKDF2-SHA256'; iterations=$iterations; cipher='AES-256-CBC-HMAC-SHA256'; salt=[Convert]::ToBase64String($salt); iv=[Convert]::ToBase64String($iv); ciphertext=[Convert]::ToBase64String($ct); mac=[Convert]::ToBase64String($tag)}
    $json = $export | ConvertTo-Json
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($json)
    $stream = [IO.File]::Open($OutputPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
    Write-Host 'Encrypted wallet export created:'
    Write-Host $OutputPath
    Write-Host 'Import this JSON only in the PoCol extension confirmation tab. Keep your export password private.'
} finally {
    foreach ($buffer in @($secret, $derived, $aesKey, $macKey, $cipher, $encoded)) { if ($null -ne $buffer) { [Array]::Clear($buffer, 0, $buffer.Length) } }
    foreach ($obj in @($aes, $kdf, $mac)) { if ($null -ne $obj) { $obj.Dispose() } }
    if ($passwordPtr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($passwordPtr) }
    if ($confirmPtr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($confirmPtr) }
    $plainPassword = $null
}
