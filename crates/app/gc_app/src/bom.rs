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

    fn table(&mut self, ui: &mut egui::Ui, session: &mut Session, rows: &[BomRow], lang: Lang) {
        let mut chosen: Option<(TreeKey, BomStructure)> = None;
        let mut select: Option<TreeKey> = None;
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
                        let quantity = ui.selectable_label(picked, row.quantity.to_string());
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
        if let Some((key, structure)) = chosen {
            // A refusal shows in the status bar.
            drop(session.set_bom_structure(key, structure));
        }
    }
}
