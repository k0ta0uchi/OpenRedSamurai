//! RSProfile*.pfd model — CONTRACT in CONTRACT_profile.md (Worker 1).
//!
//! File format (observed in real `RSProfile1.pfd`): plain INI, CRLF, one slot
//! `[GROUP{n-1}]` + `[ButtonAssigned{n-1}_1..20]` plus a legacy mirror
//! `[GROUP]` + `[ButtonAssigned_1..20]` (the legacy button sections have no
//! `LoopNumber` key). Some factory files (RSProfile4/5) omit the legacy mirror.
//!
//! - `DPIStageValue` is a 409-byte device table in hex (818 chars): 5 stage
//!   entries of 16 bytes each (`enable u32 + X le32 + Y le32 + flag u32=1`,
//!   i.e. 32 hex chars per stage), followed by an opaque trailing device
//!   table that must be preserved verbatim.
//!   NOTE: the earlier working note "24 hex chars per stage (enable u8 +
//!   pad3 + X + Y)" misreads the real files — at 12-byte stride the stage
//!   codes come out as garbage [9,1,1,39,71]; the 16-byte layout yields
//!   9/19/39/71/106, exactly the values recorded in ANALYSIS.md section 4.
//! - `ButtonAssigned{n-1}_{m}=` inside the GROUP sections is a 190-byte blob
//!   (380 hex chars): byte0 carries the assignment code (the original files
//!   mirror it in byte189); only byte0 is ever written here.
//! - `LedColor1` is a Windows COLORREF (`0x00BBGGRR`): 255=red, 65280=green,
//!   16711680=blue.

use crate::funcs::KEY_BLOB_CODES;
use crate::ini::{IniDoc, Section};
use std::io;
use std::path::{Path, PathBuf};

/// Hex chars per DPI stage inside `DPIStageValue` (16 bytes per stage).
const STAGE_HEX: usize = 32;
/// Hex chars per button assignment blob (190 bytes).
const BLOB_HEX: usize = 380;
/// DPI stage count.
const DPI_STAGES: usize = 5;
/// Button count per profile.
const BUTTONS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiStage {
    pub enabled: bool,
    pub code: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireTarget {
    MouseLeft,
    MouseRight,
    MouseMiddle,
    Keyboard(u8),
}

impl FireTarget {
    pub fn to_key_number(self) -> u8 {
        match self {
            FireTarget::MouseLeft => 1,
            FireTarget::MouseRight => 2,
            FireTarget::MouseMiddle => 3,
            FireTarget::Keyboard(u) => u,
        }
    }

    pub fn from_key_number(k: u8) -> Self {
        match k {
            1 => FireTarget::MouseLeft,
            2 => FireTarget::MouseRight,
            3 => FireTarget::MouseMiddle,
            u => FireTarget::Keyboard(u),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonAssign {
    pub button_id: u8,
    pub func: u16,
    pub loop_number: u8,
    pub key_number: u8,
    pub macro_name: String,
}

#[derive(Debug, Clone)]
pub struct Profile {
    /// 1..=5
    pub slot: usize,
    pub doc: IniDoc,
}

// ---------------------------------------------------------------------------
// hex helpers
// ---------------------------------------------------------------------------

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02X}");
    }
    out
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Encode one DPI stage: `enable u32 + X le32 + Y le32 + flag u32=1` (16 bytes).
fn stage_hex(stage: &DpiStage) -> String {
    let mut b = [0u8; 16];
    b[0..4].copy_from_slice(&(u8::from(stage.enabled) as u32).to_le_bytes());
    b[4..8].copy_from_slice(&stage.code.to_le_bytes());
    b[8..12].copy_from_slice(&stage.code.to_le_bytes());
    b[12..16].copy_from_slice(&1u32.to_le_bytes());
    to_hex(&b)
}

fn parse_stage_hex(seg: &str) -> Option<DpiStage> {
    let b = from_hex(seg)?;
    if b.len() < 16 {
        return None;
    }
    Some(DpiStage {
        enabled: u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 1,
        code: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
    })
}

/// Opaque device-table tail that follows the 5 stage entries in
/// `DPIStageValue` (409 − 5×16 = 329 bytes, byte-identical across the
/// observed original `RSProfile*.pfd` files). Kept byte-for-byte so freshly
/// written default profiles stay structurally identical to the ones the
/// original software produces; `set_dpi_stage` preserves whatever trailing
/// data is present.
fn dpi_tail_bytes() -> Vec<u8> {
    let mut b = Vec::with_capacity(329);
    b.extend_from_slice(&1u32.to_le_bytes()); // trailing entry: enabled
    b.extend_from_slice(&[0u8; 12]); // trailing entry X=Y=0
    b.extend_from_slice(&[0xFF, 0x01, 0xFF]);
    b.extend_from_slice(&[0u8; 309]);
    b.push(0xF2);
    b
}

/// Full `DPIStageValue` for a fresh default profile: 5 stages + device tail.
fn default_dpi_value_hex() -> String {
    let mut out = String::with_capacity(DPI_STAGES * STAGE_HEX + 329 * 2);
    for stage in dpi_stages_default() {
        out.push_str(&stage_hex(&stage));
    }
    out.push_str(&to_hex(&dpi_tail_bytes()));
    out
}

fn dpi_stages_default() -> [DpiStage; DPI_STAGES] {
    // 1000 / 2000 / 4000 / 8200 / 16400 DPI, all enabled (factory screenshots).
    [
        DpiStage {
            enabled: true,
            code: 9,
        },
        DpiStage {
            enabled: true,
            code: 19,
        },
        DpiStage {
            enabled: true,
            code: 39,
        },
        DpiStage {
            enabled: true,
            code: 81,
        },
        DpiStage {
            enabled: true,
            code: 163,
        },
    ]
}

/// 190-byte button blob for a fresh assignment: code at byte0, mirrored in the
/// last byte (original files keep `blob[0] == blob[189]`), zeros in between.
fn blob_with_byte0(byte0: u8) -> String {
    let len = BLOB_HEX / 2;
    let mut b = vec![0u8; len];
    b[0] = byte0;
    b[len - 1] = byte0;
    to_hex(&b)
}

/// Factory `ButtonFunc` for button `m` (1..=20), matching the original
/// screenshots: 1=左, 2=右, 3=中, 4=戻る(5), 5/6=中, 7..18=シングルキー(6),
/// 19=52, 20=53.
fn default_button_func(m: usize) -> u16 {
    match m {
        1..=3 => m as u16,
        4 => 5,
        5..=6 => 3,
        7..=18 => 6,
        19 => 52,
        20 => 53,
        _ => 255,
    }
}

/// Factory `LoopNumber` (button 4 carries 3 in the original files).
fn default_loop_number(m: usize) -> u8 {
    if m == 4 {
        3
    } else {
        255
    }
}

/// Blob byte0 for button `m` on a factory profile.
fn default_blob_byte0(m: usize) -> u8 {
    match m {
        4 => 0xF0,                       // 戻る
        7..=18 => KEY_BLOB_CODES[m - 7], // シングルキー 1..0,-,^
        _ => 0x00,
    }
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

impl Profile {
    /// `[GROUP{n-1}]`
    pub fn group_section(&self) -> String {
        format!("GROUP{}", self.slot - 1)
    }

    /// `[ButtonAssigned{n-1}_{m}]` — also the blob key inside the GROUP section.
    pub fn button_section(&self, m: usize) -> String {
        format!("ButtonAssigned{}_{}", self.slot - 1, m)
    }

    /// Legacy mirror button section `[ButtonAssigned_{m}]` (no slot index).
    fn legacy_button_section(m: usize) -> String {
        format!("ButtonAssigned_{m}")
    }

    /// Section names a scalar write must touch: the slot group plus the legacy
    /// `[GROUP]` mirror, but only when the mirror is present.
    fn scalar_sections(&self) -> Vec<String> {
        let mut v = vec![self.group_section()];
        if self.doc.section("GROUP").is_some() {
            v.push("GROUP".to_string());
        }
        v
    }

    /// `<Documents>\RED SAMURAI 16400DPI Gaming Mouse`
    pub fn doc_dir() -> Option<PathBuf> {
        dirs::document_dir().map(|d| d.join("RED SAMURAI 16400DPI Gaming Mouse"))
    }

    /// `<doc_dir>\RSProfile{slot}.pfd`
    pub fn profile_path(slot: usize) -> Option<PathBuf> {
        Self::doc_dir().map(|d| d.join(format!("RSProfile{slot}.pfd")))
    }

    /// Load from `path`. On any error return `Self::default_profile(slot)`.
    pub fn load(path: &Path, slot: usize) -> Profile {
        let slot = slot.clamp(1, 5);
        match std::fs::read_to_string(path) {
            Ok(text) => Profile {
                slot,
                doc: IniDoc::parse(&text),
            },
            Err(_) => Self::default_profile(slot),
        }
    }

    /// Factory defaults matching the original screenshots
    /// (DPI 1000/2000/4000/8200/16400 all enabled, 125Hz polling, red LED,
    /// buttons: 1=左,2=右,3=中,4=戻る,5/6=中央,7..18=シングルキー 1..0,-,^,19/20=52/53).
    pub fn default_profile(slot: usize) -> Profile {
        let slot = slot.clamp(1, 5);
        let g = slot - 1;
        let mut doc = IniDoc::default();

        let scalar_items = |with_group_id: bool| -> Vec<(String, String)> {
            let mut items: Vec<(String, String)> = Vec::new();
            if with_group_id {
                items.push(("GroupID".into(), "0".into()));
            }
            items.extend([
                ("MouseSensitivity".into(), "15".into()),
                ("Group".into(), "1".into()),
                ("PollingRate".into(), "8".into()), // 125 Hz
                ("WheelScrollLines".into(), "1".into()),
                ("DoubleClickSpeed".into(), "5".into()),
                ("UniversalScrollEnabled".into(), "0".into()),
                ("Acceleration".into(), "1".into()),
                ("XYSensitivityEnabled".into(), "0".into()),
                ("MouseSpeedX".into(), "10".into()),
                ("MouseSpeedY".into(), "10".into()),
                ("OFASensitivity".into(), "10".into()),
                ("LedMode1".into(), "3".into()),
                ("LedState1".into(), "2".into()),
                ("FlashState1".into(), "5".into()),
                ("BreathState1".into(), "5".into()),
                ("LedColor1".into(), "255".into()), // COLORREF red
                ("ANGLE".into(), "0".into()),
                ("LIFT".into(), "3".into()),
                ("DPI".into(), "0".into()),
                ("DPICurrentX".into(), "14".into()), // value used by original files
                ("DPICurrentY".into(), "14".into()),
                ("DPIStageNum".into(), "5".into()),
                ("DPIStageValue".into(), default_dpi_value_hex()),
            ]);
            items
        };

        // Section order as observed in original files:
        // [GROUP{n-1}], [ButtonAssigned{n-1}_1..20], [GROUP], [ButtonAssigned_1..20].
        let mut group = Section {
            name: format!("GROUP{g}"),
            items: scalar_items(true),
        };
        for m in 1..=BUTTONS {
            group.items.push((
                format!("ButtonAssigned{g}_{m}"),
                blob_with_byte0(default_blob_byte0(m)),
            ));
        }
        doc.sections.push(group);

        for m in 1..=BUTTONS {
            doc.sections.push(Section {
                name: format!("ButtonAssigned{g}_{m}"),
                items: vec![
                    ("ButtonID".into(), "255".into()),
                    ("ButtonFunc".into(), default_button_func(m).to_string()),
                    ("LoopNumber".into(), default_loop_number(m).to_string()),
                    ("KeyNumber".into(), "0".into()),
                    ("MacroName".into(), String::new()),
                ],
            });
        }

        // Legacy mirror (key layout without GroupID / LoopNumber, as observed).
        let mut legacy = Section {
            name: "GROUP".into(),
            items: scalar_items(false),
        };
        for m in 1..=BUTTONS {
            legacy.items.push((
                format!("ButtonAssigned_{m}"),
                blob_with_byte0(default_blob_byte0(m)),
            ));
        }
        doc.sections.push(legacy);

        for m in 1..=BUTTONS {
            doc.sections.push(Section {
                name: Self::legacy_button_section(m),
                items: vec![
                    ("ButtonID".into(), "255".into()),
                    ("ButtonFunc".into(), default_button_func(m).to_string()),
                    ("KeyNumber".into(), "0".into()),
                    ("MacroName".into(), String::new()),
                ],
            });
        }

        Profile { slot, doc }
    }

    /// Serialize and write. Creates parent dirs. CRLF, INI order preserved.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.doc.serialize())
    }

    // ---- scalar accessors (read slot section first, fall back to legacy, then default) ----

    pub fn get_i32(&self, key: &str, def: i32) -> i32 {
        self.doc
            .section(&self.group_section())
            .and_then(|s| s.get_parse::<i32>(key))
            .or_else(|| {
                self.doc
                    .section("GROUP")
                    .and_then(|s| s.get_parse::<i32>(key))
            })
            .unwrap_or(def)
    }

    /// Writes slot group section + legacy `[GROUP]` (when present).
    pub fn set_i32(&mut self, key: &str, v: i32) {
        let value = v.to_string();
        for section in self.scalar_sections() {
            self.doc.set(&section, key, &value);
        }
    }

    /// 5 stages parsed from `DPIStageValue` (first `DPIStageNum`*24 hex chars; rest preserved).
    pub fn dpi_stages(&self) -> [DpiStage; 5] {
        let num = self.get_i32("DPIStageNum", 5).clamp(0, 5) as usize;
        let raw = self
            .doc
            .section(&self.group_section())
            .and_then(|s| s.get("DPIStageValue"))
            .unwrap_or("");
        let mut out = [DpiStage {
            enabled: false,
            code: 0,
        }; 5];
        for (i, stage) in out.iter_mut().enumerate().take(num) {
            if let Some(seg) = raw.get(i * STAGE_HEX..(i + 1) * STAGE_HEX) {
                if let Some(parsed) = parse_stage_hex(seg) {
                    *stage = parsed;
                }
            }
        }
        out
    }

    /// Rewrite the stage segment inside `DPIStageValue`, preserving any trailing bytes.
    /// Writes slot + legacy `[GROUP]` (when present).
    pub fn set_dpi_stage(&mut self, idx: usize, stage: DpiStage) {
        if idx >= 5 {
            return;
        }
        let seg = stage_hex(&stage);
        let start = idx * STAGE_HEX;
        let end = start + STAGE_HEX;
        for section in self.scalar_sections() {
            let Some(sec) = self.doc.section_mut(&section) else {
                continue;
            };
            let mut value = sec.get("DPIStageValue").unwrap_or("").to_string();
            if value.len() < end {
                value.extend(std::iter::repeat_n('0', end - value.len()));
            }
            value.replace_range(start..end, &seg);
            sec.set("DPIStageValue", &value);
        }
    }

    /// `DPICurrentX` interpreted as current stage index if 0..5, else 0.
    pub fn current_dpi_stage(&self) -> usize {
        let v = self.get_i32("DPICurrentX", 0);
        if (0..5).contains(&v) {
            v as usize
        } else {
            0
        }
    }

    pub fn set_current_dpi_stage(&mut self, idx: usize) {
        let idx = idx.min(4) as i32;
        self.set_i32("DPICurrentX", idx);
        self.set_i32("DPICurrentY", idx);
    }

    /// `LedColor1` stored as Windows COLORREF (0x00BBGGRR). Returns RGB bytes.
    pub fn led_color_rgb(&self) -> [u8; 3] {
        let v = self.get_i32("LedColor1", 0x0000FF) as u32; // default: red
        [
            (v & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            ((v >> 16) & 0xFF) as u8,
        ]
    }

    pub fn set_led_color_rgb(&mut self, rgb: [u8; 3]) {
        let v = rgb[0] as i32 | (rgb[1] as i32) << 8 | (rgb[2] as i32) << 16;
        self.set_i32("LedColor1", v);
    }

    /// Button assignment `m` in 1..=20.
    pub fn button(&self, m: usize) -> ButtonAssign {
        if !(1..=BUTTONS).contains(&m) {
            return ButtonAssign {
                button_id: 255,
                func: 255,
                loop_number: 255,
                key_number: 0,
                macro_name: String::new(),
            };
        }
        let slot_sec = self.doc.section(&self.button_section(m));
        let legacy_sec = self.doc.section(&Self::legacy_button_section(m));
        let val = |key: &str, def: &str| -> String {
            slot_sec
                .and_then(|s| s.get(key))
                .or_else(|| legacy_sec.and_then(|s| s.get(key)))
                .unwrap_or(def)
                .to_string()
        };
        ButtonAssign {
            button_id: val("ButtonID", "255").trim().parse().unwrap_or(255),
            func: val("ButtonFunc", "255").trim().parse().unwrap_or(255),
            loop_number: val("LoopNumber", "255").trim().parse().unwrap_or(255),
            key_number: val("KeyNumber", "0").trim().parse().unwrap_or(0),
            macro_name: val("MacroName", ""),
        }
    }

    /// Write one button-section scalar to slot + legacy button section (when present).
    fn set_button_scalar(&mut self, m: usize, key: &str, value: &str) {
        if !(1..=BUTTONS).contains(&m) {
            return;
        }
        let slot_name = self.button_section(m);
        self.doc.set(&slot_name, key, value);
        let legacy_name = Self::legacy_button_section(m);
        if self.doc.section(&legacy_name).is_some() {
            self.doc.set(&legacy_name, key, value);
        }
    }

    /// Set ButtonFunc in slot + legacy button section.
    pub fn set_button_func(&mut self, m: usize, func: u16) {
        self.set_button_scalar(m, "ButtonFunc", &func.to_string());
    }

    /// Single-key assignment: func=6, KeyNumber=idx, blob byte0 = KEY_BLOB_CODES[idx]
    /// (slot blob only), other blob bytes untouched (zeros for a fresh assignment).
    pub fn set_button_single_key(&mut self, m: usize, key_idx: usize) {
        if !(1..=BUTTONS).contains(&m) || key_idx >= KEY_BLOB_CODES.len() {
            return;
        }
        self.set_button_func(m, 6);
        self.set_button_scalar(m, "KeyNumber", &key_idx.to_string());
        // The blob lives inside the GROUP section, keyed `ButtonAssigned{n-1}_{m}`;
        // only byte0 is written, only in the slot section.
        let blob_key = self.button_section(m);
        let group = self.group_section();
        if let Some(sec) = self.doc.section_mut(&group) {
            let value = sec.get(&blob_key).unwrap_or("");
            let rest = if value.len() >= 2 { &value[2..] } else { "" };
            let new_value = format!("{:02X}{rest}", KEY_BLOB_CODES[key_idx]);
            sec.set(&blob_key, &new_value);
        }
    }

    /// シングルキー割当を HID usage で直接指定 (dialog 用)。
    /// func=6, KeyNumber=usage, blob byte0=usage (slot GROUP section only,
    /// mirroring [`Self::set_button_single_key`] but with a raw HID usage
    /// instead of a side-keypad glyph index).
    pub fn set_button_single_key_usage(&mut self, m: usize, usage: u8) {
        if !(1..=BUTTONS).contains(&m) {
            return;
        }
        self.set_button_func(m, 6);
        self.set_button_scalar(m, "KeyNumber", &usage.to_string());
        let blob_key = self.button_section(m);
        let group = self.group_section();
        if let Some(sec) = self.doc.section_mut(&group) {
            let value = sec.get(&blob_key).unwrap_or("");
            let rest = if value.len() >= 2 { &value[2..] } else { "" };
            let new_value = format!("{usage:02X}{rest}");
            sec.set(&blob_key, &new_value);
        }
    }

    /// マクロ割当: func=100, MacroName=name (slot + legacy button sections).
    pub fn set_button_macro(&mut self, m: usize, name: &str) {
        if !(1..=BUTTONS).contains(&m) {
            return;
        }
        self.set_button_func(m, 100);
        self.set_button_scalar(m, "MacroName", name);
    }

    /// コンボキー割当 (推定エンコード, Phase2で検証):
    /// func=7, KeyNumber=usage, LoopNumber=modmask (slot + legacy sections).
    pub fn set_button_combo(&mut self, m: usize, usage: u8, modifiers: u8) {
        if !(1..=BUTTONS).contains(&m) {
            return;
        }
        self.set_button_func(m, 7);
        self.set_button_scalar(m, "KeyNumber", &usage.to_string());
        self.set_button_scalar(m, "LoopNumber", &modifiers.to_string());
    }

    /// ファイアキー割当 (連打機能):
    /// func=12, LoopNumber=times, KeyNumber=target, MacroName=delay_ms.to_string()
    pub fn set_button_fire(&mut self, m: usize, target: FireTarget, times: u8, delay_ms: u16) {
        if !(1..=BUTTONS).contains(&m) {
            return;
        }
        let key_num = target.to_key_number();
        self.set_button_func(m, 12);
        self.set_button_scalar(m, "KeyNumber", &key_num.to_string());
        self.set_button_scalar(m, "LoopNumber", &times.to_string());
        self.set_button_scalar(m, "MacroName", &delay_ms.to_string());

        let blob_key = self.button_section(m);
        let group = self.group_section();
        if let Some(sec) = self.doc.section_mut(&group) {
            let value = sec.get(&blob_key).unwrap_or("");
            let rest = if value.len() >= 2 { &value[2..] } else { "" };
            let new_value = format!("{key_num:02X}{rest}");
            sec.set(&blob_key, &new_value);
        }
    }

    /// Recover single-key glyph index (0..=11) from blob byte0; None when not a known key.
    pub fn button_key_glyph(&self, m: usize) -> Option<usize> {
        if !(1..=BUTTONS).contains(&m) {
            return None;
        }
        let blob = self
            .doc
            .section(&self.group_section())?
            .get(&self.button_section(m))?;
        let byte0 = u8::from_str_radix(blob.get(0..2)?, 16).ok()?;
        KEY_BLOB_CODES.iter().position(|&c| c == byte0)
    }
}

/// Display DPI for a stored code: `(code+1)*100` clamped to 100..=16400.
pub fn dpi_code_to_display(code: u32) -> u32 {
    (code.saturating_add(1).saturating_mul(100)).clamp(100, 16400)
}

/// Stored code for a display DPI: `(d/100)-1` clamped to 0..=163.
pub fn dpi_display_to_code(display: u32) -> u32 {
    (display / 100).saturating_sub(1).min(163)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_code_display_conversion() {
        for (code, display) in [
            (9u32, 1000u32),
            (19, 2000),
            (39, 4000),
            (81, 8200),
            (163, 16400),
        ] {
            assert_eq!(dpi_code_to_display(code), display);
            assert_eq!(dpi_display_to_code(display), code);
        }
        assert_eq!(dpi_code_to_display(0), 100);
        assert_eq!(dpi_code_to_display(u32::MAX), 16400);
        assert_eq!(dpi_display_to_code(0), 0);
        assert_eq!(dpi_display_to_code(999_999_999), 163);
    }

    #[test]
    fn stage_hex_roundtrip() {
        for enabled in [true, false] {
            for code in [0u32, 9, 71, 163] {
                let stage = DpiStage { enabled, code };
                let hex = stage_hex(&stage);
                assert_eq!(hex.len(), STAGE_HEX);
                assert_eq!(parse_stage_hex(&hex), Some(stage));
            }
        }
        // Layout check against the observed file:
        // enable u32 + X le32 + Y le32 + flag u32.
        assert_eq!(
            stage_hex(&DpiStage {
                enabled: true,
                code: 9
            }),
            "01000000090000000900000001000000"
        );
    }

    #[test]
    fn parse_stage_hex_rejects_bad_input() {
        assert_eq!(parse_stage_hex("010000000900000009000000"), None); // too short
        assert_eq!(parse_stage_hex("0"), None); // odd length
        assert_eq!(parse_stage_hex("010000000900000009000000010000ZZ"), None); // bad hex
    }

    #[test]
    fn dialog_assignment_methods() {
        let mut p = Profile::default_profile(1);
        // Single-key by HID usage: func=6, KeyNumber=usage, blob byte0=usage.
        p.set_button_single_key_usage(7, 0x04);
        let b = p.button(7);
        assert_eq!((b.func, b.key_number), (6, 0x04));
        let blob = p
            .doc
            .section(&p.group_section())
            .and_then(|s| s.get(&p.button_section(7)))
            .unwrap_or("");
        assert!(blob.starts_with("04"), "blob byte0 = usage, got {blob}");
        // Macro: func=100 + MacroName in slot and legacy sections.
        p.set_button_macro(8, "777");
        let b = p.button(8);
        assert_eq!((b.func, b.macro_name.as_str()), (100, "777"));
        // Combo (estimated encoding): func=7, KeyNumber=usage, LoopNumber=mask.
        p.set_button_combo(9, 0x1D, 0b0001);
        let b = p.button(9);
        assert_eq!((b.func, b.key_number, b.loop_number), (7, 0x1D, 1));
        // Fire key: func=12, KeyNumber=target, LoopNumber=times, MacroName=delay_ms.
        p.set_button_fire(10, FireTarget::MouseLeft, 5, 20);
        let b = p.button(10);
        assert_eq!(
            (b.func, b.key_number, b.loop_number, b.macro_name.as_str()),
            (12, 1, 5, "20")
        );
        // Out-of-range buttons are ignored.
        p.set_button_single_key_usage(0, 0x04);
        p.set_button_macro(21, "x");
        p.set_button_combo(99, 0x04, 0);
        p.set_button_fire(0, FireTarget::MouseRight, 3, 0);
    }
}
