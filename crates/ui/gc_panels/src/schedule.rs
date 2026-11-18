//! The schedule panel (plan 9.4/9.5): a Gantt chart with one row per robot (a bar for each task it is given,
//! from `start` to `end`) on a time scale, the makespan, and the tasks that are never done with why. It is worked
//! out on a worker thread when asked (`Session::request_schedule`); while that runs the panel says so, and a
//! result for an older document is shown faded.

use egui::{Align2, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use gc_i18n::{Lang, tr};
use gc_session::Session;
use gc_ui::colors;

/// What the panel keeps between frames (the result itself lives in the session).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleUi {
    /// The moment shown plays on.
    pub playing: bool,
}

impl ScheduleUi {
    /// The panel is closed or the schedule is gone: the robots go back where they are.
    pub fn stop(&mut self, session: &mut Session) {
        self.playing = false;
        session.set_schedule_playback(None);
    }
}

/// A time step of 1, 2 or 5 times a power of ten that puts about six ticks along `end` seconds.
fn tick_step(end: f64) -> f64 {
    let wanted = end / 6.0;
    let decade = gc_math::fm::powf(10.0, wanted.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * decade).find(|s| *s >= wanted).unwrap_or(10.0 * decade)
}

/// The panel: a button to ask for the schedule, then the chart and the blocked tasks.
pub fn schedule_panel(ui: &mut Ui, session: &mut Session, lang: Lang, state: &mut ScheduleUi) {
    let pending = session.schedule_pending();
    ui.horizontal(|ui| {
        if ui.add_enabled(!pending, egui::Button::new(tr!(lang, ScheduleCompute))).clicked() {
            session.request_schedule();
        }
        if pending {
            ui.spinner();
            ui.label(tr!(lang, ScheduleWorking));
            ui.ctx().request_repaint();
        }
        if let Some((schedule, current)) = session.schedule_result() {
            ui.label(tr!(lang, ScheduleMakespan, format!("{:.1}", schedule.makespan())));
            if !current {
                ui.colored_label(colors::ERROR, tr!(lang, ScheduleStale));
            }
        }
    });
    let Some((schedule, current)) = session.schedule_result().map(|(s, c)| (s.clone(), c)) else {
        if !pending {
            ui.weak(tr!(lang, ScheduleNotYet));
        }
        return;
    };
    let fade = if current { 1.0 } else { 0.4 };
    // Playback: the robots are shown where the schedule has them at a moment (the document is not changed).
    let end_time = schedule.makespan();
    let mut now = session.schedule_playback().unwrap_or(0.0).clamp(0.0, end_time);
    ui.horizontal(|ui| {
        let label = if state.playing { tr!(lang, SchedulePause) } else { tr!(lang, SchedulePlay) };
        if ui.add_enabled(end_time > 0.0, egui::Button::new(label)).clicked() {
            state.playing = !state.playing;
            if state.playing && now >= end_time {
                now = 0.0;
            }
            session.set_schedule_playback(Some(now));
        }
        if ui.button(tr!(lang, ScheduleRewind)).clicked() {
            state.stop(session);
            now = 0.0;
        }
        let slider =
            ui.add(egui::Slider::new(&mut now, 0.0..=end_time.max(1e-6)).suffix(" s").fixed_decimals(1));
        if slider.changed() {
            session.set_schedule_playback(Some(now));
        }
    });
    if state.playing {
        let dt = f64::from(ui.input(|i| i.unstable_dt).clamp(0.0, 0.1));
        now = (now + dt).min(end_time);
        session.set_schedule_playback(Some(now));
        if now >= end_time {
            state.playing = false;
        }
        ui.ctx().request_repaint();
    }
    let doc = session.doc();
    // One row per robot of the cell, in the order they were added.
    let robots: Vec<(gc_core::Id<gc_doc::Robot>, String)> =
        doc.robots().iter().map(|(id, r)| (id, r.name.clone())).collect();
    let end = schedule.makespan().max(1e-6);
    // The time scale: ticks and seconds above the rows.
    {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
        let painter = ui.painter_at(rect);
        let step = tick_step(end);
        let track_left = rect.left() + 110.0;
        let track_width = (ui.available_width() - 110.0 - 12.0).max(120.0);
        let mut t = 0.0;
        while t <= end + 1e-9 {
            #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
            let x = track_left + (t / end) as f32 * track_width;
            painter.line_segment(
                [pos2(x, rect.bottom() - 4.0), pos2(x, rect.bottom())],
                Stroke::new(1.0, colors::muted()),
            );
            painter.text(
                pos2(x, rect.top()),
                Align2::CENTER_TOP,
                format!("{t:.0}"),
                FontId::proportional(10.0),
                colors::muted(),
            );
            t += step;
        }
    }
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
        // The moment shown, a vertical line through the row.
        if session.schedule_playback().is_some() {
            #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
            let x = track.left() + (now / end) as f32 * track.width();
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.bottom())],
                Stroke::new(1.5, colors::SELECTED),
            );
        }
        for assignment in schedule.assignments.iter().filter(|a| a.robot == *robot) {
            #[allow(clippy::cast_possible_truncation, reason = "screen coordinates fit f32")]
            let (x0, x1) = (
                track.left() + (assignment.start / end) as f32 * track.width(),
                track.left() + (assignment.end / end) as f32 * track.width(),
            );
            let bar = Rect::from_min_max(pos2(x0, track.top()), pos2(x1.max(x0 + 2.0), track.bottom()));
            painter.rect_filled(bar, 3.0, colors::ACCENT.gamma_multiply(0.75 * fade));
            painter.rect_stroke(
                bar,
                3.0,
                Stroke::new(1.0, colors::ACCENT.gamma_multiply(fade)),
                egui::StrokeKind::Inside,
            );
            let task_name =
                doc.get(assignment.task).map_or_else(String::new, |t: &gc_doc::Task| t.name.clone());
            painter.text(
                bar.left_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                &task_name,
                FontId::proportional(11.0),
                egui::Color32::WHITE.gamma_multiply(fade),
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
