<#
.SYNOPSIS
    Removes the current-user RED SAMURAI installation.

.DESCRIPTION
    Removes only the stable HKCU Run value and a marker-owned install
    directory.  The product data directory is preserved deliberately.  The
    default resolver matches install: local Documents is preferred and a
    cloud-reparse Documents folder uses %LOCALAPPDATA%\OpenRedSamurai.  Use
    -WhatIf to review all changes.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter()]
    [AllowEmptyString()]
    [string] $InstallDirectory,

    [Parameter()]
    [AllowEmptyString()]
    [string] $DataDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path -Path $PSScriptRoot -ChildPath 'common.ps1')

$ProductName = 'RED SAMURAI 16400DPI Gaming Mouse'
$ExecutableName = 'redsamurai-config.exe'
$RunValueName = $ProductName
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$TrayArgument = '--tray'

if (-not $PSBoundParameters.ContainsKey('InstallDirectory')) {
    $InstallDirectory = Get-DefaultInstallDirectory
}
if (-not $PSBoundParameters.ContainsKey('DataDirectory')) {
    $DataDirectory = Get-DefaultDataDirectory -ProductName $ProductName
}

$InstallDirectory = Assert-SafeAbsolutePath -Path $InstallDirectory -Name 'InstallDirectory' -DisallowRoot
$DataDirectory = Assert-SafeAbsolutePath -Path $DataDirectory -Name 'DataDirectory' -DisallowRoot
Assert-DirectoriesDoNotOverlap -InstallDirectory $InstallDirectory -DataDirectory $DataDirectory
Assert-DirectoryMutationTarget -Path $InstallDirectory -Name 'InstallDirectory'
Assert-DirectoryMutationTarget -Path $DataDirectory -Name 'DataDirectory'

try {
    $destinationExecutable = Join-Path -Path $InstallDirectory -ChildPath $ExecutableName
}
catch {
    throw 'The executable path could not be assembled from the validated paths.'
}
$destinationExecutable = Assert-SafeAbsolutePath -Path $destinationExecutable -Name 'destination executable'
Assert-FileMutationTarget -Path $destinationExecutable -Name 'Destination executable'

$installMarker = Get-InstallMarkerPath -InstallDirectory $InstallDirectory
$markerState = if (Test-Path -LiteralPath $InstallDirectory -PathType Container) {
    Get-InstallMarkerState -MarkerPath $installMarker
}
else {
    'Absent'
}
if ($markerState -eq 'Invalid') {
    throw 'InstallDirectory contains an invalid RED SAMURAI marker; refusing to remove it.'
}

# Complete every filesystem safety check before changing the Run value.  If a
# reparse point is found, uninstall must fail closed without leaving a partial
# registry mutation behind.
if ($markerState -eq 'Valid') {
    Assert-NoReparsePointsBelow -Directory $InstallDirectory
}

$runCommand = Get-TrayAutostartCommand -ExecutablePath $destinationExecutable
Assert-TrayAutostartCommand -ExecutablePath $destinationExecutable -Command $runCommand | Out-Null
$runValueAction = 'unchanged'
try {
    $runKeyExists = Test-Path -LiteralPath $RunKey -PathType Container
}
catch {
    throw 'Current-user Run key could not be inspected.'
}
if ($runKeyExists) {
    try {
        $runProperties = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
        $runProperty = $runProperties.PSObject.Properties[$RunValueName]
    }
    catch {
        throw 'Current-user Run value could not be inspected.'
    }
    if ($null -ne $runProperty) {
        if ([string] $runProperty.Value -cne $runCommand) {
            throw 'The current-user Run value does not match this installation; refusing to remove it.'
        }
        if ($PSCmdlet.ShouldProcess("$RunKey\$RunValueName", 'Remove current-user tray startup')) {
            try {
                Remove-ItemProperty -LiteralPath $RunKey -Name $RunValueName -Force
            }
            catch {
                throw 'Current-user tray startup could not be removed.'
            }
            $runValueAction = 'removed'
        }
        else {
            $runValueAction = 'would-remove'
        }
    }
}

$installDirectoryAction = 'preserved-unmarked'
if ($markerState -eq 'Valid') {
    if ($PSCmdlet.ShouldProcess($InstallDirectory, 'Remove installed files')) {
        try {
            Remove-Item -LiteralPath $InstallDirectory -Recurse -Force
        }
        catch {
            throw "Installed files could not be removed: $InstallDirectory"
        }
        $installDirectoryAction = 'removed'
    }
    else {
        $installDirectoryAction = 'would-remove'
    }
}

[pscustomobject]@{
    Product = $ProductName
    InstallDirectory = $InstallDirectory
    DataDirectory = $DataDirectory
    RunValueName = $RunValueName
    RunCommand = $runCommand
    InstallDirectoryAction = $installDirectoryAction
    RunValueAction = $runValueAction
    DataDirectoryPreserved = $true
    Elevation = 'not required (HKCU)'
} | ConvertTo-Json -Compress
