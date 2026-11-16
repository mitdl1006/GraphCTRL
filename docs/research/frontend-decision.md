# GraphCTRL frontend research: Rust UI framework and 3D viewport architecture (2026-11-16)

Evidence tags: **V** = verified today (source cited), **R** = recalled, not re-verified. Four hosts were unreachable from this session (`webkitgtk.org`, `v2.tauri.app`, `runtimewire.com`, `www.unite.ai`); claims that rest on them are marked "search snippet only". They would need allowlisting to confirm.

## 0. Bottom line

1. **No pure-webview option can be relied on for 60 fps on multi-million-triangle scenes on Linux.** WebKitGTK shows no evidence of shipping WebGPU, and its WebGL acceleration is documented as unreliable. On Windows (WebView2, Chromium) the same options are viable.
2. **The Rust web-UI frameworks all had continuity events in 2025-2026.** Leptos was declared "lightly maintained" (May 2026), the Dioxus team reportedly joined Cognition (Sept 2026), and Yew had an 800-day release gap.
3. **Under the new ranking (3D performance, then continuity, then mainstream), in-process native wgpu with egui wins.** egui is also the most-used Rust GUI crate by crates.io numbers: 10.0M downloads/90 days against 1.9M for Leptos and 1.4M for Dioxus.
4. **Runner-up is Dioxus**, the only web-tech Rust framework with an official in-process wgpu path (Blitz `CustomPaintSource`), but that renderer is self-declared beta.

## 1. Framework status

| | Leptos 0.8 | Dioxus 0.7 | Yew 0.23 | Sycamore 0.9 |
|---|---|---|---|---|
| Latest stable (V, crates.io) | 0.8.23, 2026-10-31 | 0.7.10, 2026-07-30 | 0.23.0, 2026-03-10 | 0.9.4, 2026-10-06 |
| Next (V) | 0.9.0-rc1, 2026-10-31 | 0.8.0-alpha.3, 2026-10-16 | none | 0.10.0-beta.1, 2026-11-09 |
| Reactivity (R) | fine-grained signals (`reactive_graph`, `reactive_stores`), no VDOM | VDOM plus signals/stores, hooks | VDOM, hooks, reducer contexts | fine-grained signals |
| Global/derived/async state (R) | context, `Memo`, `Resource`/`Action`, stores | context, `use_memo`, `use_resource`, global signals | `yewdux` 0.13.0, `use_memo`, `yew-hooks` 0.6.4 | context, memos; thin async story |
| Dev loop (R) | trunk reload; cold builds in minutes | `dx serve` RSX hot reload plus Subsecond hot-patching | trunk reload | trunk reload |
| Where it runs | WASM only | WASM, native-with-webview, or native Blitz | WASM only | WASM only |
| Docs / examples (R) | good book; many pre-0.7 examples now wrong | good; API changed at 0.5, 0.6, 0.7 | large, older, stable corpus | small |

Compile time and WASM size were not measured. WASM size is irrelevant for a desktop bundle.

**Component ecosystem (V, crates.io unless noted):**

| Need | Leptos | Dioxus | Yew | egui (reference) |
|---|---|---|---|---|
| General UI kit | `thaw` 0.4.8 (2025-08-03) requires `leptos ^0.7.7`, so unusable with 0.8; `leptonic` 0.5.0 (2024-02) dead; `leptos-shadcn-ui` 0.9.0 (580 dl/90d); `singlestage` 0.4.1 | `dioxus-primitives` on crates.io is a 0.0.0 placeholder; official components are copied in by `dx` (R), not a crates.io dependency; `dioxus_components` 0.1.2 (community) | `patternfly-yew` 0.8.8 (2026-11-13, active); `yew-bootstrap` 0.14.0 | built-in menus, context menus, windows |
| Utilities | `leptos-use` 0.20.0 (2026-10-20, 534k dl/90d), `floating-ui-leptos` 0.6.0 | `dioxus-sdk` 0.7.0, `dioxus-motion` 0.4.0 | `yew-hooks`, `gloo` 0.13.0 | n/a |
| Virtualised table | `leptos-struct-table` 0.19.1 (2026-09-02) | none found | patternfly table (virtualisation not checked) | `egui_extras` (R) |
| Docking / split | `dockviewers_leptos` 0.2.2 (171 dl/90d) | `dockviewers_dioxus`, `tilers_dioxus` (hundreds of dl) | none | `egui_dock` 0.21.1, `egui_tiles` 0.18.0 |
| Icons | `leptos_icons` 0.7.1 | `dioxus-free-icons` 0.10.0 | `yew_icons` 0.11.0 | n/a |
| Node graph | none (`leptos_flow` does not exist) | none | none | `egui-snarl` 0.12.0, `egui_graphs` 0.32.0, `egui_xyflow` 0.4.3 |

- **Hand-built in every Rust web framework:** ribbon, docking, model-browser tree, property grid, ViewCube, node-graph canvas. Yew additionally gets tree/tabs/modal/toast from patternfly (R for the exact widget list).
- **Sycamore:** 21k dl/90d and 19 reverse dependencies. Not a real candidate.
- **No other serious Rust web-UI contender was found.** There is no maintained xyflow or three.js binding on crates.io. `three-rs` 0.3.0 is a port onto wgpu with 145 dl/90d; `three-d` 0.19.0 is an independent renderer, not a binding.

## 2. Packaging and where the Rust runs

**(a) Tauri 2 + WASM frontend.** UI Rust runs single-threaded as WASM in WebView2 or WebKitGTK; the core runs natively behind commands and channels.
- Binary IPC is current API (V, `tauri-2.12.6/src/ipc/channel.rs`): `Channel<InvokeResponseBody>` with `InvokeResponseBody::Raw(Vec<u8>)`, plus `tauri::ipc::Response::new(bytes)`.
- JSON under 8192 bytes is eval'd directly; larger payloads go through a fetch round trip (`plugin:__TAURI_CHANNEL__|fetch`). A source comment says an 8 kB JSON runs about 2x faster by eval than by fetch on WebView2 v135.
- I did not measure throughput. Per-message cost looks fine for 60 Hz small patches; hundreds of MB of mesh data means one-off copies.

**(b) Dioxus desktop.** Components run natively in the host process, so core crates and threads are called directly with no IPC or DTO layer.
- DOM edits are batched and sent over a local WebSocket (V, `dioxus-desktop-0.7.10/src/edits.rs`); events return over wry IPC.
- Each pointer move therefore costs two process hops before pixels change. That is fine for panels and jitter-prone for per-frame overlays.
- It needs webkit2gtk to compile on Linux.

**(c) Dioxus native (Blitz).** No webview: Blitz does HTML/CSS layout and paints with Vello on wgpu (V, `dioxus-native` 0.7.10 depends on `blitz-*` 0.2, `anyrender_vello`, winit 0.30).
- It hosts custom wgpu rendering. The official example `examples/10-integrations/wgpu-texture` (v0.7.10) uses `dioxus_native::use_wgpu`, `CustomPaintSource` and `canvas { "src": paint_source_id }`, with HTML layered above and below the texture (V).
- Status (V, Blitz README): "currently in a **beta** state ... still many bugs and missing features". `blitz-dom` last stable is 0.2.4 (2025-10-22); 0.3.0-beta.4 is dated 2026-10-16. IME support was not verified.

**(d) axum + WebSocket + browser.** A good dev and test harness for the WASM options, and it runs in our sandbox with Playwright. It needs a transport trait, because Tauri IPC is not WebSocket.

## 3. 3D viewport: ceilings and architectures

**Rendering ceilings (performance is weighted first):**

| Path | Windows | Linux | Ceiling and reason |
|---|---|---|---|
| Native wgpu in-process (DX12 / Vulkan) | yes | yes | GPU-bound. 64-bit memory, mmap loading, threaded mesh prep, indirect draws and GPU culling available. Millions of static triangles at 60 fps is routine (R). |
| wgpu-WASM on WebGPU | WebView2: yes (R) | WebKitGTK: no evidence of support | Near-native GPU throughput for static meshes. CPU side is single-threaded and wasm32 is capped at 4 GB; mesh bytes sit in WASM memory and again on the GPU. |
| wgpu-WASM on WebGL2 | yes | only path available | wgpu 30.0.2 keeps the `webgl` feature, with runtime fallback via `new_instance_with_webgpu_detection` (V, wgpu `Cargo.toml` and `src/lib.rs`). Downlevel limits apply: no compute, no indirect multi-draw (R). |
| three.js on the same webviews | yes | same problem | Same ceilings, and not Rust. |

**Linux evidence:**
- WebGPU: a 2023 WebKitGTK maintainer statement says it is not supported and nobody is working on it; searches today found nothing shipping in 2.50 or 2.52 (search snippets only).
- Tauri's Linux graphics page says WebGL/canvas "can silently land on a slow path", recommends a non-WebGL fallback, and notes WebKitGTK masks the renderer string so the app cannot detect it (search snippet only).
- The NVIDIA workaround `WEBKIT_DISABLE_DMABUF_RENDERER=1` trades speed for stability (search snippet only).
- Tauri 3 alpha has a CEF runtime (V: `tauri-runtime-cef` 3.0.0-alpha.7, `cef` 156.6.0), which would remove the WebKitGTK dependency. It is alpha, so not a basis today.

**Getting meshes in:** native reads files directly. For a webview, send bytes by raw IPC (copy plus fetch) or load inside WASM through a custom protocol (single-threaded parse, double memory).

**Canvas access for the WASM path (R, equivalent across frameworks):**
- Element handle: Leptos `NodeRef<html::Canvas>`, Dioxus-web `onmounted` then downcast to `web_sys::Element`, Yew `NodeRef::cast`, Sycamore node ref.
- Everything else is raw `web_sys`: `requestAnimationFrame`, `setPointerCapture`, a non-passive `wheel` listener, `pointerdown` with `button == 1` plus `preventDefault` to stop autoscroll, `KeyboardEvent.code`, and `ResizeObserver` with `devicePixelRatio`.
- This is not a differentiator. Dioxus desktop cannot do it, because its code is not in the webview.

**Hybrids: native wgpu viewport with web-tech UI around it:**

| Hybrid | Evidence | Verdict |
|---|---|---|
| 1. Tauri 2 window + wgpu surface + transparent or child webview | `Window` and `WebviewWindow` implement `HasWindowHandle` (V). The wry `examples/wgpu.rs` uses a transparent child webview and contains `panic!("This example doesn't support wayland!")` (V). Child webviews are "macOS, Windows and Linux (X11 Only)" (V, wry `lib.rs`). `Window::add_child` is behind the `unstable` feature (V). On Linux Tauri builds the webview as a GTK widget (V, `tauri-runtime-wry` 2.12.4). The community demo `FabianLars/tauri-v2-wgpu` has 2 commits and was archived 2024-12. | Fragile. Works on Windows and X11; nothing verified works on Wayland. Either the webview takes all viewport input and forwards it over IPC, or DOM popups cannot overlap the viewport. Not production-proven. |
| 2. Dioxus desktop `with_as_child_window()` + wgpu | Official example `examples/08-apis/wgpu_child_window.rs`, "borrowed from wry's example"; uses `build_as_child` (V). | Same X11-only limit and same z-order and input problems. Fragile. |
| 3. Dioxus native (Blitz) + `CustomPaintSource` | Official example (V). One process, one wgpu device, correct z-order above and below. | Architecturally the cleanest, but beta, with IME and CSS coverage unverified. Not production-proven. |
| 4. Native Rust GUI (egui + wgpu) | The Rerun viewer ships large 3D scenes on egui + wgpu (R). egui is sponsored by Rerun per its repo README (search snippet). | The only production-proven option here. |

iced has had one release in 24 months. gpui has no wgpu embedding path that I know of (R), and its crates.io releases lag (0.2.2, 2025-10-22).

**Streaming native frames into the webview:** reject. 1080p60 RGBA is about 500 MB/s uncompressed, and encoding adds latency to orbit.

**Recommended viewport per framework:**
- Leptos, Yew, Sycamore, Dioxus-web in Tauri: our own wgpu renderer compiled to WASM, WebGPU first with WebGL2 fallback.
- Dioxus desktop webview: none is robust (TS/three.js module, or the X11-only child window).
- Dioxus native: `CustomPaintSource`.
- egui: our renderer drawing to an offscreen MSAA + depth texture on egui's own wgpu device.
- Most robust combination: egui + wgpu.

## 4. Core crates in WASM versus native

- **parry3d-f64 0.31.1:** upstream CI builds parry2d and parry3d for `wasm32-unknown-unknown` (V, `dimforge/parry/.github/workflows/main.yml`). The f64 variant is the same source but is not explicitly in that job. `parallel` (rayon) is optional.
- **parry no longer uses nalgebra:** 0.31 depends on `glamx` (V, manifest). Check the core crates' math types against this.
- **What breaks in wasm32:**
  - `std::thread` and rayon: `wasm-bindgen-rayon` 1.3.0 (last release 2024-12) needs nightly, SharedArrayBuffer and COOP/COEP headers (R).
  - `std::time::Instant`: use `web-time` 1.1.0.
  - `getrandom` 0.4.3: needs the `wasm_js` feature (V, README).
  - File IO.
- **If a webview UI is chosen:** keep the document backend-authoritative. The native session crate owns document and undo; the frontend sends intents and receives versioned patches over a `Channel`. For latency-free dragging, run a transient solver and sketch copy in WASM during the drag and commit the result as one intent on pointer-up. Collision, planners and IO stay native.
- **Cost of that split:** a serde DTO layer, two build targets, and a second solver instance that must not diverge.
- **Native UI (egui or Dioxus native/desktop) removes the problem.** There is one process and one document, and planners run on threads behind channels.

## 5. Korean IME and fonts

- **WebView2:** Chromium IME behaviour with inline composition (R). No Tauri-specific Hangul issue was found.
- **WebKitGTK via wry:** preedit is disabled unconditionally. `wry-0.57.2/src/webkitgtk/mod.rs` lines 429-431 call `input_context.set_enable_preedit(false)` (V). Hangul therefore composes in the IME's own popup, not inline. A PR proposing an opt-in (wry #1724) is not in the 0.57.2 changelog (V).
- **Fonts in a webview:** a bundled TTF via `@font-face` is standard (R).
- **egui:** bundled TTF through `FontDefinitions` (R). IME arrives through winit and has had rough edges historically. I found no current evidence either way, so this needs a spike on Windows, X11 and Wayland.
- **Blitz:** IME unknown.

## 6. Tooling in our sandbox

| Tool | Version (V) | Sandbox notes |
|---|---|---|
| Rust targets | only `x86_64-unknown-linux-gnu` installed (V, `rustup target list`) | `rustup target add wasm32-unknown-unknown` is needed for any WASM option. |
| trunk | 0.22.1, 2026-10-26 | Downloads wasm-bindgen (default 0.2.128) and binaryen `version_131` from GitHub releases unless a matching binary is on PATH; has an offline mode (V, `src/tools.rs`). Pre-install `wasm-bindgen-cli` 0.2.130 from crates.io and disable wasm-opt. |
| dioxus-cli | 0.7.10 | Has a `no-downloads` feature and `NO_DOWNLOADS` env. Its wasm-bindgen-cli fallbacks include `cargo binstall`, then `cargo install` from source. Tailwind, NSIS, WiX and linuxdeploy come from GitHub (V, source grep). wasm-opt handling not verified. |
| tauri-cli | 2.12.5, 2026-11-16 | Needs `libwebkit2gtk-4.1-dev` (V, wry README). The sandbox is Ubuntu 24.04.5 with no webkit packages (V, dpkg). The package exists in noble (R); apt lists were absent, so not verified here. |
| wasm-bindgen-test | 0.3.80 | Needs a webdriver, which I did not check. Prefer Playwright end-to-end tests. |

- **Not verifiable here:** the Tauri window, Dioxus desktop, WebKitGTK behaviour, real GPU performance, and IME.
- **Verifiable here:** WASM frontends in Chromium via Playwright (SwiftShader, so correctness only), all headless core tests, and egui UI tests via `egui_kittest` (R).
- **GPU tests:** wgpu has a `noop` backend feature (V). A software Vulkan adapter for golden images was not checked.
- **Component test stories (R):** `dioxus-ssr` 0.7.10 string-render tests; Leptos and Yew via wasm-bindgen-test.

## 7. Node-graph editor

- No Rust web framework has a node-graph crate, and there is no xyflow binding.
- Hand-building an infinite canvas (CSS-transform pan/zoom, HTML nodes, SVG edges, marquee, drag and drop, minimap) is roughly 3-5 weeks in Leptos or Dioxus, slightly more in Yew. That is an estimate, not a measurement.
- Auto-layout is hand-built or from a layout crate in every case.
- egui starts from `egui-snarl`, `egui_graphs` or `egui_xyflow`; HFSM nesting and behaviour-tree layout remain custom.

## 8. Maintenance continuity

Release counts are stable releases from 2024-11-16 to today, computed from the crates.io API (V). "Longest gap" includes the gap to today. Counts follow the API as returned.

| Crate | Maintainer / funding | Stable releases | Longest gap (days) | Last stable | Breaking lines in 24 months | Reverse deps | 90-day downloads | 2025-26 signals |
|---|---|---|---|---|---|---|---|---|
| tauri | Tauri Programme within The Commons Conservancy (V, licence headers); CrabNebula backing (R) | 48 | 80 | 2.12.6, 2026-11-16 | none within 2.x | 809 | 18.8M | 3.0 alphas since 2026-09: a major migration is coming |
| wry | same | 35 | 86 | 0.57.2, 2026-11-10 | 12 (0.47 to 0.57) | 139 | 20.0M | hidden behind Tauri |
| leptos | one lead (gbj), hobby project, no company | 31 | 93 | 0.8.23, 2026-10-31 | 0.7, 0.8 (0.9 rc) | 634 | 1.9M | Issue #4707, 2026-05-08 (V): "not abandoned but will be lightly maintained going forward", "feature-complete", seeking maintainers; `thaw` stalled |
| dioxus | Dioxus Labs (VC-funded); team reportedly joined Cognition 2026-09-10 with a stated commitment to keep supporting Dioxus, Blitz, Taffy, Subsecond (search snippets only) | 15 | 265 | 0.7.10, 2026-07-30 | 0.6, 0.7 (0.8 alpha) | 482 | 1.4M | sponsor's product is not Dioxus; alphas continued after the move |
| blitz-dom | same team | 10 | 389 | 0.2.4, 2025-10-22 | 0.1, 0.2 | 30 | 102k | self-declared beta |
| yew | volunteers | 2 | 800 | 0.23.0, 2026-03-10 | 0.22, 0.23 | 333 | 454k | revived after a two-year stall |
| sycamore | effectively one maintainer (R) | 4 | 341 | 0.9.4, 2026-10-06 | 0.9 | 19 | 21k | tiny ecosystem |
| egui / eframe | Emil Ernerfeldt; sponsored by Rerun, whose product is built on it (R) | 19 | 127 | 0.37.0, 2026-10-15 | 8 (0.30 to 0.37) | 1,424 | 10.0M | steady cadence; no 2026 funding statement found |
| iced | one lead; COSMIC depends on a fork (R) | 1 | 444 | 0.14.0, 2025-12-07 | 0.14 | 433 | 827k | very slow releases |
| slint | SixtyFPS GmbH (R) | 20 | 104 | 1.19.0, 2026-11-16 | none (1.x) | 130 | 1.07M | best stability policy; GPL / royalty-free / commercial licence (R) |
| bevy | Bevy Foundation (R) | 16 | 122 | 0.20.1, 2026-10-28 | 6 | 2,215 | 2.1M | game engine, not a frontend crate |
| gpui | Zed Industries | 3 | 389 | 0.2.2, 2025-10-22 | 0.2 | 155 | 292k | crates.io lags the Zed repo |
| wgpu | gfx-rs; "the core of the WebGPU integration in Firefox, Servo, and Deno" (V, `lib.rs`) | 20 | 90 | 30.0.2, 2026-11-12 | 8 majors (23 to 30) | 2,115 | 19.4M | quarterly breaking releases, strongest continuity |
| winit | rust-windowing volunteers | 7 | 258 | 0.30.13, 2026-03-02 | 0.30 only | 1,660 | 17.0M | 0.31 betas, critical infrastructure |

**Lock-in (replacement cost if a dependency dies):**
- **Renderer:** written against wgpu in its own crate, taking a `Device`, `Queue` and target `TextureView`. It is reusable under egui, Blitz, a WASM canvas or a bare winit window, so the only lock-in is to wgpu, the most durable dependency here.
- **UI view layer:** fully locked to its framework in every option. Keep it thin by leaving intents and view-models in the existing headless session crate.
- **Webview options:** additionally lock in an IPC/DTO layer and a Tauri major-version migration.
- **egui:** costs about three small migrations a year.

## 9. Scored comparison (1-5)

Rows are in the new priority order. A seventh column covers Dioxus native, since it is the runner-up.

| Criterion | Leptos + Tauri | Dioxus desktop (webview) | Dioxus-web in Tauri | Yew + Tauri | Sycamore + Tauri | Dioxus native (Blitz) | egui native |
|---|---|---|---|---|---|---|---|
| **Large-scene 3D performance** | 2: fine on WebView2; WebKitGTK is WebGL2-only and unreliable | 2: no robust viewport; native child is X11-only | 2: same as Leptos | 2 | 2 | 4: in-process wgpu; beta, extra composite | 5: in-process DX12/Vulkan |
| **Maintenance continuity** | 2: Tauri strong, Leptos "lightly maintained" | 3: team moved under Cognition | 3 | 2: 800-day gap | 1: 19 reverse deps | 2: beta, 389-day stable gap | 4: funded lead, 19 releases, 1,424 reverse deps |
| Mainstream and suitable | 3: 1.9M dl | 3: 1.4M dl | 3 | 2: 0.45M dl | 1 | 2: 21k dl | 5: 10.0M dl |
| 3D viewport robustness | 3: two GPU backends to test | 2 | 3 | 3 | 3 | 3: official example only | 5: shared device, proven |
| Ready-made shell widgets | 2: `thaw` stuck on 0.7 | 2: no crates.io kit | 2 | 3: patternfly-yew | 1 | 2: CSS coverage unknown | 4: dock, tiles, tables, menus |
| Node-graph feasibility | 3: hand-build | 3 | 3 | 3 | 2 | 2 | 4: three existing crates |
| Korean IME / text | 4: WebView2 good; Linux has no inline preedit | 4 | 4 | 4 | 4 | 2: unverified | 3: unverified, needs a spike |
| API stability | 3: 0.9 pending | 3: 0.8 pending | 3 | 3 | 2 | 2 | 3: 8 breaking minors |
| Testability in our sandbox | 4: Playwright + Chromium; no Tauri window | 2: needs webkit2gtk | 4 | 4 | 4 | 3: needs software GPU, unverified | 4: plain `cargo test` |
| Drag-solve performance | 4 with solver in WASM, 3 over IPC | 3: two hops per frame | 4 / 3 | 4 / 3 | 4 / 3 | 5 | 5: same process, same frame |
| Core plugs in cleanly | 3: DTO + IPC + two targets | 5: direct calls | 3 | 3 | 3 | 5 | 5 |
| AI-assisted velocity | 3: 0.7 rewrite invalidated old examples | 3: API churn | 3 | 4: stable, older corpus | 2 | 2 | 4: very large corpus |

## Recommendation

**Winner: native egui + wgpu (the original plan, now backed by evidence).** It is the only option that is production-proven for large 3D scenes on both Windows and Linux. It also has the best continuity among UI candidates that can host a wgpu viewport, and by the numbers it is the mainstream Rust GUI crate. The owner's premise that egui is less mainstream than the web frameworks is contradicted by the crates.io data.

Concrete architecture:
- **Shell:** winit + eframe on the wgpu backend, or egui-winit + egui-wgpu directly if more frame control is needed.
- **Layout and widgets:** `egui_tiles` or `egui_dock` for docking and `egui_extras` tables. Ribbon, model-browser tree, property grid, status bar and ViewCube are custom widgets.
- **Viewport:** a separate renderer crate with no egui dependency, drawing to an offscreen MSAA + depth target on egui's device. Picking uses an ID buffer or a parry BVH. Loaded meshes get static index/vertex buffers, with frustum culling and optional LOD.
- **Core:** in-process. The session crate is the single source of truth; planners run on worker threads behind channels. No state sync is needed.
- **Build:** plain cargo, no JS toolchain.
- **Tests:** headless session tests, `egui_kittest` UI tests, renderer golden images on a software adapter, and manual GPU/IME smoke tests on Windows and Linux.
- **Node graph:** evaluate `egui-snarl` before hand-building.

**Runner-up: Dioxus, on the same renderer crate.** It is the only web-tech Rust framework with an in-process wgpu path, native access to the core, and a full-time team. Adopt it only after a spike on Blitz passes: Korean IME, a 5M-triangle scene, and docking. Dioxus-web in Tauri is its fallback, with Windows-grade performance only.

**Not recommended while the Linux large-scene requirement stands:** Leptos, Yew or Sycamore in Tauri, and any transparent-webview-over-wgpu hybrid.

## Risks and unknowns

- **egui Korean IME** on Windows, X11 and Wayland is unverified. Spike it first; it is the main risk to the winner.
- **egui bus factor:** one lead maintainer, 0.x versioning, about three breaking releases a year. The mitigant is 1,424 reverse dependencies and a commercial sponsor.
- **Hand-built CAD widgets** (ribbon, property grid, tree) are needed in every option, egui included.
- **Software-GPU availability** in the sandbox for golden-image tests was not checked.
- **The Dioxus/Cognition move** rests on secondary coverage via search snippets; I could not fetch the primary announcement.
- **WebKitGTK WebGPU absence** is negative evidence from searches. `webkitgtk.org` and `v2.tauri.app` were unreachable.
- **No performance numbers were measured.** Ceilings are architectural reasoning plus recall.

**What would change the recommendation:**
- Tauri 3 stable with the CEF runtime and WebGPU on Linux would make Dioxus-web or Leptos in Tauri viable.
- A stable Blitz with IME would promote Dioxus native to winner.
- A failed egui IME spike would force an evaluation of Slint (1.x semver, company-backed; its wgpu embedding is R only) or Dioxus.
- Measured 60 fps at target scene size on WebKitGTK across Intel, AMD and NVIDIA would reopen the webview options.
- Rerun dropping egui would lower its continuity score.

## Surprises

- Leptos' maintainer declared it "lightly maintained" and "feature-complete" in May 2026 and treats it as a hobby project. Its main UI kit, `thaw`, never moved past Leptos 0.7.
- The Dioxus team reportedly moved to Cognition in September 2026. Dioxus has had no stable release since 2026-07-30.
- Yew went 800 days without a stable release before 0.22.
- `dioxus-primitives` on crates.io is an empty 0.0.0 placeholder.
- wry hard-disables IME preedit on Linux.
- wry's own wgpu example panics on Wayland, and Tauri's child-webview API is still behind `unstable`.
- parry 0.31 dropped nalgebra for `glamx`.
- Tauri 3 alpha ships a CEF (bundled Chromium) runtime.

## Sources

Crate files (crates.io tarballs): `wry-0.57.2` (`src/lib.rs`, `examples/wgpu.rs`, `examples/gtk_opengl.rs`, `src/webkitgtk/mod.rs`, `CHANGELOG.md`, `README.md`); `tauri-2.12.6` (`src/ipc/channel.rs`, `src/window/mod.rs`, `src/webview/webview_window.rs`, `Cargo.toml`); `tauri-runtime-wry-2.12.4/src/lib.rs`; `wgpu-30.0.2` (`Cargo.toml`, `src/lib.rs`); `parry3d-f64-0.31.1/Cargo.toml`; `dioxus-desktop-0.7.10` (`src/edits.rs`, `src/config.rs`, `src/webview.rs`); `dioxus-native-0.7.10/Cargo.toml`; `dioxus-cli-0.7.10/src`; `trunk-0.22.1/src/tools.rs`; `getrandom-0.4.3/README.md`. Version, download and reverse-dependency figures are from `https://crates.io/api/v1/crates/<name>`.

- [Dioxus wgpu-texture example](https://raw.githubusercontent.com/DioxusLabs/dioxus/v0.7.10/examples/10-integrations/wgpu-texture/src/main.rs)
- [Dioxus wgpu_child_window example](https://raw.githubusercontent.com/DioxusLabs/dioxus/v0.7.10/examples/08-apis/wgpu_child_window.rs)
- [Blitz README](https://raw.githubusercontent.com/DioxusLabs/blitz/main/README.md)
- [parry CI workflow](https://raw.githubusercontent.com/dimforge/parry/master/.github/workflows/main.yml)
- [Leptos status update, issue #4707](https://github.com/leptos-rs/leptos/issues/4707)
- [FabianLars/tauri-v2-wgpu](https://github.com/FabianLars/tauri-v2-wgpu)
- [Tauri Linux graphics issues](https://v2.tauri.app/develop/debug/linux-graphics/) (search snippet only)
- [WebKitGTK 2.52 highlights](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html) (search snippet only)
- [webkit-gtk list: "No flag for WebGPU"](https://lists.webkit.org/pipermail/webkit-gtk/2023-October/003945.html) (search snippet only)
- [Cognition / Dioxus coverage](https://runtimewire.com/article/cognition-dioxus-devin-runtime-jonathan-kelley) and [second report](https://www.beri.net/article/cognition-dioxus-acquihire-blitz-taffy-crate-owners-kitesurf-dependency) (search snippets only)
- [Yew 0.22 release post](https://yew-rs--pr4069-yew-docs-185gwe7t.web.app/blog/2025/11/29/release-0-22) (search snippet only)
- [egui repository](https://github.com/emilk/egui) (search snippet only)
