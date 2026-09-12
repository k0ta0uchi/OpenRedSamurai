//! Worker 1 validation — `RSProfile*.pfd` model (CONTRACT_profile.md).
//!
//! - Byte-exact round-trip against the real `RSProfile1.pfd` written by the
//!   original MFC software (Documents/RED SAMURAI 16400DPI Gaming Mouse/).
//! - `default_profile` factory-state tests per the contract.
//!
//! Skipped gracefully when the real sample is not present on the machine.

use redsamurai_config::profile::{dpi_code_to_display, DpiStage, Profile};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Locate the real RSProfile1.pfd: shell Documents folder first (the original
/// software stores profiles in the (possibly OneDrive-redirected) Documents
/// dir), then the known absolute fallback.
fn sample_profile_path() -> Option<PathBuf> {
    if let Some(doc_dir) = Profile::doc_dir() {
        let p = doc_dir.join("RSProfile1.pfd");
        if p.is_file() {
            return Some(p);
        }
    }
    let fallback = PathBuf::from(
        "C:/Users/k0ta0/OneDrive/ドキュメント/RED SAMURAI 16400DPI Gaming Mouse/RSProfile1.pfd",
    );
    if fallback.is_file() {
        return Some(fallback);
    }
    None
}

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "redsamurai_test_{}_{}_{name}",
        std::process::id(),
        nanos
    ))
}

fn dpi_value_of(profile: &Profile, section: &str) -> String {
    profile
        .doc
        .section(section)
        .and_then(|s| s.get("DPIStageValue"))
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// Real-file round trip
// ---------------------------------------------------------------------------

#[test]
fn real_file_roundtrip_is_byte_exact() {
    let Some(src) = sample_profile_path() else {
        eprintln!("skip: real RSProfile1.pfd not found on this machine");
        return;
    };
    let original = std::fs::read(&src).expect("read sample");

    let profile = Profile::load(&src, 1);
    assert_eq!(profile.slot, 1);
    // 42 sections: GROUP0 + 20 slot buttons + legacy GROUP + 20 legacy buttons.
    assert_eq!(profile.doc.sections.len(), 42);

    let tmp = temp_path("roundtrip.pfd");
    profile.save(&tmp).expect("save round-trip");
    let written = std::fs::read(&tmp).expect("read round-trip");
    let _ = std::fs::remove_file(&tmp);

    assert_eq!(
        written.len(),
        original.len(),
        "serialized size differs from original"
    );
    assert_eq!(written, original, "round-trip must be byte-exact");
}

#[test]
fn parse_is_idempotent_on_real_file() {
    let Some(src) = sample_profile_path() else {
        eprintln!("skip: real RSProfile1.pfd not found on this machine");
        return;
    };
    let profile = Profile::load(&src, 1);
    let once = profile.doc.serialize();
    let twice = redsamurai_config::ini::IniDoc::parse(&once).serialize();
    assert_eq!(once, twice, "parse(serialize(x)) must equal serialize(x)");
}

#[test]
fn real_file_dpi_stages_parse() {
    let Some(src) = sample_profile_path() else {
        eprintln!("skip: real RSProfile1.pfd not found on this machine");
        return;
    };
    let profile = Profile::load(&src, 1);
    let stages = profile.dpi_stages();
    let codes: [u32; 5] = stages.map(|s| s.code);
    assert_eq!(codes, [9, 19, 39, 71, 106]);
    assert!(stages.iter().all(|s| s.enabled));
    let displays: [u32; 5] = codes.map(dpi_code_to_display);
    assert_eq!(displays, [1000, 2000, 4000, 7200, 10700]);
    // DPICurrentX=14 in the original file → not a valid stage index → 0.
    assert_eq!(profile.current_dpi_stage(), 0);
    // LedColor1=255 → COLORREF red.
    assert_eq!(profile.led_color_rgb(), [255, 0, 0]);
    // Slot button facts from the real file.
    assert_eq!(profile.button(1).func, 1);
    assert_eq!(profile.button(4).func, 5);
    assert_eq!(profile.button(4).loop_number, 3);
    assert_eq!(profile.button(19).func, 52);
    assert_eq!(profile.button(20).func, 53);
    // Blob byte0 of button 7 is the HID code for glyph "1".
    assert_eq!(profile.button_key_glyph(7), Some(0));
    assert_eq!(profile.button_key_glyph(18), Some(11));
    assert_eq!(profile.button_key_glyph(1), None);
}

#[test]
fn set_dpi_stage_rewrites_segment_and_preserves_trailing_bytes() {
    let Some(src) = sample_profile_path() else {
        eprintln!("skip: real RSProfile1.pfd not found on this machine");
        return;
    };
    let mut profile = Profile::load(&src, 1);
    let before_slot = dpi_value_of(&profile, "GROUP0");
    let before_legacy = dpi_value_of(&profile, "GROUP");
    assert_eq!(before_slot.len(), 818, "observed original value size");
    assert_eq!(before_legacy, before_slot);

    profile.set_dpi_stage(
        0,
        DpiStage {
            enabled: false,
            code: 20,
        },
    );

    let after_slot = dpi_value_of(&profile, "GROUP0");
    let after_legacy = dpi_value_of(&profile, "GROUP");
    assert_eq!(
        after_slot.len(),
        before_slot.len(),
        "length must not change"
    );
    assert_eq!(after_legacy, after_slot, "legacy mirror updated too");
    // Stage 0 record (16 bytes = 32 hex chars) replaced...
    assert_eq!(
        &after_slot[0..32],
        "00000000140000001400000001000000",
        "enable u32=0 + X=Y=20 little-endian + flag u32=1"
    );
    // ...everything after the first stage identical.
    assert_eq!(&after_slot[32..], &before_slot[32..]);

    // Round-trip through disk still parses back to the edited stage.
    let tmp = temp_path("dpi_edit.pfd");
    profile.save(&tmp).expect("save");
    let reloaded = Profile::load(&tmp, 1);
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(
        reloaded.dpi_stages()[0],
        DpiStage {
            enabled: false,
            code: 20
        }
    );
    assert_eq!(
        reloaded.dpi_stages()[1],
        DpiStage {
            enabled: true,
            code: 19
        }
    );
}

// ---------------------------------------------------------------------------
// default_profile
// ---------------------------------------------------------------------------

#[test]
fn default_profile_factory_values() {
    let p = Profile::default_profile(1);
    // DPI stages 1000/2000/4000/8200/16400 all enabled.
    let stages = p.dpi_stages();
    let codes: [u32; 5] = stages.map(|s| s.code);
    assert_eq!(codes, [9, 19, 39, 81, 163]);
    assert!(stages.iter().all(|s| s.enabled));
    let displays: [u32; 5] = codes.map(dpi_code_to_display);
    assert_eq!(displays, [1000, 2000, 4000, 8200, 16400]);

    // 1000 Hz polling, red LED.
    assert_eq!(p.get_i32("PollingRate", 0), 8);
    assert_eq!(p.led_color_rgb(), [255, 0, 0]);

    // Buttons: 1=左,2=右,3=中,4=戻る,5/6=中,7..18=シングルキー,19/20=52/53.
    let funcs: Vec<u16> = (1..=20).map(|m| p.button(m).func).collect();
    assert_eq!(
        funcs,
        vec![1, 2, 3, 5, 3, 3, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 52, 53]
    );
    // Single-key glyphs for buttons 7..18 = keys 1..0,-,^ (glyph idx 0..=11).
    let glyphs: Vec<Option<usize>> = (7..=18).map(|m| p.button_key_glyph(m)).collect();
    assert_eq!(glyphs, (0..=11).map(Some).collect::<Vec<_>>());
    assert_eq!(p.button_key_glyph(4), None);

    // Structure matches the original files: 42 sections with the legacy mirror.
    assert_eq!(p.doc.sections.len(), 42);
    assert!(p.doc.section("GROUP0").is_some());
    assert!(p.doc.section("GROUP").is_some());
    assert_eq!(dpi_value_of(&p, "GROUP0").len(), 818);
    assert_eq!(dpi_value_of(&p, "GROUP"), dpi_value_of(&p, "GROUP0"));
}

#[test]
fn default_profile_save_load_roundtrip() {
    let p = Profile::default_profile(3);
    let tmp = temp_path("default.pfd");
    p.save(&tmp).expect("save default");
    let text = std::fs::read_to_string(&tmp).expect("read default");
    let _ = std::fs::remove_file(&tmp);

    // CRLF line endings, order preserved.
    assert!(
        !text.contains('\n') || text.contains("\r\n"),
        "must be CRLF"
    );
    let names: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with('['))
        .map(|l| l.trim_end_matches('\r'))
        .collect();
    assert_eq!(names[0], "[GROUP2]");
    assert_eq!(names[1], "[ButtonAssigned2_1]");
    assert_eq!(names[20], "[ButtonAssigned2_20]");
    assert_eq!(names[21], "[GROUP]");
    assert_eq!(*names.last().unwrap(), "[ButtonAssigned_20]");

    let reloaded = Profile::load(&tmp, 3);
    assert_eq!(reloaded.doc.serialize(), text, "reload must be lossless");
    assert_eq!(reloaded.led_color_rgb(), [255, 0, 0]);
    assert_eq!(reloaded.dpi_stages()[4].code, 163);
    assert_eq!(reloaded.button_key_glyph(10), Some(3));
}

#[test]
fn scalar_writes_touch_slot_and_legacy_sections() {
    let mut p = Profile::default_profile(1);
    p.set_i32("MouseSensitivity", 11);
    assert_eq!(p.get_i32("MouseSensitivity", 0), 11);
    for section in ["GROUP0", "GROUP"] {
        assert_eq!(
            p.doc
                .section(section)
                .and_then(|s| s.get("MouseSensitivity")),
            Some("11"),
            "{section} must be updated"
        );
    }

    p.set_led_color_rgb([0, 0, 255]); // blue
    assert_eq!(p.get_i32("LedColor1", 0), 16711680);
    for section in ["GROUP0", "GROUP"] {
        assert_eq!(
            p.doc.section(section).and_then(|s| s.get("LedColor1")),
            Some("16711680")
        );
    }

    p.set_current_dpi_stage(2);
    assert_eq!(p.current_dpi_stage(), 2);
    assert_eq!(p.get_i32("DPICurrentY", -1), 2);

    // set_dpi_stage also mirrors into the legacy section.
    p.set_dpi_stage(
        4,
        DpiStage {
            enabled: false,
            code: 100,
        },
    );
    assert_eq!(dpi_value_of(&p, "GROUP"), dpi_value_of(&p, "GROUP0"));
    assert_eq!(
        p.dpi_stages()[4],
        DpiStage {
            enabled: false,
            code: 100
        }
    );
}

#[test]
fn writes_skip_missing_legacy_mirror() {
    // Simulate a factory file with only slot sections (RSProfile4/5 style).
    let src = "[GROUP0]\r\nMouseSensitivity=15\r\nDPIStageNum=5\r\nDPIStageValue=01000000090000000900000001000000010000001900000019000000010000000100000027000000270000000100000001000000470000004700000001000000010000006A0000006A0000000100000001000000000000000000000000000000FF01FF\r\n[ButtonAssigned0_1]\r\nButtonID=255\r\nButtonFunc=1\r\nLoopNumber=255\r\nKeyNumber=0\r\nMacroName=\r\n";
    let mut p = Profile {
        slot: 1,
        doc: redsamurai_config::ini::IniDoc::parse(src),
    };
    assert!(p.doc.section("GROUP").is_none());

    p.set_i32("MouseSensitivity", 13);
    assert_eq!(p.get_i32("MouseSensitivity", 0), 13);
    assert!(
        p.doc.section("GROUP").is_none(),
        "legacy must not be created"
    );

    // get_i32 falls back to the legacy GROUP when the slot section lacks the key.
    let with_legacy = "[GROUP0]\r\n[GROUP]\r\nPollingRate=8\r\n";
    let p2 = Profile {
        slot: 1,
        doc: redsamurai_config::ini::IniDoc::parse(with_legacy),
    };
    assert_eq!(p2.get_i32("PollingRate", 0), 8);
}

#[test]
fn set_button_single_key_writes_slot_blob_only() {
    let mut p = Profile::default_profile(1);
    p.set_button_single_key(19, 5); // button 19 → single key glyph "6"

    let assign = p.button(19);
    assert_eq!(assign.func, 6);
    assert_eq!(assign.key_number, 5);

    // Slot blob byte0 updated (0x23 = glyph idx 5).
    let slot_blob = p
        .doc
        .section("GROUP0")
        .and_then(|s| s.get("ButtonAssigned0_19"))
        .expect("slot blob");
    assert_eq!(&slot_blob[0..2], "23");
    assert_eq!(slot_blob.len(), 380);
    assert_eq!(p.button_key_glyph(19), Some(5));

    // Legacy blob byte0 untouched.
    let legacy_blob = p
        .doc
        .section("GROUP")
        .and_then(|s| s.get("ButtonAssigned_19"))
        .expect("legacy blob");
    assert_eq!(&legacy_blob[0..2], "00");

    // Legacy button section got ButtonFunc=6 too (KeyNumber as well).
    let legacy_sec = p
        .doc
        .section("ButtonAssigned_19")
        .expect("legacy button sec");
    assert_eq!(legacy_sec.get("ButtonFunc"), Some("6"));
    assert_eq!(legacy_sec.get("KeyNumber"), Some("5"));
}

#[test]
fn load_falls_back_to_defaults_on_error() {
    let missing = temp_path("does_not_exist.pfd");
    let _ = std::fs::remove_file(&missing);
    let p = Profile::load(&missing, 1);
    assert_eq!(p.dpi_stages()[0].code, 9);
    assert_eq!(p.led_color_rgb(), [255, 0, 0]);
    assert_eq!(p.button(1).func, 1);
}
