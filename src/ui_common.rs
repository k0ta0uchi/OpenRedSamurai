//! Shared widgets & geometry — CONTRACT in CONTRACT_ui.md (implemented by Worker 3).

pub type Tex = egui::TextureHandle;

/// Accessibility metadata shared by the custom-painted controls.
///
/// Egui derives an AccessKit node from [`egui::Response::widget_info`], but
/// custom-painted controls do not get a node automatically.  These helpers keep
/// the semantic role, accessible name, and stable `AuthorId` in one place.  The
/// author id is also the Windows UI Automation `AutomationId`, so UI tests can
/// select a control without relying on its screen coordinates or translated text.
pub mod a11y {

    /// Stable prefix reserved for controls in the RED SAMURAI configuration UI.
    pub const PREFIX: &str = "red-samurai";

    /// Normalize an id into the namespace owned by this application.
    pub fn stable_id(id: &str) -> String {
        if id == PREFIX || id.starts_with("red-samurai.") {
            id.to_owned()
        } else {
            format!("{PREFIX}.{id}")
        }
    }

    /// Add the stable id to the accessible name as a fallback for clients that
    /// expose names but not AccessKit's `AuthorId` property.
    pub fn name(id: &str, label: &str) -> String {
        if label.is_empty() {
            format!("{id} [a11y-id: {id}]")
        } else {
            format!("{label} [a11y-id: {id}]")
        }
    }

    /// Register a button with an id derived from egui's persistent widget id.
    /// Callers should prefer [`button`] with a semantic id when one is known.
    pub fn auto_button(response: &egui::Response, label: &str) {
        let id = format!("{PREFIX}.button.{:016x}", response.id.value());
        button(response, &id, label);
    }

    fn author_id(response: &egui::Response, id: &str) {
        // `accesskit_node_builder` is available when the crate's default
        // `accesskit` feature is enabled.  Keep this function a no-op for
        // consumers that deliberately disable that feature.
        #[cfg(feature = "accesskit")]
        {
            response.ctx.accesskit_node_builder(response.id, |builder| {
                builder.set_author_id(id.to_owned());
            });
        }
        #[cfg(not(feature = "accesskit"))]
        let _ = (response, id);
    }

    /// Register a push button (including a tab, profile pill, or menu row).
    pub fn button(response: &egui::Response, id: &str, label: &str) {
        let id = stable_id(id);
        let label = name(&id, label);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label.clone())
        });
        author_id(response, &id);
    }

    /// Register a toggle control and expose its current state.
    pub fn checkbox(response: &egui::Response, id: &str, label: &str, selected: bool) {
        let id = stable_id(id);
        let label = name(&id, label);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                response.enabled(),
                selected,
                label.clone(),
            )
        });
        author_id(response, &id);
    }

    /// Register a slider and expose the current numeric value.
    pub fn slider(response: &egui::Response, id: &str, label: &str, value: f64) {
        let id = stable_id(id);
        let label = name(&id, label);
        response.widget_info(|| egui::WidgetInfo::slider(response.enabled(), value, label.clone()));
        author_id(response, &id);
    }

    /// Register a selectable row while preserving its selected state.
    pub fn selectable(response: &egui::Response, id: &str, label: &str, selected: bool) {
        let id = stable_id(id);
        let label = name(&id, label);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                response.enabled(),
                selected,
                label.clone(),
            )
        });
        author_id(response, &id);
    }

    /// Register a radio option and expose its selected state.
    pub fn radio(response: &egui::Response, id: &str, label: &str, selected: bool) {
        let id = stable_id(id);
        let label = name(&id, label);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                response.enabled(),
                selected,
                label.clone(),
            )
        });
        author_id(response, &id);
    }

    /// Register a numeric spin button such as `egui::DragValue`.
    pub fn spinbox(response: &egui::Response, id: &str, value: f64) {
        let id = stable_id(id);
        response.widget_info(|| egui::WidgetInfo::drag_value(response.enabled(), value));
        author_id(response, &id);
    }

    /// Register a custom control that does not fit one of the built-in roles.
    pub fn other(response: &egui::Response, id: &str, label: &str) {
        let id = stable_id(id);
        let label = name(&id, label);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, response.enabled(), label.clone())
        });
        author_id(response, &id);
    }
}

/// Linear Design System tokens (from DESIGN.md).
pub mod theme {
    use egui::Color32;

    pub const VOID: Color32 = Color32::from_rgb(8, 9, 10);
    pub const CARBON: Color32 = Color32::from_rgb(15, 16, 17);
    pub const OBSIDIAN: Color32 = Color32::from_rgb(22, 23, 24);
    pub const GRAPHITE: Color32 = Color32::from_rgb(35, 37, 42);
    pub const SMOKE: Color32 = Color32::from_rgb(56, 59, 63);
    pub const ASH: Color32 = Color32::from_rgb(98, 102, 109);
    pub const FOG: Color32 = Color32::from_rgb(138, 143, 152);
    pub const MIST: Color32 = Color32::from_rgb(208, 214, 224);
    pub const BONE: Color32 = Color32::from_rgb(229, 229, 230);
    pub const PAPER: Color32 = Color32::from_rgb(255, 255, 255);

    pub const ACID_LIME: Color32 = Color32::from_rgb(228, 242, 34);
    pub const PULSE_GREEN: Color32 = Color32::from_rgb(39, 166, 68);
    pub const CORAL_RED: Color32 = Color32::from_rgb(235, 87, 87);
    pub const SIGNAL_TEAL: Color32 = Color32::from_rgb(2, 184, 204);
    pub const IRIS_VIOLET: Color32 = Color32::from_rgb(99, 102, 241);

    pub const RADIUS_CARD: f32 = 12.0;
    pub const RADIUS_BUTTON: f32 = 6.0;
    pub const RADIUS_INPUT: f32 = 6.0;
    pub const RADIUS_BADGE: f32 = 4.0;
    pub const RADIUS_PILL: f32 = 9999.0;

    pub const ROUNDING_CARD: f32 = RADIUS_CARD;
    pub const ROUNDING_BUTTON: f32 = RADIUS_BUTTON;
    pub const ROUNDING_INPUT: f32 = RADIUS_INPUT;
}

/// Shared geometry constants (800x638 window), measured from the reference screenshots.
pub mod geo {
    use egui::pos2;
    use egui::Rect;

    pub const WINDOW: [f32; 2] = [800.0, 638.0];
    /// Main navigation starts 62px below the title bar; the previous 106px
    /// gap made the editor feel detached from its content.
    pub const TAB_Y: f32 = 112.0;
    pub const TAB_X0: f32 = 75.0;
    pub const TAB_PITCH: f32 = 102.0;
    pub const TAB_SIZE: [f32; 2] = [85.0, 21.0];

    /// Shared content-card geometry for the compact editor layout.
    pub const CARD_Y: f32 = 145.0;
    pub const CARD_H: f32 = 313.0;

    pub const MIN_BTN: Rect = Rect {
        min: pos2(743.0, 30.0),
        max: pos2(765.0, 48.0),
    };
    pub const CLOSE_BTN: Rect = Rect {
        min: pos2(766.0, 30.0),
        max: pos2(788.0, 48.0),
    };

    pub const PROFILE_Y: f32 = 478.0;
    pub const PROFILE_X0: f32 = 105.0;
    pub const PROFILE_PITCH: f32 = 123.0;
    pub const PROFILE_SIZE: [f32; 2] = [109.0, 20.0];

    /// Vertically center the action row inside the footer panel (550..638).
    pub const BOTTOM_Y: f32 = 580.0;
    pub const BOTTOM_H: f32 = 27.0;
    pub const BOTTOM_GAP: f32 = 10.0;
    /// (label, x, width) for 保存/ロードファイル/既定/すべてリセット/OK/キャンセル/適用.
    pub const BOTTOM_BUTTONS: [(&str, f32, f32); 7] = [
        ("保存", 88.0, 66.0),
        ("ロードファイル", 164.0, 95.0),
        ("既定", 269.0, 62.0),
        ("すべてリセット", 341.0, 90.0),
        ("OK", 441.0, 88.0),
        ("キャンセル", 539.0, 75.0),
        ("適用", 624.0, 62.0),
    ];

    pub const MOUSE_FRONT_RECT: Rect = Rect {
        min: pos2(100.0, 155.0),
        max: pos2(300.0, 400.0),
    };
    pub const MOUSE_SIDE_RECT: Rect = Rect {
        min: pos2(100.0, 155.0),
        max: pos2(300.0, 400.0),
    };

    pub const ROWS_X: f32 = 330.0;
    pub const ROW_W: f32 = 173.0;
    pub const ROW_H: f32 = 23.0;
    pub const ROW_PITCH_FRONT: f32 = 34.0;
    pub const ROW_PITCH_SIDE: f32 = 24.0;

    pub const SLIDER_X: f32 = 520.0;
    pub const SLIDER_W: f32 = 210.0;
    pub const SLIDER_H: f32 = 19.0;

    pub const DPI_COL_X: [f32; 5] = [172.0, 305.0, 437.0, 570.0, 702.0];
    pub const DPI_TRACK_Y: f32 = 255.0;
    pub const DPI_TRACK_SIZE: [f32; 2] = [22.0, 182.0];
}

/// Full UV range for `Painter::image` (paint the whole texture).
fn uv_full() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
}

fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

/// Paint a texture stretched to fill `rect` (full UV range).
pub fn paint_tex(ui: &egui::Ui, tex: &Tex, rect: egui::Rect, tint: egui::Color32) {
    ui.painter().image(tex.id(), rect, uv_full(), tint);
}

/// Contain-fit an image of `size` (pixels) into `rect`, centered.
pub fn fit_rect(rect: egui::Rect, size: [f32; 2]) -> egui::Rect {
    let (w, h) = (size[0].max(1.0), size[1].max(1.0));
    let scale = (rect.width() / w).min(rect.height() / h);
    egui::Rect::from_center_size(rect.center(), egui::vec2(w * scale, h * scale))
}

/// Textured button with centered label. `selected` adds a red border.
pub fn image_button(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    tex: &Tex,
    label: &str,
    selected: bool,
    text_color: egui::Color32,
) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click());

    paint_tex(ui, tex, rect, egui::Color32::WHITE);
    if response.hovered() {
        // Hover brightening: translucent white overlay over the texture.
        ui.painter().rect_filled(
            rect,
            egui::Rounding::same(2.0),
            egui::Color32::from_white_alpha(40),
        );
    }
    if !label.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label.to_owned(),
            font(12.0),
            text_color,
        );
    }
    if selected {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            egui::Rounding::same(2.0),
            egui::Stroke::new(2.0, egui::Color32::RED),
        );
    }
    a11y::button(&response, id, label);
    response
}

/// 13x13 checkbox + label. Square border, red "V" when checked.
/// `checked` is mutated in place on click.
pub fn checkbox(
    ui: &egui::Ui,
    id: &str,
    pos: egui::Pos2,
    label: &str,
    checked: &mut bool,
) -> egui::Response {
    let box_rect = egui::Rect::from_min_size(pos, egui::vec2(13.0, 13.0));
    let painter = ui.painter();

    // Square: white fill + dark gray border.
    painter.rect_filled(box_rect, egui::Rounding::same(0.0), egui::Color32::WHITE);
    painter.rect_stroke(
        box_rect,
        egui::Rounding::same(0.0),
        egui::Stroke::new(1.0, egui::Color32::from_gray(96)),
    );

    // Red "V" glyph when checked.
    if *checked {
        painter.text(
            egui::pos2(box_rect.center().x, box_rect.center().y + 0.5),
            egui::Align2::CENTER_CENTER,
            "V".to_owned(),
            font(10.0),
            egui::Color32::RED,
        );
    }

    // Label to the right of the box.
    let mut hit_rect = box_rect;
    if !label.is_empty() {
        let text_rect = painter.text(
            egui::pos2(box_rect.right() + 6.0, pos.y + 1.0),
            egui::Align2::LEFT_TOP,
            label.to_owned(),
            font(12.0),
            egui::Color32::WHITE,
        );
        hit_rect = hit_rect.union(text_rect);
    }

    let response = ui.interact(hit_rect, egui::Id::new(id), egui::Sense::click());
    if response.clicked() {
        *checked = !*checked;
    }
    a11y::checkbox(&response, id, label, *checked);
    response
}

/// Thumb texture size in px (bar1.png is 15x15).
const H_THUMB: f32 = 15.0;
/// Vertical slider thumb height in px.
const V_THUMB_H: f32 = 12.0;

/// Horizontal slider (track texture + thumb texture). Returns Some(new_value) when changed.
pub fn hslider_int(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    track: &Tex,
    thumb: &Tex,
    value: i32,
    min: i32,
    max: i32,
) -> Option<i32> {
    paint_tex(ui, track, rect, egui::Color32::WHITE);

    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click_and_drag());
    let span = (max - min).max(1);
    // Thumb center travels this range so the 15px thumb stays inside the track.
    let usable = (rect.width() - H_THUMB).max(1.0);

    let mut new_value = None;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.dragged() || response.clicked() {
            let t = ((pos.x - rect.left() - H_THUMB * 0.5) / usable).clamp(0.0, 1.0);
            let v = min + (span as f32 * t).round() as i32;
            if v != value {
                new_value = Some(v);
            }
        }
    }

    // Thumb position for the currently shown value.
    let shown = new_value.unwrap_or(value).clamp(min, max);
    let t = (shown - min) as f32 / span as f32;
    let cx = rect.left() + H_THUMB * 0.5 + t * usable;
    let thumb_rect = egui::Rect::from_center_size(
        egui::pos2(cx, rect.center().y),
        egui::vec2(H_THUMB, H_THUMB),
    );
    paint_tex(ui, thumb, thumb_rect, egui::Color32::WHITE);
    a11y::slider(&response, id, id, shown as f64);

    new_value
}

/// Vertical slider inside `rect` (DPI track). Returns Some(new_code) when changed.
/// Top of the track = max, bottom = min.
pub fn vslider_int(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    track: &Tex,
    value: i32,
    min: i32,
    max: i32,
) -> Option<i32> {
    paint_tex(ui, track, rect, egui::Color32::WHITE);

    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click_and_drag());
    let span = (max - min).max(1);
    let usable = (rect.height() - V_THUMB_H).max(1.0);

    let mut new_value = None;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.dragged() || response.clicked() {
            // Top of the track = max, bottom = min.
            let t = ((pos.y - rect.top() - V_THUMB_H * 0.5) / usable).clamp(0.0, 1.0);
            let v = max - (span as f32 * t).round() as i32;
            if v != value {
                new_value = Some(v);
            }
        }
    }

    // Thumb: filled dark-red rect with bright red border.
    let shown = new_value.unwrap_or(value).clamp(min, max);
    let t = (max - shown) as f32 / span as f32; // 0 = top
    let cy = rect.top() + V_THUMB_H * 0.5 + t * usable;
    let thumb_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, cy),
        egui::vec2((rect.width() - 4.0).max(6.0), V_THUMB_H),
    );
    let painter = ui.painter();
    painter.rect_filled(
        thumb_rect,
        egui::Rounding::same(2.0),
        egui::Color32::from_rgb(110, 16, 16),
    );
    painter.rect_stroke(
        thumb_rect,
        egui::Rounding::same(2.0),
        egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 48, 48)),
    );
    a11y::slider(&response, id, id, shown as f64);

    new_value
}

/// White 12px label helper used everywhere (text drawn at pos, top-left).
pub fn label(ui: &egui::Ui, pos: egui::Pos2, text: &str, color: egui::Color32, size: f32) {
    ui.painter().text(
        pos,
        egui::Align2::LEFT_TOP,
        text.to_owned(),
        font(size),
        color,
    );
}

// -----------------------------------------------------------------------------
// Linear Design System Modern Components
// -----------------------------------------------------------------------------

/// Paint a precision card container with 12px radius and 1px border.
pub fn paint_card(ui: &egui::Ui, rect: egui::Rect, bg: egui::Color32, border: egui::Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(theme::RADIUS_CARD), bg);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(theme::RADIUS_CARD),
        egui::Stroke::new(1.0, border),
    );
}

/// Paint a 6px radius inner panel or card.
pub fn paint_panel(ui: &egui::Ui, rect: egui::Rect, bg: egui::Color32, border: egui::Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(theme::RADIUS_BUTTON), bg);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(theme::RADIUS_BUTTON),
        egui::Stroke::new(1.0, border),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// High-emphasis action button (#e4f222 Acid Lime fill, #08090a text, 6px radius)
    Primary,
    /// Ghost / Outline button (transparent or subtle bg, 1px Graphite border, Mist text)
    Ghost,
    /// High-contrast button (Bone/Paper fill, Void text, 6px radius)
    Highlight,
    /// Pill button (rounded 9999px)
    Pill { active: bool },
}

/// Precision Linear button.
pub fn linear_button(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    label_text: &str,
    kind: ButtonKind,
) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click());
    let hovered = response.hovered();
    let pressed = response.is_pointer_button_down_on();

    let (bg, border, text_color, radius) = match kind {
        ButtonKind::Primary => {
            let bg = if pressed {
                egui::Color32::from_rgb(205, 218, 30)
            } else if hovered {
                egui::Color32::from_rgb(240, 255, 45)
            } else {
                theme::ACID_LIME
            };
            (
                bg,
                egui::Color32::TRANSPARENT,
                theme::VOID,
                theme::RADIUS_BUTTON,
            )
        }
        ButtonKind::Ghost => {
            let bg = if pressed {
                theme::OBSIDIAN
            } else if hovered {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            } else {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 4)
            };
            let border = if hovered {
                theme::SMOKE
            } else {
                theme::GRAPHITE
            };
            let text_color = if hovered { theme::PAPER } else { theme::MIST };
            (bg, border, text_color, theme::RADIUS_BUTTON)
        }
        ButtonKind::Highlight => {
            let bg = if pressed {
                theme::MIST
            } else if hovered {
                theme::PAPER
            } else {
                theme::BONE
            };
            (
                bg,
                egui::Color32::TRANSPARENT,
                theme::VOID,
                theme::RADIUS_BUTTON,
            )
        }
        ButtonKind::Pill { active } => {
            let bg = if active {
                theme::OBSIDIAN
            } else if hovered {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            } else {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 6)
            };
            let border = if active {
                theme::SMOKE
            } else {
                theme::GRAPHITE
            };
            let text_color = if active {
                theme::PAPER
            } else if hovered {
                theme::MIST
            } else {
                theme::FOG
            };
            (bg, border, text_color, theme::RADIUS_PILL)
        }
    };

    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(radius), bg);
    if border != egui::Color32::TRANSPARENT {
        painter.rect_stroke(
            rect,
            egui::Rounding::same(radius),
            egui::Stroke::new(1.0, border),
        );
    }

    if !label_text.is_empty() {
        let size = if matches!(kind, ButtonKind::Primary | ButtonKind::Highlight) {
            13.0
        } else {
            12.0
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label_text,
            font(size),
            text_color,
        );
    }

    a11y::button(&response, id, label_text);
    response
}

/// Precision Linear button for use within an auto-layout Ui.
pub fn linear_ui_button(
    ui: &mut egui::Ui,
    label_text: &str,
    kind: ButtonKind,
    min_size: egui::Vec2,
) -> egui::Response {
    linear_ui_button_impl(ui, label_text, kind, min_size, None)
}

/// Precision Linear button with an explicit accessibility id.
pub fn linear_ui_button_with_id(
    ui: &mut egui::Ui,
    id: &str,
    label_text: &str,
    kind: ButtonKind,
    min_size: egui::Vec2,
) -> egui::Response {
    linear_ui_button_impl(ui, label_text, kind, min_size, Some(id))
}

fn linear_ui_button_impl(
    ui: &mut egui::Ui,
    label_text: &str,
    kind: ButtonKind,
    min_size: egui::Vec2,
    a11y_id: Option<&str>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(min_size, egui::Sense::click());
    let hovered = response.hovered();
    let pressed = response.is_pointer_button_down_on();

    let (bg, border, text_color, radius) = match kind {
        ButtonKind::Primary => {
            let bg = if pressed {
                egui::Color32::from_rgb(205, 218, 30)
            } else if hovered {
                egui::Color32::from_rgb(240, 255, 45)
            } else {
                theme::ACID_LIME
            };
            (
                bg,
                egui::Color32::TRANSPARENT,
                theme::VOID,
                theme::RADIUS_BUTTON,
            )
        }
        ButtonKind::Ghost => {
            let bg = if pressed {
                theme::OBSIDIAN
            } else if hovered {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            } else {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 4)
            };
            let border = if hovered {
                theme::SMOKE
            } else {
                theme::GRAPHITE
            };
            let text_color = if hovered { theme::PAPER } else { theme::MIST };
            (bg, border, text_color, theme::RADIUS_BUTTON)
        }
        ButtonKind::Highlight => {
            let bg = if pressed {
                theme::MIST
            } else if hovered {
                theme::PAPER
            } else {
                theme::BONE
            };
            (
                bg,
                egui::Color32::TRANSPARENT,
                theme::VOID,
                theme::RADIUS_BUTTON,
            )
        }
        ButtonKind::Pill { active } => {
            let bg = if active {
                theme::OBSIDIAN
            } else if hovered {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12)
            } else {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 6)
            };
            let border = if active {
                theme::SMOKE
            } else {
                theme::GRAPHITE
            };
            let text_color = if active {
                theme::PAPER
            } else if hovered {
                theme::MIST
            } else {
                theme::FOG
            };
            (bg, border, text_color, theme::RADIUS_PILL)
        }
    };

    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(radius), bg);
    if border != egui::Color32::TRANSPARENT {
        painter.rect_stroke(
            rect,
            egui::Rounding::same(radius),
            egui::Stroke::new(1.0, border),
        );
    }

    if !label_text.is_empty() {
        let size = if matches!(kind, ButtonKind::Primary | ButtonKind::Highlight) {
            13.0
        } else {
            12.0
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label_text,
            font(size),
            text_color,
        );
    }

    match a11y_id {
        Some(id) => a11y::button(&response, id, label_text),
        None => a11y::auto_button(&response, label_text),
    }
    response
}

/// Linear Navigation Tab Item with electric indicator.
pub fn linear_tab(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    name: &str,
    active: bool,
) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click());
    let hovered = response.hovered();

    let painter = ui.painter();
    if active {
        // Active tab: subtle elevated surface + Acid Lime indicator line at the bottom
        painter.rect_filled(
            rect,
            egui::Rounding::same(theme::RADIUS_BUTTON),
            theme::OBSIDIAN,
        );
        painter.rect_stroke(
            rect,
            egui::Rounding::same(theme::RADIUS_BUTTON),
            egui::Stroke::new(1.0, theme::SMOKE),
        );
        let indicator_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 12.0, rect.bottom() - 2.5),
            egui::pos2(rect.right() - 12.0, rect.bottom() - 0.5),
        );
        painter.rect_filled(indicator_rect, egui::Rounding::same(1.0), theme::ACID_LIME);
    } else if hovered {
        painter.rect_filled(
            rect,
            egui::Rounding::same(theme::RADIUS_BUTTON),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
    }

    let text_color = if active {
        theme::PAPER
    } else if hovered {
        theme::MIST
    } else {
        theme::FOG
    };

    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        name,
        font(13.0),
        text_color,
    );
    a11y::button(&response, id, name);

    response
}

/// Linear Profile Selector Pill.
pub fn linear_profile_button(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    label_text: &str,
    active: bool,
) -> egui::Response {
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click());
    let hovered = response.hovered();

    let painter = ui.painter();
    let bg = if active {
        theme::OBSIDIAN
    } else if hovered {
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10)
    } else {
        theme::CARBON
    };
    let border = if active {
        theme::SMOKE
    } else if hovered {
        theme::SMOKE
    } else {
        theme::GRAPHITE
    };

    painter.rect_filled(rect, egui::Rounding::same(theme::RADIUS_BUTTON), bg);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(theme::RADIUS_BUTTON),
        egui::Stroke::new(1.0, border),
    );

    // Active status dot (Acid Lime).  Keep the label in a dedicated text
    // column so the dot never collides with the first Japanese glyph.
    if active {
        let dot_pos = egui::pos2(rect.left() + 10.0, rect.center().y);
        painter.circle_filled(dot_pos, 2.5, theme::ACID_LIME);
    }

    let text_color = if active {
        theme::PAPER
    } else if hovered {
        theme::MIST
    } else {
        theme::FOG
    };

    let text_x = if active {
        rect.left() + 20.0
    } else {
        rect.center().x
    };
    painter.text(
        egui::pos2(text_x, rect.center().y),
        if active {
            egui::Align2::LEFT_CENTER
        } else {
            egui::Align2::CENTER_CENTER
        },
        label_text,
        font(if active { 11.5 } else { 12.0 }),
        text_color,
    );
    a11y::button(&response, id, label_text);

    response
}

/// Linear Modern Checkbox.
pub fn linear_checkbox(
    ui: &egui::Ui,
    id: &str,
    pos: egui::Pos2,
    label_text: &str,
    checked: &mut bool,
) -> egui::Response {
    let box_rect = egui::Rect::from_min_size(pos, egui::vec2(14.0, 14.0));
    let mut hit_rect = box_rect;

    let painter = ui.painter();
    if !label_text.is_empty() {
        let text_rect = painter.text(
            egui::pos2(box_rect.right() + 8.0, pos.y),
            egui::Align2::LEFT_TOP,
            label_text.to_owned(),
            font(12.0),
            if *checked { theme::PAPER } else { theme::FOG },
        );
        hit_rect = hit_rect.union(text_rect);
    }

    let response = ui.interact(hit_rect, egui::Id::new(id), egui::Sense::click());
    let hovered = response.hovered();

    if response.clicked() {
        *checked = !*checked;
    }

    let (bg, border) = if *checked {
        (theme::ACID_LIME, theme::ACID_LIME)
    } else if hovered {
        (theme::OBSIDIAN, theme::SMOKE)
    } else {
        (theme::CARBON, theme::GRAPHITE)
    };

    painter.rect_filled(box_rect, egui::Rounding::same(theme::RADIUS_BADGE), bg);
    painter.rect_stroke(
        box_rect,
        egui::Rounding::same(theme::RADIUS_BADGE),
        egui::Stroke::new(1.0, border),
    );

    if *checked {
        // Draw checkmark glyph
        painter.text(
            egui::pos2(box_rect.center().x, box_rect.center().y - 0.5),
            egui::Align2::CENTER_CENTER,
            "✓",
            font(11.0),
            theme::VOID,
        );
    }
    a11y::checkbox(&response, id, label_text, *checked);

    response
}

/// Linear Precision Horizontal Slider (sleek 4px track, glowing fill, circular thumb).
pub fn linear_hslider(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    value: i32,
    min: i32,
    max: i32,
) -> Option<i32> {
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click_and_drag());
    let span = (max - min).max(1);
    let thumb_radius = 6.0;
    let usable = (rect.width() - thumb_radius * 2.0).max(1.0);

    let mut new_value = None;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.dragged() || response.clicked() {
            let t = ((pos.x - rect.left() - thumb_radius) / usable).clamp(0.0, 1.0);
            let v = min + (span as f32 * t).round() as i32;
            if v != value {
                new_value = Some(v);
            }
        }
    }

    let shown = new_value.unwrap_or(value).clamp(min, max);
    let t = (shown - min) as f32 / span as f32;
    let cx = rect.left() + thumb_radius + t * usable;
    let cy = rect.center().y;

    let painter = ui.painter();
    // Track background (Graphite 4px height)
    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 4.0));
    painter.rect_filled(track_rect, egui::Rounding::same(2.0), theme::GRAPHITE);

    // Active fill up to thumb
    if cx > rect.left() {
        let active_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), track_rect.top()),
            egui::pos2(cx, track_rect.bottom()),
        );
        painter.rect_filled(active_rect, egui::Rounding::same(2.0), theme::MIST);
    }

    // Circular thumb with subtle shadow
    let thumb_pos = egui::pos2(cx, cy);
    let hovered = response.hovered() || response.dragged();
    painter.circle_filled(
        thumb_pos,
        if hovered { 7.0 } else { thumb_radius },
        theme::PAPER,
    );
    painter.circle_stroke(
        thumb_pos,
        if hovered { 7.0 } else { thumb_radius },
        egui::Stroke::new(1.0, theme::SMOKE),
    );
    a11y::slider(&response, id, id, shown as f64);

    new_value
}

/// Linear Precision Vertical Slider (for DPI tab).
pub fn linear_vslider(
    ui: &egui::Ui,
    id: &str,
    rect: egui::Rect,
    value: i32,
    min: i32,
    max: i32,
    active: bool,
) -> Option<i32> {
    let response = ui.interact(rect, egui::Id::new(id), egui::Sense::click_and_drag());
    let span = (max - min).max(1);
    let thumb_radius = 7.0;
    let usable = (rect.height() - thumb_radius * 2.0).max(1.0);

    let mut new_value = None;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.dragged() || response.clicked() {
            let t = ((pos.y - rect.top() - thumb_radius) / usable).clamp(0.0, 1.0);
            let v = max - (span as f32 * t).round() as i32; // Top is max
            if v != value {
                new_value = Some(v);
            }
        }
    }

    let shown = new_value.unwrap_or(value).clamp(min, max);
    let t = (max - shown) as f32 / span as f32; // 0 at top
    let cy = rect.top() + thumb_radius + t * usable;
    let cx = rect.center().x;

    let painter = ui.painter();
    // Track background
    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(4.0, rect.height()));
    painter.rect_filled(track_rect, egui::Rounding::same(2.0), theme::GRAPHITE);

    // Active fill from bottom up to thumb
    if active && cy < rect.bottom() {
        let active_rect = egui::Rect::from_min_max(
            egui::pos2(track_rect.left(), cy),
            egui::pos2(track_rect.right(), rect.bottom()),
        );
        painter.rect_filled(active_rect, egui::Rounding::same(2.0), theme::ACID_LIME);
    }

    // Thumb
    let thumb_pos = egui::pos2(cx, cy);
    let hovered = response.hovered() || response.dragged();
    painter.circle_filled(
        thumb_pos,
        if hovered { 8.0 } else { thumb_radius },
        if active { theme::PAPER } else { theme::ASH },
    );
    painter.circle_stroke(
        thumb_pos,
        if hovered { 8.0 } else { thumb_radius },
        egui::Stroke::new(1.0, theme::SMOKE),
    );
    a11y::slider(&response, id, id, shown as f64);

    new_value
}

#[cfg(test)]
mod tests {
    use super::{a11y, geo};

    #[test]
    fn accessibility_ids_are_namespaced_and_stable() {
        assert_eq!(
            a11y::stable_id("dialog.fire.ok"),
            "red-samurai.dialog.fire.ok"
        );
        assert_eq!(
            a11y::stable_id("red-samurai.dialog.fire.ok"),
            "red-samurai.dialog.fire.ok"
        );
        assert_eq!(
            a11y::name("red-samurai.dialog.fire.ok", "OK"),
            "OK [a11y-id: red-samurai.dialog.fire.ok]"
        );
    }

    #[test]
    fn empty_labels_still_expose_the_identifier() {
        assert_eq!(
            a11y::name("red-samurai.titlebar.close", ""),
            "red-samurai.titlebar.close [a11y-id: red-samurai.titlebar.close]"
        );
    }

    #[test]
    fn compact_layout_keeps_bottom_actions_on_one_baseline() {
        let buttons = geo::BOTTOM_BUTTONS;
        for pair in buttons.windows(2) {
            let (_, left_x, left_w) = pair[0];
            let (_, right_x, _) = pair[1];
            assert!((right_x - (left_x + left_w) - geo::BOTTOM_GAP).abs() < f32::EPSILON);
        }
        assert!(geo::CARD_Y > geo::TAB_Y + geo::TAB_SIZE[1]);
        assert!(geo::PROFILE_Y >= geo::CARD_Y + geo::CARD_H);
    }
}
