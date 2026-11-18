//! The iProperties dialog (browser right-click iProperties… on a component, File > iProperties… for the
//! document; Inventor 2026 help, iProperties Reference, GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3; Physical
//! Tab, GUID-877564AC-78CE-4216-9913-003A2DE8D698). Tabs: Summary, Project and Physical. Physical shows the
//! density and a mass override, and the calculated mass, volume, area and center of gravity with Update.

use gc_doc::{IProperties, Occurrence};
use gc_i18n::{Lang, tr};
use gc_session::{PhysicalProperties, Session};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Summary,
    Project,
    Physical,
}

/// The dialog's state.
pub(crate) struct IPropertiesDialog {
    /// Whether the dialog is open.
    pub open: bool,
    /// What it is for: a component, or the document.
    target: Option<Occurrence>,
    title: String,
    tab: Tab,
    draft: IProperties,
    /// Density and the mass override as typed (empty: none).
    typed: [String; 2],
    /// The calculated values as of the last Update; `None` when the shape is not known.
    physical: Option<PhysicalProperties>,
    error: Option<String>,
}

impl Default for IPropertiesDialog {
    fn default() -> Self {
        Self {
            open: false,
            target: None,
            title: String::new(),
            tab: Tab::Summary,
            draft: IProperties::default(),
            typed: [String::new(), String::new()],
            physical: None,
            error: None,
        }
    }
}

fn number_text(value: Option<f64>) -> String {
    value.map_or_else(String::new, |v| format!("{v}"))
}

/// A typed number: empty is `None`; anything else must be a finite number above zero.
fn parse_optional(text: &str) -> Result<Option<f64>, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() && v > 0.0 => Ok(Some(v)),
        _ => Err(()),
    }
}

impl IPropertiesDialog {
    /// Opens the dialog for a component (`Some`) or the document (`None`).
    pub(crate) fn start(&mut self, session: &Session, target: Option<Occurrence>, name: &str, lang: Lang) {
        self.open = true;
        self.target = target;
        self.title = match target {
            Some(_) => format!("iProperties — {name}"),
            None => tr!(lang, IpTitleDocument).to_owned(),
        };
        self.tab = Tab::Summary;
        self.error = None;
        self.draft = session.properties(target);
        self.typed = [number_text(self.draft.density), number_text(self.draft.mass_override)];
        self.physical = session.physical_properties(target);
    }

    fn apply(&mut self, session: &mut Session, lang: Lang) -> bool {
        let (Ok(density), Ok(mass)) = (parse_optional(&self.typed[0]), parse_optional(&self.typed[1])) else {
            self.error = Some(tr!(lang, DsInvalid).to_owned());
            return false;
        };
        self.draft.density = density;
        self.draft.mass_override = mass;
        match session.set_properties(self.target, self.draft.clone()) {
            Ok(()) => {
                self.error = None;
                // The values depend on what was just set.
                self.physical = session.physical_properties(self.target);
                true
            }
            Err(error) => {
                self.error = Some(error.to_string());
                false
            }
        }
    }

    /// The window while it is open.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        let mut close = false;
        let title = self.title.clone();
        egui::Window::new(title)
            .id(egui::Id::unique("gc_iproperties"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Summary, tr!(lang, IpTabSummary));
                    ui.selectable_value(&mut self.tab, Tab::Project, tr!(lang, IpTabProject));
                    ui.selectable_value(&mut self.tab, Tab::Physical, tr!(lang, IpTabPhysical));
                });
                ui.separator();
                match self.tab {
                    Tab::Summary => {
                        let d = &mut self.draft;
                        text_grid(
                            ui,
                            "gc_ip_summary",
                            [
                                (tr!(lang, IpTitle), &mut d.title, false),
                                (tr!(lang, IpSubject), &mut d.subject, false),
                                (tr!(lang, IpAuthor), &mut d.author, false),
                                (tr!(lang, IpCompany), &mut d.company, false),
                                (tr!(lang, IpComments), &mut d.comments, true),
                            ],
                        );
                    }
                    Tab::Project => {
                        let d = &mut self.draft;
                        text_grid(
                            ui,
                            "gc_ip_project",
                            [
                                (tr!(lang, IpPartNumber), &mut d.part_number, false),
                                (tr!(lang, IpDescription), &mut d.description, false),
                                (tr!(lang, IpRevision), &mut d.revision, false),
                                (tr!(lang, IpProject), &mut d.project, false),
                                (tr!(lang, IpDesigner), &mut d.designer, false),
                                (tr!(lang, IpVendor), &mut d.vendor, false),
                            ],
                        );
                    }
                    Tab::Physical => self.physical_tab(ui, session, lang),
                }
                ui.separator();
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(0xC0, 0x30, 0x30), error);
                }
                ui.horizontal(|ui| {
                    if ui.button(tr!(lang, Ok)).clicked() && self.apply(session, lang) {
                        close = true;
                    }
                    if ui.button(tr!(lang, DsApply)).clicked() {
                        self.apply(session, lang);
                    }
                    if ui.button(tr!(lang, Cancel)).clicked() {
                        close = true;
                    }
                });
            });
        self.open = open && !close;
    }

    fn physical_tab(&mut self, ui: &mut egui::Ui, session: &Session, lang: Lang) {
        egui::Grid::new("gc_ip_physical_in").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            for (index, label) in [tr!(lang, IpDensity), tr!(lang, IpMassOverride)].into_iter().enumerate() {
                ui.label(label);
                let field = ui.add(egui::TextEdit::singleline(&mut self.typed[index]).desired_width(120.0));
                field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
                ui.end_row();
            }
        });
        ui.separator();
        // The measurement units follow the document settings' Units tab.
        let settings = session.doc_settings();
        let unit = settings.length();
        let word = unit.word();
        let cube = unit.si_factor().powi(3);
        let square = unit.si_factor().powi(2);
        match self.physical {
            Some(p) => {
                if p.open {
                    // A mesh with holes: volume, mass and centre are not to be trusted (the area is right).
                    ui.colored_label(egui::Color32::from_rgb(0xB0, 0x70, 0x10), tr!(lang, IpOpenMesh));
                }
                let number =
                    |v: f64| format!("{v:.6}").trim_end_matches('0').trim_end_matches('.').to_owned();
                egui::Grid::new("gc_ip_physical_out").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(tr!(lang, IpMass));
                    let overridden = if p.overridden { tr!(lang, IpOverridden) } else { "" };
                    ui.label(format!("{} kg {overridden}", number(p.mass)));
                    ui.end_row();
                    ui.label(tr!(lang, IpVolume));
                    ui.label(format!("{} {word}³", number(p.volume / cube)));
                    ui.end_row();
                    ui.label(tr!(lang, IpArea));
                    ui.label(format!("{} {word}²", number(p.area / square)));
                    ui.end_row();
                    ui.label(tr!(lang, IpCentre));
                    ui.label(format!(
                        "{}, {}, {} {word}",
                        number(p.centre.x / unit.si_factor()),
                        number(p.centre.y / unit.si_factor()),
                        number(p.centre.z / unit.si_factor())
                    ));
                    ui.end_row();
                });
            }
            None => {
                ui.weak(tr!(lang, IpNotKnown));
            }
        }
        if ui.button(tr!(lang, IpUpdate)).clicked() {
            self.physical = session.physical_properties(self.target);
        }
    }
}

/// Labelled text boxes in a two-column grid; `true` marks a box with several lines.
fn text_grid<const N: usize>(ui: &mut egui::Ui, id: &str, rows: [(&str, &mut String, bool); N]) {
    egui::Grid::new(id).num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        for (label, text, multiline) in rows {
            ui.label(label);
            let field = if multiline {
                ui.add(egui::TextEdit::multiline(text).desired_width(240.0).desired_rows(3))
            } else {
                ui.add(egui::TextEdit::singleline(text).desired_width(240.0))
            };
            field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
            ui.end_row();
        }
    });
}
