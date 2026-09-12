<#
.SYNOPSIS
    Side-effect-contained checks for the installer boundary.

.DESCRIPTION
    This harness only creates and removes a uniquely named temporary fixture.
    Installer invocations use -WhatIf and therefore never write HKCU or the
    filesystem.  It deliberately does not require a device or a profile.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Join-Path ([IO.Path]::GetTempPath()) (
    'redsamurai-installer-harness-{0}' -f [Guid]::NewGuid().ToString('N')
)
$source = Join-Path $root 'source'
$install = Join-Path $root 'install'
$data = Join-Path $root 'data'
$nestedInstall = Join-Path $root 'nested-install'
$invalidMarkerInstall = Join-Path $root 'invalid-marker-install'
$installScript = Join-Path $PSScriptRoot 'install.ps1'
$uninstallScript = Join-Path $PSScriptRoot 'uninstall.ps1'
$liveScript = Join-Path $PSScriptRoot 'live-integration.ps1'
$startupScript = Join-Path $PSScriptRoot 'verify-current-user-startup.ps1'
$logonScript = Join-Path $PSScriptRoot 'logon-transition-integration.ps1'
$startupHelperScript = Join-Path $PSScriptRoot 'startup-verification-helpers.ps1'
$commonScript = Join-Path $PSScriptRoot 'common.ps1'

function Assert-CommandFailure {
    param(
        [Parameter(Mandatory = $true)] [scriptblock] $Action,
        [Parameter(Mandatory = $true)] [string] $ExpectedMessage
    )

    $failed = $false
    $message = $null
    try {
        & $Action 2>&1 | Out-Null
    }
    catch {
        $failed = $true
        $message = $_.Exception.Message
    }
    if (-not $failed) {
        throw "expected installer failure: $ExpectedMessage"
    }
    if ($message -cne $ExpectedMessage) {
        throw "unexpected installer failure: $message"
    }
}

function Assert-HelperFailure {
    param(
        [Parameter(Mandatory = $true)] [scriptblock] $Action,
        [Parameter(Mandatory = $true)] [string] $ExpectedMessage
    )

    $failed = $false
    $message = $null
    try {
        & $Action | Out-Null
    }
    catch {
        $failed = $true
        $message = $_.Exception.Message
    }
    if (-not $failed) {
        throw "expected validation failure: $ExpectedMessage"
    }
    if ($message -cne $ExpectedMessage) {
        throw "unexpected validation failure: $message"
    }
}

try {
    foreach ($scriptPath in @($commonScript, $installScript, $uninstallScript, $liveScript, $startupScript, $logonScript, $startupHelperScript)) {
        $tokens = $null
        $parseErrors = $null
        [System.Management.Automation.Language.Parser]::ParseFile(
            $scriptPath,
            [ref] $tokens,
            [ref] $parseErrors
        ) | Out-Null
        if ($parseErrors.Count -ne 0) {
            throw "PowerShell parse failed: $scriptPath"
        }
    }
    $installText = Get-Content -LiteralPath $installScript -Raw
    $uninstallText = Get-Content -LiteralPath $uninstallScript -Raw
    if ($installText -notmatch 'SupportsShouldProcess' -or
        $uninstallText -notmatch 'SupportsShouldProcess') {
        throw 'installer scripts must support ShouldProcess'
    }
    if ($installText -match '(?i)HKLM:' -or $uninstallText -match '(?i)HKLM:') {
        throw 'installer scripts must not reference HKLM'
    }
    $liveText = Get-Content -LiteralPath $liveScript -Raw
    if ($liveText -notmatch 'RED-SAMURAI-LIVE' -or
        $liveText -notmatch 'refusing live integration') {
        throw 'live integration must have an explicit confirmation gate'
    }
    if ($liveText -notmatch 'VerifyStartup' -or
        $liveText -notmatch 'verify-current-user-startup.ps1') {
        throw 'live integration must expose the bounded current-user startup check'
    }
    if ($liveText -notmatch '\$current\.\$RunValueName\s+-ceq\s+\$expectedRunCommand') {
        throw 'live integration cleanup must remove only its exact Run command'
    }
    if ($liveText -match 'Stop-FixtureTrayProcesses' -or
        $liveText -match 'Where-Object\s+\{\s*\[string\]\$_.ExecutablePath') {
        throw 'live integration must not clean up by enumerating every matching process'
    }
    if ($liveText -match 'ErrorAction\s+SilentlyContinue') {
        throw 'live integration cleanup must not hide an inspection or deletion failure'
    }
    if ($liveText -notmatch 'data_sha256_before_startup' -or
        $liveText -notmatch 'data_sha256_after_startup') {
        throw 'live integration must prove fixture data survives the startup check'
    }
    $startupText = Get-Content -LiteralPath $startupScript -Raw
    if ($startupText -notmatch 'RED-SAMURAI-STARTUP' -or
        $startupText -notmatch 'current-user-session' -or
        $startupText -notmatch 'logon_performed' -or
        $startupText -notmatch 'session_transition_attempted' -or
        $startupText -notmatch 'evidence_boundary' -or
        $startupText -notmatch 'current_session_id') {
        throw 'current-user startup verification must declare its bounded scope'
    }
    if ($startupText -match '(?i)sign.?out|logoff|shutdown') {
        throw 'current-user startup verification must not perform a session transition'
    }
    if ($startupText -match '\$unquotedExpectedCommand' -or
        $startupText -match 'CommandLine\s+-cne\s+\$expectedCommand\s+-and') {
        throw 'current-user startup verification must require the exact quoted process command line'
    }
    if ($startupText -notmatch 'Assert-ExactTrayProcessCommandLine' -or
        $startupText -notmatch 'Assert-ProcessUserIdentity' -or
        $startupText -notmatch 'Assert-ProcessCleanupState') {
        throw 'current-user startup verification must use exact command, identity, and fail-closed cleanup helpers'
    }
    $logonText = Get-Content -LiteralPath $logonScript -Raw
    if ($logonText -notmatch '\[ValidateRange\(1000, 600000\)\]' -or
        $logonText -notmatch '\$TimeoutMs\s*=\s*420000') {
        throw 'logon transition verification must allow the delayed Windows Run queue to be observed'
    }
    if ($logonText -notmatch '\$creation\s+-is\s+\[DateTime\]' -or
        $logonText -notmatch 'ManagementDateTimeConverter') {
        throw 'logon transition verification must normalize both PowerShell DateTime and WMI creation values'
    }
    if ($startupText -notmatch 'SupportsShouldProcess' -or
        $startupText -notmatch "status = 'whatif'" -or
        $startupText -notmatch 'process_cleanup = if \(\$result\.status -ceq ''whatif''\)') {
        throw 'current-user startup verification must expose a non-launching WhatIf path'
    }
    Assert-CommandFailure {
        & $liveScript -SourceDirectory (Join-Path $root 'missing-source')
    } 'Live integration requires -LiveConfirmation RED-SAMURAI-LIVE.'
    Assert-CommandFailure {
        & $startupScript -InstallDirectory (Join-Path $root 'missing-install') `
            -LiveConfirmation WRONG-CONFIRMATION
    } 'Current-user startup verification requires -LiveConfirmation RED-SAMURAI-STARTUP.'

    . $commonScript
    . $startupHelperScript
    $originalReparseProbe = (Get-Command Test-PathTraversesReparsePoint -CommandType Function).ScriptBlock
    try {
        # Force the branch that is difficult to reproduce on every test host:
        # a cloud-redirected Documents known folder.  The resolver must use a
        # deterministic local path, while explicit paths remain fail-closed.
        Set-Item -Path Function:Test-PathTraversesReparsePoint -Value {
            param([string] $Path)
            return $true
        }
        if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
            throw 'LOCALAPPDATA is required for the default data fallback test.'
        }
        $forcedFallback = Get-DefaultDataDirectory -ProductName 'RED SAMURAI 16400DPI Gaming Mouse'
        $expectedFallback = Join-Path $env:LOCALAPPDATA 'OpenRedSamurai\RED SAMURAI 16400DPI Gaming Mouse'
        if ($forcedFallback -cne $expectedFallback) {
            throw 'redirected Documents did not resolve to the deterministic local fallback.'
        }
    }
    finally {
        Set-Item -Path Function:Test-PathTraversesReparsePoint -Value $originalReparseProbe
    }
    if ((Assert-SafeAbsolutePath -Path 'C:\Users\tester\Install' `
            -Name 'InstallDirectory' -DisallowRoot) -cne 'C:\Users\tester\Install') {
        throw 'validated path was not returned deterministically'
    }
    Assert-HelperFailure {
        Assert-SafeAbsolutePath -Path 'relative\Install' -Name 'InstallDirectory'
    } 'InstallDirectory must be an absolute Windows path: relative\Install'
    Assert-HelperFailure {
        Assert-SafeAbsolutePath -Path 'C:\Install\..\evil' -Name 'InstallDirectory'
    } 'InstallDirectory contains a parent path component: C:\Install\..\evil'
    Assert-HelperFailure {
        Assert-SafeAbsolutePath -Path 'C:\Install\.\evil' -Name 'InstallDirectory'
    } 'InstallDirectory contains a dot path component: C:\Install\.\evil'
    Assert-HelperFailure {
        Assert-SafeAbsolutePath -Path 'C:\' -Name 'InstallDirectory' -DisallowRoot
    } 'InstallDirectory must not be a filesystem root: C:\'
    Assert-HelperFailure {
        Assert-SafeExecutableName 'bad?.exe'
    } 'ExecutableName contains invalid Windows file-name characters.'
    Assert-HelperFailure {
        Assert-SafeExecutableName 'CON.exe'
    } 'ExecutableName uses a reserved Windows file name.'
    Assert-HelperFailure {
        Assert-DirectoriesDoNotOverlap -InstallDirectory 'C:\Install' `
            -DataDirectory 'C:\Install\profiles'
    } 'DataDirectory must not be the install directory or a child of InstallDirectory.'
    if ((Quote-WindowsArgument 'C:\a\') -cne '"C:\a\\"' -or
        (Quote-WindowsArgument 'say "hello"') -cne '"say \"hello\""') {
        throw 'Windows command-line quoting is not deterministic'
    }
    $startupExecutable = 'C:\Install\redsamurai-config.exe'
    $startupCommand = Get-TrayAutostartCommand -ExecutablePath $startupExecutable
    if ($startupCommand -cne '"C:\Install\redsamurai-config.exe" --tray') {
        throw 'current-user tray startup command was not deterministic'
    }
    if (-not (Assert-TrayAutostartCommand -ExecutablePath $startupExecutable `
            -Command $startupCommand)) {
        throw 'valid current-user tray startup command was rejected'
    }
    Assert-HelperFailure {
        Assert-TrayAutostartCommand -ExecutablePath $startupExecutable `
            -Command '"C:\Install\redsamurai-config.exe" --editor'
    } 'Current-user Run command must be exactly the quoted executable followed by --tray.'
    if (-not (Assert-ExactTrayProcessCommandLine -ExpectedCommand $startupCommand `
            -ActualCommandLine $startupCommand -ProcessId 17)) {
        throw 'exact tray process command line was rejected'
    }
    Assert-HelperFailure {
        Assert-ExactTrayProcessCommandLine -ExpectedCommand $startupCommand `
            -ActualCommandLine 'C:\Install\redsamurai-config.exe --tray' -ProcessId 17
    } 'Started process command line must exactly match the quoted Run command for PID 17.'
    if (-not (Assert-ProcessUserIdentity -ExpectedIdentity 'KOTA-PC\k0ta0' `
            -ActualIdentity 'kota-pc\K0TA0' -ProcessId 17)) {
        throw 'matching process identity was rejected'
    }
    Assert-HelperFailure {
        Assert-ProcessUserIdentity -ExpectedIdentity 'KOTA-PC\k0ta0' `
            -ActualIdentity 'KOTA-PC\other' -ProcessId 17
    } 'Started process identity did not match current user for PID 17.'
    Assert-HelperFailure {
        Assert-ProcessCleanupState -State 'unknown'
    } 'Process cleanup could not be verified; refusing to claim completion.'
    Assert-HelperFailure {
        Get-ProcessUserIdentity -ProcessRecord ([pscustomobject]@{ ProcessId = 17 })
    } 'Process user identity could not be inspected for PID 17.'
    if (-not (Assert-ProcessSession -ExpectedSessionId 42 -ActualSessionId 42 -ProcessId 17)) {
        throw 'matching process session was rejected'
    }
    Assert-HelperFailure {
        Assert-ProcessSession -ExpectedSessionId 42 -ActualSessionId 43 -ProcessId 17
    } 'Started process session did not match verifier session for PID 17.'

    $markerRoot = Join-Path $root 'marker'
    New-Item -ItemType Directory -Path $markerRoot -Force | Out-Null
    $markerPath = Get-InstallMarkerPath -InstallDirectory $markerRoot
    if ((Get-InstallMarkerState -MarkerPath $markerPath) -cne 'Absent') {
        throw 'missing marker was not classified as absent'
    }
    Write-InstallMarker -MarkerPath $markerPath
    if ((Get-InstallMarkerState -MarkerPath $markerPath) -cne 'Valid') {
        throw 'exact marker was not classified as valid'
    }
    $markerBytes = [Convert]::ToBase64String([IO.File]::ReadAllBytes($markerPath))
    $expectedMarkerBytes = [Convert]::ToBase64String(
        [Text.UTF8Encoding]::new($false).GetBytes('redsamurai-config')
    )
    if ($markerBytes -cne $expectedMarkerBytes) {
        throw 'marker encoding was not deterministic'
    }
    Write-InstallMarker -MarkerPath $markerPath
    if ((Get-InstallMarkerState -MarkerPath $markerPath) -cne 'Valid') {
        throw 'rewriting an exact marker was not idempotent'
    }

    New-Item -ItemType Directory -Path $source -Force | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $source 'redsamurai-config.exe'),
        'test executable',
        [Text.UTF8Encoding]::new($false)
    )
    [IO.File]::WriteAllText(
        (Join-Path $source 'custom.exe'),
        'custom executable',
        [Text.UTF8Encoding]::new($false)
    )

    # A custom executable name would be registered by install but cannot be
    # proven to uninstall safely because uninstall only knows the default.
    Assert-CommandFailure {
        & $installScript -SourceDirectory $source -InstallDirectory $install `
            -DataDirectory $data -ExecutableName 'custom.exe' -WhatIf -Confirm:$false
    } 'ExecutableName must be the default redsamurai-config.exe; custom names cannot be safely uninstalled.'

    # An unmarked non-empty directory must never be overwritten, even in a
    # dry run.  This is the first red test for marker-aware idempotence.
    New-Item -ItemType Directory -Path $install -Force | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $install 'unrelated.txt'),
        'user data',
        [Text.UTF8Encoding]::new($false)
    )
    Assert-CommandFailure {
        & $installScript -SourceDirectory $source -InstallDirectory $install `
            -DataDirectory $data -WhatIf -Confirm:$false
    } 'InstallDirectory must be empty or contain a trusted RED SAMURAI marker.'

    # Data nested below the install root would be deleted by a marker-gated
    # uninstall, so the boundary must reject it before any ShouldProcess call.
    $nestedData = Join-Path $nestedInstall 'profiles'
    Assert-CommandFailure {
        & $installScript -SourceDirectory $source -InstallDirectory $nestedInstall `
            -DataDirectory $nestedData -WhatIf -Confirm:$false
    } 'DataDirectory must not be the install directory or a child of InstallDirectory.'

    # An existing marker is trusted only when its bytes are exactly the
    # installer marker; a near miss must fail closed instead of being replaced.
    New-Item -ItemType Directory -Path $invalidMarkerInstall -Force | Out-Null
    [IO.File]::WriteAllText(
        (Join-Path $invalidMarkerInstall '.redsamurai-install'),
        'not-redsamurai',
        [Text.UTF8Encoding]::new($false)
    )
    Assert-CommandFailure {
        & $installScript -SourceDirectory $source -InstallDirectory $invalidMarkerInstall `
            -DataDirectory $data -WhatIf -Confirm:$false
    } 'InstallDirectory contains an invalid RED SAMURAI marker.'

    # A valid dry-run must report no filesystem side effects.  Registry access
    # is read-only here; ShouldProcess blocks every mutation.
    $whatIfInstall = Join-Path $root 'whatif-install'
    $whatIfData = Join-Path $root 'whatif-data'
    $sourceHashBefore = (Get-FileHash -LiteralPath (Join-Path $source 'redsamurai-config.exe') -Algorithm SHA256).Hash
    & $installScript -SourceDirectory $source -InstallDirectory $whatIfInstall `
        -DataDirectory $whatIfData -WhatIf -Confirm:$false | Out-Null
    if ((Test-Path -LiteralPath $whatIfInstall) -or
        (Test-Path -LiteralPath $whatIfData) -or
        (Test-Path -LiteralPath (Join-Path $whatIfInstall '.redsamurai-install'))) {
        throw 'install -WhatIf changed the filesystem'
    }
    $sourceHashAfter = (Get-FileHash -LiteralPath (Join-Path $source 'redsamurai-config.exe') -Algorithm SHA256).Hash
    if ($sourceHashBefore -cne $sourceHashAfter) {
        throw 'install -WhatIf changed the source executable'
    }

    $protectedInstall = Join-Path $root 'protected-install'
    $protectedData = Join-Path $root 'protected-data'
    New-Item -ItemType Directory -Path $protectedInstall -Force | Out-Null
    New-Item -ItemType Directory -Path $protectedData -Force | Out-Null
    Write-InstallMarker -MarkerPath (Get-InstallMarkerPath -InstallDirectory $protectedInstall)
    [IO.File]::WriteAllText(
        (Join-Path $protectedData 'profile.pfd'),
        'must survive uninstall',
        [Text.UTF8Encoding]::new($false)
    )
    & $uninstallScript -InstallDirectory $protectedInstall -DataDirectory $protectedData `
        -WhatIf -Confirm:$false | Out-Null
    if (-not (Test-Path -LiteralPath (Get-InstallMarkerPath -InstallDirectory $protectedInstall)) -or
        -not (Test-Path -LiteralPath (Join-Path $protectedData 'profile.pfd'))) {
        throw 'uninstall -WhatIf changed marker-owned install or Documents data'
    }
    Assert-CommandFailure {
        & $uninstallScript -InstallDirectory $invalidMarkerInstall -DataDirectory $data `
            -WhatIf -Confirm:$false
    } 'InstallDirectory contains an invalid RED SAMURAI marker; refusing to remove it.'

    Write-Output 'installer harness: PASS'
}
finally {
    if (Test-Path -LiteralPath $root) {
        Remove-Item -LiteralPath $root -Recurse -Force
    }
}
