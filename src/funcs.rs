//! Button functions, labels, assignment menu — CONTRACT in CONTRACT_funcs.md (Worker 2).
//!
//! Source: `ANALYSIS.md` §4 = `images/jp/strings/general.ini` `[FUNCTION_STRING]`
//! plus the Phase 1.5 contract (`CONTRACT_phase15.md`, from `ldcfg.exe`
//! reverse engineering) for sub-feature ids:
//! basic=16..23, advanced=24..37, media=38..46, 47=DPI cycle, 48/49 and
//! 52/53=DPI +/-, 54=LED switch, 11=double-click, 100=macro.
//!
//! NOTE (id 16 collision, Phase2 to verify): the shipped `FUNCTION_STRING`
//! maps 16 to 無効 (DISABLE), while the reverse-engineered basic submenu
//! starts at 16 (切り取り). This implementation follows the Phase 1.5
//! contract (16..23 = basic); the 無効 menu leaf still writes `SetFunc(16)`
//! so disabling keeps working, but an assigned row with func=16 currently
//! renders with the basic label. USBPcap capture in Phase 2 must
//! disambiguate the real device encoding.

/// Menu action produced by the assignment menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Plain function id → ButtonFunc.
    SetFunc(u16),
    /// Single key index 0..=11 → func=6 + KeyNumber + blob byte.
    SetSingleKey(usize),
    /// Function id + KeyNumber (DPIスイッチ/プロファイルスイッチ/レポートレート sub choices).
    SetFuncWithKey(u16, u8),
    /// マクロを割り当てる (func=100 + MacroName)。
    SetMacro(String),
    /// シングルキー設定ダイアログ (キーキャプチャ) を開く。
    OpenSingleKeyDialog,
    /// コンボキー設定ダイアログを開く。
    OpenComboDialog,
    /// ファイア設定ダイアログを開く。
    OpenFireDialog,
    /// マクロマネージャーを開く。
    OpenMacroManager,
    /// Phase 2 placeholder (shows a toast with this text).
    Stub(&'static str),
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: Option<MenuAction>,
    pub children: Vec<MenuItem>,
}

/// Toast text shown for every Phase-2 feature.
pub const STUB_TEXT: &str = "Phase2で実装";

/// Side keypad glyphs in order 0..=11.
pub const KEY_GLYPHS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "^"];

/// Device-blob byte for each glyph (observed from a real RSProfile1.pfd:
/// 0x1E..0x27 for 1..0, 0x2D for '-', 0x34 for '^').
pub const KEY_BLOB_CODES: [u8; 12] = [
    0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x34,
];

/// ベーシック sub-function labels for ids 16..=23 (verbatim from
/// `images/jp/strings/general.ini` `[BASIC]`; `贴り付け` keeps the original
/// spelling with 土).
pub const BASIC_LABELS: [&str; 8] = [
    "切り取り",
    "コピー",
    "贴り付け",
    "すべて選択",
    "検索",
    "NEW",
    "印刷",
    "セーブ",
];

/// アドバンスト sub-function labels for ids 24..=37 (verbatim `[ADVANCED]`;
/// `ロックPCME` keeps the original spelling).
pub const ADVANCED_LABELS: [&str; 14] = [
    "スイッチウィンドウ",
    "クローズウィンドウ",
    "オープンウィンドウ",
    "実行",
    "デスクトップを表示",
    "ロックPCME",
    "ブラウザホーム",
    "ブラウザ進む",
    "ブラウザ戻る",
    "ブラウザストップ",
    "ブラウザリフレッシュ",
    "ブラウザ検索",
    "ブラウザお気に入り",
    "メール",
];

/// メディア sub-function labels for ids 38..=46 (`[MEDIA]` has 9 entries;
/// マイクミュート comes from CONTRACT_phase15.md ordering, placed before
/// メディアプレーヤー).
pub const MEDIA_LABELS: [&str; 9] = [
    "再生/一時停止",
    "ストップ",
    "前のページ",
    "次のページ",
    "ボリュームアップ",
    "ボリュームダウン",
    "ミュート",
    "マイクミュート",
    "メディアプレーヤー",
];

/// Labels shown in the assignment rows (FRONT: ①..⑥, SIDE: ①..⑫).
///
/// `single_key_glyph` is the side-keypad glyph index (0..=11) shown in
/// parentheses for func=6 (シングルキー). For macro rows (func=100) use
/// [`func_label_ext`] to include the macro name.
pub fn func_label(func: u16, single_key_glyph: Option<usize>) -> String {
    func_label_ext(func, single_key_glyph, None)
}

/// Extended label with an optional macro name for func=100.
/// `Some(name)` with a non-empty name renders `マクロ:<name>`.
pub fn func_label_ext(
    func: u16,
    single_key_glyph: Option<usize>,
    macro_name: Option<&str>,
) -> String {
    func_label_full(func, single_key_glyph, macro_name, None)
}

/// Full label resolution including key_number context (fire target, profile +/-/cycle, rate +/-).
pub fn func_label_full(
    func: u16,
    single_key_glyph: Option<usize>,
    macro_name: Option<&str>,
    key_number: Option<u8>,
) -> String {
    if func == 100 {
        return match macro_name {
            Some(name) if !name.is_empty() => format!("マクロ:{name}"),
            _ => "マクロ".to_owned(),
        };
    }
    if func == 6 {
        return match single_key_glyph {
            Some(idx) if idx < KEY_GLYPHS.len() => {
                format!("シングルキー({})", KEY_GLYPHS[idx])
            }
            _ => "シングルキー".to_owned(),
        };
    }
    if func == 12 {
        let target = match key_number {
            Some(1) => "L",
            Some(2) => "R",
            Some(3) => "M",
            Some(u) if u > 0 => return format!("ファイアキー({})", usage_label(u)),
            _ => "",
        };
        return if target.is_empty() {
            "ファイアキー".to_owned()
        } else {
            format!("ファイアキー({target})")
        };
    }
    if func == 14 {
        return match key_number {
            Some(1) => "プロファイル +".to_owned(),
            Some(2) => "プロファイル -".to_owned(),
            _ => "プロファイルスイッチ".to_owned(),
        };
    }
    if func == 15 {
        return match key_number {
            Some(1) => "レポートレート +".to_owned(),
            Some(2) => "レポートレート -".to_owned(),
            _ => "レポートレート".to_owned(),
        };
    }
    let label: &str = match func {
        1 => "左クリック",
        2 => "右クリック",
        3 => "中央ボタン",
        4 => "進む",
        5 => "戻る",
        7 => "コンボキー",
        8 => "ベーシック",
        9 => "アドバンスト",
        10 => "メディア",
        11 => "左ダブルクリック",
        13 => "DPIスイッチ",
        47 => "DPIスイッチサイクル",
        // Working guess from an observed .pfd — NOT in FUNCTION_STRING (unverified).
        // The official profile uses 48/49 for FRONT5/FRONT6, while the
        // factory profile uses 52/53 for the dedicated DPI entries.  Live
        // direction testing proved both pairs mean DPI increase/decrease.
        48 | 52 => "DPIスイッチ+",
        49 | 53 => "DPIスイッチ-",
        54 => "LEDモードスイッチ",
        16..=23 => BASIC_LABELS[(func - 16) as usize],
        24..=37 => ADVANCED_LABELS[(func - 24) as usize],
        38..=46 => MEDIA_LABELS[(func - 38) as usize],
        _ => return format!("機能#{func}"),
    };
    label.to_owned()
}

/// Short display label for a USB HID usage id (dialogs, single-key rows).
pub fn usage_label(usage: u8) -> String {
    let label: &str = match usage {
        0x04..=0x1D => {
            const NAMES: [&str; 26] = [
                "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P",
                "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
            ];
            NAMES[(usage - 0x04) as usize]
        }
        0x1E..=0x27 => {
            const NAMES: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
            NAMES[(usage - 0x1E) as usize]
        }
        0x28 => "Enter",
        0x29 => "Esc",
        0x2A => "Backspace",
        0x2B => "Tab",
        0x2C => "Space",
        0x2D => "-",
        0x2E => "=",
        0x2F => "[",
        0x30 => "]",
        0x31 => "\\",
        0x33 => ";",
        0x34 => "^",
        0x35 => "@",
        0x36 => ",",
        0x37 => ".",
        0x38 => "/",
        0x3A..=0x45 => {
            const NAMES: [&str; 12] = [
                "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
            ];
            NAMES[(usage - 0x3A) as usize]
        }
        0x46 => "PrtScr",
        0x49 => "Insert",
        0x4A => "Home",
        0x4B => "PgUp",
        0x4C => "Delete",
        0x4D => "End",
        0x4E => "PgDn",
        0x4F => "→",
        0x50 => "←",
        0x51 => "↓",
        0x52 => "↑",
        _ => return format!("キー({usage:02X})"),
    };
    label.to_owned()
}

/// The full assignment menu tree (matches screenshot 3 / CONTRACT_funcs.md
/// as extended by CONTRACT_phase15.md).
///
/// `macros` lists known macro names for the マクロ submenu
/// (`マクロ管理` → [`MenuAction::OpenMacroManager`] plus one
/// [`MenuAction::SetMacro`] leaf per name).
pub fn build_menu(macros: &[String]) -> Vec<MenuItem> {
    let leaf = |label: &str, action: MenuAction| MenuItem {
        label: label.to_owned(),
        action: Some(action),
        children: Vec::new(),
    };
    let sub = |label: &str, children: Vec<MenuItem>| MenuItem {
        label: label.to_owned(),
        action: None,
        children,
    };

    let mut single_children: Vec<MenuItem> = (0..KEY_GLYPHS.len())
        .map(|i| leaf(KEY_GLYPHS[i], MenuAction::SetSingleKey(i)))
        .collect();
    single_children.push(leaf("ショートキー…", MenuAction::OpenSingleKeyDialog));

    let basic_children: Vec<MenuItem> = BASIC_LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| leaf(l, MenuAction::SetFunc(16 + i as u16)))
        .collect();
    let advanced_children: Vec<MenuItem> = ADVANCED_LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| leaf(l, MenuAction::SetFunc(24 + i as u16)))
        .collect();
    let media_children: Vec<MenuItem> = MEDIA_LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| leaf(l, MenuAction::SetFunc(38 + i as u16)))
        .collect();

    let mut macro_children = vec![leaf("マクロ管理", MenuAction::OpenMacroManager)];
    macro_children.extend(
        macros
            .iter()
            .map(|name| leaf(name, MenuAction::SetMacro(name.clone()))),
    );

    vec![
        leaf("左クリック", MenuAction::SetFunc(1)),
        leaf("右クリック", MenuAction::SetFunc(2)),
        leaf("中央ボタン", MenuAction::SetFunc(3)),
        leaf("進む", MenuAction::SetFunc(4)),
        leaf("戻る", MenuAction::SetFunc(5)),
        sub("シングルキー", single_children),
        sub(
            "コンボキー",
            vec![leaf("コンボキーを割り当て…", MenuAction::OpenComboDialog)],
        ),
        sub("ベーシック", basic_children),
        sub("アドバンスト", advanced_children),
        sub("メディア", media_children),
        sub("マクロ", macro_children),
        leaf("ファイアキー...", MenuAction::OpenFireDialog),
        sub(
            "DPIスイッチ",
            vec![
                leaf("DPIスイッチサイクル", MenuAction::SetFunc(13)),
                leaf("DPI +", MenuAction::SetFunc(52)),
                leaf("DPI -", MenuAction::SetFunc(53)),
            ],
        ),
        sub(
            "プロファイルスイッチ",
            vec![
                leaf("プロファイルスイッチ", MenuAction::SetFuncWithKey(14, 0)),
                leaf("プロファイル +", MenuAction::SetFuncWithKey(14, 1)),
                leaf("プロファイル -", MenuAction::SetFuncWithKey(14, 2)),
            ],
        ),
        sub(
            "レポートレート",
            vec![
                leaf("レポートレート +", MenuAction::SetFuncWithKey(15, 1)),
                leaf("レポートレート -", MenuAction::SetFuncWithKey(15, 2)),
            ],
        ),
        leaf("LEDモードスイッチ", MenuAction::SetFunc(54)),
        // NOTE: id-16 collision, see module docs. Kept so disabling stays
        // reachable; renders with the basic label once assigned.
        leaf("無効", MenuAction::SetFunc(16)),
    ]
}

/// Map a blob byte back to glyph index (None if unknown).
pub fn blob_code_to_glyph(b: u8) -> Option<usize> {
    KEY_BLOB_CODES.iter().position(|&code| code == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_labels() {
        assert_eq!(func_label(1, None), "左クリック");
        assert_eq!(func_label(2, None), "右クリック");
        assert_eq!(func_label(3, None), "中央ボタン");
        assert_eq!(func_label(4, None), "進む");
        assert_eq!(func_label(5, None), "戻る");
        assert_eq!(func_label(11, None), "左ダブルクリック");
        assert_eq!(func_label(12, None), "ファイアキー");
        assert_eq!(func_label(13, None), "DPIスイッチ");
        assert_eq!(func_label(14, None), "プロファイルスイッチ");
        assert_eq!(func_label(15, None), "レポートレート");
        assert_eq!(func_label(47, None), "DPIスイッチサイクル");
        assert_eq!(func_label(48, None), "DPIスイッチ+");
        assert_eq!(func_label(49, None), "DPIスイッチ-");
        assert_eq!(func_label(54, None), "LEDモードスイッチ");
        assert_eq!(func_label(100, None), "マクロ");
        assert_eq!(func_label(52, None), "DPIスイッチ+");
        assert_eq!(func_label(53, None), "DPIスイッチ-");
        // Phase 1.5 contract: 16..23 are basic sub-functions (see module NOTE).
        assert_eq!(func_label(16, None), "切り取り");
        assert_eq!(func_label(23, None), "セーブ");
        assert_eq!(func_label(24, None), "スイッチウィンドウ");
        assert_eq!(func_label(37, None), "メール");
        assert_eq!(func_label(38, None), "再生/一時停止");
        assert_eq!(func_label(45, None), "マイクミュート");
        assert_eq!(func_label(46, None), "メディアプレーヤー");
    }

    #[test]
    fn single_key_label_uses_glyph() {
        assert_eq!(func_label(6, Some(0)), "シングルキー(1)");
        assert_eq!(func_label(6, Some(9)), "シングルキー(0)");
        assert_eq!(func_label(6, Some(10)), "シングルキー(-)");
        assert_eq!(func_label(6, Some(11)), "シングルキー(^)");
        assert_eq!(func_label(6, None), "シングルキー");
        assert_eq!(func_label(6, Some(99)), "シングルキー"); // out of range → plain label
    }

    #[test]
    fn macro_label_ext() {
        assert_eq!(func_label_ext(100, None, None), "マクロ");
        assert_eq!(func_label_ext(100, None, Some("")), "マクロ");
        assert_eq!(func_label_ext(100, None, Some("777")), "マクロ:777");
        // Non-macro ids ignore the name.
        assert_eq!(func_label_ext(1, None, Some("777")), "左クリック");
    }

    #[test]
    fn usage_labels() {
        assert_eq!(usage_label(0x04), "A");
        assert_eq!(usage_label(0x1D), "Z");
        assert_eq!(usage_label(0x1E), "1");
        assert_eq!(usage_label(0x27), "0");
        assert_eq!(usage_label(0x28), "Enter");
        assert_eq!(usage_label(0x2C), "Space");
        assert_eq!(usage_label(0x3A), "F1");
        assert_eq!(usage_label(0x46), "PrtScr");
        assert_eq!(usage_label(0xFF), "キー(FF)");
    }

    #[test]
    fn unknown_ids_render_as_function_number() {
        assert_eq!(func_label(0, None), "機能#0");
        assert_eq!(func_label(255, None), "機能#255");
    }

    #[test]
    fn blob_codes_roundtrip() {
        for (i, &code) in KEY_BLOB_CODES.iter().enumerate() {
            assert_eq!(blob_code_to_glyph(code), Some(i));
        }
        assert_eq!(blob_code_to_glyph(0x00), None);
        assert_eq!(blob_code_to_glyph(0xFF), None);
    }

    #[test]
    fn menu_tree_shape() {
        let menu = build_menu(&[]);
        let labels: Vec<&str> = menu.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "左クリック",
                "右クリック",
                "中央ボタン",
                "進む",
                "戻る",
                "シングルキー",
                "コンボキー",
                "ベーシック",
                "アドバンスト",
                "メディア",
                "マクロ",
                "ファイアキー...",
                "DPIスイッチ",
                "プロファイルスイッチ",
                "レポートレート",
                "LEDモードスイッチ",
                "無効",
            ]
        );

        // Parents carry no action of their own.
        assert!(menu
            .iter()
            .filter(|i| !i.children.is_empty())
            .all(|i| i.action.is_none()));

        let single = &menu[5];
        assert_eq!(single.children.len(), 13);
        assert_eq!(single.children[0].label, "1");
        assert_eq!(single.children[0].action, Some(MenuAction::SetSingleKey(0)));
        assert_eq!(
            single.children[11].action,
            Some(MenuAction::SetSingleKey(11))
        );
        assert_eq!(
            single.children[12].action,
            Some(MenuAction::OpenSingleKeyDialog)
        );

        let combo = &menu[6];
        assert_eq!(combo.children[0].action, Some(MenuAction::OpenComboDialog));

        let basic = &menu[7];
        assert_eq!(basic.children.len(), 8);
        assert_eq!(basic.children[0].action, Some(MenuAction::SetFunc(16)));
        assert_eq!(basic.children[7].action, Some(MenuAction::SetFunc(23)));

        let adv = &menu[8];
        assert_eq!(adv.children.len(), 14);
        assert_eq!(adv.children[0].action, Some(MenuAction::SetFunc(24)));
        assert_eq!(adv.children[13].action, Some(MenuAction::SetFunc(37)));

        let media = &menu[9];
        assert_eq!(media.children.len(), 9);
        assert_eq!(media.children[0].action, Some(MenuAction::SetFunc(38)));
        assert_eq!(media.children[8].action, Some(MenuAction::SetFunc(46)));

        // Empty macro list → only the manager entry.
        let macros = &menu[10];
        assert_eq!(macros.children.len(), 1);
        assert_eq!(
            macros.children[0].action,
            Some(MenuAction::OpenMacroManager)
        );

        assert_eq!(menu[11].action, Some(MenuAction::OpenFireDialog));

        let dpi = &menu[12];
        assert_eq!(dpi.children[0].action, Some(MenuAction::SetFunc(13)));
        assert_eq!(dpi.children[1].action, Some(MenuAction::SetFunc(52)));
        assert_eq!(dpi.children[2].action, Some(MenuAction::SetFunc(53)));

        let profile = &menu[13];
        assert_eq!(profile.children.len(), 3);
        assert_eq!(profile.children[0].label, "プロファイルスイッチ");
        assert_eq!(
            profile.children[0].action,
            Some(MenuAction::SetFuncWithKey(14, 0))
        );
        assert_eq!(profile.children[1].label, "プロファイル +");
        assert_eq!(
            profile.children[1].action,
            Some(MenuAction::SetFuncWithKey(14, 1))
        );
        assert_eq!(profile.children[2].label, "プロファイル -");
        assert_eq!(
            profile.children[2].action,
            Some(MenuAction::SetFuncWithKey(14, 2))
        );

        let rate = &menu[14];
        assert_eq!(rate.children.len(), 2);
        assert_eq!(rate.children[0].label, "レポートレート +");
        assert_eq!(
            rate.children[0].action,
            Some(MenuAction::SetFuncWithKey(15, 1))
        );
        assert_eq!(rate.children[1].label, "レポートレート -");
        assert_eq!(
            rate.children[1].action,
            Some(MenuAction::SetFuncWithKey(15, 2))
        );

        assert_eq!(menu[15].action, Some(MenuAction::SetFunc(54)));
        assert_eq!(menu[16].action, Some(MenuAction::SetFunc(16)));
    }

    #[test]
    fn menu_macro_children() {
        let names = vec!["777".to_owned(), "boss".to_owned()];
        let menu = build_menu(&names);
        let macros = &menu[10];
        assert_eq!(macros.children.len(), 3);
        assert_eq!(
            macros.children[1].action,
            Some(MenuAction::SetMacro("777".to_owned()))
        );
        assert_eq!(
            macros.children[2].action,
            Some(MenuAction::SetMacro("boss".to_owned()))
        );
    }
}
