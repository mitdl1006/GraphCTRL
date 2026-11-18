//! Bill of Materials: the Structured and Parts Only views of the assembly tree (Inventor 2026 help, About the BOM
//! Structure, GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF), and CSV export.
//!
//! - Normal rows are numbered and counted.
//! - Phantom rows are not listed; their children move up to the parent's level.
//! - Reference rows and everything under them are left out.
//! - Purchased rows are one line. Their children show in the structured view and hide in parts-only.
//! - Inseparable rows are parts in parts-only, where only purchased rows inside them still show.
//!
//! Rows with the same part number at one level are one line with the quantities added.

use std::collections::HashMap;

use gc_doc::{BomStructure, CellOp, Command, EditError, Occurrence, TreeKey, TreeRow};

use crate::Session;

/// Which BOM view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BomView {
    /// Every level, children under their assembly; item numbers like "1.2".
    Structured,
    /// Only parts (components without children), purchased and inseparable assemblies; no hierarchy.
    PartsOnly,
}

/// One line of a BOM view.
#[derive(Clone, Debug, PartialEq)]
pub struct BomRow {
    /// Item number: "1", "1.2" in the structured view, "1", "2" in parts-only.
    pub item: String,
    /// Indentation: 0 at the top level (always 0 in parts-only).
    pub level: u32,
    /// The Part Number iProperty, else the name.
    pub part_number: String,
    /// The Description iProperty.
    pub description: String,
    /// How many, per one of the line above it (structured) or in the whole assembly (parts-only).
    pub quantity: u32,
    /// The BOM Structure of the first tree row it stands for.
    pub structure: BomStructure,
    /// The tree rows it stands for (several when merged).
    pub keys: Vec<TreeKey>,
}

/// The assembly tree as the BOM sees it.
struct Tree {
    rows: Vec<TreeRow>,
    children: HashMap<Option<TreeKey>, Vec<usize>>,
}

impl Tree {
    fn kids(&self, parent: Option<TreeKey>) -> &[usize] {
        self.children.get(&parent).map(Vec::as_slice).unwrap_or_default()
    }

    fn has_kids(&self, key: TreeKey) -> bool {
        !self.kids(Some(key)).is_empty()
    }
}

/// A line being collected, before numbering.
struct Line {
    key: TreeKey,
    part_number: String,
    keys: Vec<TreeKey>,
    quantity: u32,
}

impl Session {
    /// The BOM Structure of a component or subassembly (Normal unless set).
    pub fn bom_structure(&self, key: TreeKey) -> BomStructure {
        self.doc().bom_structure(key)
    }

    /// Sets the BOM Structure of a component or subassembly. One undo step; saved.
    pub fn set_bom_structure(&mut self, key: TreeKey, structure: BomStructure) -> Result<(), EditError> {
        self.execute(&Command::Cell(CellOp::SetBomStructure { key, structure })).map(|_| ())
    }

    /// The lines of a BOM view, in tree order.
    pub fn bill_of_materials(&self, view: BomView) -> Vec<BomRow> {
        let rows = self.component_tree();
        let mut children: HashMap<Option<TreeKey>, Vec<usize>> = HashMap::new();
        for (i, row) in rows.iter().enumerate() {
            children.entry(row.parent).or_default().push(i);
        }
        let tree = Tree { rows, children };
        let mut out = Vec::new();
        match view {
            BomView::Structured => self.structured(&tree, None, "", 0, &mut out),
            BomView::PartsOnly => {
                let mut lines = Vec::new();
                self.parts_only(&tree, None, false, &mut lines);
                for (n, line) in lines.into_iter().enumerate() {
                    out.push(self.bom_row(line, format!("{}", n + 1), 0));
                }
            }
        }
        out
    }

    /// A BOM view as CSV (RFC 4180; header Item, Part Number, Quantity, Description, BOM Structure).
    pub fn bom_csv(&self, view: BomView) -> String {
        let mut text = String::from("Item,Part Number,Quantity,Description,BOM Structure\r\n");
        for row in self.bill_of_materials(view) {
            let fields = [
                row.item,
                row.part_number,
                row.quantity.to_string(),
                row.description,
                structure_name(row.structure).to_owned(),
            ];
            let cells: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
            text.push_str(&cells.join(","));
            text.push_str("\r\n");
        }
        text
    }

    /// The structured lines under `parent`, numbered after `prefix`.
    fn structured(
        &self,
        tree: &Tree,
        parent: Option<TreeKey>,
        prefix: &str,
        level: u32,
        out: &mut Vec<BomRow>,
    ) {
        let mut lines = Vec::new();
        self.level_lines(tree, parent, &mut lines, 0);
        for (n, line) in lines.into_iter().enumerate() {
            let item = if prefix.is_empty() { format!("{}", n + 1) } else { format!("{prefix}.{}", n + 1) };
            let key = line.key;
            let row = self.bom_row(line, item.clone(), level);
            out.push(row);
            self.structured(tree, Some(key), &item, level + 1, out);
        }
    }

    /// The lines one level shows under `parent`: its rows, phantoms replaced by their own children.
    fn level_lines(&self, tree: &Tree, parent: Option<TreeKey>, lines: &mut Vec<Line>, depth: usize) {
        // A tree is never deeper than its rows; the bound guards against a corrupt file.
        if depth > tree.rows.len() {
            return;
        }
        for &i in tree.kids(parent) {
            let Some(row) = tree.rows.get(i) else { continue };
            match self.bom_structure(row.key) {
                BomStructure::Reference => {}
                BomStructure::Phantom => self.level_lines(tree, Some(row.key), lines, depth + 1),
                BomStructure::Normal | BomStructure::Purchased | BomStructure::Inseparable => {
                    add_line(lines, row.key, self.bom_part_number(row));
                }
            }
        }
    }

    /// The parts-only lines under `parent`. Inside an inseparable assembly only purchased rows count.
    fn parts_only(&self, tree: &Tree, parent: Option<TreeKey>, inseparable: bool, lines: &mut Vec<Line>) {
        for &i in tree.kids(parent) {
            let Some(row) = tree.rows.get(i) else { continue };
            let key = row.key;
            match self.bom_structure(key) {
                BomStructure::Reference => {}
                BomStructure::Purchased => add_line(lines, key, self.bom_part_number(row)),
                BomStructure::Inseparable if !inseparable => {
                    add_line(lines, key, self.bom_part_number(row));
                    self.parts_only(tree, Some(key), true, lines);
                }
                BomStructure::Inseparable | BomStructure::Phantom => {
                    self.parts_only(tree, Some(key), inseparable, lines);
                }
                BomStructure::Normal if tree.has_kids(key) => {
                    self.parts_only(tree, Some(key), inseparable, lines)
                }
                BomStructure::Normal if !inseparable => add_line(lines, key, self.bom_part_number(row)),
                BomStructure::Normal => {}
            }
        }
    }

    fn bom_part_number(&self, row: &TreeRow) -> String {
        let own = match row.key {
            TreeKey::Component(occurrence) => self.properties(Some(occurrence)).part_number,
            TreeKey::Subassembly(_) => String::new(),
        };
        if own.is_empty() { row.name.clone() } else { own }
    }

    fn bom_row(&self, line: Line, item: String, level: u32) -> BomRow {
        let description = match line.key {
            TreeKey::Component(occurrence) => self.properties(Some(occurrence)).description,
            TreeKey::Subassembly(_) => String::new(),
        };
        BomRow {
            item,
            level,
            part_number: line.part_number,
            description,
            quantity: line.quantity,
            structure: self.bom_structure(line.key),
            keys: line.keys,
        }
    }

    /// True when a component placed in the cell is left out of mass and volume (it or a row above it is
    /// Reference).
    pub(crate) fn bom_reference(&self, item: gc_core::AnyId) -> bool {
        let doc = self.doc();
        let mut at = Some(TreeKey::Component(Occurrence::Item(item)));
        for _ in 0..=doc.subassemblies().len() + 1 {
            let Some(key) = at else { return false };
            if doc.bom_structure(key) == BomStructure::Reference {
                return true;
            }
            at = doc.tree_parent(key);
        }
        false
    }
}

/// Adds one to the line with this part number, or a new line.
fn add_line(lines: &mut Vec<Line>, key: TreeKey, part_number: String) {
    if let Some(line) = lines.iter_mut().find(|l| l.part_number == part_number) {
        line.quantity = line.quantity.saturating_add(1);
        line.keys.push(key);
    } else {
        lines.push(Line { key, part_number, keys: vec![key], quantity: 1 });
    }
}

/// The name Inventor shows for a BOM Structure.
pub fn structure_name(structure: BomStructure) -> &'static str {
    match structure {
        BomStructure::Normal => "Normal",
        BomStructure::Phantom => "Phantom",
        BomStructure::Reference => "Reference",
        BomStructure::Purchased => "Purchased",
        BomStructure::Inseparable => "Inseparable",
    }
}

/// A CSV field, quoted when it holds a comma, quote or line break. A field a spreadsheet would read as a
/// formula (starting with `=`, `+`, `-`, `@`) gets a leading apostrophe.
fn csv_field(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@']) { format!("'{text}") } else { text.to_owned() };
    if text.contains([',', '"', '\n', '\r']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
}
