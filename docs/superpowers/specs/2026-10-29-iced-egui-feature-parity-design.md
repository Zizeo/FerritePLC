# Design Specification: Iced UI Feature Parity with egui (Phase 1)

## 1. Intent & Purpose
FerritePLC Studio was originally developed using `egui` (`eframe`) and recently migrated to `iced` (0.13+) for a modern, production-grade Elm Architecture (TEA) desktop interface. While the `Iced-UI` branch has successfully elevated visual design, SFC graphical editing, and simulation commissioning controls, several core features from the `egui` branch remain unported.

This specification defines the Phase 1 feature parity implementation, restoring:
1. **Native OS Project File Operations & PLCopen TC6 XML Import/Export** via `rfd`.
2. **Global Keyboard Shortcuts** (`Ctrl+S`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+O`, `Delete`).
3. **Structured Header Menus** (`File`, `PLC`, `Panels`, `Options`) for discovering tools and actions.
4. **Raspberry Pi 40-Pin Interactive GPIO Header Map** with color-coded pin assignments, live electrical states, and clickable simulation signal toggles.
5. **Modbus TCP Live Telemetry & IEC 61131-3 Mapping Reference Table**.

---

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- **Native Project Portability**: Enable users to browse and open projects from arbitrary directories, open `.fplc` manifest files directly, save project copies to new folders ("Save As"), and export/import standardized PLCopen TC6 XML.
- **Operator Productivity**: Restore standard keyboard workflows (`Ctrl+S` to save, `Ctrl+Z` to undo, `Ctrl+Y` to redo, `Ctrl+O` to open folder).
- **Hardware Simulation Realism**: Recreate the complete 2×20 Raspberry Pi 40-pin visual header in `BottomDockTab::Gpio` showing pin names, electrical roles, active high/low states, and direct toggles for physical input simulation.
- **Fieldbus Transparency**: Display live Modbus metrics (requests, client count, error exceptions) and the standard IEC 61131-3 Modbus address mapping table in `BottomDockTab::Modbus`.
- **Zero-Panic, Strict Real-Time Safety**: Zero `unwrap()` / `expect()` in library crates, robust handling of file system errors with status banner feedback.

### Non-Goals
- Phase 2 features (in-canvas FBD PID tuning bar and Ladder Diagram contact drag-and-drop / marquee box selection) are deferred to subsequent work to keep this slice verifiable.
- Modifying underlying PLC runtime execution cycles or IEC compiler semantics.

### Anti-Goals
- Blocking the UI event loop with unhandled I/O failures (file dialog cancellations must be no-ops).
- Violating Elm Architecture principles (all interactions must flow through explicit `Message` variants and state updates in `update()`).

---

## 3. Constraints
- **Framework**: `iced` 0.13+ with Elm Architecture (`State`, `Message`, `update`, `view`, `subscription`).
- **File Dialogs**: `rfd` (0.15), keeping platform-native look and feel without external C dependencies.
- **Rust Edition**: Rust 2024 Edition across the workspace.
- **Linting & Quality**: 100% clean `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`.

---

## 4. Approaches Considered & Chosen

### Approaches
1. **Approach A (Complete Single-Pass Migration)**: Implement Tiers 1, 2, and 3 all at once. Rejected due to excessively broad scope and risk of regressions in recently stabilized SFC and canvas editors.
2. **Approach B (Staged Two-Phase Delivery - CHOSEN)**:
   - **Phase 1 (This Spec)**: Deliver Tier 1 (File Dialogs, PLCopen XML, Shortcuts, Header Menus) and Tier 2 (40-Pin GPIO Header Map, Modbus Telemetry & Mapping Table).
   - **Phase 2**: Deliver Tier 3 (FBD PID tuning bar, LD contact moving).
3. **Approach C (Minimal I/O Only)**: Only port file dialogs and GPIO panel without keyboard shortcuts or menus. Rejected because keyboard productivity is essential for PLC engineering.

---

## 5. Architectural Specification

### 5.1 Native OS File Dialogs & PLCopen TC6 XML (`rfd`)

#### Messages
Add to `crates/ferrite-gui/src/app.rs::Message`:
```rust
pub enum Message {
    // ...
    OpenProjectFolderDialog,
    OpenProjectManifestDialog,
    SaveProjectAsDialog,
    ExportPlcopenXmlDialog,
    ImportPlcopenXmlDialog,
    LoadProjectFromPath(std::path::PathBuf),
    // ...
}
```

#### Behavioral Specifications
1. **`OpenProjectFolderDialog`**:
   - Spawns `rfd::FileDialog::new().set_title(...).pick_folder()`.
   - If a directory is selected:
     - Check if `selected/FerriteProject.fplc` exists.
     - Call `Project::load(&selected)`.
     - On success: replace `self.project`, recompile runtime, clear undo/redo stacks, update `status_banner = "Project loaded from <path>"`.
     - On failure: set `status_banner = format!("Failed to load project: {err}")`.
2. **`OpenProjectManifestDialog`**:
   - Spawns `rfd::FileDialog::new().set_title(...).add_filter("FerritePLC Manifest (.fplc)", &["fplc"]).pick_file()`.
   - If selected: resolve parent directory, load via `Project::load(parent)`.
3. **`SaveProjectAsDialog`**:
   - Spawns `rfd::FileDialog::new().set_title(...).pick_folder()`.
   - If selected: update `self.project.root_dir = selected`, execute `self.project.save(&selected)`, and update `status_banner`.
4. **`ExportPlcopenXmlDialog`**:
   - Spawns `rfd::FileDialog::new().set_title("Export PLCopen TC6 XML").add_filter("PLCopen XML (.xml)", &["xml"]).set_file_name("project_plcopen.xml").save_file()`.
   - If selected: call `self.project.export_plcopen_xml()`, write to destination file path.
5. **`ImportPlcopenXmlDialog`**:
   - Spawns `rfd::FileDialog::new().set_title("Import PLCopen TC6 XML").add_filter("PLCopen XML (.xml)", &["xml"]).pick_file()`.
   - If XML selected: prompt for destination folder via `.pick_folder()`.
   - Read file content and invoke `Project::import_plcopen_xml(&xml_content, &target_dir)`.
   - On success: set `self.project = imported_project`, recompile runtime, update `status_banner`.

---

### 5.2 Global Keyboard Shortcuts

#### In `subscription(&self)`
Update `key_sub` to inspect both key names/characters and active modifier keys:
```rust
let key_sub = iced::keyboard::on_key_press(|key, modifiers| {
    if modifiers.command() {
        match key {
            iced::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                Some(Message::SaveProject)
            }
            iced::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("o") => {
                Some(Message::OpenProjectFolderDialog)
            }
            iced::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("z") => {
                if modifiers.shift() {
                    Some(Message::Redo)
                } else {
                    Some(Message::Undo)
                }
            }
            iced::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("y") => {
                Some(Message::Redo)
            }
            _ => None,
        }
    } else {
        match key {
            iced::keyboard::Key::Named(
                iced::keyboard::key::Named::Delete | iced::keyboard::key::Named::Backspace,
            ) => Some(Message::SfcRequestDeleteSelected(String::new())),
            _ => None,
        }
    }
});
```

---

### 5.3 Structured Header Menus (`File`, `PLC`, `Panels`, `Options`)

Add an enum to track which menu popover/modal is open (if any):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMenu {
    File,
    Plc,
    Panels,
    Options,
}
```
Add to `FerriteStudioApp`:
`pub active_menu: Option<ActiveMenu>,`

#### Header Bar Menu Items:
- **`[File ▾]`**:
  - `Save Project` (`Ctrl+S`)
  - `Save Project As... (OS Explorer)`
  - `Open Project Folder... (OS Explorer)` (`Ctrl+O`)
  - `Open Manifest (.fplc)...`
  - Separator
  - `Export PLCopen TC6 XML...`
  - `Import PLCopen TC6 XML...`
  - Separator
  - `Global Tags & Configuration`
  - `Exit Studio`
- **`[PLC ▾]`**:
  - `Run / Pause PLC`
  - `Single Step Scan`
  - `Reset (Warm)`
  - `Reset (Cold)`
  - Separator
  - `Check & Compile`
  - `Run Automated Scenarios`
- **`[Panels ▾]`**:
  - `Toggle Project Explorer`
  - `Toggle Virtual I/O Rack`
  - `Toggle Bottom Dock`
  - Separator
  - `Detach / Dock Explorer Window`
  - `Detach / Dock Virtual I/O Window`
  - `Detach / Dock Bottom Dock Window`
- **`[Options ▾]`**:
  - `Language`: `English` / `Français`
  - `Theme`: `Dark (Obsidian)` / `Light`

---

### 5.4 Raspberry Pi 40-Pin Interactive GPIO Header Map

Located in `BottomDockTab::Gpio` view:

#### Visual Architecture:
- Uses `ferrite_hal::RPI_40PIN_HEADER` (40 pins):
  - Formatted into 20 rows of dual pins:
    - **Left column**: Odd pins (1, 3, 5, ..., 39)
    - **Center column**: Pin number badges `[1] [2]`, `[3] [4]`
    - **Right column**: Even pins (2, 4, 6, ..., 40)
- **Pin Color Coding**:
  - 3.3V Power: Orange / Amber (`#F59E0B`)
  - 5.0V Power: Rose / Red (`#EF4444`)
  - Ground (GND): Slate / Dark Gray (`#475569`)
  - I2C (SDA/SCL): Cyan (`#06B6D4`)
  - SPI (MOSI/MISO/SCLK/CE): Indigo / Purple (`#8B5CF6`)
  - UART (TX/RX): Amber (`#D97706`)
  - General GPIO (BCM): Emerald (`#10B981`) when High, Slate (`#334155`) when Low.
- **Live State & Interactivity**:
  - When `self.hal_enabled == true`:
    - Display mapped PLC tag or address next to the pin (e.g. `BCM 17 → %IX0.0`).
    - If pin is an input pin, rendering it as a styled button: clicking it dispatches `Message::MockGpioPinToggled(bcm)`.
    - If pin is an output pin, rendering it as an indicator showing live physical state driven by the PLC output scan.

---

### 5.5 Modbus TCP Live Telemetry & Mapping Table

Located in `BottomDockTab::Modbus` view:

#### Visual Architecture:
1. **Server Header & Controls**:
   - Status badge: `[RUNNING on 127.0.0.1:<port>]` in Emerald or `[STOPPED]` in Rose.
   - Start / Stop button, Port input, Ping test button with test result log.
2. **Live Telemetry Banner** (when running):
   - Query `srv.telemetry()`:
     - `Requests Total`: Total FC01/02/03/04/05/06/15/16 requests served.
     - `Active Clients`: Currently connected client sockets.
     - `Exceptions Total`: Illegal address or unsupported function code count.
3. **IEC 61131-3 Standard Modbus Address Mapping Table**:
   - Render a structured table with 4 columns:
     - `Modbus Range`: `00001 - 01000`, `01001 - 02000`, `10001 - 11000`, `30001 - 31000`, `40001 - 41000`, `41001 - 42000`.
     - `Function Codes`: `FC01, FC05, FC15`, `FC02`, `FC04`, `FC03, FC06, FC16`.
     - `FerritePLC Address`: `%QX0.0..%QX124.7`, `%MX0.0..%MX124.7`, `%IX0.0..%IX124.7`, `%IW0..%IW999`, `%QW0..%QW999`, `%MW0..%MW999`.
     - `Access Mode`: `Read / Write` (coils/holding) vs `Read Only` (discrete inputs/input registers).

---

## 6. Verification & Quality Plan

1. **Unit & Integration Tests**:
   - Add unit test verifying that `Message::LoadProjectFromPath` correctly reloads the project, updates manifest references, and re-initializes runtime.
   - Add unit test verifying that `subscription()` shortcut mapping translates `Ctrl+S`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+O` into their expected messages.
   - Add unit test verifying Modbus telemetry query and GPIO pin state toggling.
2. **Quality Gates**:
   - `cargo check --workspace --all-targets` (0 errors).
   - `cargo test --workspace` (all 54+ tests pass).
   - `cargo clippy --workspace --all-targets -- -D warnings` (0 warnings).
