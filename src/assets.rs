//! Embedded skin textures — CONTRACT in CONTRACT_ui.md (implemented by Worker 3).

pub type Tex = egui::TextureHandle;

#[derive(Clone)]
pub struct Assets {
    pub bg_main: Tex,
    pub mouse_front: Tex,
    pub mouse_side: Tex,
    pub mouse_color: Tex,
    pub row_n: Tex,
    pub row_sel: Tex,
    pub profile_n: Tex,
    pub profile_sel: Tex,
    pub btn_n: Tex,
    pub btn_d: Tex,
    pub tab_a: Tex,
    pub tab_n: Tex,
    pub track_h: Tex,
    pub track_h_small: Tex,
    pub thumb: Tex,
    pub track_v: Tex,
    pub radio_off: Tex,
    pub radio_on: Tex,
    pub dpi_label_off: Tex,
    pub dpi_label_on: Tex,
    pub test_area: Tex,
    pub menu_item_n: Tex,
    pub menu_item_h: Tex,
}

/// Decode an embedded PNG and upload it as a linear-filtered texture.
fn tex(ctx: &egui::Context, name: &str, png: &[u8]) -> Tex {
    let img = image::load_from_memory(png).expect("embedded skin PNG failed to decode");
    let size = [img.width() as usize, img.height() as usize];
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(size, &img.into_rgba8().into_raw()),
        egui::TextureOptions::LINEAR,
    )
}

impl Assets {
    pub fn load(ctx: &egui::Context) -> Self {
        Self {
            bg_main: tex(
                ctx,
                "bg_main",
                include_bytes!("../assets/images/0409/background/cfgMainback.png"),
            ),
            mouse_front: tex(
                ctx,
                "mouse_front",
                include_bytes!("../assets/images/0409/background/mouse-layout-front.png"),
            ),
            mouse_side: tex(
                ctx,
                "mouse_side",
                include_bytes!("../assets/images/0409/background/mouse-layout-side.png"),
            ),
            mouse_color: tex(
                ctx,
                "mouse_color",
                include_bytes!("../assets/images/0409/background/mouse-layout-Color.png"),
            ),
            row_n: tex(
                ctx,
                "row_n",
                include_bytes!("../assets/images/0409/buttons/ButtonAssign-1.png"),
            ),
            row_sel: tex(
                ctx,
                "row_sel",
                include_bytes!("../assets/images/0409/buttons/ButtonAssign-2.png"),
            ),
            profile_n: tex(
                ctx,
                "profile_n",
                include_bytes!("../assets/images/0409/buttons/ProfileButton-1.png"),
            ),
            profile_sel: tex(
                ctx,
                "profile_sel",
                include_bytes!("../assets/images/0409/buttons/ProfileButton-3.png"),
            ),
            btn_n: tex(
                ctx,
                "btn_n",
                include_bytes!("../assets/images/0409/background/main-ok-btn-n.png"),
            ),
            btn_d: tex(
                ctx,
                "btn_d",
                include_bytes!("../assets/images/0409/background/main-ok-btn-d.png"),
            ),
            tab_a: tex(
                ctx,
                "tab_a",
                include_bytes!("../assets/images/0409/background/tab-a.png"),
            ),
            tab_n: tex(
                ctx,
                "tab_n",
                include_bytes!("../assets/images/0409/background/tab-n.png"),
            ),
            track_h: tex(
                ctx,
                "track_h",
                include_bytes!("../assets/images/0409/background/trackbar-back-speed.png"),
            ),
            track_h_small: tex(
                ctx,
                "track_h_small",
                include_bytes!("../assets/images/0409/background/trackbar-back-speed-dbclick.png"),
            ),
            thumb: tex(
                ctx,
                "thumb",
                include_bytes!("../assets/images/0409/background/bar1.png"),
            ),
            track_v: tex(
                ctx,
                "track_v",
                include_bytes!("../assets/images/0409/background/DPI-TRACK-R.png"),
            ),
            radio_off: tex(
                ctx,
                "radio_off",
                include_bytes!("../assets/images/0409/background/radio1.png"),
            ),
            radio_on: tex(
                ctx,
                "radio_on",
                include_bytes!("../assets/images/0409/background/radio2.png"),
            ),
            dpi_label_off: tex(
                ctx,
                "dpi_label_off",
                include_bytes!("../assets/images/0409/background/DPI-1.png"),
            ),
            dpi_label_on: tex(
                ctx,
                "dpi_label_on",
                include_bytes!("../assets/images/0409/background/DPI-2.png"),
            ),
            test_area: tex(
                ctx,
                "test_area",
                include_bytes!("../assets/images/0409/background/DB-CLICK-TEST-AREA.png"),
            ),
            menu_item_n: tex(
                ctx,
                "menu_item_n",
                include_bytes!("../assets/images/0409/background/funcmenu_1.png"),
            ),
            menu_item_h: tex(
                ctx,
                "menu_item_h",
                include_bytes!("../assets/images/0409/background/funcmenu_2.png"),
            ),
        }
    }
}
