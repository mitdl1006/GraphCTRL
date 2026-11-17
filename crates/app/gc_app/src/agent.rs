//! The AI agent connection (PLAN 9.8): runs the MCP server of `gc_agent` when the user allows it, carries
//! out the agents' calls between frames and shows how to connect an agent.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use gc_i18n::{Lang, tr};
use gc_session::Session;
use gc_session::library::Package;

/// The port used when the settings name none.
pub(crate) const DEFAULT_PORT: u16 = 47615;
/// How many entries the activity log keeps.
const LOG_LENGTH: usize = 50;

/// The agent server and its window.
pub(crate) struct Agent {
    /// Whether the user allows agent connections.
    pub enabled: bool,
    /// The port the server listens on.
    pub port: u16,
    /// The token agents must send.
    pub token: String,
    /// Whether the window is open.
    pub open: bool,
    running: Option<(gc_agent::Server, Receiver<gc_agent::Request>)>,
    /// The settings the running server was started with, to restart it when they change.
    started_with: Option<(u16, String)>,
    error: Option<String>,
    log: VecDeque<String>,
}

impl Agent {
    /// The agent connection as the saved settings describe it; a token is made when there is none.
    pub(crate) fn new(settings: &gc_persist::places::Settings) -> Self {
        let token = if settings.agent_token.is_empty() {
            gc_agent::new_token()
        } else {
            settings.agent_token.clone()
        };
        Self {
            enabled: settings.agent_enabled,
            port: if settings.agent_port == 0 { DEFAULT_PORT } else { settings.agent_port },
            token,
            open: false,
            running: None,
            started_with: None,
            error: None,
            log: VecDeque::new(),
        }
    }

    /// The address agents connect to.
    pub(crate) fn url(&self) -> String {
        let port = self.running.as_ref().map_or(self.port, |(server, _)| server.address.port());
        format!("http://127.0.0.1:{port}/mcp")
    }

    /// Starts or stops the server to match the settings.
    fn keep_in_step(&mut self, ctx: &egui::Context) {
        let wanted = self.enabled.then(|| (self.port, self.token.clone()));
        if wanted == self.started_with {
            return;
        }
        // Stopping first frees the port when only the token changed.
        self.running = None;
        self.started_with = wanted.clone();
        let Some((port, token)) = wanted else {
            self.error = None;
            return;
        };
        let repaint = ctx.clone();
        let (bridge, requests) = gc_agent::Bridge::new(Arc::new(move || repaint.request_repaint()));
        match gc_agent::start(port, &token, bridge) {
            Ok(server) => {
                log::info!("the agent server listens on {}", server.address);
                self.error = None;
                self.running = Some((server, requests));
            }
            Err(error) => {
                log::warn!("the agent server could not start: {error}");
                self.error = Some(error);
            }
        }
    }

    /// Runs the server as the settings say and carries out the calls that came in. Returns a summary of
    /// the last call that edited the cell, for the status bar.
    pub(crate) fn serve(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session,
        packages: &[Package],
    ) -> Option<String> {
        self.keep_in_step(ctx);
        let (_, requests) = self.running.as_ref()?;
        let mut edited = None;
        while let Ok(request) = requests.try_recv() {
            let result = gc_agent::execute(session, packages, &request.call);
            let summary = request.call.summary();
            let entry = match &result {
                Ok(_) => summary.clone(),
                Err(error) => format!("{summary} — {error}"),
            };
            if request.call.edits() && result.is_ok() {
                edited = Some(summary);
            }
            self.log.push_front(entry);
            self.log.truncate(LOG_LENGTH);
            // The agent may have given up waiting; nothing to do then.
            drop(request.reply.send(result));
        }
        edited
    }

    /// The connection window. Returns true when a setting changed and should be saved.
    pub(crate) fn window(&mut self, ctx: &egui::Context, lang: Lang) -> bool {
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new(tr!(lang, MenuAgent)).open(&mut open).default_width(520.0).show(ctx, |ui| {
            ui.label(tr!(lang, AgentExplain));
            ui.add_space(4.0);
            changed |= ui.checkbox(&mut self.enabled, tr!(lang, AgentEnable)).changed();
            if let Some(error) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, tr!(lang, AgentFailed, error));
            }
            ui.horizontal(|ui| {
                let label = ui.label(tr!(lang, AgentPort));
                changed |= ui
                    .add(egui::DragValue::new(&mut self.port).range(1024..=65535))
                    .labelled_by(label.id)
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label(tr!(lang, AgentToken));
                ui.monospace(&self.token);
                if ui.button(tr!(lang, AgentNewToken)).clicked() {
                    self.token = gc_agent::new_token();
                    changed = true;
                }
            });
            ui.label(tr!(lang, AgentAddress, self.url()));
            ui.separator();
            ui.label(egui::RichText::new(tr!(lang, AgentClaudeCode)).strong());
            let command = format!(
                "claude mcp add --transport http graphctrl {} --header \"Authorization: Bearer {}\"",
                self.url(),
                self.token
            );
            ui.horizontal_wrapped(|ui| {
                ui.monospace(&command);
                if ui.button(tr!(lang, AgentCopy)).clicked() {
                    ui.ctx().copy_text(command.clone());
                }
            });
            ui.separator();
            ui.label(egui::RichText::new(tr!(lang, AgentActivity)).strong());
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                for entry in &self.log {
                    ui.label(egui::RichText::new(entry).monospace().size(11.0));
                }
            });
        });
        self.open = open;
        changed
    }
}
