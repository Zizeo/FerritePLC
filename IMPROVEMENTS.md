# FerritePLC — Project Improvement Roadmap & Technical Audit

**What this repo does:** FerritePLC is a deterministic IEC 61131-3 / IEC 60848 PLC runtime, compiler, simulator, and studio IDE written in Rust (2024 Edition) supporting Ladder Diagram (LD), Sequential Function Chart / Grafcet (SFC), Structured Text (ST), and Function Block Diagram (FBD), alongside Modbus TCP/RTU, MQTT Sparkplug B, OPC UA, Linux GPIO HAL, and a 2D Digital Twin.
**Assumed load:** Continuous real-time PLC scan loops (1–50 ms cycle times) running concurrently with the Iced 60 FPS Studio GUI or TUI simulator and background fieldbus servers (up to 64 Modbus TCP clients, MQTT/OPC UA telemetry).

---

## Must Fix (Real-Time Determinism, Reliability & Fault Visibility)

1. **Eliminate remaining heap allocations in the PLC scan hot path** (`crates/ferrite-runtime/src/lib.rs:L615-616,660`, `crates/ferrite-ir/src/lib.rs:L1647-1660,1689,1784`, `crates/ferrite-lang-sfc/src/lib.rs:L1636-1643`, `crates/ferrite-lang-ld/src/lib.rs:L715-722`, `crates/ferrite-lang-fbd/src/lib.rs:L534-543,722,767`)
   - **What this is:** Phase 2 (`Task & POU Execution`) of `PlcRuntime::step_scan` executes every compiled LD, SFC, ST, and FBD program on each scan tick.
   - **Problem:** Violating `AGENTS.md §5.1` (zero heap allocations in the scan hot path), `step_scan` allocates `executed_pous: Vec<PouId>` and `scan_task_violations: Vec<_>` every tick; `IrStmt::execute` allocates a fresh `HashMap` every tick; `CompiledSfcPou`, `CompiledLdPou`, and `CompiledFbdPou` resolve symbols by string name every scan instead of pre-resolved `TagId`s; and `CompiledFbdPou` clones `output_pins` (`L722`) or constructs a throwaway `FbInstance` (`L767`) on composite blocks.
   - **Fix:** Pre-allocate `executed_pous` and `scan_task_violations` on `PlcRuntime` (calling `.clear()` each scan), store a reusable `env: HashMap<String, IecValue>` on compiled ST POUs, resolve `TagId`s and `FbId`s during `compile()` across LD/SFC/FBD/IR, and add a counting-`GlobalAlloc` test asserting 0 allocations during `step_scan()`.
   - **If we skip it:** Allocator contention and heap fragmentation cause scan-time jitter during high-frequency (1–5 ms) control loops.

2. **Add an address-to-`TagId` lookup table for direct-address writes** (`crates/ferrite-core/src/context.rs:L214-230`, `crates/ferrite-core/src/tag_table.rs:L119-160`)
   - **What this is:** `ScanContext::write_symbol` updates both the `ProcessImage` and any `TagEntry` bound to a direct hardware/marker address (`%Q`, `%M`).
   - **Problem:** Writing to a direct address (e.g. `%MW10` or `%QX0.1`) linearly scans every tag in `TagTable` (`for idx in 0..count`) on every write (`O(N)` per write).
   - **Fix:** Maintain a `HashMap<DirectAddress, SmallVec<[TagId; 2]>>` (or `Vec<TagId>`) inside `TagTable` when tags are registered so direct-address writes sync bound tags in `O(1)`.
   - **If we skip it:** Projects with ratusan/hundreds of tags pay `O(tags × direct_writes)` on every scan cycle.

3. **Bound `PlcRuntime::task_deadline_violations` with a ring buffer** (`crates/ferrite-runtime/src/lib.rs:L234,668`)
   - **What this is:** Records `(task_name, scan_count, overrun_us)` whenever a scheduled task exceeds its `watchdog_us` budget in warning mode (`strict_deadline_mode == false`).
   - **Problem:** `self.task_deadline_violations.push(violation)` appends to an unbounded `Vec`, unlike `alarm_journal` and `trace_buffer` which use bounded `VecDeque` ring buffers.
   - **Fix:** Change `task_deadline_violations` to a `VecDeque<(String, u64, u64)>` capped at a fixed capacity (e.g. `1,000` entries, popping the oldest entry when full).
   - **If we skip it:** A task that repeatedly overruns its deadline in non-strict mode slowly leaks memory until the process crashes.

4. **Surface runtime scan errors in the TUI simulator (`ferrite sim`)** (`crates/ferrite-cli/src/main.rs:L656,692`)
   - **What this is:** The interactive terminal simulator (`run_tui_sim` / `tui_loop`) steps the PLC runtime on `[s]` or on each periodic tick when `running` is true.
   - **Problem:** Both call sites do `let _ = runtime.step_scan();`, silently discarding `RuntimeError`s (such as `DivisionByZero`, `WatchdogTimeout`, or `IterationLimitExceeded`). The header continues displaying `State: RUNNING` while scan execution is aborting.
   - **Fix:** Capture `Err(e)` from `runtime.step_scan()`, set `running = false`, store `last_error: Option<String>`, and render a red `FAULT: <error>` banner in the TUI telemetry header.
   - **If we skip it:** Users debugging POUs in the TUI get no indication why outputs stopped updating after a runtime fault.

5. **Evict idle connections in `ModbusTcpServer`** (`crates/ferrite-fieldbus-modbus/src/lib.rs:L804-807,839-860`)
   - **What this is:** Background TCP server handling up to `MAX_CLIENTS = 64` concurrent Modbus TCP connections.
   - **Problem:** `ConnectedClient` has no `last_active: Instant` timestamp. If 64 clients connect and go silent (or drop without a clean TCP FIN), `clients.len() < MAX_CLIENTS` stays false forever and all new Modbus clients are silently ignored.
   - **Fix:** Track `last_active: Instant` in `ConnectedClient`, update it on every read, and disconnect clients idle for more than a configurable timeout (e.g. `Duration::from_secs(60)`).
   - **If we skip it:** Stale or half-open SCADA connections can lock out new Modbus TCP clients until the server is restarted.

---

## Should Fix (Language Completeness, GUI/UX Parity & Codebase Lean)

6. **Execute internal block networks for graphical Composite Function Blocks** (`crates/ferrite-lang-fbd/src/lib.rs:L96-106,459-478,764-783`)
   - **What this is:** User-defined composite function blocks (`FbdBlockKind::Composite`) in the FBD language crate.
   - **Problem:** When a `CompositeFbDef` does not define inline `st_code`, `CompiledFbdPou::execute` ignores `CompositeFbDef.blocks` and falls back to reducing all boolean inputs with `AND` (`L769-781`).
   - **Fix:** Compile `CompositeFbDef.blocks` recursively into a sub-`CompiledFbdPou` during `FbdProgram::compile` and execute that sub-network when `compiled_st_stmts` is `None`.
   - **If we skip it:** Composite FBD blocks built graphically without ST code do not execute their internal blocks.

7. **Persist SFC canvas pan/zoom per POU and add scroll-wheel zoom** (`crates/ferrite-gui/src/canvas_sfc.rs:L149-151,884-917,3955-3972`, `crates/ferrite-gui/src/app.rs:L549,8041-8048`)
   - **What this is:** Viewport navigation on the SFC (Grafcet) 2D editor canvas.
   - **Problem:** Ladder Diagram (`canvas_ld.rs`) persists per-POU `pan` and `zoom` in `FerriteStudioApp` (`ld_canvas_pan`, `ld_canvas_zoom`) and supports cursor-centered mouse-wheel zoom (`WheelScrolled`). SFC currently stores `pan` only in widget-local `SfcCanvasState` (which resets when switching tabs) and has no scroll-wheel zoom.
   - **Fix:** Store `sfc_canvas_pan` and `sfc_canvas_zoom` per POU in `FerriteStudioApp`, pass them into `render_sfc_editor`, and handle `mouse::Event::WheelScrolled` in `SfcCanvasProgram::update` with `frame.scale(zoom)` in `draw()`.
   - **If we skip it:** Large Grafcet sequences cannot be zoomed out, and switching POU tabs resets the user's panned viewport position.

8. **Upgrade the FBD editor from a card list to an interactive 2D wiring canvas** (`crates/ferrite-gui/src/canvas_fbd.rs:L450-1335`)
   - **What this is:** The Function Block Diagram editor in `ferrite-gui`.
   - **Problem:** While LD (`canvas_ld.rs`) and SFC (`canvas_sfc.rs`) are interactive 2D `canvas::Program` schematics, `canvas_fbd.rs` renders blocks as a vertical scrollable list of cards where block-to-block links (`B_CMP.OUT`) must be typed manually into text inputs (`L863`).
   - **Fix:** Add an interactive `canvas::Program` mode in `canvas_fbd.rs` supporting 2D grid placement (`grid_pos`), middle-mouse panning, wheel zoom, and drag-and-drop pin-to-pin wire connections.
   - **If we skip it:** Wiring multi-block FBD control loops remains slower and more error-prone than wiring SFC or Ladder diagrams.

9. **Remove unused `rune`, `egui`, `eframe`, `egui_plot` dependencies and dead `egui` Digital Twin code** (`Cargo.toml:L47,56-58`, `crates/ferrite-cli/Cargo.toml:L24`, `crates/ferrite-twin/Cargo.toml:L14`, `crates/ferrite-twin/src/draw.rs`, `crates/ferrite-twin/src/theme.rs`, `crates/ferrite-twin/src/workspace.rs:L360-1430`, `package.json`, `package-lock.json`)
   - **What this is:** Legacy UI and scripting dependencies left over from earlier prototypes.
   - **Problem:**
     - `rune` in `Cargo.toml` and `crates/ferrite-cli/Cargo.toml` is never imported anywhere in the workspace, pulling in ~20 transitive crates (`rune-*`, `musli-*`, `syntree`, etc.).
     - `eframe` and `egui_plot` in `Cargo.toml` have zero dependents in the workspace.
     - `egui` in `crates/ferrite-twin` is only used by dead `egui` rendering code (`draw.rs`, `theme.rs`, and `DigitalTwinWorkspace::ui` in `workspace.rs`), because `crates/ferrite-gui/src/digital_twin.rs` renders the Digital Twin natively in Iced.
     - `package.json` and `package-lock.json` (`144 KB`) at the repo root declare an unused npm package (`opencode-gemini-auth`) in a pure Rust project.
   - **Fix:** Remove `rune`, `egui`, `eframe`, and `egui_plot` from `Cargo.toml` and member manifests, delete `crates/ferrite-twin/src/draw.rs`, `crates/ferrite-twin/src/theme.rs`, and the unused `egui::Ui` methods in `workspace.rs` (keeping the pure data model, hit-testing, and JSON persistence), and delete `package.json` / `package-lock.json`.
   - **If we skip it:** Every clean build compiles dozens of unused crates and maintains ~1,400 lines of unreachable `egui` code.

10. **Cache `is_nerd_font_available()` in a `OnceLock<bool>`** (`crates/ferrite-gui/src/lib.rs:L68-80`, `crates/ferrite-gui/src/icons.rs:L5-83`)
    - **What this is:** Font check called by every icon helper (`icon_lock`, `icon_warning`, `icon_close`, `icon_gear`, `icon_grip`, etc.) on every GUI frame.
    - **Problem:** `is_nerd_font_available()` acquires a write lock on the global Cosmic Text font system (`iced::advanced::graphics::text::font_system().write()`) and iterates over all loaded system font faces (`db.faces().any(...)`) on every single call.
    - **Fix:** Store the result in a `static NERD_FONT_AVAILABLE: std::sync::OnceLock<bool>` computed once inside `is_nerd_font_available()`.
    - **If we skip it:** Every frame with icons repeatedly locks the global font database and scans all installed system fonts.

11. **Fix `icon_save()` fallback glyph and filter Trend Plotter Y-autoscale to the visible time window** (`crates/ferrite-gui/src/icons.rs:L68-74`, `crates/ferrite-gui/src/trend_plotter.rs:L201-217`)
    - **What this is:** Fallback icon rendering and real-time PID/tag trend chart scaling in `ferrite-gui`.
    - **Problem:**
      - `icon_save()` returns literal `"??"` when Nerd Fonts are unavailable (`icons.rs:L72`), whereas all sibling icons return Unicode/Emoji symbols.
      - `TrendCanvas::draw` computes `min_y` and `max_y` over all buffered points (`1.5 × time_window_s`) instead of filtering `t >= min_x` (`trend_plotter.rs:L208`), so a spike that already scrolled off the left edge of the chart keeps squashing the Y-axis scale.
    - **Fix:** Return `"💾"` in `icon_save()`'s else branch, and add `if t < min_x { continue; }` inside the Y-range loop in `TrendCanvas::draw` (`trend_plotter.rs:L208`).
    - **If we skip it:** Users without Nerd Fonts see `"??"` on save buttons, and the trend plotter Y-axis stays compressed after transient spikes leave the visible window.

12. **Complete 5-language localization (`En`, `Fr`, `De`, `Es`, `It`) and dynamic menu alignment** (`crates/ferrite-gui/src/i18n.rs:L53-59`, `crates/ferrite-gui/src/app.rs:L7124-7129`, `crates/ferrite-gui/src/canvas_sfc.rs:L3627-3679`, `crates/ferrite-gui/src/alarms_panel.rs:L11-250`)
    - **What this is:** Multi-language UI translation and top-bar dropdown positioning.
    - **Problem:**
      - `UiLanguage::tr(self, en, fr)` only supports English and French (falling back to English for German, Spanish, and Italian).
      - `render_sfc_editor` ignores its `_lang: UiLanguage` parameter and hardcodes English strings on all toolbar buttons, tips, and inspectors.
      - `alarms_panel.rs` hardcodes English table headers and buttons.
      - `app.rs:L7124-7129` uses fixed English pixel offsets (`88.0`, `134.0`, `180.0`, `248.0`) to position top-bar dropdown menus, which misalign when translated menu labels change width.
    - **Fix:** Migrate remaining `lang.tr(...)` and hardcoded SFC/Alarms strings to `crate::tr!` locale keys (`locales/*.yml`) and compute menu dropdown offsets from label widths or anchor each dropdown inside its header button column.
    - **If we skip it:** Selecting German, Spanish, or Italian leaves half the UI in English, and translated top-bar menus open at misaligned horizontal offsets.

---

## Nice to Have (Code Quality & API Consistency)

13. **Replace `Result<_, String>` with a typed `TwinError` (`thiserror`) in `ferrite-twin`** (`crates/ferrite-twin/Cargo.toml:L13`, `crates/ferrite-twin/src/workspace.rs:L1082-1155`)
    - **What this is:** Error type for loading and saving `digital_twin.json` workspaces.
    - **Problem:** `ferrite-twin` declares `thiserror` in `Cargo.toml` (and `AGENTS.md §5.1` requires `thiserror` in all library crates), but `load_from_file` and `save_to_file` return `Result<_, String>`.
    - **Fix:** Define `pub enum TwinError` deriving `thiserror::Error` for I/O and JSON serialization errors and return `Result<_, TwinError>`.
    - **If we skip it:** Callers cannot distinguish I/O errors from JSON schema parse errors without matching on formatted strings.

14. **Update stale default scan-cycle documentation in the built-in Help Manual** (`crates/ferrite-gui/src/help_window.rs:L1346`)
    - **What this is:** Built-in F1 Studio Reference Manual describing the default scan cycle rate.
    - **Problem:** The help text states a 10 ms default scan cycle in places where `Project::scaffold` configures a 50 ms (`scan_cycle_ms = 50`) default.
    - **Fix:** Align the help window text with the actual 50 ms scaffold default (or reference the project's configured `scan_cycle_ms`).
    - **If we skip it:** Users reading the built-in manual may miscalculate timer/counter tick rates on newly scaffolded projects.

---

- **Verdict:** Healthy and well-tested (all workspace tests and Clippy `-D warnings` pass); prioritize **#1–#3** for strict real-time scan determinism and **#9–#10** for immediate build/UI speedups.
- **Lean:** ~`-1,400` lines of dead `egui` code and `-4` direct workspace dependencies (`rune`, `egui`, `eframe`, `egui_plot`, plus root `package.json`/`package-lock.json`) removable with zero feature loss.
- **Not checked:** Physical Raspberry Pi / Linux `/sys/class/gpio` hardware execution on target boards (only host-side driver logic and mock tests were inspected).
