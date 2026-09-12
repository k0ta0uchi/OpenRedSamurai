//! Macro DB I/O: MacroSet.MSDB + *.MSMACRO — CONTRACT in CONTRACT_phase15.md (Worker 2).
//!
//! Format (verified against real files `777/888.MSMACRO`, `MacroSet.MSDB`):
//! - MSDB: CRLF INI, `[MACRO_LIST]` keys 0..19 = names (empty slots kept),
//!   `[Setting]` Device/FirstOpen preserved verbatim.
//! - MSMACRO: **cp932 (Shift-JIS)** INI, `[Setting]`: MacroFilePath (preserve value),
//!   LoopTime, DefDelayTime, LoopType, DelayType, Count,
//!   MacroSetting_0 = hex of a 190-byte device blob:
//!   [Count records of 6 bytes each:
//!   `[event u8 (0x84=down,0x04=up)][usage u8][0x00][0x06][delay u16 LE]`,
//!   zero padding, checksum byte = sum of the preceding bytes mod 256].
//!
//! Checksum evidence: `777.MSMACRO` (usages 0x24 x6 + 0x28 x2) ends in 0x7D,
//! `888.MSMACRO` (0x25 x6 + 0x28 x2) ends in 0x83 — exactly the byte sums
//! mod 256 (893→125, 899→131).
//!
//! Byte-3 quirk: every nonzero record carries `00 06` in bytes 2..3 except
//! the final nonzero record, which carries `00 00` (seen in both real
//! files). The cause is unknown (possibly an end-marker), so records that
//! were not modified are written back verbatim from [`Macro::orig_hex`];
//! only edited record sets are re-encoded (with `00 06`). Untouched files
//! therefore always round-trip byte-identically.

use crate::ini::IniDoc;
use std::path::{Path, PathBuf};

/// Slots in `[MACRO_LIST]`.
pub const MSDB_SLOTS: usize = 20;

/// Canonical key order for a fresh `.MSMACRO` `[Setting]` section
/// (matches the original files).
const MSMACRO_KEY_ORDER: [&str; 7] = [
    "MacroFilePath",
    "LoopTime",
    "DefDelayTime",
    "LoopType",
    "DelayType",
    "Count",
    "MacroSetting_0",
];

/// One recorded key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroAction {
    /// true = key down (0x84), false = key up (0x04)
    pub down: bool,
    /// USB HID usage id of the key
    pub usage: u8,
    /// delay in ms carried by the record
    pub delay_ms: u16,
}

/// One macro (one .MSMACRO file).
#[derive(Debug, Clone)]
pub struct Macro {
    pub name: String,
    pub loop_time: i32,
    pub def_delay_ms: i32,
    pub loop_type: i32,
    pub delay_type: i32,
    pub actions: Vec<MacroAction>,
    /// Original `MacroSetting_0` hex, verbatim (for byte-exact write-back of
    /// unmodified records, including the byte-3 quirk documented above).
    /// Empty for fresh macros.
    pub orig_hex: String,
    /// Verbatim `MacroFilePath` value (cp932, absolute path). Empty = derive
    /// from the save directory on write.
    pub file_path: String,
}

/// The macro database: names from MSDB + parsed macro bodies.
#[derive(Debug, Clone, Default)]
pub struct MacroDb {
    pub dir: Option<PathBuf>,
    /// slot names as stored in MSDB (may contain empty strings)
    pub names: Vec<String>,
    /// parsed macros for non-empty names, in MSDB slot order
    pub macros: Vec<Macro>,
}

fn decode_cp932(bytes: &[u8]) -> String {
    // SHIFT_JIS in encoding_rs IS Windows-31J (= cp932 superset).
    encoding_rs::SHIFT_JIS.decode(bytes).0.into_owned()
}

fn encode_cp932(text: &str) -> Vec<u8> {
    let (out, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    out.into_owned()
}

fn parse_hex_blob(hex: &str, count: usize) -> Vec<MacroAction> {
    let hex = hex.trim();
    let mut actions = Vec::new();
    for i in 0..count {
        let seg = match hex.get(i * 12..(i + 1) * 12) {
            Some(s) => s,
            None => break,
        };
        let bytes: Option<Vec<u8>> = (0..seg.len())
            .step_by(2)
            .map(|j| u8::from_str_radix(&seg[j..j + 2], 16).ok())
            .collect();
        let bytes = match bytes {
            Some(b) if b.len() == 6 => b,
            _ => break,
        };
        actions.push(MacroAction {
            down: bytes[0] == 0x84 || (bytes[0] != 0x04 && bytes[0] & 0x80 != 0),
            usage: bytes[1],
            delay_ms: u16::from_le_bytes([bytes[4], bytes[5]]),
        });
    }
    actions
}

/// Standard device-blob size in bytes (190 = 380 hex chars, same convention
/// as the profile button blobs). Fresh macros use this size; files with more
/// records grow as needed.
pub const BLOB_BYTES: usize = 190;

fn encode_hex_blob(actions: &[MacroAction], total_bytes: usize) -> String {
    let total_bytes = total_bytes.max(actions.len() * 6 + 1).max(1);
    let mut buf = Vec::with_capacity(total_bytes);
    for a in actions {
        buf.push(if a.down { 0x84u8 } else { 0x04u8 });
        buf.push(a.usage);
        buf.push(0x00);
        buf.push(0x06);
        buf.push((a.delay_ms & 0xFF) as u8);
        buf.push(((a.delay_ms >> 8) & 0xFF) as u8);
    }
    buf.resize(total_bytes - 1, 0);
    let checksum = buf.iter().fold(0u32, |s, &b| s + b as u32) as u8;
    buf.push(checksum);
    let mut out = String::with_capacity(total_bytes * 2);
    for b in buf {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02X}");
    }
    out
}

/// Total blob size in bytes: original size preserved, grown when records
/// overflow it, [`BLOB_BYTES`] for fresh macros.
fn blob_total_bytes(orig_hex_len: usize, actions: usize) -> usize {
    let need = actions * 6 + 1;
    if orig_hex_len == 0 {
        need.max(BLOB_BYTES)
    } else {
        (orig_hex_len / 2).max(need)
    }
}

fn parse_msdb_doc(bytes: &[u8]) -> IniDoc {
    IniDoc::parse(&decode_cp932(bytes))
}

fn parse_macro_file(path: &Path, name: &str) -> Option<Macro> {
    let bytes = std::fs::read(path).ok()?;
    let doc = IniDoc::parse(&decode_cp932(&bytes));
    let sec = doc.section("Setting")?;
    let get_i32 = |key: &str, def: i32| {
        sec.get(key)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(def)
    };
    let count = get_i32("Count", 0).max(0) as usize;
    let hex = sec.get("MacroSetting_0").unwrap_or("").trim().to_string();
    Some(Macro {
        name: name.to_owned(),
        loop_time: get_i32("LoopTime", 1),
        def_delay_ms: get_i32("DefDelayTime", 10),
        loop_type: get_i32("LoopType", 2),
        delay_type: get_i32("DelayType", 2),
        actions: parse_hex_blob(&hex, count),
        orig_hex: hex,
        file_path: sec.get("MacroFilePath").unwrap_or("").to_string(),
    })
}

impl MacroDb {
    fn macro_dir_default() -> Option<PathBuf> {
        crate::profile::Profile::doc_dir().map(|d| d.join("MacroDB"))
    }

    /// `<Documents>\RED SAMURAI 16400DPI Gaming Mouse\MacroDB` があれば読み込む。
    /// 無ければ空。
    pub fn load_default() -> MacroDb {
        match Self::macro_dir_default() {
            Some(dir) if dir.join("MacroSet.MSDB").is_file() => Self::load(&dir),
            Some(dir) => MacroDb {
                dir: Some(dir),
                names: vec![String::new(); MSDB_SLOTS],
                macros: Vec::new(),
            },
            None => MacroDb::default(),
        }
    }

    /// dir から MSDB と各 .MSMACRO を読む。
    pub fn load(dir: &Path) -> MacroDb {
        let mut db = MacroDb {
            dir: Some(dir.to_path_buf()),
            names: vec![String::new(); MSDB_SLOTS],
            macros: Vec::new(),
        };
        let bytes = match std::fs::read(dir.join("MacroSet.MSDB")) {
            Ok(b) => b,
            Err(_) => return db,
        };
        let doc = parse_msdb_doc(&bytes);
        if let Some(list) = doc.section("MACRO_LIST") {
            for i in 0..MSDB_SLOTS {
                let key = i.to_string();
                db.names[i] = list.get(&key).unwrap_or("").to_string();
            }
        }
        for name in db.names.clone() {
            if name.is_empty() {
                continue;
            }
            let path = dir.join(format!("{name}.MSMACRO"));
            if let Some(mac) = parse_macro_file(&path, &name) {
                db.macros.push(mac);
            }
        }
        db
    }

    /// MSDB + 変更のあった MSMACRO を全部書き戻す。
    ///
    /// Existing files are re-parsed first so `[Setting]` keys, key order and
    /// the `MacroFilePath` value survive verbatim; only the managed values
    /// are replaced. Hex blobs are `'0'`-padded to the original total length.
    pub fn save(&self) -> std::io::Result<()> {
        let dir = match &self.dir {
            Some(d) => d.clone(),
            None => return Ok(()),
        };
        std::fs::create_dir_all(&dir)?;

        // ---- MSDB ----
        let msdb_path = dir.join("MacroSet.MSDB");
        let mut doc = std::fs::read(&msdb_path)
            .ok()
            .map(|b| parse_msdb_doc(&b))
            .unwrap_or_else(|| {
                let mut doc = IniDoc::default();
                for i in 0..MSDB_SLOTS {
                    doc.set("MACRO_LIST", &i.to_string(), "");
                }
                doc.set("Setting", "Device", "1");
                doc.set("Setting", "FirstOpen", "1");
                doc
            });
        for i in 0..MSDB_SLOTS {
            let value = self.names.get(i).map(String::as_str).unwrap_or("");
            doc.set("MACRO_LIST", &i.to_string(), value);
        }
        std::fs::write(&msdb_path, encode_cp932(&doc.serialize()))?;

        // ---- MSMACRO bodies ----
        for mac in &self.macros {
            let path = dir.join(format!("{}.MSMACRO", mac.name));
            let existing = std::fs::read(&path).ok();
            let (orig_hex, orig_path) = match &existing {
                Some(b) => {
                    let doc = IniDoc::parse(&decode_cp932(b));
                    let sec = doc.section("Setting");
                    (
                        sec.and_then(|s| s.get("MacroSetting_0"))
                            .map(|v| v.trim().to_string())
                            .unwrap_or_default(),
                        sec.and_then(|s| s.get("MacroFilePath"))
                            .unwrap_or("")
                            .to_string(),
                    )
                }
                None => (String::new(), String::new()),
            };
            let mut doc = existing
                .as_deref()
                .map(|b| IniDoc::parse(&decode_cp932(b)))
                .unwrap_or_default();
            let file_path = if mac.file_path.is_empty() {
                if orig_path.is_empty() {
                    path.to_string_lossy().into_owned()
                } else {
                    orig_path
                }
            } else {
                mac.file_path.clone()
            };
            // Canonical order for fresh files; in-place for existing ones.
            let values: [(&str, String); 7] = [
                ("MacroFilePath", file_path),
                ("LoopTime", mac.loop_time.to_string()),
                ("DefDelayTime", mac.def_delay_ms.to_string()),
                ("LoopType", mac.loop_type.to_string()),
                ("DelayType", mac.delay_type.to_string()),
                ("Count", mac.actions.len().to_string()),
                (
                    "MacroSetting_0",
                    if !mac.orig_hex.is_empty()
                        && parse_hex_blob(&mac.orig_hex, mac.actions.len()) == mac.actions
                        && mac.orig_hex.len() >= mac.actions.len() * 12
                    {
                        // Records untouched → preserve original bytes verbatim
                        // (keeps the byte-3 quirk and checksum identical).
                        mac.orig_hex.clone()
                    } else {
                        encode_hex_blob(
                            &mac.actions,
                            blob_total_bytes(
                                orig_hex.len().max(mac.orig_hex.len()),
                                mac.actions.len(),
                            ),
                        )
                    },
                ),
            ];
            if doc.section("Setting").is_none() {
                for (k, v) in &values {
                    doc.set("Setting", k, v);
                }
            } else {
                // Ensure canonical key order even if the file missed keys.
                let mut ordered: Vec<(String, String)> = Vec::new();
                let sec = doc
                    .section("Setting")
                    .cloned()
                    .unwrap_or(crate::ini::Section {
                        name: "Setting".into(),
                        items: Vec::new(),
                    });
                for key in MSMACRO_KEY_ORDER {
                    if let Some(v) = values.iter().find(|(k, _)| *k == key) {
                        ordered.push((key.to_string(), v.1.clone()));
                    } else if let Some(v) = sec.get(key) {
                        ordered.push((key.to_string(), v.to_string()));
                    }
                }
                // Keep any unknown extra keys after the canonical ones.
                for (k, v) in &sec.items {
                    if !MSMACRO_KEY_ORDER
                        .iter()
                        .any(|ck| ck.eq_ignore_ascii_case(k))
                        && !ordered.iter().any(|(ok, _)| ok.eq_ignore_ascii_case(k))
                    {
                        ordered.push((k.clone(), v.clone()));
                    }
                }
                if let Some(sec_mut) = doc.section_mut("Setting") {
                    sec_mut.items = ordered;
                }
            }
            std::fs::write(&path, encode_cp932(&doc.serialize()))?;
        }
        Ok(())
    }

    pub fn macro_by_name(&self, name: &str) -> Option<&Macro> {
        self.macros.iter().find(|m| m.name == name)
    }

    /// 新規マクロを空きスロットへ追加 (保存は save() で行う)。
    /// 空きが無ければ何もしない。
    pub fn add_macro(&mut self, name: &str) {
        if name.is_empty() || self.names.iter().any(|n| n == name) {
            return;
        }
        let slot = match self.names.iter().position(|n| n.is_empty()) {
            Some(i) => i,
            None => return,
        };
        self.names[slot] = name.to_owned();
        self.macros.push(Macro {
            name: name.to_owned(),
            loop_time: 1,
            def_delay_ms: 10,
            loop_type: 2,
            delay_type: 2,
            actions: Vec::new(),
            orig_hex: String::new(),
            file_path: String::new(),
        });
    }

    /// マクロを削除し MSDB スロットを空にする (.MSMACRO ファイル削除も行う)。
    pub fn delete_macro(&mut self, name: &str) -> std::io::Result<()> {
        self.macros.retain(|m| m.name != name);
        for slot in self.names.iter_mut() {
            if slot == name {
                *slot = String::new();
            }
        }
        if let Some(dir) = &self.dir {
            match std::fs::remove_file(dir.join(format!("{name}.MSMACRO"))) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// マクロ本文を差し替える。存在しなければ追加する。
    pub fn update_macro(&mut self, name: &str, mac: Macro) {
        if let Some(slot) = self.macros.iter_mut().find(|m| m.name == name) {
            *slot = mac;
        } else {
            if !self.names.iter().any(|n| n == name) {
                if let Some(free) = self.names.iter().position(|n| n.is_empty()) {
                    self.names[free] = name.to_owned();
                } else if self.names.len() < MSDB_SLOTS {
                    self.names.push(name.to_owned());
                }
            }
            self.macros.push(mac);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_blob_roundtrip() {
        let actions = vec![
            MacroAction {
                down: true,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: false,
                usage: 0x24,
                delay_ms: 1,
            },
        ];
        let hex = encode_hex_blob(&actions, 2 * 6 + 1);
        assert_eq!(hex, "842400060100042400060100DE");
        let back = parse_hex_blob(&hex, 2);
        assert_eq!(back, actions);
    }

    #[test]
    fn hex_blob_layout_matches_real_777() {
        // First 8 records of the real 777.MSMACRO in a 190-byte blob.
        // NOTE: the encoder writes `00 06` for every record; the real file
        // carries `00 00` in the final nonzero record (byte-3 quirk), which
        // is preserved verbatim on save instead (see save path).
        let actions = vec![
            MacroAction {
                down: true,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: false,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: true,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: false,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: true,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: false,
                usage: 0x24,
                delay_ms: 1,
            },
            MacroAction {
                down: true,
                usage: 0x28,
                delay_ms: 5,
            },
            MacroAction {
                down: false,
                usage: 0x28,
                delay_ms: 0,
            },
        ];
        let hex = encode_hex_blob(&actions, BLOB_BYTES);
        assert_eq!(hex.len(), 380);
        assert!(hex.starts_with(
            "842400060100042400060100842400060100042400060100842400060100042400060100842800060500"
        ));
        assert!(hex[96..378].chars().all(|c| c == '0'));
        // Checksum over the encoded buffer (record 8 encoded with 00 06).
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(bytes.len(), BLOB_BYTES);
        let sum: u32 = bytes[..BLOB_BYTES - 1].iter().map(|&b| b as u32).sum();
        assert_eq!(bytes[BLOB_BYTES - 1], (sum % 256) as u8);
    }
}
