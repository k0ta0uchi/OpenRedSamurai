//! Focused, hardware-free checks for the Phase 3 resident platform seam.

#[path = "../src/keyboard_suppression.rs"]
#[allow(dead_code)]
mod keyboard_suppression;

#[path = "../src/keyboard_relay.rs"]
#[allow(dead_code)]
mod keyboard_relay;

#[path = "../src/keyboard_relay_windows.rs"]
#[allow(dead_code)]
mod keyboard_relay_windows;

#[path = "../src/resident_platform.rs"]
#[allow(dead_code)]
mod resident_platform;

use resident_platform::{
    is_resident_input_interface, parse_resident_input_report, select_resident_input_interfaces,
    ButtonStateTracker, ButtonTransition, ResidentInputCandidate, RESIDENT_INPUT_REPORT_LENGTH,
};

#[cfg(not(windows))]
use resident_platform::PlatformError;

#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
#[test]
fn relay_trace_lines_remain_intact_when_threads_log_together() {
    let path = std::env::temp_dir().join(format!(
        "redsamurai-relay-trace-{}-{}.log",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = std::fs::remove_file(&path);
    std::env::set_var("REDSAMURAI_RECOVERY_LOG", &path);

    let workers = (0..32)
        .map(|index| {
            std::thread::spawn(move || {
                resident_platform::write_relay_trace(&format!(
                    "event=trace_test index={index} payload={}",
                    "x".repeat(256)
                ));
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().expect("trace worker must not panic");
    }
    std::env::remove_var("REDSAMURAI_RECOVERY_LOG");

    let contents = std::fs::read_to_string(&path).expect("trace log must be readable");
    let lines = contents.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 32, "each event must occupy one complete line");
    assert!(lines.iter().all(|line| {
        line.starts_with("timestamp_ms=")
            && line.contains(" event=trace_test index=")
            && line.ends_with(&"x".repeat(256))
    }));
    let _ = std::fs::remove_file(path);
}

fn candidate(
    path: &str,
    vendor_id: u16,
    product_id: u16,
    usage_page: u16,
    usage: u16,
    interface_number: i32,
    input_report_length: usize,
) -> ResidentInputCandidate {
    ResidentInputCandidate::new(
        path,
        vendor_id,
        product_id,
        usage_page,
        usage,
        interface_number,
        input_report_length,
    )
}

#[test]
fn filtering_requires_the_exact_vid_pid_usage_and_mi_01_report_shape() {
    let good = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#{guid}\\KBD"#,
        0x04D9,
        0xFC55,
        0x0001,
        0x0006,
        1,
        RESIDENT_INPUT_REPORT_LENGTH,
    );
    let wrong_collection = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55&MI_00#{guid}"#,
        0x04D9,
        0xFC55,
        0x0001,
        0x0006,
        0,
        RESIDENT_INPUT_REPORT_LENGTH,
    );
    let wrong_usage_page = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#{guid}"#,
        0x04D9,
        0xFC55,
        0x0002,
        0x0006,
        1,
        RESIDENT_INPUT_REPORT_LENGTH,
    );
    let wrong_report_length = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#{guid}"#,
        0x04D9,
        0xFC55,
        0x0001,
        0x0006,
        1,
        8,
    );
    let missing_interface_token = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55#{guid}"#,
        0x04D9,
        0xFC55,
        0x0001,
        0x0006,
        1,
        RESIDENT_INPUT_REPORT_LENGTH,
    );
    let wrong_interface_number = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#{guid}"#,
        0x04D9,
        0xFC55,
        0x0001,
        0x0006,
        2,
        RESIDENT_INPUT_REPORT_LENGTH,
    );
    let missing_keyboard_suffix = candidate(
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#{guid}"#,
        0x04D9,
        0xFC55,
        0x0001,
        0x0006,
        1,
        RESIDENT_INPUT_REPORT_LENGTH,
    );

    assert!(is_resident_input_interface(&good));
    assert!(!is_resident_input_interface(&wrong_collection));
    assert!(!is_resident_input_interface(&wrong_usage_page));
    assert!(!is_resident_input_interface(&wrong_report_length));
    assert!(!is_resident_input_interface(&missing_interface_token));
    assert!(!is_resident_input_interface(&wrong_interface_number));
    assert!(!is_resident_input_interface(&missing_keyboard_suffix));

    let all = [
        &wrong_collection,
        &good,
        &wrong_usage_page,
        &wrong_report_length,
        &missing_interface_token,
        &missing_keyboard_suffix,
    ];
    assert_eq!(select_resident_input_interfaces(all), vec![&good]);
}

#[test]
fn parser_decodes_the_bounded_keyboard_style_report_without_feature_writes() {
    let report = [0x00, 0x05, 0x00, 0x04, 0x05, 0x00, 0x00, 0x00, 0x00];

    let parsed = parse_resident_input_report(&report).expect("known 9-byte report shape");

    assert_eq!(parsed.report_id(), 0);
    assert_eq!(parsed.modifiers(), 0x05);
    assert_eq!(parsed.key_usages(), &[0x04, 0x05, 0, 0, 0, 0]);
    assert_eq!(parsed.active_usages(), &[0xE0, 0xE2, 0x04, 0x05]);
}

#[test]
fn parser_rejects_unknown_report_shapes_before_they_become_events() {
    let mut nonzero_report_id = [0u8; RESIDENT_INPUT_REPORT_LENGTH];
    nonzero_report_id[0] = 1;
    let mut nonzero_reserved = [0u8; RESIDENT_INPUT_REPORT_LENGTH];
    nonzero_reserved[2] = 1;
    let mut rollover_usage = [0u8; RESIDENT_INPUT_REPORT_LENGTH];
    rollover_usage[3] = 1;
    let mut duplicate_usage = [0u8; RESIDENT_INPUT_REPORT_LENGTH];
    duplicate_usage[3] = 4;
    duplicate_usage[4] = 4;

    for report in [
        &[][..],
        &[0u8; RESIDENT_INPUT_REPORT_LENGTH - 1][..],
        &[0u8; RESIDENT_INPUT_REPORT_LENGTH + 1][..],
        &nonzero_report_id,
        &nonzero_reserved,
        &rollover_usage,
        &duplicate_usage,
    ] {
        assert!(parse_resident_input_report(report).is_err());
    }
}

#[test]
fn tracker_emits_only_press_and_release_edges() {
    let mut tracker = ButtonStateTracker::default();
    let press_a = [0, 0, 0, 4, 0, 0, 0, 0, 0];
    let replace_with_b = [0, 0, 0, 5, 0, 0, 0, 0, 0];
    let release_b = [0, 0, 0, 0, 0, 0, 0, 0, 0];

    assert_eq!(
        tracker.update(&press_a).unwrap(),
        vec![ButtonTransition::pressed(4)]
    );
    assert_eq!(
        tracker.update(&press_a).unwrap(),
        Vec::<ButtonTransition>::new()
    );
    assert_eq!(
        tracker.update(&replace_with_b).unwrap(),
        vec![ButtonTransition::released(4), ButtonTransition::pressed(5)]
    );
    assert_eq!(
        tracker.update(&release_b).unwrap(),
        vec![ButtonTransition::released(5)]
    );
}

#[test]
fn malformed_report_does_not_mutate_the_tracker_state() {
    let mut tracker = ButtonStateTracker::default();
    let press_a = [0, 0, 0, 4, 0, 0, 0, 0, 0];
    let malformed = [1, 0, 0, 4, 0, 0, 0, 0, 0];
    let release_a = [0, 0, 0, 0, 0, 0, 0, 0, 0];

    tracker.update(&press_a).unwrap();
    assert!(tracker.update(&malformed).is_err());
    assert_eq!(
        tracker.update(&release_a).unwrap(),
        vec![ButtonTransition::released(4)]
    );
}

#[cfg(windows)]
#[test]
fn raw_keyboard_events_map_only_known_usb_usages_and_preserve_break_edges() {
    assert_eq!(
        resident_platform::map_raw_keyboard_event(0x41, 0),
        Some(ButtonTransition::pressed(0x04))
    );
    assert_eq!(
        resident_platform::map_raw_keyboard_event(0x41, 0x0001),
        Some(ButtonTransition::released(0x04))
    );
    assert_eq!(
        resident_platform::map_raw_keyboard_event(0x70, 0),
        Some(ButtonTransition::pressed(0x3A))
    );
    assert_eq!(resident_platform::map_raw_keyboard_event(0xFF, 0), None);
}

#[cfg(windows)]
#[test]
fn raw_input_transition_reports_keep_press_and_release_edges_bounded() {
    assert_eq!(
        resident_platform::raw_input_transition_report(ButtonTransition::pressed(0x04)),
        [0, 0, 0, 0x04, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        resident_platform::raw_input_transition_report(ButtonTransition::released(0x04)),
        [0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[cfg(windows)]
#[test]
fn raw_keyboard_payload_accepts_exact_keyboard_payload_without_union_padding() {
    let header = std::mem::size_of::<windows::Win32::UI::Input::RAWINPUTHEADER>();
    let keyboard = std::mem::size_of::<windows::Win32::UI::Input::RAWKEYBOARD>();

    assert!(resident_platform::raw_keyboard_payload_is_sized(
        header + keyboard,
        header,
        keyboard,
    ));
    assert!(!resident_platform::raw_keyboard_payload_is_sized(
        header + keyboard - 1,
        header,
        keyboard,
    ));
}

#[cfg(windows)]
#[test]
fn raw_input_path_matching_allows_keyboard_class_guid_and_collection_suffix() {
    let expected = r#"\\?\HID#VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}\KBD"#;
    let raw_input_name = r#"\\?\HID#VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"#;
    let raw_input_keyboard_interface_name = format!(
        "{}\0",
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000#{884b96c3-56ef-11d1-bc8c-00a0c91405dd}"#
    );

    assert!(resident_platform::raw_input_device_path_matches(
        expected,
        raw_input_name
    ));
    assert!(resident_platform::raw_input_device_path_matches(
        expected,
        &raw_input_keyboard_interface_name
    ));
    assert!(!resident_platform::raw_input_device_path_matches(
        expected,
        r#"\\?\HID#VID_04D9&PID_FC55&MI_00#9&13e53cd8&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"#
    ));
    assert!(!resident_platform::raw_input_device_path_matches(
        expected,
        r#"\\?\HID#VID_04D9&PID_FC55&MI_01#9&13e53cd8&0&0000#{00000000-0000-0000-0000-000000000000}"#
    ));
}

#[cfg(windows)]
#[test]
fn relay_probe_arm_key_rejects_window_switching_chords_and_extended_keys() {
    assert!(!resident_platform::relay_probe_arm_key_matches(
        0x12, 0x38, 0
    ));
    assert!(!resident_platform::relay_probe_arm_key_matches(
        0x09, 0x0F, 0
    ));
    assert!(!resident_platform::relay_probe_arm_key_matches(
        0x41, 0x1E, 0x0002
    ));
    assert!(resident_platform::relay_probe_arm_key_matches(
        0x41, 0x1E, 0
    ));
    assert!(resident_platform::relay_probe_arm_key_matches(
        0x41, 0x1E, 0x0001
    ));
}

#[cfg(windows)]
#[test]
fn unknown_relay_device_identity_is_a_probe_fail_open_condition() {
    assert!(resident_platform::relay_identity_failure_requires_probe(
        "Raw Input device identity was unavailable"
    ));
    assert!(!resident_platform::relay_identity_failure_requires_probe(
        "GetRawInputData read failed (Win32 error 6)"
    ));
}

#[cfg(windows)]
#[test]
fn raw_input_startup_readiness_is_bounded_and_reports_timeout() {
    let (_sender, receiver) =
        std::sync::mpsc::sync_channel::<Result<resident_platform::RawInputThreadReady, String>>(1);
    let started = Instant::now();

    let error = resident_platform::await_raw_input_startup(&receiver, Duration::from_millis(20))
        .expect_err("startup without readiness must fail");

    assert!(started.elapsed() < Duration::from_secs(1));
    match error {
        resident_platform::PlatformError::InputOpen { message } => {
            assert!(
                message.contains("timed out"),
                "unexpected startup error: {message}"
            );
        }
        other => panic!("expected bounded startup timeout, got {other:?}"),
    }
}

#[cfg(windows)]
#[test]
fn raw_input_startup_readiness_preserves_worker_error() {
    let (sender, receiver) =
        std::sync::mpsc::sync_channel::<Result<resident_platform::RawInputThreadReady, String>>(1);
    sender
        .send(Err(
            "RegisterRawInputDevices failed (Win32 error 5)".to_owned()
        ))
        .unwrap();

    let error = resident_platform::await_raw_input_startup(&receiver, Duration::from_secs(1))
        .expect_err("worker initialization error must be surfaced");

    assert_eq!(
        error,
        resident_platform::PlatformError::InputOpen {
            message: "RegisterRawInputDevices failed (Win32 error 5)".to_owned(),
        }
    );
}

#[cfg(windows)]
#[test]
fn raw_input_receiver_surfaces_worker_errors_and_disconnects() {
    let (sender, receiver) = std::sync::mpsc::channel();
    sender
        .send(Err("GetMessageW failed (Win32 error 123)".to_owned()))
        .unwrap();
    let error = resident_platform::receive_raw_input_transition(&receiver, 20)
        .expect_err("worker error must not become a timeout");
    assert_eq!(
        error,
        resident_platform::PlatformError::InputRead {
            message: "GetMessageW failed (Win32 error 123)".to_owned(),
        }
    );

    drop(sender);
    let error = resident_platform::receive_raw_input_transition(&receiver, 20)
        .expect_err("worker disconnect must be reported");
    match error {
        resident_platform::PlatformError::InputRead { message } => {
            assert!(
                message.contains("disconnected"),
                "unexpected disconnect error: {message}"
            );
        }
        other => panic!("expected disconnect error, got {other:?}"),
    }
}

#[cfg(windows)]
#[test]
fn raw_input_thread_join_waits_for_clean_worker_exit() {
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_finished = finished.clone();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        worker_finished.store(true, std::sync::atomic::Ordering::Release);
    });

    assert!(resident_platform::join_raw_input_thread(
        thread,
        Duration::from_secs(1)
    ));
    assert!(finished.load(std::sync::atomic::Ordering::Acquire));
}

#[cfg(not(windows))]
#[test]
fn non_windows_discovery_is_unsupported_without_hardware_access() {
    assert_eq!(
        resident_platform::discover_resident_input_interfaces(),
        Err(PlatformError::UnsupportedPlatform)
    );
}

#[cfg(not(windows))]
#[test]
fn non_windows_output_helpers_fail_closed_without_touching_hardware() {
    let keyboard =
        resident_platform::send_keyboard_event(resident_platform::KeyboardEvent::key_down(0x41));
    let mouse = resident_platform::send_mouse_event(resident_platform::MouseEvent::button(
        resident_platform::MouseButton::Left,
        true,
    ));
    let media = resident_platform::send_media_event(resident_platform::MediaEvent::key_down(0xB0));

    assert_eq!(keyboard, Err(PlatformError::UnsupportedPlatform));
    assert_eq!(mouse, Err(PlatformError::UnsupportedPlatform));
    assert_eq!(media, Err(PlatformError::UnsupportedPlatform));
}
