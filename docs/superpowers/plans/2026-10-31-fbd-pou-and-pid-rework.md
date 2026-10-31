# FBD POU Overhaul: Renaming, Reordering & Advanced PID/PI Controller Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Overhaul FBD and POU management in FerritePLC Studio by enabling POU renaming/reordering in the project explorer, block renaming/reordering on the FBD canvas with automated link rewiring, and introducing the full industrial PID/PI corrector suite (Parallel, Series, Mixed, PI, P) with standardized regulation pins and an in-card architecture switcher.

**Architecture:** 
- In `ferrite-project`, implement atomic POU renaming across the manifest, task orders, and filesystem, plus adjacent POU reordering in the manifest and task schedules.
- In `ferrite-lang-fbd`, implement block renaming that automatically rewires all downstream block links pointing to the old block ID, plus block array reordering.
- In `ferrite-gui`, build in-card corrector architecture switching (`pick_list`), standard regulation pin display (`SP`, `PV`, `KP`, `TI`, `TD`, `OUT`), FBD quick-add toolbar buttons, and explorer sidebar POU reordering/renaming controls.

**Tech Stack:** Rust (2024 edition), Iced 0.13, Serde, TOML, `thiserror`.

**Spec:** [`docs/superpowers/specs/2026-10-31-fbd-pou-and-pid-rework-design.md`](file:///C:/Users/Elio/prog/FerritePLC/docs/superpowers/specs/2026-10-31-fbd-pou-and-pid-rework-design.md)

## Global Constraints

- Zero `unwrap()` / `expect()` in library crates (`ferrite-core`, `ferrite-lang-fbd`, `ferrite-project`, `ferrite-runtime`, `ferrite-gui`).
- Zero heap allocations during normal scan tick execution (`PlcRuntime::execute` hot path).
- All UI strings must support bilingual English and French via `UiLanguage::tr`.
- Atomic file operations when saving or renaming project files on disk.
- Downstream block wire links (`FbdInputSource::Link`) must never be broken or orphaned when renaming a block or switching a corrector's architecture.

## Review Focus

- Empty/Whitespace or duplicate POU names in `rename_pou` must return a clear error without corrupting the manifest or filesystem.
- Downstream wire links referencing a renamed FBD block must point to the new ID, not leave dangling links.
- Quick-added PID/PI blocks must initialize with an unbound output pin (`(Internal)`), preventing `UnknownWritableSymbol` compiler errors until the user maps it to a tag or wire.
- Switching a PID block from `PidMixed` to `P` and back must preserve `SP`, `PV`, and `KP` values, safely restoring defaults for `TI` and `TD`.
- Moving a POU up or down when at the boundary (index 0 or last index) must be a safe no-op.

---

### Task 1: Project-Level POU Renaming and Reordering in `ferrite-project`

**Files:**
- Modify: `crates/ferrite-project/src/lib.rs`
- Test: `crates/ferrite-project/src/lib.rs:tests`

**Interfaces:**
- In `crates/ferrite-project/src/lib.rs`:
  ```rust
  impl Project {
      pub fn rename_pou(&mut self, old_name: &str, new_name: &str) -> Result<(), ProjectError>;
      pub fn move_pou_up(&mut self, name: &str) -> bool;
      pub fn move_pou_down(&mut self, name: &str) -> bool;
  }
  ```

- [ ] **Step 1: Write failing unit tests for POU renaming and reordering**
  In `crates/ferrite-project/src/lib.rs:tests`:
  - `test_project_rename_pou`: Scaffolds a project, calls `rename_pou("pump_ctrl", "boiler_fbd")`. Asserts `self.pous` has `"boiler_fbd"` and no `"pump_ctrl"`, `manifest.pous` entry updated with path `"pous/boiler_fbd.fbd.json"`, task `pou_order` replaced `"pump_ctrl"` with `"boiler_fbd"`, and asserts error on duplicate name or empty name.
  - `test_project_reorder_pous`: Scaffolds a project, moves `"pump_ctrl"` up by one. Asserts index changed in `manifest.pous` and relative order updated in `manifest.tasks[0].pou_order`. Asserts boundary move (moving first item up or last item down) returns `false`.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-project test_project_rename_pou test_project_reorder_pous`
  Expected: FAIL (methods not yet implemented on `Project`).

- [ ] **Step 3: Implement `rename_pou`, `move_pou_up`, and `move_pou_down` in `crates/ferrite-project/src/lib.rs`**
  - Implement validation: `new_name` must be non-empty, alphanumeric/underscore, and not already in `self.pous`.
  - In `rename_pou`:
    1. Rename on disk if `self.root_dir` exists and old file exists.
    2. Update `self.pous` map key and internal POU name (`match pou_src`).
    3. Update `self.manifest.pous` name and path (`pous/{new_name}.<ext>`).
    4. Update `self.manifest.tasks` where `task.pou_order` contains `old_name`.
  - In `move_pou_up`:
    1. Find index `idx` in `self.manifest.pous`. If `idx == 0`, return `false`.
    2. Swap `self.manifest.pous[idx]` with `self.manifest.pous[idx - 1]`.
    3. For each task in `self.manifest.tasks`, if both names are present in `task.pou_order`, swap their relative positions.
    4. Return `true`.
  - In `move_pou_down`:
    1. Find index `idx` in `self.manifest.pous`. If `idx >= len - 1`, return `false`.
    2. Swap `self.manifest.pous[idx]` with `self.manifest.pous[idx + 1]`.
    3. Synchronize task `pou_order`.
    4. Return `true`.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-project test_project_rename_pou test_project_reorder_pous`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(project): implement POU renaming and reordering with task sync"
  ```

---

### Task 2: FBD Block Renaming, Reordering & Link Rewiring in `ferrite-lang-fbd`

**Files:**
- Modify: `crates/ferrite-lang-fbd/src/lib.rs`
- Test: `crates/ferrite-lang-fbd/src/lib.rs:tests`

**Interfaces:**
- In `crates/ferrite-lang-fbd/src/lib.rs`:
  ```rust
  impl FbdProgram {
      pub fn rename_block(&mut self, old_id: &str, new_id: &str) -> Result<(), FbdError>;
      pub fn move_block_up(&mut self, idx: usize) -> bool;
      pub fn move_block_down(&mut self, idx: usize) -> bool;
  }
  ```

- [ ] **Step 1: Write failing unit tests for block renaming and link rewiring**
  In `crates/ferrite-lang-fbd/src/lib.rs:tests`:
  - `test_fbd_rename_block_rewires_links`: Creates a program with block `A` and block `B` where `B.inputs["IN1"] = Link { from_block: "A", from_pin: "OUT" }`. Renames `A` to `Sensor_Block`. Asserts `A` is now `Sensor_Block`, and `B.inputs["IN1"]` now has `from_block == "Sensor_Block"`. Asserts error on empty or duplicate block ID.
  - `test_fbd_reorder_blocks`: Creates blocks `[A, B, C]`. Calls `move_block_down(0)`. Asserts order is `[B, A, C]`. Calls `move_block_up(2)`. Asserts order is `[B, C, A]`. Tests bounds safety.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-lang-fbd test_fbd_rename_block_rewires_links test_fbd_reorder_blocks`
  Expected: FAIL (methods not yet implemented on `FbdProgram`).

- [ ] **Step 3: Implement `rename_block`, `move_block_up`, and `move_block_down` in `crates/ferrite-lang-fbd/src/lib.rs`**
  - Implement validation: `new_id` must be non-empty and unique among all blocks.
  - In `rename_block`:
    1. Locate block with `id == old_id`.
    2. Change its `id` to `new_id.to_string()`.
    3. Iterate through all blocks in `self.blocks`. For each input `FbdInputSource::Link { from_block, .. }`, if `from_block == old_id`, set `*from_block = new_id.to_string()`.
  - In `move_block_up`:
    If `idx > 0 && idx < self.blocks.len()`, swap `self.blocks[idx]` with `self.blocks[idx - 1]` and return `true`. Otherwise return `false`.
  - In `move_block_down`:
    If `idx + 1 < self.blocks.len()`, swap `self.blocks[idx]` with `self.blocks[idx + 1]` and return `true`. Otherwise return `false`.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-lang-fbd test_fbd_rename_block_rewires_links test_fbd_reorder_blocks`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(lang-fbd): implement block renaming with link rewiring and block reordering"
  ```

---

### Task 3: Advanced PID & PI Corrector Suite, Starter Pin Definitions & Architecture Switching

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_fbd.rs`
- Modify: `crates/ferrite-gui/src/app.rs`
- Test: `crates/ferrite-gui/src/lib.rs:tests`

**Interfaces:**
- In `crates/ferrite-gui/src/canvas_fbd.rs`:
  - `add_starter_corrector(fbd: &mut FbdProgram, fb_instances: &mut Vec<FbInstance>, kind: StandardFbKind)`
  - Standard regulation pin labels: `Consigne (SP)`, `Mesure (PV)`, `Gain (Kp)`, `Ti (s)`, `Td (s)`, `Commande (OUT)`
  - Card header `pick_list` for corrector architecture selection: `[PID_PARALLEL, PID_SERIES, PID_MIXED, PI, P]`
  - Header Move Up (▲) / Move Down (▼) buttons and editable block ID input
- In `crates/ferrite-gui/src/app.rs`:
  - `Message::FbdChangeCorrectorKind(String, usize, StandardFbKind)`
  - `Message::FbdBlockIdChanged(String, usize, String)`
  - `Message::FbdMoveBlockUp(String, usize)`
  - `Message::FbdMoveBlockDown(String, usize)`

- [ ] **Step 1: Write failing unit test for corrector architecture switching and pin reconciliation**
  In `crates/ferrite-gui/src/lib.rs:tests`:
  - `test_fbd_corrector_architecture_switching`:
    Creates an app with an FBD POU. Adds a `PidMixed` block. Verifies inputs contain `SP`, `PV`, `KP`, `TI`, `TD`, and outputs has empty/unbound `OUT`.
    Switches architecture to `Pi`. Verifies `TD` is safely removed while `SP`, `PV`, `KP`, `TI` are preserved.
    Switches architecture to `P`. Verifies `TI` is removed.
    Switches architecture back to `PidParallel`. Verifies `TI` and `TD` are restored with sensible defaults and `SP`, `PV` retain original user values.
  - `test_fbd_block_header_actions`:
    Tests renaming a block via `Message::FbdBlockIdChanged` and moving up/down via `Message::FbdMoveBlockUp` / `Message::FbdMoveBlockDown`.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui test_fbd_corrector_architecture_switching test_fbd_block_header_actions`
  Expected: FAIL (messages and helper logic not yet implemented).

- [ ] **Step 3: Implement corrector starter pins, architecture switcher, and card actions**
  1. In `crates/ferrite-gui/src/canvas_fbd.rs`:
     - Update `operator_symbol` to recognize all corrector kinds: `PidParallel` ("PID_PAR", "PARALLEL"), `PidSeries` ("PID_SER", "SERIES"), `PidMixed` ("PID_MIX", "MIXED (ISA)"), `Pi` ("PI", "CORRECTOR"), `P` ("P", "CORRECTOR").
     - In `render_single_block_card`:
       - If `blk.kind` is a standard FB corrector, render a `pick_list` dropdown showing the 5 architectures, triggering `Message::FbdChangeCorrectorKind`.
       - Render editable block ID input, triggering `Message::FbdBlockIdChanged`.
       - Render ▲ and ▼ buttons, triggering `Message::FbdMoveBlockUp` and `Message::FbdMoveBlockDown`.
       - Render pin labels: map `"SP"` to `"Consigne (SP)"`, `"PV"` to `"Mesure (PV)"`, `"KP"` to `"Gain (Kp)"`, `"TI"` to `"Ti (s)"`, `"TD"` to `"Td (s)"`, `"OUT"` to `"Commande (OUT)"`.
     - In `render_fbd_editor`:
       - Add toolbar buttons for: `+ PID (Par)`, `+ PID (Ser)`, `+ PID (Mix)`, `+ PI`, `+ P`.
  2. In `crates/ferrite-gui/src/app.rs`:
     - Add `Message` variants:
       `FbdChangeCorrectorKind(String, usize, StandardFbKind)`,
       `FbdBlockIdChanged(String, usize, String)`,
       `FbdMoveBlockUp(String, usize)`,
       `FbdMoveBlockDown(String, usize)`.
     - In `update`:
       - Handle `FbdChangeCorrectorKind`: update `blk.kind`, reconcile `TI` (present if `has_integral()`, removed if not) and `TD` (present if `has_derivative()`, removed if not), update `FbInstance.kind` in `self.project.symbols.fb_instances` and live `PlcRuntime`.
       - Handle `FbdBlockIdChanged`: call `fbd.rename_block(&old_id, &new_id)`.
       - Handle `FbdMoveBlockUp`: call `fbd.move_block_up(b_idx)`.
       - Handle `FbdMoveBlockDown`: call `fbd.move_block_down(b_idx)`.
       - In `Message::FbdAddBlock`: if adding a corrector, populate starter regulation inputs `SP: 60.0`, `PV: 0.0`, `KP: 2.0`, `TI: 5.0`, `TD: 0.5` without bound output tag.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-gui test_fbd_corrector_architecture_switching test_fbd_block_header_actions`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(gui): implement advanced PID/PI corrector suite, architecture switcher, and block card actions"
  ```

---

### Task 4: UI Explorer POU Renaming & Reordering Controls in `ferrite-gui`

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs`
- Test: `crates/ferrite-gui/src/lib.rs:tests`

**Interfaces:**
- In `crates/ferrite-gui/src/app.rs`:
  - State: `pub renaming_pou: Option<(String, String)>`
  - Messages:
    ```rust
    MovePouUp(String),
    MovePouDown(String),
    StartRenamePou(String),
    PouRenameInputChanged(String),
    ConfirmRenamePou,
    CancelRenamePou,
    ```

- [ ] **Step 1: Write failing unit test for Explorer POU reordering and renaming messages**
  In `crates/ferrite-gui/src/lib.rs:tests`:
  - `test_pou_explorer_rename_and_reorder_messages`:
    Creates an app with standard POUs.
    Sends `Message::MovePouUp("pump_ctrl".to_string())`. Asserts its index moved up in `app.project.manifest.pous` and task `pou_order`.
    Sends `Message::StartRenamePou("pump_ctrl".to_string())`. Asserts `app.renaming_pou` is `Some(("pump_ctrl", "pump_ctrl"))`.
    Sends `Message::PouRenameInputChanged("boiler_ctrl".to_string())`.
    Sends `Message::ConfirmRenamePou`. Asserts POU was renamed in `app.project`, manifest updated, active tab updated if it was open, and `app.renaming_pou` reset to `None`.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui test_pou_explorer_rename_and_reorder_messages`
  Expected: FAIL (messages not yet added to `StudioApp`).

- [ ] **Step 3: Implement Explorer state, messages, and UI buttons**
  1. In `crates/ferrite-gui/src/app.rs`:
     - Add `renaming_pou: Option<(String, String)>` to `FerriteStudioApp`.
     - Add messages: `MovePouUp`, `MovePouDown`, `StartRenamePou`, `PouRenameInputChanged`, `ConfirmRenamePou`, `CancelRenamePou`.
     - In `FerriteStudioApp::update`:
       - Handle `MovePouUp(name)`: call `self.project.move_pou_up(&name)`, track history, set `modified = true`.
       - Handle `MovePouDown(name)`: call `self.project.move_pou_down(&name)`, track history, set `modified = true`.
       - Handle `StartRenamePou(name)`: set `self.renaming_pou = Some((name.clone(), name))`.
       - Handle `PouRenameInputChanged(val)`: if let Some((ref old, _)) = self.renaming_pou, update current string.
       - Handle `ConfirmRenamePou`: if let Some((old_name, new_name)) = self.renaming_pou.take(), call `self.project.rename_pou(&old_name, &new_name)`. If success, update `self.active_tab` if it was `WorkspaceTab::Pou(old_name)` to `WorkspaceTab::Pou(new_name)`, sync `st_contents`, rebuild diagnostics, mark modified.
       - Handle `CancelRenamePou`: set `self.renaming_pou = None`.
     - In `view_left_explorer_body`:
       - For each POU:
         - Render ▲ button (on press `Message::MovePouUp(p.name)`).
         - Render ▼ button (on press `Message::MovePouDown(p.name)`).
         - If `self.renaming_pou` matches this POU: render inline `text_input` with OK (✓) and Cancel (✕) buttons.
         - Else: render POU tab select button with a subtle ✏ (Rename) button on the right edge.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-gui test_pou_explorer_rename_and_reorder_messages`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(gui): add Explorer POU reordering and renaming controls with tab sync"
  ```

---

### Task 5: End-to-End Workspace Verification & Quality Gates

**Files:**
- Workspace-wide verification.

- [ ] **Step 1: Run full workspace checks**
  Run: `cargo check --workspace --all-targets`
  Expected: PASS (0 errors).

- [ ] **Step 2: Run all workspace unit and integration tests**
  Run: `cargo test --workspace`
  Expected: PASS (all tests pass).

- [ ] **Step 3: Run Clippy with `-D warnings`**
  Run: `cargo clippy --workspace --all-targets -- -D warnings`
  Expected: PASS (0 warnings).

- [ ] **Step 4: Verify headless simulation of automation plant**
  Run: `cargo run -p ferrite-cli -- check -p examples/my_automation_plant`
  Run: `cargo run -p ferrite-cli -- sim --headless -p examples/my_automation_plant`
  Expected: PASS with all scans and assertions verified.

- [ ] **Step 5: Commit any final refinements and report completion**
  ```bash
  git commit -am "chore: complete verification gates for FBD POU rework and PID suite"
  ```
