//! Tests for the Rust-owned configuration recovery path.
//!
//! The runtime must keep working when the vendor `hid.exe` process is absent.
//! These tests exercise the recovery coordinator with an injected transport;
//! no process lookup or Windows HID handle is involved.

use redsamurai_config::device_apply::{ApplyError, ApplyPlan, VerifiedSequenceTransport};
use redsamurai_config::device_protocol::VerifiedApplySequence;
use redsamurai_config::device_runtime::{ReconnectResult, RustOwnedConfig};
use redsamurai_config::ini::IniDoc;
use redsamurai_config::profile::Profile;

fn profile(polling_rate: i32, dpi: Option<i32>) -> Profile {
    let mut doc = IniDoc::default();
    doc.set("GROUP0", "PollingRate", &polling_rate.to_string());
    if let Some(dpi) = dpi {
        doc.set("GROUP0", "DPI", &dpi.to_string());
    }
    Profile { slot: 1, doc }
}

#[derive(Default)]
struct RecordingTransport {
    calls: usize,
    acknowledged: usize,
    profile_dpi: Vec<Option<i32>>,
}

impl VerifiedSequenceTransport for RecordingTransport {
    type Error = &'static str;

    fn send_verified_sequence(
        &mut self,
        sequence: &VerifiedApplySequence,
    ) -> Result<usize, Self::Error> {
        self.calls += 1;
        self.profile_dpi.push(sequence.profile_dpi_selection());
        Ok(self.acknowledged)
    }
}

#[test]
fn reconnect_reapplies_the_complete_authorized_sequence_without_vendor_process() {
    let owner = RustOwnedConfig::from_profile(&profile(8, Some(2)));
    assert!(owner.has_reconnect_sequence());
    assert_eq!(owner.profile_dpi_selection(), Some(2));

    let mut transport = RecordingTransport {
        acknowledged: 156,
        ..Default::default()
    };
    let result = owner
        .reapply_after_reconnect(&mut transport)
        .expect("the Rust-owned sequence should be transportable");

    assert_eq!(result, ReconnectResult::Applied { sent: 156 });
    assert_eq!(transport.calls, 1);
    assert_eq!(transport.profile_dpi, vec![Some(2)]);
}

#[test]
fn baseline_dpi_zero_keeps_the_polling_sequence_authorized() {
    let owner = RustOwnedConfig::from_profile(&profile(8, Some(0)));

    assert!(owner.has_reconnect_sequence());
    assert_eq!(owner.profile_dpi_selection(), None);
}

#[test]
fn malformed_dpi_does_not_fall_back_to_the_baseline_write() {
    let mut candidate = profile(8, Some(0));
    candidate.doc.set("GROUP0", "DPI", "not-a-number");

    let owner = RustOwnedConfig::from_profile(&candidate);

    assert!(!owner.has_reconnect_sequence());
}

#[test]
fn startup_preparation_is_local_and_reconnect_is_the_only_write_trigger() {
    let owner = RustOwnedConfig::from_profile(&profile(8, Some(1)));
    let mut transport = RecordingTransport {
        acknowledged: 156,
        ..Default::default()
    };

    // Constructing the owner only prepares a typed token.  A transport is
    // untouched until the explicit reconnect hook is invoked.
    assert_eq!(transport.calls, 0);
    assert!(owner.has_reconnect_sequence());
    assert_eq!(transport.calls, 0);

    owner
        .reapply_after_reconnect(&mut transport)
        .expect("reconnect hook should perform the one explicit write");
    assert_eq!(transport.calls, 1);
}

#[test]
fn unsupported_profile_values_skip_recovery_without_transport_io() {
    for candidate in [profile(4, Some(1)), profile(8, Some(3))] {
        let owner = RustOwnedConfig::from_profile(&candidate);
        assert!(!owner.has_reconnect_sequence());

        let mut transport = RecordingTransport {
            acknowledged: 156,
            ..Default::default()
        };
        assert_eq!(
            owner
                .reapply_after_reconnect(&mut transport)
                .expect("unsupported recovery must be a safe no-op"),
            ReconnectResult::SkippedNoAuthorizedSequence
        );
        assert_eq!(transport.calls, 0);
    }
}

#[test]
fn incomplete_reconnect_completion_fails_closed() {
    let owner = RustOwnedConfig::from_profile(&profile(8, Some(1)));
    let mut transport = RecordingTransport {
        acknowledged: 155,
        ..Default::default()
    };

    assert_eq!(
        owner.reapply_after_reconnect(&mut transport),
        Err(ApplyError::InvalidSequenceCompletion {
            actual: 155,
            expected: 156,
        })
    );
}

#[test]
fn known_sequence_can_be_applied_while_unverified_profile_fields_stay_untouched() {
    let mut full_profile = Profile::default_profile(1);
    full_profile.set_i32("DPI", 2);
    let plan = ApplyPlan::try_build_authorized(&full_profile).expect("authorized profile plan");
    assert!(!plan.warnings.is_empty());

    let sequence = plan
        .verified_sequence()
        .cloned()
        .expect("the proven PollingRate/DPI sequence should be retained");
    let write_plan = ApplyPlan::from_verified_sequence(sequence).expect("sequence-only plan");
    let mut transport = RecordingTransport {
        acknowledged: 156,
        ..Default::default()
    };

    let outcome = write_plan
        .apply_verified_sequence(&mut transport)
        .expect("only the known sequence should cross the transport boundary");
    assert_eq!(outcome.sent, 156);
    assert_eq!(transport.calls, 1);
}
