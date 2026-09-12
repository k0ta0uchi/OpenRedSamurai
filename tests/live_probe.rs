//! Hardware-free checks for the live probe's bounded Raw Input monitor mode.

#[path = "../examples/live_probe.rs"]
#[allow(dead_code)]
mod live_probe;

use redsamurai_config::device_apply::{ApplyOutcome, ApplyPersistence, ApplyTransferRecord};
use redsamurai_config::device_protocol::{DpiSelectionReadback, PollingRateReadback};
use redsamurai_config::resident_platform::ButtonTransition;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn polling_readback_requires_the_authorized_apply_confirmations() {
    let missing_apply = args(&[
        "--confirm-live",
        "--observe-polling-readback",
        "--confirm-hid-apply",
    ]);
    let missing_confirm = args(&[
        "--confirm-live",
        "--observe-polling-readback",
        "--apply-125hz",
    ]);
    let allowed = args(&[
        "--confirm-live",
        "--confirm-hid-apply",
        "--apply-125hz",
        "--observe-polling-readback",
    ]);

    assert!(live_probe::parse_polling_rate_readback(&missing_apply).is_err());
    assert!(live_probe::parse_polling_rate_readback(&missing_confirm).is_err());
    assert_eq!(
        live_probe::parse_polling_rate_readback(&allowed).unwrap(),
        true
    );
}

#[test]
fn dpi_selection_readback_requires_live_confirmation_and_is_read_only() {
    let missing_confirm = args(&["--observe-dpi-selection-readback"]);
    let allowed = args(&["--confirm-live", "--observe-dpi-selection-readback"]);

    assert!(live_probe::parse_dpi_selection_readback(&missing_confirm).is_err());
    assert_eq!(
        live_probe::parse_dpi_selection_readback(&allowed).unwrap(),
        true
    );
}

#[test]
fn bounded_resident_mode_requires_confirmation_and_read_flags() {
    let missing_both = args(&["--read-resident-for-ms", "1000"]);
    let missing_read = args(&["--confirm-live", "--read-resident-for-ms", "1000"]);

    let error = live_probe::parse_resident_read_for_ms(&missing_both)
        .expect_err("the monitor must require --confirm-live");
    assert!(error.contains("--confirm-live"));
    let error = live_probe::parse_resident_read_for_ms(&missing_read)
        .expect_err("the monitor must require --read-resident");
    assert!(error.contains("--read-resident"));
}

#[test]
fn bounded_resident_mode_accepts_milliseconds_and_caps_the_window() {
    let short = args(&[
        "--confirm-live",
        "--read-resident",
        "--read-resident-for-ms",
        "1500",
    ]);
    let long = args(&[
        "--confirm-live",
        "--read-resident",
        "--read-resident-for-ms",
        "999999999",
    ]);

    assert_eq!(
        live_probe::parse_resident_read_for_ms(&short).unwrap(),
        Some(1500)
    );
    assert_eq!(
        live_probe::parse_resident_read_for_ms(&long).unwrap(),
        Some(live_probe::MAX_RESIDENT_READ_FOR_MS)
    );
    assert!(live_probe::MAX_RESIDENT_READ_FOR_MS > 0);
}

#[test]
fn bounded_resident_mode_rejects_missing_zero_and_invalid_durations() {
    for values in [
        vec![
            "--confirm-live",
            "--read-resident",
            "--read-resident-for-ms",
        ],
        vec![
            "--confirm-live",
            "--read-resident",
            "--read-resident-for-ms",
            "0",
        ],
        vec![
            "--confirm-live",
            "--read-resident",
            "--read-resident-for-ms",
            "not-a-number",
        ],
    ] {
        let arguments = args(&values);
        assert!(
            live_probe::parse_resident_read_for_ms(&arguments).is_err(),
            "expected invalid duration to be rejected: {values:?}"
        );
    }
}

#[test]
fn bounded_resident_mode_rejects_input_injection_and_configuration_actions() {
    for action in [
        "--apply-125hz",
        "--sendinput-zero-move",
        "--sendinput-one-pixel",
    ] {
        let arguments = args(&[
            "--confirm-live",
            "--read-resident",
            "--read-resident-for-ms",
            "1000",
            action,
        ]);
        assert!(
            live_probe::parse_resident_read_for_ms(&arguments).is_err(),
            "bounded monitor must reject {action}"
        );
    }
}

#[test]
fn transition_format_preserves_exact_press_release_and_usage() {
    assert_eq!(
        live_probe::format_resident_transition(ButtonTransition::pressed(0x04)),
        "resident_transition=press usage=0x04"
    );
    assert_eq!(
        live_probe::format_resident_transition(ButtonTransition::released(0x17)),
        "resident_transition=release usage=0x17"
    );
}

#[test]
fn resident_monitor_keeps_waiting_after_an_idle_read_timeout() {
    let mut clock = [0_u64, 1, 2, 3, 10].into_iter();
    let mut reads = [
        Ok(None),
        Ok(Some(vec![ButtonTransition::pressed(0x04)])),
        Ok(Some(vec![ButtonTransition::released(0x04)])),
        Ok(None),
    ]
    .into_iter();

    let result = live_probe::collect_resident_transitions_until(
        10,
        |_timeout_ms| reads.next().expect("test read sequence"),
        || clock.next().expect("test clock sequence"),
    );

    assert_eq!(
        result,
        live_probe::ResidentMonitorResult::Timeout {
            transitions: vec![
                ButtonTransition::pressed(0x04),
                ButtonTransition::released(0x04),
            ],
        }
    );
}

#[test]
fn apply_format_reports_shape_completion_and_unobserved_persistence() {
    let outcome = ApplyOutcome {
        sent: 156,
        warnings: Vec::new(),
        transfer: ApplyTransferRecord {
            expected_reports: 156,
            completed_reports: 156,
            expected_report_lengths: vec![16; 155]
                .into_iter()
                .chain(std::iter::once(64))
                .collect(),
        },
        persistence: ApplyPersistence::NotObserved,
    };

    assert_eq!(
        live_probe::format_verified_apply_outcome(&outcome),
        "verified_125hz_apply_transfer=complete expected=156 completed=156 expected_16=155 expected_64=1 expected_other=0 persistence=not-observed"
    );
}

#[test]
fn readback_format_is_observational_and_keeps_persistence_unobserved() {
    let readback = PollingRateReadback::decode(&[
        0x02, 0x08, 0x32, 0x60, 0x05, 0x00, 0xFA, 0xFA, 0x08, 0x00, 0x08, 0x00, 0x02,
    ])
    .expect("official readback marker");

    assert_eq!(
        live_probe::format_polling_rate_readback(&readback),
        "verified_125hz_apply_readback=observed wire=0x08 persistence=not-observed"
    );
}

#[test]
fn dpi_selection_readback_format_reports_profile_and_wire_values() {
    let readback = DpiSelectionReadback::decode(&[
        0x03, 0x08, 0x42, 0x60, 0x20, 0x00, 0xFA, 0xFA, 0x02, 0x00, 0x01, 0x09, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x21, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3A, 0x00, 0x00, 0x00, 0x00, 0x01, 0x5C,
        0x01, 0x00, 0x00, 0x00, 0x01, 0x7C, 0x04, 0x00, 0x00, 0x00,
    ])
    .expect("official DPI=2 readback marker");

    assert_eq!(
        live_probe::format_dpi_selection_readback(&readback),
        "verified_dpi_selection_readback=observed profile=2 wire=0x02 persistence=not-observed"
    );
}

#[test]
fn dpi_apply_profile_requires_bounded_confirmed_reconnect_flow() {
    let allowed = args(&[
        "--confirm-live",
        "--confirm-hid-apply",
        "--apply-dpi-profile",
        "1",
    ]);
    assert_eq!(
        live_probe::parse_dpi_apply_profile(&allowed).unwrap(),
        Some(1)
    );

    for values in [
        &["--confirm-live", "--apply-dpi-profile", "1"][..],
        &[
            "--confirm-live",
            "--confirm-hid-apply",
            "--apply-dpi-profile",
            "0",
        ][..],
        &[
            "--confirm-live",
            "--confirm-hid-apply",
            "--apply-dpi-profile",
            "3",
        ][..],
        &[
            "--confirm-live",
            "--confirm-hid-apply",
            "--apply-dpi-profile",
        ][..],
    ] {
        let arguments = args(values);
        assert!(
            live_probe::parse_dpi_apply_profile(&arguments).is_err(),
            "DPI Apply arguments must be rejected: {values:?}"
        );
    }
}

#[test]
fn dpi_apply_profile_rejects_other_live_actions() {
    for action in [
        "--apply-125hz",
        "--observe-dpi-selection-readback",
        "--observe-polling-readback",
        "--read-resident",
        "--sendinput-one-pixel",
    ] {
        let arguments = args(&[
            "--confirm-live",
            "--confirm-hid-apply",
            "--apply-dpi-profile",
            "1",
            action,
        ]);
        assert!(
            live_probe::parse_dpi_apply_profile(&arguments).is_err(),
            "DPI Apply must reject {action}"
        );
    }
}

#[test]
fn dpi_apply_format_records_selected_profile_and_transfer_shape() {
    let outcome = ApplyOutcome {
        sent: 156,
        warnings: Vec::new(),
        transfer: ApplyTransferRecord {
            expected_reports: 156,
            completed_reports: 156,
            expected_report_lengths: vec![16; 155]
                .into_iter()
                .chain(std::iter::once(64))
                .collect(),
        },
        persistence: ApplyPersistence::NotObserved,
    };

    assert_eq!(
        live_probe::format_dpi_apply_outcome(&outcome, 1),
        "verified_dpi_apply_transfer=complete profile=1 wire=0x01 expected=156 completed=156 expected_16=155 expected_64=1 expected_other=0 persistence=not-observed"
    );
}
