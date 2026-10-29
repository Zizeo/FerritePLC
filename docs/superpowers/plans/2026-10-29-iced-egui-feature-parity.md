# Iced UI Feature Parity with egui (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore missing core productivity and simulation features from the `egui` branch to the modern `Iced-UI` branch: native OS file dialogs & PLCopen TC6 XML import/export (`rfd`), global keyboard shortcuts (`Ctrl+S`, `Ctrl+O`, `Ctrl+Z`, `Ctrl+Y`), structured top header action menus (`File`, `PLC`, `Panels`, `Options`), an interactive 40-pin Raspberry Pi GPIO header map, and Modbus TCP telemetry with address mapping reference tables.

**Architecture:** Pure Elm Architecture (TEA) in `ferrite-gui`: file dialogs dispatch via `rfd` and feed into typed `Message` variants in `update()`; keyboard shortcuts are wired through `iced::keyboard::on_key_press` in `subscription()`; top header menus render sleek context popovers via an `active_menu: Option<ActiveMenu>` state machine; the GPIO 40-pin header and Modbus telemetry are rendered as responsive Iced widget trees in their respective bottom dock tabs.

**Tech Stack:** Rust 2024 Edition, `iced` 0.13, `rfd` 0.15, `ferrite-project`, `ferrite-runtime`, `ferrite-hal`, `ferrite-fieldbus-modbus`.

**Spec:** [`docs/superpowers/specs/2026-10-29-iced-egui-feature-parity-design.md`](file:///C:/Users/Elio/prog/FerritePLC/docs/superpowers/specs/2026-10-29-iced-egui-feature-parity-design.md)

## Global Constraints
- Zero `unwrap()` / `expect()` in library crates (`thiserror` throughout).
- All UI state transitions flow through `Message` in `update(&mut self, message: Message) -> Task<Message>`.
- File dialog cancellations must be clean no-ops with zero panic.
- Pass `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`.

## Review Focus
1. **File Dialog Cancellation & Invalid File Handling**: Canceling an OS folder or file picker dialog (or selecting a corrupt `.fplc` or invalid PLCopen `.xml`) must safely return a clean error in `status_banner` without clearing the current active project or runtime.
2. **Keyboard Focus Guarding**: Global shortcuts (`Ctrl+Z`, `Ctrl+S`, etc.) must not steal keystrokes or disrupt text typing while the user is editing Structured Text (`text_editor`) or naming a tag (`text_input`).
3. **Menu Dismissal & Overlay Z-Order**: Opening an `ActiveMenu` popover must dismiss any other open menu, and selecting any menu item or clicking outside must cleanly dismiss the popover.
4. **Interactive Mock GPIO Bounds**: Clicking any 40-pin header pin that is not a configured input pin (e.g. Ground, 5V, 3.3V, or an unmapped GPIO) must not trigger invalid hardware writes or panic.
5. **Modbus Stopped vs. Running Telemetry**: When the Modbus TCP server is stopped (`modbus_server == None`), the telemetry section must cleanly hide or display `[STOPPED]` without attempting to unwrap server metrics.

---

### Task 1: Native OS Project File Operations & PLCopen TC6 XML (`rfd`)

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:145-165, 390-440, 2460-2520`
- Test: `crates/ferrite-gui/src/tests.rs` (or inside `crates/ferrite-gui/src/lib.rs` / `app.rs`)

**Interfaces:**
- Consumes: `rfd::FileDialog`, `Project::load`, `Project::save`, `Project::export_plcopen_xml`, `Project::import_plcopen_xml`.
- Produces:
  - `Message::OpenProjectFolderDialog`
  - `Message::OpenProjectManifestDialog`
  - `Message::SaveProjectAsDialog`
  - `Message::ExportPlcopenXmlDialog`
  - `Message::ImportPlcopenXmlDialog`
  - `Message::LoadProjectFromPath(PathBuf)`

- [ ] **Step 1: Write the failing test for `LoadProjectFromPath`**

In `crates/ferrite-gui/src/app.rs` or `crates/ferrite-gui/src/tests.rs`:
```rust
#[test]
fn test_load_project_from_path_and_plcopen_roundtrip() {
    let temp_dir = std::env::temp_dir().join("ferrite_test_load_path");
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    let scaffold = ferrite_project::Project::scaffold("test_load", &temp_dir).unwrap();
    scaffold.save(&temp_dir).unwrap();

    let mut app = FerriteStudioApp::new(scaffold);
    // Mutate and test LoadProjectFromPath
    let _ = app.update(Message::LoadProjectFromPath(temp_dir.clone()));
    assert_eq!(app.project.manifest.name, "test_load");
    assert!(app.status_banner.contains("test_load"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-gui --lib test_load_project_from_path`
Expected: FAIL with "no variant or associated item named `LoadProjectFromPath` in `Message`".

- [ ] **Step 3: Implement file dialog messages and handlers in `crates/ferrite-gui/src/app.rs`**

1. In `Message`:
```rust
    // Project & File Operations
    OpenProjectFolderDialog,
    OpenProjectManifestDialog,
    SaveProjectAsDialog,
    ExportPlcopenXmlDialog,
    ImportPlcopenXmlDialog,
    LoadProjectFromPath(std::path::PathBuf),
```

2. In `update(&mut self, message: Message)`:
Implement `LoadProjectFromPath(raw_path)`:
```rust
    Message::LoadProjectFromPath(raw_path) => {
        let path = if raw_path.is_file() {
            raw_path.parent().unwrap_or(&raw_path).to_path_buf()
        } else {
            raw_path
        };
        match Project::load(&path) {
            Ok(loaded) => {
                let p_name = loaded.manifest.name.clone();
                self.project = loaded;
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.last_committed_project = self.project.clone();
                self.active_tab = self.project.manifest.pous.first()
                    .map(|p| WorkspaceTab::Pou(p.name.clone()))
                    .unwrap_or(WorkspaceTab::SymbolsAndConfig);
                self.recompile_and_reset();
                self.status_banner = format!("Project '{p_name}' successfully loaded from {}", path.display());
            }
            Err(e) => {
                self.status_banner = format!("Failed to load project from {}: {e}", path.display());
            }
        }
        Task::none()
    }
```

Implement `OpenProjectFolderDialog`:
```rust
    Message::OpenProjectFolderDialog => {
        let mut dialog = rfd::FileDialog::new().set_title(self.ui_lang.tr(
            "Select FerritePLC Project Folder",
            "Sélectionner le Dossier du Projet FerritePLC",
        ));
        if self.project.root_dir.exists() {
            dialog = dialog.set_directory(&self.project.root_dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            return self.update(Message::LoadProjectFromPath(folder));
        }
        Task::none()
    }
```

Implement `OpenProjectManifestDialog`:
```rust
    Message::OpenProjectManifestDialog => {
        let mut dialog = rfd::FileDialog::new()
            .set_title(self.ui_lang.tr(
                "Open FerriteProject.fplc Manifest",
                "Ouvrir le Fichier Manifeste FerriteProject.fplc",
            ))
            .add_filter("FerritePLC Manifest (.fplc)", &["fplc"]);
        if self.project.root_dir.exists() {
            dialog = dialog.set_directory(&self.project.root_dir);
        }
        if let Some(file) = dialog.pick_file() {
            if let Some(parent) = file.parent() {
                return self.update(Message::LoadProjectFromPath(parent.to_path_buf()));
            }
        }
        Task::none()
    }
```

Implement `SaveProjectAsDialog`:
```rust
    Message::SaveProjectAsDialog => {
        let mut dialog = rfd::FileDialog::new().set_title(self.ui_lang.tr(
            "Save FerritePLC Project to Folder",
            "Enregistrer le Projet FerritePLC dans un Dossier",
        ));
        if self.project.root_dir.exists() {
            dialog = dialog.set_directory(&self.project.root_dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.project.root_dir = folder.clone();
            match self.project.save(&folder) {
                Ok(()) => {
                    self.modified = false;
                    self.status_banner = format!("Project saved to {}", folder.display());
                }
                Err(e) => {
                    self.status_banner = format!("Save As failed: {e}");
                }
            }
        }
        Task::none()
    }
```

Implement `ExportPlcopenXmlDialog`:
```rust
    Message::ExportPlcopenXmlDialog => {
        let file = rfd::FileDialog::new()
            .set_title(self.ui_lang.tr("Export PLCopen TC6 XML", "Exporter PLCopen TC6 XML"))
            .add_filter("PLCopen XML (.xml)", &["xml"])
            .set_file_name("project_plcopen.xml")
            .save_file();
        if let Some(path) = file {
            match self.project.export_plcopen_xml() {
                Ok(xml) => match std::fs::write(&path, xml) {
                    Ok(()) => {
                        self.status_banner = format!("PLCopen XML exported to {}", path.display());
                    }
                    Err(e) => {
                        self.status_banner = format!("Failed to write XML file: {e}");
                    }
                },
                Err(e) => {
                    self.status_banner = format!("PLCopen XML export error: {e}");
                }
            }
        }
        Task::none()
    }
```

Implement `ImportPlcopenXmlDialog`:
```rust
    Message::ImportPlcopenXmlDialog => {
        let file = rfd::FileDialog::new()
            .set_title(self.ui_lang.tr("Import PLCopen TC6 XML", "Importer PLCopen TC6 XML"))
            .add_filter("PLCopen XML (.xml)", &["xml"])
            .pick_file();
        if let Some(xml_path) = file {
            let target_folder = rfd::FileDialog::new()
                .set_title(self.ui_lang.tr(
                    "Select Target Directory for Imported Project",
                    "Sélectionner le Répertoire Cible pour le Projet Importé",
                ))
                .pick_folder();
            if let Some(target_dir) = target_folder {
                match std::fs::read_to_string(&xml_path) {
                    Ok(xml_content) => match Project::import_plcopen_xml(&xml_content, &target_dir) {
                        Ok(imported_project) => {
                            let p_name = imported_project.manifest.name.clone();
                            self.project = imported_project;
                            self.undo_stack.clear();
                            self.redo_stack.clear();
                            self.recompile_and_reset();
                            self.status_banner = format!(
                                "Project '{p_name}' successfully imported from PLCopen XML to {}",
                                target_dir.display()
                            );
                        }
                        Err(e) => {
                            self.status_banner = format!("PLCopen import failed: {e}");
                        }
                    },
                    Err(e) => {
                        self.status_banner = format!("Failed to read XML file: {e}");
                    }
                }
            }
        }
        Task::none()
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ferrite-gui --lib test_load_project_from_path`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/app.rs crates/ferrite-gui/src/lib.rs
git commit -m "feat(gui): implement native OS file dialogs and PLCopen TC6 XML import/export"
```

---

### Task 2: Global Keyboard Shortcuts Subscription

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:340-380` (inside `pub fn subscription`)
- Test: `crates/ferrite-gui/src/tests.rs` (unit test shortcut mapping)

**Interfaces:**
- Consumes: `iced::keyboard::on_key_press`, `modifiers.command()`.
- Produces: Dispatches `Message::SaveProject`, `Message::OpenProjectFolderDialog`, `Message::Undo`, `Message::Redo`, `Message::SfcRequestDeleteSelected`.

- [ ] **Step 1: Write the failing test for keyboard shortcut mapping helper**

In `crates/ferrite-gui/src/tests.rs` or `crates/ferrite-gui/src/app.rs`:
```rust
#[test]
fn test_keyboard_shortcut_mapping() {
    let cmd_s = handle_keyboard_shortcut(&iced::keyboard::Key::Character("s".into()), iced::keyboard::Modifiers::COMMAND);
    assert!(matches!(cmd_s, Some(Message::SaveProject)));

    let cmd_o = handle_keyboard_shortcut(&iced::keyboard::Key::Character("o".into()), iced::keyboard::Modifiers::COMMAND);
    assert!(matches!(cmd_o, Some(Message::OpenProjectFolderDialog)));

    let cmd_z = handle_keyboard_shortcut(&iced::keyboard::Key::Character("z".into()), iced::keyboard::Modifiers::COMMAND);
    assert!(matches!(cmd_z, Some(Message::Undo)));

    let cmd_shift_z = handle_keyboard_shortcut(
        &iced::keyboard::Key::Character("z".into()),
        iced::keyboard::Modifiers::COMMAND | iced::keyboard::Modifiers::SHIFT,
    );
    assert!(matches!(cmd_shift_z, Some(Message::Redo)));

    let cmd_y = handle_keyboard_shortcut(&iced::keyboard::Key::Character("y".into()), iced::keyboard::Modifiers::COMMAND);
    assert!(matches!(cmd_y, Some(Message::Redo)));

    let del = handle_keyboard_shortcut(&iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete), iced::keyboard::Modifiers::empty());
    assert!(matches!(del, Some(Message::SfcRequestDeleteSelected(_))));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-gui --lib test_keyboard_shortcut_mapping`
Expected: FAIL with "cannot find function `handle_keyboard_shortcut`".

- [ ] **Step 3: Implement `handle_keyboard_shortcut` and update `subscription`**

In `crates/ferrite-gui/src/app.rs`:
```rust
pub fn handle_keyboard_shortcut(
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Message> {
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
}
```

In `pub fn subscription(&self) -> iced::Subscription<Message>`:
Replace `key_sub` with:
```rust
        let key_sub = iced::keyboard::on_key_press(|key, modifiers| {
            handle_keyboard_shortcut(&key, modifiers)
        });
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ferrite-gui --lib test_keyboard_shortcut_mapping`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/app.rs
git commit -m "feat(gui): wire global keyboard shortcuts for Save, Open, Undo, Redo, and Delete"
```

---

### Task 3: Structured Header Action Menus (`ActiveMenu`)

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:80-140, 2400-2450, 2800-3050`
- Test: `cargo check -p ferrite-gui`

**Interfaces:**
- Consumes: `ActiveMenu`, `Message::ToggleActiveMenu(Option<ActiveMenu>)`.
- Produces: Interactive header buttons (`File ▾`, `PLC ▾`, `Panels ▾`, `Options ▾`) that toggle structured dropdown overlays for all studio features.

- [ ] **Step 1: Add `ActiveMenu` and `Message::ToggleActiveMenu`**

In `crates/ferrite-gui/src/app.rs`:
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
(Initialize to `None` in `new()` / `default()`).

Add to `Message`:
`ToggleActiveMenu(Option<ActiveMenu>),`

In `update`:
```rust
    Message::ToggleActiveMenu(menu) => {
        if self.active_menu == menu {
            self.active_menu = None;
        } else {
            self.active_menu = menu;
        }
        Task::none()
    }
```
And whenever an action inside a menu is pressed (e.g. `SaveProject`, `SetTheme`, etc.), clear `self.active_menu = None`.

- [ ] **Step 2: Render Top Header Menu Buttons and Popovers in `view_header`**

In `view_header(&self)`:
1. Render structured menu triggers in the top-left brand section:
   - Button `text("File ▾")` -> `on_press(Message::ToggleActiveMenu(Some(ActiveMenu::File)))`
   - Button `text("PLC ▾")` -> `on_press(Message::ToggleActiveMenu(Some(ActiveMenu::Plc)))`
   - Button `text("Panels ▾")` -> `on_press(Message::ToggleActiveMenu(Some(ActiveMenu::Panels)))`
   - Button `text("Options ▾")` -> `on_press(Message::ToggleActiveMenu(Some(ActiveMenu::Options)))`
2. If `let Some(active) = self.active_menu`, render an elevated card container beneath the header containing the items:
   - For `ActiveMenu::File`:
     - Save (`Ctrl+S`), Save As...
     - Open Folder... (`Ctrl+O`), Open Manifest (.fplc)...
     - Export PLCopen XML..., Import PLCopen XML...
     - Global Tags & Config, Exit Studio.
   - For `ActiveMenu::Plc`:
     - Run / Pause, Single Step, Warm Reset, Cold Reset, Check & Compile, Run Scenarios.
   - For `ActiveMenu::Panels`:
     - Toggle Explorer, Toggle I/O Rack, Toggle Bottom Dock.
     - Detach Explorer, Detach I/O Rack, Detach Bottom Dock.
   - For `ActiveMenu::Options`:
     - Language: English / Français.
     - Theme: Dark / Light.

- [ ] **Step 3: Verify build with cargo check**

Run: `cargo check -p ferrite-gui`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add crates/ferrite-gui/src/app.rs
git commit -m "feat(gui): add structured top header action menus for File, PLC, Panels, and Options"
```

---

### Task 4: Raspberry Pi 40-Pin Interactive GPIO Header Map

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:3480-3550` (inside `BottomDockTab::Gpio` view)
- Test: `crates/ferrite-gui/src/tests.rs`

**Interfaces:**
- Consumes: `ferrite_hal::RPI_40PIN_HEADER`, `self.hal_config`, `self.hal_driver`, `self.hal_enabled`.
- Produces: 20-row × 2-column styled layout of Raspberry Pi pins with color-coded badges, mapped PLC symbols, and clickable signal toggles (`Message::MockGpioPinToggled(bcm)`).

- [ ] **Step 1: Write unit test for 40-pin GPIO helper styling**

In `crates/ferrite-gui/src/tests.rs` or `crates/ferrite-gui/src/app.rs`:
```rust
#[test]
fn test_rpi_40pin_header_pin_styling() {
    let pin_1 = &ferrite_hal::RPI_40PIN_HEADER[0]; // 3.3V Power
    assert_eq!(pin_1.pin_num, 1);
    assert_eq!(pin_1.name, "3.3V Power");
    assert!(pin_1.bcm.is_none());

    let pin_11 = &ferrite_hal::RPI_40PIN_HEADER[10]; // GPIO 17
    assert_eq!(pin_11.pin_num, 11);
    assert_eq!(pin_11.bcm, Some(17));
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p ferrite-gui --lib test_rpi_40pin`
Expected: PASS.

- [ ] **Step 3: Implement the 40-pin header map in `BottomDockTab::Gpio`**

In `crates/ferrite-gui/src/app.rs` under `BottomDockTab::Gpio`:
1. Top status row:
   - `HAL Enabled` toggle button (`Message::ToggleHalEnabled`).
   - Active Driver name label (`self.hal_driver.name()`).
2. Pin layout:
   - Read `live_levels = self.hal_driver.pin_levels()`.
   - Build a 20-row scrollable grid:
     - For `pair_idx` in `0..20`:
       - `odd_pin = &RPI_40PIN_HEADER[pair_idx * 2]`;
       - `even_pin = &RPI_40PIN_HEADER[pair_idx * 2 + 1]`;
       - Format odd pin widget (left-aligned): Name, Pin #, Mapped Address (`hal_config.inputs/outputs`), electrical state badge (Emerald if High, Slate if Low).
       - Center divider with pin numbers `[odd_num] | [even_num]`.
       - Format even pin widget (right-aligned).
       - If pin is a GPIO input (`pin.bcm.is_some()`), wrap in a `button()` on_press `Message::MockGpioPinToggled(bcm)` so clicking toggles simulated physical logic level.

- [ ] **Step 4: Verify build with cargo check**

Run: `cargo check -p ferrite-gui`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/app.rs
git commit -m "feat(gui): render interactive 40-pin Raspberry Pi GPIO header in hardware dock"
```

---

### Task 5: Modbus TCP Live Telemetry & Reference Table

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:3430-3475` (inside `BottomDockTab::Modbus` view)
- Test: `cargo check -p ferrite-gui`

**Interfaces:**
- Consumes: `ModbusTcpServer::telemetry()`.
- Produces: Live stats banner (Requests, Active Clients, Exceptions) and styled IEC 61131-3 address mapping reference table.

- [ ] **Step 1: Implement live telemetry and IEC reference table in `BottomDockTab::Modbus`**

In `crates/ferrite-gui/src/app.rs` under `BottomDockTab::Modbus`:
1. If `let Some(ref srv) = self.modbus_server`:
   - Query `telem = srv.telemetry()`.
   - Render telemetry badges:
     - `Requests Total: {telem.requests_total}`
     - `Active Clients: {telem.active_clients}`
     - `Exceptions: {telem.exceptions_total}`
2. IEC 61131-3 Reference Mapping Table:
   - Header: `Modbus Register Range | Function Code | FerritePLC Address | Access Mode`
   - Row 1: `00001 - 01000 | FC01, FC05, FC15 | %QX0.0 .. %QX124.7 | Read / Write`
   - Row 2: `01001 - 02000 | FC01, FC05, FC15 | %MX0.0 .. %MX124.7 | Read / Write`
   - Row 3: `10001 - 11000 | FC02             | %IX0.0 .. %IX124.7 | Read Only`
   - Row 4: `30001 - 31000 | FC04             | %IW0 .. %IW999     | Read Only`
   - Row 5: `40001 - 41000 | FC03, FC06, FC16 | %QW0 .. %QW999     | Read / Write`
   - Row 6: `41001 - 42000 | FC03, FC06, FC16 | %MW0 .. %MW999     | Read / Write`

- [ ] **Step 2: Verify build with cargo check**

Run: `cargo check -p ferrite-gui`
Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
git add crates/ferrite-gui/src/app.rs
git commit -m "feat(gui): display Modbus TCP live telemetry and IEC 61131-3 mapping reference table"
```

---

### Task 6: Full Workspace Verification & Clippy Compliance

**Files:**
- Modify: none (verification only)

**Interfaces:**
- Consumes: All 14 crates in the FerritePLC workspace.
- Produces: 100% green compiler, test suite, and linter check.

- [ ] **Step 1: Run workspace compilation check**

Run: `cargo check --workspace --all-targets`
Expected: 0 errors.

- [ ] **Step 2: Run all workspace tests**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 3: Run workspace clippy with `-D warnings`**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 4: Commit final verification**

```bash
git commit --allow-empty -m "chore(release): verify Iced UI feature parity Phase 1 across workspace"
```
