# GraphCTRL extension architecture: research report (2026-11-16)

**Evidence tags.** [V] verified today from a primary source (crates.io API, a file inside the published crate, raw.githubusercontent.com, or the local toolchain). [S] web-search evidence only. [R] recalled from memory.

WebFetch could not reach `bytecodealliance.org` or `robodk.com`; both are outside this session's allowed network domains and would have to be allowed in the network settings. WASI-ratification and RoboDK details are therefore [S].

Repository [V]: `/home/user/GraphCTRL` holds only `README.md`, `LICENSE` (MIT) and `.gitignore`, so nothing constrains the design.

## 1. Mechanisms, status November 2026

| Mechanism | Status | Verdict |
|---|---|---|
| (a) Compile-time | Bevy's contract is `trait Plugin: Downcast + Any + Send + Sync { build(&self, &mut App); ready; finish; cleanup; name; is_unique }` [V `bevy_app-0.20.1/src/plugin.rs`]. `inventory` 0.3.25 and `linkme` 0.3.38 are current [V]. inventory gives "no guarantee about the order" and relies on constructors [V its `lib.rs`]. linkme lists Linux/macOS/Windows/BSD/illumos, not wasm [V README]. A crate must still be referenced from the binary to be linked [R]. | **Use now**: trait objects plus one explicit list. Skip inventory/linkme: unordered, hidden global state, and no saving because the Cargo dependency is needed anyway. One cargo feature per optional built-in. |
| (b) Native dylibs | `libloading` 0.9.1 (2026-11-13) [V]. `abi_stable` 0.11.3 was last released 2023-10-12 [V]. `stabby` 72.1.16 (2026-07) warns in its README that on stable Rust ≥ 1.78 its trait-object vtables live in a leaked global set with linear lookup [V]. rustc master has only `export_stable` (`incomplete`, since 1.88, issue 139939) and no `crabi` gate [V `rustc_feature/src/unstable.rs`]. No crABI goal was found for 2026 [S]. `bevy_dynamic_plugin` ended at 0.14.2 [V]. | **Never as the plugin mechanism.** Pitfalls [R]: identical rustc, flags and dependency versions required; `TypeId`, statics and TLS duplicated per library; unloading unsound; panics leaving `extern "C"` abort (since 1.81); no sandbox; per-OS signing. Use C ABI only inside out-of-process drivers that wrap vendor SDKs. |
| (c) WASM components | `wasmtime` 50.0.1 (MSRV 1.97.0) has `component-model-async` as a default feature, and `wasmtime-wasi` defaults to `p1,p2,p3` [V both `Cargo.toml`]. WASI 0.3.0 was ratified 2026-06-11 [S]; `wasip3` is at 0.10.0+wasi-0.3.2 [V]. Releases are a monthly semver-major, with every 12th an LTS supported for 24 months [V `docs/stability-release.md`]; the current LTS is 48.0.8 (MSRV 1.95) [V]. Tier 1 is x86_64 Linux/macOS/Windows; **aarch64-apple-darwin and aarch64-linux are Tier 2, aarch64-windows Tier 3** [V `docs/stability-tiers.md`]. 50.0.1 fixed five security advisories [V RELEASES.md]. rustc 1.97 lists `wasm32-wasip2` and `wasm32-wasip3` [V local]. `wit-bindgen` is 0.63.0 [V]. `cargo-component` 0.21.1 was last released 2025-03 [V]; plain `cargo build --target wasm32-wasip2` replaces it [V wit-bindgen README]. Threads in components are still planned for a 0.3.x release [S]. Interruption is by fuel (deterministic, slower) or epochs [V `docs/examples-interrupting-wasm.md`]. Alternatives: `wasmi` 2.0.0 is an interpreter with wasip1 only [V]; `extism` runs on wasmtime 49 with only `p1` [V]; `wasmer` is 7.5.0 [V]. | **Later, for sandboxed third-party compute.** No authoritative call-cost figure exists: anecdotes range from about 10 ns (Rust API) to 2 µs (C API) per call [S], and lists/strings are copied via guest `realloc` [R]. Design coarse, batched calls and measure in a spike. Guests are 32-bit (4 GiB) and single-threaded [R]. |
| (d) Out-of-process | Versions [V]: `tonic` 0.14.9, `capnp-rpc` 0.28.0, `tarpc` 0.38.0, `jsonrpsee` 0.26.1, `interprocess` 2.4.8, `rmp-serde` 1.3.1, `iceoryx2` 0.10.0, `zenoh` 1.10.2. | **Later, for controller drivers** (RTDE, URSim, vendor SDKs): crash, language and licence isolation. Start with framed MessagePack/JSON over stdio plus a Hello handshake; add shared memory only if profiling demands it. |
| (e) Scripting | Versions [V]: `rhai` 1.27.0, `mlua` 0.12.2, `rune` 0.14.2, `steel-core` 0.8.3, `pyo3` 0.30.1, `rustpython` 0.6.0, `koto` 0.16.1. | **User automation, not plugins.** Defer the engine choice; build the command layer now so any engine only calls commands. |
| (f) Data packages | `urdf-rs` 0.10.0, `openusd` 0.8.0, `sdformat` 0.1.0, `mujoco-rs` 7.1.0 (FFI, non-standard licence); no `mjcf` crate [V]. UR's own description is per-model YAML: `default_kinematics.yaml` (six frames plus a calibration `hash`), `joint_limits.yaml`, `physical_parameters.yaml` [V UniversalRobots/Universal_Robots_ROS2_Description]. | **Primary mechanism for robots**: data plus a reference to a kinematics solver family. |

## 2. How shipping applications do it

| Application | Mechanism | Lesson |
|---|---|---|
| Zed | WASM components. Host pins `wasmtime 48.0.7` with `wasmtime-wasi` feature `p2` only [V Zed `Cargo.toml`]. `extension.toml` carries `schema_version = 1`, id, name, version [V `zed_extension_api-0.7.0/README.md`]. | The host keeps bindings for every WIT generation: modules `since_v0_0_1` … `since_v0_8_0` and a ten-variant `enum Extension`. The newest generation is enabled only on Dev/Nightly, and breaking changes are batched in `PENDING_CHANGES.md` [V `extension_host/src/wasm_host/wit.rs`]. Host objects are WIT `resource`s passed as `borrow<>` [V `extension.wit`]. Extensions draw no UI [R]. |
| VS Code [R] | Separate extension-host process. `package.json` has `contributes.*`, `activationEvents`, `engines.vscode`. | Declarative contributions render before code loads; activation is lazy; the host version range lives in the manifest. |
| Nushell | Child process; stdio mandatory, local socket optional; both sides send `Hello {version, features}` and ignore unknown features [V protocol reference]. | Copy the feature negotiation. Compatibility is a caret match on the Nu version, so every 0.x release invalidates all plugins [V]; version the protocol, not the app. |
| Bevy | Compile-time plugins; the engine is assembled from its own [V]. | Dogfooding works. `&mut App` gives plugins everything; hand out a narrow registrar instead. |
| Tauri | Compile-time crates: `trait Plugin<R>: Send { name, initialize, window_created, on_navigation… }` [V `tauri-2.12.6/src/plugin.rs`]; permission files [R]. | Deny-by-default capabilities, even for compiled-in code. |
| Rerun (egui + wgpu) | No runtime plugins. You build your own viewer binary and register view classes, visualizers and panels through the Rust API [S]. | A custom build is a legitimate tier-0 extension story for an egui app. |
| Zellij | WASM on the **wasmi 1.1 interpreter** [V `zellij-server/Cargo.toml`]; `ZellijPlugin { load, update, pipe, render }` [V `zellij-tile-0.46.0`]. | An interpreter is enough for event-rate plugins. |
| Lapce | `volt.toml` with `[activation]` and `[config."key"]` [V]. Host still pins `wasmtime 14.0.0` plus a git dependency; `lapce-plugin` was last released 2023-10 [V]. | A plugin runtime is a maintenance commitment. |
| Helix | Steel plugin system: `master` has no `steel` dependency [V]; PR 8675 was still a draft in March 2026 [S]. | Scripting as a plugin system takes years. |
| Blender [R] | In-process Python; `blender_manifest.toml` with id, `blender_version_min`, SPDX licence, `[permissions]` with reasons. | Operators (`poll`/`execute`/`modal`) are the command abstraction. |
| SolidWorks / Inventor [S] | In-process COM/.NET. `ISwAddin.ConnectToSW/DisconnectFromSW`. Inventor uses `.addin` XML (ClassId, LoadBehavior) and `ApplicationAddInServer.Activate/Deactivate`. | The add-in id owns the ribbon objects it created; LoadBehavior is an activation event. |
| RoboDK [S] | Robot library files (data), post-processors (one Python file each), drivers (live link), Python Apps (`.rdkp`), and a C++/Qt plug-in interface that must match the RoboDK version. | This tiering is the right model. |
| UR PolyScope X [S] | URCapX: `manifest.yaml` (vendorID, urcapID, artifacts), Docker backends with declared ports, web frontends contributing program nodes, application nodes, sidebar items and operator screens. | The target vendor uses out-of-process plugins with declared capabilities. |
| ROS pluginlib / MoveIt | XML `<library path><class name type base_class_type="kinematics::KinematicsBase">` [V]; selected by string in YAML [R]. | Taxonomy [R]: planner, kinematics, collision, request/response adapters, controller manager. Replace strings with typed keys. |
| Omniverse Kit [S] | `config/extension.toml`: `[package]`, `[dependencies]` keyed by id, `[[python.module]]`. | Dependencies keyed by plugin id. |

Not checked today: Gazebo and Foxglove.

Manifest fields worth copying:
- `schema_version`
- reverse-DNS id
- host API range
- dependencies keyed by id
- activation hints
- config schema
- permissions with reasons, and declared ports
- SPDX licence

## 3. Extension points for GraphCTRL

Entries in bold are implemented in milestone 1.

| Point | Milestone 1 | Later | Note |
|---|---|---|---|
| Robot model packages | Schema only | Data package | A model is not an instance: allow a calibration overlay. |
| Kinematics | Trait in the unstable robotics crate | Static Rust, then WASM | Solver families (UR analytic 6R, OPW via `rs-opw-kinematics`, numeric) are chosen by id from model data, so most robots need no code. |
| Planners / trajectory generation | Sketch only | Static, then WASM, then process | Batched scene queries; separate adapters for time parameterisation. |
| Collision backend | Internal trait | Static only | Hot path sharing scene memory. |
| Program exporters (URScript) | None | Static, then WASM or script | Pure function from program IR to text. |
| Live controller backends | None | Static, then process driver | Sockets and cyclic I/O. |
| Importers / exporters | **Static, dogfooded** | WASM | Bytes in, plain mesh buffers out. |
| Control-logic paradigms | None | Node types as data; executors static, then WASM | Graphs must render without plugin code. |
| Sketch constraint / dimension kinds | **Static**, in a point owned by the sketch crate | Stay static | Solver inner loop. |
| Commands, ribbon, keybindings | **Declarative specs + handlers** | Same for every plugin kind | |
| Panels, property editors, viewport tools | **Static**, via an egui-coupled crate | Static only; sandboxed plugins get schema-driven panels and overlay primitives | Never pass `egui::Ui` or wgpu across a boundary. |
| Settings pages, i18n | **Schema-driven; Fluent files per plugin id** | Same | `fluent` 0.17.0 [V]. |

## 4. API design rules

- **Crates** (proposed, none exist yet):
  - `/home/user/GraphCTRL/crates/graphctrl-plugin-api`: public dependencies are `serde` and `semver` only.
  - `/home/user/GraphCTRL/crates/graphctrl-plugin-api-egui`: major version tracks egui.
  - `/home/user/GraphCTRL/crates/graphctrl-robotics-api`: stays 0.0.x until two implementations exist.
  - `/home/user/GraphCTRL/crates/graphctrl-plugin-host`, `/home/user/GraphCTRL/crates/graphctrl-plugin-testkit`, `/home/user/GraphCTRL/plugins/*` and `/home/user/GraphCTRL/app`.
- **No third-party types in signatures.** That covers egui, wgpu, glam/nalgebra, parry and anyhow; their breaking releases would become ours. Use own POD math types with optional `From` features.
- **Evolution.** All data types are `#[non_exhaustive]` with constructors. Host-implemented service traits are sealed, so adding methods is non-breaking. Plugin-implemented traits change only through defaulted methods or a new versioned interface kept beside the old one behind an adapter (Zed's `since_vX` pattern). Negotiate at three levels: manifest `schema_version`, the `api` range against `HOST_API`, and per-interface `name@major.minor`.
- **Typed keys, no globals.** `ExtensionPoint<T>` and `Key<T>` give typed lookup; project files store string ids resolved through the same point. The registry is built once, frozen and shared as `Arc<Registry>`. Unknown ids load as data-preserving placeholders.
- **Plugins compute, the host mutates.** Calls take data and return data or a `Transaction`; the host validates, applies and records undo. This is what makes fault isolation meaningful, and it maps directly onto WASM and process boundaries.
- **Error boundary.** Every call returns `Result` and is wrapped in `catch_unwind`; a faulted plugin's contributions are disabled. Limits [R]: nothing is caught under `panic = "abort"`, or on stack overflow, allocation failure, deadlock, infinite loop or UB. Keep `panic = "unwind"` in release; untrusted code needs wasmtime epochs and memory limits, or a killable process.
- **Threading.** Providers are `Send + Sync` with `&self`. Per-thread workers such as `KinematicsSolver` are `Send` with `&mut self`, which later maps to one wasmtime `Store` each. Long calls are synchronous with `JobControl`; no `async fn` in plugin traits.
- **Determinism.** Requests carry an explicit `seed`; plugins use no clocks or global RNG; iterated collections are ordered maps (`indexmap` 2.14.2 [V]); results record plugin id, version and parameters.
- **Testing and CI.** The testkit holds conformance suites that every plugin runs: FK/IK round trip, limits, determinism, cancel latency, importer fuzzing. CI runs `cargo-semver-checks` 0.51.0 [V] on the API crates and builds an in-tree example plugin pinned to the oldest supported API. A unit test parses a mirrored `wit/` directory with `wit-parser` 0.263.0 [V], keeping contracts WIT-expressible without depending on wasmtime.
- **Distribution.** Search paths are install, user and project. Each package has a `plugin.toml`; each project has a lock of id, version and hash. Signatures come later (`minisign-verify` 0.3.0 [V]); capabilities are shown at install.
- **Licence.** The repository is MIT [V]. Publish the API crates as MIT OR Apache-2.0. Static plugins must be licence-compatible with the binary; copyleft or proprietary code belongs behind a process or WASM boundary.

## 5. Roadmap and sketches

1. **Milestone 1.** Build the API crates, the host and the explicit static list. Write the built-ins (`core-ui`, `sketch`, `viewport`, `import-mesh`) as plugins against that API. Add the command layer, a plugin-manager panel showing versions and faults, manifest schema v1 and the CI checks.
2. **First robots.** Add data packages and a built-in `ur-robots`. Exercise the robotics API with two solver families before calling it 0.1.
3. **WASM.** Add components behind a `wasm-plugins` feature on the wasmtime LTS line, with adapters implementing the same Rust traits.
4. **Process drivers and scripting.**

Rules that keep the WASM door open at no cost now:
- Boundaries carry serde-able data only: no closures, generics or borrowed host internals.
- Calls are batched and synchronous with cancellation.
- Host objects are referenced by handle.
- UI traits live in a separate crate.
- Manifests carry interface names and versions from day one.

```rust
// graphctrl-plugin-api
pub const HOST_API: semver::Version = semver::Version::new(0, 1, 0);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PluginId(Arc<str>);            // "graphctrl.ur-robots", validated on construction
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContributionId(Arc<str>);      // "<plugin-id>/<local-id>"

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PluginManifest {               // == plugin.toml, same schema for every plugin kind
    pub schema_version: u32,
    pub id: PluginId,
    pub name: String,
    pub version: semver::Version,
    pub api: semver::VersionReq,          // host API range, e.g. "^0.1"
    pub kind: PluginKind,                 // Builtin | Data | Wasm | Process
    #[serde(default)] pub requires: Vec<Dependency>,
    #[serde(default)] pub provides: Vec<Provided>,       // interface@version + ids: lets the UI list things lazily
    #[serde(default)] pub capabilities: Capabilities,    // deny by default
    #[serde(default)] pub license: Option<String>,       // SPDX
}

pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> &PluginManifest;
    /// Once, single-threaded, before the registry freezes. No I/O, no threads.
    fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError>;
    /// Lazy heavy initialisation, off the UI thread, on first use of a contribution.
    fn activate(&self, _host: &dyn HostServices) -> Result<(), PluginError> { Ok(()) }
    fn deactivate(&self) {}
}

/// Typed extension point: `T` is the object-safe contract, the name is what manifests and WIT use.
pub struct ExtensionPoint<T: ?Sized + 'static> {
    pub interface: &'static str,
    pub version: (u16, u16),
    _contract: PhantomData<fn() -> Arc<T>>,
}
impl<T: ?Sized + 'static> ExtensionPoint<T> {
    pub const fn new(interface: &'static str, version: (u16, u16)) -> Self {
        Self { interface, version, _contract: PhantomData }
    }
}
pub struct Key<T: ?Sized + 'static> { id: ContributionId, _contract: PhantomData<fn() -> Arc<T>> }

pub struct Registrar<'a> { backend: &'a mut dyn RegistrarBackend }   // sealed; implemented by the host crate
impl Registrar<'_> {
    pub fn provide<T: ?Sized + Send + Sync + 'static>(
        &mut self, point: &ExtensionPoint<T>, local_id: &str, item: Arc<T>,
    ) -> Result<Key<T>, PluginError> {
        let id = self.backend.insert(point.interface, point.version, local_id, Box::new(item))?;
        Ok(Key { id, _contract: PhantomData })
    }
}

// graphctrl-plugin-host: immutable after build, shared as Arc<Registry>
impl Registry {
    pub fn get<T: ?Sized + 'static>(&self, key: &Key<T>) -> Option<&Arc<T>>;
    pub fn find<T: ?Sized + 'static>(&self, p: &ExtensionPoint<T>, id: &ContributionId) -> Option<&Arc<T>>;
    pub fn all<T: ?Sized + 'static>(&self, p: &ExtensionPoint<T>)
        -> impl Iterator<Item = (&ContributionId, &Arc<T>)>;          // deterministic order
}
```

```rust
// graphctrl-robotics-api (0.0.x until proven by two implementations)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[repr(C)]
pub struct Pose { pub t: [f64; 3], pub q: [f64; 4] }

pub static ROBOT_MODELS: ExtensionPoint<dyn RobotModelProvider> = ExtensionPoint::new("graphctrl:robot-model", (0, 1));
pub static KINEMATICS:   ExtensionPoint<dyn KinematicsFactory>  = ExtensionPoint::new("graphctrl:kinematics", (0, 1));
pub static PLANNERS:     ExtensionPoint<dyn MotionPlanner>      = ExtensionPoint::new("graphctrl:planner", (0, 1));

pub trait RobotModelProvider: Send + Sync {
    fn catalog(&self) -> Vec<RobotModelInfo>;          // id, vendor, dof, reach, payload: no assets loaded
    fn load(&self, model: &str, assets: &dyn AssetSource) -> Result<RobotModel, ModelError>;
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RobotModel {                                // pure data: what a data package deserialises into
    pub links: Vec<Link>, pub joints: Vec<Joint>,      // tree + position/velocity/acceleration/effort limits
    pub flange: FrameId, pub default_tcp: Pose, pub mounting: Pose, pub payload: Payload,
    pub kinematics: SolverRef,                         // { solver: ContributionId, params }
    pub calibration: Option<Calibration>,              // per-serial-number deltas
}

pub trait KinematicsFactory: Send + Sync {
    fn caps(&self) -> KinematicsCaps;                  // analytic, all_solutions, max_dof
    fn create(&self, model: &RobotModel) -> Result<Box<dyn KinematicsSolver>, KinError>;
}
/// One instance per worker thread (later: one wasmtime Store).
pub trait KinematicsSolver: Send {
    fn dof(&self) -> usize;
    /// Batched: `q.len() == n * dof`; appends n flange poses.
    fn fk(&mut self, q: &[f64], out: &mut Vec<Pose>) -> Result<(), KinError>;
    /// Appends solutions as rows of `dof` values: all branches (analytic) or nearest to `seed` (numeric).
    fn ik(&mut self, target: &Pose, seed: Option<&[f64]>, opts: &IkOptions, out: &mut Vec<f64>)
        -> Result<IkReport, KinError>;
}

pub trait MotionPlanner: Send + Sync {
    fn descriptor(&self) -> &PlannerDescriptor;        // parameter schema, capabilities
    /// `req.seed` is the only randomness source.
    fn plan(&self, req: &PlanRequest, scene: &dyn PlanningScene, job: &dyn JobControl)
        -> Result<Trajectory, PlanError>;
}
/// Host service (sealed). Batched: one boundary crossing per n states.
pub trait PlanningScene: Send + Sync {
    fn check_states(&self, q: &[f64], valid: &mut Vec<bool>);
    fn check_motions(&self, from: &[f64], to: &[f64], valid: &mut Vec<bool>);
}
pub trait JobControl: Send + Sync { fn is_cancelled(&self) -> bool; fn progress(&self, fraction: f32, msg: &str); }

// graphctrl-plugin-api
pub trait Importer: Send + Sync {
    fn formats(&self) -> &[FileFormat];
    fn import(&self, bytes: &[u8], opts: &ImportOptions, job: &dyn JobControl)
        -> Result<ImportedScene, ImportError>;         // plain buffers + node tree + units; host uploads to GPU
}
```

```rust
// graphctrl-plugin-api: declarative, valid for every plugin kind
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandSpec {
    pub id: String, pub title: Text, pub category: String, pub icon: Option<String>,
    pub keys: Option<String>, pub when: Option<String>, pub args: Vec<ArgSpec>, pub undoable: bool,
}
pub struct RibbonItem { pub tab: String, pub group: String, pub command: String, pub order: i32 }
pub struct PanelSpec { pub id: String, pub title: Text, pub dock: DockHint }
pub struct SettingsPage { pub id: String, pub title: Text, pub fields: Vec<FieldSpec> }

pub trait CommandHandler: Send + Sync {
    fn spec(&self) -> &CommandSpec;
    fn is_enabled(&self, _cx: &dyn CommandContext) -> bool { true }
    fn run(&self, cx: &dyn CommandContext, args: &Args) -> Result<Transaction, CommandError>;
}

// graphctrl-plugin-api-egui: in-process plugins only
pub trait PanelView: Send { fn ui(&mut self, ui: &mut egui::Ui, cx: &mut PanelContext<'_>); }
pub trait ViewportTool: Send {
    fn on_event(&mut self, ev: &ToolEvent, cx: &mut ToolContext<'_>) -> ToolFlow; // Continue | Commit(Transaction) | Cancel
    fn overlay(&self, out: &mut OverlayList);                                     // primitives as data, not wgpu calls
}
```

```rust
// plugins/ur-robots/src/lib.rs
pub fn plugin() -> Result<Box<dyn Plugin>, PluginError> {
    let manifest = PluginManifest::from_toml(include_str!("../plugin.toml"))?;
    Ok(Box::new(UrRobots { manifest }))
}
struct UrRobots { manifest: PluginManifest }
impl Plugin for UrRobots {
    fn manifest(&self) -> &PluginManifest { &self.manifest }
    fn register(&self, reg: &mut Registrar<'_>) -> Result<(), PluginError> {
        reg.provide(&ROBOT_MODELS, "catalog", Arc::new(UrCatalog::embedded()))?;  // TOML + URDF + meshes
        reg.provide(&KINEMATICS, "analytic-6r", Arc::new(UrAnalyticIk))?;
        reg.provide(&PROGRAM_EXPORTERS, "urscript", Arc::new(UrScriptExporter))?;
        Ok(())
    }
}

// app/src/plugins.rs: the only module that names concrete plugin crates
type Ctor = fn() -> Result<Box<dyn Plugin>, PluginError>;
const BUILTIN: &[Ctor] = &[
    graphctrl_core_ui::plugin,
    graphctrl_sketch::plugin,
    graphctrl_viewport::plugin,
    graphctrl_import_mesh::plugin,
    #[cfg(feature = "ur-robots")]
    graphctrl_ur_robots::plugin,
];

pub fn load_plugins(cfg: &PluginConfig) -> (Arc<Registry>, Vec<PluginReport>) {
    let mut host = PluginHost::new(HOST_API);
    host.add_source(StaticPlugins(BUILTIN));
    host.add_source(DataPackages::scan(&cfg.search_paths));       // robots phase
    #[cfg(feature = "wasm-plugins")]
    host.add_source(WasmComponents::scan(&cfg.search_paths));     // adapters implement the same traits
    host.finish() // api check, dependency order, register() under catch_unwind, freeze, per-plugin report
}
```

```toml
# plugins/ur-robots/plugin.toml
schema_version = 1
id = "graphctrl.ur-robots"
name = "Universal Robots"
version = "0.1.0"
api = "^0.1"
kind = "builtin"
license = "MIT OR Apache-2.0"

[[provides]]
interface = "graphctrl:robot-model@0.1"
ids = ["ur3e", "ur5e", "ur10e", "ur16e", "ur20", "ur30"]

[[provides]]
interface = "graphctrl:kinematics@0.1"
ids = ["analytic-6r"]

[capabilities]
net = ["tcp:*:30004", "tcp:*:29999"]   # RTDE, dashboard [R]
```

## Sources

- crates.io API and `static.crates.io` crate archives for every version and file tagged [V].
- [wasmtime release notes, tiers, release process, interruption docs (release-50.0.0)](https://raw.githubusercontent.com/bytecodealliance/wasmtime/release-50.0.0/RELEASES.md)
- [Zed `wit.rs`](https://raw.githubusercontent.com/zed-industries/zed/main/crates/extension_host/src/wasm_host/wit.rs)
- [Nushell plugin protocol](https://raw.githubusercontent.com/nushell/nushell.github.io/main/contributor-book/plugin_protocol_reference.md)
- [rustc feature gates](https://raw.githubusercontent.com/rust-lang/rust/master/compiler/rustc_feature/src/unstable.rs)
- [UR description config](https://raw.githubusercontent.com/UniversalRobots/Universal_Robots_ROS2_Description/rolling/config/ur5e/default_kinematics.yaml)
- [MoveIt KDL plugin XML](https://raw.githubusercontent.com/moveit/moveit2/main/moveit_kinematics/kdl_kinematics_plugin_description.xml)
- [WASI releases](https://wasi.dev/releases) and [WASI 0.3 announcement](https://bytecodealliance.org/articles/WASI-0.3)
- [Rust safe-linking goal](https://rust-lang.github.io/rust-project-goals/2025h1/safe-linking.html) and [wasm components goal](https://rust-lang.github.io/rust-project-goals/2026/wasm-components.html)
- [Helix PR 8675](https://github.com/helix-editor/helix/pull/8675)
- [Rerun: extend the UI](https://rerun.io/docs/howto/extend/extend-ui)
- [RoboDK add-ins](https://robodk.com/doc/en/Add-ins.html), [plug-in interface](https://robodk.com/doc/en/PlugIns/index.html), [post-processors](https://robodk.com/doc/en/Post-Processors.html)
- [URCapX manifest](https://docs.universal-robots.com/PolyScopeX_SDK_Documentation/build/SDK-v0.19/Resources/manifest.html)
- [Inventor add-ins](https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-API/files/CreatingAnAddIn_Overview.htm) and [SolidWorks ISwAddin](https://help.solidworks.com/2021/english/api/sldworksapiprogguide/OVERVIEW/Using_SwAddin_to_Create_a_SOLIDWORKS_Addin.htm)
- [Omniverse extension TOML](https://docs.omniverse.nvidia.com/dev-guide/latest/dev_guide/develop/develop_configure_toml_files.html)
- [wasmtime import-call overhead thread](https://bytecodealliance.github.io/zulip-archive/stream/206238-general/topic/High.20Overhead.20for.20Wasm.20Import.20Call.20Despite.20AOT.20Compilation.html)

## Recommendation

Build milestone 1 on compile-time plugins only: a small `graphctrl-plugin-api` crate, one explicit constructor list in the app, and every built-in feature registered through that same API. Do not add wasmtime, libloading, inventory or a scripting engine yet.

- **Registrar, not app access.** Plugins get a narrow, typed `Registrar` during `register`. They never get the document or `egui::Context`.
- **Two API crates.** The core crate is data-only and depends on `serde` and `semver`. The egui-typed traits live in a second crate whose major version follows egui.
- **Commands first.** Every user action is a `CommandHandler` returning a `Transaction`. That gives undo, fault isolation, macro recording and later scripting from one mechanism.
- **Robots are data.** The UR plugin is a data package plus two small code contributions: analytic IK and the URScript exporter. Other vendors with spherical wrists should need no code.
- **Robotics API stays unstable** until the UR and OPW solver families both run against it.
- **When WASM arrives**, use the wasmtime LTS line with `default-features = false` and WASI p2 worlds, as Zed does. Limit it to compute plugins: kinematics, planners, exporters, importers.
- **Live controller links** go out of process.
- **Native Rust dylib plugins: never.**

## Risks and unknowns

- **Unmeasured WASM call cost.** No authoritative numbers were found. Per-state collision callbacks from a guest planner may be too slow even when batched; a spike is needed before promising WASM planners.
- **No threads in components.** Parallel planners must use several instances or stay native.
- **Apple Silicon is Tier 2 for wasmtime**, and Windows on ARM is Tier 3. That matters for a desktop app shipping on macOS.
- **Toolchain drift.** wasmtime 50 needs exactly Rust 1.97.0, the build environment's version; 51 and later may need more. `re_viewer` 0.43.2 already needs 1.98 and `nu-plugin` 0.117.0 needs 1.97.1 [V]. The LTS (MSRV 1.95) avoids this.
- **egui churn.** The egui-coupled plugin crate will break with each egui major release; in-process UI plugins must be rebuilt.
- **Freezing too early.** The largest design risk is stabilising the kinematics and planner traits before two real implementations exist.
- **`catch_unwind` is partial.** A built-in plugin that deadlocks or overflows the stack still takes the app down.
- **Not verified today:** the RoboDK driver process model, Blender and VS Code manifest details, Rust's 1.81 abort-on-unwind change, and the claim that Zed extensions draw no UI. All are recalled.
- **Open product decisions:** a STEP importer crate and a script engine have not been chosen.

## Surprises

- **WASI 0.3 is ratified** (June 2026) and on by default in wasmtime, yet Zed still ships WASI p2 on the 48 LTS line.
- **`cargo-component` is legacy.** Its last release was March 2025; the stock `wasm32-wasip2` target replaces it, and rustc 1.97 already lists `wasm32-wasip3`.
- **Zed's host carries ten generations of WIT bindings** side by side. API evolution in WASM means keeping old adapters indefinitely.
- **Zellij runs plugins on the wasmi interpreter**, not wasmtime. Extism still uses only WASI p1 core modules.
- **No official stable Rust ABI is close.** There is no `crabi` feature gate, only an incomplete `export_stable`. `abi_stable` has had no release since 2023, and `stabby` documents a performance regression on stable Rust.
- **Other projects backed away from in-process plugins.** Bevy dropped dynamic plugins after 0.14, and Helix's Steel system is still unmerged.
- **Nushell's handshake ties compatibility to the app version**, so each 0.x release breaks every plugin.
- **UR's kinematics files carry a per-robot calibration hash.** Model and physical instance are distinct concepts and the data model should reflect that from the start.
