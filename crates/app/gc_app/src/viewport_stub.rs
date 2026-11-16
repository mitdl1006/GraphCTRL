//! Stage 0 placeholder for the 3D viewport: clears an offscreen target and shows it as an image.
//!
//! This proves the embedding path (own render target -> `register_native_texture` -> egui image) and
//! keeps working without a GPU, in which case a flat placeholder is painted instead.

use eframe::{egui, egui_wgpu, wgpu};
use gc_render::ViewTarget;

pub(crate) struct ViewportStub {
    render_state: Option<egui_wgpu::RenderState>,
    target: Option<(ViewTarget, egui::TextureId)>,
}

impl ViewportStub {
    pub(crate) fn new(render_state: Option<egui_wgpu::RenderState>) -> Self {
        Self { render_state, target: None }
    }

    pub(crate) fn has_gpu(&self) -> bool {
        self.render_state.is_some()
    }

    /// The current offscreen target, if one was rendered.
    pub(crate) fn target(&self) -> Option<(&egui_wgpu::RenderState, &ViewTarget)> {
        Some((self.render_state.as_ref()?, &self.target.as_ref()?.0))
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let Some(rs) = self.render_state.as_ref() else {
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(0xC3, 0xCA, 0xD3));
            return;
        };
        let ppp = ui.pixels_per_point();
        let px = |v: f32| ((v * ppp).round() as u32).clamp(1, 8192);
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
                None => renderer.register_native_texture(&rs.device, target.resolve_view(), wgpu::FilterMode::Nearest),
            };
            drop(renderer);
            gc_render::clear(&rs.device, &rs.queue, &target, [0.90, 0.92, 0.93, 1.0]);
            self.target = Some((target, id));
        }
        if let Some((_, id)) = self.target.as_ref() {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(*id, rect, uv, egui::Color32::WHITE);
        }
    }
}
