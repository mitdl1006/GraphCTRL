//! File importers.

use gc_mesh::MeshData;

use crate::ExtensionPoint;

/// One mesh of an imported file.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedPart {
    /// Name from the file (object or group name), or empty.
    pub name: String,
    /// The triangles, in the file's own units and frame.
    pub mesh: MeshData,
}

/// The result of importing a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    /// The meshes found, in file order.
    pub parts: Vec<ImportedPart>,
    /// Things the user should know, such as skipped unsupported content.
    pub warnings: Vec<String>,
}

/// Why a file could not be imported.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// The bytes are not a file of this format, or the file is damaged.
    #[error("the file could not be read: {0}")]
    Malformed(String),
    /// The file was read but contains no triangles.
    #[error("the file contains no geometry")]
    Empty,
}

/// Reads one or more file formats into meshes.
pub trait Importer: Send + Sync {
    /// Unique id, prefixed with the plugin id.
    fn id(&self) -> &str;

    /// File extensions this importer reads, lower case and without the dot.
    fn extensions(&self) -> &[&str];

    /// Parses a whole file. `file_name` is only a hint (for the format and for part names).
    fn import(&self, bytes: &[u8], file_name: &str) -> Result<Imported, ImportError>;
}

/// Where importers register.
pub const IMPORTERS: ExtensionPoint<dyn Importer> = ExtensionPoint::new("gc.importers");
