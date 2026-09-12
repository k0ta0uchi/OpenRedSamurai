//! Global keyboard capture for assignment dialogs — Worker 1.
//!
//! Uses `GetAsyncKeyState` (windows crate) so every physical key including
//! PrtScr is visible while a dialog is open. Maps virtual-key codes to
//! USB HID usage ids.
//!
//! Modifier mask: bit0=CTRL bit1=ALT bit2=SHIFT bit3=WIN
//! (matches the combo-key `LoopNumber` encoding in CONTRACT_phase15.md).

/// Current held modifier mask: bit0=CTRL bit1=ALT bit2=SHIFT bit3=WIN.
pub fn modifier_mask() -> u8 {
    let mut mask = 0u8;
    if key_down(0x11) {
        mask |= 1 << 0;
    } // VK_CONTROL
    if key_down(0x12) {
        mask |= 1 << 1;
    } // VK_MENU (Alt)
    if key_down(0x10) {
        mask |= 1 << 2;
    } // VK_SHIFT
    if key_down(0x5B) || key_down(0x5C) {
        mask |= 1 << 3;
    } // VK_LWIN / VK_RWIN
    mask
}

/// 一覧表示用の修飾子プレフィックス ("Ctrl + " 等)。
pub fn modifier_prefix(mask: u8) -> String {
    let mut out = String::new();
    if mask & (1 << 0) != 0 {
        out.push_str("Ctrl + ");
    }
    if mask & (1 << 1) != 0 {
        out.push_str("Alt + ");
    }
    if mask & (1 << 2) != 0 {
        out.push_str("Shift + ");
    }
    if mask & (1 << 3) != 0 {
        out.push_str("Win + ");
    }
    out
}

/// 捕まえたキーを USB HID usage へ。対象が無ければ None。
/// ポーリング (例: 30ms) で呼び、押された瞬間の VK を渡す。
pub fn vk_to_usage(vk: i32) -> Option<u8> {
    let usage = match vk as u32 {
        // A..=Z → 0x04..=0x1D
        0x41..=0x5A => (vk as u8 - 0x41) + 0x04,
        // 1..=9 → 0x1E..=0x26, 0 → 0x27
        0x31..=0x39 => (vk as u8 - 0x31) + 0x1E,
        0x30 => 0x27,
        // F1..=F24 → 0x3A..=0x51
        0x70..=0x87 => (vk as u8 - 0x70) + 0x3A,
        0x0D => 0x28, // VK_RETURN → Enter
        0x1B => 0x29, // VK_ESCAPE
        0x08 => 0x2A, // VK_BACK
        0x09 => 0x2B, // VK_TAB
        0x20 => 0x2C, // VK_SPACE
        // JIS OEM keys (approximations; Phase2 device capture will verify).
        0xBD => 0x2D,        // VK_OEM_MINUS '-'/'='
        0xBB => 0x2E,        // VK_OEM_PLUS ';'/'+'
        0xDB => 0x2F,        // VK_OEM_4 '['/'{'
        0xDD => 0x30,        // VK_OEM_6 ']'/'}'
        0xDC => 0x31,        // VK_OEM_5 '\\'/'|'
        0xBA => 0x33,        // VK_OEM_1 ';'/':' (JIS ':' )
        0xDE => 0x34,        // VK_OEM_7 '^'/'~' (JIS '^' → contract 0x34)
        0xC0 => 0x35,        // VK_OEM_3 '@'/'`' (JIS '@')
        0xBC => 0x36,        // VK_OEM_COMMA ','/'<'
        0xBE => 0x37,        // VK_OEM_PERIOD '.'/'>'
        0xBF => 0x38,        // VK_OEM_2 '/'/'?' (JIS '/')
        0xE2 => 0x89,        // VK_OEM_102 '\\'/'_' (JIS '_', HID Intl1)
        0x2C => 0x46,        // VK_SNAPSHOT → PrtScr
        0x91 => 0x47,        // VK_SCROLL → Scroll Lock
        0x13 => 0x48,        // VK_PAUSE
        0x2D => 0x49,        // VK_INSERT
        0x24 => 0x4A,        // VK_HOME
        0x21 => 0x4B,        // VK_PRIOR (PgUp)
        0x2E => 0x4C,        // VK_DELETE
        0x23 => 0x4D,        // VK_END
        0x22 => 0x4E,        // VK_NEXT (PgDn)
        0x27 => 0x4F,        // VK_RIGHT
        0x25 => 0x50,        // VK_LEFT
        0x28 => 0x51,        // VK_DOWN
        0x26 => 0x52,        // VK_UP
        0x90 => 0x53,        // VK_NUMLOCK
        0x6F => 0x54,        // VK_DIVIDE
        0x6A => 0x55,        // VK_MULTIPLY
        0x6D => 0x56,        // VK_SUBTRACT
        0x6B => 0x57,        // VK_ADD
        0x6C => 0x58,        // VK_SEPARATOR (numpad Enter)
        0x60..=0x69 => 0x00, // placeholder: numpad fixed up below
        0x6E => 0x63,        // VK_DECIMAL
        _ => return None,
    };
    // Numpad 1..=9 → 0x59..=0x61, numpad 0 → 0x62 (sequential fixup).
    let usage = if (0x60..=0x69).contains(&(vk as u32)) {
        if vk == 0x60 {
            0x62
        } else {
            (vk as u8 - 0x61) + 0x59
        }
    } else {
        usage
    };
    Some(usage)
}

/// 直近に押された非修飾子キーの VK コード (Poll を1回)。
///
/// Returns the first currently-down VK from a curated scan list
/// (modifiers excluded — read those via [`modifier_mask`]).
pub fn poll_pressed_key() -> Option<i32> {
    // Letters, digits, function keys, navigation/edit keys, numpad, OEM.
    const SCAN: &[i32] = &[
        0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
        0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x30, 0x31, 0x32, 0x33,
        0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
        0x79, 0x7A, 0x7B, 0x0D, 0x1B, 0x08, 0x09, 0x20, 0x2C, 0x2D, 0x24, 0x21, 0x2E, 0x23, 0x22,
        0x27, 0x25, 0x28, 0x26, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A,
        0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0xBD, 0xBB, 0xDB, 0xDD, 0xDC, 0xBA, 0xDE, 0xC0, 0xBC, 0xBE,
        0xBF, 0xE2, 0x91, 0x13, 0x90,
    ];
    SCAN.iter().copied().find(|&vk| key_down(vk))
}

#[cfg(windows)]
fn key_down(vk: i32) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    // High bit set = key currently down.
    unsafe { GetAsyncKeyState(vk) < 0 }
}

#[cfg(not(windows))]
fn key_down(_vk: i32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_prefix_order() {
        assert_eq!(modifier_prefix(0), "");
        assert_eq!(modifier_prefix(0b0001), "Ctrl + ");
        assert_eq!(modifier_prefix(0b0111), "Ctrl + Alt + Shift + ");
        assert_eq!(modifier_prefix(0b1111), "Ctrl + Alt + Shift + Win + ");
    }

    #[test]
    fn vk_letters_digits_function() {
        assert_eq!(vk_to_usage(0x41), Some(0x04)); // A
        assert_eq!(vk_to_usage(0x5A), Some(0x1D)); // Z
        assert_eq!(vk_to_usage(0x31), Some(0x1E)); // 1
        assert_eq!(vk_to_usage(0x30), Some(0x27)); // 0
        assert_eq!(vk_to_usage(0x70), Some(0x3A)); // F1
        assert_eq!(vk_to_usage(0x7B), Some(0x45)); // F12
        assert_eq!(vk_to_usage(0x0D), Some(0x28)); // Enter
        assert_eq!(vk_to_usage(0x20), Some(0x2C)); // Space
        assert_eq!(vk_to_usage(0x2C), Some(0x46)); // PrtScr
    }

    #[test]
    fn vk_modifiers_unmapped() {
        assert_eq!(vk_to_usage(0x10), None);
        assert_eq!(vk_to_usage(0x11), None);
        assert_eq!(vk_to_usage(0x12), None);
    }
}
