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
    /// The application's own folder (recent projects, automatic saves). `None` keeps nothing, which is
    /// what tests want; the application passes the system's settings folder.
    pub settings_dir: Option<PathBuf>,
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
    /// The mesh file to import, or `None` when the user cancelled.
    fn open_mesh(&mut self) -> Option<PathBuf> {
        None
    }
    /// The robot description file to import, or `None` when the user cancelled.
    fn open_urdf(&mut self) -> Option<PathBuf> {
        None
    }
}

struct NativeDialogs;

impl FileDialogs for NativeDialogs {
    fn open(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("GraphCTRL project", &["gctrl"]).pick_file()
    }

    fn save(&mut self, suggested_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("GraphCTRL project", &["gctrl"])
            .set_file_name(suggested_name)
            .save_file()
    }

    fn open_mesh(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("STL, COLLADA", &["stl", "STL", "dae", "DAE"]).pick_file()
    }

    fn open_urdf(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("URDF", &["urdf", "URDF", "xml"]).pick_file()
    }
}

/// Something that would discard unsaved changes and therefore waits for the user's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pending {
    New,
    /// Open the example cell.
    Example,
    Open,
    /// Open this project (from the recent list).
    OpenPath(PathBuf),
    Quit,
}

/// What ships with the application and under which licence, shown in Help > Licences.
const NOTICES: [(&str, &str); 3] = [
    ("GraphCTRL (MIT)", include_str!("../../../../LICENSE")),
    (
        "Jetendard font (SIL Open Font License 1.1)",
        include_str!("../../../../assets/fonts/LICENSE-Jetendard.txt"),
    ),
    (
        "Universal Robots ROS2 Description (BSD-3-Clause): Universal Robots A/S and contributors",
        include_str!("../../../../assets/ur/upstream/LICENSE"),
    ),
];

/// Unsaved work is saved automatically this often, in seconds.
const AUTOSAVE_SECONDS: f64 = 60.0;

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

/// The closed-form inverse kinematics plugins offer.
type Kinematics = Vec<std::sync::Arc<dyn gc_robotics_api::KinematicsFactory>>;

/// The robots plugins offer (name and description), a resolver for the files they refer to, and the
/// closed-form inverse kinematics they offer.
fn robot_library() -> (Vec<(String, String)>, gc_session::Resources, Kinematics) {
    let mut registry = gc_plugin_api::Registry::default();
    for plugin in builtin_plugins() {
        if let Err(error) = registry.load(plugin.as_ref()) {
            log::error!("plugin `{}` was not loaded: {error}", plugin.manifest().id);
        }
    }
    let kinematics = registry.get(gc_robotics_api::KINEMATICS_FACTORIES);
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
    (library, resources, kinematics)
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
    params: gc_panels::ParamsUi,
    /// The robots that can be added: name and description.
    library: Vec<(String, String)>,
    expression: String,
    ime: ImeTest,
    /// Whether the licence notices window is open.
    notices: bool,
    /// Whether the about window is open.
    about: bool,
    /// The pose calculator, when its window is open.
    pose_calc: Option<gc_panels::PoseCalc>,
    dialogs: Box<dyn FileDialogs>,
    /// The latest message about files, shown in the status bar.
    notice: Option<String>,
    pending: Option<Pending>,
    quitting: bool,
    recent: Vec<PathBuf>,
    /// An automatic save found at start, offered for recovery, and the project it belongs to.
    recovery: Option<(PathBuf, Option<PathBuf>)>,
    autosaved: bool,
    last_autosave: f64,
    title: String,
}

impl GraphCtrlApp {
    /// Creates the app: installs fonts and theme and captures the wgpu render state when there is one.
    pub fn new(cc: &eframe::CreationContext<'_>, options: RunOptions) -> Self {
        gc_ui::fonts::install(&cc.egui_ctx);
        gc_ui::install_theme(&cc.egui_ctx);
        let (library, resources, kinematics) = robot_library();
        let recent = options.settings_dir.as_deref().map(gc_persist::places::load_recent).unwrap_or_default();
        let recovery = options.settings_dir.as_deref().and_then(gc_persist::places::pending_autosave);
        // The language chosen last time, when there are saved settings.
        let lang = options
            .settings_dir
            .as_deref()
            .map(gc_persist::places::load_settings)
            .and_then(|settings| Lang::from_code(&settings.language))
            .unwrap_or_default();
        let mut session = Session::new();
        session.set_resources(resources);
        session.set_kinematics(kinematics);
        if options.background_jobs {
            let notifier = std::sync::Arc::new(Repaint(cc.egui_ctx.clone()));
            session.set_executor(gc_jobs::Executor::thread().with_notifier(notifier));
        }
        Self {
            options,
            frames: 0,
            session,
            viewport: Viewport::new(cc.wgpu_render_state.clone()),
            lang,
            tab: Tab::Model,
            cell: gc_panels::CellUi::default(),
            params: gc_panels::ParamsUi::default(),
            library,
            expression: String::new(),
            ime: ImeTest { dimension: "25 mm".to_owned(), ..ImeTest::default() },
            notices: false,
            about: false,
            pose_calc: None,
            dialogs: Box::new(NativeDialogs),
            notice: None,
            pending: None,
            quitting: false,
            recent,
            recovery,
            autosaved: false,
            last_autosave: 0.0,
            title: String::new(),
        }
    }

    /// The viewport, for inspecting what it drew.
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Opens the in-place editor of a dimension, as a double click on its text does.
    pub fn edit_dimension(&mut self, id: gc_core::Id<gc_sketch::Dimension>) {
        self.viewport.edit_dimension(&self.session, id);
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
                self.remember(&path);
                true
            }
            Err(error) => {
                self.notice = Some(tr!(lang, FileError, error));
                false
            }
        }
    }

    /// Puts a project first in the recent list.
    fn remember(&mut self, path: &std::path::Path) {
        if let Some(dir) = &self.options.settings_dir {
            match gc_persist::places::remember_recent(dir, path) {
                Ok(list) => self.recent = list,
                Err(error) => log::warn!("the recent projects could not be written: {error}"),
            }
        }
    }

    /// The recent projects, newest first.
    pub fn recent(&self) -> &[PathBuf] {
        &self.recent
    }

    /// Saves unsaved work to the application's folder (done every minute on its own), or removes the
    /// automatic save once nothing is unsaved.
    pub fn autosave(&mut self) {
        let Some(dir) = self.options.settings_dir.clone() else { return };
        if self.session.is_dirty() {
            match gc_persist::places::write_autosave(&dir, self.session.doc(), self.session.file_path()) {
                Ok(()) => self.autosaved = true,
                Err(error) => log::warn!("the automatic save failed: {error}"),
            }
        } else if self.autosaved {
            gc_persist::places::clear_autosave(&dir);
            self.autosaved = false;
        }
    }

    /// The question at start when a session that did not end normally left unsaved work.
    fn recover_question(&mut self, ctx: &egui::Context) {
        let Some((autosave, project)) = self.recovery.clone() else { return };
        let lang = self.lang;
        let mut answer = None;
        egui::Window::new(tr!(lang, AppTitle))
            .id(egui::Id::unique("gc_recover"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr!(lang, AskRecover));
                if let Some(project) = &project {
                    ui.weak(project.display().to_string());
                }
                ui.horizontal(|ui| {
                    if ui.button(tr!(lang, BtnRecover)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(tr!(lang, BtnDiscard)).clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.recovery = None;
                match self.session.open_recovered(&autosave, project.as_deref()) {
                    Ok(()) => {
                        let named = project.map(|p| p.display().to_string()).unwrap_or_default();
                        self.notice = Some(tr!(lang, FileRecovered, named));
                        self.autosaved = true;
                    }
                    Err(error) => self.notice = Some(tr!(lang, FileError, error)),
                }
            }
            Some(false) => {
                self.recovery = None;
                if let Some(dir) = &self.options.settings_dir {
                    gc_persist::places::clear_autosave(dir);
                }
            }
            None => {}
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
            Pending::Example => {
                let urdf =
                    self.library.iter().find(|(name, _)| name.contains("UR5e")).map(|(_, urdf)| urdf.clone());
                let opened = urdf
                    .ok_or_else(|| "UR5e".to_owned())
                    .and_then(|urdf| crate::example::open(&mut self.session, &urdf));
                self.tab = Tab::Robot;
                self.notice = opened.err().map(|error| tr!(lang, ExampleFailed, error));
                self.viewport.look(StandardView::Iso);
                self.viewport.fit(&self.session, [1000.0, 700.0]);
            }
            Pending::Open | Pending::OpenPath(_) => {
                let path = match action {
                    Pending::OpenPath(path) => path,
                    _ => match self.dialogs.open() {
                        Some(path) => path,
                        None => return,
                    },
                };
                match self.session.open(&path) {
                    Ok(needs_repair) => {
                        self.tab = Tab::Model;
                        self.notice = (!needs_repair.is_empty())
                            .then(|| tr!(lang, FileNeedsRepair, needs_repair.len()));
                        self.remember(&path);
                    }
                    Err(error) => self.notice = Some(tr!(lang, FileError, error)),
                }
            }
            Pending::Quit => {
                self.quitting = true;
                // Leaving on purpose: nothing to recover next time.
                if let Some(dir) = &self.options.settings_dir {
                    gc_persist::places::clear_autosave(dir);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// The question shown before unsaved changes would be lost.
    fn unsaved_question(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending.clone() else { return };
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
                egui::Key::L | egui::Key::C | egui::Key::A | egui::Key::R | egui::Key::D if !command => {
                    let (tool, name) = match key {
                        egui::Key::L => (ToolKind::Line, "L"),
                        egui::Key::C => (ToolKind::Circle, "C"),
                        egui::Key::A => (ToolKind::Arc, "A"),
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
                if ui.button(tr!(lang, CmdExample)).clicked() {
                    self.request(Pending::Example, &ctx);
                }
                if ui.button(tr!(lang, CmdSave)).clicked() {
                    self.save(false);
                }
                if ui.button(tr!(lang, CmdSaveAs)).clicked() {
                    self.save(true);
                }
                let recent = self.recent.clone();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button(tr!(lang, MenuRecent), |ui| {
                        for path in recent {
                            let name = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            if ui.button(&name).on_hover_text(path.display().to_string()).clicked() {
                                self.request(Pending::OpenPath(path), &ctx);
                            }
                        }
                    });
                });
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
            ui.menu_button(tr!(lang, MenuHelp), |ui| {
                if ui.button(tr!(lang, MenuAbout)).clicked() {
                    self.about = true;
                }
                if ui.button(tr!(lang, MenuLicences)).clicked() {
                    self.notices = true;
                }
            });
            ui.menu_button(tr!(lang, MenuTools), |ui| {
                if ui.button(tr!(lang, MenuPoseCalc)).clicked() {
                    self.pose_calc.get_or_insert_with(gc_panels::PoseCalc::default);
                }
                if ui.button(tr!(lang, MenuImeTest)).clicked() {
                    self.ime.open = true;
                }
                ui.menu_button(tr!(lang, MenuLanguage), |ui| {
                    for choice in Lang::ALL {
                        if ui.selectable_label(self.lang == choice, choice.native_name()).clicked() {
                            self.lang = choice;
                            self.save_settings();
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

    /// Saves the settings that outlive a run (the language). A failure is shown in the status bar.
    fn save_settings(&mut self) {
        let Some(dir) = &self.options.settings_dir else { return };
        let settings =
            gc_persist::places::Settings { language: self.lang.code().to_owned(), ..Default::default() };
        if let Err(error) = gc_persist::places::save_settings(dir, &settings) {
            self.notice = Some(tr!(self.lang, FileError, error));
        }
    }

    /// The version and where the application keeps its settings and recovery files.
    fn about_window(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        let mut open = self.about;
        egui::Window::new(tr!(lang, MenuAbout)).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label(egui::RichText::new("GraphCTRL").size(18.0).strong());
            ui.label(tr!(lang, AboutVersion, env!("CARGO_PKG_VERSION")));
            ui.add_space(6.0);
            match &self.options.settings_dir {
                Some(dir) => {
                    ui.label(tr!(lang, AboutSettings, gc_persist::places::settings_file(dir).display()));
                    ui.label(tr!(lang, AboutRecovery, dir.join("recovery").display()));
                }
                None => {
                    ui.label(tr!(lang, AboutNoSettings));
                }
            }
        });
        self.about = open;
    }

    /// The licences of what ships with the application: its own, the bundled font's and the UR robot
    /// descriptions'.
    fn notices_window(&mut self, ctx: &egui::Context) {
        let mut open = self.notices;
        egui::Window::new(tr!(self.lang, MenuLicences)).open(&mut open).default_size([560.0, 480.0]).show(
            ctx,
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (title, text) in NOTICES {
                        egui::CollapsingHeader::new(title).show(ui, |ui| {
                            ui.label(egui::RichText::new(text).monospace().size(11.0));
                        });
                    }
                });
            },
        );
        self.notices = open;
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
        // Unsaved work is kept safe every minute; once saved, the copy goes.
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave >= AUTOSAVE_SECONDS || (self.autosaved && !self.session.is_dirty()) {
            self.last_autosave = now;
            self.autosave();
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        let mut ribbon_action = None;
        egui::Panel::top("ribbon").show(ui, |ui| {
            ribbon_action = ribbon::show(
                ui,
                &mut self.tab,
                &mut self.session,
                &mut self.viewport,
                &mut self.cell,
                &self.library,
                lang,
            );
        });
        if ribbon_action == Some(ribbon::RibbonAction::ImportRobot)
            && let Some(path) = self.dialogs.open_urdf()
        {
            match self.session.add_robot_file(&path) {
                Ok(robot) => {
                    let missing = self.session.robot_model(robot).map_or(0, |loaded| loaded.missing.len());
                    self.notice = (missing > 0).then(|| tr!(lang, RobotMeshesMissing, missing));
                    self.viewport.fit(&self.session, [1000.0, 700.0]);
                }
                Err(error) => self.notice = Some(tr!(lang, FileError, error)),
            }
        }
        if let Some(robot) = self.cell.tool_mesh.take()
            && let Some(path) = self.dialogs.open_mesh()
        {
            self.notice =
                self.session.set_tool_mesh(robot, &path).err().map(|error| tr!(lang, FileError, error));
        }
        if ribbon_action == Some(ribbon::RibbonAction::ImportMesh)
            && let Some(path) = self.dialogs.open_mesh()
        {
            match self.session.add_mesh_body(&path) {
                Ok(_) => {
                    self.notice = None;
                    self.viewport.fit(&self.session, [1000.0, 700.0]);
                }
                Err(error) => self.notice = Some(tr!(lang, FileError, error)),
            }
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            gc_panels::status_bar(
                ui,
                &self.session,
                lang,
                self.viewport.background().has_gpu(),
                self.notice.as_deref(),
            );
        });
        // The side panels keep their width: the user can drag it within a range, content never widens it.
        egui::Panel::left("browser").default_size(210.0).size_range(180.0..=360.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match gc_panels::browser(ui, &self.session, lang, &mut self.params) {
                    Some(BrowserAction::EditSketch(id)) => {
                        self.session.enter_sketch(id);
                        self.tab = Tab::Sketch;
                    }
                    // A refused edit (still in use, bad expression) is explained in the status bar.
                    Some(BrowserAction::Edit(command)) => {
                        self.cell.playing = false;
                        if self.session.execute(&command).is_ok()
                            && matches!(*command, gc_doc::Command::AddParam { .. })
                        {
                            self.params = gc_panels::ParamsUi::default();
                        }
                    }
                    Some(BrowserAction::Select(item)) => self.session.select_item(item),
                    Some(BrowserAction::Plan(path)) => {
                        self.cell.playing = false;
                        self.session.plan(path);
                    }
                    Some(BrowserAction::Show(item, visible)) => {
                        if let Some(look) = self.session.shown_look(item) {
                            drop(self.session.set_look(item, gc_doc::Look { hidden: !visible, ..look }));
                        }
                    }
                    None => {}
                }
            });
        });
        egui::Panel::right("properties").default_size(330.0).size_range(280.0..=520.0).show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            egui::ScrollArea::vertical().show(ui, |ui| {
                gc_panels::properties(ui, &mut self.session, lang, &mut self.expression);
                gc_panels::cell(ui, &mut self.session, lang, &mut self.cell);
            });
        });
        egui::CentralPanel::no_frame().show(ui, |ui| self.viewport.ui(ui, &mut self.session, lang));
        self.reference_question(&ctx);
        self.unsaved_question(&ctx);
        self.recover_question(&ctx);
        if self.ime.open {
            self.ime_window(&ctx);
        }
        if self.notices {
            self.notices_window(&ctx);
        }
        if self.about {
            self.about_window(&ctx);
        }
        if let Some(mut calc) = self.pose_calc.take() {
            let mut open = true;
            egui::Window::new(tr!(lang, MenuPoseCalc)).open(&mut open).default_width(460.0).show(
                &ctx,
                |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| gc_panels::pose_calculator(ui, &self.session, lang, &mut calc));
                },
            );
            if open {
                self.pose_calc = Some(calc);
            }
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
