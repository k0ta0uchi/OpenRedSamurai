//! Assignment context-menu overlay — CONTRACT in CONTRACT_funcs.md §menu (Worker 2).
//!
//! Generic widget: draws the menu tree, returns the picked action (if any).
//! No dependency on `crate::app` — the caller (app.rs) applies the action.

use crate::funcs::MenuAction;
use crate::funcs::MenuItem;
use crate::ui_common::theme;
use egui::{pos2, Align2, Color32, FontId, Id, Pos2, Rect, Sense, Vec2};

/// Row size; every row stretches to this.
const ROW_W: f32 = 200.0;
const ROW_H: f32 = 28.0;
const TEXT_PAD: f32 = 12.0;
const FONT_SIZE: f32 = 12.5;
const SUBMENU_ARROW: &str = "›";

/// Open-menu state owned by the app.
#[derive(Debug, Clone)]
pub struct MenuState {
    pub open: bool,
    pub pos: egui::Pos2,
    pub items: Vec<MenuItem>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            open: false,
            pos: egui::Pos2::ZERO,
            items: Vec::new(),
        }
    }
}

/// Draw the overlay for this frame. Returns Some(action) when the user picked a leaf.
/// Closes (returns closed=true) when the user clicks elsewhere.
///
/// Note: takes `&mut Ui` (instead of the `&Ui` sketched in the contract) because
/// egui 0.29 can only register interactive widgets through `&mut Ui` — needed so
/// rows receive hover/clicks instead of the widgets behind the overlay.
pub fn draw(ui: &mut egui::Ui, state: &mut MenuState) -> (Option<MenuAction>, bool) {
    // Transient state (which submenu is open) persists across frames via egui memory.
    let open_sub_id = Id::new("assignment_menu_open_sub");
    let pass_stamp_id = Id::new("assignment_menu_pass_stamp");
    let pass = ui.ctx().cumulative_pass_nr();
    let prev_pass: Option<u64> = ui.memory(|m| m.data.get_temp(pass_stamp_id));
    // First frame after (re)opening: ignore outside clicks so the click that
    // opened the menu cannot instantly close it again.
    let just_opened = prev_pass != Some(pass.wrapping_sub(1));
    let mut open_sub: Option<usize> = if just_opened {
        None
    } else {
        ui.memory(|m| m.data.get_temp::<Option<usize>>(open_sub_id))
            .flatten()
    };

    // ---- geometry: one procedural row per item, clamped to screen ----
    let screen = ui.ctx().screen_rect();
    let column = |origin: Pos2, count: usize| -> Vec<Rect> {
        let x = origin
            .x
            .clamp(screen.left(), (screen.right() - ROW_W).max(screen.left()));
        let height = ROW_H * count as f32;
        let y = origin
            .y
            .clamp(screen.top(), (screen.bottom() - height).max(screen.top()));
        (0..count)
            .map(|i| {
                Rect::from_min_size(
                    pos2(x, y) + Vec2::new(0.0, ROW_H * i as f32),
                    Vec2::new(ROW_W, ROW_H),
                )
            })
            .collect()
    };

    let top_rects = column(state.pos, state.items.len());

    // ---- interaction: allocate top rows first to resolve active submenu ----
    let top_rows: Vec<egui::Response> = top_rects
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let response = ui.allocate_rect(*r, Sense::click());
            if let Some(item) = state.items.get(i) {
                crate::ui_common::a11y::button(
                    &response,
                    &format!("assignment-menu.item.{i}"),
                    &item.label,
                );
            }
            response
        })
        .collect();

    // Hovering a parent in the top list opens its submenu; hovering a leaf closes it.
    // If pointer is outside top rows (e.g. moving into the submenu), keep the existing open_sub.
    if let Some(i) = top_rows.iter().position(|r| r.contains_pointer()) {
        open_sub = state
            .items
            .get(i)
            .filter(|it| !it.children.is_empty())
            .map(|_| i);
    }

    // Now calculate sub_rects and sub_rows using the newly confirmed open_sub
    let sub_rects: Vec<Rect> = match open_sub {
        Some(i) if i < state.items.len() && !state.items[i].children.is_empty() => {
            let parent = top_rects[i];
            // Submenus open to the right, flipping left near the right screen edge.
            let x = if parent.right() + ROW_W <= screen.right() {
                parent.right()
            } else {
                parent.left() - ROW_W
            };
            column(pos2(x, parent.top()), state.items[i].children.len())
        }
        _ => {
            open_sub = None;
            Vec::new()
        }
    };

    let sub_rows: Vec<egui::Response> = sub_rects
        .iter()
        .enumerate()
        .map(|(j, r)| {
            let response = ui.allocate_rect(*r, Sense::click());
            if let Some(i) = open_sub {
                if let Some(child) = state.items.get(i).and_then(|item| item.children.get(j)) {
                    crate::ui_common::a11y::button(
                        &response,
                        &format!("assignment-menu.item.{i}.child.{j}"),
                        &child.label,
                    );
                }
            }
            response
        })
        .collect();

    // Left-click on a leaf returns its action.
    let mut picked: Option<MenuAction> = None;
    if let Some(i) = top_rows.iter().position(|r| r.clicked()) {
        picked = state.items.get(i).and_then(|it| it.action.clone());
    } else if let Some(j) = sub_rows.iter().position(|r| r.clicked()) {
        if let Some(i) = open_sub {
            picked = state
                .items
                .get(i)
                .and_then(|it| it.children.get(j))
                .and_then(|ch| ch.action.clone());
        }
    }

    let mut closed = picked.is_some();
    if !closed && !just_opened && ui.input(|i| i.pointer.any_click()) {
        // The click landed outside every visible row → dismiss.
        closed = !top_rows
            .iter()
            .chain(sub_rows.iter())
            .any(|r| r.contains_pointer());
    }

    // ---- painting: procedural Linear popup menu (Carbon card, Smoke border, Obsidian hover) ----
    let font = FontId::proportional(FONT_SIZE);
    let arrow_font = FontId::proportional(FONT_SIZE - 1.0);

    // Draw top-level container background & border
    if let (Some(first), Some(last)) = (top_rects.first(), top_rects.last()) {
        let container_rect = Rect::from_min_max(
            pos2(first.left() - 2.0, first.top() - 3.0),
            pos2(first.right() + 2.0, last.bottom() + 3.0),
        );
        // Shadow/glow
        ui.painter().rect_filled(
            container_rect.expand(2.0),
            10.0,
            Color32::from_black_alpha(120),
        );
        ui.painter().rect(
            container_rect,
            8.0,
            theme::CARBON,
            egui::Stroke::new(1.0, theme::SMOKE),
        );
    }

    for (i, r) in top_rects.iter().enumerate() {
        let hot = top_rows[i].hovered() || top_rows[i].contains_pointer() || open_sub == Some(i);
        if hot {
            let row_bg = r.shrink2(egui::vec2(4.0, 1.5));
            ui.painter()
                .rect_filled(row_bg, 4.0, Color32::from_rgb(0x23, 0x25, 0x2b));
        }
        let text_color = if hot { theme::PAPER } else { theme::MIST };
        ui.painter().text(
            pos2(r.left() + TEXT_PAD, r.center().y),
            Align2::LEFT_CENTER,
            &state.items[i].label,
            font.clone(),
            text_color,
        );
        if !state.items[i].children.is_empty() {
            let arrow_color = if hot { theme::PAPER } else { theme::ASH };
            ui.painter().text(
                pos2(r.right() - TEXT_PAD, r.center().y),
                Align2::RIGHT_CENTER,
                SUBMENU_ARROW,
                arrow_font.clone(),
                arrow_color,
            );
        }
    }

    // Draw sub-level container background & border
    if let Some(i) = open_sub {
        if let (Some(first), Some(last)) = (sub_rects.first(), sub_rects.last()) {
            let container_rect = Rect::from_min_max(
                pos2(first.left() - 2.0, first.top() - 3.0),
                pos2(first.right() + 2.0, last.bottom() + 3.0),
            );
            ui.painter().rect_filled(
                container_rect.expand(2.0),
                10.0,
                Color32::from_black_alpha(120),
            );
            ui.painter().rect(
                container_rect,
                8.0,
                theme::CARBON,
                egui::Stroke::new(1.0, theme::SMOKE),
            );
        }

        if let Some(item) = state.items.get(i) {
            for (j, r) in sub_rects.iter().enumerate() {
                if let Some(child) = item.children.get(j) {
                    let hot = sub_rows
                        .get(j)
                        .map_or(false, |resp| resp.hovered() || resp.contains_pointer());
                    if hot {
                        let row_bg = r.shrink2(egui::vec2(4.0, 1.5));
                        ui.painter()
                            .rect_filled(row_bg, 4.0, Color32::from_rgb(0x23, 0x25, 0x2b));
                    }
                    let text_color = if hot { theme::PAPER } else { theme::MIST };
                    ui.painter().text(
                        pos2(r.left() + TEXT_PAD, r.center().y),
                        Align2::LEFT_CENTER,
                        &child.label,
                        font.clone(),
                        text_color,
                    );
                }
            }
        }
    }

    ui.memory_mut(|m| {
        m.data.insert_temp(open_sub_id, open_sub);
        m.data.insert_temp(pass_stamp_id, pass);
    });
    if closed {
        // Reset transient state so the next open starts fresh.
        ui.memory_mut(|m| {
            m.data.remove::<Option<usize>>(open_sub_id);
            m.data.remove::<u64>(pass_stamp_id);
        });
    }

    (picked, closed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::funcs::build_menu;

    #[test]
    fn test_menu_draw_safe_with_submenus() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut state = MenuState {
                    open: true,
                    pos: egui::pos2(100.0, 100.0),
                    items: build_menu(&[]),
                };

                // 1. Initial draw (no submenu open)
                let (picked1, closed1) = draw(ui, &mut state);
                assert!(picked1.is_none());
                assert!(!closed1);

                // 2. Simulate opening "シングルキー" submenu (index 5)
                let open_sub_id = Id::new("assignment_menu_open_sub");
                ui.memory_mut(|m| m.data.insert_temp(open_sub_id, Some(5usize)));

                let (picked2, closed2) = draw(ui, &mut state);
                assert!(picked2.is_none());
                assert!(!closed2);

                // 3. Simulate switching immediately from single key (len 13) to combo key (len 1)
                ui.memory_mut(|m| m.data.insert_temp(open_sub_id, Some(6usize)));
                let (picked3, closed3) = draw(ui, &mut state);
                assert!(picked3.is_none());
                assert!(!closed3);
            });
        });
    }
}
