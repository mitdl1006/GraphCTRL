//! The viewport background: the renderer's offscreen target shown as an image.

use gc_render::ViewTarget;
use gc_ui::colors;

/// Owns the offscreen render target and its registration with egui.
pub struct Background {
    render_state: Option<egui_wgpu::RenderState>,
    target: Option<(ViewTarget, egui::TextureId)>,
}

impl Background {
    pub(crate) fn new(render_state: Option<egui_wgpu::RenderState>) -> Self {
        Self { render_state, target: None }
    }

    /// True when a GPU renders the background.
    pub fn has_gpu(&self) -> bool {
        self.render_state.is_some()
    }

    /// The current offscreen target, if one was rendered.
    pub fn target(&self) -> Option<(&egui_wgpu::RenderState, &ViewTarget)> {
        Some((self.render_state.as_ref()?, &self.target.as_ref()?.0))
    }

    pub(crate) fn paint(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(rs) = self.render_state.as_ref() else {
            ui.painter().rect_filled(rect, 0.0, colors::BACKGROUND_BOTTOM);
            return;
        };
        let ppp = ui.pixels_per_point();
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to a valid texture size"
        )]
        let px = |v: f32| ((v * ppp).round().clamp(1.0, 8192.0)) as u32;
        let size = (px(rect.width()), px(rect.height()));
        if self.target.as_ref().is_none_or(|(t, _)| t.size() != size) {
            let target = ViewTarget::new(&rs.device, size.0, size.1);
            let mut renderer = rs.renderer.write();
            let id = match self.target.take() {
                Some((_, id)) => {
                    renderer.update_egui_texture_from_wgpu_texture(
                        &rs.device,
                        target.resolve_view(),
                        wgpu::FilterMode::Nearest,
                        id,
                    );
                    id
                }
                None => renderer.register_native_texture(
                    &rs.device,
                    target.resolve_view(),
                    wgpu::FilterMode::Nearest,
                ),
            };
            drop(renderer);
            let c = colors::BACKGROUND_TOP;
            let linear = |v: u8| f64::from(v) / 255.0;
            gc_render::clear(
                &rs.device,
                &rs.queue,
                &target,
                [linear(c.r()), linear(c.g()), linear(c.b()), 1.0],
            );
            self.target = Some((target, id));
        }
        if let Some((_, id)) = self.target.as_ref() {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(*id, rect, uv, egui::Color32::WHITE);
        }
    }
}
