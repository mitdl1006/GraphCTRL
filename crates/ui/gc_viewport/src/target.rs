//! The 3D part of the viewport: the renderer's offscreen target shown as an image.

use gc_draw::Camera;
use gc_math::DMat4;
use gc_render::{DrawStats, EdgeVertex, Instance, Line, MeshRenderer, View, ViewTarget};
use gc_scene::Scene;

use crate::paint::trace_style;

/// How far behind the sketch plane the slice starts, metres: faces that lie in the plane (a body resting
/// on it) are cut away too, so that nothing covers the plane.
const SLICE_TOLERANCE: f64 = -1e-5;

/// The most edges drawn for one mesh: past it a scanned or finely tessellated mesh would turn into a
/// web of lines, so only its sharpest edges (70°) are kept, or none.
const EDGE_BUDGET: usize = 3000;

/// Owns the offscreen render target and its registration with egui.
pub struct Background {
    render_state: Option<egui_wgpu::RenderState>,
    target: Option<(ViewTarget, egui::TextureId)>,
    meshes: Option<MeshRenderer>,
    /// The feature edges of each uploaded mesh, as segments in the mesh's own frame.
    edges: std::collections::BTreeMap<u64, std::sync::Arc<Vec<[gc_math::DVec3; 2]>>>,
    stats: DrawStats,
}

impl Background {
    pub(crate) fn new(render_state: Option<egui_wgpu::RenderState>) -> Self {
        Self {
            render_state,
            target: None,
            meshes: None,
            edges: std::collections::BTreeMap::new(),
            stats: DrawStats::default(),
        }
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

    pub(crate) fn paint(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        camera: &Camera,
        scene: &Scene,
        slice: Slicing,
    ) {
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
                    // The edges drawn as lines, as CAD programs do: folds sharper than 30 degrees. A finely
                    // detailed mesh keeps only its sharpest folds, and none if even those are too many,
                    // so that the picture stays quick.
                    let at = |i: u32| {
                        let p = mesh.positions().get(i as usize).copied().unwrap_or_default();
                        gc_math::DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
                    };
                    let mut found = gc_mesh::edges::feature_edges(&mesh, 30.0_f64.to_radians());
                    if found.len() > EDGE_BUDGET {
                        found = gc_mesh::edges::feature_edges(&mesh, 70.0_f64.to_radians());
                    }
                    if found.len() > EDGE_BUDGET {
                        found.clear();
                    }
                    let edges = found.into_iter().map(|[a, b]| [at(a), at(b)]).collect();
                    self.edges.insert(key, std::sync::Arc::new(edges));
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
                // Slicing: what lies on the cut side of the plane(s) is cut away.
                slice: slice.planes.first().map(|plane| relative_plane(*plane, eye)),
                slice2: slice.planes.get(1).map(|plane| relative_plane(*plane, eye)),
                slice_union: slice.union,
                slice_cap: slice.cap,
            };
            let removed = |p: gc_math::DVec3| slice.removes(p);
            // Transparent where nothing is drawn, so that the grid painted underneath shows through.
            let clear = [0.0; 4];
            // Traces of planned paths: lines in the 3D picture, hidden where a solid is in front of them.
            let color = |c: egui::Color32| [c.r(), c.g(), c.b(), 255].map(|v| f32::from(v) / 255.0);
            let lines: Vec<Line> = scene
                .traces
                .iter()
                .flat_map(|trace| {
                    let (shade, width) = trace_style(trace);
                    let color = color(shade);
                    trace.points.windows(2).map(move |pair| Line {
                        a: vec3(pair[0] - eye),
                        b: vec3(pair[1] - eye),
                        color,
                        width,
                    })
                })
                .collect();
            // Edges in a darker shade of the solid's own colour, fading with it.
            let mut edges = Vec::new();
            for solid in &scene.solids {
                let Some(segments) = self.edges.get(&solid.shape.key()) else { continue };
                let c = solid.color;
                let color = [c[0] * 0.30, c[1] * 0.30, c[2] * 0.32, c[3]];
                for [a, b] in segments.iter() {
                    if removed(solid.pose * *a) && removed(solid.pose * *b) {
                        continue;
                    }
                    edges.push(EdgeVertex { position: vec3(solid.pose * *a - eye), color });
                    edges.push(EdgeVertex { position: vec3(solid.pose * *b - eye), color });
                }
            }
            self.stats =
                renderer.draw(&rs.device, &rs.queue, target, clear, &view, &instances, &lines, &edges);
        }
        if let Some((_, id)) = self.target.as_ref() {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(*id, rect, uv, egui::Color32::WHITE);
        }
    }
}

/// What is cut away: up to two planes `(normal, offset)` in the world (the side the normal points to is
/// cut), and with two planes whether the cut is of either side (union) or of both (intersection).
#[derive(Clone, Debug, Default)]
pub(crate) struct Slicing {
    pub(crate) planes: Vec<(gc_math::DVec3, f64)>,
    pub(crate) union: bool,
    /// Draw the cut open (a section view): the inside of the far walls is a flat cap.
    pub(crate) cap: bool,
}

impl Slicing {
    /// Whether a world point is cut away.
    fn removes(&self, p: gc_math::DVec3) -> bool {
        let beyond = |(normal, offset): &(gc_math::DVec3, f64)| normal.dot(p) - offset > SLICE_TOLERANCE;
        match self.planes.as_slice() {
            [] => false,
            [a] => beyond(a),
            [a, b, ..] => {
                if self.union {
                    beyond(a) || beyond(b)
                } else {
                    beyond(a) && beyond(b)
                }
            }
        }
    }
}

/// A world plane as the renderer takes it: relative to the eye.
fn relative_plane((normal, offset): (gc_math::DVec3, f64), eye: gc_math::DVec3) -> [f32; 4] {
    #[allow(clippy::cast_possible_truncation, reason = "GPU data is f32")]
    let plane = [
        normal.x as f32,
        normal.y as f32,
        normal.z as f32,
        (normal.dot(eye) - offset - SLICE_TOLERANCE) as f32,
    ];
    plane
}
