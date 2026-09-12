//! Macro DB round-trip tests against the real user files (Worker 2).
//!
//! Real samples (cp932):
//! `<Documents>\RED SAMURAI 16400DPI Gaming Mouse\MacroDB\`
//! `{MacroSet.MSDB, 777.MSMACRO, 888.MSMACRO}`.
//! All tests skip gracefully when the files are absent.

use redsamurai_config::ini::IniDoc;
use redsamurai_config::macro_db::MacroDb;
use std::path::PathBuf;

fn macro_dir() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("RED SAMURAI 16400DPI Gaming Mouse").join("MacroDB"))
}

fn decode_cp932(bytes: &[u8]) -> String {
    encoding_rs::SHIFT_JIS.decode(bytes).0.into_owned()
}

/// Copy the real MacroDB into a temp dir for save-then-compare tests.
fn stage_temp_copy(src: &PathBuf) -> Option<PathBuf> {
    let dst = std::env::temp_dir().join(format!("redsamurai-macro-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).ok()?;
    for name in ["MacroSet.MSDB", "777.MSMACRO", "888.MSMACRO"] {
        let bytes = std::fs::read(src.join(name)).ok()?;
        std::fs::write(dst.join(name), bytes).ok()?;
    }
    Some(dst)
}

#[test]
fn msdb_parse_serialize_byte_exact() {
    let Some(dir) = macro_dir() else { return };
    let path = dir.join("MacroSet.MSDB");
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let text = decode_cp932(&bytes);
    let doc = IniDoc::parse(&text);
    let serialized = doc.serialize();
    let (out, _, _) = encoding_rs::SHIFT_JIS.encode(&serialized);
    assert_eq!(
        out.as_ref(),
        bytes.as_slice(),
        "MSDB parse->serialize must be byte-exact"
    );
}

#[test]
fn msmacro_parse_serialize_byte_exact() {
    let Some(dir) = macro_dir() else { return };
    let path = dir.join("777.MSMACRO");
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let text = decode_cp932(&bytes);
    let doc = IniDoc::parse(&text);
    let serialized = doc.serialize();
    let (out, _, _) = encoding_rs::SHIFT_JIS.encode(&serialized);
    assert_eq!(
        out.as_ref(),
        bytes.as_slice(),
        "777.MSMACRO parse->serialize must be byte-exact"
    );
}

#[test]
fn macro_db_save_is_byte_exact() {
    let Some(dir) = macro_dir() else { return };
    if !dir.join("MacroSet.MSDB").is_file() {
        return;
    }
    let Some(tmp) = stage_temp_copy(&dir) else {
        return;
    };
    let db = MacroDb::load(&tmp);
    db.save().expect("save to temp copy");
    for name in ["MacroSet.MSDB", "777.MSMACRO", "888.MSMACRO"] {
        let before = std::fs::read(dir.join(name)).unwrap();
        let after = std::fs::read(tmp.join(name)).unwrap();
        assert_eq!(before, after, "{name} must survive load->save byte-exact");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn parse_real_777_contents() {
    let Some(dir) = macro_dir() else { return };
    if !dir.join("777.MSMACRO").is_file() {
        return;
    }
    let db = MacroDb::load(&dir);
    assert_eq!(db.names.len(), 20);
    assert_eq!(db.names[0], "777");
    assert_eq!(db.names[1], "888");
    let mac = db
        .macro_by_name("777")
        .expect("macro 777 must parse from the real MacroDB");
    // Real file: Count=15; first 8 records are key events
    // (0x24 x6 as three down/up pairs, then 0x28 down/up),
    // records 9..15 are zero padding words.
    assert_eq!(mac.actions.len(), 15, "Count=15 → 15 records");
    let usages: Vec<u8> = mac.actions.iter().take(8).map(|a| a.usage).collect();
    assert_eq!(usages, vec![0x24, 0x24, 0x24, 0x24, 0x24, 0x24, 0x28, 0x28]);
    assert_eq!(
        mac.actions
            .iter()
            .take(6)
            .map(|a| a.down)
            .collect::<Vec<_>>(),
        vec![true, false, true, false, true, false]
    );
    assert_eq!(mac.actions[0].delay_ms, 1, "first delay u16 LE = 1");
    assert!(
        mac.actions[8..].iter().all(|a| a.usage == 0),
        "records 9..15 are zero padding"
    );
    assert_eq!(mac.loop_time, 1);
    assert_eq!(mac.def_delay_ms, 10);
}
