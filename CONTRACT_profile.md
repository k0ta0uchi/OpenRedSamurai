//! Profile model for `RSProfile*.pfd` — CONTRACT (implemented by Worker 1).
//!
//! Data source: `ANALYSIS.md` section 4. File format: plain INI, CRLF.
//! Each file holds ONE profile slot: `[GROUP{n-1}]` + `[ButtonAssigned{n-1}_1..20]`
//! plus a legacy mirror `[GROUP]` + `[ButtonAssigned_1..20]`.
//!
//! Rules:
//! - Round-trip MUST be lossless (serialize(parse(x)) == x when nothing changed).
//! - Scalar edits write BOTH the slot section and the legacy section (if present).
//! - `ButtonAssigned{n}_*` scalar keys (ButtonID/ButtonFunc/LoopNumber/KeyNumber/MacroName)
//!   are authoritative. The long hex blobs (`ButtonAssigned{n}_N=` inside GROUP sections)
//!   are opaque device tables: byte0 only is written for single-key assignments.

use crate::ini::IniDoc;
use std::path::PathBuf;

/// One DPI stage as stored in `DPIStageValue`.
/// CORRECTED after real-file verification by W1: 16 bytes (32 hex chars) per stage:
/// [enable u32 LE][X code le32][Y code le32][flag u32 LE (observed 1, opaque)].
/// Codes 9,19,39,71,106 observed in RSProfile1.pfd.

# [derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct DpiStage {
    pub enabled: bool,
    /// Raw stored code (e.g. 9). Display DPI = (code+1)*100.
    pub code: u32,
}

/// One button assignment (`[ButtonAssigned{n-1}_{m}]` section).

# [derive(Debug, Clone, PartialEq, Eq)]

pub struct ButtonAssign {
    pub button_id: u8,
    pub func: u16,
    pub loop_number: u8,
    pub key_number: u8,
    pub macro_name: String,
}

# [derive(Debug, Clone)]

pub struct Profile {
    /// 1..=5
    pub slot: usize,
    pub doc: IniDoc,
}

impl Profile {
    /// `[GROUP{n-1}]`
    pub fn group_section(&self) -> String;
    /// `[ButtonAssigned{n-1}_{m}]`
    pub fn button_section(&self, m: usize) -> String;

    /// `<Documents>\RED SAMURAI 16400DPI Gaming Mouse`
    pub fn doc_dir() -> Option<PathBuf>;
    /// `<doc_dir>\RSProfile{slot}.pfd`
    pub fn profile_path(slot: usize) -> Option<PathBuf>;

    /// Load from `path`. On any error return `Self::default_profile(slot)`.
    pub fn load(path: &std::path::Path, slot: usize) -> Profile;
    /// Factory defaults matching the original screenshots
    /// (DPI 1000/2000/4000/8200/16400 all enabled, 125Hz polling, red LED,
    /// buttons: 1=左,2=右,3=中,4=戻る,5/6=中央,7..18=シングルキー 1..0,-,^,19/20=52/53).
    pub fn default_profile(slot: usize) -> Profile;
    /// Serialize and write. Creates parent dirs. CRLF, INI order preserved.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()>;

    // ---- scalar accessors (read slot section first, fall back to legacy, then default) ----
    pub fn get_i32(&self, key: &str, def: i32) -> i32;
    pub fn set_i32(&mut self, key: &str, v: i32); // writes slot + legacy (when present)

    /// 5 stages parsed from `DPIStageValue` (first `DPIStageNum`*32 hex chars; rest preserved).
    pub fn dpi_stages(&self) -> [DpiStage; 5];
    /// Rewrite the stage segment inside `DPIStageValue`, preserving any trailing bytes.
    pub fn set_dpi_stage(&mut self, idx: usize, stage: DpiStage);

    /// `DPICurrentX` interpreted as current stage index if 0..5, else 0.
    pub fn current_dpi_stage(&self) -> usize;
    pub fn set_current_dpi_stage(&mut self, idx: usize);

    /// `LedColor1` stored as Windows COLORREF (0x00BBGGRR). Returns RGB bytes.
    pub fn led_color_rgb(&self) -> [u8; 3];
    pub fn set_led_color_rgb(&mut self, rgb: [u8; 3]);

    /// Button assignment `m` in 1..=20.
    pub fn button(&self, m: usize) -> ButtonAssign;
    /// Set ButtonFunc in slot + legacy button section.
    pub fn set_button_func(&mut self, m: usize, func: u16);
    /// Single-key assignment: func=6, KeyNumber=idx, blob byte0 = KEY_BLOB_CODES[idx]
    /// (slot blob only), other blob bytes untouched (zeros for a fresh assignment).
    pub fn set_button_single_key(&mut self, m: usize, key_idx: usize);
    /// Recover single-key glyph index (0..=11) from blob byte0; None when not a known key.
    pub fn button_key_glyph(&self, m: usize) -> Option<usize>;
}

/// Display DPI for a stored code: `(code+1)*100` clamped to 100..=16400.
pub fn dpi_code_to_display(code: u32) -> u32;
/// Stored code for a display DPI: `(d/100)-1` clamped to 0..=163.
pub fn dpi_display_to_code(display: u32) -> u32;
