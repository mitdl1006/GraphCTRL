//! The Document Settings dialog (Tools > Document Settings; Inventor 2026 help, To Change Document Settings,
//! GUID-FB49837F-7832-4713-928E-1CEDBDB0C3BD; Sketch Tab Reference (Document Settings),
//! GUID-58DD9A89-84C7-48F3-8E4E-0303E755B10F): Units tab (units of measure, Modeling Dimension Display,
//! Default Parameter Input Display) and Sketch tab (snap spacing, grid display, 3D Sketch Auto-Bend Radius,
//! constraint inference). The values are saved with the document; OK applies and closes, Apply applies.

use gc_doc::{AngleUnit, DimensionDisplay, DocSettings, LengthUnit};
use gc_i18n::{Lang, tr};
use gc_session::Session;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Units,
    Sketch,
}

/// The dialog's state: the settings being edited and the lengths as typed.
pub(crate) struct DocSettingsDialog {
    /// Whether the dialog is open.
    pub open: bool,
    tab: Tab,
    draft: DocSettings,
    /// Snap spacing X, Y and the Auto-Bend radius as typed.
    typed: [String; 3],
    error: Option<String>,
}

impl Default for DocSettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Units,
            draft: DocSettings::default(),
            typed: [String::new(), String::new(), String::new()],
            error: None,
        }
    }
}

/// A length in the document's unit with every digit that matters, e.g. `"10 mm"`.
fn length_text(settings: &DocSettings, metres: f64) -> String {
    DocSettings { linear_precision: 8, ..*settings }.format_length(metres)
}

impl DocSettingsDialog {
    /// Opens the dialog on the document's current settings.
    pub(crate) fn start(&mut self, session: &Session) {
        self.open = true;
        self.tab = Tab::Units;
        self.error = None;
        self.load(session.doc_settings());
    }

    fn load(&mut self, settings: DocSettings) {
        self.draft = settings;
        self.typed = [
            length_text(&settings, settings.snap_spacing[0]),
            length_text(&settings, settings.snap_spacing[1]),
            length_text(&settings, settings.auto_bend_radius),
        ];
    }

    /// The typed lengths into the draft; the first that does not evaluate to a positive length is refused.
    fn read_lengths(&mut self, session: &Session) -> Result<(), ()> {
        let unit = self.draft.length();
        let mut metres = [0.0; 3];
        for (slot, text) in metres.iter_mut().zip(&self.typed) {
            match gc_doc::evaluate_value(session.doc(), text, unit) {
                Some(value) if value > 0.0 => *slot = value,
                _ => return Err(()),
            }
        }
        self.draft.snap_spacing = [metres[0], metres[1]];
        self.draft.auto_bend_radius = metres[2];
        Ok(())
    }

    /// Applies the draft to the document; a refusal is shown in the dialog. True when it was applied.
    fn apply(&mut self, session: &mut Session, lang: Lang) -> bool {
        if self.read_lengths(session).is_err() {
            self.error = Some(tr!(lang, DsInvalid).to_owned());
            return false;
        }
        match session.set_doc_settings(self.draft) {
            Ok(()) => {
                self.error = None;
                // The lengths show again in the (possibly new) unit.
                self.load(session.doc_settings());
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
        egui::Window::new(tr!(lang, MenuDocSettings))
            .id(egui::Id::unique("gc_doc_settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Units, tr!(lang, DsTabUnits));
                    ui.selectable_value(&mut self.tab, Tab::Sketch, tr!(lang, DsTabSketch));
                });
                ui.separator();
                match self.tab {
                    Tab::Units => self.units_tab(ui, lang),
                    Tab::Sketch => self.sketch_tab(ui, lang),
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

    fn units_tab(&mut self, ui: &mut egui::Ui, lang: Lang) {
        let d = &mut self.draft;
        egui::Grid::new("gc_ds_units").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(tr!(lang, DsLength));
            egui::ComboBox::from_id_salt("gc_ds_length")
                .selected_text(match d.length_unit {
                    LengthUnit::Mm => "mm",
                    LengthUnit::Cm => "cm",
                    LengthUnit::M => "m",
                    LengthUnit::In => "in",
                })
                .show_ui(ui, |ui| {
                    for (unit, name) in [
                        (LengthUnit::Mm, "mm"),
                        (LengthUnit::Cm, "cm"),
                        (LengthUnit::M, "m"),
                        (LengthUnit::In, "in"),
                    ] {
                        ui.selectable_value(&mut d.length_unit, unit, name);
                    }
                });
            ui.end_row();
            ui.label(tr!(lang, DsAngle));
            egui::ComboBox::from_id_salt("gc_ds_angle")
                .selected_text(if d.angle_unit == AngleUnit::Deg { "deg" } else { "rad" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.angle_unit, AngleUnit::Deg, "deg");
                    ui.selectable_value(&mut d.angle_unit, AngleUnit::Rad, "rad");
                });
            ui.end_row();
            ui.label(tr!(lang, DsLinearPrecision));
            ui.add(egui::DragValue::new(&mut d.linear_precision).range(0..=8));
            ui.end_row();
            ui.label(tr!(lang, DsAngularPrecision));
            ui.add(egui::DragValue::new(&mut d.angular_precision).range(0..=8));
            ui.end_row();
            ui.label(tr!(lang, DsDimensionDisplay));
            let shown = match d.dimension_display {
                DimensionDisplay::Value => tr!(lang, DsDisplayValue),
                DimensionDisplay::Name => tr!(lang, DsDisplayName),
                DimensionDisplay::Expression => tr!(lang, DsDisplayExpression),
                DimensionDisplay::PreciseValue => tr!(lang, DsDisplayPrecise),
            };
            egui::ComboBox::from_id_salt("gc_ds_display").selected_text(shown).show_ui(ui, |ui| {
                for (mode, name) in [
                    (DimensionDisplay::Value, tr!(lang, DsDisplayValue)),
                    (DimensionDisplay::Name, tr!(lang, DsDisplayName)),
                    (DimensionDisplay::Expression, tr!(lang, DsDisplayExpression)),
                    (DimensionDisplay::PreciseValue, tr!(lang, DsDisplayPrecise)),
                ] {
                    ui.selectable_value(&mut d.dimension_display, mode, name);
                }
            });
            ui.end_row();
        });
        ui.checkbox(&mut d.input_as_expression, tr!(lang, DsInputExpression));
    }

    fn sketch_tab(&mut self, ui: &mut egui::Ui, lang: Lang) {
        egui::Grid::new("gc_ds_sketch").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            for (index, label) in
                [tr!(lang, DsSnapX), tr!(lang, DsSnapY), tr!(lang, DsAutoBendRadius)].into_iter().enumerate()
            {
                ui.label(label);
                let field = ui.add(egui::TextEdit::singleline(&mut self.typed[index]).desired_width(110.0));
                field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
                ui.end_row();
            }
            ui.label(tr!(lang, DsSnapsPerMinor));
            ui.add(egui::DragValue::new(&mut self.draft.snaps_per_minor).range(1..=100));
            ui.end_row();
            ui.label(tr!(lang, DsMinorPerMajor));
            ui.add(egui::DragValue::new(&mut self.draft.minor_per_major).range(1..=100));
            ui.end_row();
        });
        ui.checkbox(&mut self.draft.infer_constraints, tr!(lang, DsInfer));
        ui.checkbox(&mut self.draft.persist_constraints, tr!(lang, DsPersist));
    }
}
