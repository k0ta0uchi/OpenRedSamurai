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
- a native `OpenRedSamurai-Setup.exe` installer with current-user setup and
  GitHub Releases update checks;
- 303 automated tests plus the release static audit and hardware acceptance bundle.

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
3. In that directory, run `OpenRedSamurai-Setup.exe` and press **インストール**.
   The installer is a native GUI executable. It uses HKCU only, requires no
   elevation or PowerShell execution-policy change, copies the application and
   updater into `%LOCALAPPDATA%\RED SAMURAI`, registers current-user tray
   startup, and keeps profile data outside the install tree.
4. To update after a new version is published, open the installed
   `OpenRedSamurai-Setup.exe` (or use **更新を確認** in the application's 情報
   tab), then choose **最新版を確認** and **ダウンロードして更新**. The
   updater accepts only the matching GitHub repository asset and verifies its
   SHA-256 sidecar before replacing files.

   Public releases need no credentials. If the repository is private, set a
   read-only `OPENREDSAMURAI_GITHUB_TOKEN` (or `GH_TOKEN`) for the launching
   user; the native updater sends it only for the GitHub request and does not
   save it.

   If the updater is running from the installed directory, it hands the staged
   payload to a short-lived helper before replacing its own executable. Close
   the editor/tray process when prompted so the helper can finish the update.

The legacy PowerShell entry points remain in the package for audit and controlled
automation. Existing installations may still review `.\setup.cmd -WhatIf`.
To remove an installation from the native GUI, run
`OpenRedSamurai-Setup.exe --uninstall` and press **アンインストール**.

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
.\msvc_cargo.bat build --release --bin OpenRedSamurai-Setup
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
