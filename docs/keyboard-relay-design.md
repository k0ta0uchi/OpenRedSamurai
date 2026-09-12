# All-keyboard relay design note

Status: **diagnostic design retained; superseded for production by official-compatible user-mode path**

Decision date: 2026-09-12 (Asia/Tokyo)
Scope: the Windows resident-input path only

> The project now accepts reproduction of the installed vendor software's
> user-mode method as the product target. This all-keyboard relay remains a
> comparison and failure-analysis harness. It is not enabled by default and
> is not a release gate. The product must use the standard Windows HID stack
> and a single, exactly-once user-mode action boundary.

This note records the previously approved user-mode relay experiment for
preventing duplicate delivery from the mouse's keyboard-class (`MI_01`)
collection. The source contains a platform-neutral relay state machine and a
Windows replay/lifecycle boundary, but the relay is retained only for
comparison and diagnosis. The product target is the installed vendor method:
standard Windows HID delivery plus a user-mode hook/action boundary where a
software assignment is required. The all-keyboard registration and diagnostic
`WH_KEYBOARD_LL` gate remain explicit opt-ins.

## Decision and current state

The project previously approved an **all-keyboard Raw Input relay** as a
candidate user-mode path. Registration with `RIDEV_NOLEGACY` is necessarily
usage-wide: Windows does not provide a `VID/PID/MI` selector for that flag.
The candidate therefore receives all keyboard-class input, classifies the
source, and re-injects non-target events. This behavior remains useful for
diagnosis, but it is no longer the production acceptance path.

The project does not approve a kernel HID/keyboard filter in this scope. A
kernel filter remains a separate future scope only; the official-compatible
production plan does not install, sign, or ship one. The existing
device-specific low-level-hook experiment is not the approved solution: its
ordering failure is captured in
[`keyboard-suppression-audit-20260911-200949/assessment.md`](../../captures/keyboard-suppression-audit-20260911-200949/assessment.md).

The following status is normative for the diagnostic relay record; it does not
change the official-compatible production acceptance boundary:

| Area | Status | Meaning |
| --- | --- | --- |
| Production architecture | **Official-compatible** | Standard Windows HID (`usbccgp`/`HidUsb`/`kbdhid`/`mouhid`) and a single user-mode action boundary. |
| Relay architecture | **Diagnostic only** | The all-keyboard relay is retained for comparison and failure analysis. |
| Production/default startup | **Disabled** | Normal startup must not register `RIDEV_NOLEGACY` and must not suppress ordinary legacy keyboard input. |
| Relay enablement | **Opt-in only** | The resident service recognizes `REDSAMURAI_ENABLE_KEYBOARD_RELAY=1` and installs the usage-wide registration only after startup readiness. Low-level hook suppression requires the separate `REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1` switch and is disabled by default. |
| Physical acceptance | **Diagnostic pending** | Relay comparison runs may be added for diagnosis; release promotion is judged by the official-compatible standard HID/action gates. |
| OEM/vendor runtime behavior | **Bounded** | Installed-binary inspection found user-mode hidapi and hook helpers; no custom kernel filter was found in the observed PnP stacks. |

The existing experimental switch
`REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL=1` is not the relay
switch. It enables only the current diagnostic hook path and must not be
described as production duplicate suppression.

## Exact target and relay scope

The target classification remains bounded to the observed resident collection:

- VID `04D9`, PID `FC55`;
- composite interface `MI_01`, interface number `1`, with the HID path's `KBD`
  identity;
- Usage Page `0x0001`, Usage `0x0006` (keyboard);
- observed nine-byte input shape, including the report-ID byte; and
- the exact Raw Input device path after only the documented `\\KBD` and
  keyboard/HID class-GUID normalization.

The configuration collection (`MI_02&COL02`, Usage Page `0xFFA0`) is never a
relay source. A VID/PID match by itself is insufficient. A missing, ambiguous,
or changed device path is an unknown source and must not be classified as the
target.

The all-keyboard registration also covers other physical keyboards, virtual
keyboard-class devices, and any other keyboard collection visible to the
interactive registration scope. It is not a device-specific filter. This
global scope is the central trade-off accepted by the design decision and must
be visible in the opt-in warning and in every physical report.

## Pipeline: Raw Input -> classification -> SendInput

The relay pipeline is ordered and source-aware:

```text
WM_INPUT / RAWINPUT
    -> obtain hDevice and exact device name
    -> classify target MI_01 vs non-target vs unknown
    -> parse RAWKEYBOARD and preserve key/up/down, scan code, and extended state
    -> target: debounce + profile resolution + semantic action
       non-target: construct an equivalent keyboard INPUT for forwarding
    -> SendInput with the relay self-injection marker
    -> optional low-level gate observes/pass-through of the resulting injected event
```

### Target event

For a target event that maps to an enabled profile assignment, the original
keyboard-class edge is consumed by the relay and exactly one semantic action
is emitted. A keyboard action may result in one or more marked `SendInput`
records; mouse and media actions use their existing semantic output seams. The
physical target edge itself must never be used as proof of the resulting
software action—both streams must be recorded. A separate low-level hook
observer may still see that physical edge; in the optional hook-gate mode it
can see the edge before suppression. Foreground text or clipboard cardinality
is the evidence for whether legacy delivery actually occurred.

If the target path, report shape, virtual-key mapping, profile assignment, or
action output is unknown or invalid, the relay must take its fail-open path.
It must not silently discard the source event merely because it was seen on a
target-looking path.

### Non-target event

Every non-target keyboard event captured by `RIDEV_NOLEGACY` must be forwarded
as an equivalent `SendInput` event. The forwarding record must preserve the
key identity and edge semantics that are available at the Raw Input boundary:
virtual key, scan code, key-down/key-up direction, extended `E0/E1` state, and
repeat behavior. The relay must not translate ordinary typing into clipboard
text or a character string.

An event with no reliable device identity is not safe to classify as target.
It must be forwarded or cause the relay to leave the global gate; it must not
be dropped. A forwarding failure is a relay-health failure, not a successful
suppression result.

### Output transaction

The relay must not claim success from an attempted call. It must record the
`SendInput` return count and Win32 error context, where available, and keep
separate counters for physical Raw Input, forwarded non-target events,
target-resolved actions, and marked self-injected events. A successful target
action is an exactly-once claim only when the physical gate observes the
expected source edge and one corresponding action without a second hardware
legacy edge.

## `WH_KEYBOARD_LL` gating

`WH_KEYBOARD_LL` has no Raw Input device handle, so it cannot safely decide
which physical keyboard produced an event. Earlier runs showed that enabling a
global gate while the Raw Input path was still being established can stop all
keyboard input. The production relay therefore leaves the hook gate disabled
and relies on `RIDEV_NOLEGACY` plus Raw Input classification/replay after the
probe boundary. This keeps hook suppression out of the normal opt-in path.

The gate remains available only for isolated diagnostics through
`REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1`. In that mode it is experimental:
`CallNextHookEx` is the default, injected events are always passed through,
and a health failure must disable the gate independently of Raw Input progress.
No physical acceptance result from the registration-only relay may be
attributed to the experimental hook.

## Self-injection marker

Every keyboard `KEYBDINPUT` generated by the relay—both non-target forwarding
and target action output—must carry a fixed, reserved, nonzero
`dwExtraInfo` value named `REDSAMURAI_SELF_INJECT_MARKER` (the exact literal
must be fixed and recorded with the implementation build). The low-level gate
must require **both** `LLKHF_INJECTED` and an exact marker match before calling
an event self-generated. A marker match without the injected flag is not
trusted.

The marker is a loop-prevention and accounting tag, not an authorization
token, a device identity, or a guarantee that another process cannot copy the
value. It must never be used to classify a physical target event. Markerless
injected events remain external input and are passed without being relayed.

The platform boundary constructs `KEYBDINPUT` records with the fixed marker
and exposes `send_replay_input` for the final Win32 call. The opt-in resident
path now uses that boundary for ordinary-key forwarding, target classification,
device-arrival/removal handling, and marked cleanup. Keyboard actions emitted
by the resident service also carry the marker while the opt-in switch is set.
The switch remains validation-only until the physical gates below pass.

The runtime records probe registration,
`keyboard_relay_probe_event`, `keyboard_relay_probe_arm_complete`, active `keyboard_relay_registered`,
optional `keyboard_relay_hook_ready`/`keyboard_relay_gate`,
`keyboard_relay_forward`, `keyboard_relay_target`, `keyboard_relay_self_echo_ignored`,
`keyboard_relay_output`, device-arrival/removal, failure, and cleanup events
in `REDSAMURAI_RECOVERY_LOG` when that path is provided. The output event
records the marked keyboard/media action boundary, while ordinary forwarding
is recorded by `keyboard_relay_forward`. Startup uses probe flags `0x2100`
(`RIDEV_INPUTSINK | RIDEV_DEVNOTIFY`); only after the deliberately requested
ordinary A-key (`VK=0x41`, scan `0x1E`) down/up pair from the same device is
parsed does the window switch to active flags `0x2130`
(`RIDEV_INPUTSINK | RIDEV_NOLEGACY | RIDEV_DEVNOTIFY`). The default
registration-only mode does not enable a low-level gate; the arm pair remains
legacy-visible and is never replayed. Replays are kept in a short-lived
self-echo ledger so a `SendInput` event that returns as a Raw Input packet
without an `hDevice` is ignored once, while an unmatched identity failure
immediately replaces `0x2130` with the legacy-safe `0x2100` probe. Cleanup uses
`RIDEV_REMOVE` with a null target window.

For transport diagnosis, `REDSAMURAI_KEYBOARD_RELAY_OBSERVE_ONLY=1` is a
separate validation switch used together with the relay opt-in. It keeps the
legacy-safe `0x2100` registration after the same A-key arm pair, records target
and ordinary Raw Input edges, and emits no `SendInput`, forwarded keyboard
edge, or resident action. It also disables the optional low-level gate. This
mode answers whether Raw Input arrives without creating a second foreground
event; it is not a usable action mode.

## Key-state ownership and recovery

The relay must keep physical source state separate from relay-owned output
state:

- Track the source key identity and edge state per Raw Input device. Repeated
  down samples may be forwarded as keyboard repeat for non-target devices, but
  they must not create extra target actions after debounce/classification.
- Track every successful relay-owned down record (including modifiers) and
  pair it with the corresponding marked up record. Do not infer ownership
  from `GetAsyncKeyState`, foreground text, or a low-level event without the
  marker.
- On a normal target release, release the action state exactly once. On a
  target device removal, Raw Input/message-loop error, profile switch, relay
  stop, or reconnect boundary, reset the target runtime and release all
  relay-owned held keyboard and mouse outputs before retrying.
- If a `SendInput` call fails after a down may have reached Windows, perform a
  best-effort marked key-up for the affected key and all tracked modifiers.
  Retain state when a release has not been accepted so cleanup can retry; do
  not silently clear a possibly held key.
- On relay teardown, unregister the all-keyboard Raw Input registration, stop
  the optional hook if one was installed, and run the marked release cleanup. The crash path must be tested
  separately; this note does not claim that an ungraceful process termination
  automatically restores every foreground application's state.

If recovery cannot prove that relay-owned state is released, the relay is
unhealthy. It must remove or replace the global no-legacy registration and
disable the optional hook gate for subsequent input, then record the unresolved
held-state condition. A failure to restore the current event is an explicit
acceptance failure, not permission to continue silently.

## Fail-open contract

Fail-open means that an uncertain or unhealthy relay does not intentionally
block user keyboard input. The contract has two stages:

1. **Before global registration:** do not enable `RIDEV_NOLEGACY` unless the
   relay is fully initialized, its opt-in was explicitly requested, and a
   complete ordinary A-key down/up arm pair from one device has already been
   parsed. The probe
   registration keeps the ordinary legacy path active while that proof is
   pending.
2. **After global registration:** for an unknown source, malformed or
   unclassifiable Raw Input, queue overflow, hook startup/health failure,
   marker/translation ambiguity, `SendInput` rejection, UIPI boundary, or
   recovery failure, stop consuming new events, unregister or replace the
   global no-legacy registration, disable the optional hook gate, and
   resume/pass through the legacy path where Windows permits it. Log the
   transition and preserve enough state for marked cleanup.

The fallback cannot retroactively recreate an event that was already consumed
and lost before the gate was removed. That residual loss window is an
unresolved risk and must be measured in the failure-injection and crash gates;
it must not be hidden by a green process-exit status. An ordinary key must
never be held waiting for a target-device correlation timeout.

## Scope and UIPI limits

The design is intentionally limited to a user-mode process in the current
interactive logon session and input desktop. It does not promise coverage of
Winlogon, Ctrl+Alt+Delete, secure/UAC desktops, session 0, remote desktop
transitions, or other input boundaries that the process cannot observe or
inject into.

`SendInput` is subject to Windows integrity/UIPI rules. A lower-integrity relay
may be unable to inject into a higher-integrity foreground window; Windows may
return zero without exposing a diagnostic that identifies UIPI as the cause.
The relay must report the failed injection and fail open for subsequent input;
it must not claim universal foreground delivery. Physical acceptance must
include same-integrity and elevated-foreground runs, plus an explicit blocked
or unsupported result where UIPI prevents injection.

The relay must preserve scan-code and extended-key information for ordinary
keyboard forwarding, but keyboard layouts, AltGr, dead keys, IME composition,
system shortcuts, autorepeat, and virtual keyboard devices remain behavioral
risks until physical validation. No OEM-specific layout, driver, firmware,
LED, or vendor-runtime behavior is inferred from the attached mouse or from
the existing captures.

## Physical acceptance gates

Static tests and the existing hook trace are prerequisites only. They do not
close these gates. Each run must record the relay switch, exact executable
hash, target path, keyboard device inventory, active layout/IME, foreground
process and integrity level, Raw Input/hook/send counters, marker values,
timestamps, and cleanup hashes. Use physical key/button actions for target
edges; do not use `SendInput` to generate the event under test.

| Gate | Required observation | Stop/fail condition |
| --- | --- | --- |
| R-01 opt-in boundary | Default startup has no all-keyboard `RIDEV_NOLEGACY`; explicit opt-in records registration readiness. If the separate hook-gate switch is used, hook readiness is recorded independently. | Global registration occurs without opt-in, or readiness is incomplete. |
| R-02 ordinary-key fidelity | At least one non-target keyboard produces text and down/up counts that match the Raw Input trace while the relay is active. | Any ordinary key is lost, duplicated, reordered materially, or held. |
| R-03 target exactly-once | Physical SIDE 7 (then the complete assigned factory set) gives target Raw Input press/release, zero original target legacy delivery, and one expected mapped action. | Hardware and action edges co-deliver, target action duplicates, or source is not exact. |
| R-04 interleaving | Target and ordinary keyboards are exercised interleaved and concurrently, including rapid edges. | Classification crosses devices or ordinary events are delayed/dropped. |
| R-05 marker/loop prevention | Every relay-generated keyboard event has `LLKHF_INJECTED` plus the exact marker; when the optional hook is enabled it passes once and does not re-enter the relay. Markerless external injection passes without being re-forwarded. | Missing/wrong marker, recursive relay, or marker-only ownership decision. |
| R-06 repeat/debounce | Ordinary keyboard repeat remains usable; target repeated samples produce only the specified debounced action count. | Repeat is lost for non-target keys or target actions multiply. |
| R-07 held-state recovery | Hold target and ordinary keys, release normally, unplug target, stop/restart relay, and inject a forced `SendInput` failure; all relay-owned outputs receive marked key-up cleanup. | Any stuck key/button, leaked modifier, unbounded retry, or state cleared before release. |
| R-08 failure/open recovery | Exercise Raw Input startup failure, queue/translation error, and output rejection; when the optional hook is enabled also exercise hook failure. The global no-legacy registration and optional hook gate are removed/restored and the transition is logged. | Relay continues suppressing after health failure or silently loses input. |
| R-09 layout and integrity | Repeat R-02/R-03 with the supported Japanese/English layouts as applicable, modifiers/AltGr/dead keys/IME, same-integrity and elevated foregrounds. | Layout corruption, modifier loss, unexplained UIPI claim, or no explicit unsupported result. |
| R-10 crash/session boundary | Terminate the relay while keys are held, restart it, and test session/desktop changes within the declared scope. | Stuck output, persistent global suppression, or an undocumented desktop/session assumption. |
| R-11 cleanup/provenance | After every run, the relay/optional hook/Raw Input registration is gone or intentionally reported, process count is zero, logs and hashes are saved, and no vendor/OEM behavior is inferred. | Residual process/registration, missing artifact/hash, or unsupported OEM-runtime claim. |

R-01 through R-11 remain bounded diagnostic comparison criteria. The relay
stays opt-in regardless of their status, and the current physical failure at
[`keyboard-suppression-audit-20260911-200949/result.json`](../../captures/keyboard-suppression-audit-20260911-200949/result.json)
is evidence for the design change, not evidence that any relay gate passes.
It does not block the official-compatible production path.

## Supersession note — 2026-09-12

The R-01 through R-11 table describes the historical relay experiment. The
official-compatible product path does not require these relay gates. Product
completion is instead judged by the standard HID stack, verified official
configuration traffic, and one user-mode action per physical edge.

## Related implementation boundaries

- Current Raw Input and `WH_KEYBOARD_LL` diagnostic boundary:
  [`src/resident_platform.rs`](../src/resident_platform.rs)
- Platform-neutral relay state machine:
  [`src/keyboard_relay.rs`](../src/keyboard_relay.rs)
- Windows packet/replay/lifecycle boundary:
  [`src/keyboard_relay_windows.rs`](../src/keyboard_relay_windows.rs)
- Current semantic resolver and held-state reset boundary:
  [`src/resident.rs`](../src/resident.rs)
- Current recovery worker and cleanup boundary:
  [`src/resident_service.rs`](../src/resident_service.rs)
- Verification ledger and physical gate status:
  [`../VERIFICATION.md`](../VERIFICATION.md)
- Roadmap decision and next work:
  [`../ROADMAP.md`](../ROADMAP.md)
