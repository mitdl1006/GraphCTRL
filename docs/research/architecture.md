# GraphCTRL: architecture conventions and supporting crates

**Evidence key.** [V] verified today, [R] recalled from memory. Crate versions, dates, licences and egui requirements are [V] from `https://crates.io/api/v1/crates/NAME[/VER/dependencies]` unless marked. Repo files are cited as `owner/repo:path` under `https://raw.githubusercontent.com/` (default branch). Crate files come from `https://static.crates.io/crates/NAME/NAME-VER.crate`. "Local" means this build machine. Web: [wild 0.10](https://phoronix.com/news/Wild-Linker-0.10), [wild at the Rust Foundation](https://rustfoundation.org/?p=105369), [Cranelift goal](https://goals.rust-lang.org/2025h2/production-ready-cranelift.html).

## 1. Workspace architecture

**Layers.** Folder = layer; a crate may depend on its own layer or a lower one.

| Folder | Crates | Constraint |
|---|---|---|
| `foundation/` | `gc_core` (typed ids, `Length`/`Angle`, tolerances, `Revision`), `gc_expr` | std, serde, thiserror only |
| `domain/` | `gc_sketch`, `gc_solver`, `gc_doc` (stores, transactions, history, commands, selection), `gc_persist` | headless: no egui, wgpu, winit |
| `api/` | `gc_plugin_api` | the only crate plugins see |
| `render/` | `gc_draw` (GPU-free tessellation, draw lists, camera, picking), `gc_render` (wgpu) | no egui |
| `ui/` | `gc_i18n`, `gc_ui` (tokens, ribbon, property grid, icons), `gc_viewport`, `gc_panels` | egui |
| `app/` | `gc_app` (library, kittest-able), `graphctrl` (binary) | eframe wiring, crash handler |
| `plugins/` | later: `gc_robot_ur` | depends on `gc_plugin_api` |

**Enforcement.**
- **Layer check:** `cargo xtask check-layers` over `cargo metadata` in CI. Rerun works this way: a script fails CI on an upward dependency, dev-dependencies are exempt, and sibling folders may not depend on each other [V: `rerun-io/rerun:crates/README.md`].
- **Banned direct dependencies:** cargo-deny `bans.deny` with `wrappers`, which "allows specific crates to have a direct dependency on the banned crate" [V: `EmbarkStudios/cargo-deny:docs/src/checks/bans/cfg.md`]. This keeps `egui`, `wgpu`, `winit` and `rfd` out of lower layers.
- **Unsafe:** `#![forbid(unsafe_code)]` in foundation and domain crate roots. The workspace level is `deny`, as in Rerun, egui and Bevy [V: their `Cargo.toml`].

**Conventions in other projects** [V: each root `Cargo.toml`].
- **Rerun:**
  - Clippy `all`, `pedantic` and `cargo` at warn, plus about 70 cherry-picked lints (`unwrap_used`, `todo`, `dbg_macro`, `print_stdout`, `iter_over_hash_type`, `allow_attributes`, `map_err_ignore`).
  - Pedantic opt-outs are recorded with hit counts (`must_use_candidate` 3134, the `cast_*` family).
  - `clippy.toml` sets `allow-unwrap-in-tests` and bans methods such as `glam::Vec3::normalize` ("can create NaNs") and hard-coded colours.
  - Dev `opt-level = 1`, dependencies 2.
- **Zed:** few lints ("slows down shipping"); dev `debug = "limited"`; proc-macro crates at `opt-level = 3`.
- **Ruffle:** a `ci` profile (release plus debug assertions and overflow checks) and a `dist` profile with fat LTO.
- **Bevy:** `allow_attributes_without_reason`, `undocumented_unsafe_blocks`; it notes that Cargo cannot override workspace lints per crate.
- **rmf_site:** `resolver = "3"`; the file-format crate is separate from the editor.
- **egui CI:** pinned toolchain, `-D warnings` only in CI, only `main` writes the cache [V: `emilk/egui:.github/workflows/rust.yml`].

Root manifest sketch. Every lint name exists in local clippy 0.1.97 [V: `clippy-driver -W help`].

```toml
[workspace]
resolver = "3"                       # must be explicit in a virtual workspace
members = ["crates/*/*", "xtask"]

[workspace.package]
edition = "2024"
rust-version = "1.97"                # same as rust-toolchain.toml; egui 0.37 needs >= 1.95
license = "MIT"
publish = false

[workspace.dependencies]             # one version per crate, defaults off, members opt in
egui    = { version = "0.37", default-features = false }
serde   = { version = "1", features = ["derive"] }
gc_core = { path = "crates/foundation/gc_core" }

[workspace.lints.rust]
unsafe_code = "deny"
unreachable_pub = "warn"
missing_debug_implementations = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all      = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }   # keeps float_cmp on, unlike Rerun
# no panics outside tests (clippy.toml: allow-unwrap-in-tests = true, and siblings)
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
unwrap_in_result = "deny"
indexing_slicing = "warn"            # #![expect] it in dense numeric kernels
string_slice = "warn"
exit = "deny"
mem_forget = "deny"
dbg_macro = "deny"
print_stdout = "warn"
print_stderr = "warn"
# determinism, numerics, hygiene
iter_over_hash_type = "warn"
float_cmp_const = "warn"
lossy_float_literal = "warn"
let_underscore_must_use = "warn"
map_err_ignore = "warn"
allow_attributes = "warn"            # use #[expect(.., reason = "..")]
allow_attributes_without_reason = "warn"
undocumented_unsafe_blocks = "deny"
# pedantic noise (same opt-outs as Rerun)
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
cast_possible_truncation = "allow"
cast_precision_loss = "allow"
cast_sign_loss = "allow"
similar_names = "allow"
items_after_statements = "allow"

[profile.dev]
opt-level = 1
debug = "line-tables-only"
[profile.dev.package."*"]
opt-level = 2                        # egui, wgpu and naga are very slow unoptimised
[profile.release]
panic = "unwind"                     # needed for catch_unwind and the emergency save
lto = "thin"
codegen-units = 1
debug = "line-tables-only"           # symbolised crash reports
[profile.dist]
inherits = "release"
lto = "fat"
```

**Policies.**
- **Toolchain:** pin 1.97.0 and set `rust-version` to match. Rerun, Graphite and Lapce already require 1.98 [V], so the MSRV-aware resolver 3 matters: it keeps `cargo update` away from crates newer than the toolchain [R].
- **Dependencies:** a member can override an inherited `default-features` only from Rust 1.99 [V: `rust-lang/rust:RELEASES.md`], so decide it centrally.
- **Lint exceptions:** crate-level `#![expect(lint, reason = "...")]`, because the manifest cannot override workspace lints.
- **Features:** additive and few, for example `profiling` and `gpu-tests`.
- **Edition 2024 idioms** [V: RELEASES.md]: let chains (1.88), async closures (1.85), trait upcasting (1.86), `get_disjoint_mut` (1.86), `cfg_select!` (1.95), `assert_matches!` (1.96).
- **Build time:**
  - Optimised dependencies in dev is the main win.
  - rust-lld has been the default linker on x86_64 Linux since 1.90 [V: RELEASES.md]. mold and wild are not installed here [V local], and wild 0.10 is Linux-only [V web].
  - The Cranelift backend is still a nightly-only component [V web; Rerun keeps it as an opt-in nightly profile], so skip it.
- **CI jobs:** `fmt`; `clippy --all-targets --all-features`; tests on three systems; `doc`; cargo-deny; typos; unused dependencies; `check-layers`. Scheduled: miri on foundation and domain crates, fuzzing, mutants, coverage, GPU snapshots.

## 2. Document model and editing

| Approach | Who uses it | Trade-off |
|---|---|---|
| Inverse commands | classic | every command needs a correct hand-written inverse |
| Transactions recording per-entity before-images | FreeCAD [R]; Graphite wraps edits in Start/Commit/AbortTransaction messages, 100 steps [V: `GraphiteEditor/Graphite:editor/src/messages/portfolio/document/document_message_handler.rs`, `editor/src/consts.rs`] | generic (needs only `Clone`) and the delta doubles as the dirty set; all mutation must pass the recorder |
| Whole-state snapshots | SolveSpace copies its group, request, constraint and param lists per step, `MAX_UNDO = 100` [V: `solvespace/solvespace:src/undoredo.cpp`]; Blender [R] | cannot desynchronise; O(n) per step without persistent maps (imbl 7.0.2 is MPL-2.0, rpds 1.2.1 MIT, `im` unreleased since 2022) |
| History inside a temporal store | Rerun: "when undoing, we move back time", `MAX_UNDOS = 100` [V: `rerun-io/rerun:crates/viewer_support/re_viewer_context/src/undo.rs`] | needs that store |
| Operation CRDT | Zed buffers [R]; yrs 0.28.1, automerge 0.12.0, loro 1.18.0 | only pays off with collaboration |

Recommended: the second row, made bypass-proof by the type system.

- **Ids:** typed `Id<T>` over a per-document monotonic `NonZeroU64`, stored in `BTreeMap`. Not slotmap, for two reasons:
  - It reuses slots [R], so undoing a delete cannot restore the old key.
  - It serialises the whole slot vector, vacant slots included [V: `slotmap-1.1.1/src/basic.rs:1348`].
  - The alternatives thunderdome 0.6.1 and generational-arena 0.2.9 have had no release since 2023.
- **Mutation:** `Document` fields are private. The only `&mut` path is `Txn`, which records each entity's first-touch image. Undo and redo are the same swap, and dropping a `Txn` rolls back.
- **Referential integrity:** `Entity::refs` feeds a reverse index. Deleting geometry cascades to its constraints and dimensions inside the same transaction, so undo restores all of it. `commit` rejects dangling references, and the same validator runs on every loaded file.
- **Solver:** it runs inside the transaction and writes solved coordinates through it. Undo then restores the exact pre-solve state without re-solving.
- **Change tracking:** each commit returns a `ChangeSet` and bumps `Revision`. Solve results, tessellation and GPU buffers remember the revision they were built from; panels read the document every frame.
- **Selection:** kept outside the document and the file, as a `BTreeSet<AnyId>` plus primary and hover. Each history step stores the selection to restore, and removed ids are pruned.
- **Drags:** `BeginDrag` opens a step, per-frame commits are absorbed into it, and Esc swaps it back.
- **Intents:** panels receive `&Document` and push `Intent` values; the app drains them once per frame. Rerun does the same with a `SystemCommand` queue that includes `UndoBlueprint` [V: `rerun-io/rerun:crates/viewer_support/re_viewer_context/src/command_sender.rs`].

```rust
// gc_core
/// Allocated from a saved per-document counter and never reused, so undoing a delete restores the
/// same id. Serialises as a bare u64; Copy/Eq/Ord/Hash are written by hand (no `T:` bounds).
pub struct Id<T> { raw: NonZeroU64, kind: PhantomData<fn() -> T> }

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AnyId { pub kind: Kind, pub raw: NonZeroU64 }      // erased: change sets, selection, refs

pub trait Entity: Clone + PartialEq + 'static {
    const KIND: Kind;
    fn refs(&self, out: &mut Vec<AnyId>);                      // what this entity points at
}

// gc_doc
#[derive(Clone, PartialEq)]
pub struct Store<T: Entity> { items: BTreeMap<Id<T>, T> }      // sorted: deterministic iteration and files

/// First-touch "before" images. `None` means the entity did not exist.
pub struct Delta<T: Entity> { images: BTreeMap<Id<T>, Option<T>> }

impl<T: Entity> Delta<T> {
    /// Swap images with the live store. The same call undoes and redoes; no hand-written inverses.
    fn swap(&mut self, store: &mut Store<T>, changed: &mut ChangeSet) {
        for (id, image) in &mut self.images {
            *image = match image.take() {
                Some(old) => store.items.insert(*id, old),
                None => store.items.remove(id),
            };
            changed.insert(id.erase());
        }
    }
    /// Coalesce a later step into this one (interactive drags): the earliest image wins.
    fn absorb(&mut self, later: Self) {
        for (id, image) in later.images { self.images.entry(id).or_insert(image); }
    }
}

pub struct Document { points: Store<Point>, curves: Store<Curve>, constraints: Store<Constraint>,
                      next_id: NonZeroU64, revision: Revision }
#[derive(Default)]
pub struct DocDelta { points: Delta<Point>, curves: Delta<Curve>, constraints: Delta<Constraint> }

/// Maps an entity type to its store and delta; one macro implements it per entity.
pub trait Table<T: Entity> {
    fn store(doc: &mut Document) -> &mut Store<T>;
    fn delta(delta: &mut DocDelta) -> &mut Delta<T>;
}

/// The only way to get `&mut` access to document data.
pub struct Txn<'d> { doc: &'d mut Document, delta: DocDelta }

impl Txn<'_> {
    pub fn modify<T: Entity>(&mut self, id: Id<T>, edit: impl FnOnce(&mut T)) -> Result<(), DocError>
    where Document: Table<T> {
        let item = <Document as Table<T>>::store(self.doc).items.get_mut(&id)
            .ok_or(DocError::Missing(id.erase()))?;
        <Document as Table<T>>::delta(&mut self.delta).images
            .entry(id).or_insert_with(|| Some(item.clone()));  // recorded once per transaction
        edit(item);
        Ok(())
    }
    // insert(): allocate an id, record `None`.  remove(): record `Some(old)`, cascade to referrers.
    /// Validates referential integrity, bumps the revision, returns what changed.
    pub fn commit(self) -> Result<(DocDelta, ChangeSet), DocError> { /* ... */ }
    // Dropped without commit: the delta is swapped back, so the document is untouched.
}

pub struct Step { label: Label, delta: DocDelta, selection: Selection }
pub struct History { undo: Vec<Step>, redo: Vec<Step>, open_drag: bool }

impl History {
    pub fn begin_drag(&mut self, label: Label, selection: Selection) {
        self.undo.push(Step { label, delta: DocDelta::default(), selection });
        self.open_drag = true;
    }
    pub fn record(&mut self, step: Step) {
        self.redo.clear();
        match self.undo.last_mut() {
            Some(top) if self.open_drag => top.delta.absorb(step.delta),  // whole drag = one step
            _ => self.undo.push(step),
        }
    }
    pub fn undo(&mut self, doc: &mut Document) -> Option<ChangeSet> {
        let mut step = self.undo.pop()?;
        let changed = step.delta.swap_all(doc);                           // now holds the redo images
        self.redo.push(step);
        Some(changed)
    }
}

/// Undoable edits as plain data: serialisable for tests, journals, macros and plugins.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Command {
    AddLine { sketch: Id<Sketch>, from: Id<Point>, to: Id<Point> },
    MovePoints { points: Vec<Id<Point>>, by: Vec2 },
    SetDimension { dimension: Id<Dimension>, source: String },
    Delete { targets: Vec<AnyId> },
}

/// What one frame of UI may emit. Drained once: apply, solve in the same Txn, commit, record.
pub enum Intent { Edit(Command), BeginDrag(Label), EndDrag, CancelDrag, Undo, Redo, App(AppAction) }
```

## 3. Parametric expressions

| Crate | Verdict |
|---|---|
| evalexpr 13.1.0 | AGPL-3.0-only: unusable in this MIT project |
| fasteval 0.2.4 (2020), meval 0.2.0 (2018) | dead |
| exmex 0.21.0 (2026-05, MIT/Apache) | maintained; no unit support [R]; fallback |
| rhai 1.27.0, cel 0.16.0 (cel-interpreter stopped at 0.10.0) | whole languages; too much |
| winnow 1.0.5, chumsky 0.14.1, pest 2.9.6, logos 0.16.1, lalrpop 0.23.2 | healthy, optional |
| uom 0.38.0 (2026-02) | compile-time dimensions; cannot type user input [R] |
| petgraph 0.8.3 | fine, not needed for a 20-line topological sort |

Recommendation: a hand-written lexer and Pratt parser in `gc_expr`, about 300 lines with no dependencies. We need byte spans for inline errors, localisable messages and runtime unit checking, which none of the crates give together.

MVP scope:
- **Grammar:** numbers, units `mm cm m in deg rad`, `+ - * / ^`, unary minus, parentheses, parameter names, `sin cos tan asin acos atan sqrt abs min max`, `pi`.
- **Units:** a bare number takes the field's unit; `5 mm + 3 deg` is an error.
- **Parameters:** each dimension owns or references a named parameter (`d0`, `d1`, as in Inventor).
- **Out of scope:** conditionals, strings, user functions.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Dim { pub length: i8, pub angle: i8 }               // runtime dimension exponents
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Quantity { pub si: f64, pub dim: Dim }              // metres, radians

pub enum Expr {
    Number { value: f64, unit: Option<Unit>, span: Span },
    Param { id: ParamId, span: Span },
    Neg(Box<Expr>),
    Binary { op: Op, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    Call { func: Func, args: Vec<Expr>, span: Span },
}

pub struct Param {
    pub name: Name,                                             // validated identifier, e.g. `d0`
    pub source: String,                                         // as typed: "d0 * 2 + 5 mm"
    pub parsed: Result<Expr, ParseError>,                       // an invalid entry stays editable
    pub value: Result<Quantity, EvalError>,
}

/// Kahn's algorithm over sorted maps (deterministic). `deps[p]` holds only ids that exist.
/// Returns the evaluation order plus the parameters on or behind a cycle.
pub fn eval_order(deps: &BTreeMap<ParamId, BTreeSet<ParamId>>) -> (Vec<ParamId>, BTreeSet<ParamId>) {
    let mut pending: BTreeMap<ParamId, usize> = deps.iter().map(|(p, d)| (*p, d.len())).collect();
    let mut users: BTreeMap<ParamId, Vec<ParamId>> = BTreeMap::new();
    for (p, ds) in deps { for d in ds { users.entry(*d).or_default().push(*p); } }
    let mut ready: Vec<ParamId> = pending.iter().filter(|(_, n)| **n == 0).map(|(p, _)| *p).collect();
    let mut order = Vec::with_capacity(deps.len());
    while let Some(p) = ready.pop() {
        order.push(p);
        for user in users.get(&p).into_iter().flatten() {
            if let Some(n) = pending.get_mut(user) {
                *n -= 1;
                if *n == 0 { ready.push(*user); }
            }
        }
    }
    let blocked = pending.into_iter().filter(|(_, n)| *n > 0).map(|(p, _)| p).collect();
    (order, blocked)                                            // blocked: EvalError::Cycle, never a hang
}
```

## 4. Persistence

- **Format:** one pretty-printed JSON document (`.gctrl`) headed by `{"format": "graphctrl", "version": N}`.
  - JSON beats RON 0.12.3 and TOML 1.2.1 on tooling and diffs.
  - SQLite (rusqlite 0.43.0) gives opaque diffs and a C build.
  - When meshes arrive, keep them as files referenced by relative path and add a "pack" export with zip 9.6.0.
- **Determinism:** `BTreeMap` everywhere, integer ids, no timestamps in the body, and serde_json 1.0.154 with its `float_roundtrip` feature [V: feature list].
- **Versioning:**
  - `gc_persist` holds versioned DTOs (`v1::File`, `v2::File`) with `From` migrations chained on load. Domain types do not define the file format.
  - Golden files for every old version stay in the test suite.
  - A file newer than supported is refused with a clear message.
  - Unknown plugin data is preserved as `serde_json::Value`.
- **Load errors:** serde_path_to_error 0.1.20 reports the failing path.
- **Atomic save:** temp file in the same directory, fsync, rename, using tempfile 3.27.0 or atomic-write-file 0.3.1 (atomicwrites last released in 2024). Keep one `.bak`.
- **Autosave and recovery:** debounce after each commit into the state directory with a clean-exit marker, and offer recovery at start-up. Graphite debounces at 1 s [V: consts.rs]; SolveSpace uses an interval in minutes [V: `solvespace/solvespace:src/solvespace.h`].
- **Settings and recent files:** our own versioned file at directories 6.0.0 paths. Use eframe `persistence` only for window geometry and egui memory.

## 5. Cross-cutting

- **Errors:** thiserror 2.0.21 enums per library crate; anyhow 1.0.104 only in `gc_app`. Rerun states the same rule and adds "handle each error exactly once" [V: `rerun-io/rerun:CODE_STYLE.md`].
- **Panics:** denied by lint outside tests, and release keeps `panic = "unwind"`.
  - `catch_unwind` goes around worker jobs, plugin calls and the solver.
  - The panic hook logs, writes the last committed document to the recovery directory and shows a native dialog.
  - Do not wrap egui panels; `Ui` state may be left inconsistent [R].
- **Logging:** `log` 0.4.35 in every crate (egui depends on it [V]). The app installs one sink that fans out to stderr, a file and a ring buffer for the log panel. Warnings and errors also become toasts, as in Rerun [V: CODE_STYLE.md]. tracing 0.1.44 adds little without async.
- **Profiling:** the `profiling` 1.0.18 facade, which egui itself uses [V: deps], with puffin 0.20.0 and puffin_http 0.17.0 behind a feature. puffin_egui 0.30.0 requires egui ^0.33, so use the external viewer.
- **Concurrency:** the UI thread is the only writer.
  - Workers get an immutable snapshot, a cancellation flag and the revision.
  - Results return over `std::sync::mpsc`; stale revisions are dropped.
  - egui_inbox 0.14.0 (^0.37) wakes the UI.
  - rayon 1.12.0 handles data parallelism; tokio 1.54.1 comes later, confined to a robot-communication crate.
- **Numerics:** f64 in the model, f32 only at the GPU boundary. One `Tolerance` type and no literal epsilons. `f64::total_cmp` replaces ordered-float 5.5.1; approx 0.5.1 (2022) in tests only.
- **Units:** hand-rolled `Length` and `Angle` newtypes, SI inside to match URDF and UR interfaces [R]. Revisit uom 0.38.0 when mass, torque and velocity appear.

## 6. i18n, fonts, icons

- **i18n:** Fluent files with i18n-embed 0.16.0 (2025-07, fluent ^0.17) and i18n-embed-fl 0.10.1 (2026-07), hidden behind `gc_i18n::tr!`.
  - `fl!` checks the message id and arguments at compile time against the fallback locale [V: `i18n-embed-fl-0.10.1/README.md`]. Add a test that `ko` has every `en` key.
  - The `desktop-requester` feature detects the locale through sys-locale 0.3.2.
  - Watch fluent-typed 0.10.1 (2026-11): typed functions and cross-locale checks, but about 3,000 downloads per 90 days.
  - Rejected: es-fluent 0.21.2 (MSRV 1.99), rust-i18n 4.2.4 (string keys), gettext-rs 0.8.0 (C library [R]), fluent-templates 0.15.1 (runtime lookups [R]).
- **Korean:** no plural forms; avoid particles directly after a variable. English and Korean share number formatting [R], so icu 2.4.0 can wait.
- **Fonts:** egui 0.37 depends on harfrust for shaping and loads system fonts on demand through `egui_system_font_provider` (fontique), enabled by eframe's default `system_fonts` feature [V: `emilk/egui:CHANGELOG.md`, epaint and eframe metadata].
  - Hangul therefore renders without embedding, and fontdb 0.24.0 or font-kit 0.14.3 are unnecessary.
  - Embed one font anyway, for a consistent look and for industrial PCs without CJK fonts.
- **Font sizes** [V: HTTP content-length]:

| Font file | Size (MB) |
|---|---|
| Pretendard Regular OTF | 1.57 (TTF 2.73, variable 6.74) |
| Wanted Sans OTF | 1.35 |
| NanumGothic | 2.05 |
| IBM Plex Sans KR | 2.80 |
| Noto Sans KR subset OTF | 4.64 (variable 10.4) |
| Spoqa Han Sans Neo subset | 0.51 |

- **Font licences:** Pretendard, Noto Sans KR, IBM Plex Sans KR, NanumGothic and Spoqa are OFL 1.1 with Reserved Font Names [V: licence files]. I could not find Wanted Sans's licence file.
  - Bundling is allowed if the copyright notice and licence travel with the app.
  - A subset is a Modified Version and may not keep the reserved name.
  - So ship unmodified Pretendard Regular and SemiBold, about 3.1 MB if SemiBold is the same size as Regular (only Regular was measured).
  - No crate publishes these fonts [V: crates.io search]; vendor them under `assets/fonts`.
- **Icons:**
  - egui_material_icons 0.9.0 and egui_lucide 0.2.0 support egui ^0.37.
  - egui-phosphor 0.14.0 (^0.36) and egui-remixicon 0.34.0 (^0.34) do not.
  - lucide-icons 1.73.0 has no egui dependency.
  - No general-purpose icon set covers sketch and constraint glyphs [R]. Draw SVGs and load them with egui_extras `svg` (resvg ^0.45.1).

## 7. egui 0.37 widget ecosystem

| Need | Crate, version, date, egui requirement | Verdict |
|---|---|---|
| Docking | egui_tiles 0.18.0, 2026-10-19, ^0.37 | use |
| | egui_dock 0.21.1, 2026-08-06, ^0.36 | wait |
| Tree | egui_ltreeview 0.10.0, 2026-10-25, ^0.37 | use |
| Inspector | egui-probe 0.13.0 (^0.36); egui_struct 0.4.2 (2024); egui_inspect 0.1.3 (2022) | write our own |
| Toasts | egui-notify 0.24.0, 2026-10-30, ^0.37 | use (egui-toast 0.22.0 is ^0.36) |
| File dialog | rfd 0.18.0, 2026-11-07, default `xdg-portal` | use |
| | egui-file-dialog 0.16.0, 2026-11-09, ^0.37 | fallback and UI tests |
| Modal, pan/zoom canvas | built in: `egui-0.37.0/src/containers/modal.rs`, `scene.rs` | use (egui-modal is ^0.30) |
| Command palette | egui-palette 0.7.0, 324 downloads per 90 days | write our own |
| Ribbon | none found | write our own |
| Tables | egui_extras 0.37.0; egui_table 0.11.0 (^0.37) | use |
| Plots, drag-and-drop, markdown | egui_plot 0.38.0, egui_dnd 0.18.0, egui_commonmark 0.26.0 (all ^0.37) | use when needed |
| Gizmo | transform-gizmo-egui 0.11.0, ^0.36 | write our own |
| Node graph | egui-snarl 0.12.0, egui_graphs 0.32.0 (^0.36); egui_node_graph2 0.7.0 (^0.29) | wait, or build on `Scene` |
| Themes | catppuccin-egui 5.7.0 (up to 0.33) | own tokens |
| Log panel | egui_logger 0.13.1, 2026-11-10, ^0.37 | use at first |
| UI tests | egui_kittest 0.37.0 | use |

## 8. Testing, CI, packaging

- **Property tests:** proptest 1.12.0 (quickcheck 1.1.0 and bolero 0.13.7 are the alternatives). Core properties: undo then redo is the identity; save then load is the identity; the solver never worsens the residual.
- **Snapshots:** insta 1.49.0 for file output, draw lists and solver results.
- **UI tests:** egui_kittest drives panels through AccessKit. Image snapshots need its `snapshot` and `wgpu` features. It fails a test when an input widget has no accessible name [V: `egui_kittest-0.37.0/README.md`], so every icon button needs alt text.
- **GPU tests:**
  - egui runs its snapshot tests on a macOS runner [V: rust.yml].
  - Rerun installs lavapipe on all three systems [V: `rerun-io/rerun:scripts/ci/setup_software_rasterizer.py`].
  - Validate WGSL with naga 30.0.2 in ordinary tests; wgpu 30.0.2 also has a `noop` backend feature [V: feature list].
- **Benchmarks:** criterion 0.8.2 (divan 0.1.21 has had no release since 2025-04).
- **Tools:** cargo-nextest 0.9.152, cargo-deny 0.20.2, cargo-shear 1.14.1 or cargo-machete 0.10.1, typos-cli 1.52.2, cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0, cargo-fuzz 0.13.2 (file loader, expression parser), cargo-semver-checks 0.51.0 for `gc_plugin_api`. Miri is installed locally.
- **CI actions:** `dtolnay/rust-toolchain`, `Swatinem/rust-cache@v2`, `EmbarkStudios/cargo-deny-action@v2` [V: egui rust.yml]. wgpu's matrix uses `ubuntu-26.04`, `windows-2025` and `macos-26` [V: `gfx-rs/wgpu:.github/workflows/ci.yml`].
- **Packaging:** cargo-packager 0.11.8 builds `.app`, `.dmg`, NSIS, MSI, `.deb` and AppImage [V: README], though its last release was 2025-11. cargo-dist 0.34.0 (2026-11) is active but aimed at archives and installers, not app bundles [R]. Flatpak later.
- **Windows:**
  - `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`.
  - Icon and version info with winresource 0.1.34.
  - Manifest (DPI awareness, long paths, UTF-8) with embed-manifest 1.5.1 [R for its contents].
  - With no console, file logging and the panic dialog are mandatory.
- **macOS:** bundle with a `.gctrl` document type; signing and notarisation need a Developer ID [R].

## Recommendation

1. Create the workspace as layered in section 1, with that manifest, a pinned 1.97.0 toolchain, and `check-layers` plus cargo-deny wrappers from the first commit.
2. Build `gc_doc` on typed monotonic ids, `BTreeMap` stores and swap-based transactions. Reject slotmap, inverse commands and persistent-map snapshots.
3. Make every UI edit a `Command` drained once per frame, and run the solver inside the transaction.
4. Write `gc_expr` by hand with runtime dimensions. Never depend on evalexpr.
5. Save deterministic, versioned JSON through DTOs, atomically, with debounced autosave and recovery.
6. Use `log` with our own sink, thiserror in libraries, anyhow at the edge, panics denied by lint, and `panic = "unwind"` with an emergency save.
7. Use Fluent with i18n-embed-fl, embed unmodified Pretendard, rely on egui's system-font fallback for the rest, and draw our own SVGs for CAD glyphs.
8. First-milestone UI crates: egui_tiles, egui_ltreeview, egui-notify, rfd, egui_extras (`svg`), egui_kittest. Write the ribbon, property grid, command palette and view gizmo ourselves in `gc_ui`.
9. Keep render preparation in GPU-free `gc_draw`, so most of the viewport is testable on this machine.

## Risks and unknowns

- **No GPU path on this machine.** There is no Vulkan ICD and no `libEGL`; only GLX llvmpipe and Xvfb are present [V local]. wgpu will not find an adapter, so the app and image snapshots cannot run here unless lavapipe (`mesa-vulkan-drivers`) is installed. I did not test whether eframe's `glow` backend works under Xvfb.
- **Tooling.** Only cargo, clippy, rustfmt and miri are installed [V local]; nextest, cargo-deny and the rest need installing.
- **egui churn.** 0.37 is one month old and already deprecates `Id::new` [V: CHANGELOG]. The dock, snarl, gizmo, phosphor and probe crates lag one version. Wrap egui in `gc_ui` and expect breaking upgrades [R].
- **Delta undo.** It is only as safe as the rule that all mutation goes through `Txn`; it needs the property tests above.
- **Slow-moving dependencies.** fluent 0.17.0 (2025-05), i18n-embed 0.16.0 (2025-07), cargo-packager 0.11.8 (2025-11).
- **Not verified.** Zed, Blender and FreeCAD undo internals; the wgpu `noop` API; cargo-dist formats; notarisation steps; wgpu's own rasteriser setup in CI.
- **Open decisions.** The internal length unit (metres proposed), whether `indexing_slicing` is tolerable in the solver, and when to add a packed project format.

## Surprises

- evalexpr, the best-known expression crate, is AGPL-3.0-only.
- egui 0.37 has real text shaping and on-demand system fonts, so Korean works without an embedded font. `default_fonts` is no longer a default egui feature.
- slotmap serialises vacant slots, which makes it a poor fit for diff-friendly files.
- rust-lld is already the default Linux linker; Cranelift is still nightly-only.
- Cargo cannot override workspace lints per crate, and cannot override inherited `default-features` before 1.99.
- Rerun ships release builds with `panic = "abort"` and disables incremental compilation [V: `Cargo.toml`, `.cargo/config.toml`]. We want unwinding, for the emergency save.
- puffin_egui is stuck on egui ^0.33, and catppuccin-egui stops at 0.33.
- No crate ships Pretendard or Noto Sans KR, and OFL reserved names turn subsetting into a renaming exercise.
- egui_kittest fails tests on unnamed input widgets, which makes accessibility a test requirement.
- Rerun, Graphite and Lapce already require Rust 1.98, newer than this environment's 1.97.
