# Plan review: Rust architecture and buildability

## 1. [BLOCKER] 6 (Stage 2/3 table) + 4.1 '병렬 구현 약속'

**Problem.** The stage order contradicts the crate graph, and core work has no owner. (a) gc_doc is built in Stage 2 'in parallel', but its stated deps are gc_sketch (Stage 3) and gc_robot (same wave). Its real content (entity model, Command variants and apply, delete cascade, solve-in-Txn, frame graph) is never scheduled again: Stage 3 lists no gc_doc task. (b) gc_persist runs in the same wave as gc_sketch, although it must convert finished gc_doc/gc_sketch/robot/path types. (c) gc_render runs parallel to gc_draw, although it consumes gc_draw's draw list (the table lists only wgpu and bytemuck as gc_render deps). (d) gc_plugin_ur runs parallel to gc_collide, although the UR package ships a 'precomputed ACM' that needs gc_collide plus gc_robot. Under the 'each agent edits only its own directory' rule, an agent cannot compile against a sibling being written in the same wave. It will invent local stand-in types, and these diverge at integration.

**Fix.** Replace Stage 2/3 with dependency-ordered waves. Rule to add: 'a crate enters a wave only when every workspace crate it depends on was merged and gated in an earlier wave'.
- W2a: gc_solver ∥ gc_robot ∥ gc_plan ∥ gc_traj ∥ gc_draw ∥ gc_doc::{store,txn,history} (generic over `Entity`, tested with test-local entity types, no sketch or robot types).
- W2b: gc_sketch ∥ gc_collide ∥ gc_render ∥ gc_plugin_mesh ∥ gc_plugin_ur (URDF generation and analytic IK only).
- W3a: gc_doc::{model,command} (all entities, `Command::apply`, cascade, solve-in-Txn) ∥ gc_motion ∥ gc_plugin_planners.
- W3b: gc_persist ∥ gc_tools ∥ gc_session ∥ gc_scene (the last two are defined in the next issue). The UR ACM golden is generated here by `cargo xtask gen-ur --acm` and committed.
Each wave ends with the orchestrator running the workspace gate before the next wave starts.

## 2. [BLOCKER] 4.1 crate table and layer rule (also 4.4, 5.2, 5.5)

**Problem.** Several cross-cutting responsibilities have no home, and the '주요 의존' column is too thin for Stage 0 to declare all dependencies.
1. Mesh asset type: gc_doc (bodies), gc_robot (link geometry), gc_collide (hull/TriMesh), gc_draw (tessellation, ray picking), gc_render, gc_persist (path + hash) and the `Importer` in gc_plugin_api all need one `MeshData`, content hash and asset store. No crate owns them, and mesh-loader yields f32 `Vec<[f32;3]>` scenes.
2. Document → draw list: 4.4 says 'gc_draw가 드로 리스트 생성', but gc_draw depends only on gc_math and cannot read `Document`, FK caches, plan results or tool previews. gc_tools (domain) cannot depend on gc_draw (render) either.
3. Mesh/link picking is 'ray–BVH' in gc_draw, but the only BVH is parry, which is isolated behind gc_collide.
4. `Intent`/`AppAction`, cache invalidation, job-result intake and doc → MotionRequest/SceneSnapshot bridging live only in gc_app (eframe), so none of it is headless-testable. gc_panels (deps: egui only) cannot even name `Document` or `Intent`.
5. gc_collide needs FK for state checks and ACM sampling, but lists only parry.
6. Folder order puts plugins/ above app/, yet gc_app's BUILTIN list depends on plugin crates; gc_plugin_planners depends on domain crates; and '형제 간 금지' is ambiguous (gc_sketch → gc_solver is a sibling edge).

**Fix.** Add three crates and put the full edge matrix in the plan; Stage 0 declares exactly these edges.
- NEW foundation/gc_mesh {gc_core, gc_math}: `MeshData { positions: Vec<[f32;3]>, indices: Vec<[u32;3]> }`, `MeshHash`, `AssetStore` (Arc, content-addressed), a small median-split triangle BVH with ray cast, and the `.gcmesh` codec.
- NEW domain/gc_session {gc_doc, gc_persist, gc_motion, gc_collide, gc_robot, gc_jobs, gc_mesh, gc_plugin_api, gc_robotics_api}: `Session` (doc, history, caches, registry, jobs), `Intent`, `AppAction`, `apply_intents()`, doc → SceneSnapshot/MotionRequest, job-result intake. Egui-free; golden-scene tests live here.
- NEW render/gc_scene {gc_draw, gc_session, gc_doc, gc_tools, gc_sketch, gc_robot}: `build(&SessionView, &[Preview], &ViewParams) -> (DrawList, Vec<OverlayItem>)`. Every primitive carries a `PickId` (= AnyId), so gc_draw picking stays generic.
- Other edges: gc_math{gc_core}; gc_expr{gc_core}; gc_plugin_api{gc_core, gc_mesh}; gc_robotics_api{gc_core, gc_plugin_api}; gc_sketch{gc_core, gc_math, gc_expr, gc_solver}; gc_robot{gc_core, gc_math, gc_linalg, gc_mesh, gc_robotics_api}; gc_collide{gc_core, gc_math, gc_mesh} (takes link poses, robot-agnostic; ACM sampling takes a pose-sampler callback from gc_motion); gc_motion{gc_core, gc_math, gc_jobs, gc_robot, gc_collide, gc_plan, gc_traj, gc_robotics_api}; gc_doc{gc_core, gc_math, gc_expr, gc_mesh, gc_sketch, gc_robot}; gc_tools{gc_core, gc_math, gc_sketch, gc_doc}; gc_persist{gc_core, gc_doc, gc_sketch, gc_robot, gc_mesh}; gc_draw{gc_core, gc_math, gc_mesh}; gc_render{gc_draw, gc_mesh, gc_math}; gc_ui{gc_i18n}; gc_viewport{gc_ui, gc_render, gc_draw, gc_scene, gc_session, gc_tools}; gc_panels{gc_ui, gc_i18n, gc_session, gc_doc, gc_robotics_api}; gc_plugin_ur{gc_robotics_api, gc_plugin_api, gc_math, gc_linalg, gc_mesh}; gc_plugin_mesh{gc_plugin_api, gc_mesh}; gc_plugin_planners{gc_robotics_api, gc_plugin_api, gc_plan, gc_traj}.
- Layer rule for check-layers: foundation < api < domain < render < ui < app. plugins/ may depend only on foundation + api (+ gc_plugin_api_egui), with one per-crate allowlist entry (gc_plugin_planners → gc_plan, gc_traj). Same-folder edges are allowed if acyclic. dev-dependencies are exempt.
- Move `BUILTIN` to the `graphctrl` bin and expose `gc_app::run(plugins: &[Ctor])`; gc_app tests take plugins as dev-deps.
- `ToolEvent` (gc_tools) carries a precomputed `hover: Option<PickHit>`, world ray and px scale, filled in by gc_viewport.

## 3. [BLOCKER] 4.3 플러그인 구조, 5.6, 5.8 (gc_robotics_api, gc_plugin_ur, gc_plugin_planners, gc_motion)

**Problem.** '내장 기능도 같은 API로 등록' cannot be met as written, and gc_robotics_api is frozen in Stage 1 before any consumer exists.
1. gc_motion depends directly on gc_plan (generic `Planner<S: StateSpace>`), while gc_plugin_planners registers the same planners as `dyn MotionPlanner`. The pipeline will call gc_plan statically and the registry-driven planner choice becomes decorative. The plan describes no adapter between the two.
2. `Refined<Ur6rAnalytic>` needs `gc_robot::IkSolver`, but Ur6rAnalytic lives in gc_plugin_ur, whose only dep is gc_robotics_api. The research put it in a domain `gc-ik` crate.
3. The UR plugin's family fit ('64개 무작위 q에서 FK 일치') needs FK and pose math inside the plugin, which has no math crate.
4. `PlanningScene` in the API is a host callback, contradicting '호스트 내부 참조 금지'. The 'batch only' rule would force a rewrite of every sampling planner.
5. NumericLm, path processors and PlanningProfile have no registration path, so the claim is partly false.

**Fix.** Write the contract into the plan before Stage 1. Object-safe, in-process, signatures use only `&[f64]`, serde POD and `&dyn` API traits:
```rust
pub struct Pose { pub p: [f64; 3], pub q_xyzw: [f64; 4] }
pub struct ChainSpec { pub joints: Vec<JointSpec>, pub tip: Pose, pub family_hint: Option<String>, pub nominal: BTreeMap<String, f64> }
pub trait KinematicsFactory: Send + Sync { fn fit(&self, c: &ChainSpec) -> Option<(FitQuality, Box<dyn KinematicsSolver>)>; }
pub trait KinematicsSolver: Send + Sync { fn solve_all(&self, target: &Pose, seed: &[f64], out_flat: &mut Vec<f64>, tags: &mut Vec<BranchTag>) -> Result<(), IkFailure>; fn classify(&self, q: &[f64]) -> Option<BranchTag>; }
pub struct SpaceSpec { pub lo: Vec<f64>, pub hi: Vec<f64>, pub weights: Vec<f64>, pub continuous: Vec<bool> }
pub trait StateChecker: Sync { fn state_valid(&self, q: &[f64]) -> bool; fn motion_valid(&self, a: &[f64], b: &[f64]) -> bool; fn clearance(&self, q: &[f64]) -> Option<f64>; }
pub trait MotionPlanner: Send + Sync { fn descriptor(&self) -> &PlannerDescriptor; fn plan(&self, req: &PlanRequest<'_>, chk: &dyn StateChecker, ctl: &dyn PlanControl) -> Result<PlanOutput, PlanFailure>; }
```
`PlanRequest` holds space, start, flat goals, params, seed and budget; `TimeParameterizer` follows the same shape.
Rules:
- gc_motion receives `&dyn MotionPlanner`, `&dyn TimeParameterizer` and `Option<Arc<dyn KinematicsSolver>>` from the caller and never names a concrete planner. The Free-move fallback looks up id "gc.rrt_connect". It uses gc_plan only for path processors and implements `StateChecker` over gc_robot + gc_collide. TCP-sphere mode uses the same trait with a 3-dof SpaceSpec.
- gc_plugin_planners holds one generic adapter `Adapter<P>`: SpaceSpec → `RealVectorSpace`, `&dyn StateChecker` → `StateValidity`/`MotionValidity`. It also owns the descriptors and param schemas.
- gc_robot provides `Chain::to_spec()` and `struct PluginIk(Arc<dyn KinematicsSolver>)` implementing `IkSolver`, so `Refined<PluginIk>` works.
- Plugins may use foundation crates internally (gc_math, gc_linalg); the POD rule applies to signatures only.
- State honestly what M1 routes through the API (planners, retimers, UR IK, importers, robot packages) and what stays host-fixed (NumericLm, URDF reader, collision, path processors, solver).
- `PlanningProfile` is a serde type in gc_robotics_api, reused by gc_robot (no duplicate DTO).

## 4. [MAJOR] 4.2 (Txn, 명령은 데이터), 4.4, 5.1, 5.5, 5.9

**Problem.** The Txn/Command/Intent/History rules are individually sound but not mutually specified.
1. Granularity: the plan nests points/curves/constraints inside `Sketch` (gc_sketch), while the Txn sketch it builds on has flat per-entity stores. It is undefined what a before-image is, who allocates sub-entity ids (gc_sketch sits below gc_doc), and how a waypoint → sketch-point reference is cascaded or validated.
2. Drag: the solver's owning typestate `Idle(System) ↔ Dragging` must live across frames, but Txns and Commands are per frame. Its owner is undefined, and replaying per-frame drag Commands is path-dependent.
3. Background results: 5.9 marks results stale by `DocRevision` (every drag frame bumps it), while 5.1 says only segments whose input hash changed go stale. ACM generation and import results must mutate the document at a later revision, with no stated path.
4. Commands creating several entities (line + inferred constraints, rectangle, trim) need ids that do not exist yet. LineTool chaining needs the created point id back.
5. The research sketch marks `Command` `#[non_exhaustive]`, which forces `_ =>` in gc_persist/gc_session and contradicts the no-wildcard rule.
6. The outcome of a Command whose solve fails is undefined (rollback, or commit with stale coordinates).

**Fix.** Add these rules.
1. Txn entities are the top-level store items only: Param, Frame, Sketch (whole), Body, RobotInstance, Waypoint, Path. The before-image of a sketch is one `Sketch` clone per Txn. All ids come from one per-document `gc_core::IdAlloc`, passed as `&mut` into gc_sketch functions, so raw u64s are document-unique. `Document::owner_of(AnyId)` is rebuilt at commit. `ChangeSet` = top-level ids plus, for sketches, the diff of sub-ids. `next_id` is never rolled back. `Revision` increases on undo/redo too.
2. `DragSession` (solver system + signature baseline) is owned by gc_session, 1:1 with History's open step. `Intent::{BeginDrag, DragTo, EndDrag, CancelDrag}` are not Commands. Each DragTo opens a Txn and runs `gc_sketch::drag_step`; it commits and absorbs on success, and drops the Txn on a flip or failure. EndDrag journals one `Command::SetSketchCoords { sketch, coords }`; replay assigns coordinates and checks residuals, with no re-solve. While a drag is open, Undo/Redo, other edits and job-result commands are deferred.
3. Job results never touch the document directly. Derived results are cached under `(JobId, InputHash)` and freshness is decided by InputHash only (delete the DocRevision wording in 5.9). Results that change the document (import → AddBody, ACM → SetAcm) become Commands queued as Intents and applied with a precondition (target exists, base InputHash equal), else discarded with a log entry. InputHash uses a fixed in-house hasher in gc_core (f64 via `to_bits`, −0.0 normalised); `DefaultHasher` is banned.
4. One Intent = one Txn = one History step. A user action that creates several entities is one `Command::SketchEdit { sketch, ops: Vec<SketchOp> }` with `Local(u32)` placeholders. `apply` returns `Applied { created: Vec<AnyId> }`, which the session hands back to the active tool next frame.
5. No `#[non_exhaustive]` on workspace-internal enums in M1.
6. A failed solve never rolls back the user's edit: the Txn commits the edit with the last good coordinates, and the sketch status `Unsolved { reason }` is a cache recomputed deterministically from document state (also after load and undo).

## 5. [MAJOR] 4.1 '병렬 구현 약속' + 6 '실행 방식' (shared target directory, Cargo.lock fixed in Stage 0)

**Problem.** The parallel mechanics are underspecified.
1. Local cargo 1.97.0 still lists `-Z fine-grain-locking` as unstable, so stable cargo locks the whole build directory. N agents on one target dir run strictly one at a time ('Blocking waiting for file lock'), and a first egui/wgpu build can outlast an agent's patience.
2. Any `--workspace` command an agent runs fails on siblings' work in progress.
3. A lock frozen in Stage 0 is only real if dev-dependencies (proptest, insta, criterion, oracles) are also declared then and every command uses `--locked`.
4. `-p crate` and `--workspace` builds resolve different feature sets (for example kittest adds wgpu `metal`/`gles`), which rebuilds wgpu-core/naga back and forth.
5. Shared edit hotspots break the 'own directory only' rule: the gc_core `Kind` enum, the gc_i18n catalogue, API crates and gc_draw's draw-list types.
6. No contract list exists, and stubs must pass clippy without `todo!`.

**Fix.** Add a 'parallel protocol' subsection.
- Agents run only `cargo check|clippy|test -p <own crate> --locked` and `cargo fmt -p <own crate>`. They never run `--workspace`, `cargo update`, `cargo add`, or edit any manifest.
- A missing dependency or contract change is returned as a written request. The orchestrator applies it between waves and is the only editor of root files, Cargo.lock, API crates and contract modules.
- Target dirs: headless crates (foundation/api/domain) use `CARGO_TARGET_DIR=target/agents/<crate>`; their dep trees are small (estimate under 1 GB each). Crates that pull egui/wgpu share `target/` and build one at a time.
- Stage 0 declares deps and dev-deps for every crate, with features fixed centrally in `[workspace.dependencies]` so `-p` and workspace builds resolve identically.
- Contracts the orchestrator commits as real type and trait definitions (no function bodies) before W2a: gc_core (Id, AnyId, full `Kind` list, IdAlloc, Revision, Tolerance, units, StableHasher); gc_math export list and pose convention; gc_mesh::MeshData; gc_plugin_api (Registrar, ExtensionPoint, Importer); gc_robotics_api (issue 3); gc_jobs (Executor, CancelToken, Progress); gc_draw::list (DrawList, PickId); gc_doc (Entity, Txn, Applied).
- Before W3: gc_solver and gc_plan public APIs are frozen at W2 exit; gc_tools (ToolEvent, ToolFlow, Preview); gc_session (Intent, SessionView).
- i18n: each crate owns its catalogue through a `gc_i18n::messages!` macro. gc_i18n holds only the macro and the language switch. Domain crates return diagnostics as enums; ui crates localise them.

## 6. [MAJOR] 4.2 '오류 정책'·'결정성' + 6 gate (`cargo clippy --workspace --all-targets -- -D warnings`)

**Problem.** The lint policy combined with `-D warnings` will stall agents. These points are from clippy knowledge and were not executed here.
1. The underlying manifest sketch enables `pedantic`, `indexing_slicing`, `allow_attributes` and `unreachable_pub` at warn; all become hard errors. Dense kernels (gc_linalg, gc_solver, gc_plan, gc_traj, gc_robot) would hit `indexing_slicing`, `needless_range_loop`, `many_single_char_names` and `too_many_lines` hundreds of times.
2. With `allow_attributes`, the only escape is `#[expect]`. An unfulfilled crate-level `#![expect]` (Stage 0 stubs, cfg(test) builds) is itself a warning, so it is an error.
3. `unwrap_used`/`expect_used`/`panic` deny with `--all-targets` also hits helper functions in tests/, benches/, examples/ and xtask; `allow-unwrap-in-tests` covers only `#[test]` fns and `#[cfg(test)]` modules.
4. '`match`에 와일드카드 금지' as a workspace lint (`wildcard_enum_match_arm`) is impractical in ui/render crates that match egui::Key/Event and wgpu enums.
5. `clippy::panic` does not stop index, slice, overflow or `unreachable!` panics, so the lints do not deliver panic freedom.

**Fix.** Put this lint table in the plan.
- `[workspace.lints.rust]`: `unsafe_code = "deny"` (plus `#![forbid(unsafe_code)]` in foundation/domain roots).
- `[workspace.lints.clippy]`: `all = warn` (priority −1); no blanket `pedantic`.
- deny: unwrap_used, expect_used, panic, todo, unimplemented, dbg_macro, exit, mem_forget, undocumented_unsafe_blocks.
- warn: iter_over_hash_type, float_cmp, map_err_ignore, let_underscore_must_use, print_stdout, print_stderr.
- Not enabled: allow_attributes, indexing_slicing (workspace-wide), unreachable_pub, missing_debug_implementations.
- allow: needless_range_loop, many_single_char_names, too_many_arguments, too_many_lines, the cast_* family.
- clippy.toml: allow-unwrap-in-tests, allow-expect-in-tests, allow-panic-in-tests, allow-indexing-slicing-in-tests, allow-dbg-in-tests, allow-print-in-tests = true.
- Every tests/*.rs, benches/*.rs and examples/*.rs starts with `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`. xtask has its own `[lints]` table.
- Opt-in crate-level lints: `#![warn(clippy::indexing_slicing)]` only in crates that parse untrusted input (gc_expr, gc_persist, gc_robot::urdf, gc_plugin_mesh); `#![deny(clippy::wildcard_enum_match_arm)]` only in gc_solver, gc_sketch, gc_doc.
- Sanctioned escape hatch for proven invariants: per-site `#[allow(clippy::expect_used, reason = "invariant: …")]`, counted and reported in Stage 6.
- Reword the claim: panic safety rests on `catch_unwind` boundaries plus fuzz/property tests, not on lints.

## 7. [MAJOR] 6 '프로필' + 7 검증 + 부록 E (Q3, Q4, Q8) + 8 (계획 성능)

**Problem.** Profile and test-time realities on 4 cores and 30 GB free (verified by `df`).
1. `[profile.dev.package."*"]` does not apply to workspace members. gc_linalg, gc_solver, gc_robot, gc_collide glue, gc_plan and gc_traj therefore run at opt-level 1 with debug assertions in `cargo test`, and generic planners are monomorphised in gc_motion/gc_plugin_planners at that level.
2. Appendix E gates are wall-clock ('5 s 내', 'RRT* 30 s', '< 300 ms'), which contradicts 4.2 '벽시계 대신 반복 횟수 예산'. As `cargo test` gates they are slow (100 seeds, 30 s RRT* runs, 10,000-sample ACM) and meaningless in a dev profile.
3. The plan dropped `debug = "line-tables-only"` from the research manifest. Full debuginfo for wgpu-core/naga/egui at opt-level 2, plus one statically linked test binary per tests/*.rs file, plus separate check artifacts for clippy and a thin-LTO release tree, can plausibly exceed 15–20 GB (estimate, not measured).
4. The first cold dev build of the eframe + wgpu + parry tree at opt-level 2 is likely 10–20 minutes on 4 cores (estimate), and it repeats for clippy and release.

**Fix.** Replace the profile line with:
```toml
[profile.dev]
opt-level = 1
debug = "line-tables-only"
[profile.dev.package."*"]
opt-level = 2
debug = false
# repeat for gc_solver, gc_robot, gc_collide, gc_plan, gc_traj, gc_motion, gc_plugin_planners, gc_plugin_ur
[profile.dev.package.gc_linalg]
opt-level = 3
[profile.release]
panic = "unwind"
lto = "thin"
debug = "line-tables-only"
```
Rules to add:
- All `cargo test` gates use iteration or check-count budgets; no wall-clock assertion.
- Appendix E Q3/Q4/Q8 numbers come from `cargo xtask bench-plan` (release, fixed seeds). They are reported per stage and are not a pass/fail unit test. Q4's reference is 'RRT* with N iterations', not 30 s.
- Long property or statistical tests are `#[ignore]` and run once per stage gate with `-- --ignored`.
- One integration-test binary per crate (`tests/it/main.rs` with modules).
- The release build runs only at Stage 0 (proof) and Stage 6.
- The orchestrator records `du -sh target` at every gate, with a 20 GB ceiling; above it, `cargo clean -p` the per-agent dirs.
- GPU tests sit behind a `gpu-tests` feature, so headless agents never build wgpu.

## 8. [MAJOR] 4.2 '결정성', 5.4/5.8 golden tests, 부록 E Q9, 부록 A (glamx / parry features)

**Problem.** The plan promises '`libm` 삼각함수' and byte-identical outputs, with goldens (ACM, golden-scene plans, solver snapshots) created in the Linux sandbox and checked on Windows CI. Verified on crates.io: parry3d-f64 0.31.1 depends on glamx with `nostd-libm` only, and glamx/glam use std (platform libm) trig whenever `std` is on. FK through glam quaternions and parry queries will therefore differ in the last ulp between glibc and the MSVC CRT. Sampling planners amplify one ulp into a different tree, so byte-exact goldens fail on the other OS even if all in-house code uses libm. The fix exists and is not in the plan: parry feature `enhanced-determinism` = simba/libm_force + indexmap + glamx/libm + glamx/scalar-math, which conflicts only with `simd8`.

**Fix.** Add to 부록 A and Stage 0:
- `parry3d-f64 = { version = "=0.31.1", features = ["enhanced-determinism"] }` (never `simd8` or `parallel`).
- `glamx = { version = "=0.3.1", default-features = false, features = ["std", "f64", "libm", "scalar-math"] }`, named only by gc_math.
- clippy.toml `disallowed-methods` for `f64::{sin, cos, tan, asin, acos, atan, atan2, sin_cos, exp, ln, powf, hypot, cbrt}` with the message 'use gc_math::fm'. `sqrt` and basic arithmetic are IEEE-exact and stay allowed.
- Test rule: byte-exact comparison is asserted only within one run or one platform ('same seed twice → same bytes'). Committed golden files are compared with tolerances (positions 1e-9, plan metrics by threshold), or carry a per-OS suffix.
- State Q9 as 'bit-identical on the same platform; cross-platform is best effort, backed by enhanced-determinism'.

## 9. [MAJOR] 2 '의존성 정책' + 부록 A + 5.4 (dev 오라클) + 5.6 (gen-ur)

**Problem.** Manifest facts that break 'Cargo.lock fixed in Stage 0' or the licence rule (all checked on crates.io today).
1. 'MIT/Apache/BSD 계열만' is already violated by unavoidable transitive crates: foldhash 0.2 is Zlib (non-optional parry dep); unicode-ident is Unicode-3.0; epaint_default_fonts is OFL-1.1 AND Ubuntu-font-1.0 (eframe default `default_fonts`); libloading is ISC; option-ext is MPL-2.0 if `directories` is used.
2. `cargo xtask gen-ur` parses UR YAML, but no YAML crate is chosen and serde_yaml is `0.9.34+deprecated`.
3. 'Optional dev oracles' (urdf-rs, ezpz → faer ^0.24, sketch-solver) cannot be optional: Cargo forbids optional dev-dependencies. Every `cargo test --workspace` would compile faer.
4. mesh-loader 0.1.13 (last release 2024-10-15, f32 output) pulls roxmltree ^0.20 through its default `collada` feature, a second roxmltree next to our 0.21.
5. rfd 0.18 defaults to `xdg-portal` + `wayland` (wayland-backend `client_system`); no portal exists under Xvfb/kittest.
6. `plugin.toml` needs a `toml` dep that is not listed; the asset hash crate is unspecified (blake3 needs a C/asm build).
7. Appendix A nits: roxmltree 0.21.0 and parry3d-f64 0.31.0 are yanked (use 0.21.2 and 0.31.1); wgpu 30.0.2 was published 2026-11-12.

**Fix.** Amend 2 and 부록 A.
- Licence allow-list (deny.toml): MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, BSL-1.0, CC0-1.0, Unicode-3.0, OFL-1.1, Ubuntu-font-1.0. Still banned: GPL, LGPL, AGPL, MPL, EUPL. Use our own path lookup instead of `directories`, or record an explicit option-ext exception.
- Exact pins (`=`): egui, eframe, egui-wgpu, egui_kittest, egui_extras 0.37.0; wgpu 30.0.2; egui_tiles 0.18.0; egui_ltreeview 0.10.0; glamx 0.3.1; parry3d-f64 0.31.1. Everything else uses caret + Cargo.lock + `--locked`.
- `mesh-loader = { version = "0.1.13", default-features = false, features = ["stl", "obj"] }`; `collada` only in xtask. Wrap calls in size caps and `catch_unwind`. Fallback: an in-house binary/ASCII STL reader in gc_plugin_mesh.
- YAML: `yaml-rust2 = "0.13"` (MIT/Apache, MSRV 1.85), in xtask only for M1.
- Oracles: a separate crate `tools/oracles` with its own `[workspace]` and Cargo.lock, outside the main workspace; run manually once per stage. Remove them from member dev-deps.
- eframe keeps default features (+ `persistence` if used). rfd is used only behind a `trait FileDialogs` in gc_app, with a scripted fake in tests.
- M1 `PluginManifest` is a Rust `const` (drop plugin.toml, or parse it only in a test with `toml` as a dev-dep). Asset hash: `sha2 = "0.11"` or the in-house 128-bit hasher.
- Add `.cargo/config.toml` with `[alias] xtask = "run -p xtask --"`; `rust-toolchain.toml` with `channel = "1.97.0"`, `components = ["clippy", "rustfmt"]`; `rust-version = "1.97"`.
- check-layers also asserts exactly one version each of glam, wgpu, naga, egui and winit in `cargo metadata`, and that glamx, parry, wgpu, egui, roxmltree and mesh-loader are named only by their designated crates.

## 10. [MINOR] 4.2 '패닉 격리' + 5.9

**Problem.** The panic story has three gaps.
1. '패닉 훅이 마지막 커밋 문서를 복구 폴더에 저장': the hook runs on whichever thread panicked and cannot borrow the `Document` owned by the UI thread. A panic inside `App::ui` unwinds through winit and ends the process.
2. A solver or plugin panic inside a Txn unwinds through `Txn::drop`. If rollback can itself panic, the process aborts (double panic).
3. `catch_unwind` over `&mut Registrar`/`&mut Document` needs `AssertUnwindSafe`, which is only sound if state is restored afterwards.

**Fix.** Specify:
- gc_session keeps `Arc<Mutex<Option<Arc<[u8]>>>>` holding the serialised bytes of the last committed document, refreshed on the debounced autosave path. The panic hook only writes those bytes plus a log file and never touches live state.
- `Txn::drop` rollback is swap-only and must be panic-free (no allocation-dependent logic, no validation). A property test injects a panic mid-command and asserts `doc == before`.
- `catch_unwind` + `AssertUnwindSafe` is allowed only at three places: job boundary, plugin call boundary, and `Session::apply_command` (where Txn drop restores state). Egui panel code is never wrapped.
- After a caught plugin panic, that plugin's contributions are disabled for the session.

## 11. [MINOR] 4.3 (ExtensionPoint/Key, plugin.toml, semver) + gc_plugin_api row

**Problem.** 1. The storage mechanics of `ExtensionPoint<T>`/`Key<T>` are unspecified for unsized `T = dyn Trait`. gc_plugin_api cannot know robotics traits, so the keys must be declared in gc_robotics_api and stored type-erased.
2. A runtime semver 'api 범위' check between crates compiled in one workspace compares a constant with itself. It adds a dependency and code with no M1 effect.
3. If API structs and enums are marked `#[non_exhaustive]` for future-proofing, plugins cannot build them with struct literals and must add wildcard arms.
4. The `MotionPlanner` parameter-schema types (ParamSchema, ParamValues) are referenced but owned by no crate.

**Fix.** Specify:
- `pub struct ExtensionPoint<T: ?Sized + 'static> { id: &'static str, _t: PhantomData<fn() -> Arc<T>> }`. The Registrar stores `Vec<Box<dyn Any + Send + Sync>>` holding `Arc<T>` and looks up with `downcast_ref::<Arc<T>>()`.
- Keys are `pub const` in the crate that owns the trait, for example `gc_robotics_api::MOTION_PLANNERS: ExtensionPoint<dyn MotionPlanner>`.
- Add a compile test per API trait: `const _: Option<&dyn MotionPlanner> = None;`.
- M1 manifest: `const` struct with `api: u32` (one integer `GC_PLUGIN_API_VERSION`), checked with `==`. Defer semver ranges and plugin.toml to the WASM/process milestone.
- No `#[non_exhaustive]` in API crates during 0.0.x. Trait methods added later must have default bodies.
- `ParamSchema`, `ParamValue` and `ParamValues` (a BTreeMap<String, ParamValue>) live in gc_plugin_api and are reused by the planner descriptor, retimers, the persisted `StrategyRef` in gc_doc and the auto-generated editor in gc_panels.

## 12. [MINOR] 6 Stage 0 gate + 8 (SwiftShader risk) + 7 (GPU tests)

**Problem.** Stage 0 is the right place to retire risk, but its gate ('PNG를 직접 열어 확인') omits the build-system facts that later stages depend on.
- Nothing has been compiled yet: the registry is empty (~/.cargo is 20 MB).
- Feature unification of the direct `wgpu` dep with egui-wgpu (`^30.0`, default-features off, `std` + `wgsl`) and egui_kittest (adds metal, dx12, vulkan, gles) is unproven.
- Stub crates must pass `clippy -D warnings` with no `todo!`.
- If SwiftShader fails and GPU tests become CI-only, `cargo test --workspace` in the sandbox must still pass by skipping, not by failing on 'no adapter'.

**Fix.** Extend the Stage 0 gate with recorded evidence:
1. `cargo build --workspace --locked` and `cargo clippy --workspace --all-targets --locked -- -D warnings` pass on the all-stub workspace, including the three new crates.
2. `cargo tree -d` shows a single wgpu, naga, glam, egui and winit.
3. Cold wall time and `du -sh target` for dev, clippy and one release build are written into the plan's risk table; the estimates in the profile issue are replaced by measurements.
4. The offscreen pipeline (MSAA 4x + Depth32Float + Rgba8Unorm resolve + `register_native_texture`) renders under SwiftShader through kittest, and a readback pixel assertion passes.
5. GPU tests follow one convention: feature `gpu-tests` plus a helper `gpu_or_skip()` that returns early with a printed SKIP when no adapter exists. Sandbox and CI set `GC_REQUIRE_GPU=1`, which turns a skip into a failure.
6. `cargo run -p xtask -- check-layers` runs against the matrix from the crate-table issue.

## Keep (must not change)

- Offscreen-target viewport (own MSAA + depth target shown via `register_native_texture`) with a GPU-free gc_draw and headless foundation/domain crates. Most of the product can then be tested with `cargo test -p <crate>` without compiling egui/wgpu, which is also what makes per-agent target dirs feasible.
- Typed monotonic `Id<T>` + `BTreeMap` stores + swap-based before-image `Txn` (drop = rollback, undo/redo = same swap), with solver results written through the Txn. It needs the granularity and drag rules from the Txn issue, but the mechanism itself should stay.
- Isolation of volatile dependencies behind one crate each (glamx → gc_math, parry → gc_collide, wgpu → gc_render, third-party widgets → gc_ui), with exact pins and a committed Cargo.lock. Verified today: glamx 0.3.1 requires glam ^0.33.7, so glam 0.34 cannot enter; the MSRV of every major transitive crate checked is ≤ 1.95, below the pinned 1.97.0.
- `unsafe_code = "deny"` workspace-wide with `forbid` in foundation/domain. It is compatible with bytemuck derives and wgpu use: epaint 0.37.0 itself derives `bytemuck::Pod` under `unsafe_code = "deny"`. No scoping exception is needed for the render crates.
- In-house gc_linalg, URDF reader and planners on a robot-agnostic `gc_plan`, plus compile-time plugins with an explicit constructor list and `InlineExecutor` for deterministic tests. This avoids the faer/nalgebra feature-unification and determinism hazards and keeps the arm and TCP-sphere paths on one code base.

## Verdict

The technology choices in /root/.claude/plans/os-rust-recursive-turing.md hold up, but the plan is not yet executable by parallel agents: three blockers must be fixed in the text before Stage 0. Nothing was compiled in this review; version, feature and licence facts were checked against crates.io and the local toolchain, while build-time and disk figures are estimates. The pinned versions exist and unify, MSRVs fit Rust 1.97, and the unsafe policy works with bytemuck.

The three blockers are:
- **Stage order:** Stage 2/3 contradicts the dependency graph. gc_doc is built before gc_sketch, gc_persist alongside the types it serialises, gc_render alongside gc_draw.
- **Missing owners:** the mesh asset type, Document → draw-list extraction, and headless Intent/session/cache logic have no crate, and the dependency column is too thin to freeze Cargo.lock.
- **Plugin API:** "built-ins through the same API" has no defined bridge between generic gc_plan planners or `gc_robot::IkSolver` and the object-safe POD API, and the API is frozen before any consumer exists.

All three are fixable with about three small crates (gc_mesh, gc_session, gc_scene), a full edge matrix, dependency-ordered waves and a written trait contract of roughly 150 lines.

The remaining major items are mechanical but will each cost a day of agent thrash if left implicit:
- Txn/drag/job-result rules.
- A parallel protocol (stable cargo locks the whole build directory, `--locked`, an orchestrator-owned contract list).
- A lint table that does not turn pedantic warnings into errors.
- Opt-level overrides for workspace numeric crates, and iteration budgets instead of wall-clock gates.
- parry `enhanced-determinism` with glamx `libm` for cross-OS goldens.
- A realistic licence allow-list (foldhash is Zlib, epaint fonts are OFL/Ubuntu).
- A YAML crate for gen-ur, and moving the dev oracles out of the workspace.

With those edits the M1 scope remains very large but buildable on the 4-core sandbox.
