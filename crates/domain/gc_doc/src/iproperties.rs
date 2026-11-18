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

/// The Bill of Materials settings (Inventor 2026 help, Part Number Row Merge Settings,
/// GUID-EBBCAEA4-B876-4B39-8D33-F3DC9E18E620; To Work with Bill of Materials,
/// GUID-7A1FC6F1-8833-4A76-9E2B-AA6FE5FDF223, BOM Settings).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BomSettings {
    /// Merge Rows on Part Number Match: components with the same part number are one row.
    pub merge_part_numbers: bool,
    /// Excluded Part Numbers: values that are not merged into one row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merge_excluded: Vec<String>,
    /// Hide Suppressed components in BOM: otherwise a suppressed component shows with quantity 0.
    #[serde(default)]
    pub hide_suppressed: bool,
}

impl Default for BomSettings {
    fn default() -> Self {
        Self { merge_part_numbers: true, merge_excluded: Vec::new(), hide_suppressed: false }
    }
}

impl BomSettings {
    /// True for the settings a new document has.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

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
    /// Status: "In the Status field, enter any status classification" (iProperties Reference,
    /// GUID-8ABF17C5-B9EB-47B5-92CF-9FBD335334E3).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// Custom: iProperties the user adds, by name ("Custom iProperties added in the BOM dialog box are
    /// automatically saved to the Custom tab", same page). Names are not empty and not repeated.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomProperty>,
}

/// A custom iProperty: a name and its value as text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomProperty {
    /// Its name (the column's title when it is shown in the BOM).
    pub name: String,
    /// Its value.
    pub value: String,
}

impl IProperties {
    /// The value of a custom iProperty, when there is one of that name.
    pub fn custom_value(&self, name: &str) -> Option<&str> {
        self.custom.iter().find(|p| p.name == name).map(|p| p.value.as_str())
    }

    /// True when nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// True when the numbers can be used: a density and a mass override, when given, positive and finite.
    pub fn is_valid(&self) -> bool {
        let ok = |v: Option<f64>| v.is_none_or(|v| v.is_finite() && v > 0.0);
        let named = self.custom.iter().enumerate().all(|(i, p)| {
            !p.name.trim().is_empty() && self.custom.iter().skip(i + 1).all(|other| other.name != p.name)
        });
        ok(self.density) && ok(self.mass_override) && named
    }

    /// The density used, kg/m³.
    pub fn density_or_default(&self) -> f64 {
        self.density.unwrap_or(DEFAULT_DENSITY)
    }
}
