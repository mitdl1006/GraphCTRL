//! The MCP server: tools an AI agent calls over Streamable HTTP on localhost. Each tool hands its call to
//! the application's thread through [`Bridge`] and waits for the answer, so the document is only ever
//! touched where the user's own edits happen.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, schemars, tool, tool_handler, tool_router};

use crate::exec::Call;

/// A call waiting for the application, with where its answer goes.
pub struct Request {
    /// What the agent asked for.
    pub call: Call,
    /// Where the answer (JSON, or an error message) goes.
    pub reply: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// The way from the server's threads to the application's: a queue, and a function that wakes the window.
#[derive(Clone)]
pub struct Bridge {
    sender: mpsc::Sender<Request>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Bridge {
    /// A bridge and the receiving end the application drains every frame.
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> (Self, mpsc::Receiver<Request>) {
        let (sender, receiver) = mpsc::channel();
        (Self { sender, wake }, receiver)
    }

    /// Hands a call to the application and waits for the answer. Planning can take a while, so the wait
    /// is generous.
    async fn call(&self, call: Call) -> CallToolResult {
        let (reply, answer) = tokio::sync::oneshot::channel();
        if self.sender.send(Request { call, reply }).is_err() {
            return CallToolResult::error(vec![ContentBlock::text("GraphCTRL is closing")]);
        }
        (self.wake)();
        match tokio::time::timeout(Duration::from_secs(300), answer).await {
            Ok(Ok(Ok(value))) => CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&value).unwrap_or_default(),
            )]),
            Ok(Ok(Err(message))) => CallToolResult::error(vec![ContentBlock::text(message)]),
            Ok(Err(_)) => CallToolResult::error(vec![ContentBlock::text("the call was dropped")]),
            Err(_) => CallToolResult::error(vec![ContentBlock::text("GraphCTRL did not answer in time")]),
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct NameArgs {
    #[schemars(description = "the name")]
    name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlaceArgs {
    #[schemars(description = "the robot's name, as overview lists it")]
    robot: String,
    #[schemars(description = "position of the robot's base in the world, millimetres [x, y, z]")]
    xyz_mm: [f64; 3],
    #[schemars(description = "roll, pitch, yaw of the base in degrees")]
    rpy_deg: [f64; 3],
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct WaypointArgs {
    #[schemars(description = "the robot the waypoint is for")]
    robot: String,
    #[schemars(
        description = "what the numbers are measured from: `world`, `base` (the robot's base, as on a UR teach pendant) or a work frame's name"
    )]
    reference: String,
    #[schemars(description = "tool centre point position in millimetres [x, y, z]")]
    xyz_mm: [f64; 3],
    #[schemars(
        description = "tool orientation as roll, pitch, yaw in degrees; [180, 0, 0] points the tool down in world"
    )]
    rpy_deg: [f64; 3],
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PathArgs {
    #[schemars(description = "waypoint names in the order the robot visits them")]
    waypoints: Vec<String>,
    #[schemars(
        description = "per waypoint, true to approach it in a straight line of the tool tip (linear move); missing entries are planned motions"
    )]
    #[serde(default)]
    linear: Vec<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PlanArgs {
    #[schemars(description = "the path's name")]
    path: String,
}

/// The tools of GraphCTRL.
#[derive(Clone)]
pub struct GraphCtrlTools {
    bridge: Bridge,
    tool_router: ToolRouter<Self>,
}

impl GraphCtrlTools {
    /// Tools that work through `bridge`.
    pub fn new(bridge: Bridge) -> Self {
        Self { bridge, tool_router: Self::tool_router() }
    }
}

#[tool_router]
impl GraphCtrlTools {
    #[tool(
        description = "The open robot cell: robots with their base pose and joints, bodies, work frames, waypoints with their world pose, and paths with their planning status. Positions in millimetres, angles in degrees. Call this first."
    )]
    async fn overview(&self) -> CallToolResult {
        self.bridge.call(Call::Overview).await
    }

    #[tool(description = "The robots in the robot library that add_robot accepts, with reach and payload.")]
    async fn library_robots(&self) -> CallToolResult {
        self.bridge.call(Call::LibraryRobots).await
    }

    #[tool(
        description = "Adds a robot from the robot library at the world origin. Returns its name in the cell."
    )]
    async fn add_robot(&self, Parameters(NameArgs { name }): Parameters<NameArgs>) -> CallToolResult {
        self.bridge.call(Call::AddRobot { name }).await
    }

    #[tool(description = "Places a robot's base in the world.")]
    async fn place_robot(&self, Parameters(args): Parameters<PlaceArgs>) -> CallToolResult {
        self.bridge
            .call(Call::PlaceRobot { robot: args.robot, xyz_mm: args.xyz_mm, rpy_deg: args.rpy_deg })
            .await
    }

    #[tool(
        description = "Adds a waypoint for a robot: a tool centre point pose measured from the world, the robot's base or a work frame. Returns its name and whether the robot can reach it."
    )]
    async fn add_waypoint(&self, Parameters(args): Parameters<WaypointArgs>) -> CallToolResult {
        self.bridge
            .call(Call::AddWaypoint {
                robot: args.robot,
                reference: args.reference,
                xyz_mm: args.xyz_mm,
                rpy_deg: args.rpy_deg,
            })
            .await
    }

    #[tool(
        description = "A waypoint's pose seen from the world, the robot's base and every work frame, whether the robot reaches it, its joint values there and any collision or limit problem."
    )]
    async fn waypoint_info(&self, Parameters(NameArgs { name }): Parameters<NameArgs>) -> CallToolResult {
        self.bridge.call(Call::WaypointInfo { name }).await
    }

    #[tool(description = "Makes a path through waypoints of one robot, in order.")]
    async fn create_path(&self, Parameters(args): Parameters<PathArgs>) -> CallToolResult {
        self.bridge.call(Call::CreatePath { waypoints: args.waypoints, linear: args.linear }).await
    }

    #[tool(
        description = "Plans a path: collision-free motions and their timing. Returns the status, duration, peak tool speed or why it failed. Planning may run in the background: when the status is `planning`, call overview a little later for the result."
    )]
    async fn plan_path(&self, Parameters(PlanArgs { path }): Parameters<PlanArgs>) -> CallToolResult {
        self.bridge.call(Call::PlanPath { path }).await
    }

    #[tool(description = "Undoes the last edit (the agent's or the user's).")]
    async fn undo(&self) -> CallToolResult {
        self.bridge.call(Call::Undo).await
    }

    #[tool(description = "Redoes the last undone edit.")]
    async fn redo(&self) -> CallToolResult {
        self.bridge.call(Call::Redo).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GraphCtrlTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "GraphCTRL is a robot-cell design and simulation application for Universal Robots arms. These tools \
             read and edit the cell the user has open; every edit appears in the user's window and can be undone. \
             Start with overview. Lengths are millimetres and angles degrees in every tool.",
        )
    }
}

/// A running server; dropping it stops the server.
pub struct Server {
    /// The address it listens on.
    pub address: std::net::SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        // The server may already have stopped on its own; then there is nobody to tell.
        if let Some(shutdown) = self.shutdown.take()
            && shutdown.send(()).is_err()
        {
            log::debug!("the agent server had already stopped");
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("the agent server thread panicked");
        }
    }
}

/// Starts the server on `127.0.0.1:port` (0 picks a free port) at `/mcp`. Requests must carry
/// `Authorization: Bearer <token>`.
pub fn start(port: u16, token: &str, bridge: Bridge) -> Result<Server, String> {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind(("127.0.0.1", port)))
        .map_err(|e| format!("127.0.0.1:{port}: {e}"))?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    let expected = format!("Bearer {token}");
    let (shutdown, stop) = tokio::sync::oneshot::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("gc-agent-mcp".to_owned())
        .spawn(move || {
            runtime.block_on(async move {
                let service: StreamableHttpService<GraphCtrlTools, LocalSessionManager> =
                    StreamableHttpService::new(
                        move || Ok(GraphCtrlTools::new(bridge.clone())),
                        Default::default(),
                        StreamableHttpServerConfig::default(),
                    );
                let router =
                    axum::Router::new().nest_service("/mcp", service).layer(axum::middleware::from_fn(
                        move |request: axum::extract::Request, next: axum::middleware::Next| {
                            let expected = expected.clone();
                            async move {
                                let authorised = request
                                    .headers()
                                    .get(axum::http::header::AUTHORIZATION)
                                    .and_then(|v| v.to_str().ok())
                                    .is_some_and(|v| v == expected);
                                if authorised {
                                    next.run(request).await
                                } else {
                                    axum::response::IntoResponse::into_response(
                                        axum::http::StatusCode::UNAUTHORIZED,
                                    )
                                }
                            }
                        },
                    ));
                let served = axum::serve(listener, router).with_graceful_shutdown(async move {
                    drop(stop.await);
                });
                if let Err(error) = served.await {
                    log::error!("the agent server stopped: {error}");
                }
            });
        })
        .map_err(|e| e.to_string())?;
    Ok(Server { address, shutdown: Some(shutdown), thread: Some(thread) })
}

/// A new random access token.
pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
