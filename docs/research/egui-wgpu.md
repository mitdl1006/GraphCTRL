# GraphCTRL: eframe/egui 0.37 + wgpu 30 with an embedded 3D viewport

Legend: **[V]** verified today in the named published source, **[R]** recalled from memory, **[S]** web-search snippet only.

Sources read: crate tarballs for eframe / egui / epaint / egui-wgpu / egui-winit / egui_kittest 0.37.0, egui_tiles 0.18.0, wgpu / wgpu-types / wgpu-hal 30.0.2, re_renderer / re_viewer_context 0.43.2; `emilk/egui@0.37.0` `CHANGELOG.md` and `custom3d_wgpu.rs`; `gfx-rs/wgpu@trunk` `CHANGELOG.md`. Nothing was compiled.

## 1. eframe 0.37 [V `eframe-0.37.0/src/epi.rs`, `src/lib.rs`, `Cargo.toml.orig`]

```rust
pub trait App {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut Frame) {}   // optional; no painting allowed
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame);           // the ONLY required method
    fn save(&mut self, _storage: &mut dyn Storage) {}                 // feature "persistence"
    fn on_exit(&mut self) {}  // becomes on_exit(&mut self, Option<&glow::Context>) when feature "glow" is on
    fn auto_save_interval(&self) -> core::time::Duration { /* 30 s */ }
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] { /* gamma-space floats */ }
    fn persist_egui_memory(&self) -> bool { true }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {}
}
pub type AppCreator<'app> = Box<dyn 'app + FnOnce(&CreationContext<'_>) -> Result<Box<dyn 'app + App>, DynError>>;
pub fn run_native(app_name: &str, native_options: NativeOptions, app_creator: AppCreator<'_>) -> eframe::Result;
pub fn run_ui_native(app_name: &str, o: NativeOptions, ui_fun: impl FnMut(&mut egui::Ui, &mut Frame) + 'static) -> eframe::Result;
```

- **No `update(ctx, frame)`.** It was replaced in 0.34 and is absent from the 0.37 trait. The root `Ui` "has no margin or background color", so wrap content in `Panel` / `CentralPanel`. `logic` runs before each `ui` and also while the window is hidden.
- **Other runners:** `run_native_ext(.., egui_ctx: Option<egui::Context>, ..)` and `create_native(.., &EventLoop<UserEvent>)`.
- **`NativeOptions` fields:**
  - `viewport: egui::ViewportBuilder`
  - `multisampling: u16` (0), `depth_buffer: u8` (0), `stencil_buffer: u8` (0)
  - `renderer: Renderer` (`Wgpu` when `glow` is off)
  - `wgpu_options: egui_wgpu::WgpuConfiguration` (default plus `SurfaceConfig::LOW_LATENCY`)
  - `persist_window` (true), `persistence_path: Option<PathBuf>`
  - `system_font_fallback` (true), `dithering` (true), `clipboard_shortcuts` (true)
  - `run_and_return`, `centered`, `event_loop_builder`, `window_builder`
- **`WgpuConfiguration`** is `{ surface: SurfaceConfig, wgpu_setup: WgpuSetup, on_surface_status }` [V `egui-wgpu/src/lib.rs`, `setup.rs`].
  - `WgpuSetup::CreateNew(WgpuSetupCreateNew { instance_descriptor, display_handle, power_preference, native_adapter_selector, device_descriptor: Arc<dyn Fn(&Adapter) -> DeviceDescriptor<'static>> })` or `Existing { instance, adapter, device, queue }`.
  - The default device descriptor asks for `Limits::default()` with `max_texture_dimension_2d: 8192` and no features.
  - Backends come from `WGPU_BACKEND`, else `PRIMARY | GL`. eframe fills the display handle itself.
- **Render state:** `cc.wgpu_render_state: Option<egui_wgpu::RenderState>` and `frame.wgpu_render_state() -> Option<&RenderState>`.
- **Persistence:** `cc.storage: Option<&dyn Storage>`, `eframe::get_value` / `set_value`, `eframe::APP_KEY`.
- **Features:** the default set is `accesskit, clipboard, default_fonts, links, system_fonts, wayland, web_screen_reader, wgpu, winit/default, x11`.
  - The default build is already wgpu-only; `glow` is opt-in.
  - `wgpu` = `wgpu_no_default_features` + `egui-wgpu/default`, which pulls `wgpu/default` (dx12, metal, gles, vulkan, wgsl), `wgpu/webgl` and `macos-window-resize-jitter-fix`.
  - Opt-in extras: `persistence` (ron + serde) and `inspection` (TCP port 5719 when `EGUI_INSPECTION=1`).
  - Keep default features. `winit/default` cannot be re-enabled from downstream, and it carries the Wayland client-side decorations [R].

## 2. egui-wgpu 0.37 [V `egui-wgpu-0.37.0/src/renderer.rs`, `lib.rs`, `winit.rs`]

```rust
pub trait CallbackTrait: Send + Sync {
    fn prepare(&self, _device: &wgpu::Device, _queue: &wgpu::Queue, _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder, _callback_resources: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> { Vec::new() }
    fn finish_prepare(&self, _device: &wgpu::Device, _queue: &wgpu::Queue,
        _egui_encoder: &mut wgpu::CommandEncoder, _callback_resources: &mut CallbackResources) -> Vec<wgpu::CommandBuffer> { Vec::new() }
    fn paint(&self, info: PaintCallbackInfo, render_pass: &mut wgpu::RenderPass<'static>, callback_resources: &CallbackResources);
}
impl Callback { pub fn new_paint_callback(rect: epaint::emath::Rect, callback: impl CallbackTrait + 'static) -> epaint::PaintCallback }
pub type CallbackResources = type_map::concurrent::TypeMap;   // lives in Renderer::callback_resources
pub struct RenderState { pub adapter: wgpu::Adapter, pub available_adapters: Vec<wgpu::Adapter>, pub instance: wgpu::Instance,
    pub device: wgpu::Device, pub queue: wgpu::Queue, pub target_format: wgpu::TextureFormat, pub renderer: Arc<RwLock<Renderer>> }
impl Renderer {
    pub fn register_native_texture(&mut self, device: &wgpu::Device, texture: &wgpu::TextureView, texture_filter: wgpu::FilterMode) -> epaint::TextureId;
    pub fn update_egui_texture_from_wgpu_texture(&mut self, device: &wgpu::Device, texture: &wgpu::TextureView, texture_filter: wgpu::FilterMode, id: epaint::TextureId);
    pub fn free_texture(&mut self, id: &epaint::TextureId);    // plus *_with_sampler_options variants
}
```

The docs state the registered texture "must have the texture format `Rgba8Unorm`".

| | (a) paint callback into egui's pass | (b) own offscreen target shown as an image | (c) Rerun hybrid |
|---|---|---|---|
| Colour format | `target_format`: first of `Rgba8Unorm`/`Bgra8Unorm` the surface offers, gamma-space | own; presented view must be `Rgba8Unorm` | own, any format |
| MSAA | `NativeOptions::multisampling`, applied to the whole UI | viewport only | viewport only |
| Depth | only via `depth_buffer`/`stencil_buffer` bits; cleared to 1.0, `StoreOp::Discard` | own (reverse-Z possible) | own |
| Extra targets (ID buffer) | impossible, one colour attachment | yes | yes |
| In kittest | pass is 1×, colour-only | works | works |
| Screenshot | whole window (`ViewportCommand::Screenshot`) | direct readback | direct readback |

- **Depth mapping for (a):** `depth_format_from_bits` accepts (0,8), (16,0), (24,0), (24,8), (32,0), (32,8). Anything else silently yields no depth buffer. egui's own pipeline uses `depth_write_enabled: Some(false)`, `depth_compare: Some(Always)`.
- **kittest and (a):** `WgpuTestRenderer::render` builds its pass with `..Default::default()`, so there is no depth attachment and one sample. I expect a callback pipeline built for depth or MSAA to fail wgpu validation there.
- **Rerun** [V `re_viewer_context-0.43.2/src/gpu_bridge/re_renderer_callback.rs`, on wgpu 30]: `prepare()` calls `view_builder.draw(ctx, clear_color)` and returns its own `CommandBuffer`, so the scene goes to Rerun's own targets. `paint()` sets an unclamped viewport and calls `view_builder.composite(ctx, render_pass)`. re_renderer ships `draw_phases/picking_layer.rs`, `draw_phases/screenshot.rs`, `allocator/gpu_readback_belt.rs` and `shader/composite.wgsl`.

**Verdict: (b).** It is the only option that gives own depth, MSAA, an ID target and direct screenshots while behaving identically in eframe, kittest and plain headless wgpu. Keep (c) in reserve for when a non-`Rgba8Unorm` target is needed.

## 3. wgpu 30.0.2 [V `wgpu-30.0.2/src/api/*.rs`, `wgpu-types-30.0.2/src/*.rs`]

```rust
pub struct RenderPipelineDescriptor<'a> { pub label: Label<'a>, pub layout: Option<&'a PipelineLayout>, pub vertex: VertexState<'a>,
    pub primitive: PrimitiveState, pub depth_stencil: Option<DepthStencilState>, pub multisample: MultisampleState,
    pub fragment: Option<FragmentState<'a>>, pub multiview_mask: Option<NonZeroU32>, pub cache: Option<&'a PipelineCache> }
pub struct VertexState<'a> { pub module: &'a ShaderModule, pub entry_point: Option<&'a str>,
    pub compilation_options: PipelineCompilationOptions<'a>, pub buffers: &'a [Option<VertexBufferLayout<'a>>] }
pub struct RenderPassDescriptor<'a> { pub label: Label<'a>, pub color_attachments: &'a [Option<RenderPassColorAttachment<'a>>],
    pub depth_stencil_attachment: Option<RenderPassDepthStencilAttachment<'a>>, pub timestamp_writes: Option<RenderPassTimestampWrites<'a>>,
    pub occlusion_query_set: Option<&'a QuerySet>, pub multiview_mask: Option<NonZeroU32> }
pub struct RenderPassColorAttachment<'tex> { pub view: &'tex TextureView, pub depth_slice: Option<u32>,
    pub resolve_target: Option<&'tex TextureView>, pub ops: Operations<Color> }
pub struct RenderPassDepthStencilAttachment<'tex> { pub view: &'tex TextureView, pub depth_ops: Option<Operations<f32>>, pub stencil_ops: Option<Operations<u32>> }
// Instance::new(desc: InstanceDescriptor) -> Instance      (by value; InstanceDescriptor has no Default)
// Instance::request_adapter(&RequestAdapterOptions<'_, '_>) -> impl Future<Output = Result<Adapter, RequestAdapterError>>
// Instance::enumerate_adapters(Backends) -> impl Future<Output = Vec<Adapter>>
// Adapter::request_device(&DeviceDescriptor<'_>) -> impl Future<Output = Result<(Device, Queue), RequestDeviceError>>
// Device::poll(PollType) -> Result<PollStatus, PollError>;  PollType::{Wait { submission_index: Option<_>, timeout: Option<Duration> }, Poll}
// BufferSlice::map_async(MapMode, impl FnOnce(Result<(), BufferAsyncError>) + 'static); get_mapped_range() -> Result<BufferView, MapRangeError>
```

- **Descriptor fields:**
  - `InstanceDescriptor { backends, flags, memory_budget_thresholds, backend_options, display }`, built with `new_without_display_handle()` or `..._from_env()`.
  - `RequestAdapterOptions { power_preference, force_fallback_adapter, compatible_surface, apply_limit_buckets }`.
  - `DeviceDescriptor { label, required_features, required_limits, experimental_features, memory_hints, trace }`.
  - `TextureDescriptor { label, size, mip_level_count, sample_count, dimension, format, usage, view_formats }`.
  - `TexelCopyTextureInfo { texture, mip_level, origin, aspect }`, `TexelCopyBufferInfo { buffer, layout: TexelCopyBufferLayout { offset, bytes_per_row, rows_per_image } }`.
- **Headless use and readback** (pattern from [V `egui_kittest-0.37.0/src/texture_to_image.rs`]):

```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
// render into `tex` (usage RENDER_ATTACHMENT | COPY_SRC), then:
let bpr = (4 * w).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (bpr * h) as u64,
    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
enc.copy_texture_to_buffer(tex.as_image_copy(),   // picking: TexelCopyTextureInfo { origin: Origin3d { x, y, z: 0 }, .. } + 1x1 extent
    wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: None } },
    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
let idx = queue.submit([enc.finish()]);
let (tx, rx) = std::sync::mpsc::channel();
buf.slice(..).map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
device.poll(wgpu::PollType::Wait { submission_index: Some(idx), timeout: Some(std::time::Duration::from_secs(10)) })?;
rx.recv()??;
let bytes = buf.slice(..).get_mapped_range()?;   // rows are padded to `bpr`
```

**Breaking changes 25 → 30** [V wgpu `CHANGELOG.md`]:
- **v27:** `PollType::WaitForSubmissionIndex` removed in favour of `Wait { submission_index, timeout }`. Mapping guards lost their lifetimes. `encoder.map_buffer_on_submit(..)` added.
- **v28:** `enumerate_adapters` became async. `SamplerDescriptor::mipmap_filter` is now `MipmapFilterMode`. Push constants became `PipelineLayoutDescriptor::immediate_size`. Error scopes became guards (`let s = device.push_error_scope(..); s.pop().await`). `multiview_mask` introduced.
- **v29:**
  - `Surface::get_current_texture()` returns the `CurrentSurfaceTexture` enum.
  - `InstanceDescriptor::default()` and `from_env_or_default()` removed.
  - `bind_group_layouts: &[Option<&BindGroupLayout>]`.
  - `DepthStencilState::{depth_write_enabled: Option<bool>, depth_compare: Option<CompareFunction>}`.
  - Write mappings are `WriteOnly<[u8]>`.
- **v30:**
  - `VertexState::buffers: &[Option<VertexBufferLayout>]`.
  - Integer shader I/O needs an explicit `@interpolate(flat)`, which hits object-ID shaders.
  - `get_mapped_range*` returns `Result`.
  - `SurfaceTexture::present()` became `Queue::present(..)`.
  - `SurfaceConfiguration::color_space` added.
  - `TextureUsages::TRANSIENT` renamed to `TRANSIENT_ATTACHMENT`.
  - `depth_ops`/`stencil_ops` must be `None` when the format lacks that aspect.
- **Trunk (v31)** adds `DeviceDescriptor::default_queue` and `TextureDescriptor::texture_binding_view_dimension`. Use `..Default::default()` where a `Default` exists.

## 4. egui 0.37 specifics [V `egui-0.37.0/src/**`, `epaint-0.37.0/src/text/*`, `egui-winit-0.37.0/src/lib.rs`, changelog]

**Fonts and text stack**
- The API shape is unchanged:
  - `FontDefinitions { font_data: BTreeMap<String, Arc<FontData>>, families: BTreeMap<FontFamily, Vec<String>> }`.
  - `FontData::from_static` / `from_owned` / `from_blob(Blob, index)`, plus `.tweak(FontTweak)`.
  - `ctx.set_fonts(..)`.
  - `ctx.add_font(FontInsert::new(name, data, vec![InsertFontFamily { family, priority: FontPriority::Highest | Lowest }]))`.
- The stack underneath is new: skrifa + vello_cpu replaced ab_glyph in 0.34, harfrust shaping arrived in 0.35, and 0.37 added `ctx.add_font_provider(Arc<dyn FontProvider>)`.
- Native eframe installs `egui_system_font_provider` (feature `system_fonts`), so Hangul renders wherever the OS has a Korean font.
- `default_fonts` is no longer a default feature of egui/epaint; eframe still enables it.

**IME**
- Events are `ImeEvent::{Preedit { text, active_range_chars }, Commit(String), DeleteSurrounding { .. }}`. `Enabled` and `Disabled` are deprecated.
- egui-winit carries two Windows workarounds, written against winit 0.30.12:
  - IME-processed key events are filtered via `NamedKey::Process`.
  - The preedit active range is dropped, because winit "incorrectly reports the cursor position ... during composition with the builtin Korean IME" (cites egui PR #8083).
- winit PR #4478 (macOS Korean IME fixes) has unknown release status [S].
- The setups listed as tested in egui-winit's comments are Chinese IMEs only.

**Input**
- `InputState` public fields: `raw, pointer, smooth_scroll_delta, pixels_per_point, max_texture_side, time, unstable_dt, predicted_dt, stable_dt, focused, modifiers, keys_down, events`.
- There is no `raw_scroll_delta` any more. For raw wheel data read `Event::MouseWheel { unit, delta, phase, source, modifiers }` from `events`.
- Methods: `smooth_scroll_delta()`, `zoom_delta()`, `zoom_delta_2d()`, `rotation_delta()`, `translation_delta()`, `multi_touch()`, `key_pressed/down/released`, `consume_key`, `consume_shortcut(&KeyboardShortcut) -> bool`.
- `PointerState`: `button_down(PointerButton)`, `middle_down()`, `delta()`, `motion()`, `hover_pos()`, `interact_pos()`, `press_origin()`. Buttons are `PointerButton::{Primary, Secondary, Middle, Extra1, Extra2}`.
- `Response`: `dragged_by(btn)`, `drag_started_by`, `drag_stopped_by`, `drag_delta()`, `drag_motion()`, `clicked_by`, `double_clicked_by`, `hover_pos()`, `interact_pointer_pos()`, `contains_pointer()`, `context_menu(..)`.
- `Sense::{hover, click, drag, click_and_drag}`.
- Since 0.36, modifiers arrive as `Event::ModifiersChanged`, not in `RawInput`.

**Painting**
- `Painter`: `line_segment([Pos2; 2], stroke)`, `line(Vec<Pos2>, PathStroke)`, `hline`, `vline`, `circle`, `circle_filled`, `circle_stroke`, `rect(rect, radius, fill, stroke, StrokeKind)`, `arrow(origin, vec, stroke)`, `text(pos, Align2, text, FontId, Color32) -> Rect`, `galley`, `image(texture_id, rect, uv, tint)`, `add`, `extend`.
- Dashes: `Shape::dashed_line(path: &[Pos2], stroke, dash_length, gap_length) -> Vec<Shape>`, plus `dotted_line` and `dashed_line_many`.

**Containers**
- `Panel::{left, right, top, bottom}(id_salt).show(ui, ..)` and `CentralPanel::{default(), no_frame()}.show(ui, ..)`. `SidePanel` and `TopBottomPanel` no longer exist.
- Menus: `MenuBar::new().ui(ui, ..)`, `ui.menu_button(atoms, ..) -> InnerResponse<Option<R>>`, `ui.close()`.
- Popups: `Popup::menu(&resp)`, `Popup::context_menu(&resp)`, then `.show(..)`.
- Modal: `Modal::new(id).show(ctx, ..) -> ModalResponse` with `.should_close()`.
- `Scene::new().zoom_range(..).drag_pan_buttons(DragPanButtons::MIDDLE).scroll_zooms(true).show(ui, &mut scene_rect, |ui| ..)`. The default zoom range tops out at 1.0, and text blurs when zoomed in.

**Other**
- Drag and drop: `ui.dnd_drag_source(id, payload, ..)`, `ui.dnd_drop_zone::<P, _>(frame, ..) -> (InnerResponse<R>, Option<Arc<P>>)`, `Response::dnd_*`, `DragAndDrop::*`.
- Shortcuts: `KeyboardShortcut::new(Modifiers::COMMAND, Key::S)` (const), `ctx.format_shortcut`, `conflicts_with_text_editing()`.
- Style: `ctx.set_theme`, `set_visuals_of(Theme, Visuals)`, `global_style()`, `global_style_mut`, `set_global_style`.
- `UiBuilder::new().id_salt(..).max_rect(..).layout(..).sense(..)` with `ui.scope_builder`.
- Repaint: `request_repaint()`, `request_repaint_after(Duration)`. `Context::run` became `run_ui`. `content_rect()` replaces `screen_rect`.

**Deprecated; avoid** [V `#[deprecated]` scan]: `Id::new` (use `Id::unique` / `IdSalt::new`), `Ui::id()` and `UiBuilder::id` (use `scope_id`), `Panel::show_inside` / `show_animated_inside` / `show_animated_between_inside` (use `show` / `show_collapsible` / `show_switched`), `WidgetType` (use `Role`), `TextEdit::horizontal_align` / `vertical_align`, `clip_rect_margin`, `global_theme_preference_switch`.

## 5. egui_tiles 0.18 [V `egui_tiles-0.18.0/src/*.rs`, `examples/`]

- **Dependencies:** `egui = "0.37.0"`; feature `serde` is on by default.
- **Types:** `Tree<Pane> { root: Option<TileId>, tiles: Tiles<Pane>, .. }` derives serde. `Container::{Tabs, Linear, Grid}`, `Linear { children, dir: LinearDir::{Horizontal, Vertical}, shares }`.
- **Building a tree:** `Tree::new(id, root, tiles)`, `Tiles::insert_pane`, `insert_tab_tile`, `insert_horizontal_tile`, `insert_vertical_tile`, `insert_container(impl Into<Container>)`, `Shares::set_share(id, f32)`, `Linear::new_binary(dir, [a, b], fraction)`.
- **Running it:** `tree.ui(&mut dyn Behavior<Pane>, &mut Ui)`. `tree.set_visible(tile, bool)` hides a tile but keeps its position.
- **`Behavior` required methods:**
  - `fn pane_ui(&mut self, ui: &mut Ui, tile_id: TileId, pane: &mut Pane) -> UiResponse`
  - `fn tab_title_for_pane(&mut self, pane: &Pane) -> WidgetText`
- **`Behavior` optional methods:** `is_tab_closable(&self, &Tiles<Pane>, TileId) -> bool` (default false), `on_tab_close`, `is_tile_draggable` (default true), `is_container_resizable`, `is_tab_bar_visible(&self, TileId, &Tabs)`, `simplification_options`, `tab_ui`, `top_bar_right_ui`, `tab_bar_left_ui`, `tab_bar_trailing_ui`, `tab_bar_height`, `gap_width`, `min_size`, `retain_pane`, `on_edit(EditAction)`, `cycle_tabs_with_keyboard`, `paint_on_top_of_tile`, and the colour/stroke hooks.
- **Simplification:** fields are `prune_empty_tabs, prune_empty_containers, prune_single_child_tabs, prune_single_child_containers, all_panes_must_have_tabs, join_nested_linear_containers, flatten_tabs_in_tabs`. The default prunes and joins, so set `all_panes_must_have_tabs: true` or `SimplificationOptions::OFF` for a fixed layout.
- **Persistence:** `eframe::set_value(storage, key, &tree)`, as in the advanced example.
- There are no floating or detached panes [R].

## 6. egui_kittest 0.37 [V `egui_kittest-0.37.0/src/{builder,lib,wgpu,renderer,texture_to_image,snapshot}.rs`]

- **Builder:** `Harness::builder()` with `with_size`, `with_pixels_per_point`, `with_theme`, `with_os`, `with_max_steps` (4), `with_step_dt` (0.25), `with_context_setup(|ctx| ..)`, `allow_missing_glyphs()`, `with_accessibility_check(bool)`, `with_render_every_step`, `with_render_options`, `wgpu()`, `wgpu_setup(WgpuSetup)`, `renderer(impl TestRenderer)`.
- **Build:** `build_ui(|ui| ..)`, `build_ui_state(|ui, state| .., state)`, or `build_eframe(|cc: &mut CreationContext| -> impl eframe::App)` (feature `eframe`).
- **Driving:** `run()`, `try_run()`, `run_steps(n)`, `step()`, `event(egui::Event)`, `event_modifiers`, `key_press[_modifiers]`, `key_combination`, `hover_at`, `drag_at`, `drop_at(Pos2)`, `state_mut()`, `set_size`. Nodes come from `kittest::Queryable` (`get_by_label(..)`) and offer `.click()`, `.click_secondary()`, `.hover()`, `.type_text()`, `.scroll_*`.
- **Images:** `render() -> Result<image::RgbaImage, String>`. Feature `snapshot` adds `image_snapshot(&img, name)`, `SnapshotOptions::{threshold, failed_pixel_count_threshold, output_path}` and `UPDATE_SNAPSHOTS=1`.
- **Callbacks and native textures do render.** `WgpuTestRenderer::render` calls `renderer.update_buffers` (which runs the prepare callbacks) and then `renderer.render`.
- **`cc.wgpu_render_state` is only set by `WgpuTestRenderer::setup_eframe`.** The default `LazyRenderer` leaves it `None`, so call `.wgpu()` before `build_eframe`.
- **Adapter choice** (`default_wgpu_setup`): backends come from `WGPU_BACKEND`, else `PRIMARY | GL`. The selector sorts by backend, then prefers `DeviceType::Cpu`. `WGPU_ADAPTER_NAME` is not consulted, and `force_fallback_adapter` is hard-coded false.

**Sandbox, probed read-only through ctypes:**
- With `VK_ICD_FILENAMES=/opt/pw-browsers/chromium-1243/chrome-linux64/vk_swiftshader_icd.json`, the system loader `/lib/x86_64-linux-gnu/libvulkan.so.1` enumerates one device: "SwiftShader Device (Subzero)", Vulkan 1.3.0, CPU type, conformance 1.3.3.1, `maxImageDimension2D` 16384, 84 extensions.
- That passes every hide-check I read in `wgpu-hal-30.0.2/src/vulkan/adapter.rs` (conformance major ≠ 0, API ≥ 1.1 or maintenance1) and egui's 8192 limit.
- Actual `request_device` and rendering were not run.
- Lavapipe is not installed (`/usr/share/vulkan/icd.d` is missing). It would need `mesa-vulkan-drivers`, and it is what wgpu's own CI uses [S].
- There is no system libEGL, so set `WGPU_BACKEND=vulkan`.

## 7. Rendering techniques (pointers are [R])

- **Thick anti-aliased lines and points:** instanced quads expanded in clip space (offset = px · 2/viewport · w). Clip the segment against the near plane in the vertex shader; use SDF coverage in the fragment shader. References: bevy_gizmos `lines.wgsl`, re_renderer `lines.wgsl`, three.js `Line2`.
- **Infinite grid:** fullscreen triangle, ray–plane intersection, `fwidth` anti-aliasing, write `frag_depth`. References: Ben Golus's "Best Darn Grid Shader", `bevy_infinite_grid`.
- **Sketch on faces:** `DepthStencilState::bias`, or a clip-space z nudge in the vertex shader. Add a second dimmed pass for hidden sketch geometry.
- **Picking:** CPU picking (screen-space distance, parry3d BVH) for sketch entities, since snapping needs that maths anyway. Add a GPU `R32Uint` ID pass with a 1×1 readback for meshes later.
- **ViewCube:** a second viewport and scissor in the same pass, an orthographic camera sharing the main rotation, CPU ray-vs-cube picking, egui text for labels.
- **Camera:**
  - State is f64 pivot, orientation and distance or ortho height.
  - Use reverse-Z (`Depth32Float`, clear 0, `Greater`).
  - Zoom to cursor: `eye' = p + (eye − p)/s` in perspective; `center' = p − (p − center)/s` in ortho.
  - Matching sizes across projections: `ortho_h = 2·d·tan(fov/2)`.
  - Navigation presets: SolidWorks (MMB orbit, Ctrl+MMB pan) and Inventor (MMB pan, Shift+MMB orbit).
- **HiDPI:** target size is `round(rect.size() * ui.pixels_per_point())`. Recreate on change, and convert pointer positions with `(pos − rect.min) * ppp`.

## Skeleton

```toml
[dependencies]
eframe = { version = "0.37", features = ["persistence"] }   # defaults are wgpu-only; no glow
egui_tiles = "0.18"
wgpu = "30"            # for the egui-free renderer crate; unifies with egui-wgpu's ^30
serde = { version = "1", features = ["derive"] }
pollster = "0.4"
[dev-dependencies]
egui_kittest = { version = "0.37", features = ["wgpu", "snapshot", "eframe"] }
image = "0.25"
```

```rust
use eframe::{egui, egui_wgpu, wgpu};
type BoxErr = Box<dyn std::error::Error + Send + Sync>;

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("GraphCTRL")
            .with_inner_size([1440.0, 900.0]).with_min_inner_size([960.0, 600.0]),
        ..Default::default() // Renderer::Wgpu, multisampling 0, depth_buffer 0
    };
    eframe::run_native("graphctrl", opts, Box::new(|cc| Ok(Box::new(App::new(cc)?))))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Pane { Browser, Viewport, Properties, Log }

struct App { tree: egui_tiles::Tree<Pane>, view: Viewport3d }

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, BoxErr> {
        // fonts: cc.egui_ctx.add_font(..); layout restore: eframe::get_value(storage, "dock")
        Ok(Self { tree: default_tree(), view: Viewport3d { rs: cc.wgpu_render_state.clone(), target: None } })
    }
}

fn default_tree() -> egui_tiles::Tree<Pane> {
    use egui_tiles::{Linear, LinearDir, Tiles};
    let mut t = Tiles::default();
    let (browser, view) = (t.insert_pane(Pane::Browser), t.insert_pane(Pane::Viewport));
    let (props, log) = (t.insert_pane(Pane::Properties), t.insert_pane(Pane::Log));
    let center = t.insert_tab_tile(vec![view]);
    let mut row = Linear::new(LinearDir::Horizontal, vec![browser, center, props]);
    row.shares.set_share(browser, 0.6); row.shares.set_share(center, 3.0); row.shares.set_share(props, 0.8);
    let row = t.insert_container(row);
    let mut col = Linear::new(LinearDir::Vertical, vec![row, log]);
    col.shares.set_share(row, 4.0); col.shares.set_share(log, 1.0);
    let root = t.insert_container(col);
    egui_tiles::Tree::new(egui::Id::unique("dock"), root, t)
}

struct Dock<'a> { view: &'a mut Viewport3d }
impl egui_tiles::Behavior<Pane> for Dock<'_> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText { format!("{pane:?}").into() }
    fn pane_ui(&mut self, ui: &mut egui::Ui, _: egui_tiles::TileId, pane: &mut Pane) -> egui_tiles::UiResponse {
        match pane { Pane::Viewport => self.view.ui(ui), p => { ui.label(format!("{p:?}")); } }
        egui_tiles::UiResponse::None
    }
    fn is_tile_draggable(&self, tiles: &egui_tiles::Tiles<Pane>, id: egui_tiles::TileId) -> bool {
        !matches!(tiles.get_pane(&id), Some(Pane::Viewport))
    } // is_tab_closable already defaults to false
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions { all_panes_must_have_tabs: true, ..Default::default() }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() { ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close); }
                });
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| ui.label("Ready"));
        egui::CentralPanel::no_frame().show(ui, |ui| self.tree.ui(&mut Dock { view: &mut self.view }, ui));
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) { eframe::set_value(storage, "dock", &self.tree); }
}

struct Target { size: [u32; 2], tex: wgpu::Texture, id: egui::TextureId }
struct Viewport3d { rs: Option<egui_wgpu::RenderState>, target: Option<Target> }

impl Viewport3d {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let Some(rs) = self.rs.as_ref() else { ui.painter().rect_filled(rect, 0.0, egui::Color32::DARK_GRAY); return; };
        let ppp = ui.pixels_per_point();
        let px = |v: f32| ((v * ppp).round() as u32).clamp(1, 8192);
        let size = [px(rect.width()), px(rect.height())];
        if resp.dragged_by(egui::PointerButton::Middle) { let _orbit_or_pan = resp.drag_motion(); }
        if resp.contains_pointer() { let _zoom = ui.input(|i| (i.smooth_scroll_delta().y, i.zoom_delta())); }
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            let tex = rs.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("viewport.color"),
                size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm, // required by register_native_texture
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            let mut r = rs.renderer.write(); // keep this guard short; never take it inside a paint callback
            let id = match self.target.take() {
                Some(old) => { r.update_egui_texture_from_wgpu_texture(&rs.device, &view, wgpu::FilterMode::Nearest, old.id); old.id }
                None => r.register_native_texture(&rs.device, &view, wgpu::FilterMode::Nearest),
            };
            self.target = Some(Target { size, tex, id });
        }
        let t = self.target.as_ref().unwrap();
        let view = t.tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("viewport") });
        {
            let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport.main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, depth_slice: None, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.09, g: 0.10, b: 0.12, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None, // real: 4x MSAA colour + Depth32Float (stencil_ops: None), resolve into `view`
                timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
            });
            // set_pipeline / draw: grid, meshes, sketch entities
        }
        rs.queue.submit([enc.finish()]); // lands before egui's own pass this frame
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(t.id, rect, uv, egui::Color32::WHITE);
        // 2D overlays: ui.painter_at(rect).text(..) / line_segment(..) / extend(Shape::dashed_line(..))
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable as _;
    #[test]
    fn shell_smoke() {
        let mut h = egui_kittest::Harness::builder()
            .with_size(eframe::egui::vec2(1280.0, 800.0))
            .wgpu() // before build_eframe, or cc.wgpu_render_state stays None
            .build_eframe(|cc| super::App::new(cc).unwrap());
        h.run();
        h.get_by_label("File").click();
        h.run();
        assert_eq!(h.render().unwrap().dimensions(), (1280, 800));
    }
}
```

## Gotchas

- kittest panics on a glyph no font can draw (`MissingGlyphPolicy::Panic`) and fails on input widgets without an accessible name. Korean labels in tests need a bundled font via `with_context_setup`, or `allow_missing_glyphs()`.
- `harness.run()` panics after `max_steps` if the app keeps requesting repaints. Re-render the viewport only when dirty.
- egui expects gamma-encoded, premultiplied `Rgba8Unorm`. Either apply the sRGB transfer function in the fragment shader, or render through an `Rgba8UnormSrgb` view listed in `view_formats` and hand egui the Unorm view [R].
- Any dependency enabling `eframe/glow` changes the `on_exit` signature through feature unification.
- Use 4× MSAA only; 1 and 4 are the portable sample counts [R].
- Adding `#![deny(deprecated)]` in the UI crate keeps `Id::new` and `show_inside` out.

## Recommendation

1. **Stack:** eframe 0.37 with default features plus `persistence`, egui_tiles 0.18, a direct `wgpu = "30"` in an egui-free `render` crate, and egui_kittest with `wgpu`, `snapshot`, `eframe`.
2. **Embedding: (b).** Own targets: `Rgba8Unorm` 1× resolve/present, 4× MSAA colour, `Depth32Float` reverse-Z, and later an `R32Uint` ID target. Present with `register_native_texture` and `painter.image`; draw overlays with the egui `Painter`. The catch is that the presented texture is locked to `Rgba8Unorm`.
3. **Layering:** `viewport-math` (f64 camera and navigation, no GPU), `render` (wgpu only, headless tests with readback), `ui` (egui), `app` (eframe). Let `Viewport3d` tolerate a missing `RenderState` so logic tests run without a GPU.
4. **Picking for milestone 1:** CPU only. It is deterministic and unit-testable.
5. **Layout:** a fixed egui_tiles tree, panes not closable, View-menu toggles through `set_visible`, serde-persisted under a versioned key.
6. **Fonts:** bundle one OFL Korean font at `FontPriority::Lowest` and keep `system_font_fallback` on.
7. **First implementation step:** run `VK_ICD_FILENAMES=/opt/pw-browsers/chromium-1243/chrome-linux64/vk_swiftshader_icd.json WGPU_BACKEND=vulkan cargo test` on a clear-and-readback test.

## Risks and unknowns

- **SwiftShader:** wgpu 30 device creation and rendering on it are unexecuted. Fallbacks are lavapipe (needs a package install) or tests that skip when no adapter exists (wgpu also has a `noop` backend feature; its behaviour is untested here).
- **Snapshots:** 3D output will differ between SwiftShader, lavapipe and real GPUs. Use thresholds and prefer numeric assertions.
- **Korean IME:** correctness rests on winit 0.30.x workarounds. Manual tests are needed on Windows, macOS and Linux, including single-letter tool hotkeys while a Hangul layout is active (logical-key matching).
- **`view_formats` sRGB trick:** unverified on SwiftShader. The shader-side transfer function is the fallback.
- **Churn:** egui shipped four breaking minors in 2026 (0.34 to 0.37) and wgpu about one major per quarter. egui_dock, egui-snarl and transform-gizmo-egui still target 0.36.
- **Blocking readback:** `PollType::Wait` on the UI thread needs a timeout or deferred mapping.

## Surprises

- `App::update`, `SidePanel`, `TopBottomPanel`, `Context::run` and `InputState::raw_scroll_delta` are gone, and `Id::new` is deprecated. Most egui code written from memory will not compile on 0.37.
- eframe's default renderer is wgpu; glow is opt-in.
- egui now has a real text stack with system-font fallback, so Korean works without bundling on normal desktops but not in kittest.
- kittest does execute paint callbacks and native textures, but only into a 1×, depth-less pass, and only hands out a `RenderState` after `.wgpu()`.
- Rerun does not draw its scene inside egui's pass; it renders offscreen in `prepare` and composites in `paint`.
- wgpu 30: `@interpolate(flat)` is mandatory for integer varyings, `get_mapped_range()` returns `Result`, vertex buffer layouts are `Option`s, and `InstanceDescriptor` has no `Default`.
- The sandbox's Chromium SwiftShader ICD is a conformant Vulkan 1.3 CPU device reachable through the system loader.

Sources (web search): [egui PR #8083](https://github.com/emilk/egui/pull/8083), [onnxruntime commit replacing lavapipe with SwiftShader](https://app.semanticdiff.com/gh/microsoft/onnxruntime/commit/72cb0426bfa166a1c050a5a2ace8f202752c68ed), [Mozilla bug 1837027 (lavapipe)](https://bugzilla.mozilla.org/show_bug.cgi?id=1837027). winit PR #4478 was seen only through a mirror snippet.
