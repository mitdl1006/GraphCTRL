# GraphCTRL frontend stack: evidence report (2026-11-16)

**Legend.** [V] = verified today from a primary source; [S] = seen only in search snippets; [R] = recalled from memory.
Source shorthand: **C** = `https://crates.io/api/v1/crates/<name>` (plus `/<version>/dependencies`); **T** = a file inside `https://static.crates.io/crates/<n>/<n>-<v>.crate`; **G** = `raw.githubusercontent.com`.
Three hosts were unreachable and must be allowed in the session's network settings to be read: `bevy.org`, `www.boringcactus.com`, `v2.tauri.app`. The Bevy 0.20 release notes, the 2025 boringcactus survey and Tauri's Linux graphics page were therefore not read first-hand.

## 1. Current state of the candidates

| Candidate | Latest; last 12 months | Licence | Backing | Facts that matter |
|---|---|---|---|---|
| egui + eframe | 0.37.0 (2026-10-15); breaking minors 0.34 (03-26), 0.35 (06-25), 0.36 (08-05), 0.37, plus 6 patches [V: C] | MIT OR Apache-2.0 | "egui development is sponsored by Rerun" [V: T egui-0.37.0/README.md] | eframe defaults to wgpu; wgpu ^30, winit ^0.30.13, accesskit ^0.25; text = skrifa + harfrust + vello_cpu, no parley [V: C]. 9.97 M downloads per 90 days [V] |
| Bevy | 0.20.1 (2026-10-28); breaking 0.18 (01-13), 0.19 (06-19), 0.20 (10-08) [V] | MIT OR Apache-2.0 | Foundation, many maintainers [R] | `bevy` 0.20.x declares rust-version **1.97.1**; bevy_render → wgpu ^30; bevy_egui 0.43.1 → egui ^0.36 [V: C] |
| iced | 0.14.0 (2025-12-07), the only release in 12 months [V] | MIT | One lead maintainer [R]; libcosmic is not on crates.io [V: C 404] | iced_wgpu → wgpu ^27; master is 0.15.0-dev on git forks of winit and cosmic-text [V: G] |
| Slint | 1.19.0 (today); five semver-compatible minors 1.15–1.19 [V] | GPL-3.0-only OR Royalty-free 2.0 OR commercial [V] | SixtyFPS GmbH [V] | Features `unstable-wgpu-29/30`, `renderer-vello`, `backend-qt`, `accessibility` (default) [V: T Cargo.toml] |
| gpui | **0.2.2, published 2025-10-22**; zero releases in 12 months [V: C] | Apache-2.0 | Zed Industries | gpui-component 0.10.0 (weekly releases) pins `gpui-pre =0.3.12`, a "snapshot of zed@125ca86" owned by gpui-component's author, not Zed [V: C owners] |
| Tauri + web | 2.12.6 (today); 2.9 → 2.12, plus 3.0.0-alpha.0–6 since 09-13 [V] | MIT OR Apache-2.0 | Tauri project | wry 0.57.2 → webkit2gtk =2.0.2 on Linux; `tauri-runtime-cef` 3.0.0-alpha.7 exists (owner tauri-bot) [V] |

All six run on Windows, macOS and Linux [R].
Production apps:
- **egui:** Rerun Viewer [V].
- **Bevy:** Open-RMF rmf_site_editor [V].
- **iced:** COSMIC (via a fork), Halloy, Sniffnet [R].
- **Slint:** embedded HMIs, LibrePCB's new UI [R].
- **gpui:** Zed, Longbridge [R].
- **Web:** Zoo [V]; Onshape, Plasticity, Foxglove [R].

## 2. Scored comparison (1–5)

| Criterion | egui + eframe + wgpu | Bevy + bevy_egui | iced | Slint | gpui + gpui-component | Tauri + TS/three.js |
|---|---|---|---|---|---|---|
| a. 3D viewport | **5** `CallbackTrait`, `register_native_texture`, MSAA/depth options on a shared wgpu 30 device [V: T egui-wgpu-0.37.0/src/renderer.rs] | **5** full renderer on wgpu ^30 [V]; CAD passes are custom | **4** `shader` widget + `shader::Pipeline` [V]; tied to wgpu 27 | **3** `Image::try_from(wgpu::Texture)`, rendering notifier, behind `unstable-wgpu-30` [V] | **2** `paint_surface` is macOS/iOS-only [V]; otherwise CPU upload or forks | **3** three.js/WebGL2 proven; no portable WebGPU; data copied over IPC/WASM |
| b. Shell widgets | **4** egui_tiles, egui_ltreeview, egui_table, file dialog on 0.37; menus, modal, popup in core [V]; no ribbon or property-grid crate | **4** via the egui 0.36 line; Bevy-native UI alone **2** | **3** pane_grid, table, iced_aw; no tree or tabbed docking [R] | **3** TabWidget, TableView, menus; no TreeView or docking [V] | **5** dock, tree, table, virtual_list, command, native_menu, toolbar [V] | **5** dockview, FlexLayout, AG Grid [R] |
| c. Node graph | **4** egui-snarl, egui_graphs, egui_graph, core `Scene` pan/zoom [V]; all three crates still on 0.36 | **4** same crates, already aligned on 0.36 | **3** `canvas`; no crate found [V: C search] | **2** no custom-draw canvas; none found | **3** `paint_path`; none found | **5** React Flow, Rete.js, elkjs [R] |
| d. IME, CJK, a11y, native | **3** IME rework 0.34–0.37, shaping, system-font fallback, AccessKit [V]; five open CJK/IME bugs; no native menu bar | **3** same winit; egui 0.36 lacks the 0.37 fixes | **3** IME since 0.14; no AccessKit dependency [V] | **4** IME, a11y by default, menus, optional Qt backend [V] | **4** own platform layer; accesskit ^0.24 arriving [V]; IME quality [R] | **5** browser-grade [R] |
| e. Stability, licence | **3** four breaking minors a year; funded | **2** three breaking releases a year; MSRV = newest rustc; rmf_site four releases behind [V] | **3** one release a year; 0.15 on git forks [V] | **4** stable 1.x; attribution and API-exposure clauses [V] | **2** official crate stale 13 months [V] | **3** stable 2.x; Zoo and Graphite left Tauri |
| f. Testability | **5** egui_kittest (AccessKit queries, snapshots, video), egui_inspection/egui_mcp [V] | **3** headless ECS tests; weak UI tests [R] | **4** iced_test headless + e2e, time-travel debugging [V] | **3** i-slint-backend-testing [V]; thin docs | **4** `test-support` feature [V]; deterministic scheduler [R] | **4** Playwright/Vitest; no macOS WebDriver for Tauri [R] |
| g. Performance | **4** reactive repaint, "1-2 ms per frame" [V: README] | **2** game loop by default; heaviest build [R] | **4** reactive rendering in 0.14 [V] | **5** compiled DSL [R] | **4** GPU-first [R] | **3** webview RAM, IPC [R] |
| h. UI-agnostic core, plugins | **5** UI is a function of state; a panel is a closure over `&mut Ui` | **3** good `Plugin` model, but the core becomes ECS-shaped | **3** typed `Message` hinders plugin panels | **2** build-time DSL; licence clause | **3** in-process only | **4** IPC enforces separation |
| i. Web build | **5** same code as the Rerun web viewer [V] | **3** heavy [R] | **3** [R] | **3** [R] | **1** | **5** |
| j. Velocity | **4** least boilerplate, largest corpus; pre-0.34 idioms deprecated [V] | **3** | **3** | **4** | **2** | **5** |
| **Total** | **42** | 32 | 33 | 33 | 30 | **42** |
| 100% Rust | yes | yes | yes | Rust + DSL | yes | **no** |

With a Rust/WASM frontend (Leptos, Dioxus) instead of TypeScript, the web column's b, c and j drop to about 3, for a total of about 36.

## 3. Notes behind the scores

- **Rerun's reasons** [V: G rerun ARCHITECTURE.md]: wgpu "lets us write the same code graphics code for native as for web". "The advantage of immediate mode is that is removes all state management. There is no callbacks that are called when some state has already changed". `re_ui` 0.43.2 is on egui ^0.37 and `re_renderer` on wgpu ^30 [V: C].
- **egui limitations** [V: README]: "The main disadvantage of immediate mode is it makes layout more difficult"; "If you want something that doesn't break when you upgrade it, egui isn't for you (yet)." Open issues: #4813 (blurry text in zoomed layers) and #8328 (wasm size 572 KiB → 2.52 MiB). egui_flex 0.9 and egui_taffy 0.16 (both egui ^0.37) soften the layout problem [V].
- **Text stack** [V: G egui CHANGELOG.md]:
  - 0.34 replaced ab_glyph with skrifa + vello_cpu and renamed entry points (`App::update` → `App::ui`).
  - 0.35 added harfrust shaping and `egui_inspection`/egui_mcp.
  - 0.37 added `egui_system_font_provider` (fontique) and colour emoji.
  - Parley was announced as the direction in 0.33 but is still not a dependency.
- **Korean IME** [V: GitHub]:
  - Fixed in 0.34: #7876, #7908, #7809 ("Much improved IME", #7967). 0.37 positions the candidate window (#8681).
  - Still open: #8813 (composition interrupted), #7974 (macOS 2-Set Korean hanja window), #7975 (X11 + Fcitx5), #8802 (CJK fallback glyphs off baseline), #8842 (system fallback misses full-width characters).
  - Below egui: winit PR #4478 "macOS: Fix Korean composing text commit key handling" has been open since 2026-02-07 and is in no release. It covers a doubled space and dropped keys after a committed syllable.
- **Ecosystem lag** [V: C]: 32 days after 0.37, egui_dock, egui-snarl, transform-gizmo-egui, egui_graphs and egui_graph still require ^0.36. After 0.35 and 0.36 they caught up in 2–26 days.
- **Bevy churn** [V: G]: rmf_site `main` pins bevy 0.16, bevy_egui 0.34 and egui_dock 0.16. Bevy's own editor prototype builds from a git fork with `experimental_bevy_feathers`. bevy_feathers has no tree, docking or table [V: T].
- **gpui** [V: T gpui-pre-0.3.13/src/window.rs]: `paint_surface` is `#[cfg(any(target_os = "macos", target_os = "ios"))]` and takes a `CVPixelBuffer`. `gpui_interop` 0.2.1 is "experimental", targets another fork (`bite-gpui`), and lists macOS as "Not yet implemented".
- **Slint** [V: T slint-1.19.0/lib.rs]: `require_wgpu_30`, `WGPUConfiguration::Manual`, `set_rendering_notifier` and `Image::try_from(wgpu::Texture)`.
- **Surveys**: the boringcactus 2025 survey was unreachable. Snippets [S] name Dioxus, Slint, egui, Freya and Xilem as stand-outs; no 2026 edition was found.

## 4. Native Rust versus a web frontend (Tauri) for this app

**Gained with web:**
- Ready-made docking, tree and grid components.
- React Flow plus elkjs for node graphs, and three.js loaders and controls.
- Browser-grade IME, CJK text, accessibility and CSS layout.
- The largest AI and hiring corpus, and a free browser build.

**Lost:**
- **One type system.** Every solver or scene type crosses a serialisation boundary: Tauri IPC, or copies across WASM linear memory. There is no zero-copy sharing of meshes and poses between solver, simulation and renderer.
- **Uniform 3D.** There are three engines to QA. WebGPU exists in Chromium/WebView2, in WKWebView only from macOS 26 [V: gpuweb wiki, edited 2026-10-02], and not in WebKitGTK [S, 2.52 not checked]. A wgpu-in-webview renderer is therefore not portable.
- **Rust plugins.** Panels contributed by plugins become JavaScript.
- **The 100% Rust goal.**

OS-native look is a wash: neither option is native, and CAD apps draw their own chrome anyway.

Rust/WASM frontends keep one language but give up idiomatic React Flow and three.js, and still pay the webview costs. Dioxus desktop is a wry webview; its native Blitz renderer is still 0.3.0-beta.4 [V].

**Verdict: native.** If web were chosen anyway, it should use bundled Chromium, not Tauri 2 on system webviews.

## 5. How comparable tools are built

- **Zoo Design Studio** [V: G package.json]: Electron ^40.8, React 19, three ^0.187, xstate. Tauri → Electron in PR #3315 (merged 2024-08-16). The PR gives no rationale. Neighbouring issue #625 ("RTCPeerConnection error on Linux builds") suggests WebKitGTK, but that is my inference.
- **Graphite** [V: G Cargo.toml]: `cef = "156"`, wgpu 30, winit 0.31 beta, native menus. Its Tauri shell was removed in July 2025 [S].
- **Rerun**: egui + eframe + own wgpu renderer, same code in the browser [V].
- **rmf_site_editor**: Bevy + bevy_egui + egui_dock [V].
- **CADmium**: SvelteKit + three.js + Rust/truck in WASM, Tauri for native, "Early prototype" [V].
- **Recalled [R]**: Onshape (browser WebGL, cloud kernel); Plasticity (Electron); Foxglove (Electron/React/three.js); FreeCAD and RoboDK (Qt); Dune 3D (GTK4); Fornjot (own wgpu viewer); Visual Components (.NET/WPF); Isaac Sim (Omniverse Kit).

Of the apps checked here, none ships 3D on system webviews except the CADmium prototype. Teams bundle Chromium or go native, and the native Rust examples pair egui with a custom or engine renderer.

## 6. Others

- **Makepad**: makepad-widgets 1.0.0 (2025-05-13) is the last crates.io release [V]. Own shader DSL and tiny ecosystem [R].
- **Xilem/Masonry**: 0.4.0 (2025-10-29), no release in 12 months, although parley 0.13 and vello 0.12 move fast [V]. Watch, do not adopt.
- **Floem**: 0.2.0 (2024-11-14), nothing on crates.io for two years [V].
- **Freya**: 0.5.2 (2026-11-11), about 50 releases including pre-releases [V]. Skia-based, single maintainer [R].
- **Vizia**: 0.4.0 (2026-04-23), one release a year [V]. Audio-plugin niche [R].
- **Qt via cxx-qt** 0.10.0 (2026-08-24, KDAB) [V]: the most complete widget set and proven for CAD. The UI is QML/C++, with a Qt toolchain and LGPL/commercial licensing [R]; it breaks 100% Rust.
- **GTK4-rs** 0.11.5 [V]: solid on Linux, second-class on Windows and macOS [R].
- **FLTK-rs** 1.5.23 [V]: small and stable, dated, no route to node graphs [R].

## 7. Steelman of the top three alternatives

1. **Web stack (Electron or Tauri 3 CEF + React + three.js + React Flow).** It ties egui on score and wins b, c, d and j outright. Zoo, Onshape and Plasticity prove it for CAD. It beats egui if 100% Rust is relaxed, if a browser product is first-class, or if the node-graph editor rather than 3D becomes the centre of the product.
2. **Bevy + bevy_egui.** A complete renderer, picking, asset loading and a real plugin model, with the same egui panels. It wins if the product turns simulation-heavy (physics, large textured scenes) and the team accepts three migrations a year.
3. **Slint.** The only Rust toolkit with a stable 1.x API, company support, default accessibility and a Qt backend that sidesteps winit's IME bugs. It wins if the UI is mostly static forms and long-term stability outranks custom canvases and plugin panels.

**Evidence that would change the recommendation:**
- A Korean-IME spike on Windows and macOS with blocking bugs that cannot be fixed with crates.io-only dependencies.
- Third-party egui crates still unported after two release cycles.
- Rerun dropping egui.
- gpui gaining an official release train with a cross-platform external-surface API.
- Bevy shipping a stable editor with non-experimental widgets and a slower breaking cadence.

## 8. For the product owner

We recommend building GraphCTRL's interface natively in Rust with egui, the toolkit behind the Rerun robotics viewer, and drawing the 3D workspace ourselves with wgpu. The whole product stays in one language, so the constraint solver, the simulation and the 3D view share the same data without a translation layer. It is the best Rust fit for a tool that is mostly custom drawing: sketches, 3D scenes and node graphs. The price is that egui changes several times a year and its Korean text input still has rough edges. We will pin versions, bundle a Korean font and test Korean input on every OS from the first milestone. A web interface would offer more ready-made components, but it would split the product into two languages and make 3D behave differently per operating system. The two closest open-source tools both moved off that route.

## Sources

[egui changelog](https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md) · [egui-winit changelog](https://raw.githubusercontent.com/emilk/egui/main/crates/egui-winit/CHANGELOG.md) · [egui IME/CJK issues](https://github.com/emilk/egui/issues?q=is%3Aissue+IME+korean+OR+hangul+OR+CJK+sort%3Aupdated-desc) · [winit PR #4478](https://github.com/rust-windowing/winit/pull/4478) · [winit 0.30 changelog](https://raw.githubusercontent.com/rust-windowing/winit/v0.30.x/src/changelog/v0.30.md) · [Rerun ARCHITECTURE.md](https://raw.githubusercontent.com/rerun-io/rerun/main/ARCHITECTURE.md) · [rmf_site Cargo.toml](https://raw.githubusercontent.com/open-rmf/rmf_site/main/Cargo.toml) · [bevy_editor_prototypes Cargo.toml](https://raw.githubusercontent.com/bevyengine/bevy_editor_prototypes/main/Cargo.toml) · [iced changelog](https://raw.githubusercontent.com/iced-rs/iced/master/CHANGELOG.md) · [Slint royalty-free licence](https://raw.githubusercontent.com/slint-ui/slint/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md) · [Zoo package.json](https://raw.githubusercontent.com/KittyCAD/modeling-app/main/package.json) · [Zoo PR #3315](https://github.com/KittyCAD/modeling-app/pull/3315) · [Graphite Cargo.toml](https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/Cargo.toml) · [Graphite review](https://itsfoss.com/graphite-graphics-editor/) · [CADmium README](https://raw.githubusercontent.com/CADmium-Co/CADmium/main/README.md) · [WebGPU implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status) · [WebKit bug 257694](https://bugs.webkit.org/show_bug.cgi?id=257694) · [gpui_interop docs](https://docs.rs/gpui_interop/0.2.0) · [boringcactus 2025 survey (unreachable)](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html) · crates.io API and crate tarballs as cited inline.

## Recommendation

**Winner: egui 0.37.0 + eframe 0.37.0 (wgpu backend) on wgpu 30.0.x and winit 0.30.13.**

- **Shell.** Use egui_tiles 0.18 for docking, egui_ltreeview 0.10 for the model browser, egui_table 0.11 and egui_extras for property grids, rfd 0.18 for file dialogs, and egui_flex 0.9 for toolbar layout. Build the ribbon, property grid, command palette and ViewCube in-house in a small `ui-kit` crate.
- **Viewport.** Write a renderer crate that depends on wgpu only. Render into an offscreen MSAA + depth + ID-buffer target and show it through `register_native_texture`. Draw dimension and constraint overlays with the egui painter, and edit dimensions in place with a normal `TextEdit`.
- **Node graphs (later).** Build on `egui::Scene` with an own canvas; adopt egui-snarl or egui_graphs once they reach the pinned egui.
- **Start on 0.37, not 0.36.** 0.37 carries the IME candidate-window fix and system-font fallback, and milestone 1 needs neither egui_dock nor egui-snarl.
- **Layering.** `core` (document, solver, commands; no UI) → `render` (wgpu) → `panels` (depend on `egui`, never `eframe`) → `app` (eframe). The plugin API should expose data descriptors and commands, not egui types.
- **Tests.** Use egui_kittest for every panel, and test the solver and commands without UI.

**Runner-up, as the Rust-native plan B: Bevy + bevy_egui.** It is not second on score. I pick it because egui panels and a UI-free core port over, so the fallback is a renderer swap, not a rewrite. Adopt it only if rendering or simulation outgrows a custom renderer.

**Best non-Rust alternative:** a bundled-Chromium web stack, not Tauri 2.

**Not now:** gpui (supply chain), iced (cadence, wgpu 27, no accessibility), Slint (dynamic UI, canvas, plugin licence clause).

## Risks and unknowns

1. **Churn.** egui had four breaking minors and wgpu three majors in about a year. Exact-pin egui, eframe, egui-wgpu and wgpu; upgrade once a quarter after dependants catch up; reach third-party widgets only through `ui-kit`.
2. **Korean IME and fonts.** winit #4478 is unreleased and five egui issues are open.
   - Bundle Pretendard or Noto Sans KR instead of trusting the new system fallback.
   - Use one text-field wrapper and a per-OS IME checklist in milestone 1, Windows first.
   - If macOS blocks, a vendored winit through a `[patch]` path is the only crates.io-compatible fix.
   - Unknown: real behaviour of 0.37 with the Windows Korean IME, which is not testable here.
3. **Renderer effort.** We own lines, picking, highlighting and later large assemblies. Keep milestone 1 to points, lines, arcs, grid, meshes and ID picking.
4. **Build environment** [V, local]: rustc 1.97.0; `/etc/vulkan/icd.d` is empty and libEGL is absent from the linker cache, so wgpu probably finds no adapter here. libGL, Mesa gallium and Xvfb exist. kittest logic tests need no GPU; snapshot and viewport tests need a CI runner with lavapipe. Unknown: whether eframe's glow backend runs under `xvfb-run`.
5. **Plugin ABI.** Exposing `egui::Ui` to out-of-tree plugins would break them at every egui release.
6. **Immediate-mode cost.** Large trees and tables must be virtualised.
7. **Bus factor.** egui depends on Rerun's funding [R]; watch the release cadence.
8. **Not verified.** Binary size and compile times (no builds allowed), WebKitGTK 2.52 WebGPU status, Bevy 0.20 release notes, and Zoo's stated reasons.

## Surprises

1. **gpui's date in the brief is wrong.** 0.2.2 dates from 2025-10-22; 2026-10-30 is the crate record's `updated_at`. The crates.io gpui ecosystem now runs on third-party snapshots and forks (`gpui-pre`, `gpui-fast`, `bite-gpui`).
2. **Bevy 0.20 does not match the toolchain.** `bevy` 0.20.x requires rustc 1.97.1 and the build environment has 1.97.0. Cargo would refuse it or fall back to 0.19.1 [R on resolver behaviour]. Rerun's `re_ui` and `re_renderer` 0.43 require 1.98.
3. **The egui ecosystem is split across 0.36 and 0.37.** A month after 0.37, egui_dock, egui-snarl, the gizmo crate and bevy_egui are still on 0.36, while 0.37 is the release that matters for Korean input.
4. **The macOS Hangul bug lives in winit.** egui, iced, Bevy and Slint's default backend share it; only gpui, Slint's Qt backend and webviews avoid it.
5. **egui is no longer shaping-free.** skrifa, harfrust and fontique arrived in 2026; parley has not.
6. **The closest open-source apps left Tauri.** Zoo went to Electron and Graphite to CEF, and Tauri 3 alpha ships its own CEF runtime.
7. **The web stack ties egui at 42:42.** The 100%-Rust goal and zero-copy data sharing decide, not the score.
8. **Slint's free licence has a plugin problem.** It forbids distributing an application "that exposes the APIs" of Slint, and Slint has no TreeView.
9. **iced is slow on crates.io.** It shipped one release in twelve months and sits three wgpu majors behind; libcosmic is not on crates.io.
10. **egui ships a first-party agent interface.** `egui_inspection` plus egui_mcp let an AI agent drive the app, and kittest can record test videos.
