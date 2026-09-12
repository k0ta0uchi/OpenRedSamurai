//! DPI tab — 5 stage columns with vertical precision sliders + stage selection.
//! Restyled per Linear Design System (DESIGN.md): Midnight precision instrument.

use crate::app::App;
use crate::profile::{dpi_code_to_display, DpiStage};
use crate::ui_common::{self, geo, theme};

use egui::{pos2, vec2, Align2, FontId, Rect, Sense};

pub fn show(app: &mut App, ctx: &egui::Context) {
    let stages = app.current_profile().dpi_stages();
    let current = app.current_profile().current_dpi_stage();

    // Deferred mutations.
    let mut stage_set: Option<(usize, DpiStage)> = None;
    let mut cur_set: Option<usize> = None;

    egui::Area::new(egui::Id::new("dpi_tab"))
        .fixed_pos(pos2(0.0, 0.0))
        .show(ctx, |ui| {
            // ---- Left Card: Current Stage Selector ----
            let left_card = Rect::from_min_size(pos2(75.0, geo::CARD_Y), vec2(130.0, geo::CARD_H));
            ui_common::paint_card(ui, left_card, theme::CARBON, theme::GRAPHITE);

            ui.painter().text(
                pos2(left_card.left() + 16.0, left_card.top() + 16.0),
                Align2::LEFT_TOP,
                "ACTIVE STAGE",
                FontId::proportional(11.0),
                theme::MIST,
            );

            for i in 0..5usize {
                let r = Rect::from_min_size(
                    pos2(
                        left_card.left() + 14.0,
                        left_card.top() + 42.0 + i as f32 * 46.0,
                    ),
                    vec2(102.0, 34.0),
                );
                let is_cur = i == current;
                let resp = ui.interact(r, egui::Id::new(format!("dpi_led_{i}")), Sense::click());
                ui_common::a11y::button(
                    &resp,
                    &format!("dpi.stage.{i}"),
                    &format!("ステージ {}", i + 1),
                );
                let hovered = resp.hovered();

                let (bg, border) = if is_cur {
                    (theme::OBSIDIAN, theme::SMOKE)
                } else if hovered {
                    (theme::OBSIDIAN, theme::GRAPHITE)
                } else {
                    (theme::CARBON, theme::GRAPHITE)
                };

                ui.painter()
                    .rect_filled(r, egui::Rounding::same(theme::RADIUS_BUTTON), bg);
                ui.painter().rect_stroke(
                    r,
                    egui::Rounding::same(theme::RADIUS_BUTTON),
                    egui::Stroke::new(1.0, border),
                );

                // Status dot (Acid lime when selected)
                let dot_pos = pos2(r.left() + 14.0, r.center().y);
                ui.painter().circle_filled(
                    dot_pos,
                    if is_cur { 3.5 } else { 2.5 },
                    if is_cur { theme::ACID_LIME } else { theme::ASH },
                );

                ui.painter().text(
                    pos2(r.left() + 28.0, r.center().y),
                    Align2::LEFT_CENTER,
                    format!("Stage {}", i + 1),
                    FontId::proportional(12.0),
                    if is_cur { theme::PAPER } else { theme::MIST },
                );

                if resp.clicked() {
                    cur_set = Some(i);
                }
            }

            // ---- 5 Stage Columns: Precision Cards ----
            let col_w = 98.0;
            let col_x0 = 220.0;
            let col_gap = 10.0;

            for (i, st) in stages.iter().enumerate() {
                let cx = col_x0 + i as f32 * (col_w + col_gap);
                let card_rect =
                    Rect::from_min_size(pos2(cx, geo::CARD_Y), vec2(col_w, geo::CARD_H));
                let is_cur = i == current;

                let (bg, border) = if is_cur {
                    (theme::CARBON, theme::SMOKE)
                } else {
                    (theme::CARBON, theme::GRAPHITE)
                };
                ui_common::paint_card(ui, card_rect, bg, border);

                // Stage header / badge (click toggles enabled)
                let badge_rect = Rect::from_min_size(
                    pos2(card_rect.center().x - 38.0, card_rect.top() + 14.0),
                    vec2(76.0, 24.0),
                );
                let b_resp = ui.interact(
                    badge_rect,
                    egui::Id::new(format!("dpi_label_{i}")),
                    Sense::click(),
                );
                ui_common::a11y::button(
                    &b_resp,
                    &format!("dpi.stage-enabled.{i}"),
                    &format!("ステージ {} 有効", i + 1),
                );
                let b_hover = b_resp.hovered();

                let (b_bg, b_border, b_color) = if st.enabled {
                    (theme::OBSIDIAN, theme::ACID_LIME, theme::ACID_LIME)
                } else if b_hover {
                    (theme::OBSIDIAN, theme::SMOKE, theme::MIST)
                } else {
                    (theme::VOID, theme::GRAPHITE, theme::ASH)
                };

                ui.painter().rect_filled(
                    badge_rect,
                    egui::Rounding::same(theme::RADIUS_BADGE),
                    b_bg,
                );
                ui.painter().rect_stroke(
                    badge_rect,
                    egui::Rounding::same(theme::RADIUS_BADGE),
                    egui::Stroke::new(1.0, b_border),
                );
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    if st.enabled {
                        format!("STAGE {}", i + 1)
                    } else {
                        "DISABLED".to_owned()
                    },
                    FontId::proportional(10.5),
                    b_color,
                );

                if b_resp.clicked() {
                    stage_set = Some((
                        i,
                        DpiStage {
                            enabled: !st.enabled,
                            code: st.code,
                        },
                    ));
                }

                // Vertical precision slider
                let track_rect = Rect::from_min_size(
                    pos2(card_rect.center().x - 12.0, card_rect.top() + 52.0),
                    vec2(24.0, 180.0),
                );
                if let Some(code) = ui_common::linear_vslider(
                    ui,
                    &format!("dpi_vslider_{i}"),
                    track_rect,
                    st.code as i32,
                    0,
                    163,
                    st.enabled,
                ) {
                    stage_set = Some((
                        i,
                        DpiStage {
                            enabled: st.enabled,
                            code: code as u32,
                        },
                    ));
                }

                // DPI Value display at bottom
                let disp = dpi_code_to_display(st.code);
                ui.painter().text(
                    pos2(card_rect.center().x, card_rect.bottom() - 36.0),
                    Align2::CENTER_TOP,
                    format!("{disp}"),
                    FontId::proportional(15.0),
                    if st.enabled { theme::PAPER } else { theme::ASH },
                );
                ui.painter().text(
                    pos2(card_rect.center().x, card_rect.bottom() - 18.0),
                    Align2::CENTER_TOP,
                    "DPI",
                    FontId::proportional(10.0),
                    theme::ASH,
                );
            }
        });

    if let Some((i, st)) = stage_set {
        app.current_profile_mut().set_dpi_stage(i, st);
    }
    if let Some(i) = cur_set {
        app.current_profile_mut().set_current_dpi_stage(i);
    }
}
