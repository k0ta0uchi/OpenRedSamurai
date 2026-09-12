<#
.SYNOPSIS
    Builds and packages a versioned OpenRedSamurai Windows release.
#>
[CmdletBinding()]
param(
    [Parameter()]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version = '1.0.0',

    [Parameter()]
    [string] $OutputDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) 'dist'),

    [Parameter()]
    [switch] $SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$output = [IO.Path]::GetFullPath($OutputDirectory)
$stageName = "OpenRedSamurai-v$Version-windows-x64"
$stage = Join-Path $output $stageName
$zip = Join-Path $output "$stageName.zip"
$sidecar = "$zip.sha256"

$versionFile = Join-Path $repoRoot 'VERSION'
if (-not (Test-Path -LiteralPath $versionFile -PathType Leaf)) {
    throw "Version file was not found: $versionFile"
}
$sourceVersion = (Get-Content -LiteralPath $versionFile -Raw).Trim()
if ($sourceVersion -ne $Version) {
    throw "Package version $Version does not match VERSION ($sourceVersion)"
}

New-Item -ItemType Directory -Path $output -Force | Out-Null
if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
if (Test-Path -LiteralPath $sidecar) { Remove-Item -LiteralPath $sidecar -Force }

if (-not $SkipBuild) {
    Push-Location $repoRoot
    try {
        & cmd.exe /d /c '.\msvc_cargo.bat build --release'
        if ($LASTEXITCODE -ne 0) { throw "cargo release build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
}

$executable = Join-Path $repoRoot 'target\release\redsamurai-config.exe'
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Release executable was not found: $executable"
}

$files = @(
    @{ Source = $executable; Relative = 'redsamurai-config.exe' },
    @{ Source = (Join-Path $repoRoot 'target\release\OpenRedSamurai-Setup.exe'); Relative = 'OpenRedSamurai-Setup.exe' },
    @{ Source = (Join-Path $repoRoot 'README.md'); Relative = 'README.md' },
    @{ Source = (Join-Path $repoRoot 'README.ja.md'); Relative = 'README.ja.md' },
    @{ Source = (Join-Path $repoRoot 'LICENSE'); Relative = 'LICENSE' },
    @{ Source = (Join-Path $repoRoot 'VERSION'); Relative = 'VERSION' },
    @{ Source = (Join-Path $repoRoot 'assets\icons\redsamurai.ico'); Relative = 'assets\icons\redsamurai.ico' },
    @{ Source = (Join-Path $repoRoot 'installer\install.ps1'); Relative = 'installer\install.ps1' },
    @{ Source = (Join-Path $repoRoot 'installer\uninstall.ps1'); Relative = 'installer\uninstall.ps1' },
    @{ Source = (Join-Path $repoRoot 'installer\common.ps1'); Relative = 'installer\common.ps1' },
    @{ Source = (Join-Path $repoRoot 'installer\setup.ps1'); Relative = 'installer\setup.ps1' },
    @{ Source = (Join-Path $repoRoot 'installer\README.md'); Relative = 'installer\README.md' },
    @{ Source = (Join-Path $repoRoot 'setup.ps1'); Relative = 'setup.ps1' },
    @{ Source = (Join-Path $repoRoot 'setup.cmd'); Relative = 'setup.cmd' }
)

foreach ($file in $files) {
    if (-not (Test-Path -LiteralPath $file.Source -PathType Leaf)) {
        throw "Required release file was not found: $($file.Source)"
    }
    $destination = Join-Path $stage $file.Relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath $file.Source -Destination $destination -Force
}

$exeHash = (Get-FileHash -LiteralPath (Join-Path $stage 'redsamurai-config.exe') -Algorithm SHA256).Hash
$installerHash = (Get-FileHash -LiteralPath (Join-Path $stage 'OpenRedSamurai-Setup.exe') -Algorithm SHA256).Hash
$iconHash = (Get-FileHash -LiteralPath (Join-Path $stage 'assets\icons\redsamurai.ico') -Algorithm SHA256).Hash
$manifest = [ordered]@{
    product = 'OpenRedSamurai'
    version = $Version
    platform = 'windows-x64'
    executable = [ordered]@{
        path = 'redsamurai-config.exe'
        sha256 = $exeHash
        bytes = (Get-Item (Join-Path $stage 'redsamurai-config.exe')).Length
    }
    installer = [ordered]@{
        path = 'OpenRedSamurai-Setup.exe'
        sha256 = $installerHash
        bytes = (Get-Item (Join-Path $stage 'OpenRedSamurai-Setup.exe')).Length
    }
    icon = [ordered]@{
        path = 'assets/icons/redsamurai.ico'
        sha256 = $iconHash
        bytes = (Get-Item (Join-Path $stage 'assets\icons\redsamurai.ico')).Length
    }
    legacy_installer = 'setup.ps1'
    launcher = 'setup.cmd'
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $stage 'release-manifest.json') -Encoding UTF8

Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$zipHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
"$zipHash  $(Split-Path -Leaf $zip)" | Set-Content -LiteralPath $sidecar -Encoding ASCII

[ordered]@{
    version = $Version
    package = $zip
    package_sha256 = $zipHash
    executable_sha256 = $exeHash
    installer_sha256 = $installerHash
    icon_sha256 = $iconHash
} | ConvertTo-Json | Write-Output
