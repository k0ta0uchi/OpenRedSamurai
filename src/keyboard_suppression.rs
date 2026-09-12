//! Device-correlated filtering for the keyboard-class duplicate path.
//!
//! The mouse exposes its side controls through a keyboard top-level
//! collection. Windows therefore reports the same physical edge to Raw Input
//! and to the legacy keyboard path. This module contains the small, platform
//! neutral correlation state machine used by the Windows hook boundary: only
//! a legacy edge that matches a recently observed edge from the exact target
//! Raw Input device is suppressible.

use std::collections::VecDeque;

/// Raw keyboard flag for a break/release edge (`RI_KEY_BREAK`).
pub const RAW_KEY_BREAK: u16 = 0x0001;
/// Raw keyboard flag for an extended `E0` edge (`RI_KEY_E0`).
pub const RAW_KEY_E0: u16 = 0x0002;
/// Raw keyboard flag for an extended `E1` edge (`RI_KEY_E1`).
pub const RAW_KEY_E1: u16 = 0x0004;
/// Low-level hook flag for a key-up edge (`LLKHF_UP`).
pub const LOW_LEVEL_KEY_UP: u32 = 0x0080;
/// Low-level hook flag for an injected edge (`LLKHF_INJECTED`).
pub const LOW_LEVEL_KEY_INJECTED: u32 = 0x0010;
/// Low-level hook flag for an extended edge (`LLKHF_EXTENDED`).
pub const LOW_LEVEL_KEY_EXTENDED: u32 = 0x0001;

/// One target-device keyboard edge observed through Raw Input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKeyboardSample {
    pub virtual_key: u16,
    pub scan_code: u16,
    pub flags: u16,
    pub timestamp_ms: u32,
}

impl RawKeyboardSample {
    /// Construct a raw target-device edge using the `RAWKEYBOARD` fields that
    /// are stable across the Raw Input and low-level hook boundaries.
    pub const fn new(virtual_key: u16, scan_code: u16, flags: u16, timestamp_ms: u32) -> Self {
        Self {
            virtual_key,
            scan_code,
            flags,
            timestamp_ms,
        }
    }

    const fn is_break(self) -> bool {
        self.flags & RAW_KEY_BREAK != 0
    }

    const fn is_extended(self) -> bool {
        self.flags & (RAW_KEY_E0 | RAW_KEY_E1) != 0
    }
}

/// One event presented to the global low-level keyboard hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacyKeyboardSample {
    pub virtual_key: u16,
    pub scan_code: u32,
    pub flags: u32,
    pub timestamp_ms: u32,
}

impl LegacyKeyboardSample {
    /// Construct a low-level hook event from `KBDLLHOOKSTRUCT` fields.
    pub const fn new(virtual_key: u16, scan_code: u32, flags: u32, timestamp_ms: u32) -> Self {
        Self {
            virtual_key,
            scan_code,
            flags,
            timestamp_ms,
        }
    }

    const fn is_break(self) -> bool {
        self.flags & LOW_LEVEL_KEY_UP != 0
    }

    const fn is_injected(self) -> bool {
        self.flags & LOW_LEVEL_KEY_INJECTED != 0
    }

    const fn is_extended(self) -> bool {
        self.flags & LOW_LEVEL_KEY_EXTENDED != 0
    }
}

/// Result of classifying a low-level hook event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardFilterDecision {
    /// Return a non-zero low-level hook result and stop legacy delivery.
    Suppress,
    /// Call the next hook and preserve ordinary keyboard delivery.
    Pass,
}

/// Bounded correlation state for target Raw Input and legacy hook edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardDuplicateFilter {
    match_window_ms: u32,
    pending: VecDeque<RawKeyboardSample>,
}

impl Default for KeyboardDuplicateFilter {
    fn default() -> Self {
        Self::new(8)
    }
}

impl KeyboardDuplicateFilter {
    /// Maximum number of target observations retained while hook callbacks
    /// catch up. A bounded queue prevents a broken hook or device from growing
    /// resident memory without limit.
    const MAX_PENDING: usize = 64;

    /// Construct a filter with a symmetric event-time matching window.
    pub const fn new(match_window_ms: u32) -> Self {
        Self {
            match_window_ms,
            pending: VecDeque::new(),
        }
    }

    /// Record one edge from the exact target Raw Input collection.
    pub fn observe_target(&mut self, sample: RawKeyboardSample) {
        if self.pending.len() == Self::MAX_PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(sample);
    }

    /// Classify one low-level hook event. Injected events always pass through;
    /// a hardware event is suppressed only after an exact target observation
    /// (key, scan code, direction, extended bit, and bounded timestamp) is
    /// consumed.
    pub fn classify(&mut self, event: LegacyKeyboardSample) -> KeyboardFilterDecision {
        if event.is_injected() {
            return KeyboardFilterDecision::Pass;
        }
        self.prune(event.timestamp_ms);

        let Some(index) = self.pending.iter().position(|raw| {
            raw.virtual_key == event.virtual_key
                && u32::from(raw.scan_code) == event.scan_code
                && raw.is_break() == event.is_break()
                && raw.is_extended() == event.is_extended()
                && within_window(raw.timestamp_ms, event.timestamp_ms, self.match_window_ms)
        }) else {
            return KeyboardFilterDecision::Pass;
        };
        self.pending.remove(index);
        KeyboardFilterDecision::Suppress
    }

    /// Number of target observations waiting for a matching hardware edge.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    fn prune(&mut self, timestamp_ms: u32) {
        self.pending
            .retain(|sample| !older_than(sample.timestamp_ms, timestamp_ms, self.match_window_ms));
    }
}

/// Compare Windows tick-count timestamps while tolerating the 32-bit wrap.
const fn within_window(first: u32, second: u32, window_ms: u32) -> bool {
    first.wrapping_sub(second) <= window_ms || second.wrapping_sub(first) <= window_ms
}

/// Return whether `sample` is strictly older than the retention window at
/// `now`. The half-range check keeps the comparison correct across tick-count
/// wrap while retaining observations that arrived slightly in the future on a
/// different thread.
const fn older_than(sample: u32, now: u32, window_ms: u32) -> bool {
    let age = now.wrapping_sub(sample);
    age > window_ms && age < (1 << 31)
}
