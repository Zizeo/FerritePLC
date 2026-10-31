# Comprehensive In-App Help Window & Symbol Reference Design

## 1. Intent & The Why
FerritePLC is an industrial automation programming environment supporting four IEC 61131-3 / IEC 60848 languages, deterministic runtime simulation, fieldbus protocols, digital twin emulation, and process image memory mapping. 

To make the platform fully accessible to automation engineers, students, and technicians, FerritePLC requires an **integrated, comprehensive in-application reference manual and symbol guide**. Accessible under the **File** menu (and via `F1`), it opens in an **independent floating OS window** (`iced::window::open`) so users can keep the guide open on a secondary monitor or beside their workspace while actively designing logic and running simulations.

---

## 2. Goals, Non-Goals, & Anti-Goals

### Goals
- **File Menu Access & F1 Hotkey**: Add a "Help & Documentation (F1)" option in `ActiveMenu::File` and bind the `F1` key to toggle/open the Help window.
- **Independent Floating OS Window**: Use Iced multi-window capabilities (`iced::window::open` / `iced::window::close`) with clean window lifecycle handling (`help_window_id: Option<window::Id>`).
- **Comprehensive Documentation Content**: Document every single symbol, language construct, memory marker, simulation tool, fieldbus protocol, digital twin element, and shortcut across FerritePLC:
  1. **System Architecture & Scan Loop**: Deterministic 4-phase cycle (Inputs -> Tasks -> Outputs -> Diagnostics), Virtual Clock, Watchdog.
  2. **Ladder Diagram (LD)**: Rails, rungs, contacts (NO `--[ ]--`, NC `--[/]--`, Rising Edge `--[P]--`, Falling Edge `--[N]--`), coils (Normal `--( )--`, Set `--[S]--`, Reset `--[R]--`), parallel branches, branch dissolution, inline FB calls.
  3. **Sequential Function Chart (SFC / Grafcet)**: Initial steps (`[[ 0 ]]`), Normal steps (`[ 1 ]`), Macro steps, transitions & receptivities, Action Qualifiers (`N`, `S`, `R`, `L`, `D`, `P`), Divergence & Convergence rules (AND double bar `===`, OR single bar `—`), IEC 60848 evolution rules 1–5, introspection symbols (`X<i>.state`, `X<i>.t`).
  4. **Structured Text (ST)**: Expressions, operators, assignments (`:=`), control flow (`IF/THEN/ELSIF/ELSE/END_IF`, `CASE/OF/ELSE/END_CASE`, `FOR/TO/BY/DO/END_FOR`, `WHILE/DO/END_WHILE`), FB calls, math functions (`ABS`, `SQRT`, `SIN`, `COS`, `EXP`, etc.).
  5. **Function Block Diagram (FBD)**: Execution order, block headers, input/output pins, standard library blocks (`TON`, `TOF`, `TP`, `CTU`, `CTD`, `R_TRIG`, `F_TRIG`, `SR`, `RS`), PID correctors (Parallel, Series, Mixed/Ideal), link wiring rules.
  6. **Process Image & Memory Addressing**: `%I`, `%Q`, `%M` bit/byte/word/dword addressing, symbolic tag table, IEC data types (`BOOL`, `INT`, `DINT`, `REAL`, `TIME`, `STRING`).
  7. **Simulation & Commissioning**: Run/Pause, Single Step, scan cycle duration, Force/Watch table (`[!] Forces Active`), Retain snapshot persistence, ISA 18.2 Alarm lifecycle.
  8. **Fieldbus & Hardware HAL**: Modbus TCP server/client, OPC UA address space, MQTT telemetry & Change-of-Value engine, Raspberry Pi 40-pin GPIO pinout & level monitoring.
  9. **Digital Twin 2D Simulation**: Physics canvas, tanks, pumps, valves, sensors, conveyors, shared zero-deadlock memory snapshots.
  10. **Canvas Controls & Keyboard Shortcuts**: Pan, zoom, marquee box select, wire dragging, port snapping, auto-layout, deletion modals, undo/redo (`Ctrl+Z`, `Ctrl+Y`), file save/open (`Ctrl+S`, `Ctrl+O`).
- **Interactive Structured Navigator (Approach A)**: Left sidebar with categorized topics and real-time search filter; main pane with rich styled typography, visual symbol reference cards, syntax tables, and examples.
- **Bilingual Support**: Fully localized in English and French matching FerritePLC's language preference.

### Non-Goals
- External browser dependency (the manual is 100% self-contained and offline).
- Mutable wiki or user editing of built-in documentation.

### Anti-Goals
- Modal blocking of the main IDE (the window must float independently and allow continued interaction with the project).
- Hardcoded untranslated strings.
- Window management leaks or crashes on window close.

---

## 3. Approaches Considered

### Approach A: Structured Topic Navigator with Search & Symbol Reference Cards (Picked)
- Modular architecture with `help_window.rs`.
- Categorized topic hierarchy with search filtering.
- Visual symbol cards displaying ASCII/graphical representations, symbol name, IEC specification, parameters, and code examples.
- Full bilingual integration.

### Approach B: Embedded Scrollable Markdown Document Viewer (Rejected)
- Embeds single raw markdown file.
- Lacks fast structured topic jump, real-time search filtering, and custom styled symbol cards.

### Approach C: Flat Category Tabs (Rejected)
- Top button bar switching static views.
- Awkward for deep technical subtopics (e.g., trying to quickly find "PID Tuner Equations" or "SFC Action Qualifiers").

---

## 4. UI Layout & Visual Specifications

The Help window opens at 1024x740 px (resizable, minimum 700x500 px):

```text
+----------------------------------------------------------------------------------------------------+
|  [?] FerritePLC Studio Reference Manual & Symbol Guide                     [Lang: EN | FR]  [X]   |
+-------------------------------------+--------------------------------------------------------------+
|  [ Search documentation... (Ctrl+F) ] |  Topic Title: Ladder Diagram (LD) Reference                  |
+-------------------------------------+--------------------------------------------------------------+
|  v GETTING STARTED                  |  Overview                                                    |
|    - System Architecture & Scan     |  Ladder Diagram is a graphical programming language based   |
|    - Project Structure & Files      |  on relay circuit diagrams...                                |
|  v LADDER DIAGRAM (LD)              |                                                              |
|    - Contacts Reference             |  SYMBOL REFERENCE TABLE:                                     |
|    - Coils Reference                |  +-------------+------------------+-----------------------+  |
|    - Branches & Rung Logic          |  | Symbol      | Name             | Semantics             |  |
|  v SFC / GRAFCET                    |  +-------------+------------------+-----------------------+  |
|    - Steps & Introspection          |  | --[ ]--     | Normally Open    | Passes power if TRUE  |  |
|    - Action Qualifiers (N/S/R/L/D/P)|  | --[/]--     | Normally Closed  | Passes power if FALSE |  |
|    - Divergence & Convergence       |  | --[P]--     | Rising Edge      | 1 scan pulse on 0->1  |  |
|  v STRUCTURED TEXT (ST)             |  | --[N]--     | Falling Edge     | 1 scan pulse on 1->0  |  |
|    - Expressions & Operators        |  | --( )--     | Normal Coil      | Energizes rung result |  |
|    - Control Flow Statements        |  | --[S]--     | Set / Latch      | Latches TRUE until R  |  |
|  v FUNCTION BLOCK DIAGRAM (FBD)     |  | --[R]--     | Reset / Unlatch  | Unlatches to FALSE    |  |
|    - Standard Library Blocks        |  +-------------+------------------+-----------------------+  |
|    - PID Controllers (Parallel/Mix) |                                                              |
|  v MEMORY & ADDRESSING              |  Branch Logic & Power Flow:                                  |
|    - Direct Addressing (%I/%Q/%M)   |  Branches allow parallel circuits with automatic routing...  |
|    - Tag Table & IEC Data Types     |                                                              |
|  v SIMULATION & COMMISSIONING       |                                                              |
|    - Scan Cycle, Watch & Forces     |                                                              |
|  v FIELDBUS & HARDWARE              |                                                              |
|    - Modbus TCP, OPC UA, MQTT       |                                                              |
|    - Raspberry Pi GPIO Visualizer   |                                                              |
|  v DIGITAL TWIN & CANVAS            |                                                              |
|    - 2D Schematic Simulation        |                                                              |
|    - Canvas & Keyboard Shortcuts    |                                                              |
+-------------------------------------+--------------------------------------------------------------+
```

---

## 5. Technical Architecture & State Machine

### 5.1 State Machine in `StudioApp`
```rust
pub struct StudioApp {
    // Existing fields...
    pub help_window_id: Option<iced::window::Id>,
    pub help_selected_topic: HelpTopic,
    pub help_search_query: String,
}
```

### 5.2 Messages
```rust
pub enum Message {
    // Existing messages...
    OpenHelpWindow,
    CloseHelpWindow,
    SelectHelpTopic(HelpTopic),
    HelpSearchChanged(String),
}
```

### 5.3 Window Lifecycle & View Dispatch
1. **Open**:
   - `Message::OpenHelpWindow` checks `self.help_window_id`.
   - If `None`, calls `iced::window::open` with:
     ```rust
     iced::window::Settings {
         size: iced::Size::new(1040.0, 760.0),
         min_size: Some(iced::Size::new(720.0, 500.0)),
         exit_on_close_request: false,
         ..Default::default()
     }
     ```
   - Sets `self.help_window_id = Some(id)`.
2. **Close**:
   - In `iced::window::close_requested(id)` or explicit close button:
     If `Some(id) == self.help_window_id`, set `self.help_window_id = None` and emit `iced::window::close(id)`.
3. **View**:
   - In `pub fn view(&self, window_id: iced::window::Id) -> Element<'_, Message>`:
     If `Some(window_id) == self.help_window_id`, delegate to `self.view_help_window()`.

### 5.4 File Menu Integration
In `ActiveMenu::File` (`app.rs`):
- Add a separator line `|`.
- Add button: `text(self.ui_lang.tr("Help & Manual (F1)", "Aide & Manuel (F1)"))` emitting `Message::OpenHelpWindow`.

### 5.5 Global Keyboard Shortcuts
- In `handle_key_event`: Pressing `Key::Named(key::Named::F1)` triggers `Message::OpenHelpWindow`.

---

## 6. Verification Plan & Test Matrix

1. **Unit & Integration Tests**:
   - Test `HelpTopic` catalog completeness (assert every topic renders without panicking).
   - Test search filtering: searching for "contact", "receptivity", "TON", "%MW", "PID", "Modbus" returns relevant topics.
   - Test window dispatch: `view(help_window_id)` returns valid elements.
   - Test bilingual strings: English and French variants exist for all categories and topics.
2. **Workspace Verification Suite**:
   - `cargo check --workspace --all-targets`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - Test running GUI project check and simulation headless.
