# Assets & shared widgets — CONTRACT

`src/assets.rs` embeds only the project mouse-layout images listed below.
All controls, cards, tabs, sliders, menus, and buttons are painted by the Rust
UI; the removed upstream skin files are not required at runtime.

## assets.rs

Embeds the three project mouse-layout PNGs via `include_bytes!` and decodes
them into `egui::TextureHandle`s (use the `image` crate).

```rust
pub type Tex = egui::TextureHandle;

pub struct Assets {
    pub mouse_front: Tex,    // background/mouse-layout-front.png
    pub mouse_side: Tex,     // background/mouse-layout-side.png
    pub mouse_color: Tex,    // background/mouse-layout-Color.png
}

impl Assets {
    pub fn load(ctx: &egui::Context) -> Self;
}
```text

## ui_common.rs

Pure widget helpers. NO dependency on `crate::app` (Worker 4 wires them).

```rust
/// Shared geometry constants (800x638 window), measured from the reference screenshots.
pub mod geo {
    pub const WINDOW: [f32; 2] = [800.0, 638.0];
    pub const TAB_Y: f32 = 112.0;
    pub const TAB_X0: f32 = 75.0;
    pub const TAB_PITCH: f32 = 102.0;
    pub const TAB_SIZE: [f32; 2] = [85.0, 21.0];
    pub const CARD_Y: f32 = 145.0;
    pub const CARD_H: f32 = 313.0;
    pub const MIN_BTN: egui::Rect;   // titlebar minimize (743,30 22x18)
    pub const CLOSE_BTN: egui::Rect; // titlebar close    (766,30 22x18)
    pub const PROFILE_Y: f32 = 478.0;
    pub const PROFILE_X0: f32 = 105.0;
    pub const PROFILE_PITCH: f32 = 123.0;
    pub const PROFILE_SIZE: [f32; 2] = [109.0, 20.0];
    pub const BOTTOM_Y: f32 = 580.0;
    pub const BOTTOM_H: f32 = 27.0;
    pub const BOTTOM_GAP: f32 = 10.0;
    // x/width pairs for the 7 bottom buttons: 保存/ロードファイル/既定/すべてリセット/OK/キャンセル/適用
    pub const BOTTOM_BUTTONS: [(&'static str, f32, f32); 7];
    pub const MOUSE_FRONT_RECT: egui::Rect;   // (100,155) 200x245
    pub const MOUSE_SIDE_RECT: egui::Rect;    // (100,155) 200x245
    pub const ROWS_X: f32 = 330.0;            // assignment rows left
    pub const ROW_W: f32 = 173.0;
    pub const ROW_H: f32 = 23.0;
    pub const ROW_PITCH_FRONT: f32 = 34.0;    // 6 rows
    pub const ROW_PITCH_SIDE: f32 = 24.0;     // 12 rows
    pub const SLIDER_X: f32 = 520.0;
    pub const SLIDER_W: f32 = 210.0;
    pub const SLIDER_H: f32 = 19.0;
    pub const DPI_COL_X: [f32; 5];            // column centers 172,305,437,570,702
    pub const DPI_TRACK_Y: f32 = 255.0;
    pub const DPI_TRACK_SIZE: [f32; 2] = [22.0, 182.0];
}

pub fn paint_tex(ui: &egui::Ui, tex: &Tex, rect: egui::Rect, tint: egui::Color32);
pub fn fit_rect(rect: egui::Rect, size: [f32; 2]) -> egui::Rect; // contain-fit, centered

/// Textured button with centered label. `selected` adds a red border.
pub fn image_button(ui: &egui::Ui, id: &str, rect: egui::Rect, tex: &Tex,
                    label: &str, selected: bool, text_color: egui::Color32) -> egui::Response;

/// 13x13 checkbox + label. Square border, red "V" when checked.
/// Returns (response, new_value). `checked` is `&mut bool`.
pub fn checkbox(ui: &egui::Ui, id: &str, pos: egui::Pos2, label: &str, checked: &mut bool) -> egui::Response;

/// Horizontal slider (track texture + thumb texture). Returns Some(new_value) when changed.
pub fn hslider_int(ui: &egui::Ui, id: &str, rect: egui::Rect, track: &Tex, thumb: &Tex,
                   value: i32, min: i32, max: i32) -> Option<i32>;

/// Vertical slider inside `rect` (DPI track). Returns Some(new_code) when changed.
pub fn vslider_int(ui: &egui::Ui, id: &str, rect: egui::Rect, track: &Tex,
                   value: i32, min: i32, max: i32) -> Option<i32>;

/// White 12px label helper used everywhere (text drawn at pos, top-left).
pub fn label(ui: &egui::Ui, pos: egui::Pos2, text: &str, color: egui::Color32, size: f32);
```text


## Accessibility contract

Every custom interaction must call one of the `ui_common::a11y` registration helpers after its
`egui::Response` is created. IDs are normalized under the `red-samurai.` namespace and should be
stable across frames. The helper emits the correct AccessKit role/state, adds the ID to the accessible
name as a fallback, and sets AccessKit `AuthorId` when the default `accesskit` feature is enabled.
On Windows that value is surfaced as UI Automation `AutomationId`, which gives automation a stable
selector independent of window coordinates and translated labels. New dialogs should use
`linear_ui_button_with_id` for explicit button IDs rather than relying on its generated fallback.
