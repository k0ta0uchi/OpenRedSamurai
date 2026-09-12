//! Hardware-free checks for the post-Apply readback observation seam.

use redsamurai_config::device::{DeviceError, DeviceTransport};
use redsamurai_config::device_apply::{
    ApplyError, ApplyPersistence, ApplyPlan, DpiSelectionReadbackTransport,
    PollingRateReadbackTransport,
};
use redsamurai_config::device_protocol::{
    DpiSelectionReadback, PollingRateReadback, DPI_SELECTION_READBACK_LEN,
    POLLING_RATE_READBACK_LEN, REPORT_ID_CONFIG, REPORT_ID_CONFIG_EXTENDED, REPORT_LEN,
};
use redsamurai_config::ini::IniDoc;
use redsamurai_config::profile::Profile;

const OFFICIAL_125_HZ_READBACK: [u8; POLLING_RATE_READBACK_LEN] = [
    0x02, 0x08, 0x32, 0x60, 0x05, 0x00, 0xFA, 0xFA, 0x08, 0x00, 0x08, 0x00, 0x02,
];

const OFFICIAL_DPI_SELECTION_1_READBACK: [u8; DPI_SELECTION_READBACK_LEN] = [
    0x03, 0x08, 0x42, 0x60, 0x20, 0x00, 0xFA, 0xFA, 0x01, 0x00, 0x01, 0x09, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x21, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3A, 0x00, 0x00, 0x00, 0x00, 0x01, 0x5C, 0x01, 0x00,
    0x00, 0x00, 0x01, 0x7C, 0x04, 0x00, 0x00, 0x00,
];

const OFFICIAL_DPI_SELECTION_2_READBACK: [u8; DPI_SELECTION_READBACK_LEN] = [
    0x03, 0x08, 0x42, 0x60, 0x20, 0x00, 0xFA, 0xFA, 0x02, 0x00, 0x01, 0x09, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x21, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3A, 0x00, 0x00, 0x00, 0x00, 0x01, 0x5C, 0x01, 0x00,
    0x00, 0x00, 0x01, 0x7C, 0x04, 0x00, 0x00, 0x00,
];

fn authorized_125hz_plan() -> ApplyPlan {
    let mut doc = IniDoc::default();
    doc.set("GROUP0", "PollingRate", "8");
    ApplyPlan::try_build_authorized(&Profile { slot: 1, doc }).expect("authorized profile")
}

#[test]
fn official_readback_marker_decodes_as_observation_only() {
    let readback = PollingRateReadback::decode(&OFFICIAL_125_HZ_READBACK)
        .expect("official 125 Hz readback marker");

    assert_eq!(readback.wire_value(), 0x08);
    assert!(readback.matches_wire_value(0x08));
    assert_eq!(readback.as_bytes(), &OFFICIAL_125_HZ_READBACK);
}

#[test]
fn official_dpi_selection_ab_readbacks_decode_to_profile_values() {
    let one = DpiSelectionReadback::decode(&OFFICIAL_DPI_SELECTION_1_READBACK)
        .expect("official DPI=1 readback");
    let two = DpiSelectionReadback::decode(&OFFICIAL_DPI_SELECTION_2_READBACK)
        .expect("official DPI=2 readback");

    assert_eq!(one.wire_value(), 0x01);
    assert_eq!(one.profile_value(), 1);
    assert!(one.matches_profile_value(1));
    assert!(!one.matches_profile_value(2));
    assert_eq!(two.wire_value(), 0x02);
    assert_eq!(two.profile_value(), 2);
    assert!(two.matches_profile_value(2));
    assert_eq!(one.as_bytes(), &OFFICIAL_DPI_SELECTION_1_READBACK);
    assert_eq!(two.as_bytes(), &OFFICIAL_DPI_SELECTION_2_READBACK);
}

#[test]
fn device_observes_dpi_selection_on_the_verified_extended_route() {
    let mut transport = DeviceTransport::mock();
    transport
        .queue_mock_feature_report(OFFICIAL_DPI_SELECTION_2_READBACK)
        .expect("queue mock DPI response");

    let readback = DpiSelectionReadbackTransport::observe_dpi_selection_readback(&mut transport)
        .expect("observe DPI selection readback");

    assert_eq!(readback.profile_value(), 2);
    assert_eq!(readback.wire_value(), 0x02);
    assert_eq!(
        transport.sent_mock_reports().expect("mock reports").len(),
        0
    );
    let direct = transport
        .get_feature_report_observation(REPORT_ID_CONFIG_EXTENDED, 64)
        .expect_err("the queued response was consumed by the typed route");
    assert_eq!(direct, DeviceError::MockReportUnavailable);
}

#[test]
fn observation_route_preserves_request_shape_and_does_not_send() {
    let mut transport = DeviceTransport::mock();
    transport
        .queue_mock_feature_report(OFFICIAL_125_HZ_READBACK)
        .expect("queue mock response");

    let response = transport
        .get_feature_report_observation(REPORT_ID_CONFIG, REPORT_LEN)
        .expect("observed 16-byte request route");

    assert_eq!(response, OFFICIAL_125_HZ_READBACK);
    assert!(transport
        .sent_mock_reports()
        .expect("mock reports")
        .is_empty());
}

#[test]
fn invalid_observation_length_fails_closed_before_decode() {
    let mut transport = DeviceTransport::mock();
    transport
        .queue_mock_feature_report([0x02, 0x08, 0x32])
        .expect("queue short response");

    let error = transport
        .observe_polling_rate_readback()
        .expect_err("a short readback cannot prove state");

    assert_eq!(
        error,
        DeviceError::Io {
            operation: "decode polling-rate readback",
            message: "invalid PollingRate readback length: expected 13 bytes, got 3".to_owned(),
        }
    );
    assert!(transport
        .sent_mock_reports()
        .expect("mock reports")
        .is_empty());
}

#[test]
fn short_dpi_observation_explains_standalone_route_and_keeps_no_write() {
    let mut transport = DeviceTransport::mock();
    transport
        .queue_mock_feature_report([0x03, 0x08, 0xBA, 0x63, 0x00, 0x00, 0xFA, 0xFA])
        .expect("queue standalone short DPI response");

    let error = transport
        .observe_dpi_selection_readback()
        .expect_err("standalone short response cannot prove selected DPI");

    assert_eq!(
        error,
        DeviceError::Io {
            operation: "decode DPI selection readback",
            message: "invalid DPI selection readback length: expected 40 bytes, got 8; standalone Report-03 GET returned short/alternate response bytes=03 08 BA 63 00 00 FA FA; selected-DPI readback requires the full Apply/reconnect readback sequence".to_owned(),
        }
    );
    assert!(transport
        .sent_mock_reports()
        .expect("mock reports")
        .is_empty());
}

#[test]
fn authorized_apply_can_observe_readback_without_promoting_persistence() {
    let plan = authorized_125hz_plan();
    let mut transport = DeviceTransport::mock();
    transport
        .queue_mock_feature_report(OFFICIAL_125_HZ_READBACK)
        .expect("queue mock response");

    let (outcome, readback) = plan
        .apply_verified_sequence_with_readback(&mut transport)
        .expect("authorized sequence and exact readback");

    assert_eq!(outcome.sent, 156);
    assert_eq!(outcome.persistence, ApplyPersistence::NotObserved);
    assert_eq!(readback.wire_value(), 0x08);
    assert_eq!(
        transport.sent_mock_reports().expect("mock reports").len(),
        156
    );
}

#[test]
fn unauthorized_or_mismatched_readback_is_typed_and_fail_closed() {
    let plan = authorized_125hz_plan();
    let mut transport = DeviceTransport::mock();
    let mut marker = OFFICIAL_125_HZ_READBACK;
    marker[8] = 0x04;
    transport
        .queue_mock_feature_report(marker)
        .expect("queue mismatched mock response");

    let error = plan
        .apply_verified_sequence_with_readback(&mut transport)
        .expect_err("250 Hz marker cannot prove the authorized 125 Hz Apply");

    assert_eq!(
        error,
        ApplyError::ReadbackMismatch {
            actual: 0x04,
            expected: 0x08,
        }
    );
    assert_eq!(
        transport.sent_mock_reports().expect("mock reports").len(),
        156
    );
}

#[test]
fn readback_trait_is_available_after_the_authorized_sequence() {
    let mut transport = DeviceTransport::mock();
    transport
        .queue_mock_feature_report(OFFICIAL_125_HZ_READBACK)
        .expect("queue mock response");

    let readback = PollingRateReadbackTransport::observe_polling_rate_readback(&mut transport)
        .expect("readback observation");

    assert_eq!(readback.wire_value(), 0x08);
}
