<#
.SYNOPSIS
    Runs one bounded, reversible installer integration check on Windows.

.DESCRIPTION
    This is the explicit live boundary for V-16.  It creates a unique install
    directory below %TEMP% and a unique data directory below Documents, then
    exercises the real PowerShell installer, HKCU Run registration, file
    copying, idempotent reinstall, and marker-gated uninstall.  The existing
    product Run value is inspected first and the run is refused if it already
    exists.  The disposable data file is verified after uninstall and removed
    during cleanup.  No machine-wide key, physical HID, or SendInput action is
    touched by this script.

    The literal confirmation token is intentionally required.  Without it the
    script exits before creating a directory or reading the Run value.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter()]
    [string] $LiveConfirmation,

    [Parameter()]
    [string] $SourceDirectory,

    [Parameter()]
    [string] $DataRoot,

    [Parameter()]
    [switch] $VerifyStartup
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path -Path $PSScriptRoot -ChildPath 'common.ps1')
. (Join-Path -Path $PSScriptRoot -ChildPath 'startup-verification-helpers.ps1')

$ProductName = 'RED SAMURAI 16400DPI Gaming Mouse'
$RunValueName = $ProductName
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$TrayArgument = '--tray'
$ExecutableName = 'redsamurai-config.exe'

if ($LiveConfirmation -cne 'RED-SAMURAI-LIVE') {
    throw 'Live integration requires -LiveConfirmation RED-SAMURAI-LIVE.'
}

if (-not $PSBoundParameters.ContainsKey('SourceDirectory')) {
    $SourceDirectory = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\target\release'))
}
$SourceDirectory = Assert-SafeAbsolutePath -Path $SourceDirectory -Name 'SourceDirectory' -DisallowRoot
Assert-DirectoryMutationTarget -Path $SourceDirectory -Name 'SourceDirectory'
$sourceExecutable = Assert-SafeAbsolutePath `
    -Path (Join-Path $SourceDirectory $ExecutableName) `
    -Name 'Source executable'
Assert-FileMutationTarget -Path $sourceExecutable -Name 'Source executable'
if (-not (Test-Path -LiteralPath $sourceExecutable -PathType Leaf)) {
    throw "Source executable was not found: $sourceExecutable"
}

# Do not overwrite an existing product registration.  A pre-existing value is
# evidence that this is not a disposable user profile for this run.
$runValueBefore = $null
$runKeyExists = Test-Path -LiteralPath $RunKey -PathType Container
if ($runKeyExists) {
    try {
        $existing = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
        $existingProperty = $existing.PSObject.Properties[$RunValueName]
    }
    catch {
        throw 'Current-user Run value could not be inspected before live integration.'
    }
    if ($null -ne $existingProperty) {
        $runValueBefore = [string]$existing.$RunValueName
        throw 'The production RED SAMURAI Run value already exists; refusing live integration.'
    }
}

$runId = [Guid]::NewGuid().ToString('N')
$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('redsamurai-live-' + $runId)
$installDirectory = Join-Path $fixtureRoot 'install'
$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
if ($PSBoundParameters.ContainsKey('DataRoot')) {
    $documents = Assert-SafeAbsolutePath -Path $DataRoot -Name 'DataRoot' -DisallowRoot
    if (-not (Test-Path -LiteralPath $documents -PathType Container)) {
        throw "DataRoot must be an existing directory: $documents"
    }
    Assert-DirectoryMutationTarget -Path $documents -Name 'DataRoot'
}
else {
    $documents = [Environment]::GetFolderPath('MyDocuments')
    if ([string]::IsNullOrWhiteSpace($documents)) {
        throw "Documents path could not be resolved for '$currentIdentity'."
    }
    $documents = Assert-SafeAbsolutePath -Path $documents -Name 'Documents' -DisallowRoot
    Assert-DirectoryMutationTarget -Path $documents -Name 'Documents'
}
$dataDirectory = Join-Path $documents ($ProductName + '-live-' + $runId)
$dataFile = Join-Path $dataDirectory 'live-user-data.txt'
$installScript = Join-Path $PSScriptRoot 'install.ps1'
$uninstallScript = Join-Path $PSScriptRoot 'uninstall.ps1'
$startupScript = Join-Path $PSScriptRoot 'verify-current-user-startup.ps1'
$destinationExecutable = Join-Path $installDirectory $ExecutableName
$markerPath = Join-Path $installDirectory '.redsamurai-install'
$expectedRunCommand = Get-TrayAutostartCommand -ExecutablePath $destinationExecutable
Assert-TrayAutostartCommand -ExecutablePath $destinationExecutable -Command $expectedRunCommand | Out-Null
$fixtureCreated = $false
$currentSessionId = Get-CurrentSessionId

$result = [ordered]@{
    run_id = $runId
    current_identity = $currentIdentity
    current_session_id = $currentSessionId
    documents_root = $documents
    verification_scope = 'current-user-fixture'
    registry_scope = 'HKCU'
    documents_scope = 'current-user'
    evidence_boundary = 'HKCU/Documents fixture plus active desktop session launch; no session transition or disconnect recovery'
    session_transition_attempted = $false
    logon_performed = $false
    disconnect_recovery_performed = $false
    source_sha256 = (Get-FileHash -LiteralPath $sourceExecutable -Algorithm SHA256).Hash
    install_directory = $installDirectory
    data_directory = $dataDirectory
    run_value_before = $runValueBefore
    run_value_after_install = $null
    run_value_after_uninstall = $null
    data_sha256_before_startup = $null
    data_sha256_after_startup = $null
    data_preserved_after_startup = 'not-requested'
    data_sha256_before_uninstall = $null
    data_sha256_after_uninstall = $null
    startup_verification = $null
    cleanup = [ordered]@{
        run_value = 'not-started'
        process = 'delegated-to-startup-verifier'
        data = 'not-started'
        fixture = 'not-started'
    }
    status = 'started'
}

try {
    if ($PSCmdlet.ShouldProcess($fixtureRoot, 'Create live installer fixture')) {
        try {
            New-Item -ItemType Directory -Path $fixtureRoot -Force -ErrorAction Stop | Out-Null
        }
        catch {
            throw "Live fixture could not be created: $fixtureRoot"
        }
        $fixtureCreated = $true
    }
    else {
        throw 'Live integration requires confirmation for fixture creation.'
    }

    try {
        & $installScript -SourceDirectory $SourceDirectory -InstallDirectory $installDirectory `
            -DataDirectory $dataDirectory -Confirm:$false | Out-Null
    }
    catch {
        throw "Live install could not write its per-user paths as '$currentIdentity' under '$documents': $($_.Exception.Message)"
    }

    if (-not (Test-Path -LiteralPath $destinationExecutable -PathType Leaf)) {
        throw 'Live install did not copy the executable.'
    }
    if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) {
        throw 'Live install did not create the install marker.'
    }
    $marker = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($markerPath))
    if ($marker -cne 'redsamurai-config') {
        throw 'Live install marker content was not exact.'
    }
    $run = Get-ItemProperty -LiteralPath $RunKey -Name $RunValueName -ErrorAction Stop
    $runValue = [string] $run.$RunValueName
    Assert-TrayAutostartCommand -ExecutablePath $destinationExecutable -Command $runValue | Out-Null
    $result.run_value_after_install = $runValue

    [IO.File]::WriteAllText(
        $dataFile,
        'disposable RED SAMURAI live user data',
        [Text.UTF8Encoding]::new($false)
    )
    $result.data_sha256_before_uninstall = (Get-FileHash -LiteralPath $dataFile -Algorithm SHA256).Hash

    # A second install must preserve the exact registration and fixture.
    & $installScript -SourceDirectory $SourceDirectory -InstallDirectory $installDirectory `
        -DataDirectory $dataDirectory -Confirm:$false | Out-Null
    $runAgain = Get-ItemProperty -LiteralPath $RunKey -Name $RunValueName -ErrorAction Stop
    if ([string] $runAgain.$RunValueName -cne $expectedRunCommand) {
        throw 'Idempotent install changed the Run command.'
    }

    if ($VerifyStartup) {
        $result.data_sha256_before_startup = $result.data_sha256_before_uninstall
        if (-not (Test-Path -LiteralPath $startupScript -PathType Leaf)) {
            throw "Current-user startup verifier was not found: $startupScript"
        }
        $startupOutput = @(
            & $startupScript -InstallDirectory $installDirectory `
                -LiveConfirmation RED-SAMURAI-STARTUP -Confirm:$false
        )
        if ($startupOutput.Count -eq 0) {
            throw 'Current-user startup verifier produced no result.'
        }
        try {
            $startupResult = ($startupOutput -join [Environment]::NewLine) | ConvertFrom-Json
        }
        catch {
            throw 'Current-user startup verifier did not produce valid JSON.'
        }
        $result.startup_verification = $startupResult
        if ($startupResult.status -cne 'pass') {
            throw 'Current-user startup verifier did not pass.'
        }
        if ([string]$startupResult.verification_scope -cne 'current-user-session' -or
            [string]$startupResult.registry_scope -cne 'HKCU' -or
            $startupResult.session_transition_attempted -ne $false -or
            $startupResult.logon_performed -ne $false -or
            $startupResult.disconnect_recovery_performed -ne $false -or
            [string]$startupResult.evidence_boundary -ne
                'active desktop session launch only; no session transition or disconnect recovery') {
            throw 'Current-user startup verifier returned an invalid session evidence boundary.'
        }
        if ([int]$startupResult.current_session_id -ne $currentSessionId -or
            -not ([string]$startupResult.current_identity).Equals(
                $currentIdentity, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Current-user startup verifier identity or session did not match the live harness.'
        }
        if ([string]$startupResult.run_command -cne $expectedRunCommand -or
            [string]$startupResult.process_cleanup -cne 'complete') {
            throw 'Current-user startup verifier did not prove the exact Run command and cleanup.'
        }
        $result.data_sha256_after_startup = (Get-FileHash -LiteralPath $dataFile -Algorithm SHA256).Hash
        if ($result.data_sha256_after_startup -cne $result.data_sha256_before_startup) {
            throw 'Current-user startup verification changed disposable user data.'
        }
        $result.data_preserved_after_startup = $true
    }

    & $uninstallScript -InstallDirectory $installDirectory -DataDirectory $dataDirectory `
        -Confirm:$false | Out-Null
    if (Test-Path -LiteralPath $installDirectory) {
        throw 'Live uninstall left the marker-owned install directory.'
    }
    if (-not (Test-Path -LiteralPath $dataFile -PathType Leaf)) {
        throw 'Live uninstall removed disposable user data.'
    }
    $result.data_sha256_after_uninstall = (Get-FileHash -LiteralPath $dataFile -Algorithm SHA256).Hash
    if ($result.data_sha256_after_uninstall -cne $result.data_sha256_before_uninstall) {
        throw 'Live uninstall changed disposable user data.'
    }
    $runAfterProperty = $null
    if (Test-Path -LiteralPath $RunKey -PathType Container -ErrorAction Stop) {
        $runAfter = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
        $runAfterProperty = $runAfter.PSObject.Properties[$RunValueName]
    }
    if ($null -ne $runAfterProperty) {
        throw 'Live uninstall left the product Run value.'
    }
    $result.run_value_after_uninstall = $null
    $result.status = 'pass'
}
catch {
    $result.status = 'failed'
    $result.error = $_.Exception.Message
}
finally {
    $cleanupErrors = [Collections.Generic.List[string]]::new()

    # This value is created only by this fixture because the preflight rejects
    # an existing registration.  Remove it only after re-reading and proving
    # that the value is still this fixture's exact command.
    try {
        if (-not (Test-Path -LiteralPath $RunKey -PathType Container -ErrorAction Stop)) {
            $result.cleanup.run_value = 'absent'
        }
        else {
            $current = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
            $currentProperty = $current.PSObject.Properties[$RunValueName]
            if ($null -eq $currentProperty) {
                $result.cleanup.run_value = 'absent'
            }
            elseif (-not ([string]$current.$RunValueName -ceq $expectedRunCommand)) {
                throw 'Refusing live cleanup because the current-user Run value changed.'
            }
            elseif (-not $PSCmdlet.ShouldProcess(
                    "$RunKey\$RunValueName", 'Remove fixture current-user tray startup')) {
                throw 'Live cleanup requires confirmation for the fixture Run value.'
            }
            else {
                Remove-ItemProperty -LiteralPath $RunKey -Name $RunValueName `
                    -Force -ErrorAction Stop
                $afterRemoval = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
                if ($null -ne $afterRemoval.PSObject.Properties[$RunValueName]) {
                    throw 'Live cleanup could not verify removal of the fixture Run value.'
                }
                $result.cleanup.run_value = 'removed'
            }
        }
    }
    catch {
        $result.cleanup.run_value = 'not-proven'
        [void]$cleanupErrors.Add($_.Exception.Message)
    }

    if ($null -eq $result['startup_verification']) {
        $result.cleanup.process = if ($VerifyStartup) { 'not-proven' } else { 'not-requested' }
        if ($VerifyStartup) {
            [void]$cleanupErrors.Add('Startup process cleanup was not proven by the verifier.')
        }
    }
    else {
        try {
            if ([string]$result.startup_verification.process_cleanup -cne 'complete') {
                throw 'Startup verifier did not prove cleanup of its PID-scoped processes.'
            }
            $result.cleanup.process = 'complete'
        }
        catch {
            $result.cleanup.process = 'not-proven'
            [void]$cleanupErrors.Add($_.Exception.Message)
        }
    }

    # On a failed run, retain both disposable paths so an operator can inspect
    # the failure.  Deletion is attempted only after every live assertion has
    # passed, and each deletion is verified.  No process enumeration or broad
    # process termination is safe here because the verifier owns PID cleanup.
    if ($result.status -ceq 'pass' -and $cleanupErrors.Count -eq 0) {
        try {
            if (-not (Test-Path -LiteralPath $dataDirectory -PathType Container -ErrorAction Stop)) {
                throw 'Live cleanup could not find the disposable data directory.'
            }
            Assert-NoReparsePointsBelow -Directory $dataDirectory
            $dataEntries = @(Get-ChildItem -LiteralPath $dataDirectory -Force -ErrorAction Stop)
            if ($dataEntries.Count -ne 1 -or
                $dataEntries[0].FullName -cne $dataFile -or
                $dataEntries[0].PSIsContainer) {
                throw 'Live cleanup found unexpected disposable data entries.'
            }
            if (-not $PSCmdlet.ShouldProcess($dataDirectory, 'Remove disposable data fixture')) {
                throw 'Live cleanup requires confirmation for the disposable data fixture.'
            }
            Remove-Item -LiteralPath $dataDirectory -Recurse -Force -ErrorAction Stop
            if (Test-Path -LiteralPath $dataDirectory -ErrorAction Stop) {
                throw 'Live cleanup could not verify removal of the disposable data fixture.'
            }
            $result.cleanup.data = 'removed'

            if ($fixtureCreated -and (Test-Path -LiteralPath $fixtureRoot -ErrorAction Stop)) {
                Assert-NoReparsePointsBelow -Directory $fixtureRoot
                if (-not $PSCmdlet.ShouldProcess($fixtureRoot, 'Remove live installer fixture')) {
                    throw 'Live cleanup requires confirmation for the installer fixture.'
                }
                Remove-Item -LiteralPath $fixtureRoot -Recurse -Force -ErrorAction Stop
                if (Test-Path -LiteralPath $fixtureRoot -ErrorAction Stop) {
                    throw 'Live cleanup could not verify removal of the installer fixture.'
                }
            }
            $result.cleanup.fixture = 'removed'
        }
        catch {
            $result.cleanup.data = if ($result.cleanup.data -ceq 'not-started') {
                'not-proven'
            }
            else {
                $result.cleanup.data
            }
            $result.cleanup.fixture = 'not-proven'
            [void]$cleanupErrors.Add($_.Exception.Message)
        }
    }
    else {
        try {
            $result.cleanup.data = if (Test-Path -LiteralPath $dataDirectory -ErrorAction Stop) {
                'preserved-for-review'
            }
            else {
                'absent'
            }
        }
        catch {
            $result.cleanup.data = 'not-proven'
            [void]$cleanupErrors.Add("Disposable data cleanup state could not be inspected: $($_.Exception.Message)")
        }
        try {
            $result.cleanup.fixture = if ($fixtureCreated -and
                (Test-Path -LiteralPath $fixtureRoot -ErrorAction Stop)) {
                'preserved-for-review'
            }
            else {
                'absent'
            }
        }
        catch {
            $result.cleanup.fixture = 'not-proven'
            [void]$cleanupErrors.Add("Installer fixture cleanup state could not be inspected: $($_.Exception.Message)")
        }
    }

    if ($cleanupErrors.Count -gt 0) {
        $result.status = 'failed'
        $result.cleanup.errors = @($cleanupErrors)
        $cleanupMessage = $cleanupErrors -join ' | '
        if ($result.Contains('error')) {
            $result.error = "$( $result.error ) Cleanup: $cleanupMessage"
        }
        else {
            $result.error = "Cleanup: $cleanupMessage"
        }
    }
    $result | ConvertTo-Json -Depth 4
}

if ($result.status -ne 'pass') {
    throw 'Live installer integration did not pass.'
}
