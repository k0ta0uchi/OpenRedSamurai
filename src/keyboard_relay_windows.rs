//! Opt-in Windows all-keyboard relay boundary.
//!
//! This module is intentionally a small, platform-safe seam around the
//! platform-independent [`crate::keyboard_relay`] state machine.  It owns the
//! contract for translating `RAWKEYBOARD` fields, constructing marked
//! scan-code replays, classifying low-level hook records, and ordering relay
//! lifecycle transitions.
//!
//! The resident path uses this module only when the explicit
//! [`RELAY_OPT_IN_ENV`] switch is enabled. Linking the module alone still
//! performs no Raw Input registration, low-level hook installation, or
//! `SendInput` call. The runtime integration keeps the typed lifecycle and
//! cleanup boundary at the platform edge so the default build remains
//! fail-open and the unsafe OS calls stay reviewable in one place.

use std::fmt;

use crate::keyboard_relay::{InjectedKeyEvent, KeyPhase, RawKeyboardSample};
pub use crate::keyboard_relay::{RAW_KEY_BREAK, RAW_KEY_E0, RAW_KEY_E1};

/// Explicit opt-in variable reserved for the all-keyboard relay.
pub const RELAY_OPT_IN_ENV: &str = "REDSAMURAI_ENABLE_KEYBOARD_RELAY";
/// Separate opt-in variable for the experimental low-level suppression gate.
///
/// The Raw Input relay can safely own the usage-wide registration and replay
/// path without installing a low-level hook.  Keeping hook suppression behind
/// a second switch prevents a healthy relay from turning a hook failure into
/// a system-wide keyboard outage.
pub const RELAY_GATE_OPT_IN_ENV: &str = "REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE";
/// Separate validation-only switch for a Raw Input observation run.  This
/// mode intentionally records relay traffic without forwarding or mapping it.
pub const RELAY_OBSERVE_ONLY_ENV: &str = "REDSAMURAI_KEYBOARD_RELAY_OBSERVE_ONLY";

/// Fixed marker attached to every keyboard replay created by this adapter.
///
/// This is a loop-prevention/accounting marker, not a security token or a
/// device identity.  The hook requires both `LLKHF_INJECTED` and this exact
/// value before treating an event as self-generated.
pub const REDSAMURAI_SELF_INJECT_MARKER: u64 = crate::keyboard_relay::DEFAULT_INJECTION_MARKER;

/// Low-level hook flag indicating an injected keyboard event.
pub const LLKHF_INJECTED: u32 = 0x0010;
/// Low-level hook flag indicating lower-integrity injected input.
pub const LLKHF_LOWER_IL_INJECTED: u32 = 0x0002;
/// Low-level hook flag indicating an extended key.
pub const LLKHF_EXTENDED: u32 = 0x0001;
/// Low-level hook flag indicating a key-up edge.
pub const LLKHF_UP: u32 = 0x0080;

/// Keyboard top-level collection used by the all-keyboard Raw Input relay.
pub const KEYBOARD_USAGE_PAGE: u16 = 0x0001;
pub const KEYBOARD_USAGE: u16 = 0x0006;

/// `RIDEV_INPUTSINK | RIDEV_NOLEGACY | RIDEV_DEVNOTIFY`.
///
/// `RIDEV_NOLEGACY` is usage-wide on Windows; it cannot select one VID/PID or
/// composite interface.  The adapter therefore must replay every ordinary
/// keyboard event it consumes.
pub const RAW_INPUT_REGISTER_FLAGS: u32 = 0x0100 | 0x0030 | 0x2000;
/// Safe startup registration used while proving that the Raw Input window is
/// receiving keyboard events.  Legacy delivery remains enabled until the
/// first valid event is parsed and the active relay registration succeeds.
pub const RAW_INPUT_PROBE_REGISTER_FLAGS: u32 = 0x0100 | 0x2000;
/// `RIDEV_REMOVE`, used with a null target window during cleanup.
pub const RAW_INPUT_REMOVE_FLAGS: u32 = 0x0001;

/// Interpret an environment value using the relay's deliberately narrow
/// opt-in vocabulary.
pub fn relay_opt_in_requested_from(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

/// Read the explicit relay opt-in switch.
pub fn relay_opt_in_requested() -> bool {
    relay_opt_in_requested_from(std::env::var(RELAY_OPT_IN_ENV).ok().as_deref())
}

/// Read the explicit opt-in switch for the experimental low-level gate.
pub fn relay_gate_opt_in_requested_from(value: Option<&str>) -> bool {
    relay_opt_in_requested_from(value)
}

/// Read the explicit low-level gate opt-in switch.
pub fn relay_gate_opt_in_requested() -> bool {
    relay_gate_opt_in_requested_from(std::env::var(RELAY_GATE_OPT_IN_ENV).ok().as_deref())
}

/// Interpret the explicit observation-only switch using the same narrow
/// vocabulary as the relay opt-in flags.
pub fn relay_observe_only_requested_from(value: Option<&str>) -> bool {
    relay_opt_in_requested_from(value)
}

/// Read the explicit observation-only switch.
pub fn relay_observe_only_requested() -> bool {
    relay_observe_only_requested_from(std::env::var(RELAY_OBSERVE_ONLY_ENV).ok().as_deref())
}

/// The fields of a Windows `RAWKEYBOARD` record that are relevant to relay
/// conversion.  Keeping this owned value separate from the Win32 union makes
/// conversion tests hardware-free and keeps pointer reads at the OS edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKeyboardPacket {
    virtual_key: u16,
    make_code: u16,
    flags: u16,
    timestamp_ms: u64,
}

impl RawKeyboardPacket {
    /// Construct a packet from `RAWKEYBOARD::VKey`, `MakeCode`, `Flags`, and a
    /// message timestamp.
    pub const fn new(virtual_key: u16, make_code: u16, flags: u16, timestamp_ms: u64) -> Self {
        Self {
            virtual_key,
            make_code,
            flags,
            timestamp_ms,
        }
    }

    pub const fn virtual_key(self) -> u16 {
        self.virtual_key
    }

    pub const fn make_code(self) -> u16 {
        self.make_code
    }

    pub const fn flags(self) -> u16 {
        self.flags
    }

    pub const fn timestamp_ms(self) -> u64 {
        self.timestamp_ms
    }

    pub const fn is_break(self) -> bool {
        self.flags & RAW_KEY_BREAK != 0
    }

    pub const fn is_extended(self) -> bool {
        self.flags & (RAW_KEY_E0 | RAW_KEY_E1) != 0
    }
}

/// Convert one bounded `RAWKEYBOARD` packet into a relay-core sample.
///
/// The break and E0/E1 bits are carried unchanged, so the core can retain the
/// source key identity across a down/up pair.  `device_id` should be the
/// normalized `RIDI_DEVICENAME` path resolved by the Raw Input worker; this
/// function deliberately does not classify it as target or ordinary.
pub fn raw_keyboard_to_relay_sample(
    device_id: impl Into<crate::keyboard_relay::DeviceId>,
    packet: RawKeyboardPacket,
) -> RawKeyboardSample {
    RawKeyboardSample::new(
        device_id,
        packet.virtual_key,
        packet.make_code,
        packet.flags,
        packet.timestamp_ms,
    )
}

/// Convenience conversion for callers that already copied the Win32 fields.
pub const fn raw_keyboard_packet(
    virtual_key: u16,
    make_code: u16,
    flags: u16,
    timestamp_ms: u64,
) -> RawKeyboardPacket {
    RawKeyboardPacket::new(virtual_key, make_code, flags, timestamp_ms)
}

/// Errors produced while constructing or delivering a replay record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// Neither a virtual key nor a scan code identifies the input.
    MissingKeyIdentity,
    /// The marker cannot be represented by this process's `ULONG_PTR` width.
    MarkerDoesNotFitPointer,
    /// A synthetic output was produced by a different relay marker.
    UnexpectedMarker,
    /// Windows accepted no records from `SendInput`.
    SendInputFailed { code: u32 },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKeyIdentity => {
                formatter.write_str("keyboard replay needs a virtual key or scan code")
            }
            Self::MarkerDoesNotFitPointer => {
                formatter.write_str("keyboard replay marker does not fit ULONG_PTR")
            }
            Self::UnexpectedMarker => {
                formatter.write_str("keyboard replay output carries an unexpected marker")
            }
            Self::SendInputFailed { code } => {
                write!(
                    formatter,
                    "SendInput rejected the keyboard replay (Win32 error {code})"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// Pure representation of a `KEYBDINPUT` keyboard replay.
///
/// When a scan code is present, `virtual_key` is zero and `SCANCODE` is set so
/// Windows receives the physical key identity rather than a layout-translated
/// character.  The original virtual key remains available in the source
/// [`RawKeyboardSample`] for classification and logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardReplay {
    virtual_key: u16,
    scan_code: u16,
    flags: u32,
    extra_info: u64,
}

impl KeyboardReplay {
    /// `KEYEVENTF_EXTENDEDKEY`.
    pub const EXTENDED: u32 = 0x0001;
    /// `KEYEVENTF_KEYUP`.
    pub const KEYUP: u32 = 0x0002;
    /// `KEYEVENTF_SCANCODE`.
    pub const SCANCODE: u32 = 0x0008;

    pub const fn virtual_key(self) -> u16 {
        self.virtual_key
    }

    pub const fn scan_code(self) -> u16 {
        self.scan_code
    }

    pub const fn flags(self) -> u32 {
        self.flags
    }

    pub const fn extra_info(self) -> u64 {
        self.extra_info
    }

    /// Convert the stable marker to the platform's `ULONG_PTR` representation.
    pub fn extra_info_for_pointer(self) -> Result<usize, ReplayError> {
        usize::try_from(self.extra_info).map_err(|_| ReplayError::MarkerDoesNotFitPointer)
    }
}

/// Build a marked scan-code replay using the adapter's fixed marker.
pub fn replay_keyboard_input(sample: &RawKeyboardSample) -> Result<KeyboardReplay, ReplayError> {
    if sample.virtual_key == 0 && sample.scan_code == 0 {
        return Err(ReplayError::MissingKeyIdentity);
    }

    let mut flags = 0;
    let virtual_key = if sample.scan_code == 0 {
        sample.virtual_key
    } else {
        flags |= KeyboardReplay::SCANCODE;
        0
    };
    if sample.flags & (RAW_KEY_E0 | RAW_KEY_E1) != 0 {
        flags |= KeyboardReplay::EXTENDED;
    }
    if sample.flags & RAW_KEY_BREAK != 0 {
        flags |= KeyboardReplay::KEYUP;
    }

    Ok(KeyboardReplay {
        virtual_key,
        scan_code: sample.scan_code,
        flags,
        extra_info: REDSAMURAI_SELF_INJECT_MARKER,
    })
}

/// Build a marked keyboard replay for a mapped relay output edge.
///
/// The platform adapter normally receives [`RelayEvent::Injected`] values
/// only from the same core that uses [`REDSAMURAI_SELF_INJECT_MARKER`].  Keep
/// the marker check explicit so a stale or foreign output cannot be emitted
/// as if this process owned it.
pub fn replay_injected_key(event: InjectedKeyEvent) -> Result<KeyboardReplay, ReplayError> {
    if event.marker() != REDSAMURAI_SELF_INJECT_MARKER {
        return Err(ReplayError::UnexpectedMarker);
    }
    let key = event.key();
    let mut flags = 0;
    if key.is_extended() {
        flags |= KeyboardReplay::EXTENDED;
    }
    if event.phase() == KeyPhase::Up {
        flags |= KeyboardReplay::KEYUP;
    }
    let virtual_key = if key.scan_code() == 0 { key.vk() } else { 0 };
    if virtual_key == 0 && key.scan_code() == 0 {
        return Err(ReplayError::MissingKeyIdentity);
    }
    if key.scan_code() != 0 {
        flags |= KeyboardReplay::SCANCODE;
    }
    Ok(KeyboardReplay {
        virtual_key,
        scan_code: key.scan_code(),
        flags,
        extra_info: REDSAMURAI_SELF_INJECT_MARKER,
    })
}

/// Result of the hook's O(1), no-I/O gate decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookDecision {
    /// Call `CallNextHookEx` and preserve normal delivery.
    PassThrough,
    /// Return a non-zero result for a physical event while the relay is live.
    SuppressPhysical,
}

/// Return true only for an injected event carrying this relay's exact marker.
/// A marker match without `LLKHF_INJECTED` is intentionally not trusted.
pub const fn is_self_injected(flags: u32, extra_info: u64) -> bool {
    flags & LLKHF_INJECTED != 0 && extra_info == REDSAMURAI_SELF_INJECT_MARKER
}

/// Decide the low-level hook result without waiting, locking, or performing
/// any I/O.  Any injected event is passed through; this includes markerless
/// external software input and avoids treating it as a relay source.
pub const fn classify_hook_event(
    flags: u32,
    extra_info: u64,
    relay_ready: bool,
    relay_healthy: bool,
) -> HookDecision {
    let injected = flags & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED) != 0;
    if injected || is_self_injected(flags, extra_info) {
        HookDecision::PassThrough
    } else if relay_ready && relay_healthy {
        HookDecision::SuppressPhysical
    } else {
        HookDecision::PassThrough
    }
}

/// Hook identifier used by the vendor's keyboard helper (`WH_KEYBOARD_LL`).
///
/// The value is recorded as a compatibility fact from the installed helper;
/// declaring it here does not install a hook or enable suppression.
pub const OFFICIAL_KEYBOARD_HOOK_ID: i32 = 13;
/// Private message observed in the vendor keyboard helper.
pub const OFFICIAL_KEYBOARD_HOOK_MESSAGE: u32 = 0x08D2;
/// The vendor helper forwards only the extended and break bits in its private
/// message payload (`flags & 0x81`).
pub const OFFICIAL_KEYBOARD_HOOK_FLAGS_MASK: u32 = LLKHF_EXTENDED | LLKHF_UP;

/// One selected physical scan/VK pair accepted by the vendor-style hook seam.
///
/// A real adapter may construct this value from the active software
/// assignment.  This type intentionally has no device selector: the observed
/// vendor callback is a user-mode global hook and receives no HID path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfficialKeyboardHookMapping {
    virtual_key: u32,
    scan_code: u32,
}

impl OfficialKeyboardHookMapping {
    pub const fn new(virtual_key: u32, scan_code: u32) -> Self {
        Self {
            virtual_key,
            scan_code,
        }
    }

    pub const fn virtual_key(self) -> u32 {
        self.virtual_key
    }

    pub const fn scan_code(self) -> u32 {
        self.scan_code
    }
}

/// Bounded fields copied from one `KBDLLHOOKSTRUCT` callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfficialKeyboardHookEvent {
    n_code: i32,
    virtual_key: u32,
    scan_code: u32,
    flags: u32,
    enabled: bool,
}

impl OfficialKeyboardHookEvent {
    pub const fn new(
        n_code: i32,
        virtual_key: u32,
        scan_code: u32,
        flags: u32,
        enabled: bool,
    ) -> Self {
        Self {
            n_code,
            virtual_key,
            scan_code,
            flags,
            enabled,
        }
    }

    pub const fn n_code(self) -> i32 {
        self.n_code
    }

    pub const fn virtual_key(self) -> u32 {
        self.virtual_key
    }

    pub const fn scan_code(self) -> u32 {
        self.scan_code
    }

    pub const fn flags(self) -> u32 {
        self.flags
    }

    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

/// Result of classifying one event at the vendor-compatible user-mode seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficialKeyboardHookDecision {
    /// Continue through `CallNextHookEx`.
    PassThrough,
    /// Deliver the selected key to the owning window and suppress the legacy
    /// foreground delivery by returning a non-zero hook result.
    PostPrivateMessage {
        virtual_key: u32,
        scan_code: u32,
        flags: u32,
    },
}

/// The bounded `PostMessage` payload emitted by the vendor keyboard helper.
///
/// `KBHook.dll` uses the private message as the message id, the (possibly
/// rewritten) virtual key as `wParam`, and only the extended/key-up bits as
/// `lParam`.  Keeping this payload typed prevents an adapter from accidentally
/// passing the full hook flags or a scan code in the wrong field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfficialKeyboardHookMessage {
    message: u32,
    wparam: u32,
    lparam: u32,
}

impl OfficialKeyboardHookMessage {
    pub const fn message(self) -> u32 {
        self.message
    }

    pub const fn wparam(self) -> u32 {
        self.wparam
    }

    pub const fn lparam(self) -> u32 {
        self.lparam
    }
}

impl OfficialKeyboardHookDecision {
    /// Convert a selected event into the exact private-message payload.
    /// Pass-through events have no message and therefore return `None`.
    pub const fn to_private_message(self) -> Option<OfficialKeyboardHookMessage> {
        match self {
            Self::PassThrough => None,
            Self::PostPrivateMessage {
                virtual_key, flags, ..
            } => Some(OfficialKeyboardHookMessage {
                message: OFFICIAL_KEYBOARD_HOOK_MESSAGE,
                wparam: virtual_key,
                lparam: flags,
            }),
        }
    }
}

/// Apply the small scan/VK rewrite table present in the installed
/// `KBHook.dll`.  The table changes the virtual-key value for the observed
/// navigation-key scan pairs (the helper's numpad normalization); every other
/// selected pair keeps its original virtual key.
pub const fn official_keyboard_hook_virtual_key(virtual_key: u32, scan_code: u32) -> u32 {
    match (virtual_key, scan_code) {
        (0x47, 0x24) => 0x67,
        (0x48, 0x26) => 0x68,
        (0x49, 0x21) => 0x69,
        (0x4B, 0x25) => 0x64,
        (0x4C, 0x0C) => 0x65,
        (0x4D, 0x27) => 0x66,
        (0x4F, 0x23) => 0x61,
        (0x50, 0x28) => 0x62,
        (0x51, 0x22) => 0x63,
        (0x52, 0x2D) => 0x60,
        (0x53, 0x2E) => 0x6E,
        _ => virtual_key,
    }
}

/// Reproduce the bounded decision visible in the installed `KBHook.dll`.
///
/// This is a pure classifier.  It performs no hook installation, message
/// posting, or input injection.  The caller must execute the post/suppress
/// result only from an explicitly approved user-mode adapter.  Injected input
/// is passed through so the product cannot feed its own output back into the
/// selected mapping.
pub const fn classify_official_keyboard_hook(
    event: OfficialKeyboardHookEvent,
    mapping: OfficialKeyboardHookMapping,
) -> OfficialKeyboardHookDecision {
    let injected = event.flags & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED) != 0;
    if event.n_code < 0
        || !event.enabled
        || injected
        || event.virtual_key != mapping.virtual_key
        || event.scan_code != mapping.scan_code
    {
        OfficialKeyboardHookDecision::PassThrough
    } else {
        OfficialKeyboardHookDecision::PostPrivateMessage {
            virtual_key: official_keyboard_hook_virtual_key(event.virtual_key, event.scan_code),
            scan_code: event.scan_code,
            flags: event.flags & OFFICIAL_KEYBOARD_HOOK_FLAGS_MASK,
        }
    }
}

/// Typed description of the all-keyboard Raw Input registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawInputRegistration {
    usage_page: u16,
    usage: u16,
    flags: u32,
    target_window: isize,
}

impl RawInputRegistration {
    /// Build the startup probe registration.  This deliberately omits
    /// `RIDEV_NOLEGACY` so a missing or stalled Raw Input path cannot lock the
    /// user's keyboard.
    pub const fn probe_keyboard(target_window: isize) -> Self {
        Self {
            usage_page: KEYBOARD_USAGE_PAGE,
            usage: KEYBOARD_USAGE,
            flags: RAW_INPUT_PROBE_REGISTER_FLAGS,
            target_window,
        }
    }

    /// Build the usage-wide registration used only after explicit relay opt-in.
    pub const fn all_keyboard(target_window: isize) -> Self {
        Self {
            usage_page: KEYBOARD_USAGE_PAGE,
            usage: KEYBOARD_USAGE,
            flags: RAW_INPUT_REGISTER_FLAGS,
            target_window,
        }
    }

    /// Build the matching removal request.  Windows requires a null target
    /// window when `RIDEV_REMOVE` is present.
    pub const fn remove() -> Self {
        Self {
            usage_page: KEYBOARD_USAGE_PAGE,
            usage: KEYBOARD_USAGE,
            flags: RAW_INPUT_REMOVE_FLAGS,
            target_window: 0,
        }
    }

    pub const fn usage_page(self) -> u16 {
        self.usage_page
    }

    pub const fn usage(self) -> u16 {
        self.usage
    }

    pub const fn flags(self) -> u32 {
        self.flags
    }

    pub const fn target_window(self) -> isize {
        self.target_window
    }

    pub const fn no_legacy(self) -> bool {
        self.flags & 0x0030 != 0
    }

    #[cfg(windows)]
    /// Convert the description into the Windows ABI value at the OS edge.
    pub fn as_windows(self) -> windows::Win32::UI::Input::RAWINPUTDEVICE {
        windows::Win32::UI::Input::RAWINPUTDEVICE {
            usUsagePage: self.usage_page,
            usUsage: self.usage,
            dwFlags: windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS(self.flags),
            hwndTarget: windows::Win32::Foundation::HWND(
                self.target_window as *mut std::ffi::c_void,
            ),
        }
    }
}

/// Lifecycle state for the opt-in relay's global gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayLifecycleState {
    /// No opt-in was requested; ordinary resident behavior remains untouched.
    Disabled,
    /// Opt-in was requested and startup is preparing worker/hook resources.
    Preparing,
    /// Raw Input worker and classifier are ready.
    RawInputReady,
    /// The low-level hook is installed and healthy, but global registration is
    /// not active yet.
    HookReady,
    /// All-keyboard registration is active but the gate has not been enabled.
    Registered,
    /// The relay may suppress physical legacy events.
    Active,
    /// A worker/hook/output failure disabled suppression; cleanup is pending.
    Failed,
    /// Registration, gate, and hook resources have been released.
    Stopped,
}

/// Errors for invalid relay lifecycle ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayLifecycleError {
    OptInRequired,
    InvalidTransition {
        state: RelayLifecycleState,
        operation: &'static str,
    },
}

impl fmt::Display for RelayLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OptInRequired => formatter.write_str("keyboard relay opt-in was not requested"),
            Self::InvalidTransition { state, operation } => {
                write!(formatter, "cannot {operation} while relay is in {state:?}")
            }
        }
    }
}

impl std::error::Error for RelayLifecycleError {}

/// Explicit cleanup actions returned by [`RelayLifecycle::cleanup`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelayCleanup {
    pub gate_disabled: bool,
    pub raw_input_unregistered: bool,
    pub hook_stopped: bool,
}

/// Small state machine that prevents enabling usage-wide suppression before
/// both the Raw Input worker and hook are ready, and makes fail-open cleanup
/// observable to a future OS adapter.
#[derive(Clone, Debug)]
pub struct RelayLifecycle {
    opt_in: bool,
    state: RelayLifecycleState,
    raw_input_ready: bool,
    hook_ready: bool,
    raw_input_registered: bool,
    gate_enabled: bool,
}

impl RelayLifecycle {
    pub const fn new(opt_in: bool) -> Self {
        Self {
            opt_in,
            state: if opt_in {
                RelayLifecycleState::Stopped
            } else {
                RelayLifecycleState::Disabled
            },
            raw_input_ready: false,
            hook_ready: false,
            raw_input_registered: false,
            gate_enabled: false,
        }
    }

    pub const fn is_opted_in(&self) -> bool {
        self.opt_in
    }

    pub const fn state(&self) -> RelayLifecycleState {
        self.state
    }

    pub const fn gate_enabled(&self) -> bool {
        self.gate_enabled
    }

    pub const fn is_raw_input_registered(&self) -> bool {
        self.raw_input_registered
    }

    pub const fn is_hook_ready(&self) -> bool {
        self.hook_ready
    }

    pub const fn begin(&mut self) -> Result<(), RelayLifecycleError> {
        if !self.opt_in {
            return Err(RelayLifecycleError::OptInRequired);
        }
        if !matches!(self.state, RelayLifecycleState::Stopped) {
            return Err(RelayLifecycleError::InvalidTransition {
                state: self.state,
                operation: "begin",
            });
        }
        self.state = RelayLifecycleState::Preparing;
        self.raw_input_ready = false;
        self.hook_ready = false;
        self.raw_input_registered = false;
        self.gate_enabled = false;
        Ok(())
    }

    pub const fn raw_input_ready(&mut self) -> Result<(), RelayLifecycleError> {
        if !matches!(self.state, RelayLifecycleState::Preparing) {
            return Err(RelayLifecycleError::InvalidTransition {
                state: self.state,
                operation: "mark Raw Input ready",
            });
        }
        self.raw_input_ready = true;
        self.state = RelayLifecycleState::RawInputReady;
        Ok(())
    }

    pub const fn hook_ready(&mut self) -> Result<(), RelayLifecycleError> {
        if !matches!(self.state, RelayLifecycleState::RawInputReady) {
            return Err(RelayLifecycleError::InvalidTransition {
                state: self.state,
                operation: "mark hook ready",
            });
        }
        self.hook_ready = true;
        self.state = RelayLifecycleState::HookReady;
        Ok(())
    }

    pub const fn raw_input_registered(&mut self) -> Result<(), RelayLifecycleError> {
        if !matches!(self.state, RelayLifecycleState::HookReady) {
            return Err(RelayLifecycleError::InvalidTransition {
                state: self.state,
                operation: "register Raw Input",
            });
        }
        self.raw_input_registered = true;
        self.state = RelayLifecycleState::Registered;
        Ok(())
    }

    pub const fn activate_gate(&mut self) -> Result<(), RelayLifecycleError> {
        if !self.raw_input_ready || !self.hook_ready || !self.raw_input_registered {
            return Err(RelayLifecycleError::InvalidTransition {
                state: self.state,
                operation: "activate global gate",
            });
        }
        if !matches!(self.state, RelayLifecycleState::Registered) {
            return Err(RelayLifecycleError::InvalidTransition {
                state: self.state,
                operation: "activate global gate",
            });
        }
        self.gate_enabled = true;
        self.state = RelayLifecycleState::Active;
        Ok(())
    }

    /// Disable the gate immediately while retaining resource flags for the
    /// subsequent explicit cleanup call.
    pub const fn fail_open(&mut self) {
        self.gate_enabled = false;
        if !matches!(
            self.state,
            RelayLifecycleState::Disabled | RelayLifecycleState::Stopped
        ) {
            self.state = RelayLifecycleState::Failed;
        }
    }

    pub const fn mark_raw_input_failed(&mut self) {
        self.fail_open();
    }

    pub const fn mark_hook_failed(&mut self) {
        self.fail_open();
    }

    /// Disable suppression, unregister all-keyboard Raw Input, stop the hook,
    /// and make the lifecycle idempotently stopped.  The returned booleans are
    /// the actions an OS adapter must perform; no OS calls occur here.
    pub const fn cleanup(&mut self) -> RelayCleanup {
        let cleanup = RelayCleanup {
            gate_disabled: self.gate_enabled,
            raw_input_unregistered: self.raw_input_registered,
            hook_stopped: self.hook_ready,
        };
        self.gate_enabled = false;
        self.raw_input_registered = false;
        self.raw_input_ready = false;
        self.hook_ready = false;
        if !matches!(self.state, RelayLifecycleState::Disabled) {
            self.state = RelayLifecycleState::Stopped;
        }
        cleanup
    }
}

impl Drop for RelayLifecycle {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Adapter-owned marker and lifecycle seam.  OS registration remains an
/// explicit integration step; this value is useful to callers that need one
/// object to carry the opt-in policy and replay marker together.
#[derive(Clone, Debug)]
pub struct KeyboardRelayWindowsAdapter {
    lifecycle: RelayLifecycle,
    marker: u64,
}

impl KeyboardRelayWindowsAdapter {
    pub const fn new(opt_in: bool) -> Self {
        Self {
            lifecycle: RelayLifecycle::new(opt_in),
            marker: REDSAMURAI_SELF_INJECT_MARKER,
        }
    }

    pub fn from_environment() -> Self {
        Self::new(relay_opt_in_requested())
    }

    pub const fn marker(&self) -> u64 {
        self.marker
    }

    pub const fn lifecycle(&self) -> &RelayLifecycle {
        &self.lifecycle
    }

    pub const fn lifecycle_mut(&mut self) -> &mut RelayLifecycle {
        &mut self.lifecycle
    }

    pub fn replay(&self, sample: &RawKeyboardSample) -> Result<KeyboardReplay, ReplayError> {
        let _ = self.marker;
        replay_keyboard_input(sample)
    }

    pub fn cleanup(&mut self) -> RelayCleanup {
        self.lifecycle.cleanup()
    }
}

#[cfg(windows)]
impl KeyboardReplay {
    /// Convert the pure replay record into the Win32 `INPUT` union.
    pub fn as_windows_input(
        self,
    ) -> Result<windows::Win32::UI::Input::KeyboardAndMouse::INPUT, ReplayError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, VIRTUAL_KEY,
        };

        Ok(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(self.virtual_key),
                    wScan: self.scan_code,
                    dwFlags: KEYBD_EVENT_FLAGS(self.flags),
                    time: 0,
                    dwExtraInfo: self.extra_info_for_pointer()?,
                },
            },
        })
    }
}

#[cfg(windows)]
/// Deliver one prepared replay record through `SendInput`.
///
/// This function is deliberately separate from the low-level hook classifier;
/// callers must never invoke it from a hook callback.
pub fn send_replay_input(replay: KeyboardReplay) -> Result<(), ReplayError> {
    use std::mem::size_of;
    use windows::Win32::Foundation::GetLastError;
    use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;

    let input = replay.as_windows_input()?;
    let sent = unsafe {
        SendInput(
            std::slice::from_ref(&input),
            size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        )
    };
    if sent == 1 {
        Ok(())
    } else {
        Err(ReplayError::SendInputFailed {
            code: unsafe { GetLastError().0 },
        })
    }
}
