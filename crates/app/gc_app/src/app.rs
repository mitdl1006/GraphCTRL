//! Stage 0 application shell: menu bar, Korean input test panel and the viewport placeholder.

use std::path::PathBuf;

use eframe::egui;

use crate::viewport_stub::ViewportStub;

/// Command-line driven options.
#[derive(Debug, Default, Clone)]
pub struct RunOptions {
    /// Close the window after this many frames (smoke test).
    pub smoke_frames: Option<u32>,
    /// Write the 3D target to this PNG before closing a smoke run.
    pub screenshot: Option<PathBuf>,
}

/// The application state.
pub struct GraphCtrlApp {
    options: RunOptions,
    frames: u32,
    viewport: ViewportStub,
    name_field: String,
    dimension_field: String,
    last_shortcut: String,
}

impl GraphCtrlApp {
    /// Creates the app: installs fonts and captures the wgpu render state when there is one.
    pub fn new(cc: &eframe::CreationContext<'_>, options: RunOptions) -> Self {
        gc_ui::fonts::install(&cc.egui_ctx);
        Self {
            options,
            frames: 0,
            viewport: ViewportStub::new(cc.wgpu_render_state.clone()),
            name_field: String::new(),
            dimension_field: "25 mm".to_owned(),
            last_shortcut: String::new(),
        }
    }

    fn ime_test_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("한글 입력 시험");
        ui.label("1. 이름 칸에 한글을 입력합니다 (조합 중 글자가 제자리에 보이는지).");
        ui.horizontal(|ui| {
            let label = ui.label("이름");
            ui.text_edit_singleline(&mut self.name_field).labelled_by(label.id);
        });
        ui.label("2. 치수 칸에 '25 mm' 처럼 숫자와 단위를 입력합니다.");
        ui.horizontal(|ui| {
            let label = ui.label("치수");
            ui.text_edit_singleline(&mut self.dimension_field).labelled_by(label.id);
        });
        ui.label("3. 한글 자판 상태로 뷰포트를 클릭한 뒤 L, C, D 키를 누릅니다.");
        ui.label(format!("마지막 단축키: {}", self.last_shortcut));
        ui.separator();
        ui.label(if self.viewport.has_gpu() { "GPU: 사용 중" } else { "GPU: 없음 (자리표시)" });
        ui.label(format!("프레임: {}", self.frames));
    }

    /// Records single-letter tool shortcuts by physical key so they work while a Hangul layout is active.
    fn poll_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key { key, physical_key, pressed: true, repeat: false, .. } => {
                    Some(physical_key.unwrap_or(*key))
                }
                _ => None,
            })
        });
        if let Some(key) = pressed {
            let tool = match key {
                egui::Key::L => Some("L — 선"),
                egui::Key::C => Some("C — 원"),
                egui::Key::D => Some("D — 치수"),
                _ => None,
            };
            if let Some(tool) = tool {
                self.last_shortcut = tool.to_owned();
            }
        }
    }

    fn finish_smoke_run(&mut self, ctx: &egui::Context) {
        if let (Some(path), Some((rs, target))) = (self.options.screenshot.as_ref(), self.viewport.target()) {
            let (width, height) = target.size();
            match gc_render::read_rgba(&rs.device, &rs.queue, target) {
                Ok(pixels) => {
                    if let Err(err) =
                        image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
                    {
                        log::error!("screenshot could not be written: {err}");
                    }
                }
                Err(err) => log::error!("screenshot readback failed: {err}"),
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for GraphCtrlApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frames = self.frames.saturating_add(1);
        let ctx = ui.ctx().clone();
        self.poll_shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("파일", |ui| {
                    if ui.button("끝내기").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label("준비");
        });
        egui::Panel::left("ime_test").show(ui, |ui| self.ime_test_panel(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.viewport.ui(ui));

        if let Some(limit) = self.options.smoke_frames {
            if self.frames >= limit {
                self.finish_smoke_run(&ctx);
            } else {
                ctx.request_repaint();
            }
        }
    }
}

/// Runs the desktop application until the window closes.
pub fn run(options: RunOptions) -> anyhow::Result<()> {
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GraphCTRL")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("graphctrl", native, Box::new(move |cc| Ok(Box::new(GraphCtrlApp::new(cc, options)))))
        .map_err(|err| anyhow::anyhow!("the application window could not be started: {err}"))
}
