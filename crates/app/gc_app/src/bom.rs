//! The Bill of Materials dialog (Inventor 2026 help, About the BOM Structure, GUID-A0A95805-CBF6-4524-8314-
//! 4AC8B53C19CF: "To change the BOM Structure, double-click the BOM Structure row in the Bill of Materials").
//! Structured and Parts Only views, a click on a row selects the component in the browser and the view, the
//! BOM Structure of a row is chosen from a list, and the table exports to CSV.

use gc_doc::{BomStructure, TreeKey};
use gc_i18n::{Lang, tr};
use gc_session::{BomRow, BomView, Session, structure_name};

/// The dialog's state.
#[derive(Default)]
pub(crate) struct BomDialog {
    /// Whether the dialog is open.
    pub open: bool,
    parts_only: bool,
    /// The file the CSV should go to, asked for in the next frame (a native dialog blocks).
    pub export: bool,
    /// The first tree row of the selected line.
    selected: Option<TreeKey>,
    /// The Excluded Part Numbers being typed (comma separated), and the settings they were read from.
    excluded: Option<String>,
}

impl BomDialog {
    /// The view shown.
    pub(crate) fn view(&self) -> BomView {
        if self.parts_only { BomView::PartsOnly } else { BomView::Structured }
    }

    /// The window while it is open.
    pub(crate) fn window(&mut self, ctx: &egui::Context, session: &mut Session, lang: Lang) {
        let mut open = self.open;
        egui::Window::new(tr!(lang, BomTitle))
            .id(egui::Id::unique("gc_bom"))
            .open(&mut open)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.parts_only, false, tr!(lang, BomStructured));
                    ui.selectable_value(&mut self.parts_only, true, tr!(lang, BomPartsOnly));
                });
                // BOM Settings (Inventor 2026 help, To Work with Bill of Materials, GUID-7A1FC6F1-8833-4A76-9E2B-AA6FE5FDF223):
                // Merge Rows on Part Number Match, Excluded Part Numbers, Hide Suppressed components in BOM.
                self.settings(ui, session, lang);
                ui.separator();
                let rows = session.bill_of_materials(self.view());
                if rows.is_empty() {
                    ui.weak(tr!(lang, BomEmpty));
                } else {
                    self.table(ui, session, &rows, lang);
                }
                ui.separator();
                if ui.button(tr!(lang, BomExport)).clicked() {
                    self.export = true;
                }
            });
        self.open = open;
    }

    fn settings(&mut self, ui: &mut egui::Ui, session: &mut Session, lang: Lang) {
        let current = session.bom_settings();
        let mut next = current.clone();
        egui::CollapsingHeader::new(tr!(lang, BomSettingsTitle)).id_salt("gc_bom_settings").show(ui, |ui| {
            ui.checkbox(&mut next.merge_part_numbers, tr!(lang, BomMerge));
            ui.checkbox(&mut next.hide_suppressed, tr!(lang, BomHideSuppressed));
            ui.horizontal(|ui| {
                let label = ui.label(tr!(lang, BomExcluded));
                let text = self.excluded.get_or_insert_with(|| current.merge_excluded.join(", "));
                let field = ui.add(egui::TextEdit::singleline(text).desired_width(200.0));
                field.clone().labelled_by(label.id);
                if field.lost_focus() {
                    next.merge_excluded = text
                        .split(',')
                        .map(|part| part.trim().to_owned())
                        .filter(|part| !part.is_empty())
                        .collect();
                    self.excluded = None;
                }
            });
        });
        if next != current {
            // A refusal shows in the status bar.
            drop(session.set_bom_settings(next));
        }
    }

    fn table(&mut self, ui: &mut egui::Ui, session: &mut Session, rows: &[BomRow], lang: Lang) {
        let mut chosen: Option<(TreeKey, BomStructure)> = None;
        let mut select: Option<TreeKey> = None;
        let mut set_quantity: Option<(TreeKey, Option<u32>)> = None;
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            egui::Grid::new("gc_bom_table").num_columns(5).striped(true).spacing([14.0, 4.0]).show(
                ui,
                |ui| {
                    for head in [
                        tr!(lang, BomItem),
                        tr!(lang, BomPartNumber),
                        tr!(lang, BomQuantity),
                        tr!(lang, BomDescription),
                        tr!(lang, BomStructureColumn),
                    ] {
                        ui.strong(head);
                    }
                    ui.end_row();
                    for row in rows {
                        let first = row.keys.first().copied();
                        let picked = first.is_some() && first == self.selected;
                        let indent = "  ".repeat(row.level as usize);
                        let item = ui.selectable_label(picked, format!("{indent}{}", row.item));
                        let part = ui.selectable_label(picked, &row.part_number);
                        // The quantity can be set by hand (a Static Quantity, shown in blue); the arrow puts back the
                        // counted one.
                        let quantity = ui
                            .horizontal(|ui| {
                                let mut value = row.quantity;
                                let mut drag = egui::DragValue::new(&mut value).range(0..=99_999);
                                if row.static_quantity {
                                    drag = drag.custom_formatter(|n, _| format!("{n}"));
                                }
                                let response = ui.add(drag);
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::SpinButton,
                                        true,
                                        format!("{} {}", row.part_number, tr!(lang, BomQuantity)),
                                    )
                                });
                                if (response.drag_stopped() || (response.lost_focus() && response.changed()))
                                    && value != row.quantity
                                    && let Some(key) = row.keys.first()
                                {
                                    set_quantity = Some((*key, Some(value)));
                                }
                                if row.static_quantity
                                    && ui.small_button("×").on_hover_text(tr!(lang, BomCountAgain)).clicked()
                                    && let Some(key) = row.keys.first()
                                {
                                    set_quantity = Some((*key, None));
                                }
                            })
                            .response;
                        let description = ui.selectable_label(picked, &row.description);
                        if let Some(key) = first
                            && (item.clicked()
                                || part.clicked()
                                || quantity.clicked()
                                || description.clicked())
                        {
                            select = Some(key);
                        }
                        // The BOM Structure is chosen from a list on the line.
                        if let Some(key) = first {
                            egui::ComboBox::from_id_salt(("gc_bom_structure", format!("{key:?}")))
                                .selected_text(structure_name(row.structure))
                                .show_ui(ui, |ui| {
                                    for option in [
                                        BomStructure::Normal,
                                        BomStructure::Phantom,
                                        BomStructure::Reference,
                                        BomStructure::Purchased,
                                        BomStructure::Inseparable,
                                    ] {
                                        if ui
                                            .selectable_label(row.structure == option, structure_name(option))
                                            .clicked()
                                        {
                                            chosen = Some((key, option));
                                        }
                                    }
                                });
                        } else {
                            ui.label(structure_name(row.structure));
                        }
                        ui.end_row();
                    }
                },
            );
        });
        if let Some(key) = select {
            self.selected = Some(key);
            // The browser and the view follow: a component in the cell is selected; a subassembly has
            // nothing to select in the view.
            if let TreeKey::Component(occurrence) = key {
                session.select_occurrence(Some(occurrence));
            }
        }
        if let Some((key, quantity)) = set_quantity {
            drop(session.set_bom_quantity(key, quantity));
        }
        if let Some((key, structure)) = chosen {
            // A refusal shows in the status bar.
            drop(session.set_bom_structure(key, structure));
        }
    }
}
