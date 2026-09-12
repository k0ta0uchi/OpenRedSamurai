# RED SAMURAI installer boundary

The release zip has a top-level `setup.cmd` launcher and `setup.ps1` entry point.
The launcher delegates to PowerShell with a process-scoped execution-policy
bypass; it does not change the machine or user policy. `setup.ps1` delegates to
`installer/setup.ps1`, which invokes the reviewed current-user scripts below.
From an extracted package, review the plan and then install:

```powershell
.\setup.cmd -WhatIf
.\setup.cmd
```

Use `.\setup.cmd -Uninstall -WhatIf` followed by the same command without
`-WhatIf` to remove the marker-owned installation while preserving product
data. To invoke the PowerShell script directly, first run
`Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass` in that window.
No elevation or machine-wide registry write is used. The package's
`assets/icons/redsamurai.ico` is embedded in the executable and also included
for shell/package inspection.

Maintainers can build the release archive from the repository root with:

```powershell
.\installer\package-release.ps1 -Version 1.0.0
```

The command creates a Windows x64 zip, a SHA-256 sidecar, and a release
manifest under `dist/`.

Phase 4 keeps the Rust integration layer declarative. `src/phase4.rs` builds a
typed install or uninstall plan, validates absolute paths, and produces the
quoted per-user `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
command. It does not open the registry, create directories, launch a process,
or elevate the caller.

The scripts in this directory are the explicit execution boundary:

- `install.ps1` copies the fixed executable name `redsamurai-config.exe`,
  creates the install and resolved product data directories, and registers the stable
  value `RED SAMURAI 16400DPI Gaming Mouse` with the `--tray` argument.
- `uninstall.ps1` removes that value and only removes an install directory that
  contains the install marker written by `install.ps1`. The resolved product
  data directory is always preserved.

The default data location is `Documents\RED SAMURAI 16400DPI Gaming Mouse` when
the known folder is local. If Documents traverses a cloud reparse point such as
OneDrive, both install and uninstall deterministically use
`%LOCALAPPDATA%\OpenRedSamurai\RED SAMURAI 16400DPI Gaming Mouse`. Explicit
`-DataDirectory` paths remain fail-closed and must not traverse reparse points.

`install.ps1 -ExecutableName` is retained as an explicit validation boundary,
but only accepts the default `redsamurai-config.exe`. A custom name could be
registered by install without giving uninstall proof that it is removing the
same startup command, so the installer rejects it before any filesystem or
registry mutation.

Both scripts reject relative paths and `..` components. They use `HKCU`, so a
current-user install does not require elevation. Machine-wide installation is
intentionally not supported by these artifacts; callers should receive an
explicit elevation error rather than silently switching to `HKLM`.

## Review before applying

From an elevated or non-elevated PowerShell prompt, pass explicit absolute
paths and inspect the dry run first:

```powershell
Set-Location .\redsamurai-config
& .\installer\install.ps1 `
  -SourceDirectory 'C:\Build\redsamurai' `
  -InstallDirectory "$env:LOCALAPPDATA\RED SAMURAI" `
  -WhatIf
```

If the output is correct, repeat without `-WhatIf`. Uninstallation follows the
same review-first pattern:

```powershell
& .\installer\uninstall.ps1 `
  -InstallDirectory "$env:LOCALAPPDATA\RED SAMURAI" `
  -WhatIf
```

The scripts never delete the product data under Documents. Close the running
tray process before uninstalling so Windows can remove the executable.

## Disposable live integration check

`live-integration.ps1` is the bounded V-16 check for a Windows operator. It
requires the literal confirmation token, refuses to run when the product Run
value already exists, uses a GUID-named install directory in `%TEMP%` and a
GUID-named data directory under the resolved Documents folder, verifies the
real HKCU registration and idempotent reinstall, then verifies marker-gated
uninstall, data-file SHA-256 preservation, and cleanup of its own disposable
data. With `-VerifyStartup`, it also records the current identity, exact
quoted process command line, PID-scoped process cleanup, and data hashes before
and after the active-session startup check. Snapshot the HKCU Run key before and
after the run, and review the script and close the tray/editor first:

```powershell
& .\installer\live-integration.ps1 `
  -SourceDirectory (Resolve-Path .\target\release).Path `
  -LiveConfirmation RED-SAMURAI-LIVE `
  -VerifyStartup `
  -Confirm:$false
```

The script does not perform HID or `SendInput` actions. Without an explicit
live confirmation token it exits before creating a fixture or mutating HKCU.
If the Windows Documents known folder is redirected through a cloud reparse
point, pass an existing local Documents root explicitly; the safety checks
continue to reject reparse-point paths:

```powershell
& .\installer\live-integration.ps1 `
  -SourceDirectory (Resolve-Path .\target\release).Path `
  -DataRoot 'C:\Users\k0ta0\Documents' `
  -LiveConfirmation RED-SAMURAI-LIVE `
  -VerifyStartup `
  -Confirm:$false
```

## Bounded current-user startup check

`verify-current-user-startup.ps1` is a controlled V-17 subcheck for an already
installed disposable copy. It reads the exact HKCU Run value, launches the
registered executable with `--tray` in the current desktop session, and starts
one second copy to verify the named single-instance mutex. It stops only the
PIDs it started and reports `session_transition_attempted: false` and
`logon_performed: false`; it does not sign out,
restart a shell, simulate a logon, touch Documents data, or write HKCU.

Run it only after reviewing the current-user Run value and closing other product
processes:

```powershell
& .\installer\verify-current-user-startup.ps1 `
  -InstallDirectory 'C:\absolute\disposable\RED SAMURAI' `
  -LiveConfirmation RED-SAMURAI-STARTUP `
  -Confirm:$false
```

A passing result proves the registered command, active-session process launch,
duplicate-instance exit, current-user process identity, and cleanup of the PIDs
started by the verifier. It also records the verifier and started-process
Windows session IDs, which must match. The JSON explicitly reports
`session_transition_attempted: false`, `logon_performed: false`,
`disconnect_recovery_performed: false`, and an
`evidence_boundary` limited to the active desktop session; it is not evidence
of a real sign-out/logon, physical disconnect, or resident-worker recovery.
Those V-17 gates remain operator work. The process command line must exactly
match the quoted HKCU Run value and `--tray`; an unquoted or editor-mode value
fails closed. If PID inspection or termination cannot be verified, the result
is failed with `process_cleanup: unknown` and no broad process cleanup is
attempted.

When `-VerifyStartup` is enabled, the live harness compares the disposable
Documents fixture hash before and after startup and again across uninstall.
On any assertion or cleanup failure it retains the GUID fixture/data paths for
operator review, removes a Run value only when it still exactly matches the
fixture command, and reports cleanup as `not-proven` rather than claiming a
clean run. A `status: pass` record requires the Run value to be absent, the
marker-owned install fixture to be gone, the data hash to be preserved, and
the disposable data/temp fixtures to be verified removed.

## Reboot/logon transition check

`logon-transition-integration.ps1` is the two-phase V-17 boundary check. Run
`Prepare` from the logged-in user, then reboot or sign out and run `Finish`
after the desktop returns. It creates a GUID-scoped install and local
Documents data fixture, records the exact HKCU Run command and data hash,
checks the post-logon process identity/session/command line, and performs
PID-scoped stop plus marker-gated uninstall. Use the local Documents root when
the known folder is a cloud reparse point:

```powershell
& .\installer\logon-transition-integration.ps1 `
  -Phase Prepare `
  -LiveConfirmation RED-SAMURAI-LOGON `
  -DataRoot 'C:\Users\k0ta0\Documents' `
  -Confirm:$false
shutdown.exe /l
# after the desktop returns:
& .\installer\logon-transition-integration.ps1 `
  -Phase Finish `
  -LiveConfirmation RED-SAMURAI-LOGON `
  -TimeoutMs 420000 `
  -Confirm:$false
```

The default Finish window is seven minutes because Explorer processes the
current-user Run values serially; on a busy login the target can start several
minutes after the shell appears. A passing `logon-result.json` proves the
reboot/logon startup and hash-preserving cleanup. If the same Windows session
number is reused, retain both `qwinsta` files and state that a distinct-session
sign-out assertion was not made.

## Permission troubleshooting

Run the live checks from the same interactive Windows account that owns the
device profile. Verify the context before starting:

```powershell
whoami
qwinsta
icacls "$env:USERPROFILE\Documents"
```

`HKCU` and `Documents` are per-user boundaries. A process running as a
sandbox or service account can see the logged-in user's path through
`Environment.SpecialFolder::MyDocuments` while still being unable to create
the product directory, and its `HKCU` writes would target the sandbox user's
hive. Start PowerShell as the logged-in desktop user (`whoami` must show that
account), then run the harness and the resident probe from that window.

`SendInput` also requires an interactive desktop and an integrity level that
is not below the foreground target. Run the probe in the same desktop session
and elevation level as the application receiving the test input; matching the
Documents ACL alone cannot fix a UIPI/input-desktop rejection. If a separate
automation identity must write the data, grant it `Modify` only on the product
directory (after the desktop user creates it), and remove that grant after the
test. That ACL change still does not move `HKCU` or `SendInput` into the
desktop user's context.
