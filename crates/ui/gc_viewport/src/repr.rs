//! What a view representation stores of the viewport: the camera and the section view, as JSON text the
//! document keeps without reading. Written and read by hand (flat objects of numbers) so that the viewport
//! needs no JSON dependency; a text that does not parse is ignored and the current view stays.

use gc_draw::Camera;
use gc_math::{DQuat, DVec3};

use crate::section::{Section, SectionKind, SectionPlane};

/// The raw text of the value after `"key":` in a flat JSON object: up to the matching `]` for an array,
/// else up to the next `,` or `}`.
fn raw<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let at = text.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = text[at..].trim_start().strip_prefix(':')?.trim_start();
    let end = if rest.starts_with('[') {
        // Arrays may nest one level (the section's planes).
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in rest.char_indices() {
            match c {
                '[' => depth += 1,
                ']' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        end = Some(i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        end?
    } else {
        rest.find([',', '}']).unwrap_or(rest.len())
    };
    Some(rest[..end].trim())
}

fn number(text: &str, key: &str) -> Option<f64> {
    raw(text, key)?.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The numbers of a flat array like `[1, 2, 3]`.
fn numbers(array: &str) -> Option<Vec<f64>> {
    let inner = array.trim().strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    inner.split(',').map(|part| part.trim().parse::<f64>().ok().filter(|v| v.is_finite())).collect()
}

/// The camera as JSON text.
pub fn camera_to_json(camera: &Camera) -> String {
    let r = camera.rotation;
    format!(
        "{{\"pivot\":[{},{},{}],\"rotation\":[{},{},{},{}],\"distance\":{},\"fov_y\":{},\"ortho\":{}}}",
        camera.pivot.x,
        camera.pivot.y,
        camera.pivot.z,
        r.x,
        r.y,
        r.z,
        r.w,
        camera.distance,
        camera.fov_y,
        camera.ortho
    )
}

/// The camera from JSON text; `None` when the text is not one.
pub fn camera_from_json(text: &str) -> Option<Camera> {
    let pivot = numbers(raw(text, "pivot")?)?;
    let rotation = numbers(raw(text, "rotation")?)?;
    let [px, py, pz] = pivot.as_slice() else { return None };
    let [x, y, z, w] = rotation.as_slice() else { return None };
    let distance = number(text, "distance").filter(|d| *d > 0.0)?;
    let fov_y = number(text, "fov_y").filter(|f| *f > 0.0)?;
    let ortho = match raw(text, "ortho")? {
        "true" => true,
        "false" => false,
        _ => return None,
    };
    let rotation = DQuat::from_xyzw(*x, *y, *z, *w);
    if rotation.length_squared() < 1e-12 {
        return None;
    }
    Some(Camera { pivot: DVec3::new(*px, *py, *pz), rotation: rotation.normalize(), distance, fov_y, ortho })
}

/// The section as JSON text (the editing state is not part of it).
pub fn section_to_json(section: &Section) -> String {
    let kind = match section.kind {
        SectionKind::Half => "half",
        SectionKind::Quarter => "quarter",
        SectionKind::ThreeQuarter => "three_quarter",
    };
    let planes: Vec<String> =
        section.planes.iter().map(|p| format!("[{},{},{}]", p.axis, p.offset, u8::from(p.flipped))).collect();
    format!("{{\"kind\":\"{kind}\",\"planes\":[{}]}}", planes.join(","))
}

/// The section from JSON text; `None` when the text is not one.
pub fn section_from_json(text: &str) -> Option<Section> {
    let kind = match raw(text, "kind")?.trim_matches('"') {
        "half" => SectionKind::Half,
        "quarter" => SectionKind::Quarter,
        "three_quarter" => SectionKind::ThreeQuarter,
        _ => return None,
    };
    let array = raw(text, "planes")?;
    let inner = array.strip_prefix('[')?.strip_suffix(']')?;
    let mut planes = Vec::new();
    for part in
        inner.split(']').map(|p| p.trim_start_matches([',', ' ', '[']).trim()).filter(|p| !p.is_empty())
    {
        let values = numbers(&format!("[{part}]"))?;
        let [axis, offset, flipped] = values.as_slice().try_into().ok()?;
        if !(0.0..3.0).contains(&axis) {
            return None;
        }
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked to be 0, 1 or 2"
        )]
        planes.push(SectionPlane { axis: axis as usize, offset, flipped: flipped > 0.5 });
    }
    let wanted = if kind == SectionKind::Half { 1 } else { 2 };
    (planes.len() == wanted).then_some(Section { kind, planes, editing: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_and_the_section_round_trip_and_garbage_is_refused() {
        let camera =
            Camera { pivot: DVec3::new(0.1, -0.2, 0.3), distance: 2.5, ortho: false, ..Camera::default() };
        let back = camera_from_json(&camera_to_json(&camera)).expect("parses");
        assert!((back.pivot - camera.pivot).length() < 1e-12);
        assert!(back.rotation.dot(camera.rotation).abs() > 1.0 - 1e-12);
        assert_eq!((back.distance, back.fov_y, back.ortho), (camera.distance, camera.fov_y, false));

        let section = Section {
            kind: SectionKind::Quarter,
            planes: vec![
                SectionPlane { axis: 1, offset: 0.05, flipped: false },
                SectionPlane { axis: 0, offset: -0.1, flipped: true },
            ],
            editing: None,
        };
        assert_eq!(section_from_json(&section_to_json(&section)), Some(section));

        for bad in ["", "{}", "not json", "{\"pivot\":[1,2]}", "{\"kind\":\"half\",\"planes\":[]}"] {
            assert!(camera_from_json(bad).is_none() && section_from_json(bad).is_none(), "{bad}");
        }
    }
}
