//! General tab — mouse preview, assignment rows, sliders, polling rate,
//! double-click test. CONTRACT in CONTRACT_app.md (implemented by Worker 4).

use crate::app::App;
use crate::menu::MenuState;
use crate::ui_common::{self, geo, theme};

use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense};

/// Assignment rows top edge (front: 6 rows, side: 12 rows).
const ROWS_Y_FRONT: f32 = 212.0;
const ROWS_Y_SIDE: f32 = 203.0;

/// (label, profile key, min, max, label y, small track) for the right column.
const SLIDERS: [(&str, &str, i32, i32, f32, bool); 4] = [
    ("加速度", "Acceleration", 0, 2, 198.0, false),
    ("POINTER SPEED", "MouseSensitivity", 1, 20, 247.0, false),
    (
        "スクロールスピード",
        "WheelScrollLines",
        1,
        10,
        295.0,
        false,
    ),
    (
        "ダブルクリックスピード",
        "DoubleClickSpeed",
        1,
        10,
        346.0,
        true,
    ),
];

/// Polling rate choices: label → PollingRate value.
///
/// These values are pinned by the official Apply captures: the PFD stores
/// 125/250/500/1000 Hz as 8/4/2/1 respectively.  The wire byte is the same
/// value for the reviewed 125 Hz sequence, but the other values remain
/// evidence-only at the device boundary.
const POLLING: [(&str, i32); 4] = [("125HZ", 8), ("250HZ", 4), ("500HZ", 2), ("1000HZ", 1)];

#[cfg(test)]
mod tests {
    use super::POLLING;

    #[test]
    fn polling_labels_use_the_observed_profile_values() {
        assert_eq!(
            POLLING,
            [("125HZ", 8), ("250HZ", 4), ("500HZ", 2), ("1000HZ", 1)]
        );
    }
}

/// Defaults matching the reference screenshots / original files.
fn default_for(key: &str) -> i32 {
    match key {
        "Acceleration" => 1,
        "MouseSensitivity" => 9,
        "WheelScrollLines" => 3,
        _ => 5,
    }
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let now = ctx.input(|inp| inp.time);
    let mouse = if app.side {
        app.assets.mouse_side.clone()
    } else {
        app.assets.mouse_front.clone()
    };

    // Deferred mutations (applied after the draw pass).
    let mut open_menu: Option<(usize, Pos2)> = None;
    let mut side_pick: Option<bool> = None;
    let mut sets: Vec<(&'static str, i32)> = Vec::new();
    let mut poll_pick: Option<i32> = None;
    let mut test_click = false;

    egui::Area::new(egui::Id::new("general_tab"))
        .fixed_pos(pos2(0.0, 0.0))
        .show(ctx, |ui| {
            // ---- Left Card: Mouse Preview (FRONT / SIDE) ----
            let left_card_rect = Rect::from_min_size(pos2(75.0, 195.0), vec2(230.0, 298.0));
            ui_common::paint_card(ui, left_card_rect, theme::CARBON, theme::GRAPHITE);

            let mrect = if app.side {
                Rect::from_min_size(pos2(90.0, 205.0), vec2(200.0, 235.0))
            } else {
                Rect::from_min_size(pos2(90.0, 205.0), vec2(200.0, 235.0))
            };
            let msize: [f32; 2] = if app.side {
                [547.0, 1365.0]
            } else {
                [622.0, 883.0]
            };
            ui_common::paint_tex(
                ui,
                &mouse,
                ui_common::fit_rect(mrect, msize),
                Color32::WHITE,
            );

            // FRONT / SIDE toggle pill buttons inside the card
            let front_r = Rect::from_min_size(pos2(95.0, 456.0), vec2(90.0, 24.0));
            let side_r = Rect::from_min_size(pos2(195.0, 456.0), vec2(90.0, 24.0));
            if ui_common::linear_button(
                ui,
                "side_front",
                front_r,
                "FRONT",
                ui_common::ButtonKind::Pill { active: !app.side },
            )
            .clicked()
            {
                side_pick = Some(false);
            }
            if ui_common::linear_button(
                ui,
                "side_side",
                side_r,
                "SIDE",
                ui_common::ButtonKind::Pill { active: app.side },
            )
            .clicked()
            {
                side_pick = Some(true);
            }

            // ---- Middle: Assignment Rows Card ----
            let count = if app.side { 12usize } else { 6 };
            let pitch = if app.side {
                geo::ROW_PITCH_SIDE
            } else {
                geo::ROW_PITCH_FRONT
            };
            let y0 = if app.side { ROWS_Y_SIDE } else { ROWS_Y_FRONT };
            let first = if app.side { 7usize } else { 1 };

            for k in 0..count {
                let m = first + k;
                let rect = Rect::from_min_size(
                    pos2(geo::ROWS_X, y0 + k as f32 * pitch),
                    vec2(geo::ROW_W, geo::ROW_H),
                );
                let selected = app.menu.open && app.menu_button == m;

                let resp = ui.interact(rect, egui::Id::new(format!("row_{m}")), Sense::click());
                ui_common::a11y::button(
                    &resp,
                    &format!("assignment.row.{m}"),
                    &format!("ボタン {m}"),
                );
                let hovered = resp.hovered();

                let (bg, border) = if selected {
                    (theme::OBSIDIAN, theme::ACID_LIME)
                } else if hovered {
                    (theme::OBSIDIAN, theme::SMOKE)
                } else {
                    (theme::CARBON, theme::GRAPHITE)
                };

                ui.painter()
                    .rect_filled(rect, egui::Rounding::same(theme::RADIUS_BUTTON), bg);
                ui.painter().rect_stroke(
                    rect,
                    egui::Rounding::same(theme::RADIUS_BUTTON),
                    egui::Stroke::new(1.0, border),
                );

                // Button index badge
                let badge_rect = Rect::from_min_size(
                    pos2(rect.left() + 6.0, rect.center().y - 8.0),
                    vec2(20.0, 16.0),
                );
                ui.painter().rect_filled(
                    badge_rect,
                    egui::Rounding::same(theme::RADIUS_BADGE),
                    if selected {
                        theme::SMOKE
                    } else {
                        theme::GRAPHITE
                    },
                );
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{m}"),
                    FontId::proportional(10.5),
                    theme::MIST,
                );

                let a = app.current_profile().button(m);
                let glyph = app.current_profile().button_key_glyph(m);
                let text = if a.func == 100 {
                    crate::funcs::func_label_ext(100, glyph, Some(a.macro_name.as_str()))
                } else if a.func == 6 && glyph.is_none() && a.key_number != 0 {
                    format!("シングルキー({})", crate::funcs::usage_label(a.key_number))
                } else {
                    crate::funcs::func_label_full(
                        a.func,
                        glyph,
                        Some(a.macro_name.as_str()),
                        Some(a.key_number),
                    )
                };

                let text_color = if selected {
                    theme::PAPER
                } else if hovered {
                    theme::PAPER
                } else {
                    theme::MIST
                };
                ui.painter().text(
                    pos2(rect.left() + 32.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    text,
                    FontId::proportional(11.5),
                    text_color,
                );

                // Right chevron indicator
                ui.painter().text(
                    pos2(rect.right() - 10.0, rect.center().y),
                    Align2::CENTER_CENTER,
                    "›",
                    FontId::proportional(12.0),
                    if hovered { theme::MIST } else { theme::ASH },
                );

                if resp.clicked() {
                    if let Some(p) = resp.interact_pointer_pos() {
                        open_menu = Some((m, p));
                    }
                }
            }

            // ---- Right Column: Sliders & Settings Card ----
            let right_card_rect = Rect::from_min_size(pos2(512.0, 195.0), vec2(228.0, 298.0));
            ui_common::paint_card(ui, right_card_rect, theme::CARBON, theme::GRAPHITE);

            // Sliders
            for (text, key, min, max, ly, small) in SLIDERS {
                let v = app
                    .current_profile()
                    .get_i32(key, default_for(key))
                    .clamp(min, max);

                let w = if small { 130.0 } else { 190.0 };
                let rel_y = ly + 2.0;

                // Header line: Label left, Value right
                ui.painter().text(
                    pos2(geo::SLIDER_X + 6.0, rel_y),
                    Align2::LEFT_TOP,
                    text,
                    FontId::proportional(11.0),
                    theme::MIST,
                );
                ui.painter().text(
                    pos2(geo::SLIDER_X + w + 4.0, rel_y),
                    Align2::RIGHT_TOP,
                    format!("{v}"),
                    FontId::proportional(11.0),
                    theme::PAPER,
                );

                let rect =
                    Rect::from_min_size(pos2(geo::SLIDER_X + 6.0, rel_y + 16.0), vec2(w, 16.0));
                if let Some(nv) =
                    ui_common::linear_hslider(ui, &format!("slider_{key}"), rect, v, min, max)
                {
                    sets.push((key, nv));
                }
            }

            // Double-click test target panel (inside right card)
            let trect = Rect::from_min_size(pos2(660.0, 348.0), vec2(66.0, 48.0));
            let tresp = ui.interact(trect, egui::Id::new("dblclick_test"), Sense::click());
            ui_common::a11y::button(&tresp, "general.double-click-test", "ダブルクリックテスト");
            let thover = tresp.hovered();

            ui.painter().rect_filled(
                trect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                if thover { theme::OBSIDIAN } else { theme::VOID },
            );
            ui.painter().rect_stroke(
                trect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                egui::Stroke::new(
                    1.0,
                    if thover {
                        theme::SMOKE
                    } else {
                        theme::GRAPHITE
                    },
                ),
            );

            // Radar/Target graphic
            let tc = trect.center();
            ui.painter()
                .circle_stroke(tc, 16.0, egui::Stroke::new(1.0, theme::GRAPHITE));
            ui.painter()
                .circle_stroke(tc, 8.0, egui::Stroke::new(1.0, theme::SMOKE));
            ui.painter().circle_filled(tc, 2.5, theme::CORAL_RED);
            ui.painter().text(
                pos2(trect.center().x, trect.top() + 4.0),
                Align2::CENTER_TOP,
                "TEST",
                FontId::proportional(8.5),
                theme::ASH,
            );

            if tresp.clicked() {
                test_click = true;
            }
            if let Some(res) = app.dblclick.result {
                let status_color = match res {
                    "OK" => theme::PULSE_GREEN,
                    _ => theme::CORAL_RED,
                };
                ui.painter().text(
                    pos2(trect.center().x, trect.bottom() + 3.0),
                    Align2::CENTER_TOP,
                    res,
                    FontId::proportional(11.0),
                    status_color,
                );
            }

            // Polling rate
            ui.painter().text(
                pos2(geo::SLIDER_X + 6.0, 420.0),
                Align2::LEFT_TOP,
                "ポーリングレート",
                FontId::proportional(11.0),
                theme::MIST,
            );
            let cur_poll = app.current_profile().get_i32("PollingRate", 8);
            for (k, (hz, val)) in POLLING.iter().enumerate() {
                let mut on = cur_poll == *val;
                let resp = ui_common::linear_checkbox(
                    ui,
                    &format!("poll_{k}"),
                    pos2(geo::SLIDER_X + 6.0 + k as f32 * 50.0, 442.0),
                    hz,
                    &mut on,
                );
                if resp.clicked() && on {
                    poll_pick = Some(*val);
                }
            }
        });

    // ---- apply deferred mutations ----
    for (key, v) in sets {
        app.current_profile_mut().set_i32(key, v);
    }
    if let Some(v) = poll_pick {
        app.current_profile_mut().set_i32("PollingRate", v);
    }
    if let Some(side) = side_pick {
        app.side = side;
    }
    if let Some((m, p)) = open_menu {
        app.menu_button = m;
        let macro_names: Vec<String> = app
            .macros
            .names
            .iter()
            .filter(|n| !n.is_empty())
            .cloned()
            .collect();
        app.menu = MenuState {
            open: true,
            pos: p,
            items: crate::funcs::build_menu(&macro_names),
        };
    }
    if test_click {
        match app.dblclick.last_click {
            Some(t) if now - t <= 0.5 => {
                app.dblclick.result = Some(if now - t < 0.08 { "速すぎ" } else { "OK" });
                app.dblclick.last_click = Some(now);
            }
            Some(_) => {
                app.dblclick.result = Some("遅い");
                app.dblclick.last_click = Some(now);
            }
            None => app.dblclick.last_click = Some(now),
        }
    }
}
