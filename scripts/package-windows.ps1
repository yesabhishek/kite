[CmdletBinding()]
param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [switch]$SkipBuild,
    [string]$Binary,
    [switch]$RequireSigning
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'Run this script on Windows.' }
if ($Target -ne 'x86_64-pc-windows-msvc') { throw "Unsupported target: $Target" }
$RepoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $RepoRoot
$Work = Join-Path ([IO.Path]::GetTempPath()) ('kite-package-' + [Guid]::NewGuid().ToString('N'))

function Assert-ExitCode([string]$Operation) {
    if ($LASTEXITCODE -ne 0) { throw "$Operation failed with exit code $LASTEXITCODE" }
}

try {
    $MetadataJson = & cargo metadata --locked --no-deps --format-version 1
    Assert-ExitCode 'Cargo metadata'
    $Metadata = $MetadataJson | ConvertFrom-Json
    $Package = $Metadata.packages | Where-Object { $_.name -eq 'kite' } | Select-Object -First 1
    if (-not $Package) { throw 'Cargo package kite was not found.' }
    $Version = $Package.version
    $NumericVersion = ($Version -split '[-+]')[0]
    $HasCertificate = -not [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_BASE64)
    $HasPassword = -not [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_PASSWORD)
    if ($RequireSigning -and (-not $HasCertificate -or -not $HasPassword)) {
        throw 'WINDOWS_CERTIFICATE_BASE64 and WINDOWS_CERTIFICATE_PASSWORD are required.'
    }
    if ($HasCertificate -ne $HasPassword) { throw 'Configure both Windows certificate variables together.' }
    if ($Binary) {
        $Binary = (Resolve-Path $Binary).Path
        $SkipBuild = $true
    } else {
        $Binary = Join-Path $Metadata.target_directory "$Target\release\kite.exe"
    }
    if (-not $SkipBuild) {
        & cargo build --locked --release --target $Target --bin kite
        Assert-ExitCode 'Cargo build'
    }
    if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) { throw "Executable not found: $Binary" }

    $Makensis = Get-Command makensis.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
    if (-not $Makensis) {
        $Makensis = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
    }
    if (-not (Test-Path -LiteralPath $Makensis)) { throw 'NSIS is required. Install NSIS 3 and add makensis.exe to PATH.' }

    $DistRoot = if ($env:KITE_DIST_DIR) { $env:KITE_DIST_DIR } else { Join-Path $RepoRoot 'dist' }
    $Out = Join-Path $DistRoot $Target
    New-Item -ItemType Directory -Path $Work, $Out -Force | Out-Null
    $Out = (Resolve-Path $Out).Path
    $Stem = "kite-$Version-windows-x64"
    $Portable = Join-Path $Work $Stem
    New-Item -ItemType Directory -Path $Portable | Out-Null
    $PackagedBinary = Join-Path $Portable 'kite.exe'
    Copy-Item -LiteralPath $Binary -Destination $PackagedBinary
    Copy-Item -LiteralPath (Join-Path $RepoRoot 'assets\package\kite.ico') -Destination (Join-Path $Portable 'kite.ico')
    foreach ($Document in @('README.md', 'LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.md')) {
        $Source = Join-Path $RepoRoot $Document
        if (Test-Path -LiteralPath $Source -PathType Leaf) { Copy-Item -LiteralPath $Source -Destination $Portable }
    }
    $SigningStatus = 'unsigned development artifact'
    if ($HasCertificate) {
        $SignTool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" |
            Sort-Object FullName -Descending | Select-Object -ExpandProperty FullName -First 1
        if (-not $SignTool) { throw 'SignTool from the Windows SDK is required.' }
        $CertificatePath = Join-Path $Work 'signing.pfx'
        [IO.File]::WriteAllBytes($CertificatePath, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
        $TimestampUrl = if ($env:WINDOWS_TIMESTAMP_URL) { $env:WINDOWS_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }
        & $SignTool sign /f $CertificatePath /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd SHA256 /tr $TimestampUrl /td SHA256 $PackagedBinary
        Assert-ExitCode 'Executable signing'
        & $SignTool verify /pa /all $PackagedBinary
        Assert-ExitCode 'Executable signature verification'
        $SigningStatus = 'Authenticode signed and timestamped'
    }

    $Zip = Join-Path $Out "$Stem.zip"
    $Installer = Join-Path $Out "$Stem-setup.exe"
    $BuildInfo = Join-Path $Out "$Stem-BUILD-INFO.txt"
    $Checksums = Join-Path $Out "$Stem-SHA256SUMS.txt"
    foreach ($Artifact in @($Zip, $Installer, $BuildInfo, $Checksums)) {
        if (Test-Path -LiteralPath $Artifact) { Remove-Item -LiteralPath $Artifact -Force }
    }
    & $Makensis '/V2' "/DAPP_VERSION=$Version" "/DNUMERIC_VERSION=$NumericVersion" "/DBINARY_PATH=$PackagedBinary" "/DRESOURCES_PATH=$Portable" "/DOUTPUT_PATH=$Installer" (Join-Path $RepoRoot 'assets\package\windows.nsi')
    Assert-ExitCode 'NSIS packaging'
    if ($HasCertificate) {
        & $SignTool sign /f $CertificatePath /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd SHA256 /tr $TimestampUrl /td SHA256 $Installer
        Assert-ExitCode 'Installer signing'
        & $SignTool verify /pa /all $Installer
        Assert-ExitCode 'Installer signature verification'
    }
    Compress-Archive -Path $Portable -DestinationPath $Zip -CompressionLevel Optimal
    $Revision = & git rev-parse HEAD 2>$null
    if ($LASTEXITCODE -ne 0) { $Revision = 'uncommitted workspace' }
    @"
Product: Kite
Version: $Version
Target: $Target
Source: $Revision
Signing: $SigningStatus
Built: $([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))
"@ | Set-Content -LiteralPath $BuildInfo -Encoding utf8
    $Lines = foreach ($Artifact in @($Zip, $Installer, $BuildInfo)) {
        $Hash = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
        "$Hash  $([IO.Path]::GetFileName($Artifact))"
    }
    $Lines | Set-Content -LiteralPath $Checksums -Encoding ascii
    Write-Host "Packages: $Out"
    Write-Host "Signing: $SigningStatus"
} finally {
    if (Test-Path -LiteralPath $Work) { Remove-Item -LiteralPath $Work -Recurse -Force }
    Pop-Location
}
