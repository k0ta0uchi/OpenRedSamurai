//! Platform boundary for the resident RED SAMURAI input path.
//!
//! The resident process reads the device's separate input collection and
//! translates the observed keyboard-style reports into software events.  It
//! must not reuse the configuration transport: this module has no feature
//! report write path.  All protocol decisions are deliberately explicit so a
//! different collection or report shape is ignored before it can produce an
//! event.

use std::error::Error;
use std::fmt;

// `tests/resident_platform.rs` includes this file as a module instead of
// compiling it through the library crate.  Resolving through the parent keeps
// the relay references usable in both contexts (the harness provides the two
// relay modules beside this module).
use super::{keyboard_relay, keyboard_relay_windows};

/// RED SAMURAI's USB vendor ID.
pub const RED_SAMURAI_VENDOR_ID: u16 = 0x04D9;
/// RED SAMURAI 16400DPI mouse's USB product ID.
pub const RED_SAMURAI_PRODUCT_ID: u16 = 0xFC55;
/// Usage page reported by the HID descriptor for the resident keyboard-style
/// input collection (`MI_01` with the `KBD` path suffix).
pub const RESIDENT_INPUT_USAGE_PAGE: u16 = 0x0001;
/// Usage reported by the HID descriptor for the resident keyboard-style input
/// collection (`Keyboard`).
pub const RESIDENT_INPUT_USAGE: u16 = 0x0006;
/// Composite-device interface number encoded by `MI_01`.
pub const RESIDENT_INPUT_INTERFACE_NUMBER: i32 = 1;
/// HID API buffers include the report-ID byte.  The observed resident input
/// report therefore has one ID byte plus eight keyboard-style data bytes.
pub const RESIDENT_INPUT_REPORT_LENGTH: usize = 9;
/// Short aliases for code that refers to the resident collection as the
/// input interface rather than the resident subsystem.
pub const INPUT_USAGE_PAGE: u16 = RESIDENT_INPUT_USAGE_PAGE;
pub const INPUT_USAGE: u16 = RESIDENT_INPUT_USAGE;
pub const INPUT_INTERFACE_NUMBER: i32 = RESIDENT_INPUT_INTERFACE_NUMBER;
pub const INPUT_REPORT_LENGTH: usize = RESIDENT_INPUT_REPORT_LENGTH;
const REPORT_ID_NONE: u8 = 0;
const RESERVED_BYTE_OFFSET: usize = 2;
const KEY_USAGE_OFFSET: usize = 3;
const KEY_USAGE_COUNT: usize = 6;
const MODIFIER_OFFSET: usize = 1;
const MODIFIER_COUNT: usize = 8;

/// Errors returned by the platform seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    /// This build does not include a resident HID or SendInput backend.
    UnsupportedPlatform,
    /// HID enumeration failed before a candidate could be selected.
    Discovery { message: String },
    /// No exact resident input collection was present.
    InputUnavailable,
    /// Opening the exact resident input collection failed.
    InputOpen { message: String },
    /// Reading the resident input collection failed.
    InputRead { message: String },
    /// A report did not have the one bounded shape accepted by the parser.
    InvalidInputReportLength { expected: usize, actual: usize },
    /// A report used a report ID other than the unnumbered report ID.
    UnsupportedInputReportId { report_id: u8 },
    /// The boot-keyboard reserved byte must remain zero.
    NonZeroReservedByte { value: u8 },
    /// Keyboard error-rollover usages are not actionable button events.
    KeyboardRollover { usage: u8 },
    /// A usage appeared more than once in one report.
    DuplicateUsage { usage: u8 },
    /// A software event was not a valid SendInput event.
    InvalidSoftwareEvent { message: &'static str },
    /// Windows accepted no input records from SendInput.  The Win32 error is
    /// retained so a live verification run can distinguish a rejected call
    /// from a non-interactive/UIPI boundary.
    SendInputFailed { kind: &'static str, code: u32 },
    /// A semantic Windows control, such as microphone mute, failed.
    AudioControl { message: String },
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("resident platform input is only supported on Windows")
            }
            Self::Discovery { message } => write!(f, "resident HID discovery failed: {message}"),
            Self::InputUnavailable => f.write_str("resident input collection is not available"),
            Self::InputOpen { message } => {
                write!(f, "opening the resident input collection failed: {message}")
            }
            Self::InputRead { message } => {
                write!(f, "reading the resident input collection failed: {message}")
            }
            Self::InvalidInputReportLength { expected, actual } => write!(
                f,
                "invalid resident input report length: expected {expected} bytes, got {actual}"
            ),
            Self::UnsupportedInputReportId { report_id } => {
                write!(f, "unsupported resident input report ID: 0x{report_id:02X}")
            }
            Self::NonZeroReservedByte { value } => {
                write!(f, "resident input reserved byte is non-zero: 0x{value:02X}")
            }
            Self::KeyboardRollover { usage } => {
                write!(
                    f,
                    "keyboard rollover usage is not actionable: 0x{usage:02X}"
                )
            }
            Self::DuplicateUsage { usage } => {
                write!(
                    f,
                    "resident input usage appears more than once: 0x{usage:02X}"
                )
            }
            Self::InvalidSoftwareEvent { message } => f.write_str(message),
            Self::SendInputFailed { kind, code } => {
                write!(
                    f,
                    "SendInput rejected the {kind} event (Win32 error {code})"
                )
            }
            Self::AudioControl { message } => write!(f, "Windows audio control failed: {message}"),
        }
    }
}

impl Error for PlatformError {}

/// Alias used by callers that want the error name to identify the resident
/// subsystem explicitly.
pub type ResidentPlatformError = PlatformError;
/// Error alias matching the shorter input-adapter terminology.
pub type InputPlatformError = PlatformError;

/// Metadata for a candidate HID collection.  The fields are public so a
/// discovery result can be inspected without opening the device and so tests
/// can exercise filtering with synthetic metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidentInputCandidate {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
    pub input_report_length: usize,
}

impl ResidentInputCandidate {
    /// Construct candidate metadata for filtering or a deterministic test.
    pub fn new(
        path: impl Into<String>,
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
        usage: u16,
        interface_number: i32,
        input_report_length: usize,
    ) -> Self {
        Self {
            path: path.into(),
            vendor_id,
            product_id,
            usage_page,
            usage,
            interface_number,
            input_report_length,
        }
    }

    /// Windows HID device path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// USB vendor ID.
    pub const fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    /// USB product ID.
    pub const fn product_id(&self) -> u16 {
        self.product_id
    }

    /// HID usage page.
    pub const fn usage_page(&self) -> u16 {
        self.usage_page
    }

    /// HID usage.
    pub const fn usage(&self) -> u16 {
        self.usage
    }

    /// Composite-device interface number, or a backend-specific negative
    /// sentinel when the backend does not expose it.
    pub const fn interface_number(&self) -> i32 {
        self.interface_number
    }

    /// Input report length including the report ID byte.
    pub const fn input_report_length(&self) -> usize {
        self.input_report_length
    }
}

fn path_tokens(path: &str) -> impl Iterator<Item = String> {
    path.to_ascii_uppercase()
        .split(|character| matches!(character, '&' | '#' | '\\'))
        .map(|token| token.trim_matches(|character| matches!(character, '?' | '{' | '}')))
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter()
}

fn has_path_token(path: &str, expected: &str) -> bool {
    path_tokens(path).any(|token| token == expected)
}

fn has_any_interface_token(path: &str) -> bool {
    path_tokens(path).any(|token| {
        token.len() == 5
            && token.starts_with("MI_")
            && token.as_bytes()[3].is_ascii_hexdigit()
            && token.as_bytes()[4].is_ascii_hexdigit()
    })
}

/// Return whether metadata identifies the exact resident input collection.
///
/// VID/PID and usage metadata are required.  The path must identify the
/// resident interface with both the `MI_01` token and the keyboard-class
/// `KBD` suffix.  The backend must also report interface number one; an
/// unavailable or synthetic negative interface number is not accepted.
/// The expected nine-byte shape is also required, preventing a candidate for
/// another report layout from reaching the reader.
pub fn is_resident_input_interface(candidate: &ResidentInputCandidate) -> bool {
    if candidate.vendor_id != RED_SAMURAI_VENDOR_ID
        || candidate.product_id != RED_SAMURAI_PRODUCT_ID
        || candidate.usage_page != RESIDENT_INPUT_USAGE_PAGE
        || candidate.usage != RESIDENT_INPUT_USAGE
        || candidate.input_report_length != RESIDENT_INPUT_REPORT_LENGTH
    {
        return false;
    }

    if !has_path_token(&candidate.path, "VID_04D9") || !has_path_token(&candidate.path, "PID_FC55")
    {
        return false;
    }

    has_any_interface_token(&candidate.path)
        && has_path_token(&candidate.path, "MI_01")
        && has_path_token(&candidate.path, "KBD")
        && candidate.interface_number == RESIDENT_INPUT_INTERFACE_NUMBER
}

/// Filter an iterator to exact resident input collection candidates.
pub fn select_resident_input_interfaces<'a>(
    candidates: impl IntoIterator<Item = &'a ResidentInputCandidate>,
) -> Vec<&'a ResidentInputCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| is_resident_input_interface(candidate))
        .collect()
}

/// Alias using “devices”, matching HID discovery terminology.
pub fn select_resident_input_devices<'a>(
    candidates: impl IntoIterator<Item = &'a ResidentInputCandidate>,
) -> Vec<&'a ResidentInputCandidate> {
    select_resident_input_interfaces(candidates)
}

/// Alias using “device” for single-candidate checks.
pub fn is_resident_input_device(candidate: &ResidentInputCandidate) -> bool {
    is_resident_input_interface(candidate)
}

/// Enumerate attached, exact resident input collections without opening or
/// writing to any device.
#[cfg(windows)]
pub fn discover_resident_input_interfaces() -> Result<Vec<ResidentInputCandidate>, PlatformError> {
    let api = hidapi::HidApi::new().map_err(|error| PlatformError::Discovery {
        message: error.to_string(),
    })?;

    Ok(api
        .device_list()
        .filter_map(|info| {
            let candidate = ResidentInputCandidate::new(
                info.path().to_string_lossy(),
                info.vendor_id(),
                info.product_id(),
                info.usage_page(),
                info.usage(),
                info.interface_number(),
                RESIDENT_INPUT_REPORT_LENGTH,
            );
            is_resident_input_interface(&candidate).then_some(candidate)
        })
        .collect())
}

/// HID discovery is deliberately unavailable on non-Windows builds; falling
/// back to a different HID backend could select the wrong collection.
#[cfg(not(windows))]
pub fn discover_resident_input_interfaces() -> Result<Vec<ResidentInputCandidate>, PlatformError> {
    Err(PlatformError::UnsupportedPlatform)
}

/// Alias for callers that use the device-oriented name.
pub fn discover_resident_input_devices() -> Result<Vec<ResidentInputCandidate>, PlatformError> {
    discover_resident_input_interfaces()
}

/// Short alias for HID callers that do not distinguish collections from
/// devices at their call site.
pub fn discover_input_devices() -> Result<Vec<ResidentInputCandidate>, PlatformError> {
    discover_resident_input_interfaces()
}

/// Alias for availability checks that should remain read-only.
pub fn resident_input_available() -> Result<bool, PlatformError> {
    Ok(!discover_resident_input_interfaces()?.is_empty())
}

/// Parsed, known-shape resident input report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentInputReport {
    report_id: u8,
    modifiers: u8,
    key_usages: [u8; KEY_USAGE_COUNT],
}

/// Alias for generic input-loop code.
pub type InputReport = ResidentInputReport;

impl ResidentInputReport {
    /// The report ID, which is always zero for this unnumbered collection.
    pub const fn report_id(&self) -> u8 {
        self.report_id
    }

    /// Modifier bit mask from the first data byte.
    pub const fn modifiers(&self) -> u8 {
        self.modifiers
    }

    /// Six key-array usages from the known report layout.
    pub const fn key_usages(&self) -> &[u8; KEY_USAGE_COUNT] {
        &self.key_usages
    }

    /// Return active keyboard usages, with modifiers represented by their
    /// standard usages `0xE0..=0xE7`.
    pub fn active_usages(&self) -> Vec<u8> {
        let mut usages = Vec::with_capacity(MODIFIER_COUNT + KEY_USAGE_COUNT);
        for bit in 0..MODIFIER_COUNT {
            if self.modifiers & (1 << bit) != 0 {
                usages.push(0xE0 + bit as u8);
            }
        }
        usages.extend(self.key_usages.iter().copied().filter(|usage| *usage != 0));
        usages
    }
}

/// Parse exactly the observed nine-byte, unnumbered keyboard-style report.
///
/// The function is intentionally fallible rather than treating arbitrary
/// bytes as a button bitmask.  Callers must discard errors and must not turn
/// an unrecognized report into a feature-report write.
pub fn parse_resident_input_report(report: &[u8]) -> Result<ResidentInputReport, PlatformError> {
    if report.len() != RESIDENT_INPUT_REPORT_LENGTH {
        return Err(PlatformError::InvalidInputReportLength {
            expected: RESIDENT_INPUT_REPORT_LENGTH,
            actual: report.len(),
        });
    }
    if report[0] != REPORT_ID_NONE {
        return Err(PlatformError::UnsupportedInputReportId {
            report_id: report[0],
        });
    }
    if report[RESERVED_BYTE_OFFSET] != 0 {
        return Err(PlatformError::NonZeroReservedByte {
            value: report[RESERVED_BYTE_OFFSET],
        });
    }

    let mut seen = [false; 256];
    for bit in 0..MODIFIER_COUNT {
        if report[MODIFIER_OFFSET] & (1 << bit) != 0 {
            seen[0xE0 + bit] = true;
        }
    }

    let mut key_usages = [0u8; KEY_USAGE_COUNT];
    key_usages.copy_from_slice(&report[KEY_USAGE_OFFSET..KEY_USAGE_OFFSET + KEY_USAGE_COUNT]);
    for usage in key_usages.iter().copied().filter(|usage| *usage != 0) {
        if usage <= 0x03 {
            return Err(PlatformError::KeyboardRollover { usage });
        }
        if seen[usage as usize] {
            return Err(PlatformError::DuplicateUsage { usage });
        }
        seen[usage as usize] = true;
    }

    Ok(ResidentInputReport {
        report_id: report[0],
        modifiers: report[MODIFIER_OFFSET],
        key_usages,
    })
}

/// Short alias for the bounded resident report parser.
pub fn parse_input_report(report: &[u8]) -> Result<ResidentInputReport, PlatformError> {
    parse_resident_input_report(report)
}

/// One edge in the active usage set of a resident report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonTransition {
    usage: u8,
    pressed: bool,
}

/// Alias for generic input-loop code.
pub type InputTransition = ButtonTransition;

impl ButtonTransition {
    /// Construct a press edge for a keyboard usage.
    pub const fn pressed(usage: u8) -> Self {
        Self {
            usage,
            pressed: true,
        }
    }

    /// Construct a release edge for a keyboard usage.
    pub const fn released(usage: u8) -> Self {
        Self {
            usage,
            pressed: false,
        }
    }

    /// Keyboard usage associated with this edge.
    pub const fn usage(&self) -> u8 {
        self.usage
    }

    /// Whether this edge presses the usage.
    pub const fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Whether this edge releases the usage.
    pub const fn is_released(&self) -> bool {
        !self.pressed
    }
}

/// Convert one Windows Raw Input keyboard event back to the USB HID usage
/// namespace used by the resident profile resolver. Raw Input exposes the
/// virtual key rather than the original nine-byte keyboard report, so only
/// this explicit mapping is accepted; unknown virtual keys are ignored.
#[cfg(windows)]
pub fn map_raw_keyboard_event(virtual_key: u16, flags: u16) -> Option<ButtonTransition> {
    let usage = match virtual_key {
        0x30..=0x39 => {
            if virtual_key == 0x30 {
                0x27
            } else {
                (virtual_key - 0x30 + 0x1D) as u8
            }
        }
        0x41..=0x5A => (virtual_key - 0x41 + 0x04) as u8,
        0x0D => 0x28,
        0x1B => 0x29,
        0x08 => 0x2A,
        0x09 => 0x2B,
        0x20 => 0x2C,
        0xBD => 0x2D,
        0xBB => 0x2E,
        0xDB => 0x2F,
        0xDD => 0x30,
        0xDC => 0x31,
        0xBA => 0x33,
        0xDE => 0x34,
        0xC0 => 0x35,
        0xBC => 0x36,
        0xBE => 0x37,
        0xBF => 0x38,
        0x14 => 0x39,
        0x70..=0x7B => (virtual_key - 0x70 + 0x3A) as u8,
        0x2C => 0x46,
        0x91 => 0x47,
        0x13 => 0x48,
        0x2D => 0x49,
        0x24 => 0x4A,
        0x21 => 0x4B,
        0x2E => 0x4C,
        0x23 => 0x4D,
        0x22 => 0x4E,
        0x27 => 0x4F,
        0x25 => 0x50,
        0x28 => 0x51,
        0x26 => 0x52,
        0x90 => 0x53,
        0xA2 => 0xE0,
        0xA0 => 0xE1,
        0xA4 => 0xE2,
        0x5B => 0xE3,
        0xA3 => 0xE4,
        0xA1 => 0xE5,
        0xA5 => 0xE6,
        0x5C => 0xE7,
        _ => return None,
    };
    Some(if flags & 0x0001 == 0 {
        ButtonTransition::pressed(usage)
    } else {
        ButtonTransition::released(usage)
    })
}

/// Convert one Raw Input edge into the bounded report shape consumed by the
/// existing parser.  A release is represented by an empty active set; callers
/// that need the exact released usage should consume the `ButtonTransition`
/// directly from the Raw Input adapter.
#[cfg(windows)]
pub fn raw_input_transition_report(
    transition: ButtonTransition,
) -> [u8; RESIDENT_INPUT_REPORT_LENGTH] {
    let mut report = [0u8; RESIDENT_INPUT_REPORT_LENGTH];
    if !transition.is_pressed() {
        return report;
    }

    if (0xE0..=0xE7).contains(&transition.usage()) {
        report[MODIFIER_OFFSET] = 1 << (transition.usage() - 0xE0);
    } else {
        report[KEY_USAGE_OFFSET] = transition.usage();
    }
    report
}

/// Normalize the path spellings used by the Windows HID and Raw Input APIs.
/// hidapi exposes the keyboard collection with a trailing \\KBD component and
/// the HID interface GUID, while Raw Input's RIDI_DEVICENAME commonly omits
/// \\KBD and uses the keyboard-class interface GUID.  The terminal class GUID
/// is a representation detail; VID/PID, interface, instance, and collection
/// tokens remain part of the identity comparison.
#[cfg(windows)]
pub fn normalize_raw_input_device_path(path: &str) -> String {
    let mut normalized = path.trim_matches('\0').trim().replace('/', "\\");
    normalized.make_ascii_uppercase();
    while normalized.ends_with("\\KBD") {
        normalized.truncate(normalized.len() - "\\KBD".len());
    }
    for class_guid in [
        "#{4D1E55B2-F16F-11CF-88CB-001111000030}",
        "#{884B96C3-56EF-11D1-BC8C-00A0C91405DD}",
    ] {
        if normalized.ends_with(class_guid) {
            normalized.truncate(normalized.len() - class_guid.len());
            break;
        }
    }
    normalized
}

/// Match Raw Input events to the exact HID collection selected by discovery.
/// The only tolerated spelling differences are the backend-specific \\KBD
/// suffix and the known HID/keyboard class GUID pair; VID/PID/interface and
/// the instance path remain exact.
#[cfg(windows)]
pub fn raw_input_device_path_matches(expected: &str, observed: &str) -> bool {
    normalize_raw_input_device_path(expected) == normalize_raw_input_device_path(observed)
}

/// Stateful edge detector for known resident reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ButtonStateTracker {
    active: [bool; 256],
}

/// Alias for callers that name the state machine after the input collection.
pub type ResidentButtonTracker = ButtonStateTracker;

impl Default for ButtonStateTracker {
    fn default() -> Self {
        Self {
            active: [false; 256],
        }
    }
}

impl ButtonStateTracker {
    /// Parse one report and return only changed usages.  State is unchanged if
    /// parsing fails, so a malformed report cannot synthesize releases.
    pub fn update(&mut self, report: &[u8]) -> Result<Vec<ButtonTransition>, PlatformError> {
        let parsed = parse_resident_input_report(report)?;
        let mut current = [false; 256];
        for usage in parsed.active_usages() {
            current[usage as usize] = true;
        }

        let mut transitions = Vec::new();
        for usage in 0..=u8::MAX {
            let was_active = self.active[usage as usize];
            let is_active = current[usage as usize];
            if was_active == is_active {
                continue;
            }
            transitions.push(if is_active {
                ButtonTransition::pressed(usage)
            } else {
                ButtonTransition::released(usage)
            });
        }
        self.active = current;
        Ok(transitions)
    }

    /// Forget all active usages without emitting synthetic release events.
    pub fn reset(&mut self) {
        self.active = [false; 256];
    }
}

/// A keyboard input record for the Windows SendInput boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub virtual_key: u16,
    pub scan_code: u16,
    pub flags: u32,
}

impl KeyboardEvent {
    /// Construct a virtual-key key-down event.
    pub const fn key_down(virtual_key: u16) -> Self {
        Self {
            virtual_key,
            scan_code: 0,
            flags: 0,
        }
    }

    /// Construct a virtual-key key-up event.
    pub const fn key_up(virtual_key: u16) -> Self {
        Self {
            virtual_key,
            scan_code: 0,
            flags: 0x0002,
        }
    }

    /// Construct an explicit keyboard event for callers that have a verified
    /// scan code or SendInput flag combination.
    pub const fn new(virtual_key: u16, scan_code: u16, flags: u32) -> Self {
        Self {
            virtual_key,
            scan_code,
            flags,
        }
    }
}

/// Mouse buttons supported by the SendInput mouse event helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// A mouse input record for the Windows SendInput boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    pub dx: i32,
    pub dy: i32,
    pub mouse_data: u32,
    pub flags: u32,
}

impl MouseEvent {
    /// Construct a relative mouse move.
    pub const fn move_relative(dx: i32, dy: i32) -> Self {
        Self {
            dx,
            dy,
            mouse_data: 0,
            flags: 0x0001,
        }
    }

    /// Construct a mouse-button edge.
    pub const fn button(button: MouseButton, pressed: bool) -> Self {
        let (flags, mouse_data) = match button {
            MouseButton::Left => (if pressed { 0x0002 } else { 0x0004 }, 0),
            MouseButton::Right => (if pressed { 0x0008 } else { 0x0010 }, 0),
            MouseButton::Middle => (if pressed { 0x0020 } else { 0x0040 }, 0),
            MouseButton::X1 => (if pressed { 0x0080 } else { 0x0100 }, 1),
            MouseButton::X2 => (if pressed { 0x0080 } else { 0x0100 }, 2),
        };
        Self {
            dx: 0,
            dy: 0,
            mouse_data,
            flags,
        }
    }

    /// Construct a vertical wheel event.  Positive and negative values retain
    /// the signed wheel delta expected by Windows in `mouseData`.
    pub const fn wheel(delta: i32) -> Self {
        Self {
            dx: 0,
            dy: 0,
            mouse_data: delta as u32,
            flags: 0x0800,
        }
    }

    /// Construct an explicit mouse event for a verified SendInput mapping.
    pub const fn new(dx: i32, dy: i32, mouse_data: u32, flags: u32) -> Self {
        Self {
            dx,
            dy,
            mouse_data,
            flags,
        }
    }
}

/// Media or browser key input for the Windows SendInput boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaEvent {
    pub virtual_key: u16,
    pub pressed: bool,
}

impl MediaEvent {
    /// Construct a media key-down event from a Windows media virtual key.
    pub const fn key_down(virtual_key: u16) -> Self {
        Self {
            virtual_key,
            pressed: true,
        }
    }

    /// Construct a media key-up event from a Windows media virtual key.
    pub const fn key_up(virtual_key: u16) -> Self {
        Self {
            virtual_key,
            pressed: false,
        }
    }
}

/// A software input event accepted by the platform output seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareInputEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
    Media(MediaEvent),
}

#[cfg(windows)]
fn validate_keyboard_event(event: KeyboardEvent) -> Result<(), PlatformError> {
    if event.virtual_key == 0 && event.scan_code == 0 && event.flags & (0x0004 | 0x0008) == 0 {
        return Err(PlatformError::InvalidSoftwareEvent {
            message: "a keyboard event needs a virtual key, scan code, or Unicode flag",
        });
    }
    if event.flags & !0x000F != 0 {
        return Err(PlatformError::InvalidSoftwareEvent {
            message: "keyboard event contains unknown SendInput flags",
        });
    }
    Ok(())
}

#[cfg(windows)]
fn validate_mouse_event(event: MouseEvent) -> Result<(), PlatformError> {
    // MOUSE_EVENT_FLAGS currently used by this seam: MOVE, the five button
    // edges, wheel/hwheel, MOVE_NOCOALESCE, ABSOLUTE, and VIRTUALDESK.
    if event.flags == 0 || event.flags & !0xF9FF != 0 {
        return Err(PlatformError::InvalidSoftwareEvent {
            message: "mouse event contains no known SendInput action",
        });
    }
    Ok(())
}

#[cfg(windows)]
fn is_media_virtual_key(virtual_key: u16) -> bool {
    matches!(virtual_key, 0xA6..=0xB7)
}

#[cfg(windows)]
fn validate_media_event(event: MediaEvent) -> Result<(), PlatformError> {
    if !is_media_virtual_key(event.virtual_key) {
        return Err(PlatformError::InvalidSoftwareEvent {
            message: "media event needs a Windows media or browser virtual key",
        });
    }
    Ok(())
}

/// Send one keyboard event through Windows SendInput, or return
/// `UnsupportedPlatform` on non-Windows builds.
pub fn send_keyboard_event(event: KeyboardEvent) -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        validate_keyboard_event(event)?;
        return send_windows_keyboard_event(event);
    }

    #[cfg(not(windows))]
    {
        let _ = event;
        Err(PlatformError::UnsupportedPlatform)
    }
}

/// Short alias for software keyboard injection.
pub fn send_keyboard(event: KeyboardEvent) -> Result<(), PlatformError> {
    send_keyboard_event(event)
}

/// Send one mouse event through Windows SendInput, or return
/// `UnsupportedPlatform` on non-Windows builds.
pub fn send_mouse_event(event: MouseEvent) -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        validate_mouse_event(event)?;
        return send_windows_mouse_event(event);
    }

    #[cfg(not(windows))]
    {
        let _ = event;
        Err(PlatformError::UnsupportedPlatform)
    }
}

/// Short alias for software mouse injection.
pub fn send_mouse(event: MouseEvent) -> Result<(), PlatformError> {
    send_mouse_event(event)
}

/// Send one media/browser event through Windows SendInput, or return
/// `UnsupportedPlatform` on non-Windows builds.
pub fn send_media_event(event: MediaEvent) -> Result<(), PlatformError> {
    #[cfg(windows)]
    {
        validate_media_event(event)?;
        return send_windows_media_event(event);
    }

    #[cfg(not(windows))]
    {
        let _ = event;
        Err(PlatformError::UnsupportedPlatform)
    }
}

/// Short alias for software media-key injection.
pub fn send_media(event: MediaEvent) -> Result<(), PlatformError> {
    send_media_event(event)
}

/// Dispatch a single software event through its matching SendInput helper.
pub fn send_software_event(event: SoftwareInputEvent) -> Result<(), PlatformError> {
    match event {
        SoftwareInputEvent::Keyboard(event) => send_keyboard_event(event),
        SoftwareInputEvent::Mouse(event) => send_mouse_event(event),
        SoftwareInputEvent::Media(event) => send_media_event(event),
    }
}

#[cfg(windows)]
fn send_windows_input(
    input: windows::Win32::UI::Input::KeyboardAndMouse::INPUT,
    kind: &'static str,
) -> Result<(), PlatformError> {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;

    let sent = unsafe {
        SendInput(
            &[input],
            size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32,
        )
    };
    if sent == 1 {
        Ok(())
    } else {
        let code = unsafe { windows::Win32::Foundation::GetLastError().0 };
        Err(PlatformError::SendInputFailed { kind, code })
    }
}

#[cfg(windows)]
pub(crate) fn write_relay_trace(event: &str) {
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    let Some(path) = std::env::var_os("REDSAMURAI_RECOVERY_LOG") else {
        return;
    };
    static RELAY_TRACE_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let trace_lock = RELAY_TRACE_LOCK.get_or_init(|| std::sync::Mutex::new(()));
    let _trace_guard = match trace_lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let _ = writeln!(file, "timestamp_ms={timestamp_ms} {event}");
    let _ = file.flush();
}

#[cfg(windows)]
fn send_windows_keyboard_event(event: KeyboardEvent) -> Result<(), PlatformError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, VIRTUAL_KEY,
    };

    let extra_info = keyboard_relay_extra_info()?;
    if extra_info != 0 {
        write_relay_trace(&format!(
            "event=keyboard_relay_output kind=keyboard vk=0x{:02X} scan=0x{:02X} flags=0x{:04X} marker=0x{:016X}",
            event.virtual_key, event.scan_code, event.flags, extra_info
        ));
    }

    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(event.virtual_key),
                wScan: event.scan_code,
                dwFlags: KEYBD_EVENT_FLAGS(event.flags),
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    };
    if extra_info != 0 {
        remember_pending_keyboard_input(event.virtual_key, event.scan_code, event.flags);
    }
    let result = send_windows_input(input, "keyboard");
    if result.is_err() && extra_info != 0 {
        forget_pending_keyboard_input(event.virtual_key, event.scan_code, event.flags);
    }
    result
}

#[cfg(windows)]
fn keyboard_relay_extra_info() -> Result<usize, PlatformError> {
    if !keyboard_relay_windows::relay_opt_in_requested() {
        return Ok(0);
    }
    usize::try_from(keyboard_relay_windows::REDSAMURAI_SELF_INJECT_MARKER).map_err(|_| {
        PlatformError::InvalidSoftwareEvent {
            message: "keyboard relay marker does not fit the Windows pointer width",
        }
    })
}

#[cfg(windows)]
fn send_windows_media_event(event: MediaEvent) -> Result<(), PlatformError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };

    let extra_info = keyboard_relay_extra_info()?;
    if extra_info != 0 {
        write_relay_trace(&format!(
            "event=keyboard_relay_output kind=media vk=0x{:02X} edge={} marker=0x{:016X}",
            event.virtual_key,
            if event.pressed { "down" } else { "up" },
            extra_info
        ));
    }

    let flags = if event.pressed {
        KEYBD_EVENT_FLAGS(0)
    } else {
        KEYEVENTF_KEYUP
    };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(event.virtual_key),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    };
    let replay_flags = if event.pressed { 0 } else { 0x0002 };
    if extra_info != 0 {
        remember_pending_keyboard_input(event.virtual_key, 0, replay_flags);
    }
    let result = send_windows_input(input, "media");
    if result.is_err() && extra_info != 0 {
        forget_pending_keyboard_input(event.virtual_key, 0, replay_flags);
    }
    result
}

#[cfg(windows)]
fn send_windows_mouse_event(event: MouseEvent) -> Result<(), PlatformError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    };

    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: event.dx,
                dy: event.dy,
                mouseData: event.mouse_data,
                dwFlags: MOUSE_EVENT_FLAGS(event.flags),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    send_windows_input(input, "mouse")
}

#[cfg(windows)]
/// Raw Input adapter for the resident keyboard-class collection.
///
/// Windows deliberately denies synchronous reads from the keyboard-class HID
/// path (`\\KBD`) even when the caller is elevated.  Raw Input is the supported
/// user-mode boundary for that collection, so the adapter owns a message-only
/// window on a dedicated thread. The default mode forwards only events whose
/// device path matches the exact HID candidate selected by discovery. The
/// explicit all-keyboard relay mode registers the keyboard usage with
/// `RIDEV_NOLEGACY`, classifies the exact target path, and replays ordinary
/// keyboard edges with the relay marker.
#[cfg(windows)]
struct KeyboardDuplicateState {
    filter: std::sync::Mutex<crate::keyboard_suppression::KeyboardDuplicateFilter>,
    wake: std::sync::Condvar,
    hook_alive: std::sync::atomic::AtomicBool,
    hook_error: std::sync::Mutex<Option<String>>,
    trace: std::sync::Mutex<Option<std::fs::File>>,
}

#[cfg(windows)]
impl KeyboardDuplicateState {
    fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            filter: std::sync::Mutex::new(
                crate::keyboard_suppression::KeyboardDuplicateFilter::default(),
            ),
            wake: std::sync::Condvar::new(),
            hook_alive: std::sync::atomic::AtomicBool::new(false),
            hook_error: std::sync::Mutex::new(None),
            trace: std::sync::Mutex::new(
                std::env::var_os("REDSAMURAI_KEYBOARD_SUPPRESSION_TRACE").and_then(|path| {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .ok()
                }),
            ),
        })
    }

    fn observe_target(&self, sample: crate::keyboard_suppression::RawKeyboardSample) {
        let mut filter = match self.filter.lock() {
            Ok(filter) => filter,
            Err(poisoned) => poisoned.into_inner(),
        };
        let pending_before = filter.pending_len();
        filter.observe_target(sample);
        self.trace(format!(
            "event=raw_observe vk=0x{:02X} scan=0x{:02X} flags=0x{:04X} time={} pending_before={} pending_after={}",
            sample.virtual_key,
            sample.scan_code,
            sample.flags,
            sample.timestamp_ms,
            pending_before,
            filter.pending_len()
        ));
        self.wake.notify_all();
    }

    fn classify(
        &self,
        event: crate::keyboard_suppression::LegacyKeyboardSample,
    ) -> crate::keyboard_suppression::KeyboardFilterDecision {
        use crate::keyboard_suppression::{KeyboardFilterDecision, LOW_LEVEL_KEY_INJECTED};

        let injected = event.flags & LOW_LEVEL_KEY_INJECTED != 0;
        let deadline = std::time::Instant::now().checked_add(std::time::Duration::from_millis(8));
        let mut filter = match self.filter.lock() {
            Ok(filter) => filter,
            Err(poisoned) => poisoned.into_inner(),
        };

        loop {
            let decision = filter.classify(event);
            if injected || decision == KeyboardFilterDecision::Suppress {
                self.trace(format!(
                    "event=legacy_classify vk=0x{:02X} scan=0x{:02X} flags=0x{:08X} time={} injected={} decision={:?} pending={}",
                    event.virtual_key,
                    event.scan_code,
                    event.flags,
                    event.timestamp_ms,
                    injected,
                    decision,
                    filter.pending_len()
                ));
                return decision;
            }

            let Some(deadline) = deadline else {
                return KeyboardFilterDecision::Pass;
            };
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                self.trace(format!(
                    "event=legacy_classify vk=0x{:02X} scan=0x{:02X} flags=0x{:08X} time={} injected={} decision=PassTimeout pending={}",
                    event.virtual_key,
                    event.scan_code,
                    event.flags,
                    event.timestamp_ms,
                    injected,
                    filter.pending_len()
                ));
                return KeyboardFilterDecision::Pass;
            }
            let waited = self.wake.wait_timeout(filter, remaining);
            match waited {
                Ok((next, result)) => {
                    filter = next;
                    if result.timed_out() {
                        self.trace(format!(
                            "event=legacy_classify vk=0x{:02X} scan=0x{:02X} flags=0x{:08X} time={} injected={} decision=PassWaitTimeout pending={}",
                            event.virtual_key,
                            event.scan_code,
                            event.flags,
                            event.timestamp_ms,
                            injected,
                            filter.pending_len()
                        ));
                        return KeyboardFilterDecision::Pass;
                    }
                }
                Err(poisoned) => {
                    let (mut next, _) = poisoned.into_inner();
                    let decision = next.classify(event);
                    self.trace(format!(
                        "event=legacy_classify vk=0x{:02X} scan=0x{:02X} flags=0x{:08X} time={} injected={} decision={:?} pending={}",
                        event.virtual_key,
                        event.scan_code,
                        event.flags,
                        event.timestamp_ms,
                        injected,
                        decision,
                        next.pending_len()
                    ));
                    return decision;
                }
            }
        }
    }

    fn trace(&self, message: String) {
        use std::io::Write;

        let Ok(mut trace) = self.trace.lock() else {
            return;
        };
        let Some(file) = trace.as_mut() else {
            return;
        };
        let _ = writeln!(file, "{}", message);
        let _ = file.flush();
    }

    fn mark_hook_failed(&self, message: impl Into<String>) {
        self.hook_alive
            .store(false, std::sync::atomic::Ordering::Release);
        if let Ok(mut error) = self.hook_error.lock() {
            *error = Some(message.into());
        }
        self.wake.notify_all();
    }

    fn ensure_hook_alive(&self) -> Result<(), PlatformError> {
        if self.hook_alive.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        let message = self
            .hook_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
            .unwrap_or_else(|| "keyboard suppression hook is not active".to_owned());
        Err(PlatformError::InputRead { message })
    }
}

#[cfg(windows)]
struct KeyboardSuppressionHook {
    state: std::sync::Arc<KeyboardDuplicateState>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl KeyboardSuppressionHook {
    fn new(
        state: std::sync::Arc<KeyboardDuplicateState>,
        shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self, PlatformError> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let panic_ready_sender = ready_sender.clone();
        let thread_id_cell = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let worker_thread_id_cell = thread_id_cell.clone();
        let worker_state = state.clone();
        let panic_state = state.clone();
        let worker_shutdown = shutdown_requested.clone();
        let thread = std::thread::Builder::new()
            .name("redsamurai-keyboard-filter".to_owned())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    keyboard_hook_thread(
                        worker_state,
                        worker_shutdown,
                        ready_sender,
                        worker_thread_id_cell,
                    )
                }));
                if result.is_err() {
                    let message = "low-level keyboard suppression hook panicked";
                    panic_state.mark_hook_failed(message);
                    let _ = panic_ready_sender.try_send(Err(message.to_owned()));
                }
            })
            .map_err(|error| PlatformError::InputOpen {
                message: format!("starting the keyboard suppression hook failed: {error}"),
            })?;

        let thread_id = match ready_receiver.recv_timeout(RAW_INPUT_STARTUP_TIMEOUT) {
            Ok(Ok(thread_id)) => thread_id,
            Ok(Err(message)) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                post_keyboard_hook_quit(thread_id_cell.load(std::sync::atomic::Ordering::Acquire));
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(PlatformError::InputOpen { message });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                post_keyboard_hook_quit(thread_id_cell.load(std::sync::atomic::Ordering::Acquire));
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(PlatformError::InputOpen {
                    message: format!(
                        "keyboard suppression hook startup timed out after {} ms",
                        RAW_INPUT_STARTUP_TIMEOUT.as_millis()
                    ),
                });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                post_keyboard_hook_quit(thread_id_cell.load(std::sync::atomic::Ordering::Acquire));
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(PlatformError::InputOpen {
                    message: "keyboard suppression hook disconnected before initialization"
                        .to_owned(),
                });
            }
        };

        Ok(Self {
            state,
            shutdown_requested,
            thread_id,
            thread: Some(thread),
        })
    }

    fn request_shutdown(&self) {
        self.shutdown_requested
            .store(true, std::sync::atomic::Ordering::Release);
        self.state.wake.notify_all();
        post_keyboard_hook_quit(self.thread_id);
    }
}

#[cfg(windows)]
impl Drop for KeyboardSuppressionHook {
    fn drop(&mut self) {
        self.request_shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
        }
    }
}

#[cfg(windows)]
fn post_keyboard_hook_quit(thread_id: u32) {
    if thread_id == 0 {
        return;
    }
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            thread_id,
            windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

#[cfg(windows)]
thread_local! {
    static LOW_LEVEL_KEYBOARD_STATE:
        std::cell::RefCell<Option<std::sync::Arc<KeyboardDuplicateState>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(windows)]
unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use crate::keyboard_suppression::{KeyboardFilterDecision, LegacyKeyboardSample};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    if code >= HC_ACTION as i32
        && matches!(
            wparam.0 as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
        && lparam.0 != 0
    {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if let Ok(virtual_key) = u16::try_from(info.vkCode) {
            let event =
                LegacyKeyboardSample::new(virtual_key, info.scanCode, info.flags.0, info.time);
            let state = LOW_LEVEL_KEYBOARD_STATE.with(|slot| slot.borrow().clone());
            if let Some(state) = state {
                if state.classify(event) == KeyboardFilterDecision::Suppress {
                    return windows::Win32::Foundation::LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(
        windows::Win32::UI::WindowsAndMessaging::HHOOK::default(),
        code,
        wparam,
        lparam,
    )
}

#[cfg(windows)]
fn keyboard_hook_thread(
    state: std::sync::Arc<KeyboardDuplicateState>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ready_sender: std::sync::mpsc::SyncSender<Result<u32, String>>,
    thread_id_cell: std::sync::Arc<std::sync::atomic::AtomicU32>,
) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMessageW, PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, PM_NOREMOVE,
        WH_KEYBOARD_LL,
    };

    unsafe {
        let thread_id = windows::Win32::System::Threading::GetCurrentThreadId();
        thread_id_cell.store(thread_id, std::sync::atomic::Ordering::Release);
        let hinstance: HINSTANCE = match GetModuleHandleW(PCWSTR::null()) {
            Ok(hmodule) => hmodule.into(),
            Err(error) => {
                let message = format!("GetModuleHandleW for keyboard hook failed: {error}");
                state.mark_hook_failed(message.clone());
                let _ = ready_sender.send(Err(message));
                return;
            }
        };
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, HWND(std::ptr::null_mut()), 0, 0, PM_NOREMOVE);
        if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            let _ = ready_sender.send(Err(
                "keyboard suppression hook stopped before startup".to_owned()
            ));
            return;
        }

        LOW_LEVEL_KEYBOARD_STATE.with(|slot| {
            *slot.borrow_mut() = Some(state.clone());
        });
        let hook =
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), hinstance, 0) {
                Ok(hook) => hook,
                Err(error) => {
                    LOW_LEVEL_KEYBOARD_STATE.with(|slot| {
                        slot.borrow_mut().take();
                    });
                    let message = format!("SetWindowsHookExW failed: {error}");
                    state.mark_hook_failed(message.clone());
                    let _ = ready_sender.send(Err(message));
                    return;
                }
            };

        state
            .hook_alive
            .store(true, std::sync::atomic::Ordering::Release);
        write_relay_trace(&format!(
            "event=keyboard_suppression_hook_ready thread_id={thread_id}"
        ));
        if ready_sender.send(Ok(thread_id)).is_err() {
            shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
        }

        loop {
            let result = GetMessageW(&mut message, HWND(std::ptr::null_mut()), 0, 0);
            if result.0 == -1 {
                let error = GetLastError().0;
                if !shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                    state.mark_hook_failed(format!(
                        "GetMessageW for keyboard hook failed (Win32 error {error})"
                    ));
                }
                break;
            }
            if result.0 == 0 {
                if !shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                    state.mark_hook_failed(
                        "keyboard suppression hook message loop exited unexpectedly",
                    );
                }
                break;
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&message);
            let _ = windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&message);
            if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
        }

        state
            .hook_alive
            .store(false, std::sync::atomic::Ordering::Release);
        let _ = UnhookWindowsHookEx(hook);
        LOW_LEVEL_KEYBOARD_STATE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

#[cfg(windows)]
/// Health and suppression gate owned by the all-keyboard relay hook.
///
/// The hook callback must remain O(1): it only reads these atomics and calls
/// `CallNextHookEx` or returns a suppression result.  Raw Input parsing and
/// `SendInput` happen on the relay message-loop thread instead.
struct KeyboardRelayHookState {
    gate_enabled: std::sync::atomic::AtomicBool,
    hook_alive: std::sync::atomic::AtomicBool,
    hook_error: std::sync::Mutex<Option<String>>,
}

#[cfg(windows)]
impl KeyboardRelayHookState {
    fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            gate_enabled: std::sync::atomic::AtomicBool::new(false),
            hook_alive: std::sync::atomic::AtomicBool::new(false),
            hook_error: std::sync::Mutex::new(None),
        })
    }

    fn enable_gate(&self) {
        self.gate_enabled
            .store(true, std::sync::atomic::Ordering::Release);
        write_relay_trace("event=keyboard_relay_gate enabled=true");
    }

    fn disable_gate(&self) {
        self.gate_enabled
            .store(false, std::sync::atomic::Ordering::Release);
        write_relay_trace("event=keyboard_relay_gate enabled=false");
    }

    fn mark_hook_failed(&self, message: impl Into<String>) {
        let message = message.into();
        self.disable_gate();
        self.hook_alive
            .store(false, std::sync::atomic::Ordering::Release);
        if let Ok(mut error) = self.hook_error.lock() {
            *error = Some(message.clone());
        }
        write_relay_trace(&format!(
            "event=keyboard_relay_failed error={}",
            message.replace(['\r', '\n'], " ")
        ));
    }

    fn ensure_hook_alive(&self) -> Result<(), PlatformError> {
        if self.hook_alive.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        let message = self
            .hook_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
            .unwrap_or_else(|| "keyboard relay hook is not active".to_owned());
        Err(PlatformError::InputRead { message })
    }
}

#[cfg(windows)]
struct KeyboardRelayHook {
    state: std::sync::Arc<KeyboardRelayHookState>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl KeyboardRelayHook {
    fn new(
        state: std::sync::Arc<KeyboardRelayHookState>,
        shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self, PlatformError> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let panic_ready_sender = ready_sender.clone();
        let thread_id_cell = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let worker_thread_id_cell = thread_id_cell.clone();
        let worker_state = state.clone();
        let panic_state = state.clone();
        let worker_shutdown = shutdown_requested.clone();
        let thread = std::thread::Builder::new()
            .name("redsamurai-keyboard-relay-hook".to_owned())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    keyboard_relay_hook_thread(
                        worker_state,
                        worker_shutdown,
                        ready_sender,
                        worker_thread_id_cell,
                    )
                }));
                if result.is_err() {
                    let message = "all-keyboard relay hook panicked";
                    panic_state.mark_hook_failed(message);
                    let _ = panic_ready_sender.try_send(Err(message.to_owned()));
                }
            })
            .map_err(|error| PlatformError::InputOpen {
                message: format!("starting the all-keyboard relay hook failed: {error}"),
            })?;

        let thread_id = match ready_receiver.recv_timeout(RAW_INPUT_STARTUP_TIMEOUT) {
            Ok(Ok(thread_id)) => thread_id,
            Ok(Err(message)) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                post_keyboard_hook_quit(thread_id_cell.load(std::sync::atomic::Ordering::Acquire));
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(PlatformError::InputOpen { message });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                post_keyboard_hook_quit(thread_id_cell.load(std::sync::atomic::Ordering::Acquire));
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(PlatformError::InputOpen {
                    message: format!(
                        "all-keyboard relay hook startup timed out after {} ms",
                        RAW_INPUT_STARTUP_TIMEOUT.as_millis()
                    ),
                });
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                post_keyboard_hook_quit(thread_id_cell.load(std::sync::atomic::Ordering::Acquire));
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(PlatformError::InputOpen {
                    message: "all-keyboard relay hook disconnected before initialization"
                        .to_owned(),
                });
            }
        };

        Ok(Self {
            state,
            shutdown_requested,
            thread_id,
            thread: Some(thread),
        })
    }

    fn disable_gate(&self) {
        self.state.disable_gate();
    }

    fn ensure_alive(&self) -> Result<(), PlatformError> {
        self.state.ensure_hook_alive()
    }

    fn request_shutdown(&self) {
        self.disable_gate();
        self.shutdown_requested
            .store(true, std::sync::atomic::Ordering::Release);
        post_keyboard_hook_quit(self.thread_id);
    }
}

#[cfg(windows)]
impl Drop for KeyboardRelayHook {
    fn drop(&mut self) {
        self.request_shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
        }
    }
}

#[cfg(windows)]
thread_local! {
    static LOW_LEVEL_KEYBOARD_RELAY_STATE:
        std::cell::RefCell<Option<std::sync::Arc<KeyboardRelayHookState>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(windows)]
unsafe extern "system" fn low_level_keyboard_relay_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    if code >= HC_ACTION as i32
        && matches!(
            wparam.0 as u32,
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP
        )
        && lparam.0 != 0
    {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let state = LOW_LEVEL_KEYBOARD_RELAY_STATE.with(|slot| slot.borrow().clone());
        if let Some(state) = state {
            let decision = keyboard_relay_windows::classify_hook_event(
                info.flags.0,
                info.dwExtraInfo as u64,
                state
                    .gate_enabled
                    .load(std::sync::atomic::Ordering::Acquire),
                state.hook_alive.load(std::sync::atomic::Ordering::Acquire),
            );
            if decision == keyboard_relay_windows::HookDecision::SuppressPhysical {
                return windows::Win32::Foundation::LRESULT(1);
            }
        }
    }

    CallNextHookEx(
        windows::Win32::UI::WindowsAndMessaging::HHOOK::default(),
        code,
        wparam,
        lparam,
    )
}

#[cfg(windows)]
fn keyboard_relay_hook_thread(
    state: std::sync::Arc<KeyboardRelayHookState>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ready_sender: std::sync::mpsc::SyncSender<Result<u32, String>>,
    thread_id_cell: std::sync::Arc<std::sync::atomic::AtomicU32>,
) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GetLastError, HINSTANCE, HWND};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMessageW, PeekMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, PM_NOREMOVE,
        WH_KEYBOARD_LL,
    };

    unsafe {
        let thread_id = windows::Win32::System::Threading::GetCurrentThreadId();
        thread_id_cell.store(thread_id, std::sync::atomic::Ordering::Release);
        let hinstance: HINSTANCE = match GetModuleHandleW(PCWSTR::null()) {
            Ok(hmodule) => hmodule.into(),
            Err(error) => {
                let message = format!("GetModuleHandleW for relay hook failed: {error}");
                state.mark_hook_failed(message.clone());
                let _ = ready_sender.send(Err(message));
                return;
            }
        };
        let mut message = MSG::default();
        let _ = PeekMessageW(&mut message, HWND(std::ptr::null_mut()), 0, 0, PM_NOREMOVE);
        if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            let _ = ready_sender.send(Err(
                "all-keyboard relay hook stopped before startup".to_owned()
            ));
            return;
        }

        LOW_LEVEL_KEYBOARD_RELAY_STATE.with(|slot| {
            *slot.borrow_mut() = Some(state.clone());
        });
        let hook = match SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_relay_proc),
            hinstance,
            0,
        ) {
            Ok(hook) => hook,
            Err(error) => {
                LOW_LEVEL_KEYBOARD_RELAY_STATE.with(|slot| {
                    slot.borrow_mut().take();
                });
                let message = format!("SetWindowsHookExW for relay failed: {error}");
                state.mark_hook_failed(message.clone());
                let _ = ready_sender.send(Err(message));
                return;
            }
        };

        state
            .hook_alive
            .store(true, std::sync::atomic::Ordering::Release);
        write_relay_trace(&format!(
            "event=keyboard_relay_hook_ready thread_id={thread_id}"
        ));
        if ready_sender.send(Ok(thread_id)).is_err() {
            shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
        }

        loop {
            let result = GetMessageW(&mut message, HWND(std::ptr::null_mut()), 0, 0);
            if result.0 == -1 {
                if !shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                    state.mark_hook_failed(format!(
                        "GetMessageW for relay hook failed (Win32 error {})",
                        GetLastError().0
                    ));
                }
                break;
            }
            if result.0 == 0 {
                if !shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                    state.mark_hook_failed(
                        "all-keyboard relay hook message loop exited unexpectedly",
                    );
                }
                break;
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&message);
            let _ = windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&message);
            if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
        }

        state.disable_gate();
        state
            .hook_alive
            .store(false, std::sync::atomic::Ordering::Release);
        let _ = UnhookWindowsHookEx(hook);
        LOW_LEVEL_KEYBOARD_RELAY_STATE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

#[cfg(windows)]
struct RawInputReader {
    receiver: std::sync::mpsc::Receiver<Result<ButtonTransition, String>>,
    window: isize,
    thread_id: u32,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    suppression_hook: Option<KeyboardSuppressionHook>,
    suppression_state: Option<std::sync::Arc<KeyboardDuplicateState>>,
    relay_hook: Option<KeyboardRelayHook>,
    relay_state: Option<std::sync::Arc<KeyboardRelayHookState>>,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawInputThreadReady {
    window: isize,
    thread_id: u32,
}

#[cfg(windows)]
const RAW_INPUT_STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[cfg(windows)]
const RAW_INPUT_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

#[cfg(windows)]
pub(crate) fn await_raw_input_startup(
    receiver: &std::sync::mpsc::Receiver<Result<RawInputThreadReady, String>>,
    timeout: std::time::Duration,
) -> Result<RawInputThreadReady, PlatformError> {
    match receiver.recv_timeout(timeout) {
        Ok(Ok(ready)) => Ok(ready),
        Ok(Err(message)) => Err(PlatformError::InputOpen { message }),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(PlatformError::InputOpen {
            message: format!(
                "Raw Input thread startup timed out after {} ms",
                timeout.as_millis()
            ),
        }),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(PlatformError::InputOpen {
            message: "Raw Input thread disconnected before initialization".to_owned(),
        }),
    }
}

#[cfg(windows)]
pub(crate) fn receive_raw_input_transition(
    receiver: &std::sync::mpsc::Receiver<Result<ButtonTransition, String>>,
    timeout_ms: i32,
) -> Result<Option<ButtonTransition>, PlatformError> {
    let timeout = std::time::Duration::from_millis(timeout_ms.max(0) as u64);
    match receiver.recv_timeout(timeout) {
        Ok(Ok(transition)) => Ok(Some(transition)),
        Ok(Err(message)) => Err(PlatformError::InputRead { message }),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(PlatformError::InputRead {
            message: "Raw Input thread disconnected".to_owned(),
        }),
    }
}

#[cfg(windows)]
pub(crate) fn join_raw_input_thread(
    thread: std::thread::JoinHandle<()>,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now().checked_add(timeout);
    let mut thread = Some(thread);
    while let Some(handle) = thread.as_ref() {
        if handle.is_finished() {
            return thread.take().expect("thread handle exists").join().is_ok();
        }

        let Some(deadline) = deadline else {
            drop(thread.take());
            return false;
        };
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            drop(thread.take());
            return false;
        }
        std::thread::sleep(remaining.min(std::time::Duration::from_millis(1)));
    }
    false
}

#[cfg(windows)]
impl RawInputReader {
    fn new(
        expected_path: String,
        suppress_legacy: bool,
        all_keyboard_relay: bool,
        observe_only: bool,
    ) -> Result<Self, PlatformError> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let (event_sender, event_receiver) = std::sync::mpsc::channel();
        let shutdown_requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (suppression_hook, suppression_state) = if suppress_legacy {
            let state = KeyboardDuplicateState::new();
            let hook = KeyboardSuppressionHook::new(state.clone(), shutdown_requested.clone())?;
            (Some(hook), Some(state))
        } else {
            (None, None)
        };
        let (relay_hook, relay_state) = if all_keyboard_relay
            && !observe_only
            && keyboard_relay_windows::relay_gate_opt_in_requested()
        {
            let state = KeyboardRelayHookState::new();
            let hook = KeyboardRelayHook::new(state.clone(), shutdown_requested.clone())?;
            (Some(hook), Some(state))
        } else {
            if all_keyboard_relay {
                let mode = if observe_only {
                    "observation_only"
                } else {
                    "raw_input_registration_only"
                };
                write_relay_trace(&format!("event=keyboard_relay_gate disabled mode={mode}"));
            }
            (None, None)
        };
        let worker_suppression_state = suppression_state.clone();
        let worker_relay_state = relay_state.clone();
        let worker_shutdown_requested = shutdown_requested.clone();
        let panic_ready_sender = ready_sender.clone();
        let panic_event_sender = event_sender.clone();
        let thread = std::thread::Builder::new()
            .name("redsamurai-raw-input".to_owned())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    raw_input_thread(
                        expected_path,
                        event_sender,
                        ready_sender,
                        worker_shutdown_requested,
                        worker_suppression_state,
                        worker_relay_state,
                        all_keyboard_relay,
                        observe_only,
                    )
                }));
                if result.is_err() {
                    let message = "Raw Input thread panicked during initialization or shutdown";
                    let _ = panic_ready_sender.try_send(Err(message.to_owned()));
                    let _ = panic_event_sender.send(Err(message.to_owned()));
                }
            })
            .map_err(|error| PlatformError::InputOpen {
                message: format!("starting the Raw Input thread failed: {error}"),
            })?;

        let startup = match await_raw_input_startup(&ready_receiver, RAW_INPUT_STARTUP_TIMEOUT) {
            Ok(startup) => startup,
            Err(error) => {
                shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
                if let Some(hook) = suppression_hook.as_ref() {
                    hook.request_shutdown();
                }
                if let Some(hook) = relay_hook.as_ref() {
                    hook.request_shutdown();
                }
                let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
                return Err(error);
            }
        };

        Ok(Self {
            receiver: event_receiver,
            window: startup.window,
            thread_id: startup.thread_id,
            shutdown_requested,
            thread: Some(thread),
            suppression_hook,
            suppression_state,
            relay_hook,
            relay_state,
        })
    }

    fn request_shutdown(&self) {
        self.shutdown_requested
            .store(true, std::sync::atomic::Ordering::Release);

        let mut posted_to_window = false;
        if self.window != 0 {
            let hwnd = windows::Win32::Foundation::HWND(self.window as *mut std::ffi::c_void);
            posted_to_window = unsafe {
                windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                )
                .is_ok()
            };
        }

        if !posted_to_window && self.thread_id != 0 {
            let _ = unsafe {
                windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                    self.thread_id,
                    windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                )
            };
        }
        if let Some(hook) = self.suppression_hook.as_ref() {
            hook.request_shutdown();
        }
        if let Some(hook) = self.relay_hook.as_ref() {
            hook.request_shutdown();
        }
    }

    fn read_transition(&self, timeout_ms: i32) -> Result<Option<ButtonTransition>, PlatformError> {
        if let Some(state) = self.suppression_state.as_ref() {
            state.ensure_hook_alive()?;
        }
        if let Some(hook) = self.relay_hook.as_ref() {
            hook.ensure_alive()?;
        }
        receive_raw_input_transition(&self.receiver, timeout_ms)
    }
}

#[cfg(windows)]
impl Drop for RawInputReader {
    fn drop(&mut self) {
        self.request_shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = join_raw_input_thread(thread, RAW_INPUT_SHUTDOWN_TIMEOUT);
        }
        if let Some(hook) = self.suppression_hook.take() {
            drop(hook);
        }
        if let Some(hook) = self.relay_hook.take() {
            drop(hook);
        }
        self.suppression_state.take();
        self.relay_state.take();
    }
}

#[cfg(windows)]
struct RawInputWindowState {
    expected_path: String,
    sender: std::sync::mpsc::Sender<Result<ButtonTransition, String>>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    suppression_state: Option<std::sync::Arc<KeyboardDuplicateState>>,
    relay_state: Option<RawInputRelayState>,
}

#[cfg(windows)]
struct RawInputRelayState {
    core: keyboard_relay::KeyboardRelay,
    hook_state: Option<std::sync::Arc<KeyboardRelayHookState>>,
    active: bool,
    probe_arm: Option<ProbeArmKey>,
    observe_only: bool,
}

#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProbeArmKey {
    device_id: keyboard_relay::DeviceId,
    input_key: keyboard_relay::InputKey,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug)]
struct PendingRelayEcho {
    virtual_key: u16,
    scan_code: u16,
    flags: u16,
    created_at: std::time::Instant,
}

#[cfg(windows)]
impl PendingRelayEcho {
    fn from_fields(virtual_key: u16, scan_code: u16, replay_flags: u32) -> Self {
        let mut flags = 0u16;
        if replay_flags & keyboard_relay_windows::KeyboardReplay::EXTENDED != 0 {
            flags |= keyboard_relay::RAW_KEY_E0;
        }
        if replay_flags & keyboard_relay_windows::KeyboardReplay::KEYUP != 0 {
            flags |= keyboard_relay::RAW_KEY_BREAK;
        }
        Self {
            virtual_key,
            scan_code,
            flags,
            created_at: std::time::Instant::now(),
        }
    }

    fn matches(&self, sample: &keyboard_relay::RawKeyboardSample) -> bool {
        if self.flags & keyboard_relay::RAW_KEY_BREAK
            != sample.flags & keyboard_relay::RAW_KEY_BREAK
        {
            return false;
        }
        if self.flags & (keyboard_relay::RAW_KEY_E0 | keyboard_relay::RAW_KEY_E1)
            != sample.flags & (keyboard_relay::RAW_KEY_E0 | keyboard_relay::RAW_KEY_E1)
        {
            return false;
        }
        if self.scan_code != 0 && sample.scan_code != 0 && self.scan_code != sample.scan_code {
            return false;
        }
        self.virtual_key == 0 || sample.virtual_key == 0 || self.virtual_key == sample.virtual_key
    }

    fn matches_fields(&self, virtual_key: u16, scan_code: u16, replay_flags: u32) -> bool {
        Self::from_fields(virtual_key, scan_code, replay_flags).same_identity(self)
    }

    fn same_identity(&self, other: &Self) -> bool {
        self.virtual_key == other.virtual_key
            && self.scan_code == other.scan_code
            && self.flags == other.flags
    }
}

#[cfg(windows)]
static RELAY_PENDING_SELF_ECHOES: std::sync::OnceLock<
    std::sync::Mutex<std::collections::VecDeque<PendingRelayEcho>>,
> = std::sync::OnceLock::new();

#[cfg(windows)]
fn pending_self_echoes() -> &'static std::sync::Mutex<std::collections::VecDeque<PendingRelayEcho>>
{
    RELAY_PENDING_SELF_ECHOES
        .get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

#[cfg(windows)]
fn remember_pending_keyboard_input(virtual_key: u16, scan_code: u16, replay_flags: u32) {
    const MAX_PENDING_SELF_ECHOES: usize = 128;
    let mut pending = match pending_self_echoes().lock() {
        Ok(pending) => pending,
        Err(poisoned) => poisoned.into_inner(),
    };
    let now = std::time::Instant::now();
    pending.retain(|entry| {
        now.duration_since(entry.created_at) <= std::time::Duration::from_millis(250)
    });
    if pending.len() >= MAX_PENDING_SELF_ECHOES {
        pending.pop_front();
    }
    pending.push_back(PendingRelayEcho::from_fields(
        virtual_key,
        scan_code,
        replay_flags,
    ));
}

#[cfg(windows)]
fn forget_pending_keyboard_input(virtual_key: u16, scan_code: u16, replay_flags: u32) {
    let mut pending = match pending_self_echoes().lock() {
        Ok(pending) => pending,
        Err(poisoned) => poisoned.into_inner(),
    };
    let Some(index) = pending
        .iter()
        .rposition(|entry| entry.matches_fields(virtual_key, scan_code, replay_flags))
    else {
        return;
    };
    pending.remove(index);
}

#[cfg(windows)]
fn consume_pending_self_echo(sample: &keyboard_relay::RawKeyboardSample) -> bool {
    let mut pending = match pending_self_echoes().lock() {
        Ok(pending) => pending,
        Err(poisoned) => poisoned.into_inner(),
    };
    let now = std::time::Instant::now();
    pending.retain(|entry| {
        now.duration_since(entry.created_at) <= std::time::Duration::from_millis(250)
    });
    let Some(index) = pending.iter().position(|entry| entry.matches(sample)) else {
        return false;
    };
    pending.remove(index).is_some()
}

#[cfg(windows)]
fn clear_pending_self_echoes() {
    let mut pending = match pending_self_echoes().lock() {
        Ok(pending) => pending,
        Err(poisoned) => poisoned.into_inner(),
    };
    pending.clear();
}

#[cfg(windows)]
impl RawInputRelayState {
    fn new(
        expected_path: String,
        hook_state: Option<std::sync::Arc<KeyboardRelayHookState>>,
        observe_only: bool,
    ) -> Self {
        clear_pending_self_echoes();
        let core = keyboard_relay::KeyboardRelay::new([expected_path.as_str()]);
        Self {
            core,
            hook_state,
            active: false,
            probe_arm: None,
            observe_only,
        }
    }

    fn observe_probe_edge(&mut self, sample: &keyboard_relay::RawKeyboardSample) -> bool {
        if sample.is_down() {
            let source_is_ordinary = self.core.classify_device(sample.device_id.clone())
                == keyboard_relay::DeviceClass::Ordinary;
            if source_is_ordinary
                && relay_probe_arm_key_matches(sample.virtual_key, sample.scan_code, sample.flags)
            {
                self.probe_arm = Some(ProbeArmKey {
                    device_id: sample.device_id.clone(),
                    input_key: sample.input_key(),
                });
            }
            return false;
        }

        let matches = self.probe_arm.as_ref().is_some_and(|arm| {
            arm.device_id == sample.device_id && arm.input_key == sample.input_key()
        });
        // An unmatched release must not leave a stale arm that could activate
        // on a later, unrelated key-up.  The next supported key-down can arm
        // the probe again.
        self.probe_arm = None;
        matches
    }

    fn clear_probe_arm(&mut self) {
        self.probe_arm = None;
    }

    fn remember_replay(&mut self, replay: keyboard_relay_windows::KeyboardReplay) {
        remember_pending_keyboard_input(replay.virtual_key(), replay.scan_code(), replay.flags());
    }

    fn consume_unknown_self_echo(&mut self, sample: &keyboard_relay::RawKeyboardSample) -> bool {
        consume_pending_self_echo(sample)
    }
}

/// Return true for the harmless A-key identity used to arm the relay probe.
/// The probe is intentionally limited to this ordinary, non-extended key so
/// window-switching chords such as Alt+Tab cannot activate the usage-wide
/// `RIDEV_NOLEGACY` registration before the operator's deliberate probe.
#[cfg(windows)]
pub(crate) const fn relay_probe_arm_key_matches(
    virtual_key: u16,
    scan_code: u16,
    flags: u16,
) -> bool {
    virtual_key == 0x0041
        && scan_code == 0x001E
        && flags & (keyboard_relay::RAW_KEY_E0 | keyboard_relay::RAW_KEY_E1) == 0
}

#[cfg(windows)]
pub(crate) fn relay_identity_failure_requires_probe(message: &str) -> bool {
    message == "Raw Input device identity was unavailable"
}

#[cfg(all(test, windows))]
mod relay_echo_tests {
    use super::*;

    #[test]
    fn probe_arm_requires_the_deliberate_a_pair_from_one_device() {
        let mut relay = RawInputRelayState::new("target".to_owned(), None, false);

        let alt_down = keyboard_relay::RawKeyboardSample::new("ordinary", 0x12, 0x38, 0, 0);
        let target_a_down = keyboard_relay::RawKeyboardSample::new("target", 0x41, 0x1E, 0, 0);
        let tab_up = keyboard_relay::RawKeyboardSample::new(
            "ordinary",
            0x09,
            0x0F,
            keyboard_relay::RAW_KEY_BREAK,
            1,
        );
        assert!(!relay.observe_probe_edge(&alt_down));
        assert!(!relay.observe_probe_edge(&target_a_down));
        assert!(!relay.observe_probe_edge(&tab_up));

        let a_down = keyboard_relay::RawKeyboardSample::new("ordinary", 0x41, 0x1E, 0, 2);
        let other_a_up = keyboard_relay::RawKeyboardSample::new(
            "other-keyboard",
            0x41,
            0x1E,
            keyboard_relay::RAW_KEY_BREAK,
            3,
        );
        let a_up = keyboard_relay::RawKeyboardSample::new(
            "ordinary",
            0x41,
            0x1E,
            keyboard_relay::RAW_KEY_BREAK,
            4,
        );
        assert!(!relay.observe_probe_edge(&a_down));
        assert!(!relay.observe_probe_edge(&other_a_up));
        assert!(!relay.observe_probe_edge(&a_down));
        assert!(relay.observe_probe_edge(&a_up));
    }

    #[test]
    fn observation_only_relay_state_is_explicitly_non_delivering() {
        let relay = RawInputRelayState::new("target".to_owned(), None, true);
        assert!(relay.observe_only);
    }

    #[test]
    fn observation_only_raw_event_does_not_emit_action_or_forwarded_input() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let shutdown_requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut state = RawInputWindowState {
            expected_path: "target".to_owned(),
            sender,
            shutdown_requested,
            suppression_state: None,
            relay_state: Some(RawInputRelayState::new("target".to_owned(), None, true)),
        };
        let relay_sample = keyboard_relay::RawKeyboardSample::new("target", 0x31, 0x1E, 0, 0);
        let event = RawInputKeyboardEvent {
            transition: Some(ButtonTransition::pressed(0x1E)),
            sample: crate::keyboard_suppression::RawKeyboardSample::new(0x31, 0x1E, 0, 0),
            relay_sample,
            device_identity_available: true,
        };

        assert!(relay_handle_raw_event(&mut state, event).is_ok());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn pending_keyboard_echo_matches_only_the_same_edge_and_identity() {
        let pending = PendingRelayEcho::from_fields(0x41, 0x1E, 0);
        let matching = keyboard_relay::RawKeyboardSample::new("", 0x41, 0x1E, 0, 0);
        assert!(pending.matches(&matching));

        let release = keyboard_relay::RawKeyboardSample::new(
            "",
            0x41,
            0x1E,
            keyboard_relay::RAW_KEY_BREAK,
            0,
        );
        assert!(!pending.matches(&release));

        let different_key = keyboard_relay::RawKeyboardSample::new("", 0x42, 0x30, 0, 0);
        assert!(!pending.matches(&different_key));
    }
}

#[cfg(windows)]
const RAW_INPUT_WINDOW_CLASS: &[u16] = &[
    b'R' as u16,
    b'e' as u16,
    b'd' as u16,
    b'S' as u16,
    b'a' as u16,
    b'm' as u16,
    b'u' as u16,
    b'r' as u16,
    b'a' as u16,
    b'i' as u16,
    b'R' as u16,
    b'a' as u16,
    b'w' as u16,
    b'I' as u16,
    b'n' as u16,
    b'p' as u16,
    b'u' as u16,
    b't' as u16,
    0,
];

#[cfg(windows)]
fn raw_input_thread(
    expected_path: String,
    sender: std::sync::mpsc::Sender<Result<ButtonTransition, String>>,
    ready_sender: std::sync::mpsc::SyncSender<Result<RawInputThreadReady, String>>,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    suppression_state: Option<std::sync::Arc<KeyboardDuplicateState>>,
    relay_hook_state: Option<std::sync::Arc<KeyboardRelayHookState>>,
    all_keyboard_relay: bool,
    observe_only: bool,
) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GetLastError, ERROR_CLASS_ALREADY_EXISTS, HWND};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::{
        RegisterRawInputDevices, RAWINPUTDEVICE, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, RegisterClassW,
        TranslateMessage, HMENU, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
    };

    unsafe {
        let thread_id = GetCurrentThreadId();
        let hinstance = match GetModuleHandleW(PCWSTR::null()) {
            Ok(hinstance) => hinstance.into(),
            Err(error) => {
                let _ = ready_sender.send(Err(format!("GetModuleHandleW failed: {error}")));
                return;
            }
        };
        if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }
        let class_name = PCWSTR(RAW_INPUT_WINDOW_CLASS.as_ptr());
        let class = WNDCLASSW {
            lpfnWndProc: Some(raw_input_window_proc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };
        let atom = RegisterClassW(&class);
        if atom == 0 {
            let error = GetLastError();
            if error != ERROR_CLASS_ALREADY_EXISTS {
                let _ = ready_sender.send(Err(format!(
                    "RegisterClassW failed (Win32 error {})",
                    error.0
                )));
                return;
            }
        }

        let mut state = Box::new(RawInputWindowState {
            expected_path: normalize_raw_input_device_path(&expected_path),
            sender,
            shutdown_requested: shutdown_requested.clone(),
            suppression_state,
            relay_state: all_keyboard_relay.then(|| {
                RawInputRelayState::new(
                    normalize_raw_input_device_path(&expected_path),
                    relay_hook_state,
                    observe_only,
                )
            }),
        });
        let state_ptr = (&mut *state as *mut RawInputWindowState).cast();
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            HMENU(std::ptr::null_mut()),
            hinstance,
            Some(state_ptr),
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                let _ = ready_sender.send(Err(format!("CreateWindowExW failed: {error}")));
                return;
            }
        };

        if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            let _ = DestroyWindow(hwnd);
            return;
        }

        let raw_device = if state.relay_state.is_some() {
            RAWINPUTDEVICE {
                usUsagePage: keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
                usUsage: keyboard_relay_windows::KEYBOARD_USAGE,
                dwFlags: windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS(
                    keyboard_relay_windows::RAW_INPUT_PROBE_REGISTER_FLAGS,
                ),
                hwndTarget: hwnd,
            }
        } else {
            RAWINPUTDEVICE {
                usUsagePage: RESIDENT_INPUT_USAGE_PAGE,
                usUsage: RESIDENT_INPUT_USAGE,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: hwnd,
            }
        };
        if let Err(error) =
            RegisterRawInputDevices(&[raw_device], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
        {
            let _ = DestroyWindow(hwnd);
            let _ = ready_sender.send(Err(format!("RegisterRawInputDevices failed: {error}")));
            return;
        }
        if state.relay_state.is_some() {
            write_relay_trace(&format!(
                "event=keyboard_relay_probe_registered usage_page=0x{:04X} usage=0x{:04X} flags=0x{:04X}",
                keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
                keyboard_relay_windows::KEYBOARD_USAGE,
                keyboard_relay_windows::RAW_INPUT_PROBE_REGISTER_FLAGS
            ));
        }

        if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            unregister_raw_input_devices(state.relay_state.is_some());
            let _ = DestroyWindow(hwnd);
            return;
        }

        if ready_sender
            .send(Ok(RawInputThreadReady {
                window: hwnd.0 as isize,
                thread_id,
            }))
            .is_err()
        {
            shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
        }
        if shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
            unregister_raw_input_devices(state.relay_state.is_some());
            let _ = DestroyWindow(hwnd);
            return;
        }

        let mut message_loop_error = None;
        let mut message = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        loop {
            let result = GetMessageW(&mut message, HWND(0 as *mut std::ffi::c_void), 0, 0);
            if result.0 == -1 {
                message_loop_error = Some(format!(
                    "GetMessageW failed (Win32 error {})",
                    GetLastError().0
                ));
                break;
            }
            if result.0 == 0 {
                if !shutdown_requested.load(std::sync::atomic::Ordering::Acquire) {
                    message_loop_error =
                        Some("Raw Input message loop exited unexpectedly".to_owned());
                }
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if let Some(error) = message_loop_error {
            shutdown_requested.store(true, std::sync::atomic::Ordering::Release);
            if let Some(relay) = state.relay_state.as_ref() {
                if let Some(hook_state) = relay.hook_state.as_ref() {
                    hook_state.mark_hook_failed(error.clone());
                }
            }
            let _ = state.sender.send(Err(error));
        }
        if state.relay_state.is_some() {
            if let Err(message) = relay_handle_shutdown(&mut *state) {
                if let Some(relay) = state.relay_state.as_ref() {
                    if let Some(hook_state) = relay.hook_state.as_ref() {
                        hook_state.mark_hook_failed(message.clone());
                    }
                }
                let _ = state.sender.send(Err(message));
            }
        }
        if let Some(relay) = state.relay_state.as_ref() {
            if let Some(hook_state) = relay.hook_state.as_ref() {
                hook_state.disable_gate();
            }
        }
        unregister_raw_input_devices(state.relay_state.is_some());
        if state.relay_state.is_some() {
            write_relay_trace("event=keyboard_relay_unregistered");
        }
        let _ = DestroyWindow(hwnd);
        drop(state);
    }
}

#[cfg(windows)]
unsafe fn unregister_raw_input_devices(all_keyboard_relay: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::{RegisterRawInputDevices, RAWINPUTDEVICE, RIDEV_REMOVE};

    let (usage_page, usage) = if all_keyboard_relay {
        (
            keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
            keyboard_relay_windows::KEYBOARD_USAGE,
        )
    } else {
        (RESIDENT_INPUT_USAGE_PAGE, RESIDENT_INPUT_USAGE)
    };
    let _ = RegisterRawInputDevices(
        &[RAWINPUTDEVICE {
            usUsagePage: usage_page,
            usUsage: usage,
            dwFlags: RIDEV_REMOVE,
            hwndTarget: HWND(std::ptr::null_mut()),
        }],
        std::mem::size_of::<RAWINPUTDEVICE>() as u32,
    );
}

#[cfg(windows)]
unsafe extern "system" fn raw_input_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    _wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, GetWindowLongPtrW, PostQuitMessage, SetWindowLongPtrW,
        CREATESTRUCTW, GIDC_ARRIVAL, GIDC_REMOVAL, GWLP_USERDATA, WM_CLOSE, WM_DESTROY, WM_INPUT,
        WM_INPUT_DEVICE_CHANGE,
    };

    if message == windows::Win32::UI::WindowsAndMessaging::WM_NCCREATE {
        if lparam.0 != 0 {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        return windows::Win32::Foundation::LRESULT(1);
    }

    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RawInputWindowState;
    if message == WM_INPUT && !state.is_null() {
        match read_raw_input_event(
            windows::Win32::UI::Input::HRAWINPUT(lparam.0 as *mut std::ffi::c_void),
            if (*state).relay_state.is_some() {
                None
            } else {
                Some(&(*state).expected_path)
            },
        ) {
            Ok(Some(event)) => {
                if (*state).relay_state.is_some() {
                    let relay_is_active = (*state)
                        .relay_state
                        .as_ref()
                        .map(|relay| relay.active)
                        .unwrap_or(false);
                    if !relay_is_active {
                        let sample = &event.relay_sample;
                        write_relay_trace(&format!(
                            "event=keyboard_relay_probe_event path={} identity_available={} vk=0x{:02X} scan=0x{:02X} flags=0x{:04X}",
                            sample.device_id,
                            event.device_identity_available,
                            sample.virtual_key,
                            sample.scan_code,
                            sample.flags
                        ));
                        let should_activate = event.device_identity_available
                            && (*state)
                                .relay_state
                                .as_mut()
                                .map(|relay| relay.observe_probe_edge(sample))
                                .unwrap_or(false);
                        if should_activate {
                            let activation = activate_keyboard_relay(&mut *state, hwnd);
                            if let Err(message) = activation {
                                if let Err(fallback) = fail_open_keyboard_relay_to_probe(
                                    &mut *state,
                                    hwnd,
                                    &format!("activation failed: {message}"),
                                ) {
                                    if let Some(relay) = (*state).relay_state.as_ref() {
                                        if let Some(hook_state) = relay.hook_state.as_ref() {
                                            hook_state.mark_hook_failed(fallback.clone());
                                        }
                                    }
                                    (*state)
                                        .shutdown_requested
                                        .store(true, std::sync::atomic::Ordering::Release);
                                    let _ = (*state).sender.send(Err(fallback));
                                    PostQuitMessage(0);
                                }
                            } else {
                                write_relay_trace(
                                    "event=keyboard_relay_probe_arm_complete mode=active",
                                );
                            }
                        } else if !event.device_identity_available {
                            write_relay_trace(
                                "event=keyboard_relay_probe_identity_unavailable mode=probe",
                            );
                        }
                        return windows::Win32::Foundation::LRESULT(0);
                    }
                    if !event.device_identity_available {
                        let consumed = (*state)
                            .relay_state
                            .as_mut()
                            .map(|relay| relay.consume_unknown_self_echo(&event.relay_sample))
                            .unwrap_or(false);
                        if consumed {
                            write_relay_trace(&format!(
                                "event=keyboard_relay_self_echo_ignored vk=0x{:02X} scan=0x{:02X} flags=0x{:04X}",
                                event.relay_sample.virtual_key,
                                event.relay_sample.scan_code,
                                event.relay_sample.flags
                            ));
                        } else if let Err(message) = fail_open_keyboard_relay_to_probe(
                            &mut *state,
                            hwnd,
                            "Raw Input device identity was unavailable",
                        ) {
                            if let Some(relay) = (*state).relay_state.as_ref() {
                                if let Some(hook_state) = relay.hook_state.as_ref() {
                                    hook_state.mark_hook_failed(message.clone());
                                }
                            }
                            (*state)
                                .shutdown_requested
                                .store(true, std::sync::atomic::Ordering::Release);
                            let _ = (*state).sender.send(Err(message));
                            PostQuitMessage(0);
                        }
                        return windows::Win32::Foundation::LRESULT(0);
                    }
                    if let Err(message) = relay_handle_raw_event(&mut *state, event) {
                        if let Err(fallback) = fail_open_keyboard_relay_to_probe(
                            &mut *state,
                            hwnd,
                            &format!("relay event failed: {message}"),
                        ) {
                            if let Some(relay) = (*state).relay_state.as_ref() {
                                if let Some(hook_state) = relay.hook_state.as_ref() {
                                    hook_state.mark_hook_failed(fallback.clone());
                                }
                            }
                            (*state)
                                .shutdown_requested
                                .store(true, std::sync::atomic::Ordering::Release);
                            let _ = (*state).sender.send(Err(fallback));
                            PostQuitMessage(0);
                        } else {
                            write_relay_trace(
                                "event=keyboard_relay_fail_open reason=relay_event_failed mode=probe",
                            );
                        }
                    }
                } else {
                    if let Some(suppression_state) = (*state).suppression_state.as_ref() {
                        suppression_state.observe_target(event.sample);
                    }
                    if let Some(transition) = event.transition {
                        let _ = (*state).sender.send(Ok(transition));
                    }
                }
            }
            Ok(None) => {}
            Err(message) => {
                if (*state).relay_state.is_some() && relay_identity_failure_requires_probe(&message)
                {
                    if let Err(fallback) =
                        fail_open_keyboard_relay_to_probe(&mut *state, hwnd, &message)
                    {
                        if let Some(relay) = (*state).relay_state.as_ref() {
                            if let Some(hook_state) = relay.hook_state.as_ref() {
                                hook_state.mark_hook_failed(fallback.clone());
                            }
                        }
                        (*state)
                            .shutdown_requested
                            .store(true, std::sync::atomic::Ordering::Release);
                        let _ = (*state).sender.send(Err(fallback));
                        PostQuitMessage(0);
                    } else {
                        write_relay_trace(
                            "event=keyboard_relay_fail_open reason=device_identity_unavailable mode=probe",
                        );
                    }
                    return windows::Win32::Foundation::LRESULT(0);
                }
                if let Some(relay) = (*state).relay_state.as_ref() {
                    if let Some(hook_state) = relay.hook_state.as_ref() {
                        hook_state.mark_hook_failed(message.clone());
                    }
                }
                (*state)
                    .shutdown_requested
                    .store(true, std::sync::atomic::Ordering::Release);
                let _ = (*state).sender.send(Err(message));
                PostQuitMessage(0);
            }
        }
        return windows::Win32::Foundation::LRESULT(0);
    }
    if message == WM_INPUT_DEVICE_CHANGE && !state.is_null() {
        let device = windows::Win32::Foundation::HANDLE(lparam.0 as *mut std::ffi::c_void);
        if let Some(path) = raw_input_device_name(device) {
            let is_target = raw_input_device_path_matches(&(*state).expected_path, &path);
            if _wparam.0 as u32 == GIDC_ARRIVAL {
                if let Some(relay) = (*state).relay_state.as_mut() {
                    let device_id = normalize_raw_input_device_path(&path);
                    relay.core.mark_device_available(device_id.clone());
                    write_relay_trace(&format!(
                        "event=keyboard_relay_device_arrival path={device_id}"
                    ));
                }
            } else if _wparam.0 as u32 == GIDC_REMOVAL {
                if (*state).relay_state.is_some() {
                    if let Err(message) = relay_handle_device_removed(&mut *state, &path) {
                        if let Some(relay) = (*state).relay_state.as_ref() {
                            if let Some(hook_state) = relay.hook_state.as_ref() {
                                hook_state.mark_hook_failed(message.clone());
                            }
                        }
                        let _ = (*state).sender.send(Err(message));
                    }
                }
                if is_target {
                    (*state)
                        .shutdown_requested
                        .store(true, std::sync::atomic::Ordering::Release);
                    let _ = (*state)
                        .sender
                        .send(Err("resident input device disconnected".to_owned()));
                    PostQuitMessage(0);
                }
            }
        }
        return windows::Win32::Foundation::LRESULT(0);
    }
    if message == WM_CLOSE {
        let _ = DestroyWindow(hwnd);
        return windows::Win32::Foundation::LRESULT(0);
    }
    if message == WM_DESTROY {
        PostQuitMessage(0);
        return windows::Win32::Foundation::LRESULT(0);
    }
    DefWindowProcW(hwnd, message, _wparam, lparam)
}

#[cfg(windows)]
unsafe fn activate_keyboard_relay(
    state: &mut RawInputWindowState,
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<(), String> {
    use windows::Win32::UI::Input::{RegisterRawInputDevices, RAWINPUTDEVICE};

    let Some(relay) = state.relay_state.as_ref() else {
        return Err("all-keyboard relay state was not initialized".to_owned());
    };
    if relay.active {
        return Ok(());
    }

    if relay.observe_only {
        if let Some(relay) = state.relay_state.as_mut() {
            relay.active = true;
        }
        write_relay_trace(&format!(
            "event=keyboard_relay_observe_only_active usage_page=0x{:04X} usage=0x{:04X} flags=0x{:04X}",
            keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
            keyboard_relay_windows::KEYBOARD_USAGE,
            keyboard_relay_windows::RAW_INPUT_PROBE_REGISTER_FLAGS
        ));
        return Ok(());
    }

    RegisterRawInputDevices(
        &[RAWINPUTDEVICE {
            usUsagePage: keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
            usUsage: keyboard_relay_windows::KEYBOARD_USAGE,
            dwFlags: windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS(
                keyboard_relay_windows::RAW_INPUT_REGISTER_FLAGS,
            ),
            hwndTarget: hwnd,
        }],
        std::mem::size_of::<RAWINPUTDEVICE>() as u32,
    )
    .map_err(|error| format!("RegisterRawInputDevices relay activation failed: {error}"))?;

    if let Some(relay) = state.relay_state.as_mut() {
        relay.active = true;
        if let Some(hook_state) = relay.hook_state.as_ref() {
            hook_state.enable_gate();
        }
    }
    write_relay_trace(&format!(
        "event=keyboard_relay_registered usage_page=0x{:04X} usage=0x{:04X} flags=0x{:04X}",
        keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
        keyboard_relay_windows::KEYBOARD_USAGE,
        keyboard_relay_windows::RAW_INPUT_REGISTER_FLAGS
    ));
    write_relay_trace("event=keyboard_relay_activated");
    Ok(())
}

#[cfg(windows)]
unsafe fn fail_open_keyboard_relay_to_probe(
    state: &mut RawInputWindowState,
    hwnd: windows::Win32::Foundation::HWND,
    reason: &str,
) -> Result<(), String> {
    use windows::Win32::UI::Input::{RegisterRawInputDevices, RAWINPUTDEVICE};

    let Some(relay) = state.relay_state.as_ref() else {
        return Ok(());
    };
    let was_active = relay.active;
    let registration = if was_active {
        RegisterRawInputDevices(
            &[RAWINPUTDEVICE {
                usUsagePage: keyboard_relay_windows::KEYBOARD_USAGE_PAGE,
                usUsage: keyboard_relay_windows::KEYBOARD_USAGE,
                dwFlags: windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS(
                    keyboard_relay_windows::RAW_INPUT_PROBE_REGISTER_FLAGS,
                ),
                hwndTarget: hwnd,
            }],
            std::mem::size_of::<RAWINPUTDEVICE>() as u32,
        )
        .map_err(|error| format!("RegisterRawInputDevices fail-open probe failed: {error}"))
    } else {
        Ok(())
    };

    if let Some(relay) = state.relay_state.as_mut() {
        relay.active = false;
        relay.clear_probe_arm();
        clear_pending_self_echoes();
        if let Some(hook_state) = relay.hook_state.as_ref() {
            hook_state.disable_gate();
        }
    }

    if let Err(error) = registration {
        write_relay_trace(&format!(
            "event=keyboard_relay_fail_open_failed reason={} active_was={} error={}",
            reason.replace(['\r', '\n', ' '], "_"),
            was_active,
            error.replace(['\r', '\n', ' '], "_")
        ));
        return Err(error);
    }
    write_relay_trace(&format!(
        "event=keyboard_relay_fail_open reason={} active_was={} mode=probe",
        reason.replace(['\r', '\n', ' '], "_"),
        was_active
    ));
    Ok(())
}

#[cfg(windows)]
fn relay_handle_raw_event(
    state: &mut RawInputWindowState,
    event: RawInputKeyboardEvent,
) -> Result<(), String> {
    let Some(relay) = state.relay_state.as_mut() else {
        return Err("all-keyboard relay state was not initialized".to_owned());
    };

    let sample = event.relay_sample;
    let source_class = relay.core.classify_device(sample.device_id.clone());
    if relay.observe_only {
        if source_class == keyboard_relay::DeviceClass::Target && event.transition.is_some() {
            write_relay_trace(&format!(
                "event=keyboard_relay_target path={} vk=0x{:02X} scan=0x{:02X} flags=0x{:04X} edge={}",
                sample.device_id,
                sample.virtual_key,
                sample.scan_code,
                sample.flags,
                if sample.is_down() { "down" } else { "up" }
            ));
        }
        write_relay_trace(&format!(
            "event=keyboard_relay_observe_only class={:?} path={} vk=0x{:02X} scan=0x{:02X} flags=0x{:04X} edge={}",
            source_class,
            sample.device_id,
            sample.virtual_key,
            sample.scan_code,
            sample.flags,
            if sample.is_down() { "down" } else { "up" }
        ));
        return Ok(());
    }
    if source_class == keyboard_relay::DeviceClass::Target {
        if let Some(transition) = event.transition {
            write_relay_trace(&format!(
                "event=keyboard_relay_target path={} vk=0x{:02X} scan=0x{:02X} flags=0x{:04X} edge={}",
                sample.device_id,
                sample.virtual_key,
                sample.scan_code,
                sample.flags,
                if sample.is_down() { "down" } else { "up" }
            ));
            return state
                .sender
                .send(Ok(transition))
                .map_err(|_| "resident relay event receiver disconnected".to_owned());
        }
    }

    let events = relay.core.handle(sample).into_events();
    relay_deliver_events(state, events)
}

#[cfg(windows)]
fn relay_handle_device_removed(state: &mut RawInputWindowState, path: &str) -> Result<(), String> {
    let device_id = normalize_raw_input_device_path(path);
    let events = {
        let Some(relay) = state.relay_state.as_mut() else {
            return Ok(());
        };
        write_relay_trace(&format!(
            "event=keyboard_relay_device_removal path={device_id}"
        ));
        relay.core.remove_device(device_id).into_events()
    };
    relay_deliver_events(state, events)
}

#[cfg(windows)]
fn relay_handle_shutdown(state: &mut RawInputWindowState) -> Result<(), String> {
    let events = {
        let Some(relay) = state.relay_state.as_mut() else {
            return Ok(());
        };
        relay.core.shutdown().into_events()
    };
    if !events.is_empty() {
        write_relay_trace(&format!(
            "event=keyboard_relay_shutdown_cleanup events={}",
            events.len()
        ));
    }
    relay_deliver_events(state, events)
}

#[cfg(windows)]
fn relay_deliver_events(
    state: &mut RawInputWindowState,
    events: Vec<keyboard_relay::RelayEvent>,
) -> Result<(), String> {
    for output in events {
        match output {
            keyboard_relay::RelayEvent::Forwarded(sample) => {
                write_relay_trace(&format!(
                    "event=keyboard_relay_forward path={} vk=0x{:02X} scan=0x{:02X} flags=0x{:04X} edge={} marker=0x{:016X}",
                    sample.device_id,
                    sample.virtual_key,
                    sample.scan_code,
                    sample.flags,
                    if sample.is_down() { "down" } else { "up" },
                    sample.injection_marker().unwrap_or_default()
                ));
                let replay =
                    keyboard_relay_windows::replay_keyboard_input(&sample).map_err(|error| {
                        format!("building ordinary keyboard replay failed: {error}")
                    })?;
                keyboard_relay_windows::send_replay_input(replay).map_err(|error| {
                    format!("forwarding ordinary keyboard input failed: {error}")
                })?;
                if let Some(relay) = state.relay_state.as_mut() {
                    relay.remember_replay(replay);
                }
            }
            keyboard_relay::RelayEvent::Injected(event) => {
                write_relay_trace(&format!(
                    "event=keyboard_relay_cleanup_output vk=0x{:02X} scan=0x{:02X} phase={:?} marker=0x{:016X}",
                    event.key().vk(),
                    event.key().scan_code(),
                    event.phase(),
                    event.marker()
                ));
                let replay = keyboard_relay_windows::replay_injected_key(event)
                    .map_err(|error| format!("building relay cleanup replay failed: {error}"))?;
                keyboard_relay_windows::send_replay_input(replay)
                    .map_err(|error| format!("sending relay cleanup replay failed: {error}"))?;
                if let Some(relay) = state.relay_state.as_mut() {
                    relay.remember_replay(replay);
                }
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn raw_keyboard_payload_is_sized(
    copied: usize,
    header_size: usize,
    keyboard_size: usize,
) -> bool {
    copied >= header_size.saturating_add(keyboard_size)
}

#[cfg(windows)]
struct RawInputKeyboardEvent {
    transition: Option<ButtonTransition>,
    sample: crate::keyboard_suppression::RawKeyboardSample,
    relay_sample: keyboard_relay::RawKeyboardSample,
    device_identity_available: bool,
}

#[cfg(windows)]
unsafe fn read_raw_input_event(
    raw_handle: windows::Win32::UI::Input::HRAWINPUT,
    expected_path: Option<&str>,
) -> Result<Option<RawInputKeyboardEvent>, String> {
    use windows::Win32::UI::Input::{
        GetRawInputData, RAWINPUTHEADER, RAWKEYBOARD, RID_INPUT, RIM_TYPEKEYBOARD,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetMessageTime;

    let header_size = std::mem::size_of::<RAWINPUTHEADER>();
    let mut size = 0u32;
    let required = GetRawInputData(raw_handle, RID_INPUT, None, &mut size, header_size as u32);
    if required == u32::MAX || (size as usize) < header_size {
        return Err(format!(
            "GetRawInputData size query failed (Win32 error {})",
            windows::Win32::Foundation::GetLastError().0
        ));
    }
    let mut buffer = vec![0u8; size as usize];
    let copied = GetRawInputData(
        raw_handle,
        RID_INPUT,
        Some(buffer.as_mut_ptr().cast()),
        &mut size,
        header_size as u32,
    );
    if copied == u32::MAX {
        return Err(format!(
            "GetRawInputData read failed (Win32 error {})",
            windows::Win32::Foundation::GetLastError().0
        ));
    }

    let copied = copied as usize;
    if copied < header_size {
        return Err(format!(
            "GetRawInputData read returned {copied} bytes; expected at least {header_size}"
        ));
    }

    let header = std::ptr::read_unaligned(buffer.as_ptr().cast::<RAWINPUTHEADER>());
    let trace = std::env::var_os("REDSAMURAI_TRACE_RAW_INPUT").is_some();
    if trace {
        eprintln!(
            "raw_input_trace=header type={} copied={} expected_path={:?}",
            header.dwType, copied, expected_path
        );
    }
    if header.dwType != RIM_TYPEKEYBOARD.0 {
        if trace {
            eprintln!(
                "raw_input_trace=ignored_non_keyboard type={}",
                header.dwType
            );
        }
        return Ok(None);
    }
    let (device_name, normalized_device_name, device_identity_available) =
        match raw_input_device_name(header.hDevice) {
            Some(device_name) => {
                let normalized_device_name = normalize_raw_input_device_path(&device_name);
                if let Some(expected_path) = expected_path {
                    if !raw_input_device_path_matches(expected_path, &device_name) {
                        if trace {
                            eprintln!(
                                "raw_input_trace=path_mismatch observed={:?} expected={:?}",
                                device_name, expected_path
                            );
                        }
                        return Ok(None);
                    }
                }
                (device_name, normalized_device_name, true)
            }
            None => {
                if trace {
                    eprintln!("raw_input_trace=device_name_unavailable");
                }
                if expected_path.is_some() {
                    return Ok(None);
                }
                // A SendInput replay can reappear as a keyboard Raw Input
                // record without a resolvable hDevice. Keep the packet so
                // the relay can match it against its pending self-echo
                // ledger instead of tearing down the whole message loop.
                ("<unidentified>".to_owned(), String::new(), false)
            }
        };

    let keyboard_size = std::mem::size_of::<RAWKEYBOARD>();
    if !raw_keyboard_payload_is_sized(copied, header_size, keyboard_size) {
        return Err(format!(
            "GetRawInputData keyboard payload was truncated: {copied} bytes, expected at least {}",
            header_size.saturating_add(keyboard_size)
        ));
    }
    let keyboard = std::ptr::read_unaligned(buffer.as_ptr().add(header_size).cast::<RAWKEYBOARD>());
    let transition = map_raw_keyboard_event(keyboard.VKey, keyboard.Flags);
    let sample = crate::keyboard_suppression::RawKeyboardSample::new(
        keyboard.VKey,
        keyboard.MakeCode,
        keyboard.Flags,
        GetMessageTime() as u32,
    );
    let relay_sample = keyboard_relay_windows::raw_keyboard_to_relay_sample(
        normalized_device_name.clone(),
        keyboard_relay_windows::RawKeyboardPacket::new(
            keyboard.VKey,
            keyboard.MakeCode,
            keyboard.Flags,
            GetMessageTime().max(0) as u64,
        ),
    );
    if trace {
        eprintln!(
            "raw_input_trace=keyboard path={:?} vkey=0x{:02X} flags=0x{:04X} mapped={}",
            device_name,
            keyboard.VKey,
            keyboard.Flags,
            transition.is_some()
        );
    }
    Ok(Some(RawInputKeyboardEvent {
        transition,
        sample,
        relay_sample,
        device_identity_available,
    }))
}

#[cfg(windows)]
unsafe fn raw_input_device_name(device: windows::Win32::Foundation::HANDLE) -> Option<String> {
    use windows::Win32::UI::Input::{GetRawInputDeviceInfoW, RIDI_DEVICENAME};

    let mut length = 0u32;
    let required = GetRawInputDeviceInfoW(device, RIDI_DEVICENAME, None, &mut length);
    if required == u32::MAX || length == 0 {
        return None;
    }
    let mut buffer = vec![0u16; length.max(required) as usize + 1];
    let copied = GetRawInputDeviceInfoW(
        device,
        RIDI_DEVICENAME,
        Some(buffer.as_mut_ptr().cast()),
        &mut length,
    );
    if copied == u32::MAX {
        return None;
    }
    buffer.truncate(copied as usize);
    Some(String::from_utf16_lossy(&buffer))
}

#[cfg(windows)]
/// An opened resident input collection backed by Windows Raw Input.
pub struct ResidentInputDevice {
    reader: RawInputReader,
}

#[cfg(windows)]
impl fmt::Debug for ResidentInputDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResidentInputDevice")
            .finish_non_exhaustive()
    }
}

#[cfg(windows)]
impl ResidentInputDevice {
    /// Read and parse one bounded report.  A zero-length read is a timeout,
    /// not an input event.
    pub fn read_report(
        &self,
        timeout_ms: i32,
    ) -> Result<Option<ResidentInputReport>, PlatformError> {
        let Some(transition) = self.reader.read_transition(timeout_ms)? else {
            return Ok(None);
        };
        let report = raw_input_transition_report(transition);
        parse_resident_input_report(&report).map(Some)
    }

    /// Read one report and return only button edges since the previous
    /// successfully parsed report.
    pub fn read_transitions(
        &mut self,
        timeout_ms: i32,
    ) -> Result<Option<Vec<ButtonTransition>>, PlatformError> {
        self.reader
            .read_transition(timeout_ms)
            .map(|transition| transition.map(|transition| vec![transition]))
    }
}

#[cfg(windows)]
/// Open the first exact resident input collection through Raw Input.  The
/// selected HID path is still discovered with `hidapi`, but no direct
/// keyboard-class handle is opened because Windows rejects synchronous reads
/// from that collection with `ERROR_ACCESS_DENIED`.
pub fn open_resident_input_device() -> Result<ResidentInputDevice, PlatformError> {
    open_resident_input_device_internal(false, false, false)
}

#[cfg(windows)]
/// Open the resident collection for the long-lived service.
///
/// The diagnostic all-keyboard relay is deliberately opt-in through
/// `REDSAMURAI_ENABLE_KEYBOARD_RELAY=1`. It is the only service mode that
/// registers `RIDEV_NOLEGACY`; the normal service remains read-only for the
/// exact target collection. The rejected device-specific correlation hook is
/// still available separately through
/// `REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL=1` for diagnostics,
/// but cannot be combined with the relay. The all-keyboard relay itself uses
/// registration-only Raw Input and marked `SendInput` replay by default; the
/// separate `REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1` switch is required for
/// the experimental low-level suppression gate. The validation-only
/// `REDSAMURAI_KEYBOARD_RELAY_OBSERVE_ONLY=1` switch keeps the legacy-safe
/// registration and records events without replay or actions. Read-only probes
/// must use [`open_resident_input_device`] so merely observing the collection
/// never changes foreground keyboard delivery.
pub(crate) fn open_resident_input_device_for_service() -> Result<ResidentInputDevice, PlatformError>
{
    let all_keyboard_relay = keyboard_relay_windows::relay_opt_in_requested();
    let observe_only = all_keyboard_relay && keyboard_relay_windows::relay_observe_only_requested();
    let experimental = std::env::var("REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL")
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if all_keyboard_relay && experimental {
        return Err(PlatformError::InputOpen {
            message: "all-keyboard relay cannot be combined with experimental keyboard suppression"
                .to_owned(),
        });
    }
    open_resident_input_device_internal(experimental, all_keyboard_relay, observe_only)
}

#[cfg(windows)]
fn open_resident_input_device_internal(
    suppress_legacy: bool,
    all_keyboard_relay: bool,
    observe_only: bool,
) -> Result<ResidentInputDevice, PlatformError> {
    let api = hidapi::HidApi::new().map_err(|error| PlatformError::Discovery {
        message: error.to_string(),
    })?;
    let info = api
        .device_list()
        .find(|info| {
            let candidate = ResidentInputCandidate::new(
                info.path().to_string_lossy(),
                info.vendor_id(),
                info.product_id(),
                info.usage_page(),
                info.usage(),
                info.interface_number(),
                RESIDENT_INPUT_REPORT_LENGTH,
            );
            is_resident_input_interface(&candidate)
        })
        .ok_or(PlatformError::InputUnavailable)?;
    let path = info.path().to_string_lossy().into_owned();
    let reader = RawInputReader::new(path, suppress_legacy, all_keyboard_relay, observe_only)?;
    Ok(ResidentInputDevice { reader })
}

#[cfg(not(windows))]
/// Resident HID input is unavailable on non-Windows platforms.
pub fn open_resident_input_device() -> Result<(), PlatformError> {
    Err(PlatformError::UnsupportedPlatform)
}

/// Short name for opening the resident input adapter.
#[cfg(windows)]
pub fn open_resident_input() -> Result<ResidentInputDevice, PlatformError> {
    open_resident_input_device()
}

/// Short name for opening the resident input adapter on unsupported builds.
#[cfg(not(windows))]
pub fn open_resident_input() -> Result<(), PlatformError> {
    open_resident_input_device()
}

/// A stateless namespace for the resident platform boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidentPlatform;

impl ResidentPlatform {
    /// Enumerate exact input collections without opening them.
    pub fn discover_input_interfaces() -> Result<Vec<ResidentInputCandidate>, PlatformError> {
        discover_resident_input_interfaces()
    }

    /// Send one keyboard event through the software output boundary.
    pub fn send_keyboard(event: KeyboardEvent) -> Result<(), PlatformError> {
        send_keyboard_event(event)
    }

    /// Send one mouse event through the software output boundary.
    pub fn send_mouse(event: MouseEvent) -> Result<(), PlatformError> {
        send_mouse_event(event)
    }

    /// Send one media event through the software output boundary.
    pub fn send_media(event: MediaEvent) -> Result<(), PlatformError> {
        send_media_event(event)
    }
}

/// Compatibility alias for code that names the boundary as an adapter.
pub type ResidentPlatformAdapter = ResidentPlatform;
