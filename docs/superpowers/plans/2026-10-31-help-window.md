# Help Window & Comprehensive Symbol Guide Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a comprehensive, interactive in-application reference manual and symbol guide in an independent floating OS window, accessible from the File menu and via `F1`.

**Architecture:** A dedicated module `crates/ferrite-gui/src/help_window.rs` provides a categorized, searchable documentation model and view layout with styled symbol reference cards. `crates/ferrite-gui/src/app.rs` manages the independent floating window lifecycle via `iced::window::open` / `iced::window::close`, routing `view(window_id)`, and binding the File menu item and `F1` keyboard shortcut.

**Tech Stack:** Rust 2024 Edition, Iced (multi-window `iced::window`, widgets `column`, `row`, `container`, `scrollable`, `text`, `text_input`, `button`), `ferrite-gui` StudioTheme.

**Spec:** `docs/superpowers/specs/2026-10-31-help-window-design.md`

## Global Constraints

- Rust 2024 Edition.
- Zero `unwrap()` / `expect()` in library crates (`ferrite-core`, `ferrite-lang-*`, `ferrite-project`, `ferrite-runtime`).
- Zero clippy warnings with `cargo clippy --workspace --all-targets -- -D warnings`.
- Full bilingual English and French support using `StudioApp::ui_lang`.
- Non-blocking independent OS window (`iced::window::open` with `exit_on_close_request: false`).

## Review Focus

1. **Window Close & Re-open**: Closing the Help window via the OS 'X' button or in-app button properly resets `help_window_id` to `None` so subsequent presses of File -> Help or `F1` reliably reopen the window without leaking window IDs or crashing.
2. **Search Filtering Edge Cases**: Search queries with uppercase/lowercase, whitespace, punctuation (e.g. `"--[ ]--"`, `"%MW"`, `"N"`, `"TON"`), or unmatched strings gracefully filter topics without panic or empty unusable states.
3. **Multi-Window Interaction Safety**: Interacting with the Help window while simulation is actively ticking or stepping in the main window does not cause data races or UI freezes.
4. **Symbol & Grammar Precision**: The documented IEC 60848 convergence/divergence rules, LD contact/coil behaviors, ST statements, and FBD standard block signatures accurately match FerritePLC's compiler implementations.
5. **Layout Resizability**: The Help window renders cleanly without clipping across window sizes from minimum 720x500 up to full screen.

---

### Task 1: Help Topics Catalog & Search Engine Data Model

**Files:**
- Create: `crates/ferrite-gui/src/help_window.rs`
- Modify: `crates/ferrite-gui/src/lib.rs`
- Test: `crates/ferrite-gui/src/help_window.rs:tests`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum HelpCategory {
      Architecture,
      LadderDiagram,
      SfcGrafcet,
      StructuredText,
      FunctionBlockDiagram,
      MemoryAddressing,
      Simulation,
      FieldbusHardware,
      DigitalTwin,
      Shortcuts,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum HelpTopic {
      ArchScanLoop,
      ArchProjectFiles,
      LdContacts,
      LdCoils,
      LdBranches,
      SfcSteps,
      SfcActions,
      SfcDivergenceConvergence,
      SfcIntrospection,
      StSyntax,
      StControlFlow,
      StMathFunctions,
      FbdStandardBlocks,
      FbdPidController,
      MemDirectAddressing,
      MemTagTable,
      SimExecutionAndForces,
      SimAlarmsAndRetain,
      FieldbusProtocols,
      HardwareRpiGpio,
      DigitalTwinSimulation,
      CanvasShortcuts,
  }

  pub struct HelpTopicMeta {
      pub id: HelpTopic,
      pub category: HelpCategory,
      pub title_en: &'static str,
      pub title_fr: &'static str,
      pub keywords: &'static [&'static str],
  }

  pub fn all_topics() -> &'static [HelpTopicMeta];
  pub fn filter_topics(query: &str) -> Vec<HelpTopic>;
  ```

- [ ] **Step 1: Write the failing tests for Help data model & search**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_catalog_completeness() {
        let topics = all_topics();
        assert!(topics.len() >= 20);
        for t in topics {
            assert!(!t.title_en.is_empty());
            assert!(!t.title_fr.is_empty());
            assert!(!t.keywords.is_empty());
        }
    }

    #[test]
    fn test_help_filter_topics_by_query() {
        let ld_results = filter_topics("contact");
        assert!(ld_results.contains(&HelpTopic::LdContacts));

        let sfc_results = filter_topics("receptivity");
        assert!(sfc_results.contains(&HelpTopic::SfcDivergenceConvergence) || sfc_results.contains(&HelpTopic::SfcSteps));

        let mem_results = filter_topics("%MW");
        assert!(mem_results.contains(&HelpTopic::MemDirectAddressing));

        let empty_results = filter_topics("xyznonexistentterm123");
        assert!(empty_results.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-gui test_help_catalog_completeness`
Expected: FAIL with unresolved module/items.

- [ ] **Step 3: Implement `HelpCategory`, `HelpTopic`, metadata catalog, and search in `crates/ferrite-gui/src/help_window.rs`**

Register module in `crates/ferrite-gui/src/lib.rs`. Implement category listings and topic definitions containing all 22 topics covering architecture, LD, SFC, ST, FBD, Memory, Sim, Fieldbus, Digital Twin, and Shortcuts. Implement `filter_topics` with case-insensitive matching across titles and keywords.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ferrite-gui test_help_catalog_completeness test_help_filter_topics_by_query`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/help_window.rs crates/ferrite-gui/src/lib.rs
git commit -m "feat(gui): implement help catalog data model and search engine"
```

---

### Task 2: Comprehensive Documentation Content & Topic Views

**Files:**
- Modify: `crates/ferrite-gui/src/help_window.rs`
- Test: `crates/ferrite-gui/src/help_window.rs:tests`

**Interfaces:**
- Produces:
  ```rust
  pub fn render_topic_content<'a, Message: 'a>(
      topic: HelpTopic,
      lang: crate::i18n::UiLanguage,
  ) -> iced::Element<'a, Message>;
  ```

- [ ] **Step 1: Write the failing test for topic content rendering**

```rust
#[test]
fn test_all_topics_render_without_panic() {
    let languages = [crate::i18n::UiLanguage::English, crate::i18n::UiLanguage::French];
    for topic_meta in all_topics() {
        for lang in languages {
            let _el: iced::Element<'_, ()> = render_topic_content(topic_meta.id, lang);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-gui test_all_topics_render_without_panic`
Expected: FAIL with function missing.

- [ ] **Step 3: Implement comprehensive topic rendering and symbol cards**

In `crates/ferrite-gui/src/help_window.rs`:
1. Build helper functions for rendering:
   - Section headers & lead paragraphs.
   - Symbol reference cards (glyph container, name badge, semantic description, behavioral notes, and code example).
   - Reference comparison tables (e.g., contacts NO/NC/P/N, coils Normal/Set/Reset, SFC action qualifiers N/S/R/L/D/P, divergence/convergence AND/OR rules, FBD standard timer/counter blocks, `%I/%Q/%M` memory specifiers).
2. Write comprehensive, in-depth documentation bodies for every topic:
   - **`ArchScanLoop`**: 4 deterministic scan phases, cycle time calculation, watchdog limits.
   - **`ArchProjectFiles`**: Manifest `.fplc`, `symbols/global_tags.toml`, `symbols/fb_instances.toml`, `pous/*.ld.json`, `pous/*.sfc.json`, `pous/*.st`, `pous/*.fbd.json`, `sim/scenarios.toml`.
   - **`LdContacts`**: `--[ ]--`, `--[/]--`, `--[P]--`, `--[N]--` with timing diagrams and scan behavior.
   - **`LdCoils`**: `--( )--`, `--[S]--`, `--[R]--` with latch/unlatch lifecycle.
   - **`LdBranches`**: Parallel OR branches, nested rungs, power rail flow, branch dissolution.
   - **`SfcSteps`**: Initial step `[[ 0 ]]`, Normal step `[ 1 ]`, Macro step, step activations.
   - **`SfcActions`**: Qualifiers `N` (Non-stored), `S` (Set/Stored), `R` (Reset), `L` (Time Limited), `D` (Time Delayed), `P` (Pulse), target tags and inline ST statements.
   - **`SfcDivergenceConvergence`**: AND Divergence/Convergence (double bar `===`), OR Divergence/Convergence (single bar `—`), IEC 60848 evolution rules 1–5.
   - **`SfcIntrospection`**: `X<i>.state` (`BOOL`), `X<i>.t` (`TIME`), cross-language reading and write protection.
   - **`StSyntax`**: IEC 61131-3 syntax, arithmetic (`+`, `-`, `*`, `/`, `MOD`, `**`), bitwise (`AND`, `OR`, `XOR`, `NOT`), relational (`=`, `<>`, `<`, `<=`, `>`, `>=`).
   - **`StControlFlow`**: `IF/THEN/ELSIF/ELSE/END_IF`, `CASE/OF/ELSE/END_CASE`, `FOR/TO/BY/DO/END_FOR`, `WHILE/DO/END_WHILE`.
   - **`StMathFunctions`**: `ABS`, `SQRT`, `LN`, `LOG`, `EXP`, `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`.
   - **`FbdStandardBlocks`**: `TON`, `TOF`, `TP`, `CTU`, `CTD`, `R_TRIG`, `F_TRIG`, `SR`, `RS` with input/output pin definitions.
   - **`FbdPidController`**: Parallel, Series, and Mixed PID forms, gain parameters (`Kp`, `Ti`, `Td`), anti-windup.
   - **`MemDirectAddressing`**: `%I` inputs, `%Q` outputs, `%M` markers; `%X` bit, `%B` byte, `%W` word, `%D` dword; examples `%IX0.0`, `%QW2`, `%MD10`.
   - **`MemTagTable`**: Symbolic identifiers, data types (`BOOL`, `INT`, `DINT`, `REAL`, `TIME`, `STRING`), retained variables.
   - **`SimExecutionAndForces`**: Run, Pause, Single Step, scan cycle duration slider, Watch table, Forcing inputs/outputs, `[!] Forces Active` banner.
   - **`SimAlarmsAndRetain`**: ISA 18.2 alarm states (Normal, Unacknowledged, Acknowledged, Suppressed), Retain memory snapshots.
   - **`FieldbusProtocols`**: Modbus TCP (server port 502, coils/registers mapping), OPC UA address space, MQTT telemetry publishing.
   - **`HardwareRpiGpio`**: Raspberry Pi 40-pin header interactive diagram, GPIO input/output pin styling.
   - **`DigitalTwinSimulation`**: 2D physics schematic canvas, tank levels, valve states, pump flows, sensor triggers.
   - **`CanvasShortcuts`**: Marquee drag selection, pan & zoom, auto-layout, wire dragging, port snapping, element deletion modals, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+S`, `Ctrl+O`, `F1`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ferrite-gui test_all_topics_render_without_panic`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/help_window.rs
git commit -m "feat(gui): provide comprehensive documentation content and symbol reference cards"
```

---

### Task 3: Help Window Shell View & Multi-Window Integration in `StudioApp`

**Files:**
- Modify: `crates/ferrite-gui/src/help_window.rs`
- Modify: `crates/ferrite-gui/src/app.rs`
- Test: `crates/ferrite-gui/src/app.rs:tests`

**Interfaces:**
- Consumes: `all_topics`, `filter_topics`, `render_topic_content`
- Produces in `crates/ferrite-gui/src/app.rs`:
  ```rust
  // Messages
  Message::OpenHelpWindow,
  Message::CloseHelpWindow,
  Message::SelectHelpTopic(HelpTopic),
  Message::HelpSearchChanged(String),

  // Methods
  pub fn view_help_window(&self) -> Element<'_, Message>;
  ```

- [ ] **Step 1: Write the failing tests for Help window message routing and lifecycle**

In `crates/ferrite-gui/src/app.rs`:
```rust
#[test]
fn test_help_window_messages_and_state() {
    let mut app = StudioApp::new_test_app();
    assert!(app.help_window_id.is_none());
    assert_eq!(app.help_selected_topic, crate::help_window::HelpTopic::ArchScanLoop);
    assert_eq!(app.help_search_query, "");

    // Test Search input
    let _ = app.update(Message::HelpSearchChanged("ladder".to_string()));
    assert_eq!(app.help_search_query, "ladder");

    // Test Select topic
    let _ = app.update(Message::SelectHelpTopic(crate::help_window::HelpTopic::LdContacts));
    assert_eq!(app.help_selected_topic, crate::help_window::HelpTopic::LdContacts);

    // Test view rendering
    let view_el = app.view_help_window();
    let _ = view_el;
}

#[test]
fn test_f1_key_maps_to_open_help_window() {
    let key_event = iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
        modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
        modifiers: iced::keyboard::Modifiers::default(),
        physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::F1),
        location: iced::keyboard::key::Location::Standard,
    };
    let msg = StudioApp::handle_key_event(key_event);
    assert_eq!(msg, Some(Message::OpenHelpWindow));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-gui test_help_window_messages_and_state test_f1_key_maps_to_open_help_window`
Expected: FAIL with missing fields/variants.

- [ ] **Step 3: Add State, Messages, Window Lifecycle, Menu Button, and F1 Key Handler**

1. In `crates/ferrite-gui/src/app.rs`:
   - Add fields to `StudioApp`:
     - `pub help_window_id: Option<iced::window::Id>`
     - `pub help_selected_topic: crate::help_window::HelpTopic`
     - `pub help_search_query: String`
   - Initialize in `StudioApp::new`:
     - `help_window_id: None`
     - `help_selected_topic: crate::help_window::HelpTopic::ArchScanLoop`
     - `help_search_query: String::new()`
   - Add `Message` variants:
     - `OpenHelpWindow`, `CloseHelpWindow`, `SelectHelpTopic(HelpTopic)`, `HelpSearchChanged(String)`.
   - In `StudioApp::update`:
     - `Message::OpenHelpWindow`:
       If `self.help_window_id.is_none()`, call `iced::window::open` with size 1040x760, min 720x500, `exit_on_close_request: false`. Store ID and return open task. If already open, return `Task::none()`.
     - `Message::CloseHelpWindow`:
       If let Some(id) = self.help_window_id.take(), return `iced::window::close(id)`.
     - `Message::SelectHelpTopic(topic)`:
       Set `self.help_selected_topic = topic`.
     - `Message::HelpSearchChanged(query)`:
       Set `self.help_search_query = query`.
     - In `iced::Event::Window(id, iced::window::Event::CloseRequested)`:
       If `Some(id) == self.help_window_id`, set `self.help_window_id = None; iced::window::close(id)`.
   - In `StudioApp::view(window_id)`:
     If `Some(window_id) == self.help_window_id`, return `self.view_help_window()`.
   - In `ActiveMenu::File`:
     Add button: `text(self.ui_lang.tr("Help & Manual (F1)", "Aide & Manuel (F1)"))` triggering `Message::OpenHelpWindow`.
   - In keyboard handler:
     Map `Key::Named(key::Named::F1)` to `Some(Message::OpenHelpWindow)`.
2. In `crates/ferrite-gui/src/help_window.rs`:
   - Implement `view_help_window(&StudioApp) -> Element<'_, Message>` with a two-pane layout:
     - Top bar: Title badge, search input (`Message::HelpSearchChanged`), close button (`Message::CloseHelpWindow`).
     - Left column: Category headers and topic buttons (`Message::SelectHelpTopic`), with highlight on active topic and badge for matched search results.
     - Right column: Scrollable area hosting `render_topic_content`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ferrite-gui test_help_window_messages_and_state test_f1_key_maps_to_open_help_window`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/app.rs crates/ferrite-gui/src/help_window.rs
git commit -m "feat(gui): integrate floating help window with file menu, F1 shortcut, and view dispatcher"
```

---

### Task 4: Workspace Verification & Quality Gates

**Files:**
- Test all crates across the workspace.

- [ ] **Step 1: Run full workspace checks**

Run: `cargo check --workspace --all-targets`
Expected: PASS with 0 errors.

- [ ] **Step 2: Run all workspace unit and integration tests**

Run: `cargo test --workspace`
Expected: PASS (all 93+ tests pass).

- [ ] **Step 3: Run Clippy linter**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS (clean, zero warnings).

- [ ] **Step 4: Verify headless simulation of both example projects**

Run: `cargo run -p ferrite-cli -- check -p examples/my_automation_plant`
Run: `cargo run -p ferrite-cli -- sim --headless -p examples/my_automation_plant`
Expected: PASS with 8 scans and 39 assertions verified.

- [ ] **Step 5: Commit any final refinements**

```bash
git add .
git commit -m "chore(gui): pass verification gates for help window and reference guide"
```
