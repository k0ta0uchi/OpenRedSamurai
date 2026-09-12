//! App shell — CONTRACT in CONTRACT_app.md (implemented by Worker 4).
//! Phase 2 device/apply wiring remains explicit and fail-closed; editing and
//! profile persistence continue to work without a connected mouse.

use crate::assets::Assets;
use crate::device::{self, DeviceError};
use crate::device_apply::{ApplyPlan, ApplyWarning};
use crate::funcs::MenuAction;
use crate::menu::{self, MenuState};
use crate::profile::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    General,
    Dpi,
    Light,
    Info,
}

/// Double-click test state for the spiral area in the General tab.
#[derive(Debug, Clone, Default)]
pub struct DblClickTest {
    pub last_click: Option<f64>,
    pub result: Option<&'static str>,
}

/// Connection state shown by the frame-level device indicator.
///
/// `NotChecked` is the startup state on purpose: constructing the app never
/// enumerates HID devices or opens a device.  The status can be refreshed by
/// the explicit indicator action, while the Apply action performs the only
/// path that may open a transport and send a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    NotChecked,
    Connected,
    Unavailable,
    Error(String),
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self::NotChecked
    }
}

impl DeviceStatus {
    /// Short Japanese label suitable for the compact title-bar indicator.
    pub fn label(&self) -> String {
        match self {
            Self::NotChecked => "デバイス: 未確認".to_string(),
            Self::Connected => "デバイス: 接続済み".to_string(),
            Self::Unavailable => "デバイス: 未接続 · ドライラン".to_string(),
            Self::Error(message) => format!("デバイス: エラー ({message})"),
        }
    }

    pub const fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }
}

/// Phase 2 state that must survive frames without expanding [`App`]'s public
/// struct-literal surface.  Phase 1.5's UI smoke test (and downstream users)
/// construct `App` directly, so this state lives in egui's per-context
/// temporary store instead of adding mandatory fields to `App`.
#[derive(Debug, Clone, Default)]
pub struct Phase2UiState {
    pub device_status: DeviceStatus,
    pub last_apply_plan: Option<ApplyPlan>,
    pub last_apply_warning_count: usize,
    pub device_status_checked_at: Option<f64>,
}

fn phase2_state_id() -> egui::Id {
    egui::Id::new("phase2_ui_state")
}

/// Read the persistent-in-context Phase 2 UI state, defaulting to an
/// untouched startup state.  Reading this value is local memory access only.
pub fn phase2_ui_state(ctx: &egui::Context) -> Phase2UiState {
    ctx.data(|data| data.get_temp(phase2_state_id()))
        .unwrap_or_default()
}

fn store_phase2_ui_state(ctx: &egui::Context, state: Phase2UiState) {
    ctx.data_mut(|data| data.insert_temp(phase2_state_id(), state));
}

pub struct App {
    pub assets: Assets,
    pub profiles: Vec<Profile>, // 5 entries, slots 1..=5
    pub current: usize,         // 0..=4
    pub tab: Tab,
    pub side: bool,
    pub status: Option<(String, f64)>,
    pub menu: MenuState,
    /// Button (1..=20) the open menu belongs to.
    pub menu_button: usize,
    pub dblclick: DblClickTest,
    /// Light tab: custom LED color picker popup is open.
    pub custom_color_open: bool,
    /// Light tab: in-progress custom color while the picker popup is open.
    pub temp_rgb: [u8; 3],
    /// 割付サブダイアログ (シングルキー/コンボキー) の種類。
    pub dialog: crate::ui_dialog::DialogKind,
    /// マクロマネージャーを開いているか。
    pub macro_open: bool,
    /// マクロDB (MSDB + MSMACRO)。
    pub macros: crate::macro_db::MacroDb,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        load_japanese_font(&cc.egui_ctx);
        let assets = Assets::load(&cc.egui_ctx);
        let profiles = (1..=5)
            .map(|slot| match Profile::profile_path(slot) {
                Some(path) => Profile::load(&path, slot),
                None => Profile::default_profile(slot),
            })
            .collect();
        App {
            assets,
            profiles,
            current: 0,
            tab: Tab::General,
            side: false,
            status: None,
            menu: MenuState::default(),
            menu_button: 1,
            dblclick: DblClickTest::default(),
            custom_color_open: false,
            temp_rgb: [255, 0, 0],
            dialog: crate::ui_dialog::DialogKind::default(),
            macro_open: false,
            macros: crate::macro_db::MacroDb::load_default(),
        }
    }

    pub fn current_profile(&self) -> &Profile {
        &self.profiles[self.current]
    }

    pub fn current_profile_mut(&mut self) -> &mut Profile {
        &mut self.profiles[self.current]
    }

    pub fn toast(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.status = Some((msg.into(), now));
    }

    /// Perform the opt-in, read-only device discovery used by the title-bar
    /// indicator.  This is never called from [`Self::new`] or from every
    /// frame, so startup and ordinary editing stay independent of HID state.
    pub fn probe_device(&mut self, ctx: &egui::Context) {
        let mut state = phase2_ui_state(ctx);
        state.device_status_checked_at = Some(ctx.input(|input| input.time));
        match device::is_available() {
            Ok(true) => {
                state.device_status = DeviceStatus::Connected;
                self.toast(ctx, "デバイスを確認しました");
            }
            Ok(false) => {
                state.device_status = DeviceStatus::Unavailable;
                self.toast(ctx, "デバイス未接続（ドライランを利用できます）");
            }
            Err(error) => {
                state.device_status = status_for_device_error(&error);
                self.toast(ctx, format!("デバイス確認エラー: {error}"));
            }
        }
        store_phase2_ui_state(ctx, state);
    }

    /// Build and retain the current profile's fail-closed plan without
    /// opening a device or sending any report.  This is the explicit dry-run
    /// path used when no verified device is connected.
    pub fn dry_run_current_profile(&mut self, ctx: &egui::Context) {
        let plan = ApplyPlan::dry_run(self.current_profile());
        retain_apply_plan(ctx, &plan);
        self.toast(ctx, dry_run_message(&plan));
    }

    /// Apply the current profile through the verified device boundary.
    ///
    /// The plan is always prepared first.  Unsupported fields remain visible
    /// as warnings and are excluded from I/O; a complete reviewed sequence
    /// may still be applied for the known PollingRate/DPI subset.  An
    /// unavailable device retains the same plan as a dry-run result.
    /// Consequently this method is the sole app-level path that can perform
    /// device I/O, and it is only called by the explicit Apply action in the
    /// bottom row.
    pub fn apply_current_profile(&mut self, ctx: &egui::Context) {
        let plan = match ApplyPlan::try_build_authorized(self.current_profile()) {
            Ok(plan) => plan,
            Err(error) => {
                let mut plan = ApplyPlan::default();
                plan.warnings
                    .push(ApplyWarning::new("Protocol", error.to_string()));
                retain_apply_plan(ctx, &plan);
                self.toast(ctx, unsupported_fields_message(&plan));
                return;
            }
        };
        retain_apply_plan(ctx, &plan);

        // The profile may contain fields whose device mapping is still
        // unverified.  Keep those warnings visible, but isolate the complete
        // reviewed sequence so the known PollingRate/DPI portion can still be
        // used without the vendor process.  No warning-bearing raw frame is
        // ever sent.
        let write_plan = if let Some(sequence) = plan.verified_sequence().cloned() {
            match ApplyPlan::from_verified_sequence(sequence) {
                Ok(plan) => plan,
                Err(error) => {
                    self.toast(ctx, format!("検証済み適用を準備できません: {error}"));
                    return;
                }
            }
        } else {
            // A warning-free dry-run can still have no evidence-backed write
            // tokens (for example, an empty profile).  Do not open a device
            // for a plan that has nothing authorized to send.
            if !plan.is_write_ready() {
                self.toast(ctx, no_verified_write_message(&plan));
                return;
            }
            plan.clone()
        };

        let mut transport = match device::open_configuration_device() {
            Ok(transport) => transport,
            Err(error) => {
                let mut state = phase2_ui_state(ctx);
                state.device_status = status_for_device_error(&error);
                store_phase2_ui_state(ctx, state);
                self.toast(ctx, dry_run_device_message(&plan, &error));
                return;
            }
        };

        // Only the complete evidence-backed sequence may cross the app-to-
        // device boundary.  The legacy raw-frame and individual-token seams
        // are intentionally not used here: the reviewed Apply burst must
        // reach the device in its exact order and count.
        let result = write_plan.apply_verified_sequence(&mut transport);

        match result {
            Ok(outcome) => {
                drop(transport);
                let mut state = phase2_ui_state(ctx);
                state.device_status = DeviceStatus::Connected;
                store_phase2_ui_state(ctx, state);
                if plan.warnings.is_empty() {
                    self.toast(ctx, format!("デバイスへ適用しました（{}件）", outcome.sent));
                } else {
                    self.toast(
                        ctx,
                        format!(
                            "既知項目を適用しました（{}件、未対応{}件は変更なし）",
                            outcome.sent,
                            plan.warnings.len()
                        ),
                    );
                }
            }
            Err(error) => {
                drop(transport);
                let mut state = phase2_ui_state(ctx);
                state.device_status = DeviceStatus::Error(error.to_string());
                store_phase2_ui_state(ctx, state);
                self.toast(ctx, format!("デバイス適用エラー: {error}"));
            }
        }
    }

    fn apply_menu_action(&mut self, ctx: &egui::Context, action: MenuAction) {
        match action {
            MenuAction::SetFunc(f) => {
                let m = self.menu_button;
                self.current_profile_mut().set_button_func(m, f);
            }
            MenuAction::SetSingleKey(idx) => {
                let m = self.menu_button;
                self.current_profile_mut().set_button_single_key(m, idx);
            }
            MenuAction::SetFuncWithKey(f, k) => {
                let m = self.menu_button;
                let prof = self.current_profile_mut();
                prof.set_button_func(m, f);
                let section = prof.button_section(m);
                prof.doc.set(&section, "KeyNumber", &k.to_string());
            }
            MenuAction::SetMacro(name) => {
                let m = self.menu_button;
                self.current_profile_mut().set_button_macro(m, &name);
            }
            MenuAction::OpenSingleKeyDialog => {
                self.dialog = crate::ui_dialog::DialogKind::SingleKey;
            }
            MenuAction::OpenComboDialog => {
                self.dialog = crate::ui_dialog::DialogKind::ComboKey;
            }
            MenuAction::OpenFireDialog => {
                self.dialog = crate::ui_dialog::DialogKind::FireKey;
            }
            MenuAction::OpenMacroManager => {
                self.macro_open = true;
            }
            MenuAction::Stub(msg) => self.toast(ctx, msg),
        }
        self.menu.open = false;
    }
}

fn status_for_device_error(error: &DeviceError) -> DeviceStatus {
    match error {
        DeviceError::DeviceUnavailable
        | DeviceError::UnsupportedPlatform
        | DeviceError::UnverifiedConfigurationMapping => DeviceStatus::Unavailable,
        other => DeviceStatus::Error(other.to_string()),
    }
}

fn retain_apply_plan(ctx: &egui::Context, plan: &ApplyPlan) {
    let mut state = phase2_ui_state(ctx);
    state.last_apply_warning_count = plan.warnings.len();
    state.last_apply_plan = Some(plan.clone());
    store_phase2_ui_state(ctx, state);
}

fn warning_fields(warnings: &[ApplyWarning]) -> String {
    const MAX_FIELDS: usize = 3;
    let mut fields = warnings
        .iter()
        .take(MAX_FIELDS)
        .map(|warning| warning.field_name().to_string())
        .collect::<Vec<_>>();
    if warnings.len() > MAX_FIELDS {
        fields.push(format!("他{}件", warnings.len() - MAX_FIELDS));
    }
    fields.join("、")
}

fn unsupported_fields_message(plan: &ApplyPlan) -> String {
    format!(
        "適用を停止しました: 未対応項目{}件（{}）。ドライランのみです",
        plan.warnings.len(),
        warning_fields(&plan.warnings)
    )
}

fn dry_run_message(plan: &ApplyPlan) -> String {
    if plan.warnings.is_empty() {
        return format!(
            "ドライラン: {}件の安全なフレームを準備しました",
            plan.frames.len()
        );
    }
    unsupported_fields_message(plan)
}

fn no_verified_write_message(plan: &ApplyPlan) -> String {
    if plan.verified_sequence().is_some() {
        return "ドライラン: 検証済みApplyシーケンスが送信対象です".to_string();
    }
    if plan.frames.is_empty() && plan.verified_frames().is_empty() {
        return "ドライラン: 検証済み書き込みマッピングがないため送信しません".to_string();
    }
    "ドライラン: 検証済みトークンだけが送信対象です".to_string()
}

fn dry_run_device_message(plan: &ApplyPlan, error: &DeviceError) -> String {
    if plan.warnings.is_empty() {
        let prepared = plan
            .verified_sequence()
            .map_or(plan.frames.len(), |sequence| sequence.len());
        return format!(
            "デバイス未接続（{}）。ドライラン: {}件、書込みなし",
            error, prepared
        );
    }
    format!(
        "デバイス未接続（{}）。{}",
        error,
        unsupported_fields_message(plan)
    )
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        crate::ui_frame::draw_background(self, ctx);
        crate::ui_frame::draw_titlebar(self, ctx);
        crate::ui_frame::draw_tabs(self, ctx);
        match self.tab {
            Tab::General => crate::ui_general::show(self, ctx),
            Tab::Dpi => crate::ui_dpi::show(self, ctx),
            Tab::Light => crate::ui_light::show(self, ctx),
            Tab::Info => crate::ui_info::show(self, ctx),
        }
        crate::ui_frame::draw_profile_row(self, ctx);
        crate::ui_frame::draw_bottom_row(self, ctx);

        if self.menu.open {
            let mut state = std::mem::take(&mut self.menu);
            let mut picked: Option<MenuAction> = None;
            let mut closed = false;
            egui::Area::new(egui::Id::new("assignment_menu"))
                .fixed_pos(state.pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let (p, c) = menu::draw(
                        ui,
                        &mut state,
                        &self.assets.menu_item_n,
                        &self.assets.menu_item_h,
                    );
                    picked = p;
                    closed = c;
                });
            self.menu = state;
            if let Some(action) = picked {
                self.apply_menu_action(ctx, action);
            } else if closed {
                self.menu.open = false;
            }
        }

        // ---- Phase 1.5: assignment sub-dialogs + macro manager ----
        if self.dialog != crate::ui_dialog::DialogKind::None {
            if let Some(res) = crate::ui_dialog::show(self, ctx) {
                let m = self.menu_button;
                match res {
                    crate::ui_dialog::DialogResult::SingleKey(usage) => {
                        self.current_profile_mut()
                            .set_button_single_key_usage(m, usage);
                    }
                    crate::ui_dialog::DialogResult::ComboKey(usage, modifiers) => {
                        self.current_profile_mut()
                            .set_button_combo(m, usage, modifiers);
                    }
                    crate::ui_dialog::DialogResult::FireKey {
                        target,
                        times,
                        delay_ms,
                    } => {
                        self.current_profile_mut()
                            .set_button_fire(m, target, times, delay_ms);
                    }
                }
                self.toast(ctx, "割り当てました");
            }
        }
        if self.macro_open {
            crate::ui_macro::show(self, ctx);
        }

        crate::ui_frame::draw_toast(self, ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn load_japanese_font(ctx: &egui::Context) {
    const CANDIDATES: [&str; 5] = [
        "meiryo.ttc",
        "YuGothM.ttc",
        "YuGothR.ttc",
        "msgothic.ttc",
        "msyh.ttc",
    ];
    for name in CANDIDATES {
        let path = std::path::Path::new("C:\\Windows\\Fonts").join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("jp".to_owned(), egui::FontData::from_owned(bytes));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "jp".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("jp".to_owned());
            ctx.set_fonts(fonts);
            return;
        }
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(800.0, 638.0))
            .with_decorations(false)
            .with_resizable(false)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/redsamurai.png"))
                    .expect("embedded RED SAMURAI icon must be valid PNG data"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "RED SAMURAI Gaming Mouse Configuration",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    #[test]
    fn phase2_context_state_starts_without_a_probe_request() {
        let ctx = egui::Context::default();
        let state = phase2_ui_state(&ctx);

        assert_eq!(state.device_status, DeviceStatus::NotChecked);
        assert!(state.last_apply_plan.is_none());
        assert_eq!(state.last_apply_warning_count, 0);
        assert!(state.device_status_checked_at.is_none());
    }

    #[test]
    fn unsupported_plan_message_exposes_the_dry_run_safety_boundary() {
        let plan = ApplyPlan::dry_run(&Profile::default_profile(1));
        let message = unsupported_fields_message(&plan);

        assert!(message.contains("未対応項目"));
        assert!(message.contains("ドライラン"));
        assert!(!plan.warnings.is_empty());
    }

    #[test]
    fn no_verified_write_message_explains_why_a_dry_run_stays_local() {
        let plan = ApplyPlan::default();
        let message = no_verified_write_message(&plan);

        assert!(message.contains("検証済み"));
        assert!(message.contains("ドライラン"));
    }
}
