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
    /// The robot library folder. `None` looks for a `robots` folder beside the executable or the working
    /// folder, or up to four folders above them.
    pub robot_dir: Option<PathBuf>,
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
    /// Where to save a CSV file, or `None` when the user cancelled.
    fn save_csv(&mut self, _suggested_name: &str) -> Option<PathBuf> {
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
        rfd::FileDialog::new()
            .add_filter(
                "STL, COLLADA, glTF, STEP",
                &["stl", "STL", "dae", "DAE", "glb", "GLB", "gltf", "GLTF", "stp", "STP", "step", "STEP"],
            )
            .pick_file()
    }

    fn open_urdf(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("URDF", &["urdf", "URDF", "xml"]).pick_file()
    }

    fn save_csv(&mut self, suggested_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name(suggested_name).save_file()
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

/// What is built into the program and under which licence. The robot library's packages add their own
/// licences at run time.
const BUILT_IN_NOTICES: [(&str, &str); 2] = [
    ("GraphCTRL (MIT)", include_str!("../../../../LICENSE")),
    (
        "Jetendard font (SIL Open Font License 1.1)",
        include_str!("../../../../assets/fonts/LICENSE-Jetendard.txt"),
    ),
];

/// The notices shown in Help > Licences: what is built in, then each licence the library's packages
/// carry (once per text).
fn notices(packages: &[gc_session::library::Package]) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> =
        BUILT_IN_NOTICES.iter().map(|(title, text)| ((*title).to_owned(), (*text).to_owned())).collect();
    for package in packages {
        let Some(text) = package.license_text() else { continue };
        if all.iter().any(|(_, known)| *known == text) {
            continue;
        }
        let info = &package.info;
        let who = info.attribution.clone().unwrap_or_else(|| info.vendor.clone());
        let licence = info.license_name.clone().unwrap_or_default();
        all.push((format!("{who} ({licence})"), text));
    }
    all
}

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
    vec![Box::new(gc_plugin_ur::UrPlugin), Box::new(gc_plugin_planners::PlannersPlugin)]
}

/// What plugins offer the session besides robots: closed-form inverse kinematics and motion planners.
struct Kinematics {
    factories: Vec<std::sync::Arc<dyn gc_robotics_api::KinematicsFactory>>,
    planners: Vec<std::sync::Arc<dyn gc_robotics_api::MotionPlanner>>,
}

/// A robot the user can add: a package of the robot library, or one a plugin offers.
pub(crate) enum LibraryRobot {
    /// A package folder of the robot library.
    Package(gc_session::library::Package),
    /// A robot a plugin offers, with its description.
    Plugin {
        /// The name in the list.
        name: String,
        /// Its description.
        urdf: String,
    },
}

impl LibraryRobot {
    /// The name in the list.
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Package(package) => &package.info.name,
            Self::Plugin { name, .. } => name,
        }
    }

    /// Adds the robot to the session's cell.
    pub(crate) fn add(&self, session: &mut Session) -> Result<gc_core::Id<gc_doc::Robot>, String> {
        match self {
            Self::Package(package) => session.add_library_robot(package),
            Self::Plugin { name, urdf } => {
                let name = format!("{name}_{}", session.doc().robots().len() + 1);
                session.add_robot(&name, urdf)
            }
        }
    }
}

/// The robot library folder: the given one, else a `robots` folder found from the working folder or
/// the executable's folder.
fn library_dir(given: Option<&std::path::Path>) -> Option<PathBuf> {
    if let Some(dir) = given {
        return Some(dir.to_path_buf());
    }
    let starts: Vec<PathBuf> = [
        std::env::current_dir().ok(),
        std::env::current_exe().ok().and_then(|exe| exe.parent().map(std::path::Path::to_path_buf)),
    ]
    .into_iter()
    .flatten()
    .collect();
    gc_session::library::find(&starts)
}

/// The robots the user can add (the library's packages, then what plugins offer), a resolver for the
/// files plugin robots refer to, and the closed-form inverse kinematics plugins offer.
fn robot_library(
    packages: &[gc_session::library::Package],
) -> (Vec<LibraryRobot>, gc_session::Resources, Kinematics) {
    let mut registry = gc_plugin_api::Registry::default();
    for plugin in builtin_plugins() {
        if let Err(error) = registry.load(plugin.as_ref()) {
            log::error!("plugin `{}` was not loaded: {error}", plugin.manifest().id);
        }
    }
    let kinematics = Kinematics {
        factories: registry.get(gc_robotics_api::KINEMATICS_FACTORIES),
        planners: registry.get(gc_robotics_api::MOTION_PLANNERS),
    };
    let providers = registry.get(gc_robotics_api::ROBOT_MODEL_PROVIDERS);
    let mut library: Vec<LibraryRobot> = packages.iter().cloned().map(LibraryRobot::Package).collect();
    let mut models = Vec::new();
    for provider in &providers {
        for info in provider.catalog() {
            match provider.urdf(&info.id) {
                Ok(urdf) => {
                    library.push(LibraryRobot::Plugin { name: info.name.clone(), urdf });
                    models.push((std::sync::Arc::clone(provider), info.id));
                }
                Err(error) => log::error!("robot `{}` is not available: {error}", info.id),
            }
        }
    }
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
    library: Vec<LibraryRobot>,
    /// What Help > Licences shows.
    licences: Vec<(String, String)>,
    /// The robot library's packages, for agents that add robots by name.
    packages: Vec<gc_session::library::Package>,
    /// The AI agent connection.
    agent: crate::agent::Agent,
    /// The Measure panel.
    measure: crate::measure::Measure,
    /// The Analyze Interference window.
    interference: crate::interference::Interference,
    /// The Drive dialog.
    drive: crate::drive::Drive,
    /// The Constrain dialog.
    constrain: crate::constrain::Constrain,
    assemble: crate::assemble::Assemble,
    work_feature: crate::work_feature::WorkFeatureDialog,
    conflict: crate::conflict::ConstraintError,
    /// The Joint dialog.
    joint: crate::constrain::Joint,
    pattern: crate::pattern::Pattern,
    replicate: crate::replicate::Replicate,
    /// The Parameters dialog.
    parameters: crate::parameters::Parameters,
    /// The Bill of Materials dialog.
    bom: crate::bom::BomDialog,
    /// The iProperties dialog.
    iproperties: crate::iproperties::IPropertiesDialog,
    /// Tools > Document Settings.
    doc_settings: crate::doc_settings::DocSettingsDialog,
    /// The open sketch Mirror or Pattern dialog.
    sketch_dialog: Option<crate::sketch_dialogs::SketchDialog>,
    /// The open Auto Dimension dialog.
    auto_dimension: Option<crate::sketch_dialogs::AutoDimension>,
    /// An assembly import was started and its outcome not yet shown.
    importing: bool,
    /// The status bar says meshes are loading.
    loading_shown: bool,
    expression: String,
    ime: ImeTest,
    /// Whether the licence notices window is open.
    notices: bool,
    /// Whether the log window is open.
    log_open: bool,
    /// The Application Options window.
    options_open: bool,
    /// Whether the side panels let the 3D scene show through.
    translucent: bool,
    /// Whether the dark colour scheme is in use.
    dark: bool,
    /// The ribbon tab that was open before the current sketch, and whether a sketch was open last frame.
    tab_before_sketch: Tab,
    was_sketching: bool,
    /// The widths of the floating panels, and whether they are open.
    left_width: f32,
    right_width: f32,
    left_open: bool,
    right_open: bool,
    /// The status bar is shown (View tab, Windows panel, User Interface).
    show_status: bool,
    /// Clean Screen is on: the ribbon and the browser are away; the browser's state before it.
    clean_screen: Option<bool>,
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
        // The application starts dark unless the user chose light; without a settings folder (tests) it is
        // light, so that pictures and pixel counts do not depend on the scheme.
        let dark = options
            .settings_dir
            .as_deref()
            .map(gc_persist::places::load_settings)
            .is_some_and(|settings| !settings.light_theme);
        gc_ui::install_theme(&cc.egui_ctx, dark);
        let robot_dir = library_dir(options.robot_dir.as_deref());
        let packages = robot_dir.as_deref().map(gc_session::library::scan).unwrap_or_default();
        let licences = notices(&packages);
        let (library, resources, kinematics) = robot_library(&packages);
        let recent = options.settings_dir.as_deref().map(gc_persist::places::load_recent).unwrap_or_default();
        let recovery = options.settings_dir.as_deref().and_then(gc_persist::places::pending_autosave);
        // The language and agent connection chosen last time, when there are saved settings.
        let settings =
            options.settings_dir.as_deref().map(gc_persist::places::load_settings).unwrap_or_default();
        let lang = Lang::from_code(&settings.language).unwrap_or_default();
        let agent = crate::agent::Agent::new(&settings);
        let translucent = !settings.opaque_panels;
        let mut session = Session::new();
        session.set_resources(resources);
        session.set_kinematics(kinematics.factories);
        session.set_planners(kinematics.planners);
        session.set_robot_library(robot_dir);
        session.set_assembly_reader(crate::assembly_import::reader());
        if options.background_jobs {
            let notifier = std::sync::Arc::new(Repaint(cc.egui_ctx.clone()));
            session.set_executor(gc_jobs::Executor::thread().with_notifier(notifier));
        }
        // The Application Options that outlive a run.
        if let Some(settings) = options.settings_dir.as_deref().map(gc_persist::places::load_settings) {
            session.set_autoproject_face_edges(!settings.autoproject_edges_off);
            session.set_auto_bend(settings.auto_bend);
            session.set_snap_to_grid(settings.snap_to_grid);
        }
        let saved =
            options.settings_dir.as_deref().map(gc_persist::places::load_settings).unwrap_or_default();
        let mut viewport = Viewport::new(cc.wgpu_render_state.clone());
        viewport.show_view_cube = !saved.hide_view_cube;
        viewport.show_nav_bar = !saved.hide_nav_bar;
        Self {
            options,
            frames: 0,
            session,
            viewport,
            lang,
            tab: Tab::Model,
            cell: gc_panels::CellUi::default(),
            params: gc_panels::ParamsUi::default(),
            library,
            licences,
            packages,
            agent,
            measure: crate::measure::Measure::default(),
            interference: crate::interference::Interference::default(),
            drive: crate::drive::Drive::default(),
            constrain: crate::constrain::Constrain::default(),
            assemble: crate::assemble::Assemble::default(),
            work_feature: crate::work_feature::WorkFeatureDialog::default(),
            conflict: crate::conflict::ConstraintError::default(),
            joint: crate::constrain::Joint::default(),
            pattern: crate::pattern::Pattern::default(),
            replicate: crate::replicate::Replicate::default(),
            sketch_dialog: None,
            auto_dimension: None,
            parameters: crate::parameters::Parameters::default(),
            bom: crate::bom::BomDialog::default(),
            iproperties: crate::iproperties::IPropertiesDialog::default(),
            doc_settings: crate::doc_settings::DocSettingsDialog::default(),
            importing: false,
            loading_shown: false,
            expression: String::new(),
            ime: ImeTest { dimension: "25 mm".to_owned(), ..ImeTest::default() },
            notices: false,
            log_open: false,
            options_open: false,
            translucent,
            dark,
            tab_before_sketch: Tab::Model,
            was_sketching: false,
            left_width: 210.0,
            right_width: 330.0,
            left_open: !saved.browser_closed,
            right_open: !saved.properties_closed,
            show_status: !saved.hide_status_bar,
            clean_screen: None,
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

    /// Sets the Drive dialog's End value, for tests (typing into a drag value is awkward there).
    #[doc(hidden)]
    pub fn drive_end_for_test(&mut self, end: f64) {
        self.drive.set_end(end);
    }

    /// Opens the Drive dialog for an assembly relationship (Inventor: right-click it in the browser >
    /// Drive). The browser's relationship menu calls this.
    pub fn drive_relationship(&mut self, id: gc_core::Id<gc_doc::Relationship>) {
        self.drive.start_relationship(&mut self.session, id);
    }

    /// Closes the Drive dialog as its close button does, for tests.
    #[doc(hidden)]
    pub fn close_drive_for_test(&mut self) {
        self.drive.close(&mut self.session);
    }

    /// Says in the status bar while meshes or an assembly file are loading, and why an import failed
    /// (once, when it finishes).
    fn import_notice(&mut self, lang: Lang) {
        let loading = self.session.meshes_loading();
        // `None` while the file is being read (or nothing was imported); then whether it failed.
        let status = self.session.import_status().map(|r| r.as_ref().err().cloned());
        if status.is_none() && self.importing {
            // Still reading the file.
            self.notice = Some(tr!(lang, ImportLoading, loading.max(1)));
            return;
        }
        if self.importing {
            self.importing = false;
            self.notice = status.flatten().map(|error| tr!(lang, ImportFailed, error));
        }
        if loading > 0 {
            self.notice = Some(tr!(lang, ImportLoading, loading));
            self.loading_shown = true;
        } else if self.loading_shown {
            self.loading_shown = false;
            self.notice = None;
        }
    }

    /// The viewport, for inspecting what it drew.
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Frames everything in the view, as "fit" in the View tab does.
    pub fn fit_view(&mut self) {
        self.viewport.fit(&self.session, [1000.0, 700.0]);
    }

    /// The viewport, for tests that move the view.
    /// Place Component (Inventor 2026 help, Place Components, GUID-2E0B9110-BBAE-482B-A9AF-953BC40E8F2A):
    /// asks for a file. The first component stays at the origin (the session grounds it); a later body hangs
    /// on the cursor until it is set down with a click, Esc or a right click ends. An assembly file is read
    /// by a job and arrives at the origin: it cannot hang on the cursor yet.
    fn place_component(&mut self, lang: Lang) {
        let Some(path) = self.dialogs.open_mesh() else { return };
        let had_components = !self.session.components().is_empty();
        if crate::assembly_import::is_assembly(&path) {
            self.session.import_assembly(&path.to_string_lossy(), crate::assembly_import::importer());
            self.importing = true;
            self.notice = Some(tr!(lang, ImportLoading, 1));
            return;
        }
        match self.session.add_mesh_body(&path) {
            Ok(body) => {
                self.notice = None;
                if had_components {
                    self.viewport.start_placing(gc_session::CellItem::Body(body), false);
                } else {
                    self.fit_view();
                }
            }
            Err(error) => self.notice = Some(tr!(lang, FileError, error)),
        }
    }

    pub fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
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

    /// Clean Screen (Inventor 2026 help, To Work with Graphics Windows, GUID-84B053B0-E83B-4D9C-8436-
    /// 1BE9AD5E9EBE: hides the browser and the ribbon; again brings them back as they were).
    fn toggle_clean_screen(&mut self) {
        match self.clean_screen.take() {
            Some(browser_was_open) => self.left_open = browser_was_open,
            None => {
                self.clean_screen = Some(self.left_open);
                self.left_open = false;
            }
        }
        self.save_settings();
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
                        let warnings = self.session.load_warnings();
                        if !warnings.is_empty() {
                            self.notice = Some(tr!(lang, FileWarnings, warnings.len(), warnings.join("; ")));
                        }
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
                let opened = match self.library.iter().find(|robot| robot.name() == crate::example::ROBOT) {
                    Some(robot) => crate::example::open(&mut self.session, robot),
                    None => Err(tr!(lang, ExampleNeedsRobot, crate::example::ROBOT)),
                };
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
                        // What the file had that could not be used (a damaged joint range, ...) was left
                        // out; the status bar says so.
                        let warnings = self.session.load_warnings();
                        if !warnings.is_empty() {
                            self.notice = Some(tr!(lang, FileWarnings, warnings.len(), warnings.join("; ")));
                        }
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
                // Ctrl+0 leaves Clean Screen (GUID-5ED57F9B-6FA8-440B-94F3-0B92CACBE295: "press Ctrl 0 (zero) to
                // switch from Clean Screen display"); it also switches it on.
                egui::Key::Num0 if command => self.toggle_clean_screen(),
                egui::Key::N if command => self.request(Pending::New, ctx),
                egui::Key::O if command => self.request(Pending::Open, ctx),
                egui::Key::Y if command => {
                    self.session.redo();
                }
                // Ctrl+C copies the selected body, Ctrl+V pastes it hanging on the cursor like Place
                // Component: each click sets a copy down, Esc or a right click ends (Inventor 2026 help,
                // shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E). Without relationships, as Copy
                // Components has them off by default.
                egui::Key::C if command && !sketching => {
                    // The selected component (a body, a robot with its tool, an imported assembly); a
                    // refusal (nothing copyable selected) changes nothing.
                    if let Some(occurrence) = self.session.selected_occurrence() {
                        drop(self.session.copy_to_clipboard(&[gc_doc::TreeKey::Component(occurrence)]));
                    }
                }
                egui::Key::V if command && !sketching => {
                    // The paste waits for the click that gives its place: one undo step.
                    self.viewport.start_paste(&self.session);
                }
                // F10 shows or hides the sketch being edited, else the latest sketch (Inventor 2026 help,
                // shortcut table GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E: "F10 Sketch visibility").
                egui::Key::F10 => {
                    let sketch = self
                        .session
                        .active_sketch()
                        .map(|(id, _)| id)
                        .or_else(|| self.session.doc().sketches().iter().map(|(id, _)| id).last());
                    if let Some(sketch) = sketch {
                        let item = sketch.erase();
                        let shown = self.session.is_visible(item);
                        drop(self.session.set_visible(item, !shown));
                    }
                }
                egui::Key::F6 => self.viewport.look(StandardView::Iso),
                // J starts the Joint command, as in Inventor (outside sketches, where J is free).
                egui::Key::J if !command && !sketching => self.joint.start(),
                // In an assembly C adds a constraint and P places a component (Inventor 2026 help,
                // Predefined Command Alias and Shortcut Reference, GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E);
                // in a sketch C is the circle.
                egui::Key::C if !command && !sketching => self.constrain.start(),
                egui::Key::P if !command && !sketching => self.place_component(self.lang),
                // Tab demotes the selected component (asks for the new subassembly's name, as the browser
                // menu does) and Shift + Tab promotes it one level up (Inventor 2026 help, shortcut table
                // GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E: "Tab Demote", "Shift + Tab Promote").
                egui::Key::Tab if !command && !sketching => {
                    ctx.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                        i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                    });
                    if let Some(occurrence) = self.session.selected_occurrence() {
                        let key = gc_doc::TreeKey::Component(occurrence);
                        if !modifiers.shift {
                            self.params.group_name =
                                Some(gc_panels::GroupName::Demote(vec![key], String::new()));
                        } else if let Some(gc_doc::TreeKey::Subassembly(unit)) =
                            self.session.doc().tree_parent(key)
                        {
                            let into =
                                match self.session.doc().tree_parent(gc_doc::TreeKey::Subassembly(unit)) {
                                    Some(gc_doc::TreeKey::Subassembly(parent)) => Some(parent),
                                    _ => None,
                                };
                            // A refusal is explained in the status bar.
                            drop(self.session.edit_subassemblies(gc_doc::SubassemblyOp::Move {
                                members: vec![key],
                                into,
                            }));
                        }
                    }
                }
                // S makes a 2D sketch on the selected plane (an origin plane of a component or a work
                // plane), else on XY, and starts with the line tool as the ribbon does.
                egui::Key::S if !command && !sketching => {
                    let made = match (self.viewport.origin_selected, self.viewport.work_selected) {
                        (_, Some(work)) => self.session.create_sketch_on_work_plane(work).is_ok(),
                        (Some((occurrence, gc_doc::OriginElement::Plane(plane))), None) => {
                            self.session.create_sketch_on_component(occurrence, plane).is_ok()
                        }
                        _ => self.session.create_sketch("XY").is_ok(),
                    };
                    if made {
                        self.session.set_tool(ToolKind::Line);
                        self.tab = Tab::Sketch;
                    }
                }
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
                if ui.button(tr!(lang, MenuOptions)).clicked() {
                    self.options_open = true;
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
            ui.menu_button(tr!(lang, MenuHelp), |ui| {
                if ui.button(tr!(lang, MenuAbout)).clicked() {
                    self.about = true;
                }
                if ui.button(tr!(lang, MenuLog)).clicked() {
                    self.log_open = true;
                }
                if ui.button(tr!(lang, MenuLicences)).clicked() {
                    self.notices = true;
                }
            });
            ui.menu_button(tr!(lang, MenuTools), |ui| {
                if ui.button(tr!(lang, BomTitle)).clicked() {
                    self.bom.open = true;
                    ui.close();
                }
                if ui.button(tr!(lang, CmdIProperties)).clicked() {
                    self.iproperties.start(&self.session, None, "", lang);
                    ui.close();
                }
                if ui.button(tr!(lang, MenuDocSettings)).clicked() {
                    self.doc_settings.start(&self.session);
                    ui.close();
                }
                if ui.button(tr!(lang, MenuPoseCalc)).clicked() {
                    self.pose_calc.get_or_insert_with(gc_panels::PoseCalc::default);
                }
                if ui.checkbox(&mut self.viewport.perf_overlay, tr!(lang, MenuPerfOverlay)).clicked() {
                    ui.close();
                }
                if ui.button(tr!(lang, MenuImeTest)).clicked() {
                    self.ime.open = true;
                }
                if ui.button(tr!(lang, MenuAgent)).clicked() {
                    self.agent.open = true;
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
            // The Quick Access Toolbar (Inventor 2026 help, To Customize the Quick Access Toolbar,
            // GUID-3BF60B85-4BE9-4619-9FD3-0A179FA680E0: commands across all environments, Undo and Redo, and
            // Return, "available only from the QAT", which quits in-place editing and returns to the previous
            // editing state).
            ui.separator();
            let ctx = ui.ctx().clone();
            let qat = |ui: &mut egui::Ui, symbol: &str, label: &str, enabled: bool| {
                // Named apart from the menu's own entries ("저장 (빠른 실행)"), so a screen can tell them apart.
                let name = format!("{label} ({})", tr!(lang, QatTitle));
                let response = ui.add_enabled(enabled, egui::Button::new(symbol).frame(false));
                response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name.as_str()));
                response.on_hover_text(label).clicked()
            };
            if qat(ui, "+", tr!(lang, CmdNew), true) {
                self.request(Pending::New, &ctx);
            }
            if qat(ui, "O", tr!(lang, CmdOpen), true) {
                self.request(Pending::Open, &ctx);
            }
            if qat(ui, "S", tr!(lang, CmdSave), true) {
                self.save(false);
            }
            let (can_undo, can_redo) =
                (self.session.history().undo_depth() > 0, self.session.history().redo_depth() > 0);
            if qat(ui, "←", tr!(lang, CmdUndo), can_undo) {
                self.session.undo();
            }
            if qat(ui, "→", tr!(lang, CmdRedo), can_redo) {
                self.session.redo();
            }
            let editing = self.session.edit_target().is_some();
            if qat(ui, "↑", tr!(lang, CmdReturn), editing) {
                self.session.return_edit();
            }
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
        let settings = gc_persist::places::Settings {
            language: self.lang.code().to_owned(),
            agent_enabled: self.agent.enabled,
            agent_port: self.agent.port,
            agent_token: self.agent.token.clone(),
            opaque_panels: !self.translucent,
            light_theme: !self.dark,
            autoproject_edges_off: !self.session.autoproject_face_edges(),
            auto_bend: self.session.auto_bend(),
            snap_to_grid: self.session.snap_to_grid(),
            hide_view_cube: !self.viewport.show_view_cube,
            hide_nav_bar: !self.viewport.show_nav_bar,
            hide_status_bar: !self.show_status,
            browser_closed: !self.clean_screen.unwrap_or(self.left_open),
            properties_closed: !self.right_open,
            ..Default::default()
        };
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
    /// Help > Log: the last lines of the log, newest at the bottom.
    /// Application Options (Inventor: File > Options). Only the Sketch tab so far. Reference: Inventor 2026
    /// help, Sketch Tab Reference (Application Options), GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D.
    /// The choice is saved to the settings file at once.
    fn options_window(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        let mut open = self.options_open;
        let mut changed = false;
        egui::Window::new(tr!(lang, MenuOptions)).open(&mut open).default_width(420.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                // The tabs of the dialog; the sketch tab is the only one.
                drop(ui.selectable_label(true, tr!(lang, TabSketch)));
            });
            ui.separator();
            let mut on = self.session.autoproject_face_edges();
            if ui.checkbox(&mut on, tr!(lang, OptAutoproject)).changed() {
                self.session.set_autoproject_face_edges(on);
                changed = true;
            }
            // Snap to grid (Sketch tab; the spacing is the document's Snap Spacing): off by default.
            let mut snap = self.session.snap_to_grid();
            if ui.checkbox(&mut snap, tr!(lang, OptSnapToGrid)).changed() {
                self.session.set_snap_to_grid(snap);
                changed = true;
            }
            // Auto-bend with 3D line creation (Sketch tab, 3D Sketch; GUID-1898C432-CFD2-46F1-8BAE-631392DFBF1D):
            // off by default; the radius is a document setting (Tools > Document Settings, Sketch tab).
            let mut bend = self.session.auto_bend();
            if ui.checkbox(&mut bend, tr!(lang, OptAutoBend)).changed() {
                self.session.set_auto_bend(bend);
                changed = true;
            }
        });
        if changed {
            self.save_settings();
        }
        self.options_open = open;
    }

    fn log_window(&mut self, ctx: &egui::Context) {
        let mut open = self.log_open;
        egui::Window::new(tr!(self.lang, MenuLog)).open(&mut open).default_size([640.0, 360.0]).show(
            ctx,
            |ui| {
                egui::ScrollArea::both().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                    for line in crate::diagnostics::recent_log() {
                        ui.label(egui::RichText::new(line).monospace().size(11.5));
                    }
                });
            },
        );
        self.log_open = open;
    }

    fn notices_window(&mut self, ctx: &egui::Context) {
        let mut open = self.notices;
        egui::Window::new(tr!(self.lang, MenuLicences)).open(&mut open).default_size([560.0, 480.0]).show(
            ctx,
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (title, text) in &self.licences {
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
        self.import_notice(lang);
        // Agents' calls are carried out here, between frames, like the user's own edits.
        let before = (self.session.doc().robots().len(), self.session.doc().bodies().len());
        if let Some(edit) = self.agent.serve(&ctx, &mut self.session, &self.packages) {
            self.cell.playing = false;
            // A robot or body an agent put in is brought into view, as when the user adds one.
            if (self.session.doc().robots().len(), self.session.doc().bodies().len()) != before {
                self.fit_view();
            }
            self.notice = Some(tr!(lang, AgentEdited, edit));
        }
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

        // Starting a sketch opens the sketch tab; finishing it brings back the tab that was open before.
        let sketching = self.session.active_sketch().is_some();
        if sketching != self.was_sketching {
            // A tab the user went to on purpose (the robot tab, to make a frame from sketch points) stays.
            if sketching && self.tab == Tab::Model {
                self.tab = Tab::Sketch;
            } else if !sketching && self.tab == Tab::Sketch {
                self.tab = self.tab_before_sketch;
            }
            self.was_sketching = sketching;
        } else if !sketching {
            self.tab_before_sketch = self.tab;
        }
        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        let looks = (self.translucent, self.dark);
        let mut ribbon_action = None;
        let interface = ribbon::Interface {
            view_cube: self.viewport.show_view_cube,
            nav_bar: self.viewport.show_nav_bar,
            browser: self.left_open,
            properties: self.right_open,
            status_bar: self.show_status,
            clean_screen: self.clean_screen.is_some(),
        };
        // Clean Screen hides the ribbon.
        if self.clean_screen.is_none() {
            egui::Panel::top("ribbon").show(ui, |ui| {
                ribbon_action = ribbon::show(
                    ui,
                    &mut self.tab,
                    &mut self.session,
                    &mut self.viewport,
                    &mut self.cell,
                    &self.library,
                    &mut self.translucent,
                    &mut self.dark,
                    interface,
                    lang,
                );
            });
        }
        if looks != (self.translucent, self.dark) {
            if looks.1 != self.dark {
                gc_ui::install_theme(&ctx, self.dark);
            }
            self.save_settings();
        }
        match ribbon_action {
            Some(ribbon::RibbonAction::StartMeasure) => self.measure.start(&self.session),
            Some(ribbon::RibbonAction::StartInterference) => self.interference.start(&self.session),
            Some(ribbon::RibbonAction::StartDrive) => {
                // Without a robot, the first relationship that can be driven (not a rigid joint).
                let drivable = self
                    .session
                    .relationships()
                    .iter()
                    .find(|(_, r)| !matches!(r.kind, gc_doc::RelationKind::Rigid { .. }))
                    .map(|(id, _)| *id);
                match (self.session.doc().robots().is_empty(), drivable) {
                    (true, Some(id)) => self.drive_relationship(id),
                    _ => self.drive.start(&mut self.session),
                }
            }
            Some(ribbon::RibbonAction::SetInterface(element, on)) => {
                match element {
                    ribbon::InterfaceElement::ViewCube => self.viewport.show_view_cube = on,
                    ribbon::InterfaceElement::NavBar => self.viewport.show_nav_bar = on,
                    ribbon::InterfaceElement::Browser => self.left_open = on,
                    ribbon::InterfaceElement::Properties => self.right_open = on,
                    ribbon::InterfaceElement::StatusBar => self.show_status = on,
                }
                self.save_settings();
            }
            Some(ribbon::RibbonAction::CleanScreen) => self.toggle_clean_screen(),
            Some(ribbon::RibbonAction::StartConstrain) => self.constrain.start(),
            Some(ribbon::RibbonAction::StartAssemble) => self.assemble.start(),
            Some(ribbon::RibbonAction::StartWorkFeature(category)) => self.work_feature.start(category),
            Some(ribbon::RibbonAction::StartJoint) => self.joint.start(),
            Some(ribbon::RibbonAction::StartPattern) => self.pattern.start(Vec::new()),
            Some(ribbon::RibbonAction::StartReplicate(mode)) => self.replicate.start(mode),
            Some(ribbon::RibbonAction::StartParameters) => self.parameters.open = true,
            Some(ribbon::RibbonAction::StartBom) => self.bom.open = true,
            Some(ribbon::RibbonAction::StartAutoDimension) => {
                self.auto_dimension = Some(crate::sketch_dialogs::AutoDimension::new());
            }
            Some(ribbon::RibbonAction::StartSketchMirror) => {
                self.sketch_dialog = Some(crate::sketch_dialogs::SketchDialog::new(
                    crate::sketch_dialogs::Kind::Mirror,
                    &mut self.session,
                ));
            }
            Some(
                start @ (ribbon::RibbonAction::StartSketchRectPattern
                | ribbon::RibbonAction::StartSketchCircPattern),
            ) => {
                use crate::sketch_dialogs::{Kind, SketchDialog};
                // With a pattern's curve selected the dialog edits that pattern (Edit Pattern).
                self.sketch_dialog = Some(match self.session.selected_pattern() {
                    Some(pattern) => SketchDialog::edit(&pattern, &mut self.session),
                    None if start == ribbon::RibbonAction::StartSketchRectPattern => {
                        SketchDialog::new(Kind::Rectangular, &mut self.session)
                    }
                    None => SketchDialog::new(Kind::Circular, &mut self.session),
                });
            }
            Some(ribbon::RibbonAction::ToggleGround) => {
                let name = self
                    .session
                    .selected_occurrence()
                    .and_then(|o| {
                        self.session.components().into_iter().find(|n| n.occurrence == o).map(|n| n.name)
                    })
                    .unwrap_or_default();
                self.notice = Some(match self.session.toggle_ground_selected() {
                    Ok(true) => tr!(lang, GroundOn, name),
                    Ok(false) => tr!(lang, GroundOff, name),
                    Err(error) => tr!(lang, FileError, error),
                });
            }
            Some(ribbon::RibbonAction::ProjectCutEdges) => {
                self.notice = Some(match crate::cut_edges::project_cut_edges(&mut self.session) {
                    Ok(count) => tr!(lang, CutEdgesDone, count),
                    Err(crate::cut_edges::CutError::Refused(why)) => tr!(lang, FileError, why),
                    Err(crate::cut_edges::CutError::NothingCut | crate::cut_edges::CutError::NoSketch) => {
                        tr!(lang, CutEdgesNone).to_owned()
                    }
                });
            }
            _ => {}
        }
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
        if ribbon_action == Some(ribbon::RibbonAction::Place) {
            self.place_component(lang);
        }
        if ribbon_action == Some(ribbon::RibbonAction::ImportMesh)
            && let Some(path) = self.dialogs.open_mesh()
        {
            // An assembly file (glTF, STEP) is read by a job; its parts arrive with the next frames.
            if crate::assembly_import::is_assembly(&path) {
                self.session.import_assembly(&path.to_string_lossy(), crate::assembly_import::importer());
                self.importing = true;
                self.notice = Some(tr!(lang, ImportLoading, 1));
            } else {
                match self.session.add_mesh_body(&path) {
                    Ok(_) => {
                        self.notice = None;
                        self.viewport.fit(&self.session, [1000.0, 700.0]);
                    }
                    Err(error) => self.notice = Some(tr!(lang, FileError, error)),
                }
            }
        }
        if self.show_status {
            egui::Panel::bottom("status").show(ui, |ui| {
                gc_panels::status_bar(
                    ui,
                    &self.session,
                    lang,
                    self.viewport.background().has_gpu(),
                    self.notice.as_deref(),
                    (self.session.active_sketch().is_some() && !self.session.sketch_is_spatial())
                        .then_some(&mut self.viewport.slice),
                );
            });
        }
        // The 3D view fills the window; the browser and the properties float over it, see-through, and
        // the view's own overlays keep to what they leave free.
        let stage = ui.available_rect_before_wrap();
        let reserved =
            |open: bool, width: f32| if open { width + 2.0 * gc_ui::colors::PANEL_MARGIN } else { 44.0 };
        self.viewport.clear = Some(egui::Rect::from_min_max(
            egui::pos2(stage.left() + reserved(self.left_open, self.left_width), stage.top()),
            egui::pos2(stage.right() - reserved(self.right_open, self.right_width), stage.bottom()),
        ));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.measure.follow_view(&mut self.viewport, ui.input(|i| i.modifiers.shift));
            self.viewport.ui(ui, &mut self.session, lang);
            self.measure.follow_view(&mut self.viewport, ui.input(|i| i.modifiers.shift));
            self.measure.follow(&self.session);
            self.measure.overlay(ui.painter(), &self.viewport, &self.session);
            self.interference.follow(&self.session);
            self.interference.zoom(&mut self.viewport);
            self.interference.overlay(ui.painter(), &self.viewport);
        });
        if self.interference.open {
            self.interference.window(&ctx, &self.session, lang);
        }
        if self.bom.open {
            self.bom.window(&ctx, &mut self.session, lang);
            if std::mem::take(&mut self.bom.export)
                && let Some(path) = self.dialogs.save_csv("bom.csv")
            {
                let text = self.session.bom_csv(self.bom.view());
                self.notice = Some(match std::fs::write(&path, text) {
                    Ok(()) => tr!(lang, BomExported, path.display()),
                    Err(error) => tr!(lang, FileError, error),
                });
            }
        }
        if self.iproperties.open {
            self.iproperties.window(&ctx, &mut self.session, lang);
        }
        if self.doc_settings.open {
            self.doc_settings.window(&ctx, &mut self.session, lang);
        }
        if self.parameters.open {
            self.parameters.window(&ctx, &mut self.session, lang);
        }
        if let Some(dialog) = &mut self.auto_dimension
            && !dialog.window(&ctx, &mut self.session, lang)
        {
            self.auto_dimension = None;
        }
        if let Some(dialog) = &mut self.sketch_dialog
            && !dialog.window(&ctx, &mut self.session, lang)
        {
            self.sketch_dialog = None;
        }
        match self.session.tool() {
            gc_tools::ToolKind::Polygon => {
                crate::sketch_dialogs::polygon_window(&ctx, &mut self.session, lang)
            }
            gc_tools::ToolKind::Offset => crate::sketch_dialogs::offset_window(&ctx, &mut self.session, lang),
            gc_tools::ToolKind::Chamfer => {
                crate::sketch_dialogs::chamfer_window(&ctx, &mut self.session, lang)
            }
            gc_tools::ToolKind::Move
            | gc_tools::ToolKind::Copy
            | gc_tools::ToolKind::Rotate
            | gc_tools::ToolKind::Scale
            | gc_tools::ToolKind::Stretch => {
                crate::sketch_dialogs::transform_window(&ctx, &mut self.session, lang);
            }
            _ => {}
        }
        if self.drive.open {
            self.drive.tick(&ctx, &mut self.session);
            self.drive.window(&ctx, &mut self.session, lang);
        }
        if self.measure.open {
            self.measure.window(&ctx, &self.session, lang);
        }
        // A path made by Create Path in the view becomes the one the panel works on.
        if let Some(path) = self.viewport.take_route_made() {
            self.cell.path = Some(path);
        }
        // The browser row under the pointer is boxed in the view, from the next frame on.
        if !self.left_open {
            self.params.hovered = None;
        }
        self.viewport.browser_hover = self.params.hovered;
        self.params.role = self.viewport.role_selected;
        let opaque = !self.translucent;
        side_panel(
            &ctx,
            SidePanel {
                id: "browser",
                left: true,
                stage,
                width: &mut self.left_width,
                range: 180.0..=360.0,
                open: &mut self.left_open,
                opaque,
                title: tr!(lang, PanelBrowser),
                lang,
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.params.origin_selected = self.viewport.origin_selected;
                    match gc_panels::browser(ui, &self.session, lang, &mut self.params) {
                        Some(BrowserAction::SelectRole(role)) => self.viewport.role_selected = role,
                        Some(BrowserAction::FindInWindow(any)) => {
                            self.viewport.find_in_window(&self.session, any);
                        }
                        Some(BrowserAction::Properties(occurrence)) => {
                            let name = self
                                .session
                                .components()
                                .into_iter()
                                .find(|n| n.occurrence == occurrence)
                                .map(|n| n.name)
                                .unwrap_or_default();
                            self.iproperties.start(&self.session, Some(occurrence), &name, lang);
                        }
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
                        Some(BrowserAction::Select(item)) => {
                            self.viewport.origin_selected = None;
                            self.session.select_item(item);
                        }
                        Some(BrowserAction::OriginVisible(occurrence, elements, visible)) => {
                            // A refusal is shown in the status bar.
                            drop(self.session.set_origin_visible(occurrence, &elements, visible));
                        }
                        Some(BrowserAction::OriginSelect(chosen)) => {
                            self.session.select_item(None);
                            self.viewport.origin_selected = chosen;
                        }
                        Some(BrowserAction::DriveRelationship(id)) => {
                            self.drive.start_relationship(&mut self.session, id)
                        }
                        Some(BrowserAction::JointLock(id, on)) => {
                            // A refusal is explained in the status bar.
                            drop(self.session.set_joint_lock(id, on));
                        }
                        Some(BrowserAction::JointLimits(id)) => self.joint.edit_limits(&self.session, id),
                        Some(BrowserAction::JointProtect(id, on)) => {
                            drop(self.session.set_joint_protect(id, on));
                        }
                        Some(BrowserAction::EditWorkFeature(id)) => self.work_feature.edit(&self.session, id),
                        Some(BrowserAction::EditComponent(key)) => {
                            drop(self.session.edit_component(key));
                        }
                        Some(BrowserAction::Subassembly(op)) => {
                            // A refusal (a name in use) is explained in the status bar.
                            drop(self.session.edit_subassemblies(op));
                        }
                        Some(BrowserAction::NewView(name)) => {
                            let camera = Some(gc_viewport::camera_to_json(&self.viewport.camera));
                            let section = self.viewport.section.as_ref().map(gc_viewport::section_to_json);
                            let op = gc_doc::CellOp::AddViewRepresentation { name, camera, section };
                            drop(self.session.execute(&gc_doc::Command::Cell(op)));
                        }
                        Some(BrowserAction::ActivateView(index)) => {
                            let op = gc_doc::CellOp::ActivateViewRepresentation(index);
                            if self.session.execute(&gc_doc::Command::Cell(op)).is_ok()
                                && let Some(view) = self.session.representations().views.get(index)
                            {
                                // What the viewport stored comes back; text that does not parse leaves the
                                // current view as it is.
                                if let Some(camera) =
                                    view.camera.as_deref().and_then(gc_viewport::camera_from_json)
                                {
                                    self.viewport.camera = camera;
                                    self.viewport.section =
                                        view.section.as_deref().and_then(gc_viewport::section_from_json);
                                }
                            }
                        }
                        Some(BrowserAction::ContactSet(item, on)) => {
                            drop(self.session.set_contact_set(&[item], on));
                        }
                        Some(BrowserAction::SuppressRelationship(id, suppressed)) => {
                            drop(self.session.set_relationship_suppressed(id, suppressed));
                        }
                        Some(BrowserAction::SketchOnComponent(occurrence, plane)) => {
                            drop(self.session.create_sketch_on_component(occurrence, plane));
                        }
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
            },
        );
        side_panel(
            &ctx,
            SidePanel {
                id: "properties",
                left: false,
                stage,
                width: &mut self.right_width,
                range: 280.0..=520.0,
                open: &mut self.right_open,
                opaque,
                title: tr!(lang, PanelProperties),
                lang,
            },
            |ui| {
                ui.set_max_width(ui.available_width());
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // A sketch role selected in the browser has its own properties.
                    if let Some(role) =
                        self.viewport.role_selected.filter(|id| self.session.doc().get(*id).is_some())
                    {
                        gc_panels::role_properties(ui, &mut self.session, lang, role);
                        ui.separator();
                    }
                    gc_panels::properties(ui, &mut self.session, lang, &mut self.expression);
                    self.cell.hold_previews = self.viewport.is_grip_snapping();
                    gc_panels::cell(ui, &mut self.session, lang, &mut self.cell);
                });
            },
        );
        // What the marking menu of the view asked for.
        match self.viewport.take_request() {
            Some(gc_viewport::ViewportRequest::Constrain) => self.constrain.start(),
            Some(gc_viewport::ViewportRequest::PlaceComponent) => self.place_component(lang),
            Some(gc_viewport::ViewportRequest::EditWorkFeature(id)) => {
                self.work_feature.edit(&self.session, id)
            }
            Some(gc_viewport::ViewportRequest::EditUnit(occurrence)) => {
                if let Some(key) = self.session.pick_unit(occurrence) {
                    drop(self.session.edit_component(key));
                }
            }
            None => {}
        }
        // The Constrain dialog picks origin planes, axes and points in the view.
        self.viewport.constrain_mode = self.constrain.picking()
            || self.joint.open
            || self.pattern.open
            || self.replicate.picking()
            || self.assemble.picking();
        if let Some((first, second)) = self.viewport.take_quick_constrain() {
            crate::assemble::quick_mate(&mut self.session, first, second, &mut self.assemble.conflict);
        }
        if let Some(id) = self.viewport.take_constrain_work()
            && let Some(selection) = self.session.work_feature_selection(id)
        {
            // Only the Assemble command and the Constrain dialog take work features as geometry.
            if self.assemble.picking() {
                self.assemble.pick_selection(&self.session, selection);
            } else if self.constrain.picking() {
                self.constrain.pick_selection(&self.session, selection);
            }
        }
        if let Some((occurrence, element)) = self.viewport.take_constrain_pick() {
            if self.assemble.picking() {
                self.assemble.pick(&self.session, occurrence, element);
            } else if self.replicate.picking() {
                self.replicate.pick(occurrence, element);
            } else if self.pattern.open {
                self.pattern.pick(occurrence, element);
            } else if self.joint.open {
                self.joint.pick(occurrence, element);
            } else {
                self.constrain.pick(&self.session, occurrence, element);
            }
        }
        if self.joint.open {
            self.joint.window(&ctx, &mut self.session, lang);
        }
        if self.pattern.open {
            self.pattern.window(&ctx, &mut self.session, lang);
        }
        if self.replicate.open {
            self.replicate.window(&ctx, &mut self.session, lang);
        }
        if self.assemble.open {
            self.assemble.window(&ctx, &mut self.session, lang);
        }
        if self.work_feature.open {
            self.work_feature.window(&ctx, &mut self.session, lang);
        }
        if let Some(conflict) = self
            .joint
            .conflict
            .take()
            .or_else(|| self.constrain.conflict.take())
            .or_else(|| self.assemble.conflict.take())
        {
            self.conflict.show_for(&self.session, conflict);
        }
        self.conflict.window(&ctx, &mut self.session, lang);
        if self.constrain.open {
            self.constrain.window(&ctx, &mut self.session, lang);
        }
        self.reference_question(&ctx);
        self.unsaved_question(&ctx);
        self.recover_question(&ctx);
        if self.ime.open {
            self.ime_window(&ctx);
        }
        if self.notices {
            self.notices_window(&ctx);
        }
        if self.options_open {
            self.options_window(&ctx);
        }
        if self.log_open {
            self.log_window(&ctx);
        }
        if self.about {
            self.about_window(&ctx);
        }
        if self.agent.open && self.agent.window(&ctx, lang) {
            self.save_settings();
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
    crate::diagnostics::install_diagnostics(options.settings_dir.as_deref());
    log::info!("GraphCTRL {} starts", env!("CARGO_PKG_VERSION"));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("GraphCTRL")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0]);
    if let Some(icon) = crate::diagnostics::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let native = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("graphctrl", native, Box::new(move |cc| Ok(Box::new(GraphCtrlApp::new(cc, options)))))
        .map_err(|err| anyhow::anyhow!("the application window could not be started: {err}"))
}

/// What a floating side panel needs.
struct SidePanel<'a> {
    id: &'static str,
    /// On the left edge of the stage, else on the right.
    left: bool,
    /// The area the panel floats in.
    stage: egui::Rect,
    width: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
    open: &'a mut bool,
    opaque: bool,
    /// The panel's name, for the button that folds it.
    title: &'a str,
    lang: Lang,
}

/// A panel that floats over the viewport: rounded, see-through, with a handle on its inner edge that
/// changes its width and a button that folds it away. The mouse never reaches the viewport through it.
fn side_panel(ctx: &egui::Context, panel: SidePanel<'_>, content: impl FnOnce(&mut egui::Ui)) {
    let margin = gc_ui::colors::PANEL_MARGIN;
    let SidePanel { id, left, stage, width, range, open, opaque, title, lang } = panel;
    let fold_label = |open: bool| {
        if open { tr!(lang, PanelFold, title) } else { tr!(lang, PanelUnfold, title) }
    };
    if !*open {
        // Folded: a small button in the corner brings it back.
        let corner = if left {
            stage.left_top() + egui::vec2(margin, margin)
        } else {
            stage.right_top() + egui::vec2(-margin - 28.0, margin)
        };
        egui::Area::new(egui::Id::unique(("gc_panel_unfold", id)))
            .fixed_pos(corner)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let label = fold_label(false);
                let button = ui.add_sized(
                    [28.0, 28.0],
                    egui::Button::new(if left { ">" } else { "<" }).fill(gc_ui::panel_fill(opaque)),
                );
                button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
                if button.on_hover_text(&label).clicked() {
                    *open = true;
                }
            });
        return;
    }
    *width = width.clamp(*range.start(), *range.end());
    let size = egui::vec2(*width, (stage.height() - 2.0 * margin).max(80.0));
    let min = if left {
        stage.left_top() + egui::vec2(margin, margin)
    } else {
        egui::pos2(stage.right() - margin - size.x, stage.top() + margin)
    };
    let rect = egui::Rect::from_min_size(min, size);
    egui::Area::new(egui::Id::unique(("gc_panel", id))).fixed_pos(min).order(egui::Order::Middle).show(
        ctx,
        |ui| {
            // Registered first, so the panel's widgets win and the empty parts still keep the mouse.
            ui.interact(rect, egui::Id::unique(("gc_panel_block", id)), egui::Sense::click_and_drag());
            gc_ui::panel_frame(opaque).show(ui, |ui| {
                ui.set_min_size(size - egui::vec2(18.0, 18.0));
                ui.set_max_size(size - egui::vec2(18.0, 18.0));
                content(ui);
            });
            // The handle on the inner edge.
            let edge = if left { rect.right() } else { rect.left() };
            let handle = egui::Rect::from_min_max(
                egui::pos2(edge - 4.0, rect.top() + 10.0),
                egui::pos2(edge + 4.0, rect.bottom() - 10.0),
            );
            let drag = ui.interact(handle, egui::Id::unique(("gc_panel_edge", id)), egui::Sense::drag());
            if drag.hovered() || drag.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if drag.dragged() {
                let dx = drag.drag_delta().x;
                *width = (*width + if left { dx } else { -dx }).clamp(*range.start(), *range.end());
            }
            // The fold button.
            // Top right corner, clear of the panel's header at the left.
            let button_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - 30.0, rect.top() + 6.0),
                egui::vec2(22.0, 22.0),
            );
            let label = fold_label(true);
            let button = ui.put(button_rect, egui::Button::new(if left { "<" } else { ">" }).small());
            button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
            if button.on_hover_text(&label).clicked() {
                *open = false;
            }
        },
    );
}
