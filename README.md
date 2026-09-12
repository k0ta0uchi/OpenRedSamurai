# OpenRedSamurai

OpenRedSamurai is a Rust/egui Windows configuration and resident-tray tool for the
RED SAMURAI 16400DPI Gaming Mouse (`VID_04D9`, `PID_FC55`). It reproduces the
official user-mode workflow while keeping device writes behind an evidence-backed,
reviewed boundary.

[日本語 README](README.ja.md) · [Roadmap](ROADMAP.md) · [Verification ledger](VERIFICATION.md)

## v1.0.0 scope

The official-compatible product scope is complete: **17/17 acceptance gates (100%)**.
The release includes:

- profile (`.pfd`) load/save and the egui configuration UI;
- standard Windows HID enumeration for the configuration, mouse, and keyboard
  collections of `VID_04D9/PID_FC55`;
- the reviewed 156-report Apply sequence for the evidence-backed profile fields;
- Rust-owned resident input, disconnect recovery, tray mode, and current-user
  startup registration;
- native pass-through for the device's existing keyboard usages;
- software button actions through the Windows `SendInput` and Core Audio boundaries;
- macro recording/playback, combo assignment, microphone mute, DPI controls, and
  UI accessibility identifiers;
- a multi-size `redsamurai.ico` embedded in the executable and shipped with the
  installer package;
- 295 automated tests plus the release static audit and hardware acceptance bundle.

The supported runtime uses the standard Windows `usbccgp`/`HidUsb`/`kbdhid`/`mouhid`
stack. No kernel filter, test-signed driver, or official `hid.exe` process is needed.
The following remain explicit non-blocking `DEFERRED-BY-DESIGN` extensions: full
Rust-only Report-03 reconnect context, a kernel filter/test-signed driver, active
all-keyboard `RIDEV_NOLEGACY` suppression, and unverified P2 wire mappings. They are
not guessed or silently enabled.

## Install the release

1. Download `OpenRedSamurai-v1.0.0-windows-x64.zip` from the
   [v1.0.0 GitHub release](https://github.com/k0ta0uchi/OpenRedSamurai/releases/tag/v1.0.0).
2. Extract it to a directory you control.
3. In that directory, review and run:

   ```powershell
   Set-Location 'C:\Path\to\OpenRedSamurai-v1.0.0-windows-x64'
   .\setup.cmd -WhatIf
   .\setup.cmd
   ```

   `setup.cmd` starts PowerShell with a process-scoped execution-policy bypass;
   it does not change the machine or user policy. If you invoke the script
   directly, run `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass`
   in that PowerShell window first. The installer copies `redsamurai-config.exe`
   to the current user's LocalAppData, creates the product data directory under
   Documents when that folder is local, and automatically uses
   `%LOCALAPPDATA%\OpenRedSamurai\RED SAMURAI 16400DPI Gaming Mouse` when
   Documents is redirected through a cloud reparse point. You can override the
   path with `-DataDirectory` when invoking `installer/install.ps1` directly.
   It registers one quoted `--tray` command under
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. It does not elevate
   or write machine-wide state. To remove the installation while preserving user
   data, run `.\setup.cmd -Uninstall -WhatIf` and then repeat without `-WhatIf`.

The packaged `installer/` directory contains the individual reviewed scripts for
auditing or controlled automation. Close the tray process before uninstalling.

## Build and test

Use the included MSVC wrapper from a Visual Studio 2022 developer environment:

```powershell
Set-Location .\redsamurai-config
.\msvc_cargo.bat fmt --all -- --check
.\msvc_cargo.bat check --all-targets --no-default-features
.\msvc_cargo.bat test --all -- --test-threads=1
.\msvc_cargo.bat build --release
```

The release package can be produced with:

```powershell
.\installer\package-release.ps1 -Version 1.0.0
```

The script writes the zip, SHA-256 sidecar, and a machine-readable manifest to
`dist/` (which is intentionally ignored by Git).

## Safety and data boundaries

All installation state is current-user state. The product data directory is kept in
`Documents\RED SAMURAI 16400DPI Gaming Mouse` when Documents is local. If the
known folder is redirected through OneDrive or another reparse-point provider,
the installer uses `%LOCALAPPDATA%\OpenRedSamurai\RED SAMURAI 16400DPI Gaming Mouse`
instead. Uninstall deliberately preserves whichever data directory was resolved.
The application starts with a dry-run plan, performs no HID I/O on startup, and only
allows the complete verified Apply sequence to cross the device boundary. Local
captures and generated hardware evidence stay outside the repository and are ignored
by `.gitignore`.

The compact release evidence summary is tracked at
[`docs/evidence/release-acceptance.md`](docs/evidence/release-acceptance.md).

## License

OpenRedSamurai is released under the [MIT License](LICENSE).
