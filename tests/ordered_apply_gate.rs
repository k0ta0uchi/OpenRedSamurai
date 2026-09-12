//! Adversarial checks for the evidence-backed ordered Apply sequence.
//!
//! These tests intentionally exercise the production sequence authorizer.  A
//! cloned report vector is only accepted when it is the exact 156-report
//! template; no test-local template or raw transport shortcut is used.

use redsamurai_config::device_apply::{ApplyError, ApplyPlan, VerifiedSequenceTransport};
use redsamurai_config::device_protocol::{
    CommandCapability, VerifiedApplySequence, APPLY_DPI_SELECTION_FRAME_INDEX, CMD_F1, CMD_F3,
    DPI_SELECTION_APPLY_OBSERVED_OFFSET, EXTENDED_REPORT_LEN, PARAMS_OFFSET,
    POLLING_RATE_OBSERVED_OFFSET, REPORT_ID_CONFIG, REPORT_ID_CONFIG_EXTENDED, REPORT_LEN,
    SUBCMD_F1_02, SUBCMD_F3_20, SUBCMD_F3_32, SUBCMD_F3_38,
};
use redsamurai_config::ini::IniDoc;
use redsamurai_config::profile::Profile;

const APPLY_FRAME_COUNT: usize = 156;
const POLLING_FRAME_INDEX: usize = 13;
const EXTENDED_FRAME_INDEX: usize = 15;

fn authorized_apply_plan() -> ApplyPlan {
    let mut doc = IniDoc::default();
    doc.set("GROUP0", "PollingRate", "8");
    ApplyPlan::try_build_authorized(&Profile { slot: 1, doc })
        .expect("the audited 125 Hz profile must build an authorized plan")
}

fn authorized_125hz_dpi_sequence(profile_dpi: i32) -> VerifiedApplySequence {
    VerifiedApplySequence::for_profile_polling_rate_and_dpi(8, profile_dpi)
        .expect("the A/B-proven DPI selection must be authorized")
}

#[derive(Default)]
struct CountingSequenceTransport {
    calls: usize,
    acknowledged: usize,
    sequence_lengths: Vec<usize>,
}

impl VerifiedSequenceTransport for CountingSequenceTransport {
    type Error = &'static str;

    fn send_verified_sequence(
        &mut self,
        sequence: &VerifiedApplySequence,
    ) -> Result<usize, Self::Error> {
        self.calls += 1;
        self.sequence_lengths.push(sequence.len());
        Ok(self.acknowledged)
    }
}

fn authorized_125hz_sequence() -> VerifiedApplySequence {
    VerifiedApplySequence::for_profile_polling_rate(8)
        .expect("the audited 125 Hz sequence must be constructible")
}

fn encode_all(sequence: &VerifiedApplySequence) -> Vec<Vec<u8>> {
    sequence
        .frames()
        .iter()
        .map(|frame| frame.encode().expect("authorized frame must encode"))
        .collect()
}

fn hex_bytes(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).expect("hex") as u8;
            let low = (pair[1] as char).to_digit(16).expect("hex") as u8;
            (high << 4) | low
        })
        .collect()
}

#[test]
fn authorized_sequence_is_exactly_156_frames_and_validates() {
    let sequence = authorized_125hz_sequence();

    assert_eq!(sequence.frames().len(), APPLY_FRAME_COUNT);
    assert_eq!(sequence.len(), APPLY_FRAME_COUNT);
    VerifiedApplySequence::try_from_frames(sequence.frames())
        .expect("the production sequence must validate");

    let frames = encode_all(&sequence);
    assert_eq!(frames.len(), APPLY_FRAME_COUNT);
    assert!(frames
        .iter()
        .all(|frame| { frame.len() == REPORT_LEN || frame.len() == EXTENDED_REPORT_LEN }));
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame.len() == EXTENDED_REPORT_LEN)
            .count(),
        1
    );
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame.len() == REPORT_LEN)
            .count(),
        155
    );
}

#[test]
fn polling_rate_is_the_only_audited_substitution_in_the_sequence() {
    let sequence = authorized_125hz_sequence();
    let baseline = encode_all(&sequence);

    // The four independently observed profile values map to these wire bytes.
    // The current write-authorized constructor intentionally accepts only the
    // physically verified 125 Hz profile value (8 -> 0x08); the other values
    // remain evidence-only until their own reconnect/readback proof exists.
    for (profile_value, wire_value) in [(8u8, 0x08u8), (4, 0x04), (2, 0x02), (1, 0x01)] {
        let mut expected = baseline.clone();
        expected[POLLING_FRAME_INDEX][POLLING_RATE_OBSERVED_OFFSET] = wire_value;
        assert_eq!(
            &expected[POLLING_FRAME_INDEX][..3],
            &[REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_32],
            "profile value {profile_value} must target the observed polling header"
        );

        let changed_indices: Vec<usize> = expected
            .iter()
            .zip(&baseline)
            .enumerate()
            .filter_map(|(index, (candidate, original))| (candidate != original).then_some(index))
            .collect();

        if profile_value == 8 {
            assert_eq!(expected, baseline);
            assert!(changed_indices.is_empty());
        } else {
            assert_eq!(changed_indices, vec![POLLING_FRAME_INDEX]);
            assert!(
                VerifiedApplySequence::for_profile_polling_rate(i32::from(profile_value)).is_err(),
                "unproven profile value {profile_value} must not mint write authority"
            );
        }
    }
}

#[test]
fn selected_dpi_is_the_single_authorized_apply_substitution() {
    let dpi_one = encode_all(&authorized_125hz_dpi_sequence(1));
    let dpi_two = encode_all(&authorized_125hz_dpi_sequence(2));

    assert_eq!(dpi_one.len(), APPLY_FRAME_COUNT);
    assert_eq!(dpi_two.len(), APPLY_FRAME_COUNT);
    assert_eq!(
        dpi_one[APPLY_DPI_SELECTION_FRAME_INDEX],
        hex_bytes("02f34200020000000100000000000000")
    );
    assert_eq!(
        dpi_two[APPLY_DPI_SELECTION_FRAME_INDEX],
        hex_bytes("02f34200020000000200000000000000")
    );
    let changed_indices: Vec<usize> = dpi_one
        .iter()
        .zip(&dpi_two)
        .enumerate()
        .filter_map(|(index, (one, two))| (one != two).then_some(index))
        .collect();
    assert_eq!(changed_indices, vec![APPLY_DPI_SELECTION_FRAME_INDEX]);
    assert_eq!(
        dpi_one[APPLY_DPI_SELECTION_FRAME_INDEX][DPI_SELECTION_APPLY_OBSERVED_OFFSET],
        0x01
    );
    assert_eq!(
        dpi_two[APPLY_DPI_SELECTION_FRAME_INDEX][DPI_SELECTION_APPLY_OBSERVED_OFFSET],
        0x02
    );
}

#[test]
fn unobserved_dpi_selection_values_remain_fail_closed() {
    for profile_dpi in [0, 3, 4, 5, 255] {
        assert!(
            VerifiedApplySequence::for_profile_polling_rate_and_dpi(8, profile_dpi).is_err(),
            "DPI profile value {profile_dpi} must remain unverified"
        );
    }
}

#[test]
fn authorized_profile_uses_the_proven_dpi_selection_in_its_sequence() {
    let mut doc = IniDoc::default();
    doc.set("GROUP0", "PollingRate", "8");
    doc.set("GROUP0", "DPI", "2");
    let plan = ApplyPlan::try_build_authorized(&Profile { slot: 1, doc })
        .expect("the proven DPI selection must build an authorized plan");

    assert!(plan.warnings.is_empty());
    let sequence = plan
        .verified_sequence()
        .expect("authorized profile must retain the ordered sequence");
    assert_eq!(
        sequence.frames()[APPLY_DPI_SELECTION_FRAME_INDEX]
            .encode()
            .expect("DPI selection frame must encode"),
        hex_bytes("02f34200020000000200000000000000")
    );
}

#[test]
fn extended_frame_and_f1_f5_boundaries_are_preserved() {
    let frames = encode_all(&authorized_125hz_sequence());

    assert_eq!(&frames[0][..3], &[REPORT_ID_CONFIG, 0xF5, 0x00]);
    assert_eq!(frames[0].len(), REPORT_LEN);

    assert_eq!(
        &frames[EXTENDED_FRAME_INDEX][..3],
        &[REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_20]
    );
    assert_eq!(frames[EXTENDED_FRAME_INDEX].len(), EXTENDED_REPORT_LEN);
    assert_eq!(frames[EXTENDED_FRAME_INDEX][16], 0x01);
    assert!(frames[EXTENDED_FRAME_INDEX][17..]
        .iter()
        .all(|byte| *byte == 0));

    // These values are the first parameter byte of the observed F1/02
    // markers.  They are not F1 subcommands: the complete wire reports in
    // usbpcap_evidence.rs are 02/F1/02/<marker>/....
    for &(index, marker) in &[
        (21usize, 0x02u8),
        (22, 0x10),
        (48, 0x10),
        (50, 0x01),
        (151, 0x04),
        (152, 0x01),
        (153, 0x02),
        (154, 0x08),
    ] {
        assert_eq!(
            &frames[index][..3],
            &[REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_02],
            "F1 marker moved at sequence index {index}"
        );
        assert_eq!(
            frames[index][PARAMS_OFFSET], marker,
            "F1 parameter marker changed at sequence index {index}"
        );
        assert_eq!(frames[index].len(), REPORT_LEN);
    }

    let last = frames.last().expect("sequence terminator");
    assert_eq!(&last[..3], &[REPORT_ID_CONFIG, 0xF5, 0x01]);
    assert_eq!(last.len(), REPORT_LEN);
}

#[test]
fn malformed_order_length_header_and_standalone_polling_frame_fail_before_io() {
    let authorized = authorized_125hz_sequence();

    let standalone = vec![authorized.frames()[POLLING_FRAME_INDEX].clone()];
    assert!(
        VerifiedApplySequence::try_from_frames(&standalone).is_err(),
        "a polling frame alone must not become a verified sequence"
    );

    let mut wrong_order = authorized.frames().to_vec();
    wrong_order.swap(0, 1);
    assert!(
        VerifiedApplySequence::try_from_frames(&wrong_order).is_err(),
        "reordered frames must be rejected before transport"
    );

    let mut wrong_length = authorized.frames().to_vec();
    wrong_length[EXTENDED_FRAME_INDEX].params.pop();
    assert!(
        VerifiedApplySequence::try_from_frames(&wrong_length).is_err(),
        "a malformed 64-byte frame must be rejected before transport"
    );

    let mut wrong_header = authorized.frames().to_vec();
    wrong_header[POLLING_FRAME_INDEX].subcmd = SUBCMD_F3_38;
    assert!(
        VerifiedApplySequence::try_from_frames(&wrong_header).is_err(),
        "a header substitution must be rejected before transport"
    );

    let mut wrong_report_id = authorized.frames().to_vec();
    wrong_report_id[POLLING_FRAME_INDEX].report_id = REPORT_ID_CONFIG_EXTENDED;
    assert!(
        VerifiedApplySequence::try_from_frames(&wrong_report_id).is_err(),
        "a report-id substitution must be rejected before transport"
    );
}

#[test]
fn standalone_report_shape_stays_non_writable() {
    let frame = authorized_125hz_sequence().frames()[POLLING_FRAME_INDEX].clone();

    assert_eq!(frame.capability(), CommandCapability::Unsupported);
    assert!(!frame.can_write());
    assert_eq!(frame.expected_len(), REPORT_LEN);
    assert_eq!(frame.params.len(), REPORT_LEN - PARAMS_OFFSET);
    assert_eq!(frame.report_id, REPORT_ID_CONFIG);
    assert_eq!(frame.cmd, CMD_F3);
    assert_eq!(frame.subcmd, SUBCMD_F3_32);
    assert_eq!(
        frame.params[POLLING_RATE_OBSERVED_OFFSET - PARAMS_OFFSET],
        0x08
    );
}

#[test]
fn sequence_transport_requires_exact_completion_and_blocks_raw_entrypoints() {
    let plan = authorized_apply_plan();
    assert!(plan.verified_sequence().is_some());

    let mut accepted = CountingSequenceTransport {
        acknowledged: APPLY_FRAME_COUNT,
        ..Default::default()
    };
    let outcome = plan
        .apply_verified_sequence(&mut accepted)
        .expect("the complete authorized sequence must be transportable");
    assert_eq!(outcome.sent, APPLY_FRAME_COUNT);
    assert_eq!(accepted.calls, 1);
    assert_eq!(accepted.sequence_lengths, vec![APPLY_FRAME_COUNT]);

    let mut incomplete = CountingSequenceTransport {
        acknowledged: APPLY_FRAME_COUNT - 1,
        ..Default::default()
    };
    assert_eq!(
        plan.apply_verified_sequence(&mut incomplete),
        Err(ApplyError::InvalidSequenceCompletion {
            actual: APPLY_FRAME_COUNT - 1,
            expected: APPLY_FRAME_COUNT,
        })
    );
    assert_eq!(incomplete.calls, 1);
    assert_eq!(incomplete.sequence_lengths, vec![APPLY_FRAME_COUNT]);

    let mut raw_calls = 0;
    let error = plan
        .apply_with(|_| {
            raw_calls += 1;
            Ok::<(), &'static str>(())
        })
        .expect_err("an authorized sequence must use the sequence-gated seam");
    assert_eq!(error, ApplyError::VerifiedSequenceRequiresGatedApply);
    assert_eq!(raw_calls, 0);
}

#[test]
fn missing_sequence_fails_before_transport_callback() {
    let mut transport = CountingSequenceTransport {
        acknowledged: APPLY_FRAME_COUNT,
        ..Default::default()
    };

    assert_eq!(
        ApplyPlan::default().apply_verified_sequence(&mut transport),
        Err(ApplyError::MissingVerifiedSequence)
    );
    assert_eq!(transport.calls, 0);
    assert!(transport.sequence_lengths.is_empty());
}
