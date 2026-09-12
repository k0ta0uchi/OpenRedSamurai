//! Info tab — version/model/copyright lines.
//! Restyled per Linear Design System (DESIGN.md): Midnight precision instrument.

use crate::app::App;
use crate::ui_common::{self, geo, theme, ButtonKind};

use egui::{pos2, vec2, Align2, FontId, Rect};

/// (label, value) pairs formatted per Linear specification.
const DETAILS: [(&str, &str); 7] = [
    ("バージョン", "V1.0.0"),
    ("プロダクト", "RED SAMURAI 16400DPI Gaming Mouse"),
    ("モデル番号", "HKW-GMMS01-BK/1"),
    ("コピーライト", "COPYRIGHT (C) 2021 FSC Co., Ltd."),
    ("権利表記", "ALL RIGHT RESERVED"),
    ("公式サイト", "http://www.e-fsc.jp/index.html"),
    ("サポート", "mail:support@e-fsc.jp"),
];

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut update_clicked = false;
    egui::Area::new(egui::Id::new("info_tab"))
        .fixed_pos(pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let card_rect =
                Rect::from_center_size(pos2(400.0, geo::CARD_Y + 145.0), vec2(480.0, 290.0));
            ui_common::paint_card(ui, card_rect, theme::CARBON, theme::GRAPHITE);

            // Header identity inside the card
            ui.painter().text(
                pos2(card_rect.left() + 24.0, card_rect.top() + 24.0),
                Align2::LEFT_TOP,
                "SYSTEM INFORMATION",
                FontId::proportional(11.0),
                theme::MIST,
            );

            for (i, (lbl, val)) in DETAILS.iter().enumerate() {
                let y = card_rect.top() + 58.0 + i as f32 * 30.0;

                // Subtle horizontal separator between rows
                if i > 0 {
                    ui.painter().line_segment(
                        [
                            pos2(card_rect.left() + 24.0, y - 6.0),
                            pos2(card_rect.right() - 24.0, y - 6.0),
                        ],
                        egui::Stroke::new(0.5, theme::GRAPHITE),
                    );
                }

                ui.painter().text(
                    pos2(card_rect.left() + 24.0, y),
                    Align2::LEFT_TOP,
                    *lbl,
                    FontId::proportional(12.0),
                    theme::ASH,
                );

                let value = if i == 0 {
                    format!("V{}", crate::installer::CURRENT_VERSION)
                } else {
                    (*val).to_owned()
                };
                ui.painter().text(
                    pos2(card_rect.right() - 24.0, y),
                    Align2::RIGHT_TOP,
                    value,
                    FontId::proportional(12.0),
                    theme::PAPER,
                );
            }

            let update_rect = Rect::from_min_size(
                pos2(card_rect.left() + 24.0, card_rect.bottom() - 34.0),
                vec2(166.0, 26.0),
            );
            let response = ui_common::linear_button(
                ui,
                "info.update",
                update_rect,
                "更新を確認",
                ButtonKind::Ghost,
            );
            update_clicked = response.clicked();
        });
    if update_clicked {
        match crate::installer::launch_installer(&["--update"]) {
            Ok(()) => app.toast(ctx, "更新確認画面を開きました"),
            Err(error) => app.toast(ctx, format!("更新画面を開けません: {error}")),
        }
    }
}
