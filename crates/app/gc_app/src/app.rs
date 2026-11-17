//! The application window.

use std::path::PathBuf;

use eframe::egui;
use gc_draw::StandardView;
use gc_i18n::{Lang, tr};
use gc_panels::BrowserAction;
use gc_session::Session;
use gc_tools::ToolKind;
use gc_viewport::Viewport;

use crate::ribbon::{self, Tab};

/// Command-line driven options.
#[derive(Debug, Default, Clone)]
pub struct RunOptions {
    /// Close the window after this many frames (smoke test).
    pub smoke_frames: Option<u32>,
    /// Write the 3D target to this PNG before closing a smoke run.
    pub screenshot: Option<PathBuf>,
    /// Plan on a worker thread so that the window stays responsive. Off, planning finishes inside the
    /// command that starts it, which is what scripted tests want.
    pub background_jobs: bool,
}

/// Wakes the window when a background job has finished.
struct Repaint(egui::Context);

impl gc_jobs::Notifier for Repaint {
    fn notify(&self) {
        self.0.request_repaint();
    }
}

/// Asks the user for file names. The application uses the operating system's dialogs; tests script the
/// answers.
pub trait FileDialogs {
    /// The file to open, or `None` when the user cancelled.
    fn open(&mut self) -> Option<PathBuf>;
    /// The file to save to, or `None` when the user cancelled.
    fn save(&mut self, suggested_name: &str) -> Option<PathBuf>;
}

struct NativeDialogs;

impl FileDialogs for NativeDialogs {
    fn open(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("GraphCTRL", &["gctrl"]).pick_file()
    }

    fn save(&mut self, suggested_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("GraphCTRL", &["gctrl"]).set_file_name(suggested_name).save_file()
    }
}

/// Something that would discard unsaved changes and therefore waits for the user's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    New,
    Open,
    Quit,
}

/// The fields of the Korean input test window.
#[derive(Default)]
struct ImeTest {
    open: bool,
    name: String,
    dimension: String,
    last_shortcut: String,
}

/// The built-in plugins. Their contributions reach the application only through the registry, the same
/// way a third-party plugin's would.
fn builtin_plugins() -> Vec<Box<dyn gc_plugin_api::Plugin>> {
    vec![Box::new(gc_plugin_ur::UrPlugin)]
}

/// The bundled sample robot: a six-axis arm described with basic solids.
const SAMPLE_ROBOT: &str = include_str!("../../../../fixtures/urdf/six_axis.urdf");

/// The robots plugins offer (name and description) and a resolver for the files they refer to.
fn robot_library() -> (Vec<(String, String)>, gc_session::Resources) {
    let mut registry = gc_plugin_api::Registry::default();
    for plugin in builtin_plugins() {
        if let Err(error) = registry.load(plugin.as_ref()) {
            log::error!("plugin `{}` was not loaded: {error}", plugin.manifest().id);
        }
    }
    let providers = registry.get(gc_robotics_api::ROBOT_MODEL_PROVIDERS);
    let mut library = Vec::new();
    let mut models = Vec::new();
    for provider in &providers {
        for info in provider.catalog() {
            match provider.urdf(&info.id) {
                Ok(urdf) => {
                    library.push((info.name.clone(), urdf));
                    models.push((std::sync::Arc::clone(provider), info.id));
                }
                Err(error) => log::error!("robot `{}` is not available: {error}", info.id),
            }
        }
    }
    library.push(("6R".to_owned(), SAMPLE_ROBOT.to_owned()));
    let resources: gc_session::Resources = std::sync::Arc::new(move |uri: &str| {
        models.iter().find_map(|(provider, model)| provider.resource(model, uri).ok())
    });
    (library, resources)
}

/// The application state.
pub struct GraphCtrlApp {
    options: RunOptions,
    frames: u32,
    /// The document and everything that edits it.
    pub session: Session,
    viewport: Viewport,
    lang: Lang,
    tab: Tab,
    cell: gc_panels::CellUi,
    /// The robots that can be added: name and description.
    library: Vec<(String, String)>,
    expression: String,
    ime: ImeTest,
    dialogs: Box<dyn FileDialogs>,
    /// The latest message about files, shown in the status bar.
    notice: Option<String>,
    pending: Option<Pending>,
    quitting: bool,
    title: String,
}

impl GraphCtrlApp {
    /// Creates the app: installs fonts and theme and captures the wgpu render state when there is one.
    pub fn new(cc: &eframe::CreationContext<'_>, options: RunOptions) -> Self {
        gc_ui::fonts::install(&cc.egui_ctx);
        gc_ui::install_theme(&cc.egui_ctx);
        let (library, resources) = robot_library();
        let mut session = Session::new();
        session.set_resources(resources);
        if options.background_jobs {
            let notifier = std::sync::Arc::new(Repaint(cc.egui_ctx.clone()));
            session.set_executor(gc_jobs::Executor::thread().with_notifier(notifier));
        }
        Self {
            options,
            frames: 0,
            session,
            viewport: Viewport::new(cc.wgpu_render_state.clone()),
            lang: Lang::default(),
            tab: Tab::Model,
            cell: gc_panels::CellUi::default(),
            library,
            expression: String::new(),
            ime: ImeTest { dimension: "25 mm".to_owned(), ..ImeTest::default() },
            dialogs: Box::new(NativeDialogs),
            notice: None,
            pending: None,
            quitting: false,
            title: String::new(),
        }
    }

    /// The viewport, for inspecting what it drew.
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Replaces the file dialogs, so that tests can answer them.
    #[must_use]
    pub fn with_dialogs(mut self, dialogs: Box<dyn FileDialogs>) -> Self {
        self.dialogs = dialogs;
        self
    }

    /// Saves the document, asking for a file name when it has none or when `choose` is set.
    /// Returns whether it was saved.
    pub fn save(&mut self, choose: bool) -> bool {
        let lang = self.lang;
        let path = match self.session.file_path() {
            Some(path) if !choose => Some(path.to_path_buf()),
            _ => self.dialogs.save("Untitled.gctrl"),
        };
        let Some(path) = path else { return false };
        match self.session.save_as(&path) {
            Ok(()) => {
                self.notice = Some(tr!(lang, FileSaved, path.display()));
                true
            }
            Err(error) => {
                self.notice = Some(tr!(lang, FileError, error));
                false
            }
        }
    }

    /// Starts an action that discards the current document; asks first when there are unsaved changes.
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        if self.session.is_dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }

    fn perform(&mut self, action: Pending, ctx: &egui::Context) {
        let lang = self.lang;
        match action {
            Pending::New => {
                self.session.new_document();
                self.tab = Tab::Model;
                self.notice = None;
            }
            Pending::Open => {
                let Some(path) = self.dialogs.open() else { return };
                match self.session.open(&path) {
                    Ok(needs_repair) => {
                        self.tab = Tab::Model;
                        self.notice = (!needs_repair.is_empty())
                            .then(|| tr!(lang, FileNeedsRepair, needs_repair.len()));
                    }
                    Err(error) => self.notice = Some(tr!(lang, FileError, error)),
                }
            }
            Pending::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// The question shown before unsaved changes would be lost.
    fn unsaved_question(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending else { return };
        let lang = self.lang;
        let mut answer = None;
        egui::Window::new(tr!(lang, AppTitle))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr!(lang, AskSaveChanges));
                ui.horizontal(|ui| {
                    if ui.button(tr!(lang, CmdSave)).clicked() {
                        answer = Some(Some(true));
                    }
                    if ui.button(tr!(lang, BtnDiscard)).clicked() {
                        answer = Some(Some(false));
                    }
                    if ui.button(tr!(lang, Cancel)).clicked() {
                        answer = Some(None);
                    }
                });
            });
        match answer {
            Some(Some(true)) => {
                self.pending = None;
                if self.save(false) {
                    self.perform(action, ctx);
                }
            }
            Some(Some(false)) => {
                self.pending = None;
                self.perform(action, ctx);
            }
            Some(None) => self.pending = None,
            None => {}
        }
    }

    /// Keeps the window title in step with the file name and the unsaved marker.
    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .session
            .file_path()
            .and_then(|p| p.file_name())
            .map_or_else(|| tr!(self.lang, Untitled).to_owned(), |n| n.to_string_lossy().into_owned());
        let title = format!("{name}{} - GraphCTRL", if self.session.is_dirty() { "*" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Single-key tool shortcuts, matched by physical key so they work while a Hangul layout is active.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key { key, physical_key, pressed: true, repeat: false, modifiers } => {
                        Some((physical_key.unwrap_or(*key), *modifiers))
                    }
                    _ => None,
                })
                .collect()
        });
        for (key, modifiers) in pressed {
            let command = modifiers.command || modifiers.ctrl;
            let sketching = self.session.active_sketch().is_some();
            match key {
                egui::Key::Z if command => {
                    self.session.undo();
                }
                egui::Key::S if command => {
                    self.save(modifiers.shift);
                }
                egui::Key::N if command => self.request(Pending::New, ctx),
                egui::Key::O if command => self.request(Pending::Open, ctx),
                egui::Key::Y if command => {
                    self.session.redo();
                }
                egui::Key::F6 => self.viewport.look(StandardView::Iso),
                egui::Key::L | egui::Key::C | egui::Key::R | egui::Key::D if !command => {
                    let (tool, name) = match key {
                        egui::Key::L => (ToolKind::Line, "L"),
                        egui::Key::C => (ToolKind::Circle, "C"),
                        egui::Key::R => (ToolKind::Rectangle, "R"),
                        _ => (ToolKind::Dimension, "D"),
                    };
                    name.clone_into(&mut self.ime.last_shortcut);
                    if sketching {
                        self.session.set_tool(tool);
                        self.tab = Tab::Sketch;
                    }
                }
                _ => {}
            }
        }
        self.viewport.keys(ctx, &mut self.session);
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr!(lang, MenuFile), |ui| {
                let ctx = ui.ctx().clone();
                if ui.button(tr!(lang, CmdNew)).clicked() {
                    self.request(Pending::New, &ctx);
                }
                if ui.button(tr!(lang, CmdOpen)).clicked() {
                    self.request(Pending::Open, &ctx);
                }
                if ui.button(tr!(lang, CmdSave)).clicked() {
                    self.save(false);
                }
                if ui.button(tr!(lang, CmdSaveAs)).clicked() {
                    self.save(true);
                }
                ui.separator();
                if ui.button(tr!(lang, CmdQuit)).clicked() {
                    self.request(Pending::Quit, &ctx);
                }
            });
            ui.menu_button(tr!(lang, MenuEdit), |ui| {
                let (can_undo, can_redo) =
                    (self.session.history().undo_depth() > 0, self.session.history().redo_depth() > 0);
                if ui.add_enabled(can_undo, egui::Button::new(tr!(lang, CmdUndo))).clicked() {
                    self.session.undo();
                }
                if ui.add_enabled(can_redo, egui::Button::new(tr!(lang, CmdRedo))).clicked() {
                    self.session.redo();
                }
            });
            ui.menu_button(tr!(lang, MenuTools), |ui| {
                if ui.button(tr!(lang, MenuImeTest)).clicked() {
                    self.ime.open = true;
                }
                ui.menu_button(tr!(lang, MenuLanguage), |ui| {
                    for choice in Lang::ALL {
                        if ui.selectable_label(self.lang == choice, choice.native_name()).clicked() {
                            self.lang = choice;
                        }
                    }
                });
            });
        });
    }

    /// The window that asks whether an over-constraining dimension should become a reference dimension.
    fn reference_question(&mut self, ctx: &egui::Context) {
        if self.session.reference_offer().is_none() {
            return;
        }
        let lang = self.lang;
        let mut answer = None;
        egui::Window::new(tr!(lang, ToolDimension))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr!(lang, AskReference));
                ui.horizontal(|ui| {
                    if ui.button(tr!(lang, Ok)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(tr!(lang, Cancel)).clicked() {
                        answer = Some(false);
                    }
                });
            });
        if let Some(accept) = answer {
            self.session.answer_reference_offer(accept);
        }
    }

    /// The checklist window for testing Korean text input on the user's machine.
    fn ime_window(&mut self, ctx: &egui::Context) {
        let mut open = self.ime.open;
        egui::Window::new(tr!(self.lang, MenuImeTest)).open(&mut open).show(ctx, |ui| {
            ui.label("1. 이름 칸에 한글을 입력합니다 (조합 중 글자가 제자리에 보이는지).");
            ui.horizontal(|ui| {
                let label = ui.label("이름");
                ui.text_edit_singleline(&mut self.ime.name).labelled_by(label.id);
            });
            ui.label("2. 치수 칸에 '25 mm' 처럼 숫자와 단위를 입력합니다.");
            ui.horizontal(|ui| {
                let label = ui.label("치수");
                ui.text_edit_singleline(&mut self.ime.dimension).labelled_by(label.id);
            });
            ui.label("3. 한글 자판 상태로 뷰포트를 클릭한 뒤 L, C, D 키를 누릅니다.");
            ui.label(format!("마지막 단축키: {}", self.ime.last_shortcut));
        });
        self.ime.open = open;
    }

    fn finish_smoke_run(&mut self, ctx: &egui::Context) {
        if let (Some(path), Some((rs, target))) =
            (self.options.screenshot.as_ref(), self.viewport.background().target())
        {
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
        let lang = self.lang;
        self.shortcuts(&ctx);
        self.session.poll_jobs();
        // Closing the window with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested()) && self.session.is_dirty() && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        self.update_title(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::top("ribbon").show(ui, |ui| {
            ribbon::show(
                ui,
                &mut self.tab,
                &mut self.session,
                &mut self.viewport,
                &mut self.cell,
                &self.library,
                lang,
            );
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            gc_panels::status_bar(
                ui,
                &self.session,
                lang,
                self.viewport.background().has_gpu(),
                self.notice.as_deref(),
            );
        });
        egui::Panel::left("browser").show(ui, |ui| {
            ui.set_min_width(190.0);
            if let Some(BrowserAction::EditSketch(id)) = gc_panels::browser(ui, &self.session, lang) {
                self.session.enter_sketch(id);
                self.tab = Tab::Sketch;
            }
        });
        egui::Panel::right("properties").show(ui, |ui| {
            ui.set_min_width(250.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                gc_panels::properties(ui, &mut self.session, lang, &mut self.expression);
                gc_panels::cell(ui, &mut self.session, lang, &mut self.cell);
            });
        });
        egui::CentralPanel::no_frame().show(ui, |ui| self.viewport.ui(ui, &mut self.session, lang));
        self.reference_question(&ctx);
        self.unsaved_question(&ctx);
        if self.ime.open {
            self.ime_window(&ctx);
        }

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
