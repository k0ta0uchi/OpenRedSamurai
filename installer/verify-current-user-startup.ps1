<#
.SYNOPSIS
    Verifies the current-user tray Run contract in the active desktop session.

.DESCRIPTION
    This is a bounded V-17 subcheck, not a session-transition test.  It reads
    the product value from HKCU, verifies the exact quoted executable and
    --tray command, launches that command in the current user session, and
    checks that a second launch exits while the first instance remains alive.
    Registry and Documents data are read-only in this script.  Only processes
    started by this script are stopped during cleanup.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter(Mandatory = $true)]
    [AllowEmptyString()]
    [string] $InstallDirectory,

    [Parameter(Mandatory = $true)]
    [AllowEmptyString()]
    [string] $LiveConfirmation,

    [Parameter()]
    [ValidateRange(250, 30000)]
    [int] $TimeoutMs = 5000
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($LiveConfirmation -cne 'RED-SAMURAI-STARTUP') {
    throw 'Current-user startup verification requires -LiveConfirmation RED-SAMURAI-STARTUP.'
}

. (Join-Path -Path $PSScriptRoot -ChildPath 'common.ps1')
. (Join-Path -Path $PSScriptRoot -ChildPath 'startup-verification-helpers.ps1')

$ProductName = 'RED SAMURAI 16400DPI Gaming Mouse'
$RunValueName = $ProductName
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$ExecutableName = 'redsamurai-config.exe'
$TrayArgument = '--tray'
$startedProcessId = $null
$secondProcessId = $null
$destinationExecutable = $null
$startedCreationDate = $null
$secondCreationDate = $null
$cleanupErrors = [Collections.Generic.List[string]]::new()
$currentIdentity = Get-CurrentUserIdentityName
$currentSessionId = Get-CurrentSessionId

$result = [ordered]@{
    verification_scope = 'current-user-session'
    evidence_boundary = 'active desktop session launch only; no session transition or disconnect recovery'
    current_identity = $currentIdentity
    current_session_id = $currentSessionId
    session_transition_attempted = $false
    logon_performed = $false
    disconnect_recovery_performed = $false
    install_directory = $null
    executable = $null
    registry_scope = 'HKCU'
    run_value_name = $RunValueName
    run_command = $null
    started_command_line = $null
    second_command_line = $null
    started_identity = $null
    second_identity = $null
    started_session_id = $null
    second_session_id = $null
    started_pid = $null
    second_pid = $null
    second_instance_exit_code = $null
    process_cleanup = 'not-started'
    status = 'started'
}

function Get-RedSamuraiProcessRecords {
    [CmdletBinding()]
    param()

    return @(
        Get-CimInstance -ClassName Win32_Process `
            -Filter "Name = '$ExecutableName'" -ErrorAction Stop
    )
}

function Get-ProcessRecord {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [int] $ProcessId)

    $records = @(Get-CimInstance -ClassName Win32_Process `
        -Filter "ProcessId = $ProcessId" -ErrorAction Stop)
    if ($records.Count -eq 0) {
        return $null
    }
    if ($records.Count -ne 1) {
        throw "Process record was ambiguous for PID $ProcessId."
    }
    return $records[0]
}

function Test-ProcessAlive {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [int] $ProcessId)

    try {
        $process = Get-Process -Id $ProcessId -ErrorAction Stop
        return -not $process.HasExited
    }
    catch {
        throw "Tracked process state could not be inspected for PID $ProcessId."
    }
}

function Assert-TrackedProcessAlive {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [int] $ProcessId,
        [Parameter(Mandatory = $true)] [string] $ExpectedExecutable,
        [Parameter()] [string] $ExpectedCreationDate
    )

    $record = Get-ProcessRecord -ProcessId $ProcessId
    if ($null -eq $record) {
        throw "Tracked process was not observable for PID $ProcessId."
    }
    if ([string]$record.ExecutablePath -cne $ExpectedExecutable) {
        throw "Tracked process executable mismatch for PID $ProcessId."
    }
    if (-not [string]::IsNullOrWhiteSpace($ExpectedCreationDate) -and
        [string]$record.CreationDate -cne $ExpectedCreationDate) {
        throw "Tracked process identity changed for PID $ProcessId."
    }
    if (-not (Test-ProcessAlive -ProcessId $ProcessId)) {
        throw "The first tray process exited before verification completed for PID $ProcessId."
    }
    return $true
}

function Wait-ForProcessRecord {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)] [int] $ProcessId)

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        $record = Get-ProcessRecord -ProcessId $ProcessId
        if ($null -ne $record) {
            return $record
        }
        Start-Sleep -Milliseconds 100
    }
    while ([DateTime]::UtcNow -lt $deadline)
    throw "The tray process command line was not observable within $TimeoutMs ms."
}

function Stop-StartedProcess {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)] [int] $ProcessId,
        [Parameter(Mandatory = $true)] [string] $ExpectedExecutable,
        [Parameter()] [string] $ExpectedCreationDate
    )

    $record = Get-ProcessRecord -ProcessId $ProcessId
    if ($null -eq $record) {
        return 'already-exited'
    }
    if ([string]$record.ExecutablePath -cne $ExpectedExecutable) {
        throw "Refusing to stop PID $ProcessId after its executable changed."
    }
    if (-not [string]::IsNullOrWhiteSpace($ExpectedCreationDate) -and
        [string]$record.CreationDate -cne $ExpectedCreationDate) {
        throw "Refusing to stop PID $ProcessId after its process identity changed."
    }
    try {
        Stop-Process -Id $ProcessId -Force -ErrorAction Stop
    }
    catch {
        throw "Started process PID $ProcessId could not be stopped safely."
    }

    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        $remaining = Get-ProcessRecord -ProcessId $ProcessId
        if ($null -eq $remaining) {
            return 'complete'
        }
        if ([string]$remaining.ExecutablePath -cne $ExpectedExecutable -or
            (-not [string]::IsNullOrWhiteSpace($ExpectedCreationDate) -and
                [string]$remaining.CreationDate -cne $ExpectedCreationDate)) {
            throw "PID $ProcessId was reused before cleanup could be verified."
        }
        Start-Sleep -Milliseconds 100
    }
    while ([DateTime]::UtcNow -lt $deadline)
    throw "Started process PID $ProcessId remained alive after cleanup."
}

try {
    $InstallDirectory = Assert-SafeAbsolutePath -Path $InstallDirectory `
        -Name 'InstallDirectory' -DisallowRoot
    Assert-DirectoryMutationTarget -Path $InstallDirectory -Name 'InstallDirectory'
    $result.install_directory = $InstallDirectory

    try {
        $destinationExecutable = Join-Path -Path $InstallDirectory -ChildPath $ExecutableName
    }
    catch {
        throw 'The executable path could not be assembled from the validated path.'
    }
    $destinationExecutable = Assert-SafeAbsolutePath `
        -Path $destinationExecutable -Name 'Destination executable'
    Assert-FileMutationTarget -Path $destinationExecutable -Name 'Destination executable'
    if (-not (Test-Path -LiteralPath $destinationExecutable -PathType Leaf)) {
        throw "Destination executable was not found: $destinationExecutable"
    }
    $result.executable = $destinationExecutable

    $expectedCommand = Get-TrayAutostartCommand -ExecutablePath $destinationExecutable
    if (-not (Test-Path -LiteralPath $RunKey -PathType Container)) {
        throw 'Current-user Run key was not found.'
    }
    $runProperties = Get-ItemProperty -LiteralPath $RunKey -ErrorAction Stop
    $runProperty = $runProperties.PSObject.Properties[$RunValueName]
    if ($null -eq $runProperty) {
        throw 'Current-user RED SAMURAI Run value was not found.'
    }
    $runCommand = [string] $runProperty.Value
    Assert-TrayAutostartCommand -ExecutablePath $destinationExecutable `
        -Command $runCommand | Out-Null
    $result.run_command = $runCommand

    $existing = @(Get-RedSamuraiProcessRecords)
    if ($existing.Count -gt 0) {
        throw 'A redsamurai-config process is already running; refusing startup verification.'
    }

    if ($PSCmdlet.ShouldProcess($destinationExecutable, 'Launch current-user tray startup verification')) {
        $started = Start-Process -FilePath $destinationExecutable `
            -ArgumentList @($TrayArgument) -WorkingDirectory $InstallDirectory -PassThru
        $startedProcessId = [int] $started.Id
        $result.started_pid = $startedProcessId
        $startedRecord = Wait-ForProcessRecord -ProcessId $startedProcessId
        $startedCreationDate = [string]$startedRecord.CreationDate
        if ($null -eq $startedRecord.SessionId) {
            throw "Started process session could not be inspected for PID $startedProcessId."
        }
        $startedSessionId = [int]$startedRecord.SessionId
        Assert-ProcessSession -ExpectedSessionId $currentSessionId `
            -ActualSessionId $startedSessionId -ProcessId $startedProcessId | Out-Null
        if ([string]$startedRecord.ExecutablePath -cne $destinationExecutable) {
            throw "Started process executable mismatch: $($startedRecord.ExecutablePath)"
        }
        $startedCommandLine = ([string]$startedRecord.CommandLine).Trim()
        Assert-ExactTrayProcessCommandLine -ExpectedCommand $expectedCommand `
            -ActualCommandLine $startedCommandLine -ProcessId $startedProcessId | Out-Null
        $startedIdentity = Get-ProcessUserIdentity -ProcessRecord $startedRecord
        Assert-ProcessUserIdentity -ExpectedIdentity $currentIdentity `
            -ActualIdentity $startedIdentity -ProcessId $startedProcessId | Out-Null
        $result.started_command_line = $startedCommandLine
        $result.started_identity = $startedIdentity
        $result.started_session_id = $startedSessionId
        Assert-TrackedProcessAlive -ProcessId $startedProcessId `
            -ExpectedExecutable $destinationExecutable `
            -ExpectedCreationDate $startedCreationDate | Out-Null

        $second = Start-Process -FilePath $destinationExecutable `
            -ArgumentList @($TrayArgument) -WorkingDirectory $InstallDirectory -PassThru
        $secondProcessId = [int] $second.Id
        $result.second_pid = $secondProcessId
        if (-not $second.WaitForExit($TimeoutMs)) {
            throw 'The second tray launch did not exit within the verification window.'
        }
        $result.second_instance_exit_code = $second.ExitCode
        if ($second.ExitCode -ne 0) {
            throw "The second tray launch returned exit code $($second.ExitCode)."
        }
        Assert-TrackedProcessAlive -ProcessId $startedProcessId `
            -ExpectedExecutable $destinationExecutable `
            -ExpectedCreationDate $startedCreationDate | Out-Null
        $result.status = 'pass'
    }
    else {
        $result.status = 'whatif'
    }
}
catch {
    $result.status = 'failed'
    $result.error = $_.Exception.Message
}
finally {
    foreach ($trackedProcess in @(
        [pscustomobject]@{ Id = $secondProcessId; CreationDate = $secondCreationDate },
        [pscustomobject]@{ Id = $startedProcessId; CreationDate = $startedCreationDate }
    )) {
        if ($null -eq $trackedProcess.Id) {
            continue
        }
        try {
            [void](Stop-StartedProcess -ProcessId $trackedProcess.Id `
                -ExpectedExecutable $destinationExecutable `
                -ExpectedCreationDate $trackedProcess.CreationDate)
        }
        catch {
            [void]$cleanupErrors.Add($_.Exception.Message)
        }
    }
    if ($cleanupErrors.Count -gt 0) {
        $result.process_cleanup = 'unknown'
        $result.status = 'failed'
        $cleanupMessage = $cleanupErrors -join ' | '
        if ($result.Contains('error')) {
            $result.error = "$( $result.error ) Cleanup: $cleanupMessage"
        }
        else {
            $result.error = "Cleanup: $cleanupMessage"
        }
    }
    elseif ($null -eq $startedProcessId -and $null -eq $secondProcessId) {
        $result.process_cleanup = if ($result.status -ceq 'whatif') {
            'not-required'
        }
        else {
            'not-started'
        }
    }
    else {
        $result.process_cleanup = 'complete'
    }
    try {
        Assert-ProcessCleanupState -State $result.process_cleanup | Out-Null
    }
    catch {
        $result.status = 'failed'
        $result.error = $_.Exception.Message
    }
    $result | ConvertTo-Json -Depth 4
}

if ($result.status -notin @('pass', 'whatif')) {
    throw 'Current-user startup verification did not pass.'
}
