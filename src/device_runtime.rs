//! Rust-owned configuration lifecycle.
//!
//! The vendor `hid.exe` process is not part of the application runtime.  This
//! module keeps the small set of reviewed device state in memory and exposes
//! one explicit hook for replaying it after a physical reconnect.  Discovery,
//! handle ownership, and report I/O remain in [`crate::device`]; the worker
//! only receives the typed sequence authorization from [`crate::device_apply`].

use crate::device_apply::{
    build_rust_owned_reconnect_sequence, ApplyError, VerifiedSequenceTransport,
};
use crate::device_protocol::VerifiedApplySequence;
use crate::profile::Profile;

/// Result of the Rust-owned reconnect hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectResult {
    /// The complete reviewed sequence was acknowledged by the transport.
    Applied { sent: usize },
    /// No reviewed sequence could be derived from the current profile, so the
    /// worker left the device untouched.
    SkippedNoAuthorizedSequence,
}

/// The configuration state retained by the Rust process.
///
/// Construction is side-effect free.  A device handle is opened only when a
/// caller invokes [`Self::reapply_after_reconnect`], which is reserved for a
/// reconnect boundary in the resident worker.
#[derive(Debug, Clone, Default)]
pub struct RustOwnedConfig {
    sequence: Option<VerifiedApplySequence>,
}

impl RustOwnedConfig {
    /// Prepare Rust-owned state from a profile without opening HID or sending
    /// a report.  Unverified profile fields remain outside this state.
    pub fn from_profile(profile: &Profile) -> Self {
        Self {
            sequence: build_rust_owned_reconnect_sequence(profile),
        }
    }

    /// Replace the desired state after a resident profile switch.  This is a
    /// local operation; the new state is replayed only at the next reconnect.
    pub fn set_profile(&mut self, profile: &Profile) {
        self.sequence = build_rust_owned_reconnect_sequence(profile);
    }

    /// Return the reviewed sequence, if the profile has one.
    pub fn sequence(&self) -> Option<&VerifiedApplySequence> {
        self.sequence.as_ref()
    }

    /// Whether a complete, reviewed sequence is available for reconnect.
    pub fn has_reconnect_sequence(&self) -> bool {
        self.sequence.is_some()
    }

    /// Return the selected-DPI value carried by the desired sequence.
    pub fn profile_dpi_selection(&self) -> Option<i32> {
        self.sequence
            .as_ref()
            .and_then(VerifiedApplySequence::profile_dpi_selection)
    }

    /// Replay the complete reviewed sequence after the caller has reopened a
    /// fresh configuration handle for a physically reconnected device.
    ///
    /// The transport is called once with the sequence token and must
    /// acknowledge all 156 reports.  A missing sequence is a safe no-op; an
    /// incomplete acknowledgement is returned as a typed fail-closed error.
    pub fn reapply_after_reconnect<T>(
        &self,
        transport: &mut T,
    ) -> Result<ReconnectResult, ApplyError>
    where
        T: VerifiedSequenceTransport,
    {
        let Some(sequence) = self.sequence.clone() else {
            return Ok(ReconnectResult::SkippedNoAuthorizedSequence);
        };

        let plan = crate::device_apply::ApplyPlan::from_verified_sequence(sequence)?;
        let outcome = plan.apply_verified_sequence(transport)?;
        Ok(ReconnectResult::Applied { sent: outcome.sent })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_protocol::VerifiedApplySequence;
    use crate::ini::IniDoc;

    #[derive(Default)]
    struct RecordingTransport {
        calls: usize,
        acknowledged: usize,
    }

    impl VerifiedSequenceTransport for RecordingTransport {
        type Error = &'static str;

        fn send_verified_sequence(
            &mut self,
            sequence: &VerifiedApplySequence,
        ) -> Result<usize, Self::Error> {
            self.calls += 1;
            assert_eq!(sequence.len(), 156);
            Ok(self.acknowledged)
        }
    }

    fn profile(polling_rate: i32, dpi: Option<i32>) -> Profile {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", &polling_rate.to_string());
        if let Some(dpi) = dpi {
            doc.set("GROUP0", "DPI", &dpi.to_string());
        }
        Profile { slot: 1, doc }
    }

    #[test]
    fn preparation_does_not_touch_a_transport() {
        let owner = RustOwnedConfig::from_profile(&profile(8, Some(1)));
        let transport = RecordingTransport::default();

        assert!(owner.has_reconnect_sequence());
        assert_eq!(owner.profile_dpi_selection(), Some(1));
        assert_eq!(transport.calls, 0);
    }

    #[test]
    fn unsupported_profile_values_have_no_reconnect_write() {
        let owner = RustOwnedConfig::from_profile(&profile(8, Some(3)));
        assert!(!owner.has_reconnect_sequence());

        let mut transport = RecordingTransport {
            acknowledged: 156,
            ..Default::default()
        };
        assert_eq!(
            owner.reapply_after_reconnect(&mut transport).unwrap(),
            ReconnectResult::SkippedNoAuthorizedSequence
        );
        assert_eq!(transport.calls, 0);
    }
}
