//! Model tests for the platform-independent all-keyboard relay.
//!
//! These tests deliberately stop at relay decisions and synthetic event
//! records.  They do not install a Windows hook, register Raw Input, or send
//! physical input.

use redsamurai_config::keyboard_relay::{
    ActionMapping, DeviceClass, FailOpenReason, InputKey, KeyPhase, KeyboardRelay, OutputKey,
    RawKeyboardSample, RelayDecision, RelayEvent,
};

const TARGET: &str = "target-keyboard";
const ORDINARY: &str = "ordinary-keyboard";

fn down(device: &str, virtual_key: u16, timestamp_ms: u64) -> RawKeyboardSample {
    RawKeyboardSample::key_down(device, virtual_key, 0, false, timestamp_ms)
}

fn up(device: &str, virtual_key: u16, timestamp_ms: u64) -> RawKeyboardSample {
    RawKeyboardSample::key_up(device, virtual_key, 0, false, timestamp_ms)
}

fn output(virtual_key: u16) -> OutputKey {
    OutputKey::virtual_key(virtual_key)
}

fn injected_keys(result: &redsamurai_config::keyboard_relay::RelayResult) -> Vec<(u16, KeyPhase)> {
    result
        .events()
        .iter()
        .filter_map(|event| match event {
            RelayEvent::Injected(event) => Some((event.key().virtual_key_code(), event.phase())),
            RelayEvent::Forwarded(_) => None,
        })
        .collect()
}

fn forwarded_sample(result: &redsamurai_config::keyboard_relay::RelayResult) -> &RawKeyboardSample {
    result
        .events()
        .iter()
        .find_map(|event| match event {
            RelayEvent::Forwarded(sample) => Some(sample),
            RelayEvent::Injected(_) => None,
        })
        .expect("result should contain a forwarded sample")
}

#[test]
fn ordinary_keyboard_edges_are_forwarded_with_the_relay_marker() {
    let mut relay = KeyboardRelay::new([TARGET]);

    let result = relay.handle(down(ORDINARY, 0x41, 10));

    assert_eq!(result.decision(), RelayDecision::PassThrough);
    let forwarded = forwarded_sample(&result);
    assert_eq!(forwarded.device_id().as_str(), ORDINARY);
    assert!(forwarded.is_down());
    assert_eq!(forwarded.injection_marker(), Some(relay.injection_marker()));
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);
}

#[test]
fn ordinary_keyboard_down_up_edges_preserve_identity_and_are_not_replayed() {
    let mut relay = KeyboardRelay::new([TARGET]);
    let physical_down = RawKeyboardSample::key_down(ORDINARY, 0x41, 0x1E, true, 10);
    let physical_up = RawKeyboardSample::key_up(ORDINARY, 0x41, 0x1E, true, 20);

    for physical in [physical_down, physical_up] {
        let result = relay.handle(physical.clone());
        assert_eq!(result.decision(), RelayDecision::PassThrough);
        assert_eq!(result.events().len(), 1);

        let forwarded = forwarded_sample(&result);
        assert_eq!(forwarded.device_id(), physical.device_id());
        assert_eq!(forwarded.virtual_key, physical.virtual_key);
        assert_eq!(forwarded.scan_code, physical.scan_code);
        assert_eq!(forwarded.flags, physical.flags);
        assert_eq!(forwarded.timestamp_ms, physical.timestamp_ms);
        assert_eq!(forwarded.injection_marker(), Some(relay.injection_marker()));

        let replayed = forwarded.clone();
        let replay_result = relay.handle(replayed);
        assert_eq!(replay_result.decision(), RelayDecision::IgnoredSelfInjected);
        assert!(replay_result.events().is_empty());
    }
}

#[test]
fn removed_ordinary_keyboard_releases_a_forwarded_key_once() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.handle(RawKeyboardSample::key_down(ORDINARY, 0x41, 0x1E, true, 10));
    assert_eq!(relay.held_forwarded_count(), 1);

    let removed = relay.remove_device(ORDINARY);
    assert_eq!(removed.decision(), RelayDecision::DeviceRemoved);
    assert_eq!(relay.held_forwarded_count(), 0);
    let release = forwarded_sample(&removed);
    assert!(release.is_up());
    assert_eq!(release.virtual_key, 0x41);
    assert_eq!(release.scan_code, 0x1E);
    assert!(release.input_key().is_extended());
    assert_eq!(release.injection_marker(), Some(relay.injection_marker()));

    let late = relay.handle(RawKeyboardSample::key_up(ORDINARY, 0x41, 0x1E, true, 20));
    assert_eq!(
        late.decision(),
        RelayDecision::FailOpen(FailOpenReason::DeviceUnavailable)
    );
}

#[test]
fn shutdown_releases_forwarded_keys_before_restart() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.handle(down(ORDINARY, 0x41, 1));

    let shutdown = relay.shutdown();
    assert_eq!(shutdown.decision(), RelayDecision::Shutdown);
    let release = forwarded_sample(&shutdown);
    assert!(release.is_up());
    assert_eq!(release.virtual_key, 0x41);
    assert_eq!(relay.held_forwarded_count(), 0);

    relay.restart();
    assert_eq!(relay.held_forwarded_count(), 0);
}

#[test]
fn mapped_target_key_snapshots_and_emits_one_output_pair() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));

    let pressed = relay.handle(down(TARGET, 0x31, 100));
    assert_eq!(pressed.decision(), RelayDecision::Mapped);
    assert_eq!(injected_keys(&pressed), vec![(0x41, KeyPhase::Down)]);
    assert_eq!(relay.output_ref_count(output(0x41)), 1);

    let released = relay.handle(up(TARGET, 0x31, 120));
    assert_eq!(released.decision(), RelayDecision::Mapped);
    assert_eq!(injected_keys(&released), vec![(0x41, KeyPhase::Up)]);
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);
}

#[test]
fn simultaneous_sources_share_a_mapped_output_by_reference_count() {
    let second_target = "target-keyboard-2";
    let mut relay = KeyboardRelay::new([TARGET, second_target]);
    relay.set_mapping(0x31, output(0x41));

    let first_down = relay.handle(down(TARGET, 0x31, 1));
    let second_down = relay.handle(down(second_target, 0x31, 2));
    assert_eq!(injected_keys(&first_down), vec![(0x41, KeyPhase::Down)]);
    assert!(injected_keys(&second_down).is_empty());
    assert_eq!(relay.output_ref_count(output(0x41)), 2);

    let first_up = relay.handle(up(TARGET, 0x31, 3));
    assert!(injected_keys(&first_up).is_empty());
    assert_eq!(relay.output_ref_count(output(0x41)), 1);

    let second_up = relay.handle(up(second_target, 0x31, 4));
    assert_eq!(injected_keys(&second_up), vec![(0x41, KeyPhase::Up)]);
    assert_eq!(relay.output_ref_count(output(0x41)), 0);
}

#[test]
fn mapping_changes_do_not_change_the_route_of_a_held_key() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));

    let pressed = relay.handle(down(TARGET, 0x31, 10));
    relay.set_mapping(0x31, output(0x42));

    let released = relay.handle(up(TARGET, 0x31, 20));
    assert_eq!(injected_keys(&pressed), vec![(0x41, KeyPhase::Down)]);
    assert_eq!(injected_keys(&released), vec![(0x41, KeyPhase::Up)]);
    assert_eq!(relay.output_ref_count(output(0x42)), 0);
}

#[test]
fn missing_release_fails_open_without_creating_a_stuck_output() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));

    let result = relay.handle(up(TARGET, 0x31, 10));

    assert_eq!(
        result.decision(),
        RelayDecision::FailOpen(FailOpenReason::MissingKeyDown)
    );
    assert_eq!(injected_keys(&result), Vec::<(u16, KeyPhase)>::new());
    assert!(forwarded_sample(&result).is_up());
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);
}

#[test]
fn removed_device_releases_held_outputs_and_late_release_fails_open() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));
    relay.handle(down(TARGET, 0x31, 1));

    let removed = relay.remove_device(TARGET);
    assert_eq!(removed.decision(), RelayDecision::DeviceRemoved);
    assert_eq!(injected_keys(&removed), vec![(0x41, KeyPhase::Up)]);
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);

    let late_release = relay.handle(up(TARGET, 0x31, 2));
    assert_eq!(
        late_release.decision(),
        RelayDecision::FailOpen(FailOpenReason::DeviceUnavailable)
    );
    assert!(forwarded_sample(&late_release).is_up());
}

#[test]
fn self_injected_marker_is_ignored_without_affecting_held_state() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));
    let own_marker = relay.injection_marker();
    let sample = down(TARGET, 0x31, 1).with_injection_marker(own_marker);

    let result = relay.handle(sample);

    assert_eq!(result.decision(), RelayDecision::IgnoredSelfInjected);
    assert!(result.events().is_empty());
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);
}

#[test]
fn foreign_injected_target_sample_fails_open_instead_of_being_remapped() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));
    let sample = down(TARGET, 0x31, 1).with_injection_marker(0xDEAD);

    let result = relay.handle(sample);

    assert_eq!(
        result.decision(),
        RelayDecision::FailOpen(FailOpenReason::ForeignInjected)
    );
    assert!(injected_keys(&result).is_empty());
    assert_eq!(
        forwarded_sample(&result).injection_marker(),
        Some(relay.injection_marker())
    );
    assert_eq!(relay.held_input_count(), 0);
}

#[test]
fn shutdown_releases_every_output_once_and_restart_recovers_cleanly() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_mapping(0x31, output(0x41));
    relay.set_mapping(0x32, output(0x41));
    relay.handle(down(TARGET, 0x31, 1));
    relay.handle(down(TARGET, 0x32, 2));

    let shutdown = relay.shutdown();
    assert_eq!(shutdown.decision(), RelayDecision::Shutdown);
    assert_eq!(injected_keys(&shutdown), vec![(0x41, KeyPhase::Up)]);
    assert_eq!(relay.held_input_count(), 0);
    assert_eq!(relay.held_output_count(), 0);

    let while_stopped = relay.handle(down(TARGET, 0x31, 3));
    assert_eq!(
        while_stopped.decision(),
        RelayDecision::FailOpen(FailOpenReason::RelayInactive)
    );
    assert!(forwarded_sample(&while_stopped).is_down());

    relay.restart();
    let after_restart = relay.handle(down(TARGET, 0x31, 4));
    assert_eq!(after_restart.decision(), RelayDecision::Mapped);
    assert_eq!(injected_keys(&after_restart), vec![(0x41, KeyPhase::Down)]);
}

#[test]
fn exact_input_mapping_can_preserve_scan_and_extended_identity() {
    let mut relay = KeyboardRelay::new([TARGET]);
    relay.set_key_mapping(
        InputKey::new(0x31, 0x45, true),
        ActionMapping::from(output(0x42)),
    );

    let ordinary_scan = RawKeyboardSample::key_down(TARGET, 0x31, 0x45, false, 1);
    let exact_scan = RawKeyboardSample::key_down(TARGET, 0x31, 0x45, true, 2);
    assert_eq!(
        relay.handle(ordinary_scan).decision(),
        RelayDecision::FailOpen(FailOpenReason::TargetUnmapped)
    );
    assert_eq!(
        injected_keys(&relay.handle(exact_scan)),
        vec![(0x42, KeyPhase::Down)]
    );
}

#[test]
fn target_unmapped_key_is_explicitly_fail_open() {
    let mut relay = KeyboardRelay::new([TARGET]);

    let result = relay.handle(down(TARGET, 0x7F, 10));

    assert_eq!(
        result.decision(),
        RelayDecision::FailOpen(FailOpenReason::TargetUnmapped)
    );
    assert!(forwarded_sample(&result).is_down());
}

#[test]
fn device_classification_tracks_target_and_removed_devices() {
    let mut relay = KeyboardRelay::new([TARGET]);

    assert_eq!(relay.classify_device(TARGET), DeviceClass::Target);
    assert_eq!(relay.classify_device(ORDINARY), DeviceClass::Ordinary);
    relay.remove_device(TARGET);
    assert_eq!(relay.classify_device(TARGET), DeviceClass::Unavailable);
    relay.mark_device_available(TARGET);
    assert_eq!(relay.classify_device(TARGET), DeviceClass::Target);
}
