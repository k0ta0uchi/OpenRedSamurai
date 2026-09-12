<#
.SYNOPSIS
    Release-package entry point for current-user install and uninstall.
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
$packageRoot = Split-Path -Parent $PSScriptRoot

if ($Uninstall) {
    $delegate = Join-Path $PSScriptRoot 'uninstall.ps1'
    $invoke = @{}
} else {
    $delegate = Join-Path $PSScriptRoot 'install.ps1'
    $invoke = @{ SourceDirectory = $packageRoot }
}
if ($PSBoundParameters.ContainsKey('InstallDirectory')) {
    $invoke.InstallDirectory = $InstallDirectory
}
if ($PSBoundParameters.ContainsKey('DataDirectory')) {
    $invoke.DataDirectory = $DataDirectory
}
if ($WhatIfPreference) { $invoke.WhatIf = $true }

& $delegate @invoke
