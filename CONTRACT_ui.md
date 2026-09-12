# Assets & shared widgets — CONTRACT (implemented by Worker 3)

Files owned by Worker 3: `src/assets.rs`, `src/ui_common.rs`.
Everything else is read-only for Worker 3.

## assets.rs

Embeds the original skin PNGs from `assets/images/0409/...` via `include_bytes!`
and decodes them into `egui::TextureHandle`s (use the `image` crate).

```rust
pub type Tex = egui::TextureHandle;

pub struct Assets {
    pub bg_main: Tex,        // background/cfgMainback.png       (801x638 window frame incl. header art)
    pub mouse_front: Tex,    // background/mouse-layout-front.png (622x883, has ①..⑥ circles)
    pub mouse_side: Tex,     // background/mouse-layout-side.png  (547x1365, plain keypad)
    pub mouse_color: Tex,    // background/mouse-layout-Color.png (622x883, no circles)
    pub row_n: Tex,          // buttons/ButtonAssign-1.png        (173x23 assignment row normal)
    pub row_sel: Tex,        // buttons/ButtonAssign-2.png        (173x23 assignment row selected)
    pub profile_n: Tex,      // buttons/ProfileButton-1.png       (109x20 profile button normal)
    pub profile_sel: Tex,    // buttons/ProfileButton-3.png       (109x21 profile button selected)
    pub btn_n: Tex,          // background/main-ok-btn-n.png      (94x27 bottom button normal)
    pub btn_d: Tex,          // background/main-ok-btn-d.png      (94x27 bottom button pressed)
    pub tab_a: Tex,          // background/tab-a.png              (85x21 active tab)
    pub tab_n: Tex,          // background/tab-n.png              (84x21 inactive tab)
    pub track_h: Tex,        // background/trackbar-back-speed.png   (210x19 horizontal slider)
    pub track_h_small: Tex,  // background/trackbar-back-speed-dbclick.png (59x19; stretch to 122 wide)
    pub thumb: Tex,          // background/bar1.png               (15x15 red LED thumb)
    pub track_v: Tex,        // background/DPI-TRACK-R.png        (22x182 vertical DPI track)
    pub radio_off: Tex,      // background/radio1.png             (13x13)
    pub radio_on: Tex,       // background/radio2.png             (13x13)
    pub dpi_label_off: Tex,  // background/DPI-1.png              (43x17)
    pub dpi_label_on: Tex,   // background/DPI-2.png              (43x17, with white check)
    pub test_area: Tex,      // background/DB-CLICK-TEST-AREA.png (109x150 red spiral)
    pub menu_item_n: Tex,    // background/funcmenu_1.png         (212x27 menu row)
    pub menu_item_h: Tex,    // background/funcmenu_2.png         (212x27 menu row hover)
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
    pub const TAB_Y: f32 = 162.0;
    pub const TAB_X0: f32 = 75.0;
    pub const TAB_PITCH: f32 = 102.0;
    pub const TAB_SIZE: [f32; 2] = [85.0, 21.0];
    pub const MIN_BTN: egui::Rect;   // titlebar minimize (743,30 22x18)
    pub const CLOSE_BTN: egui::Rect; // titlebar close    (766,30 22x18)
    pub const PROFILE_Y: f32 = 513.0;
    pub const PROFILE_X0: f32 = 105.0;
    pub const PROFILE_PITCH: f32 = 123.0;
    pub const PROFILE_SIZE: [f32; 2] = [109.0, 20.0];
    pub const BOTTOM_Y: f32 = 560.0;
    pub const BOTTOM_H: f32 = 27.0;
    // x/width pairs for the 7 bottom buttons: 保存/ロードファイル/既定/すべてリセット/OK/キャンセル/適用
    pub const BOTTOM_BUTTONS: [(&'static str, f32, f32); 7];
    pub const MOUSE_FRONT_RECT: egui::Rect;   // (100,205) 200x245
    pub const MOUSE_SIDE_RECT: egui::Rect;    // (100,205) 200x245
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
