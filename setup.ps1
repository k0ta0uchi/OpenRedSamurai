<#
.SYNOPSIS
    Installs or removes the packaged OpenRedSamurai current-user application.

.DESCRIPTION
    Legacy compatibility entry point shipped alongside the native
    OpenRedSamurai-Setup.exe. It delegates to the reviewed scripts under
    installer/ and never requests elevation.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [Parameter()]
    [switch] $Uninstall,

    [Parameter()]
    [AllowEmptyString()]
    [string] $InstallDirectory,

    [Parameter()]
    [AllowEmptyString()]
    [string] $DataDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$packageRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$delegate = Join-Path $packageRoot 'installer\setup.ps1'
if (-not (Test-Path -LiteralPath $delegate -PathType Leaf)) {
    throw "The packaged installer entry point is missing: $delegate"
}

$invoke = @{}
if ($Uninstall) { $invoke.Uninstall = $true }
if ($PSBoundParameters.ContainsKey('InstallDirectory')) {
    $invoke.InstallDirectory = $InstallDirectory
}
if ($PSBoundParameters.ContainsKey('DataDirectory')) {
    $invoke.DataDirectory = $DataDirectory
}
if ($WhatIfPreference) { $invoke.WhatIf = $true }

& $delegate @invoke
