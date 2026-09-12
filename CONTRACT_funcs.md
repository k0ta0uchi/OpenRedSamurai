//! Button-function model, labels and assignment menu — CONTRACT (implemented by Worker 2).
//!
//! Source: `ANALYSIS.md` section 4 (FUNCTION_STRING from general.ini).
//! Known-correct function ids: 1=左クリック 2=右クリック 3=中央ボタン 4=進む 5=戻る
//! 6=シングルキー 12=ファイアキー 13=DPIスイッチ 14=プロファイルスイッチ
//! 15=レポートレート 16=無効 100=マクロ.
//! Observed-but-unverified ids must render as `機能#N` (52/53 = DPI +/- is our working guess,
//! marked in code comments). Sub-feature ids are Phase 2 → use `MenuAction::Stub`.

/// Menu action produced by the assignment menu.
# [derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Plain function id → ButtonFunc.
    SetFunc(u16),
    /// Single key index 0..=11 → func=6 + KeyNumber + blob byte.
    SetSingleKey(usize),
    /// Function id + KeyNumber (DPIスイッチ/プロファイルスイッチ/レポートレート sub choices).
    SetFuncWithKey(u16, u8),
    /// Phase 2 placeholder (shows a toast with this text).
    Stub(&'static str),
}

# [derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: Option<MenuAction>,
    pub children: Vec<MenuItem>,
}

/// Labels shown in the assignment rows (FRONT: ①..⑥, SIDE: ①..⑫).
pub fn func_label(func: u16, single_key_glyph: Option<usize>) -> String;

/// The full right-click menu tree (matches screenshot 3):
/// 左クリック/右クリック/中央ボタン/進む/戻る/シングルキー▶(keys)/コンボキー▶(stub)/
/// ベーシック▶(stub items)/アドバンスト▶(stub items)/メディア▶(stub items)/マクロ▶(stub)/
/// ファイアキー…(stub)/DPIスイッチ▶(サイクル=13, +,− stub)/プロファイルスイッチ▶(14 + key 0..4)/
/// レポートレート▶(15 + key 0..3)/LEDモードスイッチ(stub)/無効(16).
pub fn build_menu() -> Vec<MenuItem>;

/// Side keypad glyphs in order 0..=11.
pub const KEY_GLYPHS: [&str; 12];
/// Device-blob byte for each glyph (observed from a real RSProfile1.pfd:
/// 0x1E..0x27 for 1..0, 0x2D for '-', 0x34 for '^').
pub const KEY_BLOB_CODES: [u8; 12];
/// Map a blob byte back to glyph index (None if unknown).
pub fn blob_code_to_glyph(b: u8) -> Option<usize>;
