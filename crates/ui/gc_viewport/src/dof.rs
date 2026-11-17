//! Degrees-of-freedom symbols (Inventor's "Degrees of Freedom" in the View tab, Visibility panel): at each
//! component whose symbol is shown, an arrow along every direction it can still slide, an arc round every
//! axis it can still turn about, and a small cube when it is constrained to components that are not fully
//! constrained themselves. The shapes are our own drawings; the original help does not show them.
//! Reference: docs/ux/inventor-assembly-parity.md A15/A16 (GUIDs truncated there; original not read).

use egui::{Color32, Painter, Pos2, Shape, Stroke};
use gc_math::DVec3;
use gc_session::Session;

/// Arrows slide, arcs turn.
const SLIDE: Color32 = Color32::from_rgb(0x1E, 0x6F, 0xD9);
const TURN: Color32 = Color32::from_rgb(0xE0, 0x7A, 0x10);

/// Draws the symbols the session says are shown. `size` is the world length of an arrow.
pub(crate) fn paint(
    painter: &Painter,
    screen: &impl Fn(DVec3) -> Option<Pos2>,
    session: &Session,
    size: f64,
) {
    for (_, symbol) in session.dof_symbols() {
        let origin = DVec3::from_array(symbol.origin);
        for direction in &symbol.translations {
            let tip = origin + DVec3::from_array(*direction) * size;
            arrow(painter, screen, origin, tip, Stroke::new(2.0, SLIDE));
            arrow(painter, screen, origin, origin - (tip - origin), Stroke::new(2.0, SLIDE));
        }
        for axis in &symbol.rotations {
            arc(
                painter,
                screen,
                DVec3::from_array(axis.point),
                DVec3::from_array(axis.direction),
                size * 0.7,
            );
        }
        if !symbol.relative_to.is_empty() {
            cube(painter, screen, origin + DVec3::splat(size * 0.45), size * 0.12);
        }
    }
}

/// A line with a head at `to`.
fn arrow(painter: &Painter, screen: &impl Fn(DVec3) -> Option<Pos2>, from: DVec3, to: DVec3, stroke: Stroke) {
    let (Some(a), Some(b)) = (screen(from), screen(to)) else { return };
    painter.line_segment([a, b], stroke);
    let along = b - a;
    let length = along.length();
    if length < 6.0 {
        return;
    }
    let unit = along / length;
    let side = egui::vec2(-unit.y, unit.x);
    let back = b - unit * 8.0;
    painter.add(Shape::convex_polygon(
        vec![b, back + side * 4.0, back - side * 4.0],
        stroke.color,
        Stroke::NONE,
    ));
}

/// Three quarters of a circle round `axis` through `centre`, with a head at its end.
fn arc(painter: &Painter, screen: &impl Fn(DVec3) -> Option<Pos2>, centre: DVec3, axis: DVec3, radius: f64) {
    let axis = axis.normalize_or_zero();
    if axis == DVec3::ZERO {
        return;
    }
    let helper = if axis.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u);
    let steps = 24;
    let at = |i: usize| {
        let angle = std::f64::consts::TAU * 0.75 * i as f64 / steps as f64;
        centre + (u * gc_math::fm::cos(angle) + v * gc_math::fm::sin(angle)) * radius
    };
    let points: Vec<Pos2> = (0..=steps).filter_map(|i| screen(at(i))).collect();
    if points.len() == steps + 1 {
        painter.add(Shape::line(points, Stroke::new(2.0, TURN)));
        arrow(painter, screen, at(steps - 1), at(steps), Stroke::new(2.0, TURN));
    }
}

/// A small wire cube: the component is constrained to another one that is still free.
fn cube(painter: &Painter, screen: &impl Fn(DVec3) -> Option<Pos2>, centre: DVec3, half: f64) {
    let corner = |i: usize| {
        centre
            + DVec3::new(
                if i & 1 == 0 { -half } else { half },
                if i & 2 == 0 { -half } else { half },
                if i & 4 == 0 { -half } else { half },
            )
    };
    let stroke = Stroke::new(1.5, TURN);
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0
                && let (Some(a), Some(b)) = (screen(corner(i)), screen(corner(i | bit)))
            {
                painter.line_segment([a, b], stroke);
            }
        }
    }
}
