<# Builds the portable app and the Inno Setup installer (requires Inno Setup 6; winget install JRSoftware.InnoSetup). #>
[CmdletBinding()]
param([string]$CertificateThumbprint, [string]$TimestampServer)

$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $version = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $iscc = @((Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source,
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe") | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    if (-not $iscc) { throw 'Inno Setup 6 is required: winget install JRSoftware.InnoSetup' }
    function Sign($path) {
        if (-not $CertificateThumbprint) { return }
        if (-not $TimestampServer) { throw 'Provide -TimestampServer when signing a release' }
        $certificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$CertificateThumbprint"
        if (-not $certificate.HasPrivateKey) { throw 'The signing certificate has no private key' }
        $signature = Set-AuthenticodeSignature -FilePath $path -Certificate $certificate -HashAlgorithm SHA256 -TimestampServer $TimestampServer
        if ($signature.Status -ne 'Valid') { throw "Code signing failed: $($signature.StatusMessage)" }
    }
    Sign 'target\release\veluno.exe'
    & $iscc /Qp "/DAppVersion=$version" 'installer\veluno.iss'
    if ($LASTEXITCODE -ne 0) { throw 'Installer build failed' }
    Sign 'target\release\veluno-setup.exe'
    & (Join-Path $PSScriptRoot 'verify-package.ps1')
    & (Join-Path $PSScriptRoot '..\tests\installer.test.ps1')
    Copy-Item 'target\release\veluno.exe' 'target\release\felaris.exe' -Force
    Copy-Item 'target\release\veluno-setup.exe' 'target\release\felaris-setup.exe' -Force
    $checksums = foreach ($name in @('veluno.exe', 'veluno-setup.exe', 'felaris.exe', 'felaris-setup.exe')) {
        $hash = (Get-FileHash -LiteralPath (Join-Path 'target\release' $name) -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $name"
    }
    Set-Content -LiteralPath 'target\release\SHA256SUMS' -Value $checksums -Encoding ASCII
    Write-Host 'App: target\release\veluno.exe'
    Write-Host 'Setup: target\release\veluno-setup.exe'
} finally {
    Pop-Location
}
