<#
.SYNOPSIS
    Installs RED SAMURAI for the current Windows user.

.DESCRIPTION
    This installer intentionally uses HKCU rather than HKLM.  It does not
    require elevation, does not create a service, and registers one stable
    per-user Run value that starts the application in tray mode.  Use -WhatIf
    to inspect every write before applying it.  A local Documents folder is
    preferred; when the Windows known folder traverses a cloud reparse point,
    the default data directory falls back to %LOCALAPPDATA%\OpenRedSamurai.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [Parameter()]
    [AllowEmptyString()]
    [string] $SourceDirectory,

    [Parameter()]
    [AllowEmptyString()]
    [string] $InstallDirectory,

    [Parameter()]
    [AllowEmptyString()]
    [string] $DataDirectory,

    [Parameter()]
    [AllowEmptyString()]
    [string] $ExecutableName = 'redsamurai-config.exe'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path -Path $PSScriptRoot -ChildPath 'common.ps1')

$ProductName = 'RED SAMURAI 16400DPI Gaming Mouse'
$DefaultExecutableName = 'redsamurai-config.exe'
$RunValueName = $ProductName
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$TrayArgument = '--tray'

if (-not $PSBoundParameters.ContainsKey('SourceDirectory')) {
    $SourceDirectory = $PSScriptRoot
}
if (-not $PSBoundParameters.ContainsKey('InstallDirectory')) {
    $InstallDirectory = Get-DefaultInstallDirectory
}
if (-not $PSBoundParameters.ContainsKey('DataDirectory')) {
    $DataDirectory = Get-DefaultDataDirectory -ProductName $ProductName
}

$ExecutableName = Assert-SafeExecutableName $ExecutableName
if ($ExecutableName -cne $DefaultExecutableName) {
    # Uninstall has no trustworthy source of a custom name, so accepting one
    # would allow install to create a Run value that uninstall cannot prove is
    # the same registration.  Keep the boundary fail-closed.
    throw 'ExecutableName must be the default redsamurai-config.exe; custom names cannot be safely uninstalled.'
}
$SourceDirectory = Assert-SafeAbsolutePath -Path $SourceDirectory -Name 'SourceDirectory' -DisallowRoot
$InstallDirectory = Assert-SafeAbsolutePath -Path $InstallDirectory -Name 'InstallDirectory' -DisallowRoot
$DataDirectory = Assert-SafeAbsolutePath -Path $DataDirectory -Name 'DataDirectory' -DisallowRoot
Assert-DirectoriesDoNotOverlap -InstallDirectory $InstallDirectory -DataDirectory $DataDirectory

if (-not (Test-Path -LiteralPath $SourceDirectory -PathType Container)) {
    throw "SourceDirectory was not found: $SourceDirectory"
}
Assert-DirectoryMutationTarget -Path $SourceDirectory -Name 'SourceDirectory'
Assert-DirectoryMutationTarget -Path $InstallDirectory -Name 'InstallDirectory'
Assert-DirectoryMutationTarget -Path $DataDirectory -Name 'DataDirectory'

try {
    $sourceExecutable = Join-Path -Path $SourceDirectory -ChildPath $ExecutableName
    $destinationExecutable = Join-Path -Path $InstallDirectory -ChildPath $ExecutableName
}
catch {
    throw 'The executable path could not be assembled from the validated paths.'
}
$sourceExecutable = Assert-SafeAbsolutePath -Path $sourceExecutable -Name 'source executable'
$destinationExecutable = Assert-SafeAbsolutePath -Path $destinationExecutable -Name 'destination executable'

if (-not (Test-Path -LiteralPath $sourceExecutable -PathType Leaf)) {
    throw "The source executable was not found: $sourceExecutable"
}
Assert-FileMutationTarget -Path $sourceExecutable -Name 'Source executable'
Assert-FileMutationTarget -Path $destinationExecutable -Name 'Destination executable'

$installMarker = Get-InstallMarkerPath -InstallDirectory $InstallDirectory
$markerState = if (Test-Path -LiteralPath $InstallDirectory -PathType Container) {
    Get-InstallMarkerState -MarkerPath $installMarker
}
else {
    'Absent'
}
if ($markerState -eq 'Invalid') {
    throw 'InstallDirectory contains an invalid RED SAMURAI marker.'
}
if ($markerState -eq 'Absent' -and
    (Test-Path -LiteralPath $InstallDirectory -PathType Container)) {
    $entries = @(Get-ChildItem -LiteralPath $InstallDirectory -Force -ErrorAction Stop)
    if ($entries.Count -gt 0) {
        throw 'InstallDirectory must be empty or contain a trusted RED SAMURAI marker.'
    }
}

# Inspect the existing startup value before any filesystem mutation.  A
# conflicting value must fail closed without leaving a partially installed
# executable, marker, or data directory behind.
$runCommand = Get-TrayAutostartCommand -ExecutablePath $destinationExecutable
Assert-TrayAutostartCommand -ExecutablePath $destinationExecutable -Command $runCommand | Out-Null
$runValuePresent = $false
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
            throw 'The current-user Run value does not match this installation; refusing to overwrite it.'
        }
        $runValuePresent = $true
    }
}

$installDirectoryAction = 'unchanged'
if (-not (Test-Path -LiteralPath $InstallDirectory -PathType Container)) {
    if ($PSCmdlet.ShouldProcess($InstallDirectory, 'Create install directory')) {
        try {
            New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
        }
        catch {
            throw "Install directory could not be created: $InstallDirectory"
        }
        $installDirectoryAction = 'created'
    }
    else {
        $installDirectoryAction = 'would-create'
    }
}

$dataDirectoryAction = 'unchanged'
if (-not (Test-Path -LiteralPath $DataDirectory -PathType Container)) {
    if ($PSCmdlet.ShouldProcess($DataDirectory, 'Create data directory')) {
        try {
            New-Item -ItemType Directory -Path $DataDirectory -Force | Out-Null
        }
        catch {
            throw "Data directory could not be created: $DataDirectory"
        }
        $dataDirectoryAction = 'created'
    }
    else {
        $dataDirectoryAction = 'would-create'
    }
}

$executableAction = 'copied'
if ($PSCmdlet.ShouldProcess($destinationExecutable, 'Copy executable')) {
    try {
        Copy-Item -LiteralPath $sourceExecutable -Destination $destinationExecutable -Force
    }
    catch {
        throw "Executable could not be copied to: $destinationExecutable"
    }
}
else {
    $executableAction = 'would-copy'
}

$markerAction = 'unchanged'
if ($markerState -eq 'Absent') {
    if ($PSCmdlet.ShouldProcess($installMarker, 'Write install marker')) {
        Write-InstallMarker -MarkerPath $installMarker
        $markerAction = 'created'
    }
    else {
        $markerAction = 'would-create'
    }
}

$runValueAction = 'unchanged'
if ($runKeyExists) {
    if ($runValuePresent) {
        $runValueAction = 'unchanged'
    }
    elseif ($PSCmdlet.ShouldProcess("$RunKey\$RunValueName", 'Register current-user tray startup')) {
        try {
            New-ItemProperty -LiteralPath $RunKey -Name $RunValueName `
                -PropertyType String -Value $runCommand -Force | Out-Null
        }
        catch {
            throw 'Current-user tray startup could not be registered.'
        }
        $runValueAction = 'registered'
    }
    else {
        $runValueAction = 'would-register'
    }
}
else {
    if ($PSCmdlet.ShouldProcess($RunKey, 'Create current-user Run key')) {
        try {
            New-Item -Path $RunKey -Force | Out-Null
        }
        catch {
            throw 'Current-user Run key could not be created.'
        }
    }
    if ($PSCmdlet.ShouldProcess("$RunKey\$RunValueName", 'Register current-user tray startup')) {
        try {
            New-ItemProperty -LiteralPath $RunKey -Name $RunValueName `
                -PropertyType String -Value $runCommand -Force | Out-Null
        }
        catch {
            throw 'Current-user tray startup could not be registered.'
        }
        $runValueAction = 'registered'
    }
    else {
        $runValueAction = 'would-register'
    }
}

[pscustomobject]@{
    Product = $ProductName
    InstallDirectory = $InstallDirectory
    DataDirectory = $DataDirectory
    RunValueName = $RunValueName
    RunCommand = $runCommand
    InstallDirectoryAction = $installDirectoryAction
    DataDirectoryAction = $dataDirectoryAction
    ExecutableAction = $executableAction
    MarkerAction = $markerAction
    RunValueAction = $runValueAction
    Elevation = 'not required (HKCU)'
} | ConvertTo-Json -Compress
