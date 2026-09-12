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
- `LICENSE` applies to the original source; `NOTICE.md` keeps bundled visual
  and compatibility resources under any separate upstream terms.
- `installer/package-release.ps1` produces the Windows x64 zip, sidecar, and
  manifest entries for both the editor and native installer.

The final package produced from this tree is:

| Artifact | SHA-256 |
| --- | --- |
| `target/release/redsamurai-config.exe` | `096BBE1083145A549EBAF6F80757139D5ABF520DFDCED76071F89CEC57009523` |
| `target/release/OpenRedSamurai-Setup.exe` | `6B766C760BF2578B11378A9B82F1BE9F2E4C836156517850A5CA3205BEAA253E` |
| `target/release/examples/live_probe.exe` | `9839AAF6A1924EC0746A9F92F3125D39DBFC60D379E1D5640DDC892FD636E3CA` |
| `assets/icons/redsamurai.ico` | `D3BED40A1D12889D106AF06B6E9FFE9B6A47B93C7CF50A8EEA53280A01F68F5B` |
| `dist/OpenRedSamurai-v1.0.0-windows-x64.zip` | `3D6AA4A2925E238775617E0873305A5FE641F597C9EB94857842634045F7CE5E` |
| `dist/OpenRedSamurai-v1.0.0-windows-x64.zip.sha256` | `4F820FFC45ADBC844D73D6EB4BEEF07FA42A92B24C833DFE6046CBE95C832AF0` |

The same values are recorded in the zip's `release-manifest.json`, the SHA-256
sidecar, and the GitHub release notes.

The final source tree test run is recorded at
`C:\Workspace\OpenRedSamurai\captures\release-static-audit-20260912-184019\test.log`
with 303 passing tests and zero failures (log SHA-256
`AE0284A7B4564E7506C4235EF90506AB07359F550CF8D943E518C2864E5975DB`).
The latest static audit is
`C:\Workspace\OpenRedSamurai\captures\release-static-audit-20260912-184019\result.json`
with status `pass`, result SHA-256
`C7A9C0BBB40AEA27D8E6154CB94B03544F2FD3C01C3F4C85103657214A229AEB`, summary
SHA-256 `A3339725D94359D85ED53DE1D61FB7A5072898BBDE7F465119A4FDD26891B002`,
and zero tracked processes after the audit.
The external compatibility manifest at
`C:\Workspace\OpenRedSamurai\captures\official-compatibility-manifest-20260912.json`
was refreshed with the final hashes and has SHA-256
`6EDC1697CB7FDC44AB9307C383083549FACABA76AD231A1FDC8487DE5E4A3721`.
The final UI accessibility smoke passed with 36/34/74/20 nodes and the
installer update identifier; its editor executable SHA-256 is
`096BBE1083145A549EBAF6F80757139D5ABF520DFDCED76071F89CEC57009523`.
The modal readability capture is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\modal-readability-20260912.png`
(SHA-256 `F867DB5B9C5C40B37AF4B4435DB927BE7EAFB9DB3241A61E3842C6B1567F0CEF`);
the dialog body is fully bright while only the background backdrop is dimmed.
The final UI capture is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\ui-release-20260912-183422\`
with general-tab SHA-256 `A8A12CA745F822A0E0A254D9384B6170B9CDDC5F249BE146E04EE0BD17E8B89E`
and info-tab SHA-256 `63090778B048D319BFC67C0D3E5624D4E17CB5203295D0A5DCED39F59E7E6D10`.
The native installer capture is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\installer-public-update-20260912-183326\installer.png`
(SHA-256 `D2721B359DD3212D91F092A139D40A5860010895722BBE0B627B3CB5316E8AA0`),
and the unauthenticated public-release update result is
`C:\Workspace\OpenRedSamurai\redsamurai-config\captures\installer-public-update-20260912-183326\result.json`
(SHA-256 `7A1AE8D30548E3E9E963712888C94C696B34A1890F9D42559455C10876ADA1F8`).

The installer fallback audit at
`C:\Workspace\OpenRedSamurai\captures\installer-fallback-audit-20260912-131509\result.json`
is PASS (SHA-256
`A4A1443D623896B42F610C44639F6437361110046D22A6C05ABC3625D35B699E`). It
ran `setup.cmd -WhatIf`, direct `setup.ps1` with a process-scoped bypass, and
uninstall WhatIf against the OneDrive-redirected Documents environment; all
three resolved the same local AppData fallback and returned exit code 0.

The native updater GUI was smoke-tested against the public GitHub repository
without a token: it displayed `最新版です: v1.0.0` and no 404/error state. The
result is recorded above. Private forks can still use a short-lived read-only
`OPENREDSAMURAI_GITHUB_TOKEN`/`GH_TOKEN`; the token is read only from the
launching environment and is never written to disk. The package manifest and
sidecar above were generated after the final release build.
