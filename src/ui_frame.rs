//! Window frame: background, titlebar, tabs, profile row, bottom row, toast.
//! Restyled per Linear Design System (DESIGN.md): Midnight precision instrument.

use crate::app::{App, Tab};
use crate::ini::IniDoc;
use crate::profile::Profile;
use crate::ui_common::{self, geo, theme, ButtonKind};

use egui::{pos2, vec2, Align2, FontId, Order, Rect, Sense};

fn size2(s: [f32; 2]) -> egui::Vec2 {
    vec2(s[0], s[1])
}

/// Linear Midnight Canvas background (Void #08090a with subtle header/footer panels and border).
pub fn draw_background(_app: &mut App, ctx: &egui::Context) {
    egui::Area::new(egui::Id::new("bg"))
        .fixed_pos(pos2(0.0, 0.0))
        .order(Order::Background)
        .interactable(false)
        .show(ctx, |ui| {
            let p = ui.painter();
            let win_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 638.0));

            // Main canvas (Void #08090a) with 12px corner rounding
            p.rect_filled(
                win_rect,
                egui::Rounding::same(theme::RADIUS_CARD),
                theme::VOID,
            );
            // Outer hairline border (Graphite #23252a)
            p.rect_stroke(
                win_rect,
                egui::Rounding::same(theme::RADIUS_CARD),
                egui::Stroke::new(1.0, theme::GRAPHITE),
            );

            // Top Header bar panel (Carbon #0f1011)
            let header_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 56.0));
            p.rect_filled(
                header_rect,
                egui::Rounding {
                    nw: theme::RADIUS_CARD,
                    ne: theme::RADIUS_CARD,
                    sw: 0.0,
                    se: 0.0,
                },
                theme::CARBON,
            );
            // Header bottom hairline divider
            p.line_segment(
                [pos2(0.0, 56.0), pos2(800.0, 56.0)],
                egui::Stroke::new(1.0, theme::GRAPHITE),
            );

            // Bottom action bar panel (Carbon #0f1011)
            let footer_rect = Rect::from_min_size(pos2(0.0, 550.0), vec2(800.0, 88.0));
            p.rect_filled(
                footer_rect,
                egui::Rounding {
                    nw: 0.0,
                    ne: 0.0,
                    sw: theme::RADIUS_CARD,
                    se: theme::RADIUS_CARD,
                },
                theme::CARBON,
            );
            // Footer top hairline divider
            p.line_segment(
                [pos2(0.0, 550.0), pos2(800.0, 550.0)],
                egui::Stroke::new(1.0, theme::GRAPHITE),
            );
        });
}

/// Linear Titlebar: Logo, brand metadata, window drag, and clean minimize/close buttons.
pub fn draw_titlebar(app: &mut App, ctx: &egui::Context) {
    // Header drag zone (excluding button area at the right)
    egui::Area::new(egui::Id::new("titlebar_drag"))
        .fixed_pos(pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(
                Rect::from_min_size(pos2(0.0, 0.0), vec2(720.0, 56.0)),
                Sense::drag(),
            );
            if resp.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            // Left brand identity
            let p = ui.painter();
            // Geometric red accent dot / square (representing Samurai precision)
            p.rect_filled(
                Rect::from_center_size(pos2(26.0, 28.0), vec2(8.0, 8.0)),
                egui::Rounding::same(2.0),
                theme::CORAL_RED,
            );
            p.text(
                pos2(38.0, 20.0),
                Align2::LEFT_TOP,
                "RED SAMURAI",
                FontId::proportional(14.0),
                theme::PAPER,
            );
            p.text(
                pos2(148.0, 22.0),
                Align2::LEFT_TOP,
                "16400DPI Precision Control",
                FontId::proportional(11.0),
                theme::ASH,
            );
        });

    // Window controls (Minimize & Close buttons) at top right
    egui::Area::new(egui::Id::new("titlebar_controls"))
        .fixed_pos(pos2(724.0, 14.0))
        .show(ctx, |ui| {
            let min_r = Rect::from_min_size(pos2(726.0, 14.0), vec2(28.0, 28.0));
            let min_resp = ui.interact(min_r, egui::Id::new("btn_min"), Sense::click());
            ui_common::a11y::button(&min_resp, "titlebar.minimize", "最小化");
            let min_hover = min_resp.hovered();
            let p = ui.painter();
            if min_hover {
                p.rect_filled(
                    min_r,
                    egui::Rounding::same(theme::RADIUS_BUTTON),
                    theme::OBSIDIAN,
                );
            }
            p.text(
                min_r.center(),
                Align2::CENTER_CENTER,
                "−",
                FontId::proportional(16.0),
                if min_hover { theme::PAPER } else { theme::FOG },
            );
            if min_resp.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }

            let close_r = Rect::from_min_size(pos2(758.0, 14.0), vec2(28.0, 28.0));
            let close_resp = ui.interact(close_r, egui::Id::new("btn_close"), Sense::click());
            ui_common::a11y::button(&close_resp, "titlebar.close", "閉じる");
            let close_hover = close_resp.hovered();
            if close_hover {
                p.rect_filled(
                    close_r,
                    egui::Rounding::same(theme::RADIUS_BUTTON),
                    theme::CORAL_RED,
                );
            }
            p.text(
                close_r.center(),
                Align2::CENTER_CENTER,
                "✕",
                FontId::proportional(13.0),
                if close_hover {
                    theme::PAPER
                } else {
                    theme::FOG
                },
            );
            if close_resp.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

    draw_device_status(app, ctx);
}

/// Paint a compact, read-only-by-default device indicator in the title bar.
/// Clicking the pill performs one explicit metadata-only discovery check;
/// it never opens the device or sends a report.  Keeping this interaction
/// outside the regular frame loop makes status rendering non-blocking for the
/// normal editing path and keeps startup free of HID work.
fn draw_device_status(app: &mut App, ctx: &egui::Context) {
    let rect = Rect::from_min_size(pos2(490.0, 14.0), vec2(220.0, 28.0));
    let phase2 = crate::app::phase2_ui_state(ctx);
    let status_label = if phase2.last_apply_warning_count == 0 {
        phase2.device_status.label()
    } else {
        format!(
            "{} · 未対応{}件",
            phase2.device_status.label(),
            phase2.last_apply_warning_count
        )
    };
    let mut clicked = false;
    egui::Area::new(egui::Id::new("device_status"))
        .fixed_pos(pos2(0.0, 0.0))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            let response = ui.interact(rect, egui::Id::new("device_status_button"), Sense::click());
            ui_common::a11y::button(&response, "device.status", &status_label);
            let (dot, text_color) = match &phase2.device_status {
                crate::app::DeviceStatus::Connected => (theme::PULSE_GREEN, theme::MIST),
                crate::app::DeviceStatus::Unavailable => (theme::CORAL_RED, theme::FOG),
                crate::app::DeviceStatus::Error(_) => (theme::CORAL_RED, theme::FOG),
                crate::app::DeviceStatus::NotChecked => (theme::ASH, theme::FOG),
            };
            let painter = ui.painter();
            painter.rect_filled(
                rect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                if response.hovered() {
                    theme::OBSIDIAN
                } else {
                    theme::CARBON
                },
            );
            painter.rect_stroke(
                rect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                egui::Stroke::new(1.0, theme::GRAPHITE),
            );
            painter.circle_filled(pos2(rect.left() + 13.0, rect.center().y), 3.5, dot);
            painter.text(
                pos2(rect.left() + 24.0, rect.center().y),
                Align2::LEFT_CENTER,
                status_label.clone(),
                FontId::proportional(10.0),
                text_color,
            );
            let response_clicked = response.clicked();
            if response.hovered() {
                response.on_hover_text("クリックで軽量な接続確認（書き込みなし）");
            }
            clicked = response_clicked;
        });
    if clicked {
        app.probe_device(ctx);
    }
}

/// Linear Navigation Tabs: 一般 / DPI / ライト / 情報 with electric indicators.
pub fn draw_tabs(app: &mut App, ctx: &egui::Context) {
    let tabs = [
        (Tab::General, "一般"),
        (Tab::Dpi, "DPI"),
        (Tab::Light, "ライト"),
        (Tab::Info, "情報"),
    ];
    let mut clicked: Option<Tab> = None;
    egui::Area::new(egui::Id::new("tabs"))
        .fixed_pos(pos2(geo::TAB_X0, geo::TAB_Y))
        .show(ctx, |ui| {
            // Container bar for tabs
            let bar_rect = Rect::from_min_size(
                pos2(geo::TAB_X0 - 4.0, geo::TAB_Y - 3.0),
                vec2(geo::TAB_PITCH * 4.0 + 8.0, geo::TAB_SIZE[1] + 6.0),
            );
            ui_common::paint_panel(ui, bar_rect, theme::CARBON, theme::GRAPHITE);

            for (i, (tab, name)) in tabs.iter().enumerate() {
                let rect = Rect::from_min_size(
                    pos2(geo::TAB_X0 + i as f32 * geo::TAB_PITCH, geo::TAB_Y),
                    size2(geo::TAB_SIZE),
                );
                let active = app.tab == *tab;
                let resp = ui_common::linear_tab(ui, &format!("tab_{i}"), rect, name, active);
                if resp.clicked() {
                    clicked = Some(*tab);
                }
            }
        });
    if let Some(tab) = clicked {
        app.tab = tab;
    }
}

/// Linear Profile Selector: プロファイル#1..#5 pill buttons with Acid Lime active dot.
pub fn draw_profile_row(app: &mut App, ctx: &egui::Context) {
    let mut clicked: Option<usize> = None;
    egui::Area::new(egui::Id::new("profile_row"))
        .fixed_pos(pos2(geo::PROFILE_X0, geo::PROFILE_Y))
        .show(ctx, |ui| {
            for i in 0..5usize {
                let rect = Rect::from_min_size(
                    pos2(
                        geo::PROFILE_X0 + i as f32 * geo::PROFILE_PITCH,
                        geo::PROFILE_Y,
                    ),
                    size2(geo::PROFILE_SIZE),
                );
                let sel = app.current == i;
                let resp = ui_common::linear_profile_button(
                    ui,
                    &format!("profile_{i}"),
                    rect,
                    &format!("プロファイル #{}", i + 1),
                    sel,
                );
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
    if let Some(i) = clicked {
        app.current = i;
    }
}

/// The 7 bottom action buttons styled per Linear design system:
/// "適用" is the singular chromatic Acid Lime primary button.
/// "OK" is high-contrast bone highlight.
/// Others are sleek ghost/outline buttons with hairline graphite borders.
pub fn draw_bottom_row(app: &mut App, ctx: &egui::Context) {
    let mut pressed: Option<usize> = None;
    egui::Area::new(egui::Id::new("bottom_row"))
        .fixed_pos(pos2(0.0, geo::BOTTOM_Y))
        .show(ctx, |ui| {
            for (i, (name, x, w)) in geo::BOTTOM_BUTTONS.iter().enumerate() {
                let rect = Rect::from_min_size(pos2(*x, geo::BOTTOM_Y), vec2(*w, geo::BOTTOM_H));
                let kind = match *name {
                    "適用" => ButtonKind::Primary,
                    "OK" => ButtonKind::Highlight,
                    _ => ButtonKind::Ghost,
                };
                let resp = ui_common::linear_button(ui, &format!("bottom_{i}"), rect, name, kind);
                if resp.clicked() {
                    pressed = Some(i);
                }
            }
        });
    if let Some(i) = pressed {
        handle_bottom(app, ctx, i);
    }
}

fn handle_bottom(app: &mut App, ctx: &egui::Context, i: usize) {
    match i {
        0 => save_as_dialog(app, ctx),   // 保存: 名前をつけて保存
        1 => load_file_dialog(app, ctx), // ロードファイル
        2 => {
            // 既定: reset current slot to defaults, keeping the slot.
            let slot = app.profiles[app.current].slot;
            app.profiles[app.current] = Profile::default_profile(slot);
            app.toast(ctx, "初期値にリセットしました");
        }
        3 => {
            // すべてリセット: reset all 5 slots.
            for i in 0..5 {
                let slot = app.profiles[i].slot;
                app.profiles[i] = Profile::default_profile(slot);
            }
            app.toast(ctx, "全プロファイルをリセットしました");
        }
        4 => {
            // OK: save current file, then close.
            if save_current(app, ctx) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        5 => {
            // キャンセル: reload all 5 from disk, then close.
            reload_all(app);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        6 => {
            // 適用: persist the profile, then explicitly prepare/apply the
            // fail-closed device plan.  No other bottom-row action writes to
            // hardware.
            // A failed persistence step must prevent the device write: the
            // bytes on disk are the source of truth for the profile that is
            // about to cross the HID boundary.
            if save_current(app, ctx) {
                app.apply_current_profile(ctx);
            }
        }
        _ => {}
    }
}

/// Write the current profile to its on-disk `RSProfile{slot}.pfd`.
fn save_current(app: &mut App, ctx: &egui::Context) -> bool {
    let Some(path) = Profile::profile_path(app.current + 1) else {
        app.toast(ctx, "保存先を取得できないため処理を中止しました");
        return false;
    };
    match app.current_profile().save(&path) {
        Ok(()) => true,
        Err(e) => {
            app.toast(ctx, format!("保存エラー: {e}"));
            false
        }
    }
}

/// 保存: save-as dialog, writes the current profile as a `.pfd`.
fn save_as_dialog(app: &mut App, ctx: &egui::Context) {
    let mut dialog = rfd::FileDialog::new()
        .add_filter("RED SAMURAI profile (*.pfd)", &["pfd"])
        .set_file_name(format!("RSProfile{}.pfd", app.current + 1));
    if let Some(dir) = Profile::doc_dir() {
        dialog = dialog.set_directory(&dir);
    }
    let Some(path) = dialog.save_file() else {
        return;
    };
    match app.current_profile().save(&path) {
        Ok(_) => {
            let file_name = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "RSProfile.pfd".to_string());
            app.toast(ctx, format!("保存しました: {file_name}"));
        }
        Err(e) => {
            app.toast(ctx, format!("保存エラー: {e}"));
        }
    }
}

/// ロードファイル: open dialog, import a `.pfd` into the current slot
/// (GROUP*/ButtonAssigned{n}_* sections are renamed to the current slot naming).
fn load_file_dialog(app: &mut App, ctx: &egui::Context) {
    let mut dialog = rfd::FileDialog::new().add_filter("RED SAMURAI profile (*.pfd)", &["pfd"]);
    if let Some(dir) = Profile::doc_dir() {
        dialog = dialog.set_directory(&dir);
    }
    let Some(path) = dialog.pick_file() else {
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let mut doc = IniDoc::parse(&text);
            let cur = app.current; // 0..=4 → GROUP{cur} / ButtonAssigned{cur}_{m}
            for sec in &mut doc.sections {
                if let Some(rest) = sec.name.strip_prefix("GROUP") {
                    // Numbered GROUP{n} sections are renamed; legacy [GROUP] stays.
                    if !rest.is_empty() && rest.parse::<usize>().is_ok() {
                        sec.name = format!("GROUP{cur}");
                    }
                } else if let Some(rest) = sec.name.strip_prefix("ButtonAssigned") {
                    if let Some((n, m)) = rest.split_once('_') {
                        // Numbered ButtonAssigned{n}_{m}; legacy ButtonAssigned_{m} stays.
                        if !n.is_empty() && n.parse::<usize>().is_ok() && m.parse::<usize>().is_ok()
                        {
                            sec.name = format!("ButtonAssigned{cur}_{m}");
                        }
                    }
                }
            }
            app.profiles[cur] = Profile { slot: cur + 1, doc };
            let file_name = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "RSProfile.pfd".to_string());
            app.toast(ctx, format!("読み込みました: {file_name}"));
        }
        Err(e) => app.toast(ctx, format!("読み込みエラー: {e}")),
    }
}

/// Reload all 5 profiles from disk (falling back to defaults when missing).
fn reload_all(app: &mut App) {
    app.profiles = (1..=5)
        .map(|slot| match Profile::profile_path(slot) {
            Some(path) => Profile::load(&path, slot),
            None => Profile::default_profile(slot),
        })
        .collect();
}

/// Small centered dark toast near y=600, auto-hidden after ~2.5s.
pub fn draw_toast(app: &mut App, ctx: &egui::Context) {
    let Some((msg, t0)) = app.status.clone() else {
        return;
    };
    let now = ctx.input(|inp| inp.time);
    if now - t0 >= 2.5 {
        app.status = None;
        return;
    }
    let galley = ctx.fonts(|f| f.layout_no_wrap(msg, FontId::proportional(12.0), theme::PAPER));
    let text_size = galley.size();
    let rect = Rect::from_center_size(
        pos2(400.0, 520.0),
        vec2((text_size.x + 32.0).max(140.0), text_size.y + 16.0),
    );
    egui::Area::new(egui::Id::new("toast"))
        .fixed_pos(rect.min)
        .order(Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let p = ui.painter();
            p.rect_filled(
                rect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                theme::OBSIDIAN,
            );
            p.rect_stroke(
                rect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                egui::Stroke::new(1.0, theme::SMOKE),
            );
            // Pulse green or acid lime status dot
            p.circle_filled(
                pos2(rect.left() + 14.0, rect.center().y),
                2.5,
                theme::ACID_LIME,
            );
            p.galley(
                pos2(rect.left() + 24.0, rect.center().y - text_size.y / 2.0),
                galley,
                theme::PAPER,
            );
        });
    // Keep repainting while the toast counts down to its hide time.
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}
