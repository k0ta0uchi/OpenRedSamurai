# RED SAMURAI verification audit and execution plan

Date of this audit: 2026-09-10 (Asia/Tokyo)
Last updated: 2026-09-12 (Asia/Tokyo)

This document is the verification ledger for the current repository. It separates
what is proven by Rust tests or checked-in captures from what still needs an
operator, a Windows host, a real mouse, or a real runtime. A result marked
`PASS (existing evidence)` refers to evidence already in this repository; it is
not a claim that the same hardware action was repeated during this audit.

The audit covers `ANALYSIS.md`, `redsamurai-config/README.md`, all four
`CONTRACT_*.md` files, the Rust source and tests, the installer scripts, and the
checked-in capture directories. This file does not authorize a new protocol
mapping and does not replace the fail-closed rules in the source.

The historical all-keyboard duplicate-delivery experiment is recorded in
[`docs/keyboard-relay-design.md`](docs/keyboard-relay-design.md). The current
product acceptance direction is official-compatible user-mode behavior:
standard Windows HID delivery, the same observed feature-report boundary, and
one user-mode action path for software-only assignments. The relay's
`RIDEV_NOLEGACY` registration and self-injection marker remain diagnostic
opt-ins and are not a release requirement.

The current v1.0.1 release build is `target/release/redsamurai-config.exe`
(SHA-256 `88BAEA05054A82E72A81A12CD766C5BDD4A6DF24038A1D39822A8C2814B37A98`)
and `target/release/examples/live_probe.exe` (SHA-256
`9839AAF6A1924EC0746A9F92F3125D39DBFC60D379E1D5640DDC892FD636E3CA`).

## Official-compatible user-mode decision — 2026-09-12

The acceptance criterion is reproduction of the installed vendor method. The
observed device uses the Microsoft composite/HID stack (`usbccgp`, `HidUsb`,
`kbdhid`/`kbdclass`, and `mouhid`/`mouclass`) for `MI_00`, `MI_01`, and `MI_02`;
no RED SAMURAI kernel service or filter was present in the inspected device
stacks. The installed `hid.exe` imports the vendor `hidapi.dll` feature-report
API, while the installed `KBHook.dll` and `keydll3.dll` export user-mode
`WH_KEYBOARD_LL` and `WH_MOUSE_LL` helpers. These observations establish the
method boundary; they do not require binary compatibility with the vendor
DLLs.

The bounded disassembly detail is recorded separately: an enabled
`KBHook.dll::_hookproc@12` transforms selected scan/VK pairs, applies the
observed 11-entry navigation rewrite table, posts private message `0x8D2`
with `wParam=rewritten VK` and `lParam=flags & 0x81`, and returns `1`;
disabled or negative-`nCode` paths call `CallNextHookEx`. The Rust seam exposes
this as a pure classifier and typed payload; it does not install a hook or post
a message during normal startup. `keydll3.dll` posts mouse messages `0x202`/`0x205`
as private message `0x4AC` before calling the next hook. Neither callback
contains a device selector, and the installed files do not prove that the
resident process loads or enables either helper in every state. This is
user-mode behavior evidence, not evidence for a kernel filter or driver package.

The product path therefore has these rules:

1. Open only the exact `MI_02&COL02` configuration collection for the reviewed
   ordered feature-report sequence. Do not add a guessed standalone report.
2. Keep `MI_00`/`MI_01` input on the Windows standard HID path. A software-only
   action may use a user-mode capture boundary, but one physical edge must
   produce one foreground action and must not be delivered once as native
   keyboard input and again through Rust `SendInput`.
3. Keep the all-keyboard relay and the device-specific suppression hook as
   diagnostics. They remain opt-in and do not count toward product completion.
4. Treat native device mappings and software-only mappings separately. Native
   mappings require a verified device Apply sequence; Core Audio microphone
   mute, macros, and other actions with no verified device encoding remain
   software actions with an explicit exactly-once boundary.

The remaining implementation gate follows this official-compatible boundary:
obtain the official button-Apply evidence, implement only its verified wire
fields, then prove a single physical edge, reconnect recovery, and cleanup with
the Rust binary and no vendor process. The one-action rule is the product
policy for software-only assignments; the static vendor inspection does not
establish the vendor's event cardinality for every function. A kernel filter,
driver package, or test signature is outside this acceptance scope. The command
output, installed-file hashes, and static import/export observations are retained in
[`../captures/official-user-mode-stack-20260912.md`](../captures/official-user-mode-stack-20260912.md).
The selected import/export inventory is also retained in
[`../captures/official-binary-imports-20260912.json`](../captures/official-binary-imports-20260912.json)
(SHA-256 `B7311A3A38B7497EBB3CBEFBE075AF8392D4E2C571D11F703DD7B99290C8DE49`).
The implementation scope and acceptance boundary are summarized in
[`../captures/official-compatibility-acceptance-20260912.md`](../captures/official-compatibility-acceptance-20260912.md).
The same decision and artifact hashes are bundled in the machine-readable
[`../captures/official-compatibility-manifest-20260912.json`](../captures/official-compatibility-manifest-20260912.json)
(SHA-256 `532CF298F66A516D3B7BD8F47C428EA917EF16B0A8BF75D631C146DDCB7445B2`).

The same release also passed the read-only UI Automation smoke on 2026-09-12:
[`ui-smoke-official-compatible-20260912-022943/result.json`](../captures/ui-smoke-official-compatible-20260912-022943/result.json)
records 36/34/74/19/36 nodes for General/DPI/Light/Info/restored General,
the current tray SHA-256, and exit code 0. The smoke did not invoke Device,
Apply, profile actions, dialogs, file writes, reset, or reboot.

## Native keyboard identity pass-through seam — 2026-09-12

The resident worker now treats one narrow case as firmware-native: when the
exact `MI_01` transition resolves to a plain keyboard action with the same HID
usage and no modifiers, Rust records the button's held state and does not call
`SendInput`. Windows' standard keyboard-class path is then the single
foreground delivery. The release consults that held ledger, so a profile change
while the button is held cannot turn the release into a synthetic key-up.

This seam covers the observed factory single-key assignments (for example
SIDE 7 `usage=0x1E` → `VK=0x31`) and is intentionally narrower than a general
remap. The final factory side key is a documented device-specific alias:
the profile stores `0x34`, while the attached MI_01 collection reports
`0x33`; only button 18 accepts that alias. Combos, different keyboard usages,
mouse/media actions, macros, and unverified `ButtonFunc` encodings remain
software or fail-closed paths until a complete official Apply mapping is
available. The source tests prove the decision, all twelve observed factory
usages, the alias, and their release pairing. The bounded live gate for the
factory single-key path is now PASS; software-only action exactly-once remains
a separate gate.

The rebuilt v1.0.1 release used for the next live gate is
`target/release/redsamurai-config.exe` (SHA-256
`88BAEA05054A82E72A81A12CD766C5BDD4A6DF24038A1D39822A8C2814B37A98`); the
matching probe is
`target/release/examples/live_probe.exe` (SHA-256
`9839AAF6A1924EC0746A9F92F3125D39DBFC60D379E1D5640DDC892FD636E3CA`).

The bounded operator harness for this gate is
`../captures/native-keyboard-pass-through-audit.ps1 -ConfirmLive`. It starts
the low-level observer before the Rust tray, captures the resident transition
and USBPcap trace, then requires one standard keyboard-class `VK=0x31` down/up,
one `native_keyboard_passthrough` press/release pair, zero injected target
edges, foreground clipboard exactly `1`, and zero vendor/Rust processes after
cleanup. A result that does not satisfy all of these fields is retained as
`captured` or `failed`; it must not be promoted by inference.

The attached-device run is
[`../captures/native-keyboard-pass-through-20260912-113532/result.json`](../captures/native-keyboard-pass-through-20260912-113532/result.json)
(SHA-256 `E95DF25D99CF29FE03B999D4A9B624F5867CC2FC1C8969C79DA9A8BD4658AB6F`).
It is `status=pass` with exactly one raw/native press and release, one hardware
down/up pair, zero injected target edges, and `foreground_clipboard_exact=true`
for clipboard `1`. The USBPcap SHA-256 is
`9CC23081CA7665DE014D0B789329C333A4D2C53BFF9BA143C244D3B94352D9E4`; the event
and recovery log SHA-256 values are
`92F489AC022FEF9B0801459A1C77698148D95DC2125C34B6ECB216C30110E14C` and
`AD78F8D0F83A42B469B66C76170C3471E7DC8E8569B75E728DA53C6672F7012A`.
Both official and Rust tray process counts after cleanup are zero. This closes
the narrow factory SIDE 7 native pass-through gate and does not establish
exactly-once for software-only mappings.

## Status vocabulary and safety boundary

- **STATIC PASS** — deterministic Rust/unit/integration proof only. It does not
  prove a Windows HID handle, physical button, `SendInput`, tray, registry, or
  installer run.
- **EVIDENCE PASS** — a checked-in physical capture or report establishes the
  stated observation. It must be cited with its exact artifact and must not be
  broadened beyond the stated operation.
- **LIVE PENDING** — requires a bounded Windows/device/operator run.
- **DESIGN APPROVED / PHYSICAL PENDING** — the architecture and opt-in runtime
  are implemented, but physical evidence is not release acceptance.
- **BLOCKED/STOP** — stop before hardware I/O when a prerequisite or expected
  safety gate is absent.

The non-negotiable rules for every physical run are:

1. Back up the whole product data directory and record SHA-256 hashes before
   touching it: `Documents\RED SAMURAI 16400DPI Gaming Mouse` (profiles and
   macro files). Use a disposable profile/slot for destructive tests.
2. The configuration collection is only VID `04D9`, PID `FC55`, Usage Page
   `0xFFA0`, `MI_02&COL02`. The resident input collection is only `MI_01`
   (the HID path carries a `KBD` suffix), interface 1, Usage Page `0x0001`,
   Usage `0x0006`, with the observed 9-byte
   input shape. Never select a device by VID/PID alone.
3. Do not send a standalone observed report, a hand-built report, an F1/F5
   frame, a status query, or a guessed field mapping. The only production write
   authorization is the complete ordered 125 Hz sequence described below.
4. Capture from before enumeration or the first action through the final
   completion/readback. Record the target USB address, PnP instance, report
   counts and lengths, IRP completion statuses, profile hashes, screenshots,
   executable/tool hashes, and the capture hash.
5. Change one setting per bounded run and click Apply exactly once. Do not
   infer persistence from a UI label or from a correlated byte delta.
6. If Windows reports `IsRebootRequired=True`, or `pnputil` returns the pending
   reboot condition, stop. Do not force a second restart or a normal Windows
   reboot as part of an unattended run.
7. For a Rust-only runtime run, verify that the official `hid.exe`/vendor UI is
   absent; the product must not depend on it. Close the vendor UI before any
   configuration-transport capture, unless the test is explicitly measuring
   read-only contention. Do not run the editor Apply path and the resident
   service against the same configuration collection at the same time without
   recording the result.

## Repository baseline

The supported build/test entry point is an MSVC shell on Windows:

```powershell
Set-Location C:\Workspace\OpenRedSamurai\redsamurai-config
.\msvc_cargo.bat build
.\msvc_cargo.bat test
```

The following counts are the earlier audit snapshot retained for provenance;
the authoritative post-tray-fix count is in the continuation section below.
That audit run completed successfully: 88 library tests, 4
macro round-trip tests, 7 ordered-Apply-gate tests, 10 profile tests, 25
resident-core tests, 5 resident-platform tests, and 21 USBPcap evidence tests
(160 tests total; zero failures). The compiler emitted dead-code warnings but
no test failures. This is a **STATIC PASS** and an offline fixture pass only.

The tests prove the following, without proving live hardware behavior:

| Area | Static proof | Boundary that remains live |
| --- | --- | --- |
| INI/PFD | Ordered CRLF parsing, slot/legacy sections, DPI stage records, COLORREF conversion, default/edit and real-file byte-exact round trips | A real Documents backup, dialogs, file permissions, and user-visible save/load behavior |
| Macro files | `MacroSet.MSDB` and cp932 `.MSMACRO` parse/serialize, 6-byte records, checksum fixture and unchanged-file write-back | Japanese text, recording, UI editing, file replacement/deletion, and real `SendInput` playback |
| Protocol | 16/64/1024-byte shape rules, report-ID/header checks, read-only status separation, raw-send rejection, and exact 156-frame token validation | Actual HID path/collection selection, handle contention, transfer completion, and device persistence |
| Ordered Apply | Exactly 156 reports; 155 ordinary 16-byte reports plus one 64-byte `03/F3/20`; index 13 `02/F3/32` wire byte `08`; exact order/start/end | A real Rust Apply run and a post-reconnect/readback observation |
| Profile planner | Unsupported fields become warnings; ordinary/raw/mixed apply paths fail before transport; only profile `PollingRate=8` can attach the sequence | UI warning visibility, save-before-Apply ordering, and actual no-I/O behavior |
| Resident parser/core | Exact `MI_01` metadata filter, 9-byte parser, rollover/duplicate rejection, edge tracking, 5 ms debounce, resolver, scheduler, reset release | Real input collection, physical button edges, focus/UIPI, Windows `SendInput`, and device disconnects |
| UI accessibility | Custom controls register AccessKit roles/states, names, and stable `red-samurai.*` `AuthorId` values; helper/unit tests and the release UIA smoke pass | Repeat the tree check for each packaged build or alternate Windows accessibility stack |
| Phase 4 | Typed current-user plan, absolute/traversal/quote validation, data-retention and marker rules; plan `apply()` is dry-run only | PowerShell filesystem/registry writes, tray startup, logon, and uninstall behavior |
| USBPcap evidence | Offline parsing, attribution, exact lengths, zero-status pairs, known capture deltas, reconnect/readback fixtures | A new capture is required to make a new claim about the current binary or hardware state |

The evidence tests intentionally skip some real-file checks when a fixture is
absent. A passing test must therefore record whether the real Documents/macro
fixture was present; a skip is not a compatibility claim.

## Audit execution record

The earlier static gate was rerun on 2026-09-10 in the checkout above. The exact
historical results were:

- `cargo fmt --all -- --check` — **PASS**.
- PowerShell parser checks for every `installer/*.ps1` file — **PASS**.
- `installer/test-installer.ps1` — **PASS**; its install and uninstall calls
  used `-WhatIf`, and its temporary fixture was removed afterward.
- `msvc_cargo.bat check` — **PASS**.
- `msvc_cargo.bat test` — **PASS**, 88 library + 4 macro + 7 ordered-Apply +
  10 profile + 25 resident-core + 5 resident-platform + 21 USBPcap tests
  (160 passed, 0 failed; doctests 0).
- `cargo check --target x86_64-pc-windows-msvc --all-targets` — **PASS**;
  dead-code warnings remain but no errors were emitted.
- `msvc_cargo.bat build --release` — **PASS**. The resulting
  `target/release/redsamurai-config.exe` SHA-256 was
  `1A2454D02B7DECA3281235C479976AB57216D637BADE5CCE784B4CAC5647E6EE`.
- `verify-ui-accessibility.ps1` — **PASS**. The release executable was
  launched in a disposable editor process; the script invoked only the four
  tab controls and never invoked Device, Apply, profile, dialog, or file
  actions. UI Automation exposed 36 initial nodes, 34 DPI nodes, 74 Light
  nodes, 19 Info nodes, and 36 nodes after returning to General. The checked
  IDs included `red-samurai.titlebar.close`, `red-samurai.assignment.row.1`,
  `red-samurai.slider_MouseSensitivity`, `red-samurai.dpi_vslider_0`,
  `red-samurai.light.custom-color`, and `red-samurai.bottom_6`. The reported
  executable hash matched the release hash above.

These commands did not open a physical HID device, send a feature report,
invoke `SendInput`, alter the real registry, or touch the real Documents
directory. They establish the static and disposable-fixture gates only; the
live items below remain pending until an operator records their required
evidence.

### Continuation attempt: bounded live gates — 2026-09-10

The current managed execution environment was checked before attempting any
live operation. PowerShell PnP enumeration was empty, but the Rust `hidapi`
probe found one exact configuration collection:
`\\?\HID#VID_04D9&PID_FC55&MI_02&Col02#9&2ba8c91d&0&0001#{4d1e55b2-f16f-11cf-88cb-001111000030}`
(Usage Page `FFA0`, interface 2). The resident `MI_01` collection was absent,
in the first filtered probe because the resident usage constants were reversed;
that run opened no resident handle or input report. A raw
`hidapi` diagnostic then showed the live resident path
`\\?\HID#VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}\\KBD`
with Usage Page `0001`, Usage `0006`, interface 1, and the same 9-byte shape.
The resident filter constants were corrected to those descriptor values. A
read-only `pnputil /enum-devices /connected /class HIDClass` check also showed
the target MI_01 and MI_02 stacks `Started`. The Orca runtime became
unreachable (`runtime_unavailable`; `orca open --json` timed out), after four
tasks and four Dispatch records had been created; no worker was allowed to
perform a live action. A direct disposable HKCU probe was rejected by the host
execution policy before PowerShell started, so no registry or Documents write
occurred. The new `installer/live-integration.ps1` script provides the
reviewed, GUID-scoped V-16 run for an operator on a permitted Windows host; it
requires `-LiveConfirmation RED-SAMURAI-LIVE`, refuses an existing product Run
value, verifies the real registration and data retention, and cleans up its own
fixture.

For the requested four-worker continuation, Orca Run `run_19c17c73c341` was
created with tasks `task_5662540896ed`, `task_b6fa1a4cf6b6`,
`task_121b0a20441a`, and `task_31e65fd9ca57`. Four worker Dispatch attempts
were recorded, but Codex startup was blocked and the Orca runtime then became
unavailable; no generic-agent substitute or live worker action was used.

After the user authorized a temporary stop, the official `hid.exe` process
(PID 34588, confirmed by process name and start time) was stopped. With that
contention removed, `examples/live_probe.rs --confirm-live --apply-125hz
--confirm-hid-apply` opened the exact configuration collection and sent
`verified_125hz_apply_sent=156` (155 x 16-byte reports and one x 64-byte
report). The live probe executable SHA-256 was
`BE6233AD9F9821B3692A4C0C97FFAE6C43F46F06704688889E85A1E262DC85ED`.
No USBPcap capture or post-reconnect readback was running, so this is an
execution record only; V-05 remains pending for transport capture and V-06
remains pending for persistence/readback. The separate harmless
`--sendinput-zero-move` call reached Windows `SendInput` and was rejected with
Win32 error 5 (`Access denied`) in the managed session. `hid.exe` remained
stopped during the live run and was then restored from
`C:\Program Files (x86)\RED SAMURAI 16400DPI Gaming Mouse\hid.exe` after
matching SHA-256 `8DAFABEFE196AF24DE42F628C0B87438418AF0951BFA3658E971C027F518F109`;
the first restored process was PID 10292 and responding. After the resident
read boundary below, the same verified executable was restored again as PID
62916 and remained responding. The release UIA smoke was
also rerun after the source changes and passed with release SHA-256
`1A2454D02B7DECA3281235C479976AB57216D637BADE5CCE784B4CAC5647E6EE`.
These bounded results add implementation and evidence of the stop conditions,
but do not change V-01–V-17 from **LIVE PENDING**.

After correcting the resident metadata filter, the same opt-in probe found
`resident_candidates=1` with the path and descriptor values above. Opening the
exact resident collection succeeded far enough to issue the bounded 250 ms
read, but Windows returned `ERROR_ACCESS_DENIED` (os error 5) from the read;
the probe stopped without synthesizing an input event. This is recorded as a
contention/OS boundary failure, not as proof that the resident report shape is
wrong. The follow-up discovery/SendInput probe executable SHA-256 was
`732ACC1C6A45CA7A5946EDC21902D352F61473E3F81E53C18105375AE82DD644`; it
reported the same one exact configuration and one exact resident candidate
before the zero-distance `SendInput` call returned Win32 error 5. The final
read-only state check found the product Run value and product Documents
directory absent. The disposable registry/data harness was then run with
`run_id=8d701d63f6cd4a19860e908910e8814d` and source SHA-256
`1A2454D02B7DECA3281235C479976AB57216D637BADE5CCE784B4CAC5647E6EE`; it
created its GUID temp root, then stopped when the managed host denied creating
the Documents data directory. Its `finally` cleanup removed the temp root and
no Run value or data directory remained. V-16 is still pending on a host that
permits the intended per-user Documents write.

The explicit one-pixel mouse variant was also invoked once to distinguish a
zero-distance no-op from a general input restriction. Its probe SHA-256 was
`A743FDD8BBF388B3BBCBE61F6C989D114E4EAB429DC615FFA8C82208CA75C634`; Windows
returned the same Win32 error 5, so no cursor movement or key/button event was
accepted.

The managed shell identity was then checked directly: `whoami` returned
`Kota-PC\\CodexSandboxOffline`, while the active console session was owned by
`Kota-PC\\k0ta0`. The `Documents` ACL grants the sandbox group read/execute
access only and grants `Kota-PC\\k0ta0` full control. This explains why the
live harness could resolve the desktop user's Documents path but could not
create its disposable product directory. It also means that this shell's
`HKCU` is the sandbox user's hive. The tray entry point was started once with
the official process stopped and was then terminated; the official process
was relaunched and is responding. These checks do not promote the SendInput,
resident-read, registry, or Documents gates to live pass. They identify the
execution-context prerequisite: repeat them from the logged-in desktop user
at a matching integrity level.

After the harness was instrumented with that context, a repeat run
(`run_id=dcb3c48e855a41849d6c18fc6969366d`) reported the identity and resolved
Documents root before failing at the first data-directory creation. Its
`finally` cleanup removed the GUID temp install root; no Run value or product
data directory remained.

### Continuation attempt: permitted interactive context — 2026-09-10

The managed shell was subsequently relaunched as `KOTA-PC\\k0ta0` at high
integrity in the active console session. The rebuilt live probe (SHA-256
`D22CC9B291F411FCA4279A88529394E332187EACD989E5ADD319E3455B913071`) reported
the effective token identity through `whoami`, and the one-pixel mouse event
was accepted by Windows (`sendinput_one_pixel=accepted`, exit code 0). This is
the first successful real `SendInput` call in the project run.

With the official `hid.exe` stopped (PID 58480) and restored afterward as PID
42128, the same probe opened the exact configuration collection and sent
`verified_125hz_apply_sent=156`. It then found the exact `MI_01` resident
collection but its bounded read still returned `ERROR_ACCESS_DENIED`; no
synthetic resident event was generated. This separates the resolved
SendInput/shell boundary from the remaining resident HID read boundary.

The real per-user installer harness was run with an explicit local Documents
root because the default known folder resolves to the OneDrive reparse-point
`C:\\Users\\k0ta0\\OneDrive\\ドキュメント`. Run
`20206a389c1c48ef97c59a518f91598a` passed as `KOTA-PC\\k0ta0`: it created the
GUID install directory, registered and verified the real HKCU Run command,
created a disposable file under `C:\\Users\\k0ta0\\Documents`, verified its
SHA-256 before and after marker-gated uninstall, and removed the fixture. The
post-run check found no product Run value and no `*-live-*` data directory.

The same harness was repeated after the final release rebuild with
`redsamurai-config.exe` SHA-256
`2BA084E5F973A5841BE4DD262CE930825FEBB79B6F976F39552D019D286B7F1B`.
Run `f29d0a1e483f4d6293e9a213b47f55b7` passed as `KOTA-PC\\k0ta0` against the
explicit local `C:\\Users\\k0ta0\\Documents` root: the real HKCU Run value was
created and removed, the disposable data hash stayed
`7E481FC4EA44A0AF430A9D1453EAB374C97DDFF1C99AC2DB2E6620EFA508BBA3`, and no
fixture remained afterward.

The same current main executable passed `verify-ui-accessibility.ps1` with PID
26068 and SHA-256
`2BA084E5F973A5841BE4DD262CE930825FEBB79B6F976F39552D019D286B7F1B`:
36 initial nodes, 34 DPI nodes, 74 Light nodes, 19 Info nodes, and 36 nodes
after returning to General. The script invoked tabs only and did not touch
device, Apply, profile, dialog, or file actions.

The current release was also started with `--tray` as PID 61560 for 1.8 s;
the process was responsive, then terminated cleanly by the operator. No
`redsamurai-config` process remained afterward. This is a lifecycle smoke only;
it does not close the full tray menu, logon, or disconnect-recovery evidence.

### Continuation attempt: Raw Input resident path and current binary — 2026-09-10

The resident reader was changed after the direct-handle `ERROR_ACCESS_DENIED`
observation. Windows keyboard-class collections do not permit the synchronous
HID read used by the old path, so the current implementation keeps `hidapi`
for exact metadata discovery only and receives `WM_INPUT` on a message-only
window. Each event is accepted only when `RIDI_DEVICENAME` matches the selected
`VID_04D9/PID_FC55/MI_01` instance; the backend-specific trailing `\\KBD`
spelling is normalized without weakening the instance check. Discovery now
also rejects a missing `MI_01`, missing `KBD`, wrong interface number, or wrong
9-byte shape.

The current release probe SHA-256 is
`6348DC7AA1A448AD78CC059558BF8764B61443CE23503883F846CE254D44DF85`. From the
interactive high-integrity `KOTA-PC\\k0ta0` console:

- `--confirm-live --read-resident` enumerated one exact configuration and one
  exact resident candidate, initialized Raw Input, and returned
  `resident_report=timeout` with exit code 0. No direct resident HID read or
  `os error 5` occurred.
- `--confirm-live --sendinput-one-pixel` returned
  `sendinput_one_pixel=accepted` with exit code 0.
- With the official `hid.exe` stopped temporarily and restored afterward, the
  same binary sent `verified_125hz_apply_sent=156`, then completed the Raw Input
  resident probe with `resident_report=timeout`. The official process was
  restored and responding (PID changed to 12904 after restart).

This proves the current initialization, path filter, timeout, physical Apply,
and one-pixel `SendInput` boundaries. A physical button edge was not generated
in this bounded run, so V-09/V-11 physical edge and disconnect evidence remain
pending. The direct keyboard-class read failure is retained as historical
diagnostic evidence; it is no longer the production backend.

Before the tray wake fix, the static suite was 165 tests (89 library, 4 macro, 7 ordered Apply,
10 PFD, 26 resident-core, 8 resident-platform, and 21 USBPcap), with
`cargo fmt --all -- --check`, `msvc_cargo.bat check --all-targets`, and
`msvc_cargo.bat test --all-targets` passing. Existing warnings are dead-code
warnings in hardware-free test inclusions only.

### Continuation after tray wake fix — 2026-09-10

The current checkout now includes the bounded tray reliability fix in
`src/tray.rs`: the resident worker publishes its completion status before
posting a `WorkerFinished` user event, so the winit loop processes a stopped
worker without waiting for an unrelated event. The hardware-free tray tests
cover notification ordering and success/failure state transitions.

The authoritative current static run is 168 tests: 92 library, 4 macro, 7
ordered Apply, 10 PFD, 26 resident-core, 8 resident-platform, and 21 USBPcap;
`msvc_cargo.bat test --all-targets`, `msvc_cargo.bat check --all-targets`, and
`cargo fmt --all -- --check` all pass (dead-code warnings only). The current
release rebuild hashes are:

- `target/release/redsamurai-config.exe` —
  `7F7367CC2AB285775F8CBDD70BF0C8B189FB904C0CEB85A438E2E6D536CDBAB1`.
- `target/release/examples/live_probe.exe` —
  `09770F52B234B0FF8C8709FB38B8D91584A8071CCCE9E214D11D93839B735B01`.

That main executable passed the current UI Automation smoke (PID 41796; 36
initial, 34 DPI, 74 Light, 19 Info, and 36 restored-General nodes). A current
`--tray` lifecycle smoke (PID 22404) was responsive for 1.8 seconds and left
no `redsamurai-config` process after cleanup.

The current live probe, run as `KOTA-PC\\k0ta0` at high integrity, found exactly
one configuration and one resident candidate. `--read-resident` returned the
clean `resident_report=timeout`, `--sendinput-one-pixel` returned
`sendinput_one_pixel=accepted`, and the bounded run with the official `hid.exe`
temporarily stopped sent `verified_125hz_apply_sent=156` before the same Raw
Input timeout. The vendor process was restored and responding as PID 39436;
no physical resident button edge was generated, so V-09/V-11 disconnect and
held/debounce evidence remain pending.

The current-user installer harness run `16dc7c1ab218476b9a935031a2ebe96f`
used the explicit local `C:\\Users\\k0ta0\\Documents` root with the current
release hash. It created and removed the real HKCU Run value, preserved the
disposable data hash
`7E481FC4EA44A0AF430A9D1453EAB374C97DDFF1C99AC2DB2E6620EFA508BBA3`, and
left no live fixture or Run value. Full tray menu, logon, disconnect recovery,
Rust transport/readback persistence, and current-package V-01–V-17 acceptance
remain live gates.

### Bounded Raw Input monitor run — 2026-09-10

The current release was rebuilt with `msvc_cargo.bat build --release` and the
live probe example was rebuilt with `msvc_cargo.bat build --release --example
live_probe`. The monitor was run from a high-integrity console as
`KOTA-PC\\k0ta0` (Windows `whoami` reported the case-normalized
`kota-pc\\k0ta0`) with:

```powershell
.\target\release\examples\live_probe.exe --confirm-live --read-resident --read-resident-for-ms 30000
```

The example SHA-256 was
`9CBD737D2AE037B66F164E31FEFDD05D3A5BAA4098ACA8F331A5AAB333FFB157`, and the
main release SHA-256 was
`39B5776F4E2BBADBCB3DEF78ED60A1500D19EEB4B2D05D75F3CF557E0F98EF0F`. At
`2026-09-10T17:09:59.4936306+09:00`, discovery found exactly one
configuration collection and one resident collection:

```text
resident path="\\\\?\\HID#VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}\\KBD"
usage_page=0001 usage=0006 interface=1 report_length=9
resident_monitor=starting duration_ms=30000 max_duration_ms=30000
resident_monitor=timeout duration_ms=30000 transitions=0
```

The process exited 0 at `2026-09-10T17:10:29.7934677+09:00`. No physical
press or release edge arrived during the finite window, so no
`resident_transition=press|release` record can honestly be reported. The
monitor used the Raw Input transition path, included no Apply or `SendInput`
flag, sent no configuration HID report, and left no `redsamurai-config`
process; registry and user data were not touched. This is clean timeout
evidence only; physical-edge and disconnect evidence remain pending.

### Current continuation after bounded monitor and safety hardening — 2026-09-10

The authoritative current static run is 179 tests: 98 library tests, 5
bounded-monitor tests, 4 macro round-trip tests, 7 ordered Apply tests, 10 PFD
tests, 26 resident-core tests, 8 resident-platform tests, and 21 USBPcap tests.
`msvc_cargo.bat test --all-targets`, `msvc_cargo.bat check --all-targets`, and
`cargo fmt --all -- --check` all pass; only the existing dead-code warnings in
hardware-free test inclusions remain.

The current release hashes are `39B5776F4E2BBADBCB3DEF78ED60A1500D19EEB4B2D05D75F3CF557E0F98EF0F`
for `target/release/redsamurai-config.exe` and
`9CBD737D2AE037B66F164E31FEFDD05D3A5BAA4098ACA8F331A5AAB333FFB157` for
`target/release/examples/live_probe.exe`. The UI Automation smoke passed with
36/34/74/19/36 nodes. The tray smoke stayed responsive for 1.8 seconds, the
second simultaneous `--tray` launch exited 0 while the first stayed responsive,
and both processes were cleaned up.

As `KOTA-PC\\k0ta0`, the current live probe found exactly one configuration and
one resident collection. `--read-resident` returned `resident_report=timeout`,
`--sendinput-one-pixel` returned `sendinput_one_pixel=accepted`, and the
authorized run with the official `hid.exe` temporarily stopped sent
`verified_125hz_apply_sent=156` before the Raw Input timeout. The official
process was restored at its exact path and was responding as PID 61832. The
30-second monitor recorded `resident_monitor=timeout` with `transitions=0`; no
physical edge or disconnect evidence was observed.

The current-user installer harness run `5543e6f7c5da4407b5fcf4a2965cc130`
used `C:\\Users\\k0ta0\\Documents`, created and removed the real HKCU Run
value, preserved the disposable data hash
`7E481FC4EA44A0AF430A9D1453EAB374C97DDFF1C99AC2DB2E6620EFA508BBA3`, and
left no product Run value, temporary install directory, or `*-live-*` data
directory. The installer parser/WhatIf harness also passed. Full tray menu,
logon, disconnect recovery, transport/readback persistence, and physical
button-edge evidence remain live gates.

### Current implementation and live evidence after the four-worker wave — 2026-09-10

The authoritative MSVC all-target run now passes 196 tests: 106 library tests,
6 live-probe tests, 4 macro round-trip tests, 7 ordered Apply tests, 10 PFD
tests, 30 resident-core tests, 12 resident-platform tests, and 21 USBPcap
tests. `cmd /c msvc_cargo.bat fmt --all -- --check`, `check --all-targets`,
and `test --all-targets` all pass; only the existing dead-code warnings in
hardware-free test inclusions remain.

The final release artifacts used for the checks below are:

- `target/release/redsamurai-config.exe` —
  `8B2009A59B21E1C00E2F2EB1512EEB29CB30D02E8F4A8BD1271D004828D56793`.
- `target/release/examples/live_probe.exe` —
  `B50363C8BE39546EF659F0882D0FD9E6F8FCDEFFE1E35D22BB78DB2342837230`.

The resident platform now has bounded startup readiness, bounded shutdown/join,
panic and worker-error reporting, and exact-device removal notifications while
keeping `hidapi` for metadata discovery only. The exact
`VID_04D9/PID_FC55/MI_01`, usage `0001:0006`, interface 1, 9-byte filter is
unchanged. The final UI Automation smoke passed with 36/34/74/19/36 nodes;
the tray smoke stayed responsive and a second simultaneous `--tray` launch
exited 0 while the first remained responsive.

As `KOTA-PC\\k0ta0`, the final one-shot Raw Input probe found one configuration
and one resident collection and returned `resident_report=timeout`; the
one-pixel Windows `SendInput` probe returned `sendinput_one_pixel=accepted`.
The bounded monitor found the exact resident path and ended with
`resident_monitor=timeout transitions=0` (the 30-second worker run and a
1-second final smoke); no physical button edge or disconnect was observed.

With the official vendor process stopped only for the authorized operation, the
final release Apply sent 156 reports and reported:

```text
verified_125hz_apply_sent=156
verified_125hz_apply_transfer=complete expected=156 completed=156 expected_16=155 expected_64=1 expected_other=0 persistence=not-observed
```

The exact vendor executable was restored at
`C:\\Program Files (x86)\\RED SAMURAI 16400DPI Gaming Mouse\\hid.exe` as
responding PID 64436. This is current-binary transfer evidence; device-side
persistence/readback is deliberately still `not-observed`.

The separate current-Rust USBPcap artifact from the transport worker is
`captures/rust-current-apply-20260910-worker/evidence-report.md` and
`rust-live-apply-usbpcap2.pcap` (SHA-256
`4E60F695C6E1BEA7C74BA0D1579F38254BE17EA90F6EB05DCDB7889C90C2EB93`). It
records 156 target `SET_REPORT`s (155 x 16 bytes and 1 x 64 bytes), 156
same-IRP zero-status completions, and zero target `GET_REPORT`s. Its recorded
probe hash is `6C9094ADAD7AC21AFF52B7D2072822E55E267832F8D3EBC33CB6FB15F5BDA147`;
the later final release hash above is cited separately, so this capture is not
claimed as a readback proof for that later executable.

The current-user installer/startup run `ebde99d70489406eb0376e2af006d031`
used the explicit local `C:\\Users\\k0ta0\\Documents` root and final release
hash, created the real HKCU Run value, launched the tray, observed the second
instance exit code 0, preserved data hash
`7E481FC4EA44A0AF430A9D1453EAB374C97DDFF1C99AC2DB2E6620EFA508BBA3`, and
removed the Run value, install fixture, data fixture, and started processes.
Its bounded startup record is explicitly `logon_performed=false` and
`disconnect_recovery_performed=false`; no sign-out/logon or physical
disconnect/recovery was performed.

The remaining live gates are physical press/release and disconnect/recovery,
controlled sign-out/logon, Rust device persistence/readback, and complete
V-01–V-17 acceptance. V-18 remains unauthorized.

### Bounded physical interface restart observation — 2026-09-10

The exact resident collection instance
`HID\VID_04D9&PID_FC55&MI_01\9&13E53CD8&0&0000` was restarted once with
`pnputil /restart-device` while the official vendor process was stopped for
the authorized boundary test. `pnputil` exited 0 and reported “Device restarted
successfully”; a follow-up PnP query returned `Status=OK` and
`Problem=CM_PROB_NONE`. The bounded Rust monitor still ended with
`resident_monitor=timeout duration_ms=30000 transitions=0`, and the post-restart
one-shot probe also returned `resident_report=timeout`. The official executable
was then restored at its exact path and was responding as PID 14044.

This is evidence that the device-interface restart completed and the device
returned to a healthy PnP state. It is not evidence of a Raw Input removal
callback, an automatic worker recovery, a physical button edge, or device
persistence/readback; those live gates remain open.

## Existing physical evidence ledger

These artifacts are useful baseline evidence, but each has a deliberately narrow
meaning.

### E-125: official 125 Hz Apply and readback — PASS (existing evidence)

`captures/hardware-evidence-20260909T071720685Z-5d9fdff2/evidence-report.md`
and its two classic pcap files document an operator using the official vendor
`ldcfg.exe` after one physical reconnect. The run used no Rust binary and no
hand-built HID frame.

- The official Apply burst is 156 target `SET_REPORT`s: 155 x 16 bytes and one
  x 64 bytes. Every report has a zero-status completion; the Apply window has
  no target `GET_REPORT`.
- The PollingRate report is
  `02 f3 32 00 06 00 00 00 08 00 08 00 02 00 00 00`, where the observed wire
  byte is `08` for the 125 Hz PFD value `8`.
- After `pnputil /restart-device`, the restart/readback capture has 195 target
  SETs and 193 target GET/readback pairs, all zero-status. The only PollingRate
  readback is `02 08 32 60 05 00 fa fa 08 00 08 00 02` (wire `08`). The
  post-Apply profile hash is
  `3acbf08cf550cbd1ede29f177a9e383965b66aa22bb82bca7215634db4fe14b5`.
- `pnputil` returned 3010 and the device stayed healthy, but Windows changed
  `IsRebootRequired` to true. No normal reboot was performed.

This proves the physical vendor sequence and a post-restart readback. It does
not prove that the Rust executable opened the same endpoint or that any
unsupported field mapping is safe.

### E-250: reconnect readback — PASS (existing evidence)

The same hardware report records 193 GET/readback reports before and after the
official 125 Hz Apply. The two resident readback columns match and show the
250 Hz marker `02 08 32 60 05 00 fa fa 04 00 08 00 02` (wire `04`). The
readback shape is 108 x 16-byte and 85 x 64-byte GET routes. This is a baseline
for a bounded A/B run; it is not authorization to send `02/F3/32` alone.

### E-UI: correlations and negative evidence — bounded only

The repository contains UI-correlated full bursts for PollingRate, Light, DPI,
and button changes. They establish observations such as `02/F3/44`, `49`,
`4F`, `5C`, `8E`, and `02/F3/32`; none is a verified standalone write. The
elevated and UAC-off UI captures show no target GET_REPORT on Apply. The
controlled-polling directory has a pre-state/UI mismatch (the UI was already at
250 Hz), so it must not be cited as a clean 125-to-250 transition. The
`rs-observation-*` run and `reconnect-pnp-*` run are negative/blocked evidence,
not persistence proof; the latter reports a pending reboot and no SET/GET
readback.

### E-REENUM: resident/vendor read-only observation — bounded only

`hid_reenum_usbpcap2_20260909_131359_ca7d7c9e.pcap` records the official
resident `hid.exe` running for 78 seconds: 64 interrupt input reports and 64
zero-length completions, six enumeration control records, and zero target SET
or GET reports. The profile hash stayed unchanged. This proves only that that
process did not show feature traffic during that interval.

## Live verification matrix

Every item below needs the fields **Prerequisites**, **Procedure**, **Expected**,
**Risk/stop condition**, and **Completion evidence** recorded in the run log.
The word “PASS” in the expected section is a completion condition, not a claim
that it has already occurred.

### V-01 — Windows build, editor launch, and startup no-I/O

**Status:** LIVE PENDING. **Owner:** Windows operator.

**Prerequisites:** Windows/MSVC, a clean process list, a screen-capture tool,
and USBPcap2 or equivalent capture started before launch. A real mouse is
preferred for endpoint observation but is not needed to check the no-device
editor startup.

**Procedure:** Build with `msvc_cargo.bat`; start the editor with no argument;
leave the process idle for at least 10 seconds; do not click Device, Apply, or a
profile action. Then click the title-bar device indicator once. Record the
displayed status and the target HID control traffic. Close the editor without
using Apply.

**Expected:** `App::new` starts as `NotChecked`; startup opens no HID device and
does not issue SET or GET reports. The indicator action performs only the
documented read-only metadata/availability check. A present device is reported
only for the exact configuration collection; an absent or wrong collection is
reported as unavailable. A screenshot must show the title bar and status.

**Risk/stop condition:** Any report traffic before an explicit action, a path
that selects another collection, or a process that stays resident after close
is a failure. Do not continue to Apply to “see if it works.”

**Completion evidence:** executable SHA-256, Windows build output, screenshot,
capture hash, target SET/GET counts for startup and indicator click, and process
exit confirmation.

### V-02 — Editor frame, tabs, controls, and persistence

**Status:** LIVE PENDING overall; the release UI Automation subcheck is PASS.

**Prerequisites:** Backup and hashes of all five `RSProfile*.pfd` files and
`MacroSet.MSDB`; a disposable profile slot or a copied product data directory;
the Japanese Windows font candidates used by `app.rs`.

**Procedure:** Open the editor and check the 800 x 638 borderless frame,
title-bar drag/minimize/close, four tabs, five profile buttons, and the seven
bottom actions. Exercise, using disposable data:

- General: front/side mouse view, mouse sensitivity, wheel/universal scroll,
  OFA, double-click speed and test area, acceleration, X/Y link, mouse speed,
  and the 125/250/500/1000 polling radio labels.
- DPI: five stage columns, enable state, displayed values, current stage, and
  stage edits.
- Light: palette, custom color, brightness, speed, mode, and breath controls.
- Info: the expected centered Japanese information lines.
- Save, Load file, default, reset all, OK, Cancel, and Apply. Verify that Save
  and Load use the expected file dialogs; Cancel reloads disk state; OK follows
  Apply-and-close semantics; and Apply saves before it attempts device I/O.
- Accessibility: run `verify-ui-accessibility.ps1` for the read-only tab smoke,
  then use Windows Accessibility Insights/Inspect (or an equivalent UI
  Automation tree viewer) for any additional controls. Record each node's
  `AutomationId` and verify that it starts with `red-samurai.` and remains
  unchanged after a repaint, tab switch, and value change. The release smoke
  already covers the title bar, assignment row, General slider, DPI slider,
  Light palette, bottom Apply, and tab transitions.

**Expected:** Custom-painted controls appear in the accessibility tree with
button/checkbox/slider semantics, readable names, and the documented stable
`red-samurai.*` `AutomationId` values. IDs remain stable across repaint and
localization-independent actions. Asset dimensions/positions and labels match
the contracts and the checked-in smoke reference; edits are visible;
unchanged PFD files remain byte-identical; changed values appear in the intended
slot/legacy sections with
CRLF preserved. No hardware report is emitted by ordinary editing or saving.

**Risk/stop condition:** Never use the production profile as the first test
file. Restore the backup if a Save/Reset action touches another slot, changes
line endings, or changes a file with no intended edit.

**Completion evidence:** before/after file hashes, screenshots for each tab and
bottom action, an exported UI Automation tree (or equivalent node dump) with
`AutomationId` values, and a list of any intentional byte changes. This check
does not make any HID field mapping verified.

### V-03 — Dry-run and fail-closed Apply behavior

**Status:** LIVE PENDING; planner and transport tests are STATIC PASS.

**Prerequisites:** A disconnected device or a capture running with a real
device; a disposable profile containing the normal default fields; a way to
force a Save failure safely (backup plus a temporary read-only/locked copy).

**Procedure:** With the ordinary default profile, open the dry-run/apply path.
Inspect every warning before clicking anything that could write. Change one
unsupported field at a time and use Apply. Separately cause the profile Save to
fail, then press Apply. Test the device-unavailable case with a clean minimal
profile. Do not bypass a warning by calling a lower-level API.

**Expected:** Warnings are visible and Apply stops before opening the transport
when warnings exist. A Save failure stops before any HID I/O. A disconnected
device retains a dry-run plan and shows an unavailable status. The read-only
`02/F2/2C` status observation, if displayed in a plan, is never sent by a
write-oriented Apply path. No target SET/GET appears in the blocked cases.

**Risk/stop condition:** A warning-free UI with an ordinary full profile is not
expected: the current planner warns for unsupported fields. Any hidden write,
or any Apply after a failed save, is a release blocker.

**Completion evidence:** screenshots/toast text, warning field list, the exact
reason for the forced save failure, and a trace with zero target reports for
each blocked case.

### V-04 — HID discovery and endpoint separation

**Status:** LIVE PENDING; metadata filters are STATIC PASS.

**Prerequisites:** The target mouse connected; vendor/HID enumeration output;
USBPcap descriptors; no second target mouse connected unless testing rejection.

**Procedure:** Enumerate all HID paths and record VID/PID, Usage Page, Usage,
interface/MI token, collection, and report lengths. Click the editor device
indicator and separately start tray mode. Observe which handles are opened and
which USB addresses generate traffic.

**Expected:** Configuration traffic is limited to VID `04D9`/PID `FC55`, Usage
Page `FFA0`, `MI_02&COL02`, with 16/64/1024-byte feature-report rules. Resident
traffic opens only `MI_01` (the `KBD` path suffix), interface 1, Usage Page
`0001`, Usage `0006`, and a
9-byte input report. A look-alike path, wrong collection, missing interface
token, or wrong report length is rejected. The resident path has no feature
report write API.

**Risk/stop condition:** VID/PID-only selection, a SET on `MI_01`, a GET/SET
from tray mode on `MI_02`, or a 1024-byte transfer attributed to this mouse
without the exact observed header is a hard failure.

**Completion evidence:** full descriptor/path inventory, selected-path record,
and a pcap filtered by the target address showing the separation.

### V-05 — Rust 125 Hz ordered Apply

**Status:** LIVE PENDING for the Rust binary; the official-vendor equivalent is
E-125 PASS (existing evidence).

**Prerequisites:** Real VID/PID device healthy and not pending reboot; all
production PFD/macro files backed up; official `hid.exe` and tray service
closed; USBPcap2 started before the editor; a disposable profile containing
only the supported gate, for example:

```ini
[GROUP0]
PollingRate=8
```

The slot section must contain `PollingRate=8`; do not turn a complete default
profile into a write-ready profile by hiding warnings. If the UI cannot load a
minimal disposable profile without adding unsupported values, stop and record
the guard as a blocker.

**Procedure:** Start the capture, launch the Rust editor, select the disposable
slot, confirm the plan is warning-free/write-ready, and click Apply exactly
once. Do not click Device repeatedly, change another field, or send any report
manually. Save the pcap, profile hash, UI result, and process log.

**Expected:** The Rust path saves the profile first, opens only the exact
configuration endpoint, and sends one complete 156-report token: 155 x 16B and
one x 64B `03/F3/20`. The first report is the observed F5/00 boundary, the
last is F5/01, report index 13 is `02/F3/32` with byte[8] `08`, and every
target SET has a zero-status completion. The Apply window has no target GET.
There is no standalone write, mixed raw frame, or invented commit frame.

**Risk/stop condition:** Stop on any warning, endpoint ambiguity, vendor-handle
contention, non-zero completion, count other than 156, wrong length, wrong
order, or device health change. Do not retry repeatedly; preserve the capture.

**Completion evidence:** the Rust executable hash, pcap hash, parsed sequence
summary, exact SET/GET counts and lengths/statuses, profile before/after hash,
and the UI success/failure result. This proves transport execution only until
the readback check below passes.

### V-06 — 125 Hz persistence and post-reconnect/readback

**Status:** LIVE PENDING for a Rust Apply; E-125/E-250 are existing official
evidence.

**Prerequisites:** A completed V-05 pcap; device `Status=OK`, `ProblemCode=0`,
and `IsRebootRequired=False`; one known baseline (the existing 250 Hz marker is
wire `04`); USBPcap2; `pnputil`; permission for a single bounded device restart.

**Procedure:** Capture the pre-state readback, perform the one Rust 125 Hz
Apply, then run `pnputil /restart-device` once and capture the restart/readback
path. Record the PnP exit code and state. Do not force another restart if
Windows returns 3010 or sets `IsRebootRequired=True`.

**Expected:** The readback includes the 125 Hz marker
`02 08 32 60 05 00 fa fa 08 00 08 00 02`, and the before/after profile hash
matches the intended disposable PFD. All paired requests have zero status. A
normal reboot is not part of this test; a pending-reboot result is recorded as
an environmental limitation, not silently retried.

**Risk/stop condition:** Do not claim persistence from the UI alone, from a
single `SET_REPORT`, or from the official E-125 capture when the Rust pcap is
missing. Stop on a mismatched marker, unmatched IRP, stale USB address, device
problem code, or unexpected file change.

**Completion evidence:** pre/post readback columns, exact GET/SET pairing,
marker, PnP output, profile hashes, and the Rust Apply pcap linked together.

### V-07 — Unsupported-field warnings and no speculative writes

**Status:** LIVE PENDING.

**Prerequisites:** A disposable profile and a capture. Use one field per run;
the default profile is useful for checking the complete warning list but not for
checking a successful Apply.

**Procedure:** Edit one field in each group, inspect the dry-run, and press
Apply once: `GroupID`, `Group`, `MouseSensitivity`, `WheelScrollLines`,
`UniversalScrollEnabled`, `OFASensitivity`, `DoubleClickSpeed`, `Acceleration`,
`XYSensitivityEnabled`, `MouseSpeedX`, `MouseSpeedY`, `LedState1`,
`FlashState1`, `LedColor1`, `ANGLE`, `LIFT`, `DPI`, `PollingRate` values other
than the authorized `8`, `DPIStageNum`, `DPIStageValue`, `DPICurrentX`,
`DPICurrentY`, `LedMode1`, and `BreathState1`. Exercise button functions,
including explicit unverified subfunctions 48, 49, 52, and 53.

**Expected:** Each field is named as a warning; no guessed report is produced;
Apply stops before open/send. For a profile with `PollingRate=4`, `2`, or `1`,
the UI remains warning-only even though those wire correlations appear in
captures. The `VERIFIED_COMMAND_MAPPINGS` registry remains empty.

**Risk/stop condition:** Never test this by manually sending the correlated
frame. A target SET in any warning case means fail the run and retain all
artifacts.

**Completion evidence:** warning screenshots/list, per-field test profile
hashes, and zero-target-I/O capture summaries.

### V-08 — Report lengths, completion handling, and contention

**Status:** LIVE PENDING; shape and rejection tests are STATIC PASS.

**Prerequisites:** A real device, the V-05 capture setup, and a second run with
the official resident/vendor handle intentionally left open.

**Procedure:** First repeat V-05 with the official process stopped. Then repeat
the blocked-open case with the vendor handle resident. Observe whether Rust
fails before I/O or obtains a safe exclusive handle. Never resolve contention
by adding retries that send duplicate sequences.

**Expected:** Ordinary reports are exactly 16 bytes; the one observed extended
report is exactly 64 bytes; the unobserved `04/F3/C8` candidate is never sent as
1024 bytes. A short response, wrong report ID, wrong collection, or partial
transport acknowledgment is rejected. With contention, the user receives a
clear failure and no partial speculative burst.

**Risk/stop condition:** A partial burst, automatic duplicate Apply, or a
write to a second HID path is a hard stop.

**Completion evidence:** both pcap summaries, handle/process list, error/toast
text, and confirmation that no source-level fallback was needed.

### V-09 — Resident MI_01 read loop and physical button edges

**Status:** LIVE PENDING; parser/filter tests are STATIC PASS.

**Prerequisites:** Windows target mouse, backed-up profiles, Rust binary, tray
mode, and a safe foreground test application such as Notepad. Close the editor
Apply path. Prepare profiles with visible but harmless mappings.

**Procedure:** Run `redsamurai-config.exe --tray`. Record the selected HID path
and run for at least 30 seconds. Press and release every physical button that
maps to usages `0x04..0x17`, including a held button and a rapid repeat. Keep a
USB capture active to distinguish the interrupt input path from feature reports.

**Expected:** Only the exact 9-byte `MI_01` input collection is read; usage
`0x04` maps to button 1 and `0x17` to button 20. Report ID is zero, the
reserved byte is zero, no rollover usage appears, and duplicate usages do not
become events. Tray mode never opens `MI_02&COL02` and never writes a feature
report. Unknown paths/reports are ignored or surfaced as a resident error.

**Risk/stop condition:** Unexpected text, clicks, or media actions in the wrong
window, feature traffic from the tray process, a stuck held action, or a
resident handle on the config collection is a stop.

**Completion evidence:** selected-path metadata, input report samples or
filtered pcap, target feature SET/GET count (zero), physical button/action
table, and clean stop result.

### V-10 — `SendInput` action matrix

**Status:** LIVE PENDING; resolver and adapter construction tests are STATIC
PASS.

**Prerequisites:** A disposable profile, Notepad/browser/media test windows,
an event recorder or visible result, and an operator who can avoid destructive
shortcuts. Do not assign dangerous actions to a real work profile.

**Procedure:** Assign and exercise one button per action class, pressing and
releasing each button once and holding it where relevant:

| Profile function | Runtime check |
| --- | --- |
| 1..5 | left/right/middle/back/forward mouse down/up |
| 6 | single HID-key usage, including a safe letter |
| 7 | HID key plus each Ctrl/Alt/Shift/Win modifier combination used by the UI |
| 11 | left double-click trigger |
| 12 | fire target (left/right/middle or a safe keyboard key), count and delay |
| 16..23 | cut, copy, paste, select all, find, new, print, save in a disposable document |
| 24..37 | window/browser/mail actions; test close/lock/run only in a disposable VM or omit with an explicit safety note |
| 38..46 | play/stop/previous/next/volume/mute/microphone/media-player behavior |
| 100 | macro playback, covered in V-12 |
| 13, 47, 52, 53 | confirm DPI intents are ignored safely and generate no feature write |
| 14 | profile switch, covered in V-13 |

**Expected:** Key modifiers go down before the key and up in reverse order;
mouse X1/X2 and media virtual keys reach the intended foreground application;
double-click and fire counts/delays are correct; invalid usages produce an
error rather than an arbitrary key. No action sends configuration HID traffic.

**Risk/stop condition:** Check target focus before every action. Stop on a
wrong-window injection, an unbalanced key/button down, a system lock/close/run
side effect outside the test boundary, or any `SendInput` acceptance without
the expected visible result.

**Completion evidence:** mapping table with observed result/timestamp, focus and
integrity level of the target window, any `SendInput` error, and pcap showing no
config write.

### V-11 — Debounce, held state, reset, and disconnect behavior

**Status:** LIVE PENDING; resident-core state tests are STATIC PASS.

**Prerequisites:** V-09 tray run, harmless key/mouse mapping, a way to produce
quick repeats, and a second profile with a distinct harmless output.

**Procedure:** Test a normal press/release, a held press, repeated edges inside
the 5 ms debounce window where the hardware permits, opposite transitions,
profile change while held, tray Exit while held, and physical USB disconnect.
Reconnect only after recording the failure state, then restart tray mode if
needed.

**Expected:** Only stable edges reach the runtime; a bounce/opposite edge inside
the debounce window is suppressed. Reset releases held keyboard/mouse actions
and clears pending macro/debounce state. Service failure leaves the tray
available and changes its tooltip to the stopped state; it does not continue
injecting stale actions. Reconnect does not cause configuration writes.

**Risk/stop condition:** Any stuck key/button, repeated fire after disconnect,
or silent service restart that sends an unexpected action is a failure.

**Completion evidence:** timed event log/video, process/tooltip states, pcap
before/after disconnect, and explicit held-action release confirmation.

### V-12 — Macro manager, recording, persistence, and playback

**Status:** LIVE PENDING; macro parser/fixture tests are STATIC PASS.

**Prerequisites:** Backup `MacroSet.MSDB` and all `.MSMACRO` files; a disposable
macro name/slot; cp932-capable editor or byte dump; Notepad as the foreground
target. Deletion tests must use a disposable copy.

**Procedure:** Open the macro manager from the button-function menu. Create a
macro, record safe key presses, stop, select/edit/reorder/copy/cut/paste rows,
change default delay/loop/delay options, save, close, and reload. Test Load
file and Cancel separately. Assign the macro to function 100 and run it from
the tray. Finally test delete only on disposable data.

**Expected:** Recorded down/up pairs appear with the expected HID usages and
delays. MSDB retains 20 slots and empty names; MSMACRO stays cp932, uses the
6-byte record layout and checksum, and preserves `MacroFilePath`. An untouched
file is byte-identical, including the final-record byte-3 quirk; an edited file
changes only the intended encoded data. Playback emits the expected sequence,
delays, repeats, and balanced releases. Macro playback does not use feature
reports.

**Risk/stop condition:** Back up before Save/Delete. Stop on UTF-8 replacement,
path rewriting, a checksum mismatch, an unexpected file deletion, input sent to
the wrong foreground window, or a stuck macro key.

**Completion evidence:** before/after file hashes and byte diff, decoded action
table, playback observation, and zero configuration SET/GET from the tray.

### V-13 — Software profile switching

**Status:** LIVE PENDING; resolver/profile-intent tests are STATIC PASS.

**Prerequisites:** Five disposable profiles with unmistakably different safe
outputs and a button assigned to function 14. Keep all outputs within a test
application.

**Procedure:** Exercise next, previous, and cycle forms, including wraparound.
Switch while a keyboard/mouse action and a macro are held/queued. Verify the
editor's file state separately from resident active state.

**Expected:** Active profile changes among 0..4, old held actions are released,
pending macro work is canceled, and the replacement profile is used for the
next edge. Profile switching is software-only: no `MI_02` open, SET_REPORT, or
guessed DPI feature report. DPI actions (13/47/52/53) are safely ignored.

**Risk/stop condition:** Never use a profile switch to infer a device-side
profile/commit mapping. Stop on a stale held action, wrong profile output, or
any config report.

**Completion evidence:** profile/output timeline, process and pcap evidence,
and confirmation of release/cancellation on switch.

### V-14 — Keyboard capture and assignment dialogs

**Status:** LIVE PENDING; virtual-key/usage unit tests are STATIC PASS.

**Prerequisites:** Editor window, disposable profile, safe keys, and a target
application. Avoid capturing system shortcuts that could close or lock the
session.

**Procedure:** Open Single Key and Combo Key dialogs from a button menu; press
letters, digits, function keys, JIS/OEM keys, and Ctrl/Alt/Shift/Win modifiers.
Confirm, cancel, reopen, and save/load the assignment. Check the displayed
usage label and modifier order.

**Expected:** `GetAsyncKeyState` captures one intended key; combo masks are
  Ctrl, Alt, Shift, Win in the documented order; Cancel changes nothing;
  Confirm writes only the intended profile field/blob. A key without a safe
  usage mapping is rejected or remains unset. No hardware Apply is implied.

**Risk/stop condition:** Do not let the captured key leak into a destructive
  foreground window. Restore the profile if a cancel mutates it or an OEM key
  is mislabeled.

**Completion evidence:** screenshots, saved PFD diff, usage/modifier table, and
  no target HID traffic.

### V-15 — Tray lifecycle and editor handoff

**Status:** LIVE PENDING; argument parser is STATIC PASS.

**Prerequisites:** Windows notification area, Rust executable, V-09 resident
  prerequisites, and a clean process list.

**Procedure:** Launch with no argument and with `--tray`/`--TRAY`. Confirm tray
  icon and tooltip, left-click icon, choose `設定を開く`, choose `終了`, and
  close the child editor. Repeat with the resident device unavailable and with
  the resident worker failing after a disconnect.

**Expected:** Normal launch is editor mode; case-insensitive `--tray` launches
  the tray worker. Icon/menu shows `設定を開く` and `終了`; left click/open
  starts a separate `--editor` process; Exit sets the stop flag, joins the
  worker, and leaves no stale tray/editor process. Worker failure leaves the
  icon available and changes the tooltip to the stopped state.

**Risk/stop condition:** No hidden editor, orphaned worker, duplicate tray,
  or automatic configuration write may appear. Stop before testing auto-start
  if the prior Run value has not been backed up.

**Completion evidence:** process tree before/after, tray screenshots, command
  line, tooltip/error text, and clean exit confirmation.

### V-16 — Installer and uninstaller, WhatIf first

**Status:** LIVE PENDING; `phase4.rs` plans and safety tests are STATIC PASS.

**Prerequisites:** Windows PowerShell, a built `redsamurai-config.exe`, unique
  absolute temporary install/data directories, and a backup of any existing
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value named
  `RED SAMURAI 16400DPI Gaming Mouse`. Close tray/editor before uninstall.

**Procedure:** From `redsamurai-config`, first review the exact dry runs:

```powershell
& .\installer\install.ps1 `
  -SourceDirectory 'C:\absolute\build\directory' `
  -InstallDirectory 'C:\absolute\temp\RED SAMURAI' `
  -DataDirectory 'C:\absolute\temp\Documents\RED SAMURAI 16400DPI Gaming Mouse' `
  -WhatIf

& .\installer\uninstall.ps1 `
  -InstallDirectory 'C:\absolute\temp\RED SAMURAI' `
  -DataDirectory 'C:\absolute\temp\Documents\RED SAMURAI 16400DPI Gaming Mouse' `
  -WhatIf
```

For the bounded live integration check, close the tray/editor, verify that the
production Run value is absent, then run the disposable harness once:

```powershell
& .\installer\live-integration.ps1 `
  -SourceDirectory (Resolve-Path .\target\release).Path `
  -LiveConfirmation RED-SAMURAI-LIVE `
  -Confirm:$false
```

Retain its JSON output with the run log. The harness is intentionally separate
from `-WhatIf`: it performs real HKCU and temporary Documents operations only
inside its GUID-scoped fixture and removes that fixture in `finally`.

Confirm WhatIf made no files or registry changes. Run install once, inspect the
JSON and filesystem, inspect the HKCU Run value, run install again for
idempotence, then close all processes and run uninstall WhatIf and uninstall.
Repeat uninstall to test the no-install/no-marker case. Test rejected relative
paths, `..`, NUL/invalid executable names, missing source executable, and an
untrusted/missing marker in a disposable directory. Do not use a machine-wide
path to make the script silently elevate.

**Expected:** Install creates only the selected install/data directories,
copies the executable, writes an exact `.redsamurai-install` marker, and sets
one current-user Run value named `RED SAMURAI 16400DPI Gaming Mouse` to a
quoted executable path plus `--tray`. No elevation/HKLM/service is used.
Uninstall removes that Run value and recursively removes only a directory with
the exact marker; Documents/product data is preserved. Repeated runs are safe.

**Risk/stop condition:** Never test against the real product Run value without
restoring its backup. Stop if a path is accepted with `..`, a data directory is
deleted, an unmarked directory is removed, or an install writes HKLM.

**Completion evidence:** WhatIf output, before/after directory listings and
hashes, registry value, marker contents, JSON output, logon/autostart result,
and proof that data survived uninstall.

### V-17 — Autostart at logon and resident recovery

**Status:** LIVE PENDING.

**Prerequisites:** A completed V-16 install in a disposable user/profile or VM,
the Run value backup, and a known-safe profile. The operator must be able to
sign out or restart Explorer without losing unsaved work.

**Procedure:** Sign out/in (or use a controlled logon test), observe the tray
startup, then disconnect the mouse, wait for the worker error, reconnect, and
exercise the documented recovery (restart tray if automatic recovery is not
implemented). Inspect the Run command and process arguments.

**Expected:** Logon starts the quoted executable with `--tray`; the tray stays
available when the resident worker stops and accurately reports the stopped
state. Recovery never changes profile files or sends configuration writes until
an explicit editor Apply.

**Risk/stop condition:** Do not leave a test Run value behind. Stop on an
unquoted path, editor mode at logon, duplicate workers, orphaned processes, or
silent feature writes.

**Completion evidence:** registry value, logon process command line, tray
screenshots, disconnect/recovery timeline, and cleanup confirmation.

### V-18 — Future protocol-mapping promotion gate

**Status:** NOT AUTHORIZED / LIVE PENDING. No new mapping is promoted by this
document.

The following observations remain unverified and must stay warnings/raw-data
only: `02/F3/44` DPI values, `02/F3/5C` DPI enable, `02/F3/49` LED mode/color,
`02/F3/4F` brightness, `02/F3/8E` button function, all ordinary profile scalar
fields, subfunctions 48/49/52/53, `02/F2/2C` status/commit behavior, F1/F5
meaning, and the `04/F3/C8` 1024-byte candidate.

To qualify a future mapping, run a clean A/B test with one field changed, use
the full official Apply burst, capture both values, perform a reconnect or
restart readback that persists the intended value, prove the exact endpoint and
report length, and retain the complete ordered sequence (start, end, every
report, and completion). A UI screenshot or a one-byte difference is
insufficient. Until all of that exists, do not add an entry to
`VERIFIED_COMMAND_MAPPINGS`, do not use a standalone frame, and do not broaden
the 125 Hz token.

## Evidence record template

For each live run, append or attach a record containing:

```text
Run ID / operator / UTC and local time:
Windows version and integrity level:
Rust executable and installer SHA-256:
Tool versions and capture command:
VID/PID, USB address, PnP instance, HID path, collection/usage/interface:
Pre-run profile/macro hashes and device health:
Single intentional action and expected result:
SET/GET counts, lengths, report IDs, IRP completion statuses:
Readback marker/profile hash (or explicit “not attempted” reason):
Screenshots/logs/pcap paths and SHA-256:
Cleanup, restoration, pending-reboot state, and operator sign-off:
Final status: STATIC PASS / EVIDENCE PASS / LIVE PASS / BLOCKED / FAIL
```

## Release/closure criteria

The repository's static safety gate is currently green. A hardware/runtime
release must additionally have signed records for V-01 through V-17, with V-05
and V-06 tied to the Rust executable hash, not only to E-125's official
`ldcfg.exe` capture. V-07 must show no speculative I/O; V-09 through V-15 must
show the MI_01/read-only separation and safe software input; V-16 and V-17 must
show WhatIf review, marker-protected uninstall, data preservation, and clean
autostart/recovery. V-18 remains closed until a separate evidence review
authorizes a new mapping.

Until those records exist, the honest status is: PFD/UI/static protocol proof
is complete, the official 125 Hz physical sequence/readback is existing
evidence, and current-binary Windows/device/runtime acceptance is still
pending.

## Latest coordinator continuation after the four-worker wave — 2026-09-10

The final shared-tree validation after all four Orca workers completed is
green. `cmd /c msvc_cargo.bat fmt --all -- --check`, `check --all-targets`,
and `test --all-targets` pass. The suite contains 213 passing tests: 115
library tests, 6 readback tests, 8 live-probe tests, 4 macro round-trip tests,
7 ordered Apply tests, 10 PFD tests, 30 resident-core tests, 12
resident-platform tests, and 21 USBPcap tests. The PowerShell installer
harness reports `installer harness: PASS`, and the release build with the
examples succeeds.

The current release hashes are:

- `target/release/redsamurai-config.exe` —
  `8B2009A59B21E1C00E2F2EB1512EEB29CB30D02E8F4A8BD1271D004828D56793`.
- `target/release/examples/live_probe.exe` —
  `5D07D092192277F43961E959B838E862ECE9BB61C2490E4DB248811DEABD53E8`.

The resident service now has a finite, stop-aware retry loop with capped
backoff, reset-before-retry cleanup, recoverable-error classification, and a
sticky stopped tray state after exhaustion. The device transport has a
read-only `PollingRate` observation seam behind the authorized 156-frame
Apply token; it does not add a write registry entry or claim persistence.
Installer startup verification now records the current user, HKCU scope,
Windows session, exact quoted Run command, process owner/session, and
creation-aware cleanup state. The live current-user installer run
`3d92d39c661c4c25855d831c66266c24` passed with matching source/data hashes and
no remaining Run value, process, or fixture.

### Current-binary readback attempt — fail-closed evidence

The rebuilt probe was run once with the official `hid.exe` stopped only for
the authorized operation:

```text
live_probe.exe --confirm-live --apply-125hz --confirm-hid-apply --observe-polling-readback
```

USBPcap recorded 156 target `SET_REPORT`s (155 x 16 bytes and 1 x 64 bytes),
156 zero-status completions, and one target `GET_REPORT` request. The capture
is `captures/wave3-readback-20260910T183938188-5432bbae/final-release-apply-readback-usbpcap2.pcap`,
95,323 bytes, SHA-256
`FF14D27D64909678C569A10AC9BF8697DC4DE3876713B2ED3F07F119E724854C`. The
probe discovered exactly one configuration and one resident collection, then
returned:

```text
PollingRate readback observation failed: ... expected 13 bytes, got 8
```

The device response therefore did not match the exact 13-byte marker shape
required by `PollingRateReadback`; the probe exited 1 and made no persistence
claim. The pcap parser summary and stdout/stderr are retained beside the
capture. Cleanup restored the exact official executable, responding as PID
48324, and left no Rust/USBPcap process, HKCU product Run value, or Documents
fixture. This is useful negative evidence for the current direct route; the
existing vendor restart/readback captures remain the only persistence
evidence.

The bounded Raw Input monitor still reports no physical button transition,
and no physical unplug/replug, automatic disconnect recovery, or controlled
sign-out/logon has been performed. The closure decision remains **NOT
COMPLETE** until those operator-only gates and the Rust reconnect/persistence
readback are separately evidenced. V-18 remains unauthorized.

The reproducible operator procedures for these four gates are recorded in
`../REMAINING_LIVE_GATES_RUNBOOK.md`. The runbook also records the distinction
between the current direct 8-byte GET response and the 13-byte marker required
for a persistence claim.

After the runbook was written, the bounded monitor was corrected so an idle
`read_transitions` timeout continues until the requested deadline. A new
test-first regression test covers an idle timeout followed by press/release
edges. `target/release/examples/live_probe.exe` now hashes to
`0BFBDE72F34A685607A7A65F9ED0D8C44F160D08E7F378027C4BB8EEF8AAD098`; a
3,000 ms live monitor with no input ran for about 3,306 ms and ended with
`resident_monitor=timeout ... transitions=0`, proving it no longer exits on
the first idle interval. The all-target suite is now 214 tests including the
new regression.

## Operator monitor retry after log-path correction — 2026-09-10

The first manual command reported a PowerShell `Tee-Object` path error because
it was run from `redsamurai-config` while its local `captures` directory did
not exist. The probe was rerun with the shared capture directory created
explicitly. The artifact is
`../captures/manual-physical-edge-20260910-190345.log`; it used the current
release probe and ran the full 30,000 ms window. It recorded exactly one
resident candidate and ended with `resident_monitor=timeout ... transitions=0`.
No physical edge is claimed from this run. The runbook now creates the shared
capture directory before invoking `Tee-Object`.

The operator then repeated the monitor while clicking the ordinary pointer
button. The exact device list shows that collection as `MI_00` (`Class=Mouse`),
while the probe's selected path is `MI_01\\KBD` (`Class=Keyboard`). The resulting
`transitions=0` is therefore consistent with the collection filter and is not
evidence of an OS access failure. The MI_01 gate still requires a physical
side, extra, or macro button that is routed through the keyboard-style
collection; normal pointer-button coverage is a separate MI_00 implementation
requirement and remains unclaimed.

## Raw Input keyboard payload-length fix — 2026-09-10

When the operator pressed GUI button 7, the old release reported
`GetRawInputData read failed (Win32 error 6)`. The reader compared the returned
keyboard packet with the padded size of the entire `RAWINPUT` union, so a valid
header-plus-`RAWKEYBOARD` packet was rejected. A test-first regression now
accepts the exact keyboard payload length and the reader decodes the keyboard
payload only after that bound check. The new all-target suite passes 215 tests;
`fmt --check`, `check --all-targets`, and the release build pass. The rebuilt
probe hash is
`FDD33717F091FEBBFDAC5E7F03B9525C1EEEFD36AB43AA1C74A80A17E8ADA0F8`.
The new binary's 3,000 ms no-input run completed with a normal timeout. A
30-second GUI-7 press/release run is still required to record the physical
`press` and `release` transitions.

## Raw Input interface-GUID normalization fix — 2026-09-10

The diagnostic artifact `captures/raw-input-trace-20260910-193116.log` captured the target mouse's keyboard collection during the operator run: `VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000`. The event was previously rejected only because hidapi reported the HID interface GUID `{4D1E55B2-F16F-11CF-88CB-001111000030}`, while Raw Input reported the keyboard-class GUID `{884B96C3-56EF-11D1-BC8C-00A0C91405DD}`. The VID/PID, MI token, instance path, and collection were identical.

The path matcher now removes only that known HID/keyboard class-GUID representation pair (plus the existing `\\KBD` suffix and NUL terminator) before comparison. The instance and collection identity remain exact; an unrelated GUID and `MI_00` are still rejected by regression tests. The all-target suite passes 215 tests, formatting/check pass, and the release probe hash is `8F3DA99BD6FFCCD499ECA3B37197E8B2B92539FD962D2E61EB4B469BDE9CAB2D`.

A new 30-second operator run with this binary and SIDE button 7 is still required. The previous trace is evidence of the matching bug, not a physical press/release transition claim.

## Physical MI_01 edge evidence — 2026-09-10

The rebuilt release probe accepted the target keyboard-style collection after the interface-GUID normalization fix. The operator artifact is `captures/raw-input-trace-20260910-193116.log` (SHA-256 `512A9E4F80043DD36315E42E5C661FC94E7918FC745933AA4165431124C3A250`). It records the exact `VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000` path and contains `resident_transition=press usage=0x1E` followed by `resident_transition=release usage=0x1E`; the monitor ended normally with `resident_monitor=timeout duration_ms=30000 transitions=92`.

Usage `0x1E` is the configured key for SIDE button 7. The additional press records are Windows keyboard autorepeat during the held-button interval; they are not counted as separate physical clicks. This is positive evidence for a real MI_01 press/release cycle with no feature-report writes. Full V-09 coverage (all assigned buttons, tray runtime actions, and USBPcap attribution) remains pending.

## Resident recovery instrumentation — 2026-09-10

The resident worker now writes an opt-in `REDSAMURAI_RECOVERY_LOG` trace. It
records `input_open_start`/`input_open_ok` or the bounded open error, reset and
backoff events, and each accepted Raw Input press/release transition. The
default policy remains finite at 30 reconnect retries with 100 ms initial and
1,000 ms maximum backoff (about 27.5 seconds of total wait). A tray smoke run
with the current main release reached `input_open_ok` in about 132 ms and was
then stopped by exact executable-path cleanup; no physical disconnect was
performed in that smoke run.

The worker also polls read-only resident HID availability every 500 ms. This
is a fallback for hardware that removes the Raw Input handle before Windows
delivers a path that can be matched; a missing target emits
`event=input_presence_missing` and enters the same reset/retry state.

The current all-target MSVC suite passes 216 tests, with formatting, checks,
and the release build passing. Current hashes are main
`0A90E8345BB6D59A7C85C56636B427899587F315A3FD49827DE1C6E17EA11B93` and
probe `901DDCFB8833734FD04535DAEEA31A5E823D0B6C48D14C0BA82F7B73EACDFEF1`.
Physical unplug/reconnect, sign-out/logon, and Rust Apply persistence/readback
remain live operator gates; this instrumentation does not promote any of them
without their corresponding run artifacts.

The current release also completed the harmless one-pixel Windows input probe:
`sendinput_one_pixel=accepted`. The captured output is
`../captures/sendinput-one-pixel-20260910-195426.log` (SHA-256
`96658F02521D21CA8F493FCE3C171DA30A1FD1354F9E32439B31E8CDAD9007B9`). This
confirms the `SendInput` API boundary for the current probe; it is not evidence
that a physical resident button was translated into an action.

## Physical disconnect and resident recovery evidence — 2026-09-10

The current main release (`0A90E8345BB6D59A7C85C56636B427899587F315A3FD49827DE1C6E17EA11B93`)
was run in tray mode with `REDSAMURAI_RECOVERY_LOG`. The operator physically
removed the mouse, left it absent long enough for the read-only presence poll
to observe the missing resident collection, reconnected it to the same USB
port, and pressed SIDE 7 once after PnP returned. The artifact is
`../captures/physical-recovery-20260910-200405682/recovery.log` (SHA-256
`331888CAF6F09A7D7A301757B574617C19B234A3083B2F8E35656CEAB6525DE6`).

The log records `input_presence_missing` at `timestamp_ms=1789038258741`,
followed immediately by `step_error`, a successful runtime `reset`, and
bounded backoffs of 100, 200, 400, 800, then 1,000 ms. Eight bounded open
attempts failed while the device was absent. After reconnection, the next
attempt reached `input_open_ok` at `timestamp_ms=1789038265592`, followed by
`usage=0x1E` press and release transitions. The process remained the same
tray worker; cleanup left no Rust process, while the official vendor process
remained at its exact path.

This is positive evidence for physical disconnect detection, held-state reset,
finite retry, re-open, and post-reconnect resident input. It closes the
physical disconnect/recovery subcheck for the tested device and button. Full
V-09 all-button coverage, sign-out/logon, and Rust Apply persistence/readback
remain separate live gates.

## Reboot/logon tray startup and readback evidence — 2026-09-10

The two-phase logon harness completed a real reboot-to-automatic-logon run
using the current-user HKCU Run registration. The retained artifact is
`../captures/logon-transition-27fba53a3340437ca2d1decf4d9399b6/`; its
`logon-result.json` has `status=pass`, process ID `38732`, the exact quoted
`--tray` command, matching `KOTA-PC\\k0ta0` identity and session `1`, a
matching data SHA-256 before/after logon, and complete PID, Run, install, and
data cleanup. The disposable fixture was removed only after the hash was
rechecked. The fixture data hash was
`CF4DA339429694F38581F4B360C49FDB40587CE138EAD9D7BBC40EE8DA522589`.

The saved `shell-core-run-evidence.txt` records the HKCU Run enumeration and
the target command dispatch at `20:27:05.878 +09:00`, after the enumeration
started at `20:21:51.048 +09:00`; this explains why a 15-second Finish check
reported zero processes. `boot-session-evidence.txt` records the reboot at
`20:20:39.500 +09:00` and `qwinsta` before/after. The session ID was reused as
`1`, so this artifact proves startup after reboot and automatic logon plus
readback/cleanup, while a sign-out followed by a different session ID remains
unclaimed because passwordless sign-in did not resume after sign-out.

The logon harness now defaults to a 420,000 ms observation window and accepts
PowerShell 7 `DateTime` as well as WMI DMTF process creation values. Its
side-effect-contained installer test passes (`installer harness: PASS`).

## Final acceptance addendum — 2026-09-10

This addendum supersedes stale LIVE PENDING labels where the cited artifact
now provides direct evidence. It does not broaden a bounded observation into
an untested whole-gate result. The current release hashes used by the live
artifacts are:

- redsamurai-config/target/release/redsamurai-config.exe —
  0A90E8345BB6D59A7C85C56636B427899587F315A3FD49827DE1C6E17EA11B93
- redsamurai-config/target/release/examples/live_probe.exe —
  901DDCFB8833734FD04535DAEEA31A5E823D0B6C48D14C0BA82F7B73EACDFEF1

| Gate | Final status | Direct evidence and limit |
| --- | --- | --- |
| V-01 startup/no-I/O | PASS (bounded) | captures/ui-acceptance-20260910-220824-7c877600/acceptance-report.md, SHA-256 C613ED1123067313F4094B7993956AA19C4050A06A1897E4D20E328C74ACE749; startup USBPcap artifact is startup-no-io-usbpcap2.pcap, and editor/USBPcap cleanup counts are zero. |
| V-02 frame, tabs, accessibility | PASS (bounded) | UI report records the General/DPI/Light/Info states and stable red-samurai.* IDs. Full persistence of every control and real-device writes remain outside this bounded run. |
| V-03 dry-run/fail-closed planning | STATIC PASS | Existing Rust test/build ledger remains authoritative; no new live write is inferred from UI state. |
| V-04 endpoint separation | PASS (bounded) | Live probe found exactly one configuration collection (VID_04D9, PID_FC55, MI_02&Col02, Usage Page FFA0) and one resident MI_01\KBD collection (Usage Page 0001, Usage 0006, 9-byte report). |
| V-05 ordered Rust Apply | PASS | captures/rust-persistence-20260910-204420/rust-apply-reconnect.pcap, 2,270,659 bytes, SHA-256 3180F808532AD9AE4AFB9347FFF0BB24B3EEFEF2188AF3AAA04F467A9D576C8B: Apply window contains 156 target SET_REPORTs (155×16 and 1×64), all status 0, with wire byte 08 at report 13. |
| V-06 persistence/readback | PASS (bounded 250→125 A/B) | Same pcap post-reconnect window contains 193 GET_REPORT requests with matching status-0 responses and the 13-byte marker 02 08 32 60 05 00 fa fa 08 00 08 00 02 (wire byte 08, 125 Hz). Profile hash intentionally changes from 250 Hz pre-apply state to known 125 Hz backup; this is an A/B transition, not an unchanged-data claim. |
| V-07 unsupported fields/no speculative writes | PASS (bounded) | Apply capture and Rust planning/test ledger show ordered path only; no standalone or guessed feature report was authorized. |
| V-08 completion/reconnect enumeration | PASS (bounded) | Target PnP rows were Status=OK after reconnect and every target Apply/readback completion was status 0. Two unrelated 0xC0000004 enumeration controls are retained as a known caveat, not counted as feature-report failures. |
| V-09 physical resident edges | EVIDENCE PASS (factory side-key set) | `captures/mi01-action-matrix-20260911-154744/assessment.md` records all 12 observed factory usages (`0x1E..0x27`, `0x2D`, `0x33`) with raw press/release rows under the current release. This is bounded to the attached device's MI_01 factory side-key set; other unassigned usages are not inferred. |
| V-10 live action matrix | PARTIAL — MI_01 factory-side routing and native SIDE 7 pass-through PASS; microphone mute PASS; full action-class gate PENDING; all-keyboard relay diagnostic only | `captures/mi01-action-matrix-20260911-154744/assessment.md` records 12/12 raw-to-injected routes and expected VKs. `captures/native-keyboard-pass-through-20260912-113532/result.json` records one raw/native and hardware pair, zero injected target edges, and exact foreground clipboard `1` for SIDE 7. `captures/microphone-mute-20260911-180756/result.json` records Core Audio `communications` readback `true → false`. `captures/keyboard-duplicate-audit-20260911-162712/assessment.md` confirms co-delivered hardware and Rust events for software action. Broader profile action classes (mouse/media/macro/combo) remain pending; relay physical acceptance is comparison evidence, not a product gate. |
| V-11 debounce/held/reset/disconnect | PARTIAL — held/reset/disconnect release, same-state repeat, and factory action coverage subchecks PASS; full gate PENDING; relay diagnostic only | The current-release proof in `captures/rust-owned-recovery-20260911-152302/held-disconnect-proof-result.json` ties an injected key-up to the reset boundary. `captures/debounce-duplicate-20260911-153434/result.json` records one injected pair for 76 repeated raw presses during a hold. A sub-5 ms bounce remains untested; keyboard-class co-delivery is confirmed and the diagnostic relay is not the official-compatible production path. |
| V-12 macro manager/record/playback | PARTIAL — parser PASS; live manager gate PENDING | redsamurai-config/captures/macro-profile-acceptance-20260910-task_34de4db0cd84/REPORT.md records 4/4 macro round-trip tests PASS and production hashes unchanged. Safe manager child-node click did not expose expected nodes, so recording/edit/save/playback remains PENDING. |
| V-13 software profile switching/persistence | PASS (bounded) | Same macro/profile report records 10/10 profile round-trip tests and UI selector timeline 0→1→2→3→4→0, with disposable-path writes only. |
| V-14 keyboard capture/assignment | PARTIAL | UI report proves Single Key capture/cancel; Combo/modifier/commit remains PENDING. |
| V-15 tray lifecycle/editor handoff | PENDING | No complete tray lifecycle/action matrix was claimed by the safe UI or zero-transition resident run. |
| V-16 installer/uninstaller | PASS (side-effect contained) | Existing current-user installer harness passed with hash/cleanup checks; the logon run below also records complete process, install, and data cleanup. |
| V-17 sign-out/logon/autostart | PASS | captures/logon-transition-2b4deece1f8c43f8871e5c912041c287/logon-result.json reports status=pass, session_id_changed=true, exact quoted --tray command, identity KOTA-PC\k0ta0, process session 2, equal data SHA-256 before/after (CF4DA339429694F38581F4B360C49FDB40587CE138EAD9D7BBC40EE8DA522589), and complete process/run/install/data cleanup. |
| V-18 protocol-mapping promotion | NOT AUTHORIZED | No new mapping or standalone report is authorized by this acceptance record. |

### Final artifact notes

The official vendor restart check was captured separately in
captures/rust-persistence-20260910-204420/baseline-official-restart.pcap
(SHA-256 EFFB064147433D4A8BD3E88476BA47AC24E138959D1148155F7F04FE75271199);
the restored official hid.exe remained at its exact installed path and
responding. The zero-transition all-button attempt is retained as negative
evidence and is not a pass. All PASS labels above are bounded by the cited
artifact; remaining PENDING rows require a future operator run with the same
exact-collection and no-synthetic-input rules.

## Selected DPI mapping promotion addendum — 2026-09-11

This addendum supersedes the V-18 status only for the selected profile `DPI`
values `1` and `2`. It does not authorize a standalone `02/F3/42` write, the
DPI stage/table fields, or any other unverified mapping.

The rebuilt release artifacts used for the static validation are
`target/release/redsamurai-config.exe` (SHA-256
`71BBA57FA1582CA13EE899784C42B3DEC177A5DEC613367D69E5B761FD081395`) and
`target/release/examples/live_probe.exe` (SHA-256
`7E1231FE48CC150F4810F8C00A3DF612CB6C05106BB2D0C611F32AF90A4F6C28`).

The controlled official Run A/Run B captures are recorded in
`../captures/dpi-reconnect-readback-comparison-20260911.md`. Run A kept
`DPI=1`; Run B changed only the selected value to `DPI=2`. Both runs had eight
target collections at `Status=OK` before and after reconnect and 193 paired
status-0 GET responses after reconnect. The complete 156-report Apply bursts
were identical except at report index 16:

```text
Run A: 02f34200020000000100000000000000
Run B: 02f34200020000000200000000000000
```

The post-reconnect GET response sequences were identical except at response
8, where the same byte changed from `01` to `02`. The profile after-copies
also retained `DPI=1` and `DPI=2`. The corresponding PCAP hashes are
`47976E65CEF53219B19A745670B208FA882BDF820475190D55E8C3F29652C29C` and
`0B51997C8D49886DC4ED17AD66CA7DA9221755E6F7E49E1E0533D83CECBDC4ED`.

Rust now represents this evidence as a typed, read-only 40-byte
`DpiSelectionReadback` on the exact report-03 GET route. The authorized
`VerifiedApplySequence` can substitute byte 8 of its report-index-16
`02/F3/42` frame only for profile values `1` and `2`; all other values remain
fail-closed warnings. `--observe-dpi-selection-readback` probes that route in
`live_probe`; a standalone GET may return an 8-byte alternate response and
therefore does not produce a selected-DPI value. A 40-byte observation still
deliberately retains `persistence=not-observed` because a single direct
observation is not a reconnect proof.

| Gate | Updated status | Boundary |
| --- | --- | --- |
| V-18 selected-DPI subset | EVIDENCE PASS / STATIC PASS | Values `1` and `2` are promoted only as substitutions inside the complete 156-report token, backed by the Run A/B Apply→reconnect→readback evidence above. `VERIFIED_COMMAND_MAPPINGS` remains empty. |
| V-18 remaining mappings | NOT AUTHORIZED | DPI stage/table, other DPI values, standalone reports, button/LED mappings, and commit semantics remain warnings/raw-data only. |

### Direct selected-DPI probe context — 2026-09-11

The standalone current-binary command
`--confirm-live --observe-dpi-selection-readback` was captured in
`../captures/dpi-readback-diagnostic-20260911-20260911-123457/`. Discovery
found exactly one verified `MI_02&COL02` collection and the read-only request
was `A1 01 03 03 02 00 40 00`, but the device returned the short response
`03 08 BA 63 00 00 FA FA` (8 bytes). The PCAP SHA-256 is
`F37F02D4188D3109A72D2A67628268014865FA7E5118C2BFC7192126266458CA`.

This is a valid alternate response to a standalone GET, not an OS access or
sandbox failure. The 40-byte selected-DPI response appears in the official
captures only after the complete Apply sequence has issued the preceding
`03/F3/42` SET_REPORT. The Rust path therefore keeps the short response
fail-closed and now reports its bytes plus the required Apply/reconnect
context; it never treats the short response as `DPI=1` or `DPI=2`.

The rebuilt binary was rechecked with the same read-only command after this
diagnostic handling was added. The log is
`../captures/dpi-short-probe-20260911-125715/live-probe.log` with SHA-256
`5B8B67657704693F4C19F9D06CEA73C6355CADFB7A2D045102A796D8118A4EAA`.
Discovery again found one configuration and one resident collection, and the
device returned `03 08 B6 63 00 00 FA FA` (8 bytes). The process exited with
code 1 by design: the short response was rejected with its byte values and the
required full Apply/reconnect context instead of being interpreted as a DPI
selection. This rerun confirms the diagnostic fix; it does not close the
separate current-Rust Apply/reconnect/readback gate.

### Current-Rust selected-DPI reconnect probe — ready for live execution

The release `live_probe` now has a separate `--apply-dpi-profile 1|2` action.
It accepts only the two A/B-proven selected-DPI values, requires both
`--confirm-live` and `--confirm-hid-apply`, sends the complete 156-report token
with the selected value substituted at the reviewed report index, pauses for a
physical unplug/reinsert, and opens a fresh configuration handle for one
40-byte Report-03 readback. It rejects standalone/readback/SendInput actions in
the same invocation. The wrapper
`../captures/dpi-rust-reconnect-readback.ps1` bounds USBPcap, temporarily stops
and restores the exact official `hid.exe` process, and writes `result.json`,
the probe log, and PCAP hashes. No live result is promoted until both profile
values satisfy the 156-report, status-0, PnP-OK, and matching 40-byte criteria.

### Current-Rust selected-DPI reconnect probe — negative result — 2026-09-11

The two current-Rust runs are retained as bounded negative evidence. Profile 1
sent the exact authorized 156-report sequence with report index 16 byte[8]
`0x01`; Profile 2 sent the same sequence with that byte `0x02`. Both runs had
156 status-0 SET completions and found one configuration collection after
reconnect. Each then issued one `A1 01 03 03 02 00 40 00` GET_REPORT and received
the same short response `03 08 00 00 00 00 FA FA` (8 bytes), so the typed
40-byte decoder rejected it and the process exited 1 by design.

| Run | Probe SHA-256 | PCAP SHA-256 | Result artifact |
|---|---|---|---|
| Profile 1 (`131357`) | `7E1231FE48CC150F4810F8C00A3DF612CB6C05106BB2D0C611F32AF90A4F6C28` | `22C1041B7730BB0908DACD225C3551413A4A8DB73FB0B2CD9CC2BD5220BBB8DA` | `../captures/dpi-rust-reconnect-readback-20260911-131357/` |
| Profile 2 (`131530`) | `7E1231FE48CC150F4810F8C00A3DF612CB6C05106BB2D0C611F32AF90A4F6C28` | `B9CEEA4769B90827B9910D5DD1EE464746AF98FE7B09237AD264E3D9C43D53D5` | `../captures/dpi-rust-reconnect-readback-20260911-131530/` |

PCAP parsing shows one Rust Apply burst before unplug/reinsert and one
standalone GET after reconnect. The official A/B captures that returned the
40-byte selected-DPI response also contain a post-reconnect context of 195
SET_REPORT requests and 193 paired GET_REPORT responses. That context is not
present in either Rust run. Therefore the result is **Apply/PnP PASS but
Rust-only selected-DPI persistence readback NOT PROVEN**; it is not an OS,
sandbox, permission, or HID collection discovery failure. Do not promote the
short response and do not repeat the same two commands. M2 remains pending
until the context sequence is either explicitly implemented under a new
reviewed authorization boundary or the acceptance scope is changed to an
external official-reader oracle.

### Rust-owned runtime without the vendor process — static implementation — 2026-09-11

The product runtime no longer needs the vendor `hid.exe` or `ldcfg.exe` process.
`src/device_runtime.rs` keeps only a reviewed `VerifiedApplySequence` in memory;
constructing it performs no HID I/O. The tray worker still opens the exact
read-only `MI_01` collection at startup. After a later input reconnect it opens
the exact configuration collection and replays the complete 156-report token
only when the active profile contains the audited `PollingRate=8` and optional
selected `DPI=1|2` values (or the captured baseline `DPI=0`). Unsupported values
are skipped, and a configuration replay error is logged without stopping input
recovery.

The editor Apply path uses the same isolation: a profile can retain warnings
for unverified fields while the known sequence is applied; no warning-bearing
raw frame is sent. This removes the former practical need to keep the official
process alive while preserving the fail-closed protocol boundary.

Static evidence for this change:

| Check | Result |
|---|---|
| Release tray binary | `target/release/redsamurai-config.exe`, SHA-256 `FACCC52DA83EBD88B6D74CC25074ADC7559CF2C427CDA21479E5A2DC49E5726A` |
| Release probe binary | `target/release/examples/live_probe.exe`, SHA-256 `CBCBFBCA512762715C05B3B3A0A12EDBA87D2D99F74143475DD944FE1A561AB9` |
| `tests/rust_owned_config.rs` | **PASS**, 7 tests: no-vendor sequence replay, baseline-DPI polling replay, malformed-value rejection, startup no-I/O, unsupported-value no-op, incomplete completion rejection, and known-sequence isolation |
| `msvc_cargo.bat test --all-targets` | **PASS**, 236 test cases across the library and integration targets |
| `msvc_cargo.bat fmt --all -- --check` | **PASS** |
| `msvc_cargo.bat check --all-targets` | **PASS** |
| Live no-vendor recovery | **PASS**: `../captures/rust-owned-recovery-20260911-140802/result.json` and `recovery.log`; official process count 0, input disappearance/reopen, Rust replay 156 reports |

The live no-vendor run completed with tray SHA-256
`FACCC52DA83EBD88B6D74CC25074ADC7559CF2C427CDA21479E5A2DC49E5726A` and
recovery-log SHA-256
`242E91F8D124952840454DEC692D76CFCCF71D5A45296F9E4FFB561DEA5DD486`,
and result SHA-256
`029F1CB54110F00576B90DE79909961381BE3BDCE91C4714762A6CEBEFE180F1`.
It records `official_processes_remaining=0`,
`input_presence_missing=true`, `input_open_ok_count=2`,
`config_reapply_ok owner=rust reports=156`, and a post-reconnect
`usage=0x1E` press/release pair. This is the live proof that the product
runtime does not require the vendor process.

The existing standalone selected-DPI readback limitation is unchanged. A short
8-byte Report-03 response is still rejected; that readback proof is an evidence
gate and is not a runtime dependency.

### Held-disconnect injection proof correction — 2026-09-11

The proof attempt in `../captures/held-disconnect-proof-20260911-145928/` ran the
low-level keyboard hook for the full 180 seconds and recorded only
`watch_start_utc` and `watch_end_utc` (zero `injected_keyboard` events; final
log SHA-256 `DE9FF0DBDED8773ED51AAA06CE41892C57B8A0A91762C3EE4D6A688F7F3F6F46`).
The paired Rust-owned recovery result in
`../captures/rust-owned-recovery-20260911-145929/result.json` is still a recovery
PASS: the resident collection disappeared, the worker reset/backed off, reopened
it, and replayed the authorized configuration sequence with no official process.

This run therefore does not prove a synthetic key-up at the disconnect boundary.
The physical stream is `usage=0x1E`; the binary used for this run accepted only
the `0x04..0x17` button namespace in `button_number_from_usage()`, so
`feed_transition()` returned before `ResidentRuntime` could call the
`SendInput` sink. The `1` observed in Notepad is compatible with the mouse's
keyboard-class hardware input and cannot be used as Rust-injection evidence.
At the time of this historical run, the next corrective task was to specify and
test the physical usage-to-profile button mapping together with the policy for
suppressing duplicate keyboard-class input. The mapping task was completed in
the current source; the resulting current-release held-disconnect proof is
recorded below. Duplicate suppression remains a separate acceptance item.

### Factory side-key usage mapping correction — 2026-09-11

The live MI_01 captures show the attached mouse emitting the factory side-key
usages `0x1E..0x27`, `0x2D`, and `0x33` (the profile table records the final
glyph as `0x34`). `src/resident_service.rs` now accepts those observed values
and maps them to profile buttons 7..18 while retaining the existing bounded
`0x04..0x17` namespace. Regression coverage includes the `0x1E` route through
`feed_transition()` into the resident runtime sink.

The test-first regression was red before the change (`0x1E` mapped to `None`)
and green after it. The current validation is `msvc_cargo.bat test --all-targets`,
`msvc_cargo.bat fmt --all -- --check`, `msvc_cargo.bat check --all-targets`, and
`msvc_cargo.bat build --release --all-targets`, all successful. The new release
hashes are:

| Binary | SHA-256 |
|---|---|
| `target/release/redsamurai-config.exe` | `3989154175A5CEBF0C89C4F8AA2F01E7F0CAB505A1F554A4E2B293C2929EEBE5` |
| `target/release/examples/live_probe.exe` | `CDDCA41F8EB27CA15CF42FA917A6EF89DC41B8F5F1D559A624520EA413913DE2` |

This correction establishes the software dispatch seam. The subsequent current-
release live run is recorded below; it separates Rust output from the mouse's
keyboard-class hardware key output with the injected-event watcher. It does not
by itself establish a policy for suppressing the original keyboard-class event
that Windows may deliver alongside a software action.

### Held-disconnect injection proof — current release — 2026-09-11 15:23 JST

The current release (`redsamurai-config.exe` SHA-256
`3989154175A5CEBF0C89C4F8AA2F01E7F0CAB505A1F554A4E2B293C2929EEBE5`) was run
without the vendor process. The paired recovery artifact
`../captures/rust-owned-recovery-20260911-152302/result.json` is `status=pass`
with `official_processes_remaining=0`, `input_presence_missing=true`,
`input_open_ok_count=2`, and `rust_config_reapply_ok=true`.

The machine-readable proof is
`../captures/rust-owned-recovery-20260911-152302/held-disconnect-proof-result.json`;
the recovery log SHA-256 is
`DB98F594FD9A35797DF93A35C1E10884D853598D6971D85EF89782EE3591A267`, and the
low-level hook log `../captures/held-proof-new.log` has SHA-256
`C3FA9B99803CB7B72CF61209EA8AF823255D9CF7B910A448CEF8E28A47EB1CC5`.

The hook recorded an injected `vk=0x31` down at `1789107789709`, an injected
up at `1789107793724`, then a new down/up pair at `1789107804126` and
`1789107804308`. The recovery log recorded `input_presence_missing` at
`1789107793722` and `step_start step=reset` at `1789107793724`. Thus the
injected key-up is exactly at the reset boundary (0 ms from reset start, 2 ms
after presence loss) and precedes the fresh post-reconnect pair. This is direct
evidence that the Rust reset released the held synthetic key and did not carry
the held state across reconnection. The full V-11 gate remains partial because
short-interval debounce, all-button action coverage, and suppression of any
co-delivered keyboard-class hardware event are separate checks.

### Debounce and duplicate-action capture — 2026-09-11 15:34 JST

The current-release capture is
`../captures/debounce-duplicate-20260911-153434/assessment.md`, with the
machine-readable result in `result.json`. For SIDE 7 (`usage=0x1E`), normal
input produced 10 raw press/release pairs and 10 injected down/up pairs. The
rapid block produced 19 complete pairs and 19 injected pairs. During the hold
block, 76 raw press samples and one release produced only one injected down/up
pair. This confirms stable-edge parity and suppression of same-state keyboard
autorepeat by the resident runtime.

The raw minimum interval was 27 ms, so this run did not exercise the configured
5 ms opposite-edge window; no sub-5 ms bounce claim is made. The low-level hook
records only Rust-injected events, while the `1` characters typed into the
PowerShell prompts cannot separate the mouse's original keyboard-class input
from the software action. Hardware duplicate suppression remains pending.

### MI_01 all-button action matrix — 2026-09-11 15:47 JST

The current release action-matrix run is recorded in
`../captures/mi01-action-matrix-20260911-154744/assessment.md`; its machine-readable
result is `result.json` (SHA-256
`79CE84210EE30D3FD6362611199F2807F4194834FA06C61FD3796CC5BA2B2FDD`). The tray
binary SHA-256 is
`3989154175A5CEBF0C89C4F8AA2F01E7F0CAB505A1F554A4E2B293C2929EEBE5`.

All twelve factory side-key usages were exercised under separate marker
intervals: `0x1E..0x27`, `0x2D`, and `0x33`. Each row contains a physical raw
press/release and a matching Rust-injected down/up with the expected virtual key
`1,2,3,4,5,6,7,8,9,0,-,^`. The totals are raw `13/13` and injected `13/13`
because button 9 (`0x20`) produced two physical cycles. The equal raw and
injected counts mean that extra cycle is not a software-only duplicate.

The official process count and Rust tray count were both zero after cleanup.
This closes the attached device's factory-side usage-to-action routing check.
The foreground clipboard (`1122333344556677889900--:^`) is supplemental only:
MI_01 is a keyboard-class collection, so clipboard text cannot distinguish the
mouse's original hardware delivery from Rust's `SendInput`. Suppression of that
co-delivered hardware event, sub-5 ms bounce behavior, and broader mouse/media/
macro/combo action classes remain separate live gates.

### MI_01 hardware / Rust duplicate-delivery audit — 2026-09-11 16:27 JST

The diagnostic run is recorded in
`../captures/keyboard-duplicate-audit-20260911-162712/assessment.md`; the
machine-readable result is `result.json` (SHA-256
`B2A9A6DCA72C18076299B7AC5EA08E4BFCDC5232649D4BA765F2C07A148A8640`). The
current tray SHA-256 is
`3989154175A5CEBF0C89C4F8AA2F01E7F0CAB505A1F554A4E2B293C2929EEBE5`.

For SIDE 7 (`VK=0x31`), the same low-level hook recorded two hardware
`injected=false` down/up pairs and two Rust `injected=true` down/up pairs. The
first cycle was hardware down at `1789111642648`, injected down at
`1789111642649`, hardware up at `1789111642808`, and injected up at
`1789111642809`. The recovery log contains the matching two `usage=0x1E`
physical cycles. This proves co-delivery of the original keyboard-class event
and the Rust `SendInput` event; it is not an OS permission or sandbox failure.

The intended single operation produced two physical cycles and clipboard `11`,
so this run is not a click-count/debounce measurement. It is sufficient to
close the co-delivery observation: suppression is currently **not implemented**.
No production `RIDEV_NOLEGACY` or low-level hook suppression change is authorized
by this capture; such a policy must be designed with a regression check for
ordinary keyboard input before implementation. Official and Rust tray process
counts were both zero after cleanup.

### Microphone mute semantic action — implementation — 2026-09-11

The former label 「マイミュート」 is now 「マイクミュート」 in the function table and
assignment menu (function ID 45). The action is semantic: it does not emit the
unrelated `VK_LAUNCH_APP1 (0xB6)` value that previously opened Explorer. The
resident service calls Windows Core Audio `IAudioEndpointVolume` for the default
capture endpoint, preferring the `eCommunications` role and falling back to
`eConsole` when needed.

Each trigger performs `GetMute`, computes the inverse, calls `SetMute`, and calls
`GetMute` again. A mismatch is returned as `AudioControl` and is not reported as a
successful action. The release edge is not held or replayed, so one physical press
produces one toggle. When `REDSAMURAI_RECOVERY_LOG` is set, successful changes are
recorded as `event=microphone_mute role=communications|console muted=true|false`.

Static evidence for the implementation:

| Check | Result |
|---|---|
| Trigger contract | `microphone_mute_is_a_trigger_and_does_not_hold_a_media_key` PASS |
| Full MSVC test suite | PASS |
| Formatting | `msvc_cargo.bat fmt --all -- --check` PASS |
| All-target check | `msvc_cargo.bat check --all-targets` PASS |
| Release build | PASS |
| Current tray binary | `target/release/redsamurai-config.exe`, SHA-256 `A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` |
| Current probe binary | `target/release/examples/live_probe.exe`, SHA-256 `CDDCA41F8EB27CA15CF42FA917A6EF89DC41B8F5F1D559A624520EA413913DE2` |

The live check passed in
`../captures/microphone-mute-20260911-180756/result.json`. The current release
SHA-256 is `A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83`.
Two SIDE 7 triggers produced the following readback events:

```text
timestamp_ms=1789117685439 event=microphone_mute role=communications muted=true
timestamp_ms=1789117692952 event=microphone_mute role=communications muted=false
```

The paired recovery log is
`../captures/microphone-mute-20260911-180756/recovery.log` with SHA-256
`BE94D2235462BF2586B54F1465383AD749B0161A25706CA195E675EC91182532`.
This closes the shared Core Audio endpoint toggle and readback check. The OS speaker
volume overlay is not a microphone-state oracle. OEM LEDs, exclusive-mode
applications, and hardware-only mute paths remain outside this contract.

### Volume-up keyboard action — live evidence — 2026-09-11 18:13 JST

The current release was tested with `VK_VOLUME_UP (0xAF)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-AF-20260911-181327/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`4053CFE4F708EE67A07255A5A0235F3A34CCB649AAEB24BEE8673892B5B6FF6E`.

This closes the current-release volume-up action check. The broader V-10 action
matrix remains partial until the remaining safe/explicit action classes and the
keyboard-class duplicate-delivery policy are addressed.

### Volume-down keyboard action — live evidence — 2026-09-11 18:19 JST

The current release was tested with `VK_VOLUME_DOWN (0xAE)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-AE-20260911-181920/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`CB7E1891DCDC9F95DBEED22BBB7B591F47291BB0BD26690B54112FEE3BE6222B`.

This closes the current-release volume-down action check.

### Mute toggle keyboard action — live evidence — 2026-09-11 18:22–18:26 JST

The current release was tested with `VK_VOLUME_MUTE (0xAD)` through
`keyboard-action-audit.ps1`. The first run,
`../captures/keyboard-action-AD-20260911-182212/result.json`, delivered one
injected down/up pair and changed the system to muted. A second run,
`../captures/keyboard-action-AD-20260911-182613/result.json`, delivered one
injected down/up pair while the Rust tray was active and the operator observed
the system mute being released. Both runs recorded zero hardware down/up events
and zero remaining official or Rust tray processes.

The PCAP SHA-256 values are
`4380C41E307480F8EC46FC957FF0A4D44537B2D52A56084DF3317ED44F694E63` and
`BD4B0EA368334C6875A497EB9D3657B26368C5C7A0EBECDCC63D037F7D03CC69`.
This closes the current-release speaker mute toggle check. The separate
microphone mute action uses Core Audio and is documented above.

### Media-player keyboard action — live evidence — 2026-09-11 18:16 JST

The current release was tested with `VK_LAUNCH_MEDIA (0xB5)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-B5-20260911-181606/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`4865D2A76013B3A4CBBDEF6B0811C19CF44FC8DD99886D82C35BEAB3A9A559B6`.

This closes the current-release media-player action check. The older B6 capture
labelled 「マイミュート」 belongs to the pre-Core-Audio implementation and is
retained only as historical evidence; it is not used for the microphone-mute
acceptance result.

### Play/pause keyboard action — live evidence — 2026-09-11 18:59 JST

The current release was tested with `VK_MEDIA_PLAY_PAUSE (0xB3)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-B3-20260911-185943/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`A7D1EF4DA9FBF38EEA188661145FCA52B96812C52794FC19E7C560476C262ADB`.

### Stop keyboard action — live evidence — 2026-09-11 19:02 JST

The current release was tested with `VK_MEDIA_STOP (0xB2)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-B2-20260911-190214/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`5435ED602F1D312664AB3EA70BB92215DE50BE3F84FA8C0084F95DA23780FE45`.

### Previous-track keyboard action — live evidence — 2026-09-11 19:05 JST

The current release was tested with `VK_MEDIA_PREV_TRACK (0xB1)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-B1-20260911-190539/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`F9A422982421752FAE5CE9CEE88D423C53F007C81020CDD87D51129D9D43F8BF`.

### Next-track keyboard action — live evidence — 2026-09-11 19:08 JST

The current release was tested with `VK_MEDIA_NEXT_TRACK (0xB0)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-B0-20260911-190826/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`A7B3BB94DC4B65710076F47F884A826C11509BD7DE3B2EA063CA1C6CF5691234`.

### Browser-refresh keyboard action — live evidence — 2026-09-11 19:11 JST

The current release was tested with `VK_BROWSER_REFRESH (0xA8)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-A8-20260911-191102/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`C61C2FA011D45F315B3E567BF98E9C490B11CFBE3DA1E1E858C61223283412B2`.

### Browser-back keyboard action — live evidence — 2026-09-11 19:13 JST

The current release was tested with `VK_BROWSER_BACK (0xA6)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-A6-20260911-191343/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`5F78E1758A0BD6A42D451FD47D45B2E252CCA44DBDC32348F6F397A8FFED182F`.

### Browser-forward keyboard action — live evidence — 2026-09-11 19:16 JST

The current release was tested with `VK_BROWSER_FORWARD (0xA7)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-A7-20260911-191602/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`4515E237F49590F46E90EE69970B30B57F1DCE0704DAA233FD1DC92272C367AE`.

### Browser-stop keyboard action — live evidence — 2026-09-11 19:18 JST

The current release was tested with `VK_BROWSER_STOP (0xA9)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-A9-20260911-191840/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`F99551B77849FC812BC982EC89C7576C417F35923D4AFC3297F9F71A59208E7E`.

### Browser-search keyboard action — live evidence — 2026-09-11 19:21 JST

The current release was tested with `VK_BROWSER_SEARCH (0xAA)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-AA-20260911-192111/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`89B754DE72E524E2979E7F704ACCC3D220104C07C0C12EE370A7907A77FE0D8E`.

### Browser-favorites keyboard action — live evidence — 2026-09-11 19:24 JST

The current release was tested with `VK_BROWSER_FAVORITES (0xAB)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-AB-20260911-192404/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`24160430E23984DA50763ECC8A3B4B7E19A12B1DBE4C3F923C6B1D6883F5C26E`.

### Browser-home keyboard action — live evidence — 2026-09-11 19:27 JST

The current release was tested with `VK_BROWSER_HOME (0xAC)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-AC-20260911-192719/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83` and the
PCAP SHA-256 is
`29F1AF5C7CC7C2A6F3B2C1C94E79845FC2A55BF0766A094799E01D32A48DF69E`.

### Mail keyboard action — live evidence — 2026-09-11 19:32 JST

The current release was tested with `VK_LAUNCH_MAIL (0xB4)` through
`keyboard-action-audit.ps1`. The artifact is
`../captures/keyboard-action-B4-20260911-193222/result.json`; it records
`status=pass`, one injected down/up pair, zero hardware down/up events, and
zero remaining official or Rust tray processes. The tray binary SHA-256 is
`A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83`.
The event-log SHA-256 is
`556C212ECBF0AFA39855FA518BCBF5ECF8270775519FEC3A8863952B28BE3B89`, the
recovery-log SHA-256 is
`E2A776C8204E9D0B59AD5D926C05915B9458499E7FB19C0B8420FE286AC637E2`, and
the PCAP SHA-256 is
`E9900E167C00A73171B08EDC3CD2AEFBD111433EA0215B47081ED71A0C8E6E68`.

### Historical keyboard-class duplicate-delivery suppression diagnostic hook — 2026-09-11

The initial duplicate audit established the failure mode: a SIDE 7 edge from
the MI_01 keyboard-class collection reached the foreground as a hardware
legacy edge and also caused a Rust `SendInput` edge. The first attempted fix
kept the exact target Raw Input registration and added a service-only
low-level keyboard hook. The hook waited for the matching target Raw Input
observation, then tried to suppress only a hardware edge with the same
virtual key, scan code, press/release direction, extended bit, and timestamp
within an 8 ms bounded window. That ordering experiment is retained here as
historical evidence; it was rejected and is no longer the approved relay
path.

In that historical revision, `open_resident_input_device_for_service()`
enabled the hook only for the long-lived resident service, while
`open_resident_input_device()` remained the read-only path used by probes.
The hook used the current module handle and a dedicated message-loop thread;
startup or thread failure was surfaced as an input error so the recovery loop
could reopen the collection. The relay described below is a historical
diagnostic experiment. The current production acceptance boundary is the
official-compatible standard HID path with one user-mode action boundary; the
relay addendum is retained to explain why a usage-wide suppression design is
not promoted.

Static evidence:

| Check | Result |
|---|---|
| Filter regression seam | 5 tests PASS: target edge suppression, ordinary-key pass-through, injected pass-through, stale/mismatched pass-through, independent press/release matching |
| Relay core state machine | 15 tests PASS: target mapping, ordinary-key forwarding, held-route snapshots, self-injection, device removal, forwarded-key cleanup, shutdown/restart, fail-open |
| Windows relay boundary | 11 tests PASS: `RAWKEYBOARD` conversion, scan-code replay/marker, mapped-output cleanup replay, hook gate, probe/active registration, lifecycle cleanup, SIDE 7 self-echo, separate hook-gate opt-in |
| Full MSVC test suite | 277 tests PASS, 0 failed |
| Formatting | `msvc_cargo.bat fmt -- --check` PASS |
| All-target check | `msvc_cargo.bat check --all-targets` PASS |
| Release build | `msvc_cargo.bat build --release --all-targets` PASS |
| Current tray binary | `target/release/redsamurai-config.exe`, SHA-256 `98E3D02AF99A70BA7703414CE630002A9AC5E1C8B93057AD99C466A8C5D4CCBB` |
| Current probe binary | `target/release/examples/live_probe.exe`, SHA-256 `EDC1A5BA2CFAA20CD445D2DCD3517600104FD01D74B2B044DD944E7E6C6878FE` |
| Hook startup/cleanup smoke | `../captures/keyboard-suppression-startup-20260911-195845/recovery.log` records `input_open_ok`; process cleanup left tray count 0 |

The following was the original physical-gate procedure. It has since been
executed and rejected by the ordering evidence in the addendum below. Do not
repeat it as if it were still pending. Run
`../captures/keyboard-suppression-audit.ps1 -ConfirmLive` with the official
processes absent. The run asks for one ordinary A key and one SIDE 7 edge in
Notepad, then stores the foreground clipboard and the low-level event log. A
PASS requires at least one Raw Input press/release for SIDE 7, zero hardware
VK `0x31` down/up events, exactly one injected VK `0x31` down/up pair, at least
one hardware VK `0x41` down/up pair for the ordinary key, and zero remaining
official or Rust tray processes. Until that result is captured, V-10/V-11
remain partial even though the implementation and static regression gates are
green.

### Keyboard suppression physical gate — rejected by ordering — 2026-09-11 20:09 JST

The current release was run with the audit harness in
`../captures/keyboard-suppression-audit-20260911-200949/`. The result is
**FAIL**, not a pending capture: `target_raw_press/release=1/1`,
`target_hardware_vk_0x31_down/up=1/1`, and
`target_injected_vk_0x31_down/up=1/1`. The foreground clipboard was `a11`,
while ordinary `A` hardware remained `2/2`; official and Rust tray processes
were both zero after cleanup. The result and hashes are in `result.json` and
the supporting assessment is `assessment.md`.

The suppression trace shows the decisive order for the same target press:

```text
legacy_classify ... vk=0x31 ... decision=PassWaitTimeout pending=0
raw_observe ... vk=0x31 ... pending_before=0 pending_after=1
legacy_classify ... vk=0x31 ... injected=true decision=Pass pending=1
```

The release follows the same order. The low-level callback has to return before
the target `WM_INPUT` observation is delivered, so a user-mode correlation
queue cannot identify this device in time to block the legacy edge. The
`KBDLLHOOKSTRUCT` callback has no Raw Input device handle, and `RIDEV_NOLEGACY`
selects a usage class rather than this VID/PID/MI_01 collection. The previous
hook implementation is therefore a diagnostic experiment only and is now
opt-in with `REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL=1`; normal
service startup remains fail-open and does not add the wait to ordinary keys.

At the time of this capture, INPUT-03/V-10 duplicate suppression remained
blocked pending selection of a device-boundary solution: a kernel HID/keyboard
filter, or an explicitly accepted all-keyboard relay with its focus, integrity,
layout, and crash-recovery risks. The decision and current status are updated
in the addendum below; no further repetition of the same low-level-hook audit
is required.

## Historical all-keyboard relay decision and physical-gate addendum — 2026-09-11

The preceding hook audit remains the authoritative negative result for the
device-specific `WH_KEYBOARD_LL` experiment. At the time, the design decision
approved an all-keyboard Raw Input relay as a candidate user-mode path and
deferred a kernel HID/keyboard filter. The 2026-09-12 official-compatible
decision supersedes that production choice; this section remains as historical
relay evidence and diagnosis.
The current release keeps that path behind an explicit opt-in switch, but no
`RIDEV_NOLEGACY` registration has passed physical acceptance and the
current OEM/vendor runtime has not been verified. This is diagnostic evidence
only; the official-compatible production path does not depend on this relay.

The detailed design is in
[`docs/keyboard-relay-design.md`](docs/keyboard-relay-design.md). It is part
of this verification contract and is the source of the relay's pipeline,
marker, recovery, scope, and gate definitions.

### Historical relay runtime versus current production boundary

The current release remains the safe default; the relay implementation is an
explicit diagnostic opt-in. It is retained for comparison and is not the
official-compatible production path:

- Normal startup uses the exact target `MI_01` Raw Input path and does not
  register global `RIDEV_NOLEGACY`. It does not intentionally block ordinary
  keyboard legacy delivery.
- `REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL=1` enables only the
  existing diagnostic low-level correlation hook. It is not an all-keyboard
  relay switch and its physical suppression result is already **FAIL** in
  [`keyboard-suppression-audit-20260911-200949/result.json`](../captures/keyboard-suppression-audit-20260911-200949/result.json).
- The `keyboard_relay` and `keyboard_relay_windows` modules are wired into the
  resident service when `REDSAMURAI_ENABLE_KEYBOARD_RELAY=1` is present. The
  service first installs a legacy-safe keyboard probe registration
  (`RIDEV_INPUTSINK | RIDEV_DEVNOTIFY`, `0x2100`). After the Raw Input window
  parses a complete same-device ordinary A-key (`VK=0x41`, scan `0x1E`) down/up arm pair,
  it switches to the usage-wide
  `RIDEV_INPUTSINK | RIDEV_NOLEGACY | RIDEV_DEVNOTIFY` registration (`0x2130`),
  classifies the exact target path from `hDevice`, replays ordinary keyboard
  edges with the fixed marker, and unregisters on stop/failure. The default
  relay mode does not install or enable a low-level suppression hook; that
  gate requires the separate `REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1` switch.
  The arm pair remains legacy-visible and is not replayed; replay echoes that
  lack a resolvable Raw Input device identity are consumed from a short-lived
  ledger, while unmatched identity failures restore the `0x2100` probe.
  Target action keyboard output is marked while the relay switch is set. The
  switch remains opt-in under the current decision; the physical gates below
  are diagnostic comparison criteria only.
- Relay lifecycle and per-event records are appended to
  `REDSAMURAI_RECOVERY_LOG`: hook readiness, registration/gate state, target
  and ordinary forwarding, arrival/removal, failures, cleanup, and marked
  releases. `open_resident_input_device()` remains the read-only probe path and
  never enables the relay.

An independent `WH_KEYBOARD_LL` watcher can record a physical edge. In the
default registration-only relay mode it is observation only; it does not
represent foreground delivery. For the physical gate, pair that observation
with foreground text/clipboard cardinality and the relay recovery log; the
foreground result determines whether a legacy edge was delivered.

### Approved pipeline and gating contract

The relay's source of truth is `WM_INPUT`/`RAWINPUT`, not the low-level hook:

1. Register keyboard usage input only in the explicitly opted-in validation
   mode, starting with the legacy-safe probe registration. Gate
   `RIDEV_NOLEGACY` behind a successfully parsed first `WM_INPUT`; if that
   proof never arrives, leave legacy delivery enabled and fail open. Keep the
   low-level suppression hook disabled unless the separate experimental gate
   switch is explicitly requested.
2. Read `hDevice` and the device name, then classify the event as the exact
   target (`04D9:FC55`, `MI_01`, interface 1, Usage Page `0x0001`, Usage
   `0x0006`, observed nine-byte shape), non-target, or unknown. The
   configuration collection `MI_02&COL02` is never a source.
3. For a mapped target edge, run the existing parse/debounce/profile resolver
   and emit one semantic action. For a non-target edge, preserve the
   available virtual key, scan code, up/down direction, extended flags, and
   repeat behavior and forward it with `SendInput`. Unknown or malformed input
   must not be silently discarded.
4. Tag every relay-generated keyboard `KEYBDINPUT` with a fixed nonzero
   `REDSAMURAI_SELF_INJECT_MARKER` in `dwExtraInfo`. The low-level hook may
   treat an event as self-generated only when both `LLKHF_INJECTED` and the
   exact marker are present; markerless injected events are external and are
   passed without re-relaying.
5. If the experimental hook gate is enabled, `WH_KEYBOARD_LL` is a
   readiness/health and loop-prevention gate, not a target-device classifier.
   It has no Raw Input device handle and must not wait for a later `WM_INPUT`
   callback. When relay state is absent, unhealthy, or tearing down, it calls
   the next hook. The default registration-only relay has no hook gate.

The new `keyboard_relay_windows` boundary constructs marked `KEYBDINPUT`
records and exposes `send_replay_input`; the resident tray now calls that
boundary only in the explicit opt-in mode. This distinction keeps the default
startup free of global legacy suppression while making the physical relay
validation reproducible. None of these relay details changes the
official-compatible production acceptance boundary.

### Key-state and fail-open verification contract

The relay must keep per-device physical state separate from relay-owned output
state. It must track successful marked down records (including modifiers),
release them on the matching up edge, and reset/debounce target state on
device removal, Raw Input error, profile switch, stop, and reconnect. A failed
`SendInput` down may have reached Windows, so recovery must attempt marked
key-up for the affected key and modifiers before retrying or disabling the
relay; state must not be cleared merely because an API call returned an error.

Fail-open has a startup and a runtime side:

- Before registration, no opt-in means no `RIDEV_NOLEGACY`; incomplete hook,
  classifier, marker, or output readiness means no global gate.
- After registration, unknown device identity, malformed input, queue overflow,
  hook failure, marker/translation ambiguity, `SendInput` rejection, UIPI
  failure, or unrecoverable held state must stop new consumption, unregister or
  replace the global no-legacy registration where possible, and log the
  transition. The current event may already be unrecoverable; that residual
  loss window is a measured failure, not a pass.

The scope is the current interactive logon session and input desktop. The
relay must not claim coverage of secure/UAC or Winlogon desktops, Ctrl+Alt+Del,
session 0, or other boundaries outside the process. `SendInput` remains
subject to UIPI; a lower-integrity process may be unable to inject into a
higher-integrity foreground window, and a zero return does not by itself prove
which boundary rejected it. Japanese/English layout behavior, AltGr, dead
keys, IME, autorepeat, virtual keyboard devices, and OEM/vendor behavior remain
diagnostic questions; they do not block the official-compatible product path.

### Historical relay diagnostic gates (relay remains opt-in)

These criteria are retained for comparison and failure analysis; they are not
release requirements under the official-compatible decision. If a diagnostic
run is performed, static tests, the existing 8-ms hook trace, and an ordinary
process-exit code are insufficient. Each run must capture the opt-in setting,
SHA-256, device inventory/path, active layout/IME, foreground process and
integrity, raw/forwarded/action/injected counters, marker values, timestamps,
and cleanup status. A target edge must be generated physically, never with
`SendInput`.

| Gate | PASS evidence required | Immediate failure/stop |
| --- | --- | --- |
| V-REL-01 startup boundary | Default run has no global `RIDEV_NOLEGACY`; opt-in run starts with a legacy-safe `0x2100` probe and records active `0x2130` registration only after the first parsed `WM_INPUT` plus classifier/output readiness. Hook readiness is required only when the separate experimental gate is enabled. | Registration without explicit opt-in, probe failure that still blocks legacy input, or incomplete readiness. |
| V-REL-02 ordinary keyboard fidelity | A non-target keyboard's text and down/up/repeat trace survive relay with no duplicate or material reorder. | Lost, duplicated, reordered, or stuck ordinary key. |
| V-REL-03 target exactly-once | Physical SIDE 7 and then assigned target set show Raw Input press/release, zero original target legacy edges, and exactly one expected action. | Co-delivered target hardware edge, duplicate action, or wrong source classification. |
| V-REL-04 interleaving | Target and non-target keyboards exercised rapidly and concurrently without cross-device classification. | Cross-device action, drop, or unbounded delay. |
| V-REL-05 marker/loop | Relay-generated records show `LLKHF_INJECTED` plus exact marker; when the optional hook is enabled they pass once through it and never re-enter the relay. Markerless external injections are not re-forwarded. | Missing/wrong marker, recursion, or marker-only ownership. |
| V-REL-06 state/recovery | Normal release, target unplug/reconnect, relay stop/restart, and forced output failure leave no held key, modifier, or mouse button; marked cleanup is logged. | Stuck state, leaked modifier, retry loop, or premature state clearing. |
| V-REL-07 fail-open | Startup, queue, translation, `SendInput`, and UIPI failure paths remove/restore the global no-legacy registration for subsequent input and preserve a failure artifact; add hook failure only in optional hook mode. | Continued suppression after health failure or silent loss. |
| V-REL-08 layout/integrity | Relevant Japanese/English layouts and modifier/AltGr/dead-key/IME cases are exercised at same integrity and elevated foregrounds, with blocked cases explicitly labeled. | Layout corruption, unexplained injection claim, or no UIPI result. |
| V-REL-09 crash/session | Forced termination while keys are held, restart, and in-scope session/desktop transition leave no persistent suppression or stuck output. | Persistent gate, stuck output, or undocumented scope assumption. |
| V-REL-10 cleanup/provenance | Hook/Raw Input registration and processes are cleaned up, logs/hashes are retained, and no OEM runtime behavior is inferred. | Residual process/registration, missing hash, or unsupported OEM claim. |

V-REL-01 through V-REL-10 are bounded diagnostic comparison criteria for the
historical relay experiment. They are not release blockers under the
official-compatible decision. The relay remains opt-in and must not be
promoted to the production path. The remaining V-10/V-11 work is tracked by
the official-compatible action and recovery gates; do not repeat the rejected
same-order 8-ms hook audit.

### 2026-09-11 relay startup fail-open correction

The opt-in physical run `captures/keyboard-duplicate-audit-20260911-221013`
recorded `keyboard_relay_registered ... flags=0x2130` and
`keyboard_relay_gate enabled=true` before any relay event, while the foreground
observer saw a physical key edge. This is a fail-open failure: a usage-wide
`RIDEV_NOLEGACY` registration can remove legacy keyboard delivery even when the
Raw Input worker has not processed an event. The tray was stopped and the
keyboard path was restored by process termination.

The interim build started with a legacy-safe `0x2100` probe and enabled a
low-level gate after switching to active `0x2130`. A follow-up physical run
reproduced a second failure: after the gate became active, the external hook
observer saw SIDE 7 while the relay recorded no subsequent Raw Input. The tray
was stopped again and the keyboard path was restored.

The current build keeps the `0x2100` probe and active `0x2130` registration but
does not install the low-level suppression hook by default. The hook requires
the separate `REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1` diagnostic switch. The
registration-only startup smoke
`captures/relay-registration-only-startup-20260911-225040/result.json` is PASS
with one probe registration, no hook readiness, no gate enable, and zero
remaining official or Rust tray processes. This proves only the safe startup
boundary; active registration-only physical acceptance and V-REL-02 through
V-REL-10 remain pending as diagnostic work. They do not block the
official-compatible production path.

### 2026-09-11 registration-only relay audit — captured, rejected for runtime retry

The first registration-only physical audit after that correction is preserved at
`captures/keyboard-relay-audit-20260911-225926/`. It ended with zero official
and Rust tray processes, but it is **not** a PASS: the worker reopened the
usage-wide registration eight times after `Raw Input device identity was
unavailable`. The first valid probe event was repeatedly promoted to `0x2130`
before that failure, so the final SIDE 7 observation had
`target_raw_down=0`, `target_raw_up=1`, and no target output. The foreground
clipboard remained `1`, but that value cannot establish a SIDE 7 action.

The release implementation was then tightened before the next physical run:

- activation now waits for a complete same-device ordinary A-key down/up pair,
  so Alt/Tab switching cannot arm it, the arm key is not split, and a target
  down edge is not used as the activation event;
- marked `SendInput` replays are retained in a short-lived echo ledger, and an
  identity-less matching Raw Input packet is consumed once instead of being
  mistaken for a new physical source;
- an unmatched identity failure replaces active `0x2130` registration with the
  legacy-safe `0x2100` probe, disables the optional hook gate, clears relay
  state, and keeps the worker alive.

The new hardware-free regression checks are
`probe_arm_requires_the_deliberate_a_pair_from_one_device`,
`relay_probe_arm_key_rejects_window_switching_chords_and_extended_keys`, and
`unknown_relay_device_identity_is_a_probe_fail_open_condition`; the full MSVC
284-test suite passes after the change. The release hashes for the build awaiting the
repeat physical gate are:

| Artifact | SHA-256 |
|---|---|
| `target/release/redsamurai-config.exe` | `C5A0708B2869C821D11D29C50DE8B620C5B502940AA3F4CECB2C4FE1A957B4FA` |
| `target/release/examples/live_probe.exe` | `DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C` |

The next physical run must use the same registration-only audit script and
must show one probe registration, one active registration, a complete SIDE 7
Raw Input down/up pair, one ordinary forwarding pair, clipboard `1`, and zero
remaining processes. A fail-open marker or a second active registration is a
captured runtime failure and must remain in the evidence set.

### Registration-only relay audit follow-up — 2026-09-11

The follow-up run is preserved at
`captures/keyboard-relay-audit-20260911-231958/`. It recorded one probe
registration, one completed arm, one active registration, one SIDE 7 Raw Input
down/up pair, one marked output down/up pair, 45 consumed self-echoes, zero
fail-open events, and zero remaining official or Rust tray processes. The
independent low-level observer also recorded exactly two physical target edges.

The run remains **CAPTURED**, rather than PASS, because the foreground
clipboard was `11`. The relay log and observer show exactly one target source
pair and one output pair; `11` means the editor was not empty before the
post-action copy and cannot be used as a duplicate-delivery proof. Ordinary
forward counts are also repeat-sensitive (`32` down versus `11` up) because
held modifier/repeat samples are logged as additional downs.

The probe boundary is now narrowed to a same-device ordinary A-key pair
(`VK=0x41`, scan `0x1E`). Alt/Tab window-switching events cannot activate the
usage-wide `RIDEV_NOLEGACY` registration, and an unmatched release clears the
pending arm. Hardware-free platform tests cover this identity boundary. Any
future physical relay run is diagnostic comparison only; it is not a required
official-compatible product gate.

The post-hardening release build passed `msvc_cargo.bat test --all --
--test-threads=1` with 284 tests and passed the release build. Its artifacts are
`target/release/redsamurai-config.exe` (SHA-256
`C5A0708B2869C821D11D29C50DE8B620C5B502940AA3F4CECB2C4FE1A957B4FA`) and
`target/release/examples/live_probe.exe` (SHA-256
`DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C`).

### Source rebuild before the official hook seam — 2026-09-12

After the official-compatible documentation decision, the shared tree was
rebuilt without starting a vendor or Rust process. `msvc_cargo.bat fmt --all --
--check`, `check --all-targets --no-default-features`,
`test --all -- --test-threads=1` (288 tests), and `build --release` all passed.
The release executable hashes at that point were `EF632292354BEB682718835A5BC04A4AD2AE0DF866341AD97A307D9E584CF6B2`
for `redsamurai-config.exe` and
`DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C` for
`live_probe.exe`. This rebuild updates the current binary reference; it does
not rewrite the hashes attached to earlier physical captures.

The native pass-through regression now includes a table-driven check for all
observed factory SIDE 7..18 usages, including the button-18 `0x33`/`0x34`
device/profile alias. That historical MSVC run therefore contains 288 tests; all
passed with no vendor or Rust tray process running.

### Static release audit — 2026-09-12

The Codex-only release audit is retained at
`../captures/release-static-audit-20260912-020218/result.json` (SHA-256
`3AA4B55D4581EEECE627BFC188E62FCECC328F4D7960B0808A8F1ABA1084FD67`). Its
human-readable summary is
`../captures/release-static-audit-20260912-020218/summary.md` (SHA-256
`53E1198EF397C038E95AEE68D696702E9469C6F839CC08D81C53C998C7CAB294`). It ran
`fmt --all -- --check`, `check --all-targets --no-default-features`,
`test --all -- --test-threads=1`, and `build --release`; all four commands
returned exit code 0. The artifact records the audit-time tray hash
`EF632292354BEB682718835A5BC04A4AD2AE0DF866341AD97A307D9E584CF6B2` and
probe hash
`DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C`, checks
that both hashes are referenced by the current ledger, and shows zero vendor
or Rust tray processes before and after. This closes the static portion of
RELEASE-01; live V-01–V-17 rebinding and operator-only gates remain bounded
by their cited evidence.

### Observation-only duplicate isolation — implementation ready

The relay now has a separate `REDSAMURAI_KEYBOARD_RELAY_OBSERVE_ONLY=1`
validation mode, exposed by `keyboard-relay-audit.ps1 -ObserveOnly`. It keeps
the legacy-safe `0x2100` registration after the A-key arm pair, records Raw
Input target/ordinary edges, and deliberately emits no Rust replay, forwarded
keyboard edge, or resident action. The optional low-level gate is disabled in
this mode. Hardware-free tests cover the explicit switch and the no-delivery
boundary; the physical comparison is recorded below.

### Observation-only duplicate isolation — physical PASS

The live run `captures/keyboard-relay-audit-20260911-234606/result.json` is a
`pass` for the observation-only contract. After one same-device ordinary A-key
arm pair, the process kept the legacy-safe `0x2100` registration and observed
one target Raw Input down/up pair. It emitted zero target output edges and zero
ordinary forwarded edges; the foreground clipboard contained exactly `1`.
`active_registered=0`, `hook_ready=0`, `gate_enabled=0`, `self_echo_ignored=0`,
and `fail_open=0`. Both official and Rust tray process counts were zero after
cleanup. The tray SHA-256 was
`C5A0708B2869C821D11D29C50DE8B620C5B502940AA3F4CECB2C4FE1A957B4FA`, the
USBPcap SHA-256 was
`43BB5917275B568CC0E2F5CA375D61A897A42CF04FDA7176F5815DAB6585DD7D`, the
recovery log SHA-256 was
`BF78B78EE45164CB825027041A4ED008DC8869BBA7BB6FE1A735C0758EBADE8B`, and the
event log SHA-256 was
`3494AD6E2D389974CF907DE8950B3FF73393A8279D497908FB125EFEDBAFF10F`.

The paired normal registration-only run
`captures/keyboard-relay-audit-20260911-233259/result.json` produced clipboard
`11` with one target raw pair and one target replay pair. The observation-only
`1` versus normal `11` comparison therefore attributes the duplicate to the
physical legacy delivery plus Rust replay. It does not prove target suppression
for the normal relay; the relay comparison gate remains closed and diagnostic.
Official-compatible acceptance is tracked separately by the standard HID,
verified Apply, and one-action-per-edge gates.

### Official hook semantic seam and current release audit — 2026-09-12

The Rust Windows boundary now contains a pure classifier for the bounded
`KBHook.dll` observation: hook id `13`, private message `0x8D2`, exact selected
scan/VK matching, the observed 11-entry navigation VK rewrite table, and
`flags & 0x81`. Its typed payload is `message=0x8D2`, `wParam=rewritten VK`,
and `lParam=flags & 0x81`. Negative `nCode`, disabled state, unselected pairs,
and injected events pass through. The classifier performs no hook installation
or message posting; normal startup remains unchanged and the all-keyboard
relay remains diagnostic-only.

The current release was rebuilt after this seam was added. The full static audit
`../captures/release-static-audit-20260912-114635/result.json` (SHA-256
`DD1E4D842950265CACCF37685A5973A36DF919E96279004BAC28ED4A058543AA`) records
`fmt`, all-target check, all **292 tests**, and release build as exit code 0,
with zero official/Rust processes before and after. The current tray SHA-256 is
`6557F26013404C565271E366E6EA4476DA81BEA6B685BE40C1021BD701DB2F6C`; the
probe SHA remains
`DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C`.

### Release closure — 2026-09-12

The official-compatible product scope is now closed at **17/17 acceptance gates
(100%)**. This section supersedes historical `PARTIAL`/`PENDING` labels above;
those labels describe earlier runs and are retained for audit history. The current
scope is standard Windows HID, the reviewed 156-report Apply sequence, Rust-owned
tray/recovery, native keyboard pass-through, bounded user-mode actions, disposable
macro/combo flows, and tray/logon cleanup.

| Evidence | Result |
| --- | --- |
| `captures/release-static-audit-20260912-125149/result.json` | PASS; fmt, check, 295 tests, final binary hash and manifest checks; zero tracked processes after |
| `captures/release-acceptance-tests-20260912-120307/result.json` | PASS; disposable macro record/edit/save/load/playback, combo commit/cancel, tray/autostart contracts |
| `captures/macro-manager-ui-live-20260912-120216/result.json` | PASS; current release UI open/inspect/cancel and all macro AutomationIds |
| `captures/release-acceptance-bundle-20260912-122423/result.json` | PASS; product gate bundle and cleanup |
| `captures/native-keyboard-pass-through-20260912-113532/result.json` | PASS; SIDE 7 native 1/1, injected 0/0, foreground exact |

The machine-readable bundle is the release decision record. Its result SHA-256 is
`04C50CD9C406D9E2E359F465962B6305F2E9E413661DA21E25C43C0277281411` and its
report SHA-256 is `1DC33C28E920105C655908248CF77A8CE3715BFD7E6285724E4066A32A792920`.

The following remain explicit non-blocking `DEFERRED-BY-DESIGN` extensions: the
Rust-only full Report-03 reconnect context, a kernel filter or test-signed driver,
active `RIDEV_NOLEGACY` all-keyboard relay suppression, and unverified P2 wire
mappings. No standalone or guessed HID write is introduced for them. Each reopens
only after a reviewed complete Apply sequence, zero-status completion, reconnect
readback, and current-SHA evidence are captured in one controlled run.\n
