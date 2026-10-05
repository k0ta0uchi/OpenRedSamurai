//! Regression seam for device-correlated keyboard duplicate suppression.

use redsamurai_config::keyboard_suppression::{
    KeyboardDuplicateFilter, KeyboardFilterDecision, LegacyKeyboardSample, RawKeyboardSample,
};

fn target_sample(vk: u16, scan_code: u16, flags: u16, time_ms: u32) -> RawKeyboardSample {
    RawKeyboardSample::new(vk, scan_code, flags, time_ms)
}

fn hardware_sample(vk: u16, scan_code: u32, flags: u32, time_ms: u32) -> LegacyKeyboardSample {
    LegacyKeyboardSample::new(vk, scan_code, flags, time_ms)
}

#[test]
fn target_raw_edge_suppresses_only_matching_hardware_legacy_edge() {
    let mut filter = KeyboardDuplicateFilter::new(8);
    filter.observe_target(target_sample(0x31, 0x02, 0, 100));

    assert_eq!(
        filter.classify(hardware_sample(0x31, 0x02, 0, 101)),
        KeyboardFilterDecision::Suppress
    );
    assert_eq!(filter.pending_len(), 0);
}

#[test]
fn ordinary_keyboard_edge_without_target_observation_passes_through() {
    let mut filter = KeyboardDuplicateFilter::new(8);

    assert_eq!(
        filter.classify(hardware_sample(0x41, 0x1E, 0, 100)),
        KeyboardFilterDecision::Pass
    );
}

#[test]
fn injected_edges_are_never_suppressed_or_consumed_as_hardware() {
    let mut filter = KeyboardDuplicateFilter::new(8);
    filter.observe_target(target_sample(0x31, 0x02, 0, 100));

    assert_eq!(
        filter.classify(hardware_sample(0x31, 0x02, 0x10, 0)),
        KeyboardFilterDecision::Pass
    );
    assert_eq!(filter.pending_len(), 1);
    assert_eq!(
        filter.classify(hardware_sample(0x31, 0x02, 0, 102)),
        KeyboardFilterDecision::Suppress
    );
}

#[test]
fn mismatched_or_stale_target_observations_do_not_block_ordinary_input() {
    let mut filter = KeyboardDuplicateFilter::new(8);
    filter.observe_target(target_sample(0x31, 0x02, 0, 100));

    assert_eq!(
        filter.classify(hardware_sample(0x32, 0x03, 0, 101)),
        KeyboardFilterDecision::Pass
    );
    assert_eq!(
        filter.classify(hardware_sample(0x31, 0x02, 0, 120)),
        KeyboardFilterDecision::Pass
    );
    assert_eq!(filter.pending_len(), 0);
}

#[test]
fn press_and_release_edges_are_correlated_independently() {
    let mut filter = KeyboardDuplicateFilter::new(8);
    filter.observe_target(target_sample(0x31, 0x02, 0, 100));
    filter.observe_target(target_sample(0x31, 0x02, 0x01, 150));

    assert_eq!(
        filter.classify(hardware_sample(0x31, 0x02, 0, 101)),
        KeyboardFilterDecision::Suppress
    );
    assert_eq!(
        filter.classify(hardware_sample(0x31, 0x02, 0x80, 151)),
        KeyboardFilterDecision::Suppress
    );
}
