use crate::app::App;
use crate::profile::FireTarget;
use crate::ui_common::{linear_ui_button_with_id, theme, ButtonKind};
use egui::{vec2, Align2, Color32, Context, Frame, Margin, RichText, Stroke, Window};

/// Which modal is open (owned by App).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum DialogKind {
    #[default]
    None,
    /// シングルキー: captures one key.
    SingleKey,
    /// コンボキー: modifiers + one key.
    ComboKey,
    /// ファイア設定 (連打機能)
    FireKey,
}

pub enum DialogResult {
    SingleKey(u8),
    ComboKey(u8, u8),
    FireKey {
        target: FireTarget,
        times: u8,
        delay_ms: u16,
    },
}

/// egui-memory keys for SingleKey / ComboKey.
fn usage_key() -> egui::Id {
    egui::Id::new("assign_dialog_captured_usage")
}
fn mods_key() -> egui::Id {
    egui::Id::new("assign_dialog_captured_mods")
}

fn take_capture(ctx: &Context) -> (Option<u8>, u8) {
    (
        ctx.memory(|m| m.data.get_temp::<u8>(usage_key())),
        ctx.memory(|m| m.data.get_temp::<u8>(mods_key()))
            .unwrap_or(0),
    )
}

fn clear_capture(ctx: &Context) {
    ctx.memory_mut(|m| {
        m.data.remove::<u8>(usage_key());
        m.data.remove::<u8>(mods_key());
    });
}

// Memory keys for FireKey
fn fire_mode_id() -> egui::Id {
    egui::Id::new("fire_dialog_mode") // 0=L, 1=R, 2=M, 3=Keyboard
}
fn fire_key_id() -> egui::Id {
    egui::Id::new("fire_dialog_key")
}
fn fire_times_id() -> egui::Id {
    egui::Id::new("fire_dialog_times")
}
fn fire_delay_id() -> egui::Id {
    egui::Id::new("fire_dialog_delay")
}

/// Dispatch modal dialog rendering.
pub fn show(app: &mut App, ctx: &Context) -> Option<DialogResult> {
    match app.dialog {
        DialogKind::None => None,
        DialogKind::SingleKey | DialogKind::ComboKey => show_key_dialog(app, ctx),
        DialogKind::FireKey => show_fire_dialog(app, ctx),
    }
}

fn show_key_dialog(app: &mut App, ctx: &Context) -> Option<DialogResult> {
    let is_combo = app.dialog == DialogKind::ComboKey;
    let title = if is_combo {
        "コンボキー設定"
    } else {
        "シングルキー設定"
    };
    let subtitle = if is_combo {
        "修飾キー (Ctrl / Alt / Shift / Win) + 割り当てるキーを押してください"
    } else {
        "キーボード上で割り当てたいキーを押してください"
    };

    // Poll the physical keyboard every frame while open.
    if let Some(vk) = crate::keycapture::poll_pressed_key() {
        if let Some(usage) = crate::keycapture::vk_to_usage(vk) {
            ctx.memory_mut(|m| {
                m.data.insert_temp(usage_key(), usage);
                if is_combo {
                    m.data
                        .insert_temp(mods_key(), crate::keycapture::modifier_mask());
                }
            });
        }
    }

    let (captured, mods) = take_capture(ctx);
    let value_text = match captured {
        Some(u) => {
            let key = crate::funcs::usage_label(u);
            if is_combo {
                format!("{}{}", crate::keycapture::modifier_prefix(mods), key)
            } else {
                key
            }
        }
        None => "キーを押してください…".to_owned(),
    };

    let mut ok = false;
    let mut cancel = false;

    // Semi-transparent backdrop overlay
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("modal_backdrop"),
    ))
    .rect_filled(screen, 0.0, Color32::from_black_alpha(160));

    Window::new("assign_modal")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        // The backdrop lives on the middle layer; keep the dialog above it so
        // the modal itself remains fully legible.
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(
            Frame::none()
                .fill(theme::CARBON)
                .stroke(Stroke::new(1.0, theme::SMOKE))
                .rounding(theme::ROUNDING_CARD)
                .inner_margin(Margin::same(24.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(440.0);

            // Header row with title and close button
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(16.0).strong().color(theme::PAPER));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if linear_ui_button_with_id(
                        ui,
                        "dialog.key.close",
                        "✕",
                        ButtonKind::Ghost,
                        vec2(28.0, 26.0),
                    )
                    .clicked()
                    {
                        cancel = true;
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(RichText::new(subtitle).size(12.0).color(theme::FOG));

            ui.add_space(14.0);

            // Captured key input box
            let box_stroke = if captured.is_some() {
                Stroke::new(1.5, theme::ACID_LIME)
            } else {
                Stroke::new(1.0, theme::SMOKE)
            };

            Frame::none()
                .fill(theme::OBSIDIAN)
                .stroke(box_stroke)
                .rounding(theme::ROUNDING_BUTTON)
                .inner_margin(Margin::symmetric(16.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(390.0);
                    ui.set_height(42.0);
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        let text_color = if captured.is_some() {
                            theme::PAPER
                        } else {
                            theme::ASH
                        };
                        ui.label(
                            RichText::new(value_text)
                                .size(18.0)
                                .color(text_color)
                                .strong(),
                        );
                    });
                });

            ui.add_space(16.0);

            // Bottom action buttons: Cancel & OK
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ok_style = if captured.is_some() {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Ghost
                };
                if linear_ui_button_with_id(ui, "dialog.key.ok", "OK", ok_style, vec2(90.0, 32.0))
                    .clicked()
                    && captured.is_some()
                {
                    ok = true;
                }
                ui.add_space(8.0);
                if linear_ui_button_with_id(
                    ui,
                    "dialog.key.cancel",
                    "キャンセル",
                    ButtonKind::Ghost,
                    vec2(90.0, 32.0),
                )
                .clicked()
                {
                    cancel = true;
                }
            });

            if captured.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                ok = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

    if cancel {
        app.dialog = DialogKind::None;
        clear_capture(ctx);
        return None;
    }
    if ok {
        if let Some(usage) = captured {
            app.dialog = DialogKind::None;
            clear_capture(ctx);
            return Some(if is_combo {
                DialogResult::ComboKey(usage, mods)
            } else {
                DialogResult::SingleKey(usage)
            });
        }
    }
    None
}

fn show_fire_dialog(app: &mut App, ctx: &Context) -> Option<DialogResult> {
    let mut mode: usize = ctx
        .memory(|m| m.data.get_temp::<usize>(fire_mode_id()))
        .unwrap_or(0);
    let mut key_usage: Option<u8> = ctx
        .memory(|m| m.data.get_temp::<Option<u8>>(fire_key_id()))
        .flatten();
    let mut times: u8 = ctx
        .memory(|m| m.data.get_temp::<u8>(fire_times_id()))
        .unwrap_or(3);
    let mut delay_ms: u16 = ctx
        .memory(|m| m.data.get_temp::<u16>(fire_delay_id()))
        .unwrap_or(0);

    // If keyboard mode (3), poll key capture
    if mode == 3 {
        if let Some(vk) = crate::keycapture::poll_pressed_key() {
            if let Some(usage) = crate::keycapture::vk_to_usage(vk) {
                key_usage = Some(usage);
                ctx.memory_mut(|m| m.data.insert_temp(fire_key_id(), Some(usage)));
            }
        }
    }

    let mut ok = false;
    let mut cancel = false;

    // Semi-transparent backdrop overlay
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("modal_backdrop"),
    ))
    .rect_filled(screen, 0.0, Color32::from_black_alpha(160));

    Window::new("fire_modal")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(vec2(460.0, 310.0))
        .frame(
            Frame::none()
                .fill(theme::CARBON)
                .stroke(Stroke::new(1.0, theme::SMOKE))
                .rounding(theme::ROUNDING_CARD)
                .inner_margin(Margin::same(20.0)),
        )
        .show(ctx, |ui| {
            ui.set_width(420.0);

            // Header
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("ファイア設定")
                            .size(16.0)
                            .strong()
                            .color(theme::PAPER),
                    );
                    ui.label(
                        RichText::new("ボタン連打機能の割り当て")
                            .size(11.5)
                            .color(theme::FOG),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if linear_ui_button_with_id(
                        ui,
                        "dialog.fire.close",
                        "✕",
                        ButtonKind::Ghost,
                        vec2(26.0, 24.0),
                    )
                    .clicked()
                    {
                        cancel = true;
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(10.0);

            // 2 Columns: Type (left) and Options (right)
            ui.horizontal_top(|ui| {
                // Left Column: Type selection
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    ui.label(
                        RichText::new("連打対象 (タイプ)")
                            .size(11.0)
                            .color(theme::ASH)
                            .strong(),
                    );
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        let resp = ui.radio_value(
                            &mut mode,
                            0,
                            RichText::new("L (左)").color(theme::PAPER).size(12.0),
                        );
                        crate::ui_common::a11y::radio(
                            &resp,
                            "dialog.fire.target-left",
                            "L (左)",
                            mode == 0,
                        );
                        ui.add_space(6.0);
                        let resp = ui.radio_value(
                            &mut mode,
                            1,
                            RichText::new("R (右)").color(theme::PAPER).size(12.0),
                        );
                        crate::ui_common::a11y::radio(
                            &resp,
                            "dialog.fire.target-right",
                            "R (右)",
                            mode == 1,
                        );
                        ui.add_space(6.0);
                        let resp = ui.radio_value(
                            &mut mode,
                            2,
                            RichText::new("M (中)").color(theme::PAPER).size(12.0),
                        );
                        crate::ui_common::a11y::radio(
                            &resp,
                            "dialog.fire.target-middle",
                            "M (中)",
                            mode == 2,
                        );
                    });

                    ui.add_space(8.0);
                    let resp = ui.radio_value(
                        &mut mode,
                        3,
                        RichText::new("キーボードキー:")
                            .color(theme::PAPER)
                            .size(12.0),
                    );
                    crate::ui_common::a11y::radio(
                        &resp,
                        "dialog.fire.target-keyboard",
                        "キーボードキー",
                        mode == 3,
                    );
                    ui.add_space(4.0);

                    // Key capture box
                    let is_active = mode == 3;
                    let box_stroke = if is_active && key_usage.is_some() {
                        Stroke::new(1.5, theme::ACID_LIME)
                    } else if is_active {
                        Stroke::new(1.0, theme::CORAL_RED)
                    } else {
                        Stroke::new(1.0, theme::SMOKE)
                    };
                    let key_label = if let Some(u) = key_usage {
                        crate::funcs::usage_label(u)
                    } else if is_active {
                        "キーを入力…".to_string()
                    } else {
                        "---".to_string()
                    };

                    Frame::none()
                        .fill(theme::OBSIDIAN)
                        .stroke(box_stroke)
                        .rounding(theme::ROUNDING_BUTTON)
                        .inner_margin(Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(200.0);
                            ui.set_height(30.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(key_label)
                                        .size(14.0)
                                        .color(if is_active { theme::PAPER } else { theme::ASH })
                                        .strong(),
                                );
                            });
                        });
                });

                ui.add_sized(vec2(1.0, 130.0), egui::Separator::default().vertical());

                // Right Column: Parameters (Fire times & Delay ms)
                ui.vertical(|ui| {
                    ui.set_width(170.0);
                    ui.label(
                        RichText::new("オプション")
                            .size(11.0)
                            .color(theme::ASH)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("ファイア(時間):")
                                .size(11.5)
                                .color(theme::FOG),
                        );
                        let resp = ui.add(egui::DragValue::new(&mut times).range(1..=255));
                        crate::ui_common::a11y::spinbox(&resp, "dialog.fire.times", times as f64);
                    });
                    ui.label(
                        RichText::new("(連打回数 1〜255)")
                            .size(10.0)
                            .color(theme::ASH),
                    );

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("遅延(ms):").size(11.5).color(theme::FOG));
                        let resp =
                            ui.add(egui::DragValue::new(&mut delay_ms).range(0..=1000).speed(5));
                        crate::ui_common::a11y::spinbox(
                            &resp,
                            "dialog.fire.delay-ms",
                            delay_ms as f64,
                        );
                    });
                    ui.label(
                        RichText::new("(連打間隔ms 0〜1000)")
                            .size(10.0)
                            .color(theme::ASH),
                    );
                });
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Bottom Buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_ok = mode != 3 || key_usage.is_some();
                let ok_style = if can_ok {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Ghost
                };
                if linear_ui_button_with_id(ui, "dialog.fire.ok", "OK", ok_style, vec2(85.0, 30.0))
                    .clicked()
                    && can_ok
                {
                    ok = true;
                }
                ui.add_space(8.0);
                if linear_ui_button_with_id(
                    ui,
                    "dialog.fire.cancel",
                    "キャンセル",
                    ButtonKind::Ghost,
                    vec2(85.0, 30.0),
                )
                .clicked()
                {
                    cancel = true;
                }
            });

            if (mode != 3 || key_usage.is_some()) && ctx.input(|i| i.key_pressed(egui::Key::Enter))
            {
                ok = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

    ctx.memory_mut(|m| {
        m.data.insert_temp(fire_mode_id(), mode);
        m.data.insert_temp(fire_key_id(), key_usage);
        m.data.insert_temp(fire_times_id(), times);
        m.data.insert_temp(fire_delay_id(), delay_ms);
    });

    if cancel {
        app.dialog = DialogKind::None;
        return None;
    }
    if ok {
        let target = match mode {
            0 => FireTarget::MouseLeft,
            1 => FireTarget::MouseRight,
            2 => FireTarget::MouseMiddle,
            3 => FireTarget::Keyboard(key_usage.unwrap_or(0x04)),
            _ => FireTarget::MouseLeft,
        };
        app.dialog = DialogKind::None;
        return Some(DialogResult::FireKey {
            target,
            times,
            delay_ms,
        });
    }
    None
}
