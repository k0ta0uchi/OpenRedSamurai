# All-keyboard relay audit harness

This is the integration-facing contract for the opt-in Windows relay. The
Rust tests are hardware-free; physical input is manual only and must never be
generated with `SendInput`. The relay is a diagnostic comparison path under
the official-compatible production decision; the manual gates below are not
release requirements and must never be enabled by default.

## Run the hardware-free contract

From `redsamurai-config`:

```powershell
cmd /c msvc_cargo.bat test --all
```

The suite must keep these contracts green without relaxing existing
assertions:

- a target SIDE 7 packet (`VK=0x31`, `MakeCode=0x1E`) emits exactly one
  marked down/up pair, does not forward the physical edge, and ignores the
  marked self-injection echo;
- ordinary keyboard down/up edges are forwarded once with virtual key, scan
  code, flags, timestamp, and direction preserved;
- a held target route releases its mapped output on key-up, device removal,
  and relay shutdown, including the existing reference-count assertions; and
- a failed relay removes the global registration before cleanup completes;
  when the optional hook gate is enabled it is disabled first, while an
  unready/unhealthy hook passes input through.

`cmd /c msvc_cargo.bat build --release --all-targets` is required before any
physical run. Record the release executable's SHA-256 in the run directory.

## Manual gate procedure

1. Capture a pre-run process snapshot. The official processes must be absent:

   ```powershell
   $official = @(
     (Join-Path ${env:ProgramFiles(x86)} 'RED SAMURAI 16400DPI Gaming Mouse\hid.exe'),
     (Join-Path ${env:ProgramFiles(x86)} 'RED SAMURAI 16400DPI Gaming Mouse\ldcfg.exe')
   )
   $before = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
     Where-Object { $official -contains ([string]$_.ExecutablePath) })
   if ($before.Count -ne 0) { throw "official processes present before run" }
   $before | ConvertTo-Json -Depth 4 | Set-Content .\processes-before.json
   ```

2. Start the event logger (and the optional hook logger) and USBPcap capture
   before enabling the relay.
   Record the target Raw Input path, keyboard inventory, layout/IME,
   foreground process and integrity level, marker, relay switch, and UTC
   timestamps. The opt-in switch is
   `REDSAMURAI_ENABLE_KEYBOARD_RELAY=1`; with it absent, prove that no
   usage-wide `RIDEV_NOLEGACY` registration occurs. In the opt-in run, the
   recovery log must first show `keyboard_relay_probe_registered` (`flags=0x2100`)
   and a parsed same-device ordinary A-key (`VK=0x41`, scan `0x1E`) down/up
   arm pair, then `keyboard_relay_registered`
   (`flags=0x2130`).
   The normal opt-in run must also record
   `keyboard_relay_gate disabled mode=raw_input_registration_only`; the
   low-level hook and `keyboard_relay_gate enabled=true` are only expected
   when `REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1` is explicitly set. If the
   probe event never arrives, the run must remain legacy-safe and must not be
   called a relay PASS. Require the corresponding
   `keyboard_relay_forward`/`keyboard_relay_output` records only after active
   registration.

3. With a safe foreground editor, perform these physical actions and record
   both Raw Input and hook/output streams:

   First press and release the ordinary A key (`VK=0x41`, scan `0x1E`) on a
   separate keyboard to arm the probe. Confirm that the recovery log contains
   the complete same-device arm pair
   and active registration; in the default mode confirm that no hook gate is
   enabled. Then clear the foreground editor before pressing SIDE 7. The arm
   pair remains legacy-visible and is not replayed; a replay echo with no
   resolvable Raw Input device is consumed from the relay's short-lived ledger.

   A parallel `WH_KEYBOARD_LL` observer can still log the physical edge before
   the relay hook returns its suppression result. That observer line proves
   that hardware was seen by the hook chain; it does not prove that the edge
   reached the foreground application. The registration-only mode has no
   relay hook, so use the foreground text/clipboard count together with the
   relay recovery log to judge legacy delivery.

   | Gate | Manual action | Required result |
   | --- | --- | --- |
   | Ordinary preservation | A down/up and a short repeat on a separate keyboard | Text and down/up edges are preserved once; no reorder, loss, or stuck key. |
   | SIDE 7 exactly-once | One physical SIDE 7 (`usage=0x1E`, `VK=0x31`) down/up | One target Raw Input pair, one foreground action with no duplicate text/action, and one marked injected down/up pair. A low-level observer's physical line is not counted as foreground delivery. |
   | Self-injection | Observe the relay's marked replay in the same run; use the optional hook mode for hook-level evidence | When the optional hook is enabled, `LLKHF_INJECTED` plus the exact marker passes once and never re-enters the relay; markerless/foreign injection is not re-forwarded. |
   | Recovery | Hold SIDE 7, remove the target, then stop/restart the opt-in relay; repeat with a forced output failure if available | Every relay-owned output gets a marked key-up; gate release and reconnect are logged; no held key remains. |
   | Fail-open | Exercise Raw Input, queue/translation, and output failure paths; add hook failure only in optional hook mode | The no-legacy registration is removed immediately, the optional hook gate is disabled, and ordinary input passes through; cleanup unregisters Raw Input. |

4. Stop the logger/relay, capture a post-run process snapshot, and require
   both official-process count and relay-process count to be zero. Preserve
   the run even on failure; do not turn a missing artifact into a PASS.

### Observation-only comparison

Run the same script with `-ObserveOnly` for the transport comparison. This
sets `REDSAMURAI_KEYBOARD_RELAY_OBSERVE_ONLY=1`; the probe remains legacy-safe,
the A-key pair arms observation, and no Rust replay or resident action is
sent. A successful observation run requires one target Raw Input down/up pair,
zero target output events, zero ordinary forward events, clipboard `1`, and
zero remaining processes. Compare it with the normal registration-only run:
if the normal run produces `11` while observation-only produces `1`, the
duplicate is the expected physical-plus-replay combination rather than a
second target classification.

5. Save these artifacts together: event/recovery/hook logs, foreground
   observation, USBPcap (`.pcap`/`.pcapng`), `result.json`, pre/post process
   snapshots, and the release executable. Generate a manifest with:

   ```powershell
   Get-ChildItem . -File | Get-FileHash -Algorithm SHA256 |
     Select-Object Path,Hash | ConvertTo-Json |
     Set-Content .\sha256.json
   ```

A physical gate is `PASS` only when its required observation, cleanup, and
artifact hashes are present. The resident tray wires the all-keyboard
registration and marked replay behind `REDSAMURAI_ENABLE_KEYBOARD_RELAY=1`;
the low-level suppression gate is a separate diagnostic opt-in. The remaining
work is manual physical validation of registration, ordinary-key fidelity,
target exactly-once delivery, failure recovery, and cleanup. Runtime lifecycle
events are appended to `REDSAMURAI_RECOVERY_LOG` when that variable points to
the run log.
