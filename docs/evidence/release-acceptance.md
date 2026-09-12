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

The release test suite contains 295 tests. The workstation evidence bundle is
the machine-readable final record; its raw paths and hashes are retained in the
local compatibility manifest and acceptance bundle. The release package adds a
second, independently computed SHA-256 sidecar and `release-manifest.json`.

## Artifact checklist

- `redsamurai-config.exe` is built from this source and contains the multi-size
  `assets/icons/redsamurai.ico` Windows resource.
- `setup.cmd`/`setup.ps1` install through `HKCU` only, require no elevation, and
  preserve the resolved product data directory during uninstall. A redirected
  OneDrive Documents folder automatically uses the local AppData fallback.
- `README.md` and `README.ja.md` describe the same v1.0.0 scope and deferred
  boundaries.
- `installer/package-release.ps1` produces the Windows x64 zip and hash sidecar.

The final package produced from this tree is:

| Artifact | SHA-256 |
| --- | --- |
| `target/release/redsamurai-config.exe` | `6557F26013404C565271E366E6EA4476DA81BEA6B685BE40C1021BD701DB2F6C` |
| `target/release/examples/live_probe.exe` | `DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C` |
| `assets/icons/redsamurai.ico` | `D3BED40A1D12889D106AF06B6E9FFE9B6A47B93C7CF50A8EEA53280A01F68F5B` |
| `dist/OpenRedSamurai-v1.0.0-windows-x64.zip` | `C051ED2BE836B440092998AA4968C9F713F72E47F08BBF69560ECBCA4A3818E5` |

The same values are recorded in the zip's `release-manifest.json`, the SHA-256
sidecar, and the GitHub release notes.

The final static audit was generated at
`C:\Workspace\OpenRedSamurai\captures\release-static-audit-20260912-125149`.
Its result is PASS with SHA-256
`3D019C08F0951A2F6E62078D3F4C31DE02FFFA7E1C6C10EBC18CBF27783236D2`, and its
summary SHA-256 is
`82AF95452A3F3896D2081B98DB535ACE2CA04A8C6D4CB9CD83D72842B45980A8`.
The external compatibility manifest was updated with the final binary, audit,
installer fallback audit, and package values (SHA-256
`532CF298F66A516D3B7BD8F47C428EA917EF16B0A8BF75D631C146DDCB7445B2`).

The installer fallback audit at
`C:\Workspace\OpenRedSamurai\captures\installer-fallback-audit-20260912-131509\result.json`
is PASS (SHA-256
`A4A1443D623896B42F610C44639F6437361110046D22A6C05ABC3625D35B699E`). It
ran `setup.cmd -WhatIf`, direct `setup.ps1` with a process-scoped bypass, and
uninstall WhatIf against the OneDrive-redirected Documents environment; all
three resolved the same local AppData fallback and returned exit code 0.

The public v1.0.0 asset was downloaded again at
`C:\Workspace\OpenRedSamurai\captures\release-download-verify-20260912-131840\result.json`
and passed SHA-256 sidecar, `AllSigned` parent plus `setup.cmd`, and direct
PowerShell bypass checks (result SHA-256
`0CF884FEACC6CA1E680ED931156F18F3B887C15BB75DFD2365C89B0431E7FBC5`).
