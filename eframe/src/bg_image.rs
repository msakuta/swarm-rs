//! A widget for background image.

use egui::{Color32, Painter, Pos2, Rect, Response, TextureOptions, Vec2};
use swarm_rs::AppData;

pub(crate) struct BgImage {
    texture: Option<egui::TextureHandle>,
}

impl BgImage {
    pub fn new() -> Self {
        Self { texture: None }
    }

    pub fn clear(&mut self) {
        self.texture.take();
    }

    pub fn paint(
        &mut self,
        response: &Response,
        painter: &Painter,
        app_data: &AppData,
        img_getter: impl Fn(&AppData) -> egui::ColorImage,
    ) {
        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            let image = img_getter(app_data);
            // Load the texture only once.
            painter.ctx().load_texture(
                "my-image",
                image,
                TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                },
            )
        });

        let to_screen = egui::emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        let size = texture.size_vec2() * app_data.scale as f32;
        let min =
            Vec2::new(app_data.origin[0] as f32, app_data.origin[1] as f32) * app_data.scale as f32;
        let max = min + size;
        let rect = Rect {
            min: min.to_pos2(),
            max: max.to_pos2(),
        };
        const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(
            texture.id(),
            to_screen.transform_rect(rect),
            UV,
            Color32::WHITE,
        );
    }
}
