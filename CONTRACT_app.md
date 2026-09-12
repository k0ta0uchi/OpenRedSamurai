# App shell & tabs — CONTRACT (implemented by Worker 4)

Files owned by Worker 4: `src/app.rs`, `src/ui_frame.rs`, `src/ui_general.rs`,
`src/ui_dpi.rs`, `src/ui_light.rs`, `src/ui_info.rs`. Also `src/main.rs` glue if needed.
Everything else is read-only.

## app.rs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab { General, Dpi, Light, Info }

/// Overlay state for the assignment context menu (opened by clicking a row).
pub struct MenuState {
    pub open: bool,
    pub pos: egui::Pos2,
    pub items: Vec<crate::funcs::MenuItem>,
}

/// Double-click test state for the spiral area in the General tab.
pub struct DblClickTest { pub last_click: Option<f64>, pub result: Option<&'static str> }

pub struct App {
    pub assets: crate::assets::Assets,
    pub profiles: Vec<crate::profile::Profile>, // 5 entries, slots 1..=5
    pub current: usize,                          // 0..=4
    pub tab: Tab,
    pub side: bool,                              // General tab FRONT(false)/SIDE(true)
    pub status: Option<(String, f64)>,           // toast text + creation time (ctx.input time)
    pub menu: Option<MenuState>,
    pub dblclick: DblClickTest,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self; // loads fonts + assets + profiles
    pub fn current_profile(&self) -> &crate::profile::Profile;
    pub fn current_profile_mut(&mut self) -> &mut crate::profile::Profile;
    pub fn toast(&mut self, ctx: &egui::Context, msg: impl Into<String>);
}

impl eframe::App for App { fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame); }
pub fn run() -> eframe::Result<()>; // NativeOptions: 800x638, decorations(false), resizable(false)
```text

`update()` order: draw background frame → titlebar → tabs → tab content → profile row →
bottom row → menu overlay → toast. Fonts: load Meiryo/Yu Gothic/MS Gothic from
`C:\Windows\Fonts` (`FontData::from_owned`, insert at index 0 of Proportional; keep defaults).

## ui_frame.rs

```rust
pub fn draw_background(app: &mut App, ctx: &egui::Context); // bg_main texture at (0,0)
pub fn draw_titlebar(app: &mut App, ctx: &egui::Context);   // invisible min/close over the
    // baked-in art; drag on header (y 0..60 full width) starts window drag (ViewportCommand::StartDrag)
pub fn draw_tabs(app: &mut App, ctx: &egui::Context);       // 一般/DPI/ライト/情報, active tab
pub fn draw_profile_row(app: &mut App, ctx: &egui::Context); // プロファイル#1..#5 switches slot
pub fn draw_bottom_row(app: &mut App, ctx: &egui::Context);
// 保存=save-as dialog(rfd) → writes .pfd; ロードファイル=open dialog → import into current slot
// (rename GROUP* sections to current slot naming); 既定=reset current to defaults;
// すべてリセット=reset all 5; 適用=save current slot file and send only the
// evidence-gated complete Apply sequence (including verified rainbow mode);
// OK=適用+Close; キャンセル=reload all from disk+Close. Disabled look for nothing; all active.
pub fn draw_toast(app: &mut App, ctx: &egui::Context);      // status toast, hide after ~2.5s
```text

## Tab modules (each: `pub fn show(app: &mut App, ctx: &egui::Context)`)

- `ui_general.rs`: mouse image FRONT/SIDE (geo.MOUSE_*_RECT), FRONT/SIDE switch buttons at
  (95,455 78x23)/(185,455 78x23), assignment rows (6 front rows pitch 34 / 12 side rows pitch 24,
  label = `funcs::func_label(func, glyph)`, row click opens `app.menu` at pointer,
  `MenuAction` applied to `app.current_profile_mut()`), right column: 加速度 slider (0..=2),
  POINTER SPEED (1..=20, MouseSensitivity), スクロールスピード (1..=10, WheelScrollLines),
  ダブルクリックスピード (1..=10, DoubleClickSpeed) with small track, "+" glyph after each track,
  polling radio boxes 125/250/500/1000HZ → PollingRate {1,2,4,8}, double-click test spiral
  (clicks within 500ms set result "OK" else "速すぎ/遅い" text under the spiral).
- `ui_dpi.rs`: 5 columns (geo.DPI_COL_X): dpi label texture at y200 (checked=enabled from
  `dpi_stages()`), "+" above track, vslider (code 0..=163) with thumb, wheel over a slider
  changes one code (100 DPI), and the value below the track is a click-to-edit numeric field
  rounded to the nearest 100 DPI; left LED column at x105: 5 radio textures, radio_on marks
  `current_dpi_stage()`.
- `ui_light.rs`: mouse_color image, ライトカラー palette 8x5 cells 19x19 gap4 at (330,228):
  HSV rows [reds..yellows, greens, cyans, blues, magentas] + last cell rainbow (sets
  `LedMode1=2`, backed by the official complete Apply capture), click sets LedColor1 (COLORREF);
  カスタムライトカラー combo at (330,395)
  170x20 shows current color bar (egui color picker button is fine); 明度レベル radios
  オフ/低/中/高 → LedState1 0..3; 呼吸スピード 2-col radios 低/中/高/フルライト → BreathState1
  {2,5,7,0}; LEDモードスイッチ rows [フラッシュ,オフ],[虹,フルライト],[呼吸,波] → LedMode1 0..5.
- `ui_info.rs`: centered white lines from skins.ini 37..41,91,92 at y=230 step 37.
