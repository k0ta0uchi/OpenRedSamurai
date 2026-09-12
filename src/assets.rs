//! Project-owned image assets embedded into the executable.
//!
//! The rest of the interface is painted procedurally. Keeping this list small
//! prevents unused upstream skin resources from being carried into releases.

pub type Tex = egui::TextureHandle;

#[derive(Clone)]
pub struct Assets {
    pub mouse_front: Tex,
    pub mouse_side: Tex,
    pub mouse_color: Tex,
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
        }
    }
}
