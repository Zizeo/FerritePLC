# FerritePLC v0.1.0

**Release date:** 2026-11-12 · **Commit:** `8da0126` · **License:** MIT

First public build of FerritePLC — a deterministic PLC compiler, runtime, simulator
and desktop IDE written in Rust (2024 edition). It covers four industrial
programming languages with cross-language state sharing over one symbol table and
one process image.

## Downloads

| Platform | Files | Requirements |
| --- | --- | --- |
| Windows x64 | `ferrite.exe`, `ferrite-gui.exe` | Windows 10+ |
| Linux x64 | `ferrite`, `ferrite-gui` | glibc 2.30+, X11 or Wayland (GUI) |

Both binaries are stripped release builds. `ferrite` is the CLI/TUI, `ferrite-gui`
is FerritePLC Studio.

## Highlights

### Four languages, one project
- **Ladder Diagram (LD)** — rungs with NO/NC contacts, rising/falling edge contacts,
  normal/set/reset coils, inline function block calls, on-canvas symbol editing and
  drag-and-drop reordering.
- **Grafcet / SFC (IEC 60848)** — initial, normal, macro and enclosing steps; single
  transitions; OR and AND divergences/convergences; action qualifiers `N`, `S`, `R`,
  `L`, `D`, `P` plus force qualifiers `F`/`FZ`; macro steps with embedded
  sub-diagrams expanded at compile time.
- **Structured Text (ST)** — `IF/ELSIF/ELSE`, `CASE`, `FOR`, `WHILE`, local variables,
  user functions, arithmetic/bitwise/boolean expressions, extended standard library.
- **Function Block Diagram (FBD)** — topological sorting, combinational-loop
  detection, IEC blocks (`TON`, `TOF`, `TP`, `CTU`, `CTD`, `R_TRIG`, `F_TRIG`, `SR`,
  `RS`, comparators, math, logic), PID correctors and composite/custom ST blocks.

### Inter-language execution
Every POU implements the same `ExecutablePou` trait and runs against a shared
`ScanContext`: a ladder rung can grant a Grafcet permissive, a Grafcet step can
trigger an ST routine or FBD timer, an FBD comparator can open a ladder NC contact.
Grafcet steps expose `X<i>.state` and `X<i>.t` globally.

### Deterministic runtime
Four-phase scan loop (input latch → task/POU execution → output commit →
diagnostics/watchdog) with configurable cyclic, continuous and event tasks,
per-task watchdogs, virtual clock, force/watch tables, breakpoints per POU, in-scan
trace, alarm journal and bounded event history. Release builds keep
`overflow-checks = true` so silent integer wrap cannot corrupt physical outputs.

### FerritePLC Studio (GUI)
- iced 0.13 desktop IDE with detachable multi-window panels (POU explorer, tag
  watch/force, scenarios and diagnostics).
- On-canvas direct editing for LD, SFC, FBD and ST; classic desktop dropdown menus;
  light/dark themes with persisted user settings.
- Live simulation: continuous run, single-scan step, input toggling, forcing,
  trend plotter, commissioning breakpoints, online change reload.
- Digital Twin window: 2D CAD editor for plant layouts with runtime mode.
- Integrated help manual (per-language and per-topic) and localization: English,
  French, German, Spanish, Italian — OS language detection with a manual override.

### Fieldbus and IoT
- **Modbus** — TCP client/server and RTU serial master/slave, frame accumulation with
  per-client reassembly and pipelining, strict MBAP validation, zero-allocation scan
  breakpoints.
- **IoT** — MQTT telemetry client (change-of-value publishing, connection-state
  tracking) and an OPC UA address space with typed variable nodes and write sinks.
- **HAL** — GPIO/sysfs driver layer for plant I/O.

### Tooling
```
ferrite new <dir>                     scaffold a project
ferrite add-pou <name> --lang <ld|sfc|st|fbd>
ferrite save                          atomic, formatted project write
ferrite check [--format text|json]    syntax, cross-language symbols, loop detection
ferrite sim [--headless] [--format text|json]
ferrite gui <project>                 launch Studio
ferrite export-plcopen / import-plcopen
```
`check` and `sim` emit machine-readable JSON for CI; headless scenarios
(`sim/scenarios.toml`) support assertions and per-scenario failure reporting.

## Project format
Git-friendly directories: `FerriteProject.fplc` manifest (TOML), `symbols/`
(global tags, shared FB instances), `pous/` (`.ld.json`, `.sfc.json`, `.fbd.json`,
plain `.st`), `sim/scenarios.toml`. Writes are temp-file + rename; PLCopen TC6 XML
import/export is supported.

## Known limitations
- Fieldbus, HAL and twin paths are simulator/loopback tested, not yet validated
  against real plant hardware.
- Some scan-hot-path allocations remain (watch/telemetry paths) — see
  `REVIEW_REPORT.md`.
- PLCopen import does not fully guard POU name collisions.
- Remaining IEC 61131-3 coverage gaps (string function family, some conversions,
  online monitoring depth) are tracked in `docs/FEATURE_GAP_ANALYSIS.md`.

## Building

```bash
cargo build --release --workspace --bins          # Windows / native
pwsh -File scripts/build-linux.ps1                # Linux GNU from Windows (zigbuild)
cargo zigbuild --release --workspace --bins --target x86_64-unknown-linux-gnu

# Linux, plain cargo only (fully static musl binary, no zig/WSL):
rustup target add x86_64-unknown-linux-musl
cargo build --release --workspace --bins --target x86_64-unknown-linux-musl
```

Verification per workspace rules: `cargo check --workspace --all-targets`,
`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`.
