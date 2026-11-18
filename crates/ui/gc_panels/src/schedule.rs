//! The schedule panel (plan 9.4/9.5): a Gantt chart with one row per robot (a bar for each task it is given,
//! from `start` to `end`), the makespan, and the tasks that are never done with why. It is worked out when asked
//! (`Session::schedule` plans every bid and is not instant), and shown until the next time.

use egui::{Align2, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use gc_i18n::{Lang, tr};
use gc_session::{Schedule, Session};
use gc_ui::colors;

/// The schedule shown last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleUi {
    /// The result, when it was worked out.
    pub result: Option<Schedule>,
}

/// The panel: a button to work the schedule out, then the chart and the blocked tasks.
pub fn schedule_panel(ui: &mut Ui, session: &Session, lang: Lang, state: &mut ScheduleUi) {
    ui.horizontal(|ui| {
        if ui.button(tr!(lang, ScheduleCompute)).clicked() {
            state.result = Some(session.schedule());
        }
        if let Some(schedule) = &state.result {
            ui.label(tr!(lang, ScheduleMakespan, format!("{:.1}", schedule.makespan())));
        }
    });
    let Some(schedule) = &state.result else {
        ui.weak(tr!(lang, ScheduleNotYet));
        return;
    };
    let doc = session.doc();
    // One row per robot of the cell, in the order they were added.
    let robots: Vec<(gc_core::Id<gc_doc::Robot>, String)> =
        doc.robots().iter().map(|(id, r)| (id, r.name.clone())).collect();
    let end = schedule.makespan().max(1e-6);
    let label_width = 110.0;
    let row_height = 26.0;
    let width = (ui.available_width() - label_width - 12.0).max(120.0);
    for (robot, name) in &robots {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.text(
            pos2(rect.left() + 2.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(12.0),
            colors::text(),
        );
        let track = Rect::from_min_size(
            pos2(rect.left() + label_width, rect.top() + 2.0),
            vec2(width, row_height - 4.0),
        );
        painter.rect_filled(track, 3.0, colors::muted().gamma_multiply(0.12));
        for assignment in schedule.assignments.iter().filter(|a| a.robot == *robot) {
            #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
            let (x0, x1) = (
                track.left() + (assignment.start / end) as f32 * track.width(),
                track.left() + (assignment.end / end) as f32 * track.width(),
            );
            let bar = Rect::from_min_max(pos2(x0, track.top()), pos2(x1.max(x0 + 2.0), track.bottom()));
            painter.rect_filled(bar, 3.0, colors::ACCENT.gamma_multiply(0.75));
            painter.rect_stroke(bar, 3.0, Stroke::new(1.0, colors::ACCENT), egui::StrokeKind::Inside);
            let task_name =
                doc.get(assignment.task).map_or_else(String::new, |t: &gc_doc::Task| t.name.clone());
            painter.text(
                bar.left_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                &task_name,
                FontId::proportional(11.0),
                egui::Color32::WHITE,
            );
            // The times, on the bar when the pointer is over it.
            let response =
                ui.interact(bar, egui::Id::unique("gc_schedule_bar").with(assignment.task), Sense::hover());
            response.on_hover_text(format!("{task_name}: {:.1} – {:.1} s", assignment.start, assignment.end));
        }
    }
    if !schedule.blocked.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(tr!(lang, ScheduleBlocked)).strong());
        for (task, why) in &schedule.blocked {
            let name = doc.get(*task).map_or_else(String::new, |t: &gc_doc::Task| t.name.clone());
            ui.horizontal(|ui| {
                ui.label(&name);
                ui.colored_label(colors::ERROR, why);
            });
        }
    }
}
