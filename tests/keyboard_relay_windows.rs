//! Hardware-free contract tests for the opt-in Windows relay seam.

use redsamurai_config::keyboard_relay::{
    KeyPhase, KeyboardRelay, OutputKey, RawKeyboardSample, RelayDecision, RelayEvent,
};
use redsamurai_config::keyboard_relay_windows::{
    classify_hook_event, classify_official_keyboard_hook, raw_keyboard_to_relay_sample,
    relay_gate_opt_in_requested_from, relay_observe_only_requested_from,
    relay_opt_in_requested_from, replay_injected_key, replay_keyboard_input, HookDecision,
    KeyboardReplay, OfficialKeyboardHookDecision, OfficialKeyboardHookEvent,
    OfficialKeyboardHookMapping, RawInputRegistration, RawKeyboardPacket, RelayCleanup,
    RelayLifecycle, RelayLifecycleState, RAW_INPUT_PROBE_REGISTER_FLAGS, RAW_INPUT_REGISTER_FLAGS,
    RAW_KEY_BREAK, RAW_KEY_E0, REDSAMURAI_SELF_INJECT_MARKER,
};

#[test]
fn raw_keyboard_conversion_preserves_make_break_and_extended_identity() {
    let packet = RawKeyboardPacket::new(0xA5, 0x1D, RAW_KEY_BREAK | RAW_KEY_E0, 42);

    let sample = raw_keyboard_to_relay_sample("keyboard-path", packet);

    assert_eq!(sample.device_id().as_str(), "keyboard-path");
    assert_eq!(sample.virtual_key, 0xA5);
    assert_eq!(sample.scan_code, 0x1D);
    assert_eq!(sample.flags, RAW_KEY_BREAK | RAW_KEY_E0);
    assert!(sample.is_up());
    assert!(sample.input_key().is_extended());
}

#[test]
fn replay_uses_scan_code_extended_flag_and_stable_marker() {
    let sample = RawKeyboardSample::new("ordinary-keyboard", 0x25, 0x4D, RAW_KEY_E0, 7);

    let replay = replay_keyboard_input(&sample).expect("valid scan-code event");

    assert_eq!(replay.virtual_key(), 0);
    assert_eq!(replay.scan_code(), 0x4D);
    assert_eq!(
        replay.flags() & KeyboardReplay::SCANCODE,
        KeyboardReplay::SCANCODE
    );
    assert_eq!(
        replay.flags() & KeyboardReplay::EXTENDED,
        KeyboardReplay::EXTENDED
    );
    assert_eq!(replay.flags() & KeyboardReplay::KEYUP, 0);
    assert_eq!(replay.extra_info(), REDSAMURAI_SELF_INJECT_MARKER);
}

#[test]
fn replay_marks_break_without_losing_extended_state() {
    let sample = RawKeyboardSample::new(
        "ordinary-keyboard",
        0x25,
        0x4D,
        RAW_KEY_BREAK | RAW_KEY_E0,
        8,
    );

    let replay = replay_keyboard_input(&sample).expect("valid scan-code event");

    assert_ne!(replay.flags() & KeyboardReplay::KEYUP, 0);
    assert_ne!(replay.flags() & KeyboardReplay::EXTENDED, 0);
}

#[test]
fn cleanup_replay_preserves_mapped_output_identity_and_marker() {
    let event = redsamurai_config::keyboard_relay::InjectedKeyEvent::new(
        OutputKey::extended(0x25, 0x4D),
        KeyPhase::Up,
        REDSAMURAI_SELF_INJECT_MARKER,
    );
    let replay = replay_injected_key(event).expect("cleanup output is replayable");
    assert_eq!(replay.virtual_key(), 0);
    assert_eq!(replay.scan_code(), 0x4D);
    assert_ne!(replay.flags() & KeyboardReplay::SCANCODE, 0);
    assert_ne!(replay.flags() & KeyboardReplay::EXTENDED, 0);
    assert_ne!(replay.flags() & KeyboardReplay::KEYUP, 0);
    assert_eq!(replay.extra_info(), REDSAMURAI_SELF_INJECT_MARKER);
}

#[test]
fn side_7_physical_and_self_echo_edges_emit_one_marked_pair() {
    let target = "mi01-keyboard";
    let mut relay = KeyboardRelay::new([target]);
    relay.set_mapping(0x31, OutputKey::virtual_key(0x31));

    let down_packet = RawKeyboardPacket::new(0x31, 0x1E, 0, 100);
    let down_sample = raw_keyboard_to_relay_sample(target, down_packet);
    let down = relay.handle(down_sample.clone());
    assert_eq!(down.decision(), RelayDecision::Mapped);
    assert_eq!(down.events().len(), 1);
    let down_event = match down.events()[0] {
        RelayEvent::Injected(event) => event,
        RelayEvent::Forwarded(_) => {
            panic!("mapped SIDE 7 input must not forward its physical edge")
        }
    };
    assert_eq!(down_event.key(), OutputKey::virtual_key(0x31));
    assert_eq!(down_event.phase(), KeyPhase::Down);
    assert_eq!(down_event.marker(), relay.injection_marker());

    let self_echo_down = down_sample.with_injection_marker(down_event.marker());
    let echoed_down = relay.handle(self_echo_down);
    assert_eq!(echoed_down.decision(), RelayDecision::IgnoredSelfInjected);
    assert!(echoed_down.events().is_empty());

    let up_packet = RawKeyboardPacket::new(0x31, 0x1E, RAW_KEY_BREAK, 120);
    let up_sample = raw_keyboard_to_relay_sample(target, up_packet);
    let up = relay.handle(up_sample.clone());
    assert_eq!(up.decision(), RelayDecision::Mapped);
    assert_eq!(up.events().len(), 1);
    let up_event = match up.events()[0] {
        RelayEvent::Injected(event) => event,
        RelayEvent::Forwarded(_) => {
            panic!("mapped SIDE 7 release must not forward its physical edge")
        }
    };
    assert_eq!(up_event.key(), OutputKey::virtual_key(0x31));
    assert_eq!(up_event.phase(), KeyPhase::Up);
    assert_eq!(up_event.marker(), relay.injection_marker());

    let self_echo_up = up_sample.with_injection_marker(up_event.marker());
    let echoed_up = relay.handle(self_echo_up);
    assert_eq!(echoed_up.decision(), RelayDecision::IgnoredSelfInjected);
    assert!(echoed_up.events().is_empty());
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);
}

#[test]
fn hook_gate_only_suppresses_physical_events_when_ready_and_healthy() {
    assert_eq!(
        classify_hook_event(0, 0, true, true),
        HookDecision::SuppressPhysical
    );
    assert_eq!(
        classify_hook_event(0, 0, false, true),
        HookDecision::PassThrough
    );
    assert_eq!(
        classify_hook_event(0, 0, true, false),
        HookDecision::PassThrough
    );
    assert_eq!(
        classify_hook_event(
            redsamurai_config::keyboard_relay_windows::LLKHF_INJECTED,
            REDSAMURAI_SELF_INJECT_MARKER,
            true,
            true,
        ),
        HookDecision::PassThrough
    );
    assert_eq!(
        classify_hook_event(
            redsamurai_config::keyboard_relay_windows::LLKHF_INJECTED,
            0,
            true,
            true,
        ),
        HookDecision::PassThrough
    );
}

#[test]
fn official_hook_posts_only_the_selected_scan_vk_pair() {
    let mapping = OfficialKeyboardHookMapping::new(0x31, 0x1E);
    let down = OfficialKeyboardHookEvent::new(0, 0x31, 0x1E, 0x00, true);
    let up = OfficialKeyboardHookEvent::new(0, 0x31, 0x1E, 0x80, true);

    assert_eq!(
        classify_official_keyboard_hook(down, mapping),
        OfficialKeyboardHookDecision::PostPrivateMessage {
            virtual_key: 0x31,
            scan_code: 0x1E,
            flags: 0x00,
        }
    );
    assert_eq!(
        classify_official_keyboard_hook(up, mapping),
        OfficialKeyboardHookDecision::PostPrivateMessage {
            virtual_key: 0x31,
            scan_code: 0x1E,
            flags: 0x80,
        }
    );
}

#[test]
fn official_hook_passes_unselected_disabled_negative_and_injected_events() {
    let mapping = OfficialKeyboardHookMapping::new(0x31, 0x1E);
    let cases = [
        OfficialKeyboardHookEvent::new(0, 0x32, 0x1F, 0x00, true),
        OfficialKeyboardHookEvent::new(0, 0x31, 0x1E, 0x00, false),
        OfficialKeyboardHookEvent::new(-1, 0x31, 0x1E, 0x00, true),
        OfficialKeyboardHookEvent::new(
            0,
            0x31,
            0x1E,
            redsamurai_config::keyboard_relay_windows::LLKHF_INJECTED,
            true,
        ),
    ];

    for event in cases {
        assert_eq!(
            classify_official_keyboard_hook(event, mapping),
            OfficialKeyboardHookDecision::PassThrough
        );
    }
}

#[test]
fn official_hook_rewrites_observed_scan_vk_pairs_and_builds_private_message() {
    let observed_pairs = [
        (0x47, 0x24, 0x67),
        (0x48, 0x26, 0x68),
        (0x49, 0x21, 0x69),
        (0x4B, 0x25, 0x64),
        (0x4C, 0x0C, 0x65),
        (0x4D, 0x27, 0x66),
        (0x4F, 0x23, 0x61),
        (0x50, 0x28, 0x62),
        (0x51, 0x22, 0x63),
        (0x52, 0x2D, 0x60),
        (0x53, 0x2E, 0x6E),
    ];

    for (virtual_key, scan_code, rewritten_virtual_key) in observed_pairs {
        let decision = classify_official_keyboard_hook(
            OfficialKeyboardHookEvent::new(0, virtual_key, scan_code, 0x81, true),
            OfficialKeyboardHookMapping::new(virtual_key, scan_code),
        );
        assert_eq!(
            decision,
            OfficialKeyboardHookDecision::PostPrivateMessage {
                virtual_key: rewritten_virtual_key,
                scan_code,
                flags: 0x81,
            }
        );

        let message = decision
            .to_private_message()
            .expect("selected event must produce the vendor message");
        assert_eq!(message.message(), 0x08D2);
        assert_eq!(message.wparam(), rewritten_virtual_key);
        assert_eq!(message.lparam(), 0x81);
    }
}

#[test]
fn official_hook_keeps_unlisted_virtual_keys_and_private_message_flags_bounded() {
    let decision = classify_official_keyboard_hook(
        OfficialKeyboardHookEvent::new(0, 0x31, 0x02, 0xFFFF_FF00, true),
        OfficialKeyboardHookMapping::new(0x31, 0x02),
    );

    assert_eq!(
        decision,
        OfficialKeyboardHookDecision::PostPrivateMessage {
            virtual_key: 0x31,
            scan_code: 0x02,
            flags: 0,
        }
    );
    assert_eq!(
        decision
            .to_private_message()
            .expect("selected event must produce a message")
            .lparam(),
        0
    );
}

#[test]
fn opt_in_is_explicit_and_registration_is_usage_wide_no_legacy() {
    assert!(!relay_opt_in_requested_from(None));
    assert!(!relay_opt_in_requested_from(Some("0")));
    assert!(relay_opt_in_requested_from(Some("1")));
    assert!(relay_opt_in_requested_from(Some("TrUe")));

    let registration = RawInputRegistration::all_keyboard(123);
    assert_eq!(registration.usage_page(), 0x0001);
    assert_eq!(registration.usage(), 0x0006);
    assert_eq!(registration.flags(), RAW_INPUT_REGISTER_FLAGS);
    assert!(registration.no_legacy());
    assert_eq!(registration.target_window(), 123);
}

#[test]
fn relay_probe_keeps_legacy_until_raw_input_is_received() {
    let probe = RawInputRegistration::probe_keyboard(123);
    assert_eq!(probe.usage_page(), 0x0001);
    assert_eq!(probe.usage(), 0x0006);
    assert_eq!(probe.flags(), RAW_INPUT_PROBE_REGISTER_FLAGS);
    assert!(!probe.no_legacy());
    assert_eq!(probe.target_window(), 123);

    let active = RawInputRegistration::all_keyboard(123);
    assert!(active.no_legacy());
}

#[test]
fn low_level_gate_requires_a_separate_explicit_opt_in() {
    assert!(!relay_gate_opt_in_requested_from(None));
    assert!(!relay_gate_opt_in_requested_from(Some("0")));
    assert!(!relay_gate_opt_in_requested_from(Some("false")));
    assert!(relay_gate_opt_in_requested_from(Some("1")));
    assert!(relay_gate_opt_in_requested_from(Some("TRUE")));
}

#[test]
fn observe_only_relay_requires_a_separate_explicit_opt_in() {
    assert!(!relay_observe_only_requested_from(None));
    assert!(!relay_observe_only_requested_from(Some("0")));
    assert!(relay_observe_only_requested_from(Some("1")));
    assert!(relay_observe_only_requested_from(Some("true")));
}

#[test]
fn lifecycle_cannot_activate_without_opt_in_and_cleanup_is_idempotent() {
    let mut disabled = RelayLifecycle::new(false);
    assert_eq!(disabled.state(), RelayLifecycleState::Disabled);
    assert!(disabled.begin().is_err());
    assert!(!disabled.gate_enabled());

    let mut relay = RelayLifecycle::new(true);
    relay.begin().expect("opt-in starts preparation");
    relay.raw_input_ready().expect("raw worker ready");
    relay.hook_ready().expect("hook ready");
    relay.raw_input_registered().expect("registration ready");
    relay.activate_gate().expect("gate starts last");
    assert_eq!(relay.state(), RelayLifecycleState::Active);
    assert!(relay.gate_enabled());

    let cleanup = relay.cleanup();
    assert_eq!(
        cleanup,
        RelayCleanup {
            gate_disabled: true,
            raw_input_unregistered: true,
            hook_stopped: true,
        }
    );
    assert_eq!(relay.state(), RelayLifecycleState::Stopped);
    assert!(!relay.gate_enabled());
    assert_eq!(relay.cleanup(), RelayCleanup::default());
}

#[test]
fn fail_open_releases_hook_gate_before_resource_cleanup() {
    let mut relay = RelayLifecycle::new(true);
    relay.begin().expect("opt-in starts preparation");
    relay.raw_input_ready().expect("raw worker ready");
    relay.hook_ready().expect("hook ready");
    relay.raw_input_registered().expect("registration ready");
    relay.activate_gate().expect("gate starts last");

    relay.fail_open();
    assert_eq!(relay.state(), RelayLifecycleState::Failed);
    assert!(!relay.gate_enabled());
    assert!(relay.is_raw_input_registered());
    assert!(relay.is_hook_ready());
    assert_eq!(
        classify_hook_event(0, 0, relay.gate_enabled(), false),
        HookDecision::PassThrough
    );

    assert_eq!(
        relay.cleanup(),
        RelayCleanup {
            gate_disabled: false,
            raw_input_unregistered: true,
            hook_stopped: true,
        }
    );
    assert_eq!(relay.state(), RelayLifecycleState::Stopped);
    assert!(!relay.is_raw_input_registered());
    assert!(!relay.is_hook_ready());
}
