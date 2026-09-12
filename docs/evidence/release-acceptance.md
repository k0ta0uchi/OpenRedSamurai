# OpenRedSamurai v1.0.0 release evidence

This tracked summary records the release decision without copying large PCAP and
UI capture files into Git. The raw captures remain on the validation workstation
under `C:\Workspace\OpenRedSamurai\captures`.

## Decision

**PASS — 17/17 product gates (100%)** for the official-compatible user-mode
scope. The scope covers standard Windows HID, the reviewed 156-report Apply
sequence, Rust-owned tray and recovery, native keyboard pass-through, bounded
user-mode actions, macro/combo flows, microphone mute, and current-user startup.

The release intentionally defers four non-blocking extensions:

1. Rust-only full Report-03 reconnect context (the device returns a short
   standalone response, so no guessed replay is sent).
2. A kernel HID filter or test-signed driver.
3. Active all-keyboard `RIDEV_NOLEGACY` suppression.
4. Unverified P2 wire mappings.

Each deferred item has a documented restart condition: a reviewed complete
sequence, zero-status transfers, reconnect readback, and fresh SHA-256 evidence.

## Reproducible checks

The final release run repeats these commands from the repository root:

```powershell
.\msvc_cargo.bat fmt --all -- --check
.\msvc_cargo.bat check --all-targets --no-default-features
.\msvc_cargo.bat test --all -- --test-threads=1
.\msvc_cargo.bat build --release
.\installer\package-release.ps1 -Version 1.0.0 -SkipBuild
```

The release test suite contains 303 tests. The workstation evidence bundle is
the machine-readable final record; its raw paths and hashes are retained in the
local compatibility manifest and acceptance bundle. The release package adds a
second, independently computed SHA-256 sidecar and `release-manifest.json`.

## Artifact checklist

- `redsamurai-config.exe` is built from this source and contains the multi-size
  `assets/icons/redsamurai.ico` Windows resource.
- `OpenRedSamurai-Setup.exe` is a native Windows GUI installer built from
  `src/bin/installer.rs`. It performs current-user install/update/uninstall,
  registers the quoted `--tray` command under HKCU, and verifies GitHub release
  ZIP downloads with their SHA-256 sidecar before extraction.
- The editor uses a shared compact geometry: the main cards start at y=145,
  the profile row is separated from the cards, and the footer actions share a
  single baseline with 10px gaps. Active profile labels reserve a text column
  beside the status dot.
- Modal backdrops remain on the middle layer while dialog windows are placed on
  the foreground layer. Dialog fade-in is disabled so the modal body and its
  controls stay fully readable from the first visible frame. The tray icon is
  decoded from the embedded `assets/icons/redsamurai.png` logo.
- The legacy `setup.cmd`/`setup.ps1` path and the native installer both install
  through `HKCU` only, require no elevation, and preserve the resolved product
  data directory during uninstall. A redirected OneDrive Documents folder
  automatically uses the local AppData fallback.
- `README.md` and `README.ja.md` describe the same v1.0.0 scope and deferred
  boundaries.
- `installer/package-release.ps1` produces the Windows x64 zip, sidecar, and
  manifest entries for both the editor and native installer.

The final package produced from this tree is:

| Artifact | SHA-256 |
| --- | --- |
| `target/release/redsamurai-config.exe` | `6B81C67A1B0BFFDAC75F11C6D08B147D981DBC82D310D30A9FEBEF83FAF6CA62` |
| `target/release/OpenRedSamurai-Setup.exe` | `332CC662A7E85F08D6F9ADC026122BAFC4782FD39165627F8298042A7F10441F` |
| `target/release/examples/live_probe.exe` | `D42E6AB1D3DE79EC5F1C6558AE83B23C56FDEEB84FD992D0EFEB24DC5A365826` |
| `assets/icons/redsamurai.ico` | `D3BED40A1D12889D106AF06B6E9FFE9B6A47B93C7CF50A8EEA53280A01F68F5B` |
| `dist/OpenRedSamurai-v1.0.0-windows-x64.zip` | `776B72E742C8924EA19E921A830464511A2A784BA218863B7493AA82FAD3EF1A` |
| `dist/OpenRedSamurai-v1.0.0-windows-x64.zip.sha256` | `8DA846DD3BEE1324E780E2160A970D67BDA7D3F1FEDCD1F589BB5D9E9754F04D` |

The same values are recorded in the zip's `release-manifest.json`, the SHA-256
sidecar, and the GitHub release notes.

The final source tree test run is recorded at
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\full-test-20260912-150044\cargo-test.log`
with 303 passing tests and zero failures (log SHA-256
`D0B88D0B0E3ED38218A910ABE8E50B82810CDE49C4DA62A5EE455BCA5BC7CC53`).
The latest static audit is
`C:\Workspace\OpenRedSamurai\captures\release-static-audit-20260912-150723\result.json`
with status `pass`, result SHA-256
`0ED38C5F3FDE93B37426D9F97293268D8971A83F2685E4908F0EAEA9B5D4EA9A`, summary
SHA-256 `C17B5CC1583AC941BB4B2107DFE63A4F430A5712ED5E26031FD048E8E1773B3E`,
and zero tracked processes after the audit.
The final UI accessibility smoke passed with 36/34/74/20 nodes and the
installer update identifier; its editor executable SHA-256 is
`6B81C67A1B0BFFDAC75F11C6D08B147D981DBC82D310D30A9FEBEF83FAF6CA62`.
The modal readability capture is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\modal-readability-20260912.png`
(SHA-256 `F867DB5B9C5C40B37AF4B4435DB927BE7EAFB9DB3241A61E3842C6B1567F0CEF`);
the dialog body is fully bright while only the background backdrop is dimmed.
The native installer Japanese-font capture is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\installer-japanese-20260912.png`
(SHA-256 `F111F8BFB96F7F7DFA1689B6581386033AC306097CE60737EACCFCDA333A7F8C`).

The installer fallback audit at
`C:\Workspace\OpenRedSamurai\captures\installer-fallback-audit-20260912-131509\result.json`
is PASS (SHA-256
`A4A1443D623896B42F610C44639F6437361110046D22A6C05ABC3625D35B699E`). It
ran `setup.cmd -WhatIf`, direct `setup.ps1` with a process-scoped bypass, and
uninstall WhatIf against the OneDrive-redirected Documents environment; all
three resolved the same local AppData fallback and returned exit code 0.

The native updater GUI was smoke-tested against the private GitHub repository
with a short-lived `gh auth token`: it displayed the current release and no
404/error state. The result is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\installer-update-auth-20260912\result.json`
(SHA-256 `0BC14B083BAC7B2ED541663579EA80412D063CD6BC6E2D551E3D15D1535924BC`).
The token is read only from the launching environment and is never written to
disk. The package manifest and sidecar above were generated after the final
release build.
