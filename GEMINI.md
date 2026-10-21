# FerritePLC — Antigravity Workspace Rules & Architecture Spec

## 1. Project Overview
**FerritePLC** is a deterministic, high-performance Programmable Logic Controller (PLC) runtime, compiler, and simulator written in Rust (2024 Edition). It supports four IEC 61131-3 / IEC 60848 industrial programming paradigms with seamless **inter-language communication** and **project/file management**.

### Supported Languages
1. **Ladder Diagram (LD):** Rung-based boolean logic, contacts (NO/NC, rising/falling edge), coils (normal, set/latch, reset/unlatch), and inline Function Block calls.
2. **Grafcet / Sequential Function Chart (SFC):** Steps (initial, normal, macro), directed links, transitions (receptivities), and step-associated actions (Qualifier types: N, S, R, L, D, P).
3. **Structured Text (ST):** High-level imperative language (`IF/THEN/ELSIF/ELSE`, `CASE`, `FOR`, `WHILE`, arithmetic/bitwise/boolean expressions, and FB invocations).
4. **Function Block Diagram (FBD):** Signal-flow blocks, standard library blocks (`TON`, `TOF`, `TP`, `CTU`, `CTD`, `R_TRIG`, `F_TRIG`, `SR`, `RS`), and user-defined composite function blocks.

---

## 2. Core Architecture & Inter-Language Communication

To allow any language to trigger, read, or mutate state in another language (e.g., a **Ladder rung** activating a **Grafcet transition**, or a **Grafcet step** calling a **Structured Text routine**), FerritePLC enforces a **Unified Symbol Table & Shared Process Image** architecture.

### 2.1 The Shared Process Image (`MemoryMap`)
All languages read from and write to a single deterministic memory space during each scan cycle:
- **Inputs (`%I`):** Physical/simulated discrete and analog inputs (`%IX0.0`, `%IW0`).
- **Outputs (`%Q`):** Physical/simulated discrete and analog outputs (`%QX0.0`, `%QW0`).
- **Internal Memory (`%M`):** Shared bit/byte/word/dword markers (`%MX0.0`, `%MW10`).
- **Symbolic Tag Registry (`TagTable`):** Named variables (`Start_Button`, `Conveyor_Ready`, `Tank_Level`) mapped to strong types (`BOOL`, `INT`, `DINT`, `REAL`, `TIME`, `STRING`) or structured Function Block instances.
- **Grafcet Introspection Symbols:** Every Grafcet step `i` automatically exposes its state to the global symbol table as `X<i>.state` (`BOOL`, read-only outside SFC engine) and `X<i>.t` (`TIME`, active step elapsed duration). Every transition receptivity can bind directly to any global symbol, Ladder coil (`%M` / named `BOOL`), or FBD output pin.

### 2.2 Inter-Language Execution Contract
- **Common Intermediate Representation (FerriteIR) & Trait Interface:**
  Every compiled Program Organization Unit (POU)—regardless of whether its source is `.ld`, `.sfc`, `.st`, or `.fbd`—implements the `ExecutablePou` trait:
  ```rust
  pub trait ExecutablePou {
      fn id(&self) -> PouId;
      fn kind(&self) -> LanguageKind;
      fn execute(&mut self, ctx: &mut ScanContext) -> Result<(), RuntimeError>;
  }
  ```
- **Cross-Language Examples to Support by Default:**
  - *Ladder -> Grafcet:* A Ladder rung evaluates safety interlocks and writes to a symbolic boolean `Permissive_OK`. A Grafcet transition uses `Permissive_OK AND Sensor_1` as its receptivity condition.
  - *Grafcet -> Structured Text / FBD:* An active Grafcet step `Step_Fill` executes an `N` (Non-stored) action that invokes an ST script or triggers the `IN` pin of an FBD `TON` timer instance.
  - *FBD -> Ladder:* An FBD PID or comparator block outputs a `High_Temp_Alarm` boolean that breaks a Normally Closed (`NC`) contact in a Ladder emergency stop rung.

### 2.3 Deterministic Scan Cycle
The `ferrite-runtime` engine must strictly execute the PLC scan loop in four atomic phases:
1. **Input Scan:** Latch simulated hardware inputs into the `ProcessImage` input buffer.
2. **Task & POU Execution:** Execute scheduled tasks in configured priority order (`Cyclic`, `Continuous`, `Event`). For Grafcet POUs, strictly follow the IEC 60848 evolution rules (evaluate receptivities -> Simultaneous clearing of active steps -> Simultaneous activation of target steps -> Evaluate step actions).
3. **Output Scan:** Commit the `ProcessImage` output buffer to the simulator I/O bus.
4. **Diagnostics & Watchdog:** Record scan duration (`us`), check for watchdog timeouts, and publish state snapshots to the UI/CLI telemetry channel.

---

## 3. Cargo Workspace Layout

Maintain strict crate boundaries. Never introduce circular dependencies:

```text
ferrite-plc/
├── AGENTS.md                  # Antigravity CLI workspace rules (this file)
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── ferrite-core/          # Data types (IEC types), TagTable, ProcessImage, ScanContext, Errors
│   ├── ferrite-ir/            # Common IR, cross-language symbol resolver, PouGraph
│   ├── ferrite-lang-ld/       # Ladder logic AST, parser, validator, and IR compiler
│   ├── ferrite-lang-sfc/      # Grafcet (SFC) AST, step/transition solver, action qualifiers
│   ├── ferrite-lang-st/       # Structured Text lexer (`logos`), parser (`chumsky`/`pest`), compiler
│   ├── ferrite-lang-fbd/      # Function Block network graph, topological sorter, IEC standard blocks
│   ├── ferrite-project/       # Project workspace manager, serialization, file I/O, schema migrations
│   ├── ferrite-runtime/       # Deterministic scan loop, virtual clock, breakpoints, force/watch table
│   └── ferrite-cli/           # TUI/CLI binary (`ratatui` + `clap`) to create, load, inspect, and simulate
└── examples/                  # Sample multi-language FerritePLC projects
```

---

## 4. Project & File Management Specification (`ferrite-project`)

FerritePLC projects are self-contained, human-readable, and Git-friendly directories managed via a root `.fplc` manifest (TOML) and structured language files.

### 4.1 On-Disk Project Structure
```text
my_automation_plant/
├── FerriteProject.fplc        # Main project manifest (TOML): metadata, scan cycle rate, task order
├── symbols/
│   ├── global_tags.toml       # Global variables, %I/%Q/%M mappings, initial values, comments
│   └── fb_instances.toml      # Shared Function Block instances (e.g., Timer_1: TON)
├── pous/
│   ├── main_safety.ld.json    # Ladder Diagram POU
│   ├── batch_seq.sfc.json     # Grafcet sequence POU
│   ├── math_pid.st            # Structured Text POU (plain text .st)
│   └── pump_ctrl.fbd.json     # Function Block Diagram POU
└── sim/
    └── scenarios.toml         # Automated stimulus/test sequences for headless simulation
```

### 4.2 File Operations Required
The `ferrite-project` crate and `ferrite-cli` binary must support:
- `ferrite new <project_name>`: Scaffolds a new project folder with `FerriteProject.fplc`, default symbol table, and starter POUs.
- `ferrite save` / `Project::save(&self, path)`: Atomically writes the project manifest, symbol tables, and all modified POU files (using write-to-temp + rename to prevent corruption).
- `ferrite add-pou <name> --lang <ld|sfc|st|fbd>`: Creates a new source file in `pous/` and registers it in the task execution table of `FerriteProject.fplc`.
- `ferrite check`: Validates syntax across all POUs, resolves cross-language symbols, checks for type mismatches, and detects combinational loops in FBD/Ladder.
- `ferrite sim [--headless]`: Runs the interactive TUI simulator or executes a headless test scenario.

---

## 5. Coding Standards & Antigravity Agent Workflow

### 5.1 Rust Conventions
- **Zero `unwrap()` / `expect()` in library crates:** Use `thiserror` in library crates (`ferrite-core`, `ferrite-lang-*`, `ferrite-runtime`, `ferrite-project`) and `anyhow` or `miette` (for rich source-span compiler diagnostics) in `ferrite-cli`.
- **Real-Time Safety in Scan Loop:** The `ferrite-runtime` scan cycle (`execute` hot path) must avoid heap allocations (`Vec::push`, `String` formatting, `Box::new`) during normal tick execution. Pre-allocate memory tables and arena indices during project compilation/loading.
- **Serialization:** Use `serde` (`Serialize`, `Deserialize`) with `toml` for manifests/symbols and `serde_json` for graphical POU ASTs (`.ld.json`, `.sfc.json`, `.fbd.json`). Keep `.st` files as raw UTF-8 text.

### 5.2 Verification Loops (Mandatory for Antigravity Agents)
Whenever implementing a feature or modifying a crate, execute the following verification loop before concluding a task:
1. `cargo check --workspace --all-targets`
2. `cargo test --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. **Integration Check:** Any change to a language crate (`ferrite-lang-*`) must include a unit test demonstrating **inter-language state mutation** via `ScanContext` (e.g., executing an LD rung, then stepping an SFC transition that depends on that rung's output).

---

## 6. Quick Start & CLI Workflow

1. **Initialize the repository and place this file:**
   ```bash
   mkdir ferrite-plc && cd ferrite-plc
   # Save this file as AGENTS.md at the workspace root
   ```
2. **Launch Antigravity CLI in your project folder:**
   When you start `agy` (Antigravity CLI) inside `ferrite-plc/`, it automatically loads `AGENTS.md` into its system context.
3. **Kick off the build using the Explore -> Plan -> Execute pattern:**
   Prompt the CLI agent with:
   > *"Read AGENTS.md and generate an implementation plan to scaffold the Cargo workspace, starting with `ferrite-core` (ProcessImage, TagTable, IEC data types) and `ferrite-project` (saving/loading `.fplc` projects)."*