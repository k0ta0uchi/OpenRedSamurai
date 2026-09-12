//! マクロマネージャー modal — Worker 3 (screenshot 7: マクロマネージャー).
//! Left: マクロ名 list + NEW/削除; center: アクション table (状態/ボタン/経過時間MS)
//! + 総経過時間/総アクション + record(●)/stop(■); right: オプション + 編集ツール;
//! bottom: ロードファイル / 保存する / OK / キャンセル.
//!
//! Editing works on a per-frame clone of the selected macro (kept in egui
//! memory); it is written back via `MacroDb::update_macro` on 保存する/OK.

use crate::app::App;
use crate::macro_db::{Macro, MacroAction, MacroDb};
use crate::ui_common::{linear_ui_button_with_id, theme, ButtonKind};
use egui::{vec2, Align2, Context, Frame, Margin, RichText, Stroke, Window};

fn sel_id() -> egui::Id {
    egui::Id::new("macro_mgr_selected")
}
fn row_id() -> egui::Id {
    egui::Id::new("macro_mgr_row")
}
fn clip_id() -> egui::Id {
    egui::Id::new("macro_mgr_clipboard")
}
fn rec_id() -> egui::Id {
    egui::Id::new("macro_mgr_recording")
}
fn prev_id() -> egui::Id {
    egui::Id::new("macro_mgr_prev_vk")
}
fn edit_id() -> egui::Id {
    egui::Id::new("macro_mgr_edit")
}
fn defdelay_id() -> egui::Id {
    egui::Id::new("macro_mgr_use_def_delay")
}

fn action_icon(a: &MacroAction) -> &'static str {
    if a.down {
        "▼"
    } else {
        "▲"
    }
}

pub fn show(app: &mut App, ctx: &Context) {
    // ---- load transient state (all owned; written back at the end) ----
    let names: Vec<String> = app
        .macros
        .names
        .iter()
        .filter(|n| !n.is_empty())
        .cloned()
        .collect();
    let mut sel: usize = ctx
        .memory(|m| m.data.get_temp::<usize>(sel_id()))
        .unwrap_or(0);
    if !names.is_empty() {
        sel = sel.min(names.len() - 1);
    }
    let mut row: Option<usize> = ctx
        .memory(|m| m.data.get_temp::<Option<usize>>(row_id()))
        .flatten();
    let mut recording: bool = ctx
        .memory(|m| m.data.get_temp::<bool>(rec_id()))
        .unwrap_or(false);
    let mut use_def_delay: bool = ctx
        .memory(|m| m.data.get_temp::<bool>(defdelay_id()))
        .unwrap_or(true);

    // Editing snapshot: reload when the selection changed.
    let selected_name: Option<String> = names.get(sel).cloned();
    let mut edit: Option<(String, Macro)> = ctx
        .memory(|m| m.data.get_temp::<Option<(String, Macro)>>(edit_id()))
        .flatten();
    if edit.as_ref().map(|(n, _)| n) != selected_name.as_ref() {
        edit = selected_name
            .clone()
            .and_then(|n| app.macros.macro_by_name(&n).cloned().map(|mac| (n, mac)));
        row = None;
    }

    // ---- recording: poll the keyboard, append down/up pairs ----
    if recording {
        ctx.request_repaint();
        let vk = crate::keycapture::poll_pressed_key();
        let prev: Option<i32> = ctx
            .memory(|m| m.data.get_temp::<Option<i32>>(prev_id()))
            .flatten();
        if vk != prev {
            ctx.memory_mut(|m| m.data.insert_temp(prev_id(), vk));
            if let (Some(v), Some((_, mac))) = (vk, edit.as_mut()) {
                if let Some(usage) = crate::keycapture::vk_to_usage(v) {
                    let delay = if use_def_delay {
                        mac.def_delay_ms.max(0) as u16
                    } else {
                        0
                    };
                    mac.actions.push(MacroAction {
                        down: true,
                        usage,
                        delay_ms: delay,
                    });
                    mac.actions.push(MacroAction {
                        down: false,
                        usage,
                        delay_ms: delay,
                    });
                    row = Some(mac.actions.len() - 1);
                }
            }
        }
    }

    // ---- actions decided by the buttons (applied after render) ----
    enum Cmd {
        New,
        DeleteMacro,
        AddRow,
        DeleteRow,
        MoveUp,
        MoveDown,
        Copy,
        Cut,
        Paste,
        Save,
        Ok,
        Cancel,
        Load,
    }
    let mut cmd: Option<Cmd> = None;

    Window::new("macro_manager_modal")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(vec2(710.0, 450.0))
        .frame(
            Frame::none()
                .fill(theme::CARBON)
                .stroke(Stroke::new(1.0, theme::SMOKE))
                .rounding(theme::ROUNDING_CARD)
                .inner_margin(Margin::symmetric(16.0, 12.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(678.0);
            ui.set_height(426.0);

            // Header row with title and close button
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("マクロマネージャー")
                            .size(15.0)
                            .strong()
                            .color(theme::PAPER),
                    );
                    ui.label(
                        RichText::new("キーストロークの記録と編集")
                            .size(11.0)
                            .color(theme::FOG),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if linear_ui_button_with_id(
                        ui,
                        "macro.close",
                        "✕",
                        ButtonKind::Ghost,
                        vec2(26.0, 24.0),
                    )
                    .clicked()
                    {
                        cmd = Some(Cmd::Cancel);
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let col_height = 270.0;
            // 3-Column main area
            ui.horizontal_top(|ui| {
                // ---- Left Column: Macro Names ----
                ui.allocate_ui_with_layout(
                    vec2(150.0, col_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(150.0);
                        ui.label(
                            RichText::new("マクロ名")
                                .size(11.0)
                                .color(theme::ASH)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (i, name) in names.iter().enumerate() {
                                    let is_sel = sel == i;
                                    let text = RichText::new(name)
                                        .color(if is_sel {
                                            theme::ACID_LIME
                                        } else {
                                            theme::MIST
                                        })
                                        .size(12.0)
                                        .strong();
                                    let response = ui.selectable_label(is_sel, text);
                                    crate::ui_common::a11y::selectable(
                                        &response,
                                        &format!("macro.name.{i}"),
                                        name,
                                        is_sel,
                                    );
                                    if response.clicked() {
                                        sel = i;
                                        row = None;
                                    }
                                }
                                if names.is_empty() {
                                    ui.label(
                                        RichText::new("(マクロなし)").size(11.5).color(theme::ASH),
                                    );
                                }
                            });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if linear_ui_button_with_id(
                                ui,
                                "macro.new",
                                "新規作成",
                                ButtonKind::Highlight,
                                vec2(70.0, 26.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::New);
                            }
                            if linear_ui_button_with_id(
                                ui,
                                "macro.delete",
                                "削除",
                                ButtonKind::Ghost,
                                vec2(70.0, 26.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::DeleteMacro);
                            }
                        });
                    },
                );

                ui.add_sized(vec2(1.0, col_height), egui::Separator::default().vertical());

                // ---- Center Column: Actions Table ----
                ui.allocate_ui_with_layout(
                    vec2(310.0, col_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(310.0);
                        ui.label(
                            RichText::new("アクションシーケンス")
                                .size(11.0)
                                .color(theme::ASH)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        // 固定ヘッダー
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("状態").size(10.5).color(theme::FOG));
                            ui.add_space(12.0);
                            ui.label(RichText::new("キー").size(10.5).color(theme::FOG));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new("遅延(MS)").size(10.5).color(theme::FOG),
                                    );
                                },
                            );
                        });
                        ui.separator();

                        let total_ms: u32 = edit
                            .as_ref()
                            .map(|(_, mac)| mac.actions.iter().map(|a| a.delay_ms as u32).sum())
                            .unwrap_or(0);
                        let total_n = edit.as_ref().map(|(_, mac)| mac.actions.len()).unwrap_or(0);

                        egui::ScrollArea::vertical()
                            .max_height(155.0)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                if let Some((_, mac)) = edit.as_mut() {
                                    for (i, a) in mac.actions.iter().enumerate() {
                                        let is_sel = row == Some(i);
                                        let icon_text = action_icon(a);
                                        let key_str = crate::funcs::usage_label(a.usage);
                                        let label = format!(
                                            "{icon_text}  {key_str:<12} {:>5} ms",
                                            a.delay_ms
                                        );
                                        let text = RichText::new(&label)
                                            .color(if is_sel {
                                                theme::ACID_LIME
                                            } else if a.down {
                                                theme::PAPER
                                            } else {
                                                theme::FOG
                                            })
                                            .size(11.5);
                                        let res = ui.selectable_label(is_sel, text);
                                        crate::ui_common::a11y::selectable(
                                            &res,
                                            &format!("macro.action.{i}"),
                                            &label,
                                            is_sel,
                                        );
                                        if res.clicked() {
                                            row = Some(i);
                                        }
                                        if recording && i + 1 == mac.actions.len() {
                                            res.scroll_to_me(Some(egui::Align::BOTTOM));
                                        }
                                    }
                                }
                            });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("総時間: {total_ms} ms"))
                                    .size(11.0)
                                    .color(theme::FOG),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("アクション数: {total_n}"))
                                    .size(11.0)
                                    .color(theme::FOG),
                            );
                        });

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if recording {
                                if linear_ui_button_with_id(
                                    ui,
                                    "macro.record-stop",
                                    "■ 停止",
                                    ButtonKind::Highlight,
                                    vec2(80.0, 26.0),
                                )
                                .clicked()
                                {
                                    recording = false;
                                }
                                ui.label(
                                    RichText::new("● 記録中…")
                                        .size(12.0)
                                        .color(theme::CORAL_RED)
                                        .strong(),
                                );
                            } else {
                                if linear_ui_button_with_id(
                                    ui,
                                    "macro.record-start",
                                    "● 記録開始",
                                    ButtonKind::Primary,
                                    vec2(90.0, 26.0),
                                )
                                .clicked()
                                {
                                    recording = true;
                                }
                            }
                        });
                    },
                );

                ui.add_sized(vec2(1.0, col_height), egui::Separator::default().vertical());

                // ---- Right Column: Options & Edit Tools ----
                ui.allocate_ui_with_layout(
                    vec2(190.0, col_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(190.0);
                        ui.label(
                            RichText::new("オプション")
                                .size(11.0)
                                .color(theme::ASH)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        if let Some((_, mac)) = edit.as_mut() {
                            let mut rec_delay = mac.delay_type != 0;
                            let rec_resp = ui.checkbox(
                                &mut rec_delay,
                                RichText::new("遅延記録").size(11.5).color(theme::MIST),
                            );
                            crate::ui_common::a11y::checkbox(
                                &rec_resp,
                                "macro.record-delay",
                                "遅延記録",
                                rec_delay,
                            );
                            if rec_resp.changed() {
                                mac.delay_type = if rec_delay { 2 } else { 0 };
                            }
                            ui.add_space(2.0);
                            let def_resp = ui.checkbox(
                                &mut use_def_delay,
                                RichText::new("規定遅延(MS)").size(11.5).color(theme::MIST),
                            );
                            crate::ui_common::a11y::checkbox(
                                &def_resp,
                                "macro.default-delay",
                                "規定遅延(MS)",
                                use_def_delay,
                            );
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("MS:").size(11.5).color(theme::FOG));
                                let delay_resp =
                                    ui.add(egui::DragValue::new(&mut mac.def_delay_ms).speed(1));
                                crate::ui_common::a11y::spinbox(
                                    &delay_resp,
                                    "macro.default-delay-ms",
                                    mac.def_delay_ms as f64,
                                );
                            });
                            ui.add_space(2.0);
                            let mut looping = mac.loop_type != 0;
                            let loop_resp = ui.checkbox(
                                &mut looping,
                                RichText::new("ループ実行").size(11.5).color(theme::MIST),
                            );
                            crate::ui_common::a11y::checkbox(
                                &loop_resp,
                                "macro.loop",
                                "ループ実行",
                                looping,
                            );
                            if loop_resp.changed() {
                                mac.loop_type = if looping { 2 } else { 0 };
                            }
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("回数:").size(11.5).color(theme::FOG));
                                let loop_count_resp =
                                    ui.add(egui::DragValue::new(&mut mac.loop_time).speed(1));
                                crate::ui_common::a11y::spinbox(
                                    &loop_count_resp,
                                    "macro.loop-count",
                                    mac.loop_time as f64,
                                );
                            });
                        } else {
                            ui.label(RichText::new("(マクロを選択)").size(11.5).color(theme::ASH));
                        }

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("編集ツール")
                                .size(11.0)
                                .color(theme::ASH)
                                .strong(),
                        );
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            if linear_ui_button_with_id(
                                ui,
                                "macro.add-row",
                                "追加",
                                ButtonKind::Ghost,
                                vec2(85.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::AddRow);
                            }
                            if linear_ui_button_with_id(
                                ui,
                                "macro.delete-row",
                                "削除",
                                ButtonKind::Ghost,
                                vec2(85.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::DeleteRow);
                            }
                        });
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if linear_ui_button_with_id(
                                ui,
                                "macro.move-up",
                                "上へ",
                                ButtonKind::Ghost,
                                vec2(85.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::MoveUp);
                            }
                            if linear_ui_button_with_id(
                                ui,
                                "macro.move-down",
                                "下へ",
                                ButtonKind::Ghost,
                                vec2(85.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::MoveDown);
                            }
                        });
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if linear_ui_button_with_id(
                                ui,
                                "macro.copy",
                                "コピー",
                                ButtonKind::Ghost,
                                vec2(55.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::Copy);
                            }
                            if linear_ui_button_with_id(
                                ui,
                                "macro.cut",
                                "切取",
                                ButtonKind::Ghost,
                                vec2(55.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::Cut);
                            }
                            if linear_ui_button_with_id(
                                ui,
                                "macro.paste",
                                "貼付",
                                ButtonKind::Ghost,
                                vec2(55.0, 24.0),
                            )
                            .clicked()
                            {
                                cmd = Some(Cmd::Paste);
                            }
                        });
                    },
                );
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Bottom action row: Load File on left, Save / Cancel / OK on right
            ui.horizontal(|ui| {
                if linear_ui_button_with_id(
                    ui,
                    "macro.load",
                    "ロードファイル",
                    ButtonKind::Ghost,
                    vec2(100.0, 28.0),
                )
                .clicked()
                {
                    cmd = Some(Cmd::Load);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if linear_ui_button_with_id(
                        ui,
                        "macro.ok",
                        "OK",
                        ButtonKind::Primary,
                        vec2(80.0, 28.0),
                    )
                    .clicked()
                    {
                        cmd = Some(Cmd::Ok);
                    }
                    ui.add_space(8.0);
                    if linear_ui_button_with_id(
                        ui,
                        "macro.cancel",
                        "キャンセル",
                        ButtonKind::Ghost,
                        vec2(85.0, 28.0),
                    )
                    .clicked()
                    {
                        cmd = Some(Cmd::Cancel);
                    }
                    ui.add_space(8.0);
                    if linear_ui_button_with_id(
                        ui,
                        "macro.save",
                        "保存する",
                        ButtonKind::Highlight,
                        vec2(85.0, 28.0),
                    )
                    .clicked()
                    {
                        cmd = Some(Cmd::Save);
                    }
                });
            });
        });

    // ---- apply commands ----
    let write_back = |app: &mut MacroDb, name: &str, mac: &Macro| {
        app.update_macro(name, mac.clone());
    };
    match cmd {
        Some(Cmd::New) => {
            let mut n = 1;
            while app.macros.names.iter().any(|x| x == &format!("マクロ{n}")) {
                n += 1;
            }
            let name = format!("マクロ{n}");
            app.macros.add_macro(&name);
            if let Some(i) = app
                .macros
                .names
                .iter()
                .filter(|x| !x.is_empty())
                .position(|x| x == &name)
            {
                // selection index is over non-empty names in order.
                let ordered: Vec<String> = app
                    .macros
                    .names
                    .iter()
                    .filter(|x| !x.is_empty())
                    .cloned()
                    .collect();
                sel = ordered.iter().position(|x| x == &name).unwrap_or(i);
            }
            edit = None;
        }
        Some(Cmd::DeleteMacro) => {
            if let Some(name) = selected_name {
                let _ = app.macros.delete_macro(&name);
            }
            edit = None;
            row = None;
        }
        Some(Cmd::AddRow) => {
            if let Some((_, mac)) = edit.as_mut() {
                let delay = if use_def_delay {
                    mac.def_delay_ms.max(0) as u16
                } else {
                    0
                };
                // Default to 'A' (0x04); recording is the precise path.
                for down in [true, false] {
                    mac.actions.push(MacroAction {
                        down,
                        usage: 0x04,
                        delay_ms: delay,
                    });
                }
                row = Some(mac.actions.len() - 1);
            }
        }
        Some(Cmd::DeleteRow) => {
            if let (Some(i), Some((_, mac))) = (row, edit.as_mut()) {
                if i < mac.actions.len() {
                    mac.actions.remove(i);
                    row = if mac.actions.is_empty() {
                        None
                    } else {
                        Some(i.min(mac.actions.len() - 1))
                    };
                }
            }
        }
        Some(Cmd::MoveUp) => {
            if let (Some(i), Some((_, mac))) = (row, edit.as_mut()) {
                if i > 0 && i < mac.actions.len() {
                    mac.actions.swap(i - 1, i);
                    row = Some(i - 1);
                }
            }
        }
        Some(Cmd::MoveDown) => {
            if let (Some(i), Some((_, mac))) = (row, edit.as_mut()) {
                if i + 1 < mac.actions.len() {
                    mac.actions.swap(i, i + 1);
                    row = Some(i + 1);
                }
            }
        }
        Some(Cmd::Copy) => {
            if let (Some(i), Some((_, mac))) = (row, edit.as_ref()) {
                if let Some(a) = mac.actions.get(i).copied() {
                    ctx.memory_mut(|m| m.data.insert_temp(clip_id(), vec![a]));
                }
            }
        }
        Some(Cmd::Cut) => {
            if let (Some(i), Some((_, mac))) = (row, edit.as_mut()) {
                if i < mac.actions.len() {
                    let a = mac.actions.remove(i);
                    ctx.memory_mut(|m| m.data.insert_temp(clip_id(), vec![a]));
                    row = if mac.actions.is_empty() {
                        None
                    } else {
                        Some(i.min(mac.actions.len() - 1))
                    };
                }
            }
        }
        Some(Cmd::Paste) => {
            if let Some(clip) = ctx.memory(|m| m.data.get_temp::<Vec<MacroAction>>(clip_id())) {
                if let Some((_, mac)) = edit.as_mut() {
                    let at = row.map(|i| (i + 1).min(mac.actions.len()));
                    let paste_len = clip.len();
                    match at {
                        Some(i) => {
                            for (k, a) in clip.iter().enumerate() {
                                mac.actions.insert(i + k, *a);
                            }
                            row = Some(i + paste_len.saturating_sub(1));
                        }
                        None => {
                            let old_len = mac.actions.len();
                            mac.actions.extend(clip);
                            row = Some(old_len + paste_len.saturating_sub(1));
                        }
                    }
                }
            }
        }
        Some(Cmd::Save) => {
            if let Some((name, mac)) = edit.clone() {
                write_back(&mut app.macros, &name, &mac);
            }
            if app.macros.save().is_ok() {
                app.toast(ctx, "保存しました");
            } else {
                app.toast(ctx, "保存に失敗しました");
            }
        }
        Some(Cmd::Ok) => {
            if let Some((name, mac)) = edit.clone() {
                write_back(&mut app.macros, &name, &mac);
            }
            let ok = app.macros.save().is_ok();
            app.macro_open = false;
            recording = false;
            app.toast(
                ctx,
                if ok {
                    "保存しました"
                } else {
                    "保存に失敗しました"
                },
            );
        }
        Some(Cmd::Cancel) => {
            app.macro_open = false;
            recording = false;
        }
        Some(Cmd::Load) => {
            if let Some(dir) = app.macros.dir.clone() {
                app.macros = MacroDb::load(&dir);
                app.toast(ctx, "読み込みました");
            }
            edit = None;
            row = None;
        }
        None => {}
    }

    // ---- write transient state back ----
    ctx.memory_mut(|m| {
        m.data.insert_temp(sel_id(), sel);
        m.data.insert_temp(row_id(), row);
        m.data.insert_temp(rec_id(), recording);
        m.data.insert_temp(defdelay_id(), use_def_delay);
        m.data.insert_temp(edit_id(), edit);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_show_render_safe() {
        let ctx = egui::Context::default();
        let mut app = App {
            assets: crate::assets::Assets::load(&ctx),
            profiles: vec![crate::profile::Profile::default_profile(1)],
            current: 0,
            tab: crate::app::Tab::General,
            side: false,
            status: None,
            menu: crate::menu::MenuState::default(),
            menu_button: 1,
            dblclick: crate::app::DblClickTest::default(),
            custom_color_open: false,
            temp_rgb: [255, 0, 0],
            dialog: crate::ui_dialog::DialogKind::None,
            macro_open: true,
            macros: MacroDb::default(),
        };

        let _ = ctx.run(Default::default(), |ctx| {
            show(&mut app, ctx);
        });
    }
}
