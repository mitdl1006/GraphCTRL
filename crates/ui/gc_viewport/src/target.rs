//! The 3D part of the viewport: the renderer's offscreen target shown as an image.

use gc_draw::Camera;
use gc_math::DMat4;
use gc_render::{DrawStats, Instance, Line, MeshRenderer, View, ViewTarget};
use gc_scene::Scene;
use gc_ui::colors;

/// Owns the offscreen render target and its registration with egui.
pub struct Background {
    render_state: Option<egui_wgpu::RenderState>,
    target: Option<(ViewTarget, egui::TextureId)>,
    meshes: Option<MeshRenderer>,
    stats: DrawStats,
}

impl Background {
    pub(crate) fn new(render_state: Option<egui_wgpu::RenderState>) -> Self {
        Self { render_state, target: None, meshes: None, stats: DrawStats::default() }
    }

    /// True when a GPU renders the background.
    pub fn has_gpu(&self) -> bool {
        self.render_state.is_some()
    }

    /// The current offscreen target, if one was rendered.
    pub fn target(&self) -> Option<(&egui_wgpu::RenderState, &ViewTarget)> {
        Some((self.render_state.as_ref()?, &self.target.as_ref()?.0))
    }

    /// Draw calls and triangles of the last frame.
    pub fn stats(&self) -> DrawStats {
        self.stats
    }

    pub(crate) fn paint(&mut self, ui: &egui::Ui, rect: egui::Rect, camera: &Camera, scene: &Scene) {
        let Some(rs) = self.render_state.as_ref() else { return };
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
            self.target = Some((target, id));
        }
        if let Some((target, _)) = self.target.as_ref() {
            let renderer = self.meshes.get_or_insert_with(|| MeshRenderer::new(&rs.device));
            // Solids of equal size share one mesh, uploaded the first time it is seen.
            for solid in &scene.solids {
                let key = solid.shape.key();
                if !renderer.has_mesh(key)
                    && let Ok(mesh) = solid.shape.mesh()
                {
                    renderer.upload(&rs.device, &rs.queue, key, &mesh);
                }
            }
            let eye = camera.eye();
            #[allow(clippy::cast_possible_truncation, reason = "GPU data is f32; positions are eye-relative")]
            let narrow = |m: [[f64; 4]; 4]| m.map(|column| column.map(|v| v as f32));
            let instances: Vec<Instance> = scene
                .solids
                .iter()
                .map(|solid| Instance {
                    mesh: solid.shape.key(),
                    model: narrow(
                        DMat4::from_rotation_translation(solid.pose.rotation, solid.pose.translation - eye)
                            .to_cols_array_2d(),
                    ),
                    color: solid.color,
                })
                .collect();
            let bounds = scene.bounds();
            let extent = if bounds.is_empty() {
                camera.distance
            } else {
                (bounds.center() - camera.pivot).length() + bounds.size().length()
            };
            let size = [f64::from(rect.width()), f64::from(rect.height())];
            let (forward, up, right) = (camera.forward(), camera.up(), camera.right());
            let light = (forward * 0.75 - up * 0.5 + right * 0.4).normalize_or_zero();
            #[allow(clippy::cast_possible_truncation, reason = "GPU data is f32")]
            let vec3 = |v: gc_math::DVec3| [v.x as f32, v.y as f32, v.z as f32];
            let view = View {
                clip_from_relative: narrow(camera.clip_from_eye_relative(size, extent)),
                light: vec3(light),
                forward: vec3(forward),
            };
            // Transparent where nothing is drawn, so that the grid painted underneath shows through.
            let clear = [0.0; 4];
            // Traces of planned paths: lines in the 3D picture, hidden where a solid is in front of them.
            let color = |c: egui::Color32| [c.r(), c.g(), c.b(), 255].map(|v| f32::from(v) / 255.0);
            let lines: Vec<Line> = scene
                .traces
                .iter()
                .flat_map(|trace| {
                    let color = color(if trace.current { colors::TRACE } else { colors::INACTIVE });
                    trace.points.windows(2).map(move |pair| Line {
                        a: vec3(pair[0] - eye),
                        b: vec3(pair[1] - eye),
                        color,
                        width: 2.5,
                    })
                })
                .collect();
            self.stats = renderer.draw(&rs.device, &rs.queue, target, clear, &view, &instances, &lines);
        }
        if let Some((_, id)) = self.target.as_ref() {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(*id, rect, uv, egui::Color32::WHITE);
        }
    }
}
