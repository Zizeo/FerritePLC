# FerritePLC

**FerritePLC** is a deterministic, high-performance Programmable Logic Controller (PLC) runtime, compiler, simulator, and graphical IDE written in Rust (2024 Edition).

It supports four industrial programming paradigms defined by **IEC 61131-3** and **IEC 60848**, with seamless **inter-language communication** over a unified symbol table and shared process image.

> **AI-Assisted Development Notice:** This project is developed with the assistance of AI coding agents (Google Antigravity) paired with human direction, architectural review, and verification loops.

---

## Features & Supported Languages

1. **Ladder Diagram (LD)**
   - Rung-based boolean power-flow logic.
   - Contacts: Normally Open (`NO`), Normally Closed (`NC`), Rising Edge (`P`), Falling Edge (`N`).
   - Coils: Normal (`( )`), Set/Latch (`(S)`), Reset/Unlatch (`(R)`).
   - Direct on-canvas editing of contact/coil symbols and types, plus drag-and-drop reordering.
2. **Grafcet / Sequential Function Chart (SFC — IEC 60848)**
   - Steps: Initial, Normal, Macro, and Enclosing steps.
   - Directed links with single transitions, OR divergences/convergences, and simultaneous AND divergences/convergences.
   - Step-associated actions with IEC 60848 qualifiers (`N`, `S`, `R`, `L`, `D`, `P`, plus `SET_SFC`, `RESET_SFC`, `FREEZE_SFC`).
   - Direct on-canvas editing of step actions and transition receptivities.
3. **Structured Text (ST)**
   - High-level imperative PLC language (`IF/THEN/ELSIF/ELSE`, `CASE`, `FOR`, `WHILE`).
   - Arithmetic, bitwise, comparison, and boolean expressions.
   - Standard and cross-language Function Block invocations.
4. **Function Block Diagram (FBD)**
   - Signal-flow block networks with automatic topological sorting and combinational loop detection.
   - Standard IEC library blocks: Timers (`TON`, `TOF`, `TP`), Counters (`CTU`, `CTD`), Edge Triggers (`R_TRIG`, `F_TRIG`), Bistables (`SR`, `RS`), Comparators (`GT`, `GE`, `EQ`, `LT`), Math (`ADD`, `SUB`, `MUL`, `DIV`), and Logic (`AND`, `OR`, `XOR`, `NOT`).

---

## How FerritePLC Works

### 1. Unified Symbol Table & Shared Process Image
All four PLC languages read from and write to a single deterministic memory space (`ProcessImage` + `TagTable`) during each scan cycle:
- **Inputs (`%I`)**: Discrete and analog inputs (`%IX0.0`, `%IW0`).
- **Outputs (`%Q`)**: Discrete and analog outputs (`%QX0.0`, `%QW0`).
- **Internal Markers (`%M`)**: Shared bit/byte/word/dword memory (`%MX0.0`, `%MW10`).
- **Symbolic Tags (`TagTable`)**: Strongly typed named variables (`BOOL`, `INT`, `DINT`, `REAL`, `TIME`, `STRING`) optionally mapped to direct `%I`/`%Q`/`%M` addresses.
- **Shared Function Block Instances**: Named instances (`TON`, `CTU`, etc.) accessible across all POUs.
- **Grafcet Introspection Symbols**: Every Grafcet step `i` automatically exposes `X<i>.state` (`BOOL`, read-only outside the SFC engine) and `X<i>.t` (`TIME`, active elapsed duration) to the global symbol table.

### 2. Inter-Language Execution Contract
Every compiled Program Organization Unit (POU)—whether `.ld.json`, `.sfc.json`, `.st`, or `.fbd.json`—implements the common `ExecutablePou` trait:

```rust
pub trait ExecutablePou {
    fn id(&self) -> PouId;
    fn kind(&self) -> LanguageKind;
    fn execute(&mut self, ctx: &mut ScanContext) -> Result<(), RuntimeError>;
}
```

This allows any language to drive or inspect another within the same deterministic scan:
- **Ladder -> Grafcet**: A Ladder safety rung writes `Permissive_OK`, which a Grafcet transition evaluates in its receptivity (`Permissive_OK AND Sensor_1`).
- **Grafcet -> Structured Text / FBD**: An active Grafcet step `X2` triggers an `N` action or sets a boolean flag that enables an ST PID calculation or an FBD timer block.
- **FBD -> Ladder**: An FBD comparator block outputs `High_Temp_Alarm`, opening an `NC` contact in a Ladder emergency stop rung.

### 3. Deterministic 4-Phase Scan Cycle
`ferrite-runtime` executes each PLC scan cycle in four atomic phases:
1. **Input Scan**: Latch physical/simulated hardware inputs and forced values into the `ProcessImage` input buffer.
2. **Task & POU Execution**: Run scheduled tasks in configured priority order (`Cyclic`, `Continuous`, `Event`). Grafcet POUs strictly follow IEC 60848 evolution rules (evaluate receptivities -> simultaneously clear active steps -> simultaneously activate target steps -> evaluate step actions).
3. **Output Scan**: Commit the `ProcessImage` output buffer to the simulator I/O bus.
4. **Diagnostics & Watchdog**: Record scan duration (`us`), check for watchdog timeouts, and publish state snapshots to the GUI/CLI telemetry views.

---

## Workspace Architecture

```text
FerritePLC/
├── Cargo.toml                 # Rust 2024 workspace manifest
├── crates/
│   ├── ferrite-core/          # IEC data types, TagTable, ProcessImage, ScanContext, standard FBs
│   ├── ferrite-ir/            # Common IR, cross-language symbol resolver, PouGraph
│   ├── ferrite-lang-ld/       # Ladder Diagram AST, validator, and compiler
│   ├── ferrite-lang-sfc/      # Grafcet (SFC) AST, IEC 60848 solver, action qualifiers
│   ├── ferrite-lang-st/       # Structured Text lexer, parser, and compiler
│   ├── ferrite-lang-fbd/      # Function Block Diagram graph, topological sorter, loop detector
│   ├── ferrite-project/       # Project manifest (.fplc), atomic file I/O, symbol/POU serialization
│   ├── ferrite-runtime/       # Deterministic scan loop, virtual clock, force/watch table, scenarios
│   ├── ferrite-gui/           # Native desktop IDE & simulator (egui / eframe) with multi-window panels
│   └── ferrite-cli/           # CLI & terminal UI (clap + ratatui)
└── examples/                  # Multi-language sample projects (e.g., thermal_batch_plant)
```

---

## Project On-Disk Format

FerritePLC projects are human-readable, Git-friendly directories managed via a `FerriteProject.fplc` manifest:

```text
my_automation_plant/
├── FerriteProject.fplc        # Main project manifest (TOML): metadata, scan cycle rate, task order
├── symbols/
│   ├── global_tags.toml       # Global variables, %I/%Q/%M mappings, initial values, comments
│   └── fb_instances.toml      # Shared Function Block instances (e.g., Mix_Timer: TON)
├── pous/
│   ├── main_safety.ld.json    # Ladder Diagram POU
│   ├── batch_seq.sfc.json     # Grafcet sequence POU
│   ├── math_pid.st            # Structured Text POU (UTF-8 text)
│   └── pump_ctrl.fbd.json     # Function Block Diagram POU
└── sim/
    └── scenarios.toml         # Automated stimulus/assertion sequences for headless simulation
```

---

## Getting Started

### Prerequisites
- Rust toolchain supporting **Rust 2024 Edition** (`cargo`, `rustc`).

### Launching the Graphical Studio (`ferrite-gui`)
Launch the desktop IDE & live PLC simulator:

```bash
cargo run -p ferrite-gui
# Or open a specific project via CLI:
cargo run -p ferrite-cli -- gui examples/thermal_batch_plant
```

Inside **FerritePLC Studio**:
- Edit **Ladder**, **Grafcet (SFC)**, **Structured Text (ST)**, and **Function Block Diagram (FBD)** POUs directly on the interactive canvas.
- Detach side panels (**POU Explorer**, **Tag Watch & Force Table**, **Scenarios & Diagnostics**) into independent native OS windows (`⧉`) that can be moved across monitors, and re-attach them (`↙`) anytime.
- Save and load projects using native OS File Explorer dialogs.
- Run live continuous scans (`Run`), single-step scans (`Step 1 Scan`), toggle input stimuli, or force tags in real time.

### Using the CLI (`ferrite-cli`)

```bash
# Scaffold a new FerritePLC project
cargo run -p ferrite-cli -- new my_automation_plant

# Add a new POU (ld | sfc | st | fbd)
cargo run -p ferrite-cli -- add-pou my_automation_plant/FerriteProject.fplc pump_logic --lang ld

# Validate syntax, cross-language symbols, and combinational loops
cargo run -p ferrite-cli -- check examples/thermal_batch_plant

# Run headless simulation scenarios
cargo run -p ferrite-cli -- sim examples/thermal_batch_plant --headless

# Run interactive terminal UI (TUI) simulator
cargo run -p ferrite-cli -- sim examples/thermal_batch_plant
```

### Running Checks & Tests

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

---

## License

This project is licensed under the **MIT License**.

```text
MIT License

Copyright (c) 2026 FerritePLC Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
