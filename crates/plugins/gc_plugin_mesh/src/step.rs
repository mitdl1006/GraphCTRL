//! STEP files with tessellated shapes (ISO 10303-242, the tessellated geometry of ISO 10303-42): the
//! product tree with its placements, the triangles of each part, colours and the length unit.
//!
//! Only tessellated representations are read (`TRIANGULATED_FACE`, `TRIANGULATED_SURFACE_SET`, their
//! `COMPLEX_` forms with strips and fans, grouped by `TESSELLATED_SOLID` / `TESSELLATED_SHELL`); exact
//! B-rep geometry is skipped with a warning (it would need a tessellator of its own). Coordinates are
//! converted to metres; STEP is Z-up already.
//!
//! The reader is a small exchange-structure (ISO 10303-21) parser of its own, so that nothing beyond this
//! crate is needed.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Assembly, AssemblyMesh, AssemblyNode, IDENTITY, ImportError, multiply};

/// A parameter of an entity instance.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Ref(u64),
    Num(f64),
    Str(String),
    Enum(String),
    List(Vec<Value>),
    /// A typed parameter, `LENGTH_MEASURE(1.)`.
    Typed(String, Vec<Value>),
    /// `$` or `*`.
    Unset,
}

impl Value {
    fn reference(&self) -> Option<u64> {
        match self {
            Value::Ref(r) => Some(*r),
            _ => None,
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Typed(_, args) => args.first()?.number(),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    fn list(&self) -> &[Value] {
        match self {
            Value::List(items) => items,
            _ => &[],
        }
    }
}

/// An entity instance: one record (`#1=NAME(...)`), or several for a complex instance
/// (`#1=(A(...) B(...))`), by name.
type Records = Vec<(String, Vec<Value>)>;

/// How deep parameter lists may nest: real files need a handful of levels; a damaged or hostile file must
/// not exhaust the stack.
const MAX_DEPTH: usize = 256;

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
    /// Lists open around the current position.
    depth: usize,
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        loop {
            while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
            if self.text.get(self.at..self.at + 2) == Some(b"/*") {
                let end = self.text[self.at + 2..].windows(2).position(|w| w == b"*/");
                self.at = end.map_or(self.text.len(), |e| self.at + 2 + e + 2);
            } else {
                return;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_space();
        self.text.get(self.at).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", byte as char, self.at))
        }
    }

    fn keyword(&mut self) -> String {
        self.skip_space();
        let start = self.at;
        while self.text.get(self.at).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-') {
            self.at += 1;
        }
        String::from_utf8_lossy(&self.text[start..self.at]).to_ascii_uppercase()
    }

    fn arguments(&mut self) -> Result<Vec<Value>, String> {
        if self.depth >= MAX_DEPTH {
            return Err(format!("lists nested deeper than {MAX_DEPTH} at byte {}", self.at));
        }
        self.depth += 1;
        let result = self.arguments_inside();
        self.depth -= 1;
        result
    }

    fn arguments_inside(&mut self) -> Result<Vec<Value>, String> {
        self.expect(b'(')?;
        let mut out = Vec::new();
        if self.peek() == Some(b')') {
            self.at += 1;
            return Ok(out);
        }
        loop {
            out.push(self.value()?);
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b')') => {
                    self.at += 1;
                    return Ok(out);
                }
                _ => return Err(format!("expected `,` or `)` at byte {}", self.at)),
            }
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek().ok_or("unexpected end of file")? {
            b'#' => {
                self.at += 1;
                let start = self.at;
                while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
                let digits = String::from_utf8_lossy(&self.text[start..self.at]);
                digits.parse().map(Value::Ref).map_err(|_bad| format!("bad reference at byte {start}"))
            }
            b'\'' => {
                self.at += 1;
                let mut s = Vec::new();
                loop {
                    match self.text.get(self.at) {
                        None => return Err("unterminated string".to_owned()),
                        Some(b'\'') if self.text.get(self.at + 1) == Some(&b'\'') => {
                            s.push(b'\'');
                            self.at += 2;
                        }
                        Some(b'\'') => {
                            self.at += 1;
                            return Ok(Value::Str(String::from_utf8_lossy(&s).into_owned()));
                        }
                        Some(c) => {
                            s.push(*c);
                            self.at += 1;
                        }
                    }
                }
            }
            b'.' => {
                self.at += 1;
                let start = self.at;
                while self.text.get(self.at).is_some_and(|c| *c != b'.') {
                    self.at += 1;
                }
                let name = String::from_utf8_lossy(&self.text[start..self.at]).to_ascii_uppercase();
                self.at += 1;
                Ok(Value::Enum(name))
            }
            b'(' => Ok(Value::List(self.arguments()?)),
            b'$' | b'*' => {
                self.at += 1;
                Ok(Value::Unset)
            }
            b'"' => {
                // Binary: kept as text, unused here.
                self.at += 1;
                while self.text.get(self.at).is_some_and(|c| *c != b'"') {
                    self.at += 1;
                }
                self.at += 1;
                Ok(Value::Unset)
            }
            c if c == b'-' || c == b'+' || c.is_ascii_digit() => {
                let start = self.at;
                self.at += 1;
                while self
                    .text
                    .get(self.at)
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'E' | b'e' | b'-' | b'+'))
                {
                    self.at += 1;
                }
                let text = String::from_utf8_lossy(&self.text[start..self.at]);
                // `1.` and `1.E-3` are valid STEP reals.
                let normal = text.replace(".E", ".0E").replace(".e", ".0e");
                let normal = if normal.ends_with('.') { format!("{normal}0") } else { normal };
                normal.parse().map(Value::Num).map_err(|_bad| format!("bad number `{text}` at byte {start}"))
            }
            c if c.is_ascii_alphabetic() => {
                let name = self.keyword();
                Ok(Value::Typed(name, self.arguments()?))
            }
            c => Err(format!("unexpected `{}` at byte {}", c as char, self.at)),
        }
    }

    /// The instances of the DATA section(s).
    fn data(&mut self) -> Result<BTreeMap<u64, Records>, String> {
        let mut out = BTreeMap::new();
        let text = self.text;
        // After the header section, so that a "DATA" in its strings is not taken for the keyword.
        let mut search = find(text, b"ENDSEC").map_or(0, |e| e + 6);
        while let Some(found) = find(&text[search..], b"DATA") {
            self.at = search + found + 4;
            // `DATA;` or `DATA('name', ...);`
            if self.peek() == Some(b'(') {
                self.arguments()?;
            }
            self.expect(b';')?;
            while self.peek() == Some(b'#') {
                self.at += 1;
                let start = self.at;
                while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
                let id: u64 = String::from_utf8_lossy(&self.text[start..self.at])
                    .parse()
                    .map_err(|_bad| format!("bad instance name at byte {start}"))?;
                self.expect(b'=')?;
                let records = if self.peek() == Some(b'(') {
                    self.at += 1;
                    let mut records = Vec::new();
                    while self.peek() != Some(b')') {
                        let name = self.keyword();
                        if name.is_empty() {
                            return Err(format!("bad complex instance #{id}"));
                        }
                        records.push((name, self.arguments()?));
                    }
                    self.at += 1;
                    records
                } else {
                    let name = self.keyword();
                    vec![(name, self.arguments()?)]
                };
                self.expect(b';')?;
                out.insert(id, records);
            }
            let rest = &text[self.at..];
            let Some(end) = find(rest, b"ENDSEC") else {
                return Err("DATA section without ENDSEC".to_owned());
            };
            search = self.at + end + 6;
        }
        Ok(out)
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The instances of a file, with lookups by name.
struct Model {
    instances: BTreeMap<u64, Records>,
}

impl Model {
    /// The arguments of the record `name` of instance `id`.
    fn args(&self, id: u64, name: &str) -> Option<&[Value]> {
        self.instances.get(&id)?.iter().find(|(n, _)| n == name).map(|(_, args)| args.as_slice())
    }

    fn is(&self, id: u64, name: &str) -> bool {
        self.args(id, name).is_some()
    }

    /// The first record's name and arguments.
    fn first(&self, id: u64) -> Option<(&str, &[Value])> {
        self.instances.get(&id)?.first().map(|(n, a)| (n.as_str(), a.as_slice()))
    }

    fn with(&self, name: &str) -> impl Iterator<Item = (u64, &[Value])> + '_ {
        let name = name.to_owned();
        self.instances.iter().filter_map(move |(id, records)| {
            records.iter().find(|(n, _)| *n == name).map(|(_, args)| (*id, args.as_slice()))
        })
    }

    fn point(&self, id: u64) -> Option<[f64; 3]> {
        let args = self.args(id, "CARTESIAN_POINT")?;
        triple(args.get(1)?.list())
    }

    fn direction(&self, id: u64) -> Option<[f64; 3]> {
        let args = self.args(id, "DIRECTION")?;
        triple(args.get(1)?.list())
    }

    /// An `AXIS2_PLACEMENT_3D` as a column-major matrix (translation in file units).
    fn placement(&self, id: u64) -> Option<[[f64; 4]; 4]> {
        let args = self.args(id, "AXIS2_PLACEMENT_3D")?;
        let origin = self.point(args.get(1)?.reference()?)?;
        let z =
            args.get(2).and_then(Value::reference).and_then(|d| self.direction(d)).unwrap_or([0.0, 0.0, 1.0]);
        let x =
            args.get(3).and_then(Value::reference).and_then(|d| self.direction(d)).unwrap_or([1.0, 0.0, 0.0]);
        let z = normalize(z)?;
        // X made perpendicular to Z, as the standard says.
        let dot = x[0] * z[0] + x[1] * z[1] + x[2] * z[2];
        let x = normalize([x[0] - dot * z[0], x[1] - dot * z[1], x[2] - dot * z[2]]).or_else(|| {
            normalize(if z[0].abs() < 0.9 {
                [1.0 - z[0] * z[0], -z[0] * z[1], -z[0] * z[2]]
            } else {
                [-z[1] * z[0], 1.0 - z[1] * z[1], -z[1] * z[2]]
            })
        })?;
        let y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]];
        Some([
            [x[0], x[1], x[2], 0.0],
            [y[0], y[1], y[2], 0.0],
            [z[0], z[1], z[2], 0.0],
            [origin[0], origin[1], origin[2], 1.0],
        ])
    }
}

fn triple(values: &[Value]) -> Option<[f64; 3]> {
    Some([
        values.first()?.number()?,
        values.get(1)?.number()?,
        values.get(2).and_then(Value::number).unwrap_or(0.0),
    ])
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (length > 1e-12 && length.is_finite()).then(|| [v[0] / length, v[1] / length, v[2] / length])
}

/// The inverse of a rigid column-major matrix.
fn rigid_inverse(m: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let r = |row: usize, column: usize| m[column][row];
    let t = [m[3][0], m[3][1], m[3][2]];
    let mut out = IDENTITY;
    for row in 0..3 {
        for column in 0..3 {
            out[column][row] = r(column, row);
        }
        out[3][row] = -(0..3).map(|k| r(k, row) * t[k]).sum::<f64>();
    }
    out
}

fn scaled_translation(mut m: [[f64; 4]; 4], scale: f64) -> [[f64; 4]; 4] {
    for v in &mut m[3][..3] {
        *v *= scale;
    }
    m
}

/// Metres per file length unit: the first length unit the file declares (millimetres when it declares
/// none, with a warning).
fn length_unit(model: &Model, warnings: &mut Vec<String>) -> f64 {
    for (id, _) in model.with("LENGTH_UNIT") {
        if let Some(args) = model.args(id, "SI_UNIT") {
            let prefix = match args.first() {
                Some(Value::Enum(p)) => p.as_str(),
                _ => "",
            };
            let factor = match prefix {
                "MILLI" => 1e-3,
                "CENTI" => 1e-2,
                "DECI" => 1e-1,
                "MICRO" => 1e-6,
                "KILO" => 1e3,
                _ => 1.0,
            };
            return factor;
        }
        if let Some(args) = model.args(id, "CONVERSION_BASED_UNIT") {
            let name = args.first().and_then(Value::text).unwrap_or_default().to_ascii_uppercase();
            return match name.as_str() {
                "INCH" => 0.0254,
                "FOOT" => 0.3048,
                _ => {
                    warnings.push(format!("length unit `{name}` is not known; millimetres assumed"));
                    1e-3
                }
            };
        }
    }
    warnings.push("no length unit; millimetres assumed".to_owned());
    1e-3
}

/// The triangles of a tessellated face or surface set, in file units: positions and indices.
fn triangles(model: &Model, id: u64) -> Option<(Vec<[f64; 3]>, Vec<u32>)> {
    let (name, args) = model.first(id)?;
    // Arguments: name, coordinates, pnmax, normals, [geometric_link,] pnindex, triangles | strips, fans.
    let face = name.ends_with("FACE");
    let complex = name.starts_with("COMPLEX_");
    let coordinates = model.args(args.get(1)?.reference()?, "COORDINATES_LIST")?;
    let points: Vec<[f64; 3]> = coordinates.get(2)?.list().iter().filter_map(|p| triple(p.list())).collect();
    let first = if face { 5 } else { 4 };
    let pnindex: Vec<usize> =
        args.get(first)?.list().iter().filter_map(Value::number).map(|n| n as usize).collect();
    let vertex = |i: usize| -> Option<usize> {
        let at = if pnindex.is_empty() { i } else { *pnindex.get(i.checked_sub(1)?)? };
        at.checked_sub(1).filter(|v| *v < points.len())
    };
    let index_list =
        |v: &Value| -> Vec<usize> { v.list().iter().filter_map(Value::number).map(|n| n as usize).collect() };
    let mut out: Vec<u32> = Vec::new();
    let mut push = |a: usize, b: usize, c: usize| {
        if let (Some(a), Some(b), Some(c)) = (vertex(a), vertex(b), vertex(c))
            && a != b
            && b != c
            && a != c
        {
            out.extend([a, b, c].map(|v| u32::try_from(v).unwrap_or(u32::MAX)));
        }
    };
    if complex {
        for strip in args.get(first + 1)?.list() {
            let s = index_list(strip);
            for k in 2..s.len() {
                if k % 2 == 0 { push(s[k - 2], s[k - 1], s[k]) } else { push(s[k - 1], s[k - 2], s[k]) }
            }
        }
        for fan in args.get(first + 2).map(Value::list).unwrap_or_default() {
            let f = index_list(fan);
            for k in 2..f.len() {
                push(f[0], f[k - 1], f[k]);
            }
        }
    } else {
        for triangle in args.get(first + 1)?.list() {
            let t = index_list(triangle);
            if let [a, b, c] = t[..] {
                push(a, b, c);
            }
        }
    }
    Some((points, out))
}

const TESSELLATED_FACES: [&str; 4] = [
    "TRIANGULATED_FACE",
    "TRIANGULATED_SURFACE_SET",
    "COMPLEX_TRIANGULATED_FACE",
    "COMPLEX_TRIANGULATED_SURFACE_SET",
];

fn is_face(model: &Model, id: u64) -> bool {
    model.first(id).is_some_and(|(name, _)| TESSELLATED_FACES.contains(&name))
}

/// The faces under a tessellated item (a solid or shell lists them; a face is itself).
fn faces_of(model: &Model, id: u64, out: &mut Vec<u64>, depth: usize) {
    if depth > 8 {
        return;
    }
    if is_face(model, id) {
        out.push(id);
        return;
    }
    if let Some((name, args)) = model.first(id)
        && (name == "TESSELLATED_SOLID" || name == "TESSELLATED_SHELL")
    {
        for item in args.get(1).map(Value::list).unwrap_or_default() {
            if let Some(r) = item.reference() {
                faces_of(model, r, out, depth + 1);
            }
        }
    }
}

/// The colour a `STYLED_ITEM` gives, looked for down its style references.
fn colour(model: &Model, id: u64, depth: usize) -> Option<[f32; 4]> {
    if depth > 10 {
        return None;
    }
    let (name, args) = model.first(id)?;
    if name == "COLOUR_RGB" {
        #[allow(clippy::cast_possible_truncation, reason = "colours are 0..1")]
        let c = |i: usize| args.get(i).and_then(Value::number).unwrap_or(0.7).clamp(0.0, 1.0) as f32;
        return Some([c(1), c(2), c(3), 1.0]);
    }
    fn refs(values: &[Value], out: &mut Vec<u64>) {
        for v in values {
            match v {
                Value::Ref(r) => out.push(*r),
                Value::List(items) | Value::Typed(_, items) => refs(items, out),
                _ => {}
            }
        }
    }
    let mut below = Vec::new();
    refs(args, &mut below);
    below.into_iter().find_map(|r| colour(model, r, depth + 1))
}

/// A product occurrence still to be added: definition, name, placement, parent node, depth.
type Pending = (u64, String, [[f64; 4]; 4], Option<usize>, usize);

/// Reads a STEP file with tessellated shapes (AP242).
pub fn read_step(bytes: &[u8]) -> Result<Assembly, ImportError> {
    if find(bytes, b"ISO-10303-21").is_none() {
        return Err(ImportError::Unreadable("not a STEP (ISO 10303-21) file".to_owned()));
    }
    let instances = Parser { text: bytes, at: 0, depth: 0 }.data().map_err(ImportError::Unreadable)?;
    let model = Model { instances };
    let mut warnings = Vec::new();
    let unit = length_unit(&model, &mut warnings);

    // Colours of tessellated items.
    let mut colours: BTreeMap<u64, [f32; 4]> = BTreeMap::new();
    for (_, args) in model.with("STYLED_ITEM") {
        let Some(item) = args.get(2).and_then(Value::reference) else { continue };
        let styles = args.get(1).map(Value::list).unwrap_or_default();
        if let Some(c) = styles.iter().filter_map(Value::reference).find_map(|r| colour(&model, r, 0)) {
            colours.insert(item, c);
        }
    }

    // Representations holding tessellated items, and their meshes (one per top-level item).
    let mut meshes: Vec<AssemblyMesh> = Vec::new();
    let mut rep_meshes: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    let mut exact = 0usize;
    for (id, records) in &model.instances {
        let Some((name, args)) = records.first() else { continue };
        if !name.ends_with("REPRESENTATION") || name.ends_with("_RELATIONSHIP") {
            continue;
        }
        let rep_name = args.first().and_then(Value::text).unwrap_or_default().to_owned();
        for item in args.get(1).map(Value::list).unwrap_or_default().iter().filter_map(Value::reference) {
            let mut faces = Vec::new();
            faces_of(&model, item, &mut faces, 0);
            if faces.is_empty() {
                if model.first(item).is_some_and(|(n, _)| n.contains("BREP") || n == "MANIFOLD_SOLID_BREP") {
                    exact += 1;
                }
                continue;
            }
            let mut positions: Vec<[f32; 3]> = Vec::new();
            let mut indices: Vec<u32> = Vec::new();
            for face in &faces {
                let Some((points, tris)) = triangles(&model, *face) else {
                    warnings.push(format!("#{face}: damaged tessellated face; skipped"));
                    continue;
                };
                let base = u32::try_from(positions.len()).unwrap_or(u32::MAX);
                #[allow(clippy::cast_possible_truncation, reason = "mesh coordinates are f32")]
                positions.extend(points.iter().map(|p| p.map(|v| (v * unit) as f32)));
                indices.extend(tris.iter().map(|i| i + base));
            }
            if indices.is_empty() {
                continue;
            }
            let colour = colours
                .get(&item)
                .or_else(|| faces.iter().find_map(|f| colours.get(f)))
                .copied()
                .unwrap_or([0.7, 0.7, 0.7, 1.0]);
            let item_name =
                model.first(item).and_then(|(_, a)| a.first()?.text().map(str::to_owned)).unwrap_or_default();
            let name = if item_name.is_empty() { rep_name.clone() } else { item_name };
            meshes.push(AssemblyMesh { name, positions, indices, color: colour });
            rep_meshes.entry(*id).or_default().push(meshes.len() - 1);
        }
    }
    if exact > 0 {
        warnings.push(format!("{exact} exact (B-rep) solids have no tessellation in the file; skipped"));
    }
    if meshes.is_empty() {
        return Err(ImportError::Empty);
    }

    // Representations that belong together (a shape representation and the tessellated one it is related
    // to without a transformation).
    let mut related: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for (id, records) in &model.instances {
        let is_transform =
            records.iter().any(|(n, _)| n == "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION");
        if is_transform {
            continue;
        }
        if let Some(args) = model
            .args(*id, "SHAPE_REPRESENTATION_RELATIONSHIP")
            .or_else(|| model.args(*id, "REPRESENTATION_RELATIONSHIP"))
            && let (Some(a), Some(b)) =
                (args.get(2).and_then(Value::reference), args.get(3).and_then(Value::reference))
        {
            related.entry(a).or_default().push(b);
            related.entry(b).or_default().push(a);
        }
    }
    let meshes_of_rep = |rep: u64| -> Vec<usize> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![rep];
        let mut out = Vec::new();
        while let Some(r) = stack.pop() {
            if !seen.insert(r) {
                continue;
            }
            out.extend(rep_meshes.get(&r).into_iter().flatten().copied());
            stack.extend(related.get(&r).into_iter().flatten().copied());
        }
        out
    };

    // Product definitions: their name and their shape representation.
    let product_name = |pd: u64| -> String {
        let formation = model.args(pd, "PRODUCT_DEFINITION").and_then(|a| a.get(2)?.reference());
        let product = formation.and_then(|f| {
            model
                .args(f, "PRODUCT_DEFINITION_FORMATION")
                .or_else(|| model.args(f, "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE"))?
                .get(2)?
                .reference()
        });
        product
            .and_then(|p| {
                let args = model.args(p, "PRODUCT")?;
                args.get(1).and_then(Value::text).filter(|s| !s.is_empty()).or_else(|| args.first()?.text())
            })
            .map_or_else(|| format!("Part #{pd}"), str::to_owned)
    };
    let mut shape_of: BTreeMap<u64, u64> = BTreeMap::new();
    for (_, args) in model.with("SHAPE_DEFINITION_REPRESENTATION") {
        let (Some(pds), Some(rep)) =
            (args.first().and_then(Value::reference), args.get(1).and_then(Value::reference))
        else {
            continue;
        };
        if let Some(pd) = model.args(pds, "PRODUCT_DEFINITION_SHAPE").and_then(|a| a.get(2)?.reference())
            && model.is(pd, "PRODUCT_DEFINITION")
        {
            shape_of.insert(pd, rep);
        }
    }
    // Occurrences: parent and child product definitions, and the child's placement in the parent.
    struct Use {
        parent: u64,
        child: u64,
        name: String,
        matrix: [[f64; 4]; 4],
    }
    let mut uses: Vec<Use> = Vec::new();
    for (nauo, args) in model.with("NEXT_ASSEMBLY_USAGE_OCCURRENCE") {
        let (Some(parent), Some(child)) =
            (args.get(3).and_then(Value::reference), args.get(4).and_then(Value::reference))
        else {
            continue;
        };
        let name = args
            .get(1)
            .and_then(Value::text)
            .filter(|s| !s.is_empty())
            .map_or_else(|| product_name(child), str::to_owned);
        let mut matrix = IDENTITY;
        for (_, cdsr) in model.with("CONTEXT_DEPENDENT_SHAPE_REPRESENTATION") {
            let on_this = cdsr
                .get(1)
                .and_then(Value::reference)
                .and_then(|pds| model.args(pds, "PRODUCT_DEFINITION_SHAPE")?.get(2)?.reference())
                == Some(nauo);
            if !on_this {
                continue;
            }
            let Some(rel) = cdsr.first().and_then(Value::reference) else { continue };
            let transform = model
                .args(rel, "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION")
                .and_then(|a| a.first()?.reference());
            if let Some(idt) = transform.and_then(|t| model.args(t, "ITEM_DEFINED_TRANSFORMATION"))
                && let (Some(a), Some(b)) = (
                    idt.get(2).and_then(Value::reference).and_then(|p| model.placement(p)),
                    idt.get(3).and_then(Value::reference).and_then(|p| model.placement(p)),
                )
            {
                // The child's frame `a` is put onto the parent's `b`.
                matrix = scaled_translation(multiply(&b, &rigid_inverse(&a)), unit);
            }
        }
        uses.push(Use { parent, child, name, matrix });
    }

    let mut assembly = Assembly { nodes: Vec::new(), meshes, warnings };
    if uses.is_empty() && shape_of.is_empty() {
        // No product structure: one part per representation.
        for (rep, list) in &rep_meshes {
            let name = model
                .first(*rep)
                .and_then(|(_, a)| a.first()?.text().map(str::to_owned))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("Part #{rep}"));
            assembly.nodes.push(AssemblyNode { name, parent: None, matrix: IDENTITY, meshes: list.clone() });
        }
        return Ok(assembly);
    }
    let children: BTreeSet<u64> = uses.iter().map(|u| u.child).collect();
    let mut roots: Vec<u64> = shape_of
        .keys()
        .copied()
        .chain(uses.iter().map(|u| u.parent))
        .filter(|pd| !children.contains(pd))
        .collect();
    roots.sort_unstable();
    roots.dedup();
    // Depth first; a definition used several times appears once per use (sharing its meshes).
    let mut stack: Vec<Pending> =
        roots.iter().rev().map(|pd| (*pd, product_name(*pd), IDENTITY, None, 0)).collect();
    while let Some((pd, name, matrix, parent, depth)) = stack.pop() {
        let meshes = shape_of.get(&pd).map(|rep| meshes_of_rep(*rep)).unwrap_or_default();
        assembly.nodes.push(AssemblyNode { name, parent, matrix, meshes });
        let me = assembly.nodes.len() - 1;
        if depth > 32 {
            assembly.warnings.push("the product tree is deeper than 32 levels; cut".to_owned());
            continue;
        }
        for u in uses.iter().filter(|u| u.parent == pd).collect::<Vec<_>>().into_iter().rev() {
            stack.push((u.child, u.name.clone(), u.matrix, Some(me), depth + 1));
        }
    }
    Ok(assembly)
}
