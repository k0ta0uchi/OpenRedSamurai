//! Light tab — LED color palette, custom color, brightness/breath/mode options.
//! Restyled per Linear Design System (DESIGN.md): Midnight precision instrument.

use crate::app::App;
use crate::device_protocol::APPLY_LIGHT_MODE_PROFILE_VALUE_RAINBOW;
use crate::ui_common::{self, geo, theme};

use egui::{pos2, vec2, Align2, Color32, FontId, Rect, Sense};

/// HSV hue span (degrees) for each palette row:
/// reds→yellows / greens / cyans / blues / magentas.
fn row_hue(row: usize, col: usize) -> f32 {
    let (s, e) = match row {
        0 => (0.0, 55.0),
        1 => (80.0, 150.0),
        2 => (160.0, 195.0),
        3 => (210.0, 260.0),
        _ => (290.0, 330.0),
    };
    s + (col as f32 + 0.5) * (e - s) / 8.0
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h as u32 / 60) % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |t: f32| ((t + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(r), f(g), f(b))
}

/// Linear precision radio / option button.
fn linear_option_chip(ui: &egui::Ui, id: &str, rect: Rect, text: &str, selected: bool) -> bool {
    let resp = ui.interact(rect, egui::Id::new(id), Sense::click());
    ui_common::a11y::selectable(&resp, id, text, selected);
    let hovered = resp.hovered();

    let (bg, border) = if selected {
        (theme::OBSIDIAN, theme::SMOKE)
    } else if hovered {
        (theme::OBSIDIAN, theme::GRAPHITE)
    } else {
        (theme::VOID, theme::GRAPHITE)
    };

    ui.painter()
        .rect_filled(rect, egui::Rounding::same(theme::RADIUS_BUTTON), bg);
    ui.painter().rect_stroke(
        rect,
        egui::Rounding::same(theme::RADIUS_BUTTON),
        egui::Stroke::new(1.0, border),
    );

    // Status dot
    let dot_pos = pos2(rect.left() + 10.0, rect.center().y);
    ui.painter().circle_filled(
        dot_pos,
        if selected { 3.0 } else { 2.0 },
        if selected {
            theme::ACID_LIME
        } else {
            theme::ASH
        },
    );

    ui.painter().text(
        pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(11.0),
        if selected { theme::PAPER } else { theme::MIST },
    );

    resp.clicked()
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let rgb = app.current_profile().led_color_rgb();
    let state1 = app.current_profile().get_i32("LedState1", 2);
    let breath = app.current_profile().get_i32("BreathState1", 5);
    let mode = app.current_profile().get_i32("LedMode1", 3);
    let mouse_color = app.assets.mouse_color.clone();

    // Deferred mutations.
    let mut picked_rgb: Option<[u8; 3]> = None;
    let mut rainbow = false;
    let mut state_pick: Option<i32> = None;
    let mut breath_pick: Option<i32> = None;
    let mut mode_pick: Option<i32> = None;
    let mut picker_toggle = false;
    let mut picker_close = false;

    egui::Area::new(egui::Id::new("light_tab"))
        .fixed_pos(pos2(0.0, 0.0))
        .show(ctx, |ui| {
            // ---- Left Card: Mouse Color Preview ----
            let left_card = Rect::from_min_size(pos2(75.0, geo::CARD_Y), vec2(210.0, geo::CARD_H));
            ui_common::paint_card(ui, left_card, theme::CARBON, theme::GRAPHITE);

            ui.painter().text(
                pos2(left_card.left() + 16.0, left_card.top() + 14.0),
                Align2::LEFT_TOP,
                "DEVICE PREVIEW",
                FontId::proportional(11.0),
                theme::MIST,
            );

            let mrect = Rect::from_min_size(
                pos2(left_card.left() + 10.0, left_card.top() + 30.0),
                vec2(190.0, 250.0),
            );
            ui_common::paint_tex(
                ui,
                &mouse_color,
                ui_common::fit_rect(mrect, [622.0, 883.0]),
                Color32::WHITE,
            );

            // ---- Center Card: Light Palette & Custom Color ----
            let center_card =
                Rect::from_min_size(pos2(295.0, geo::CARD_Y), vec2(235.0, geo::CARD_H));
            ui_common::paint_card(ui, center_card, theme::CARBON, theme::GRAPHITE);

            ui.painter().text(
                pos2(center_card.left() + 16.0, center_card.top() + 14.0),
                Align2::LEFT_TOP,
                "LIGHT PALETTE",
                FontId::proportional(11.0),
                theme::MIST,
            );

            // 8x5 Palette Grid (chips with 4px radius)
            let chip_w = 21.0;
            let chip_gap = 4.0;
            let grid_x0 = center_card.left() + 16.0;
            let grid_y0 = center_card.top() + 38.0;

            for idx in 0..40usize {
                let col = idx % 8;
                let row = idx / 8;
                let r = Rect::from_min_size(
                    pos2(
                        grid_x0 + col as f32 * (chip_w + chip_gap),
                        grid_y0 + row as f32 * (chip_w + chip_gap),
                    ),
                    vec2(chip_w, chip_w),
                );

                if idx == 39 {
                    // Rainbow accent cell
                    for (s, hue) in [0.0f32, 90.0, 180.0, 270.0].iter().enumerate() {
                        let sw = Rect::from_min_max(
                            pos2(r.left() + s as f32 * 5.25, r.top()),
                            pos2(r.left() + (s + 1) as f32 * 5.25, r.bottom()),
                        );
                        ui.painter()
                            .rect_filled(sw, 0.0, hsv_to_rgb(*hue, 1.0, 1.0));
                    }
                    ui.painter().rect_stroke(
                        r,
                        egui::Rounding::same(theme::RADIUS_BADGE),
                        egui::Stroke::new(1.0, theme::GRAPHITE),
                    );
                    if mode == APPLY_LIGHT_MODE_PROFILE_VALUE_RAINBOW {
                        ui.painter().rect_stroke(
                            r,
                            egui::Rounding::same(theme::RADIUS_BADGE),
                            egui::Stroke::new(2.0, theme::PAPER),
                        );
                    }
                    let rainbow_resp =
                        ui.interact(r, egui::Id::new("palette_rainbow"), Sense::click());
                    ui_common::a11y::button(&rainbow_resp, "light.palette.rainbow", "虹");
                    if rainbow_resp.clicked() {
                        rainbow = true;
                    }
                } else {
                    let c = hsv_to_rgb(row_hue(row, col), 1.0, 1.0);
                    let resp =
                        ui.interact(r, egui::Id::new(format!("palette_{idx}")), Sense::click());
                    ui_common::a11y::button(
                        &resp,
                        &format!("light.palette.{idx}"),
                        &format!("パレット色 {idx}"),
                    );
                    let is_active = rgb == [c.r(), c.g(), c.b()];

                    ui.painter()
                        .rect_filled(r, egui::Rounding::same(theme::RADIUS_BADGE), c);
                    if is_active {
                        ui.painter().rect_stroke(
                            r,
                            egui::Rounding::same(theme::RADIUS_BADGE),
                            egui::Stroke::new(2.0, theme::PAPER),
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_stroke(
                            r,
                            egui::Rounding::same(theme::RADIUS_BADGE),
                            egui::Stroke::new(1.0, theme::PAPER),
                        );
                    }

                    if resp.clicked() {
                        picked_rgb = Some([c.r(), c.g(), c.b()]);
                    }
                }
            }

            // Custom Color Bar
            ui.painter().text(
                pos2(center_card.left() + 16.0, center_card.top() + 185.0),
                Align2::LEFT_TOP,
                "CUSTOM COLOR",
                FontId::proportional(11.0),
                theme::MIST,
            );

            let bar_rect = Rect::from_min_size(
                pos2(center_card.left() + 16.0, center_card.top() + 208.0),
                vec2(203.0, 32.0),
            );
            ui.painter().rect_filled(
                bar_rect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
            );
            ui.painter().rect_stroke(
                bar_rect,
                egui::Rounding::same(theme::RADIUS_BUTTON),
                egui::Stroke::new(1.0, theme::SMOKE),
            );
            // Pill button inside custom bar
            let edit_pill = Rect::from_min_size(
                pos2(bar_rect.right() - 56.0, bar_rect.center().y - 10.0),
                vec2(48.0, 20.0),
            );
            ui.painter().rect_filled(
                edit_pill,
                egui::Rounding::same(theme::RADIUS_PILL),
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
            ui.painter().text(
                edit_pill.center(),
                Align2::CENTER_CENTER,
                "EDIT",
                FontId::proportional(10.0),
                theme::PAPER,
            );

            let color_resp =
                ui.interact(bar_rect, egui::Id::new("custom_color_bar"), Sense::click());
            ui_common::a11y::button(&color_resp, "light.custom-color", "カスタム色");
            if color_resp.clicked() {
                picker_toggle = true;
            }

            // ---- Right Card: LED Mode & Options ----
            let right_card =
                Rect::from_min_size(pos2(540.0, geo::CARD_Y), vec2(200.0, geo::CARD_H));
            ui_common::paint_card(ui, right_card, theme::CARBON, theme::GRAPHITE);

            // 1. 輝度レベル
            ui.painter().text(
                pos2(right_card.left() + 14.0, right_card.top() + 14.0),
                Align2::LEFT_TOP,
                "BRIGHTNESS",
                FontId::proportional(11.0),
                theme::MIST,
            );
            for (k, (text, val)) in [("オフ", 0i32), ("低", 1), ("中", 2), ("高", 3)]
                .iter()
                .enumerate()
            {
                let rx = right_card.left() + 14.0 + (k % 2) as f32 * 88.0;
                let ry = right_card.top() + 32.0 + (k / 2) as f32 * 26.0;
                let r = Rect::from_min_size(pos2(rx, ry), vec2(82.0, 22.0));
                if linear_option_chip(ui, &format!("led_state_{val}"), r, text, state1 == *val) {
                    state_pick = Some(*val);
                }
            }

            // 2. 呼吸スピード
            ui.painter().text(
                pos2(right_card.left() + 14.0, right_card.top() + 96.0),
                Align2::LEFT_TOP,
                "SPEED",
                FontId::proportional(11.0),
                theme::MIST,
            );
            for (k, (text, val)) in [("低", 2i32), ("中", 5), ("高", 7), ("全点灯", 0)]
                .iter()
                .enumerate()
            {
                let rx = right_card.left() + 14.0 + (k % 2) as f32 * 88.0;
                let ry = right_card.top() + 114.0 + (k / 2) as f32 * 26.0;
                let r = Rect::from_min_size(pos2(rx, ry), vec2(82.0, 22.0));
                if linear_option_chip(ui, &format!("breath_{val}"), r, text, breath == *val) {
                    breath_pick = Some(*val);
                }
            }

            // 3. LEDモード
            ui.painter().text(
                pos2(right_card.left() + 14.0, right_card.top() + 178.0),
                Align2::LEFT_TOP,
                "MODE",
                FontId::proportional(11.0),
                theme::MIST,
            );
            for (k, (text, val)) in [
                ("フラッシュ", 0i32),
                ("オフ", 1),
                ("虹", 2),
                ("フル点灯", 3),
                ("呼吸", 4),
                ("波", 5),
            ]
            .iter()
            .enumerate()
            {
                let rx = right_card.left() + 14.0 + (k % 2) as f32 * 88.0;
                let ry = right_card.top() + 196.0 + (k / 2) as f32 * 26.0;
                let r = Rect::from_min_size(pos2(rx, ry), vec2(82.0, 22.0));
                if linear_option_chip(ui, &format!("led_mode_{val}"), r, text, mode == *val) {
                    mode_pick = Some(*val);
                }
            }
        });

    // ---- apply deferred mutations ----
    if let Some(c) = picked_rgb {
        app.current_profile_mut().set_led_color_rgb(c);
    }
    if rainbow {
        // The official GUI's rainbow Apply is backed by the complete
        // evidence-gated sequence and is represented by LedMode1=2.
        app.current_profile_mut()
            .set_i32("LedMode1", APPLY_LIGHT_MODE_PROFILE_VALUE_RAINBOW);
    }
    if let Some(v) = state_pick {
        app.current_profile_mut().set_i32("LedState1", v);
    }
    if let Some(v) = breath_pick {
        app.current_profile_mut().set_i32("BreathState1", v);
    }
    if let Some(v) = mode_pick {
        app.current_profile_mut().set_i32("LedMode1", v);
    }

    // ---- custom color picker popup ----
    if picker_toggle {
        app.custom_color_open = !app.custom_color_open;
        if app.custom_color_open {
            app.temp_rgb = app.current_profile().led_color_rgb();
        }
    }
    if app.custom_color_open {
        let mut edit = app.temp_rgb;
        egui::Area::new(egui::Id::new("custom_color_popup"))
            .fixed_pos(pos2(310.0, geo::CARD_Y + 245.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme::OBSIDIAN)
                    .rounding(egui::Rounding::same(theme::RADIUS_CARD))
                    .stroke(egui::Stroke::new(1.0, theme::SMOKE))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_min_width(200.0);
                        ui.label(egui::RichText::new("カスタムライトカラー").color(theme::PAPER));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            egui::color_picker::color_edit_button_srgb(ui, &mut edit);
                            ui.add_space(8.0);
                            if ui_common::linear_button(
                                ui,
                                "color_picker_close",
                                Rect::from_min_size(
                                    pos2(ui.cursor().left(), ui.cursor().top()),
                                    vec2(60.0, 24.0),
                                ),
                                "閉じる",
                                ui_common::ButtonKind::Ghost,
                            )
                            .clicked()
                            {
                                picker_close = true;
                            }
                        });
                    });
            });
        if edit != app.temp_rgb {
            app.current_profile_mut().set_led_color_rgb(edit);
            app.temp_rgb = edit;
        }
        if picker_close {
            app.custom_color_open = false;
        }
    }
}
