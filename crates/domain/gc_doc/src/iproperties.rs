//! iProperties (Inventor 2026 help, iProperties Reference, GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3): the
//! Summary and Project iProperties of the document and of each component ("to classify and manage files,
//! search for files, create reports"), and the Physical tab's material density (iProperties - Physical Tab -
//! Reference, GUID-877564AC-78CE-4216-9913-003A2DE8D698: "The density of the default material is 1 kg/m^3").
//! Saved with the document; the physical values themselves are worked out from the shapes when asked.

use serde::{Deserialize, Serialize};

/// The BOM Structure of a component or subassembly (About the BOM Structure, GUID-A0A95805-CBF6-4524-8314-4AC8B53C19CF).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BomStructure {
    /// "the default BOM Structure for most components … numbered and included in quantity calculations".
    #[default]
    Normal,
    /// "exist in the design, but are not line items … children appear in Structured BOM views … the
    /// quantity of children is multiplied by the quantity of the phantom component".
    Phantom,
    /// "ignored in the BOM … excluded from mass and volume calculations … child components … are ignored".
    Reference,
    /// "not fabricated … a single BOM line item, even though it is a part or an assembly"; its children
    /// are listed in the structured BOM but hidden in parts-only.
    Purchased,
    /// "must be physically damaged to separate them": a part in parts-only, like a purchased assembly,
    /// except that purchased children inside it still show.
    Inseparable,
}

/// The default density, kg/m³ (the Physical tab's default material).
pub const DEFAULT_DENSITY: f64 = 1.0;

/// The iProperties kept for the document or a component.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IProperties {
    /// Summary: Title.
    pub title: String,
    /// Summary: Subject.
    pub subject: String,
    /// Summary: Author.
    pub author: String,
    /// Summary: Company.
    pub company: String,
    /// Summary: Comments.
    pub comments: String,
    /// Project: Part Number (empty: the name stands for it, as Inventor uses the file name).
    pub part_number: String,
    /// Project: Description.
    pub description: String,
    /// Project: Revision Number.
    pub revision: String,
    /// Project: Project.
    pub project: String,
    /// Project: Designer.
    pub designer: String,
    /// Project: Vendor.
    pub vendor: String,
    /// Physical: the material's density, kg/m³ (`None`: the default material, 1 kg/m³).
    pub density: Option<f64>,
    /// Physical: a mass entered by hand, kg, used instead of the one worked out (an override).
    pub mass_override: Option<f64>,
}

impl IProperties {
    /// True when nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// True when the numbers can be used: a density and a mass override, when given, positive and finite.
    pub fn is_valid(&self) -> bool {
        let ok = |v: Option<f64>| v.is_none_or(|v| v.is_finite() && v > 0.0);
        ok(self.density) && ok(self.mass_override)
    }

    /// The density used, kg/m³.
    pub fn density_or_default(&self) -> f64 {
        self.density.unwrap_or(DEFAULT_DENSITY)
    }
}
