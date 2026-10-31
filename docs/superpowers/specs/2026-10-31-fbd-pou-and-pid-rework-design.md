# FBD POU Overhaul: Renaming, Reordering & Advanced PID/PI Controller Suite

**Date:** 2026-10-31  
**Status:** Draft  
**Author:** Antigravity  

---

## 1. Intent & Overview

In industrial automation engineering, Program Organization Units (POUs) and Function Block Diagrams (FBD) must provide clear structural organization, unambiguous signal nomenclature, and precise controller architecture choices. 

This feature overhauls the FerritePLC Studio FBD and POU management subsystems in three interconnected dimensions:
1. **POU Management in Project Explorer**: Allow control engineers to rename POUs (updating on-disk files, project manifest, active tabs, and task bindings) and reorder them (moving them up/down to configure evaluation priority in tasks).
2. **FBD Block Management on Canvas**: Allow editing block IDs directly in the schematic card header (automatically updating any downstream block wire connections referencing the old ID) and reordering blocks (moving them up/down in the diagram sequence).
3. **Advanced PID & PI Corrector Suite**: Overhaul corrector function blocks to support the three fundamental industrial PID architectures—**Parallel**, **Series** (interacting), and **Mixed** (ideal/ISA)—along with **PI** and **P**, exposing standard industrial regulation pins: **Consigne (`SP` / Setpoint)**, **Mesure (`PV` / Process Variable)**, **Gain ($K_p$)**, **$T_i$ (Integral time in seconds)**, **$T_d$ (Derivative time in seconds)**, and **Commande (`OUT` / Manipulated Variable)**. An in-card pick-list allows instantaneous switching between controller architectures while preserving existing signal wiring.

---

## 2. Goals, Non-Goals & Anti-Goals

### 2.1 Goals
- **POU Renaming & Reordering**:
  - Reorder POUs with Move Up (▲) and Move Down (▼) buttons in the Project Explorer sidebar.
  - Reordering updates both `manifest.pous` and `manifest.tasks[].pou_order` in `FerriteProject.fplc`.
  - Rename POUs via an inline edit button or prompt; atomically rename files on disk (`pous/<old>.<ext>` $\rightarrow$ `pous/<new>.<ext>`), update the manifest, update open tabs, and sync runtime state.
- **FBD Block Renaming & Reordering**:
  - Make block ID directly editable in the card header with input validation.
  - When block ID is renamed (e.g., from `Blk_1` to `Speed_Reg`), automatically migrate all existing `FbdInputSource::Link { from_block: "Blk_1", .. }` references in other blocks to the new ID.
  - Provide Move Up (▲) and Move Down (▼) buttons on each block card to reorder execution within `program.blocks`.
- **PID & PI Corrector Suite**:
  - Expose industrial regulation terminology: Consigne (`SP`), Mesure (`PV`), Gain (`KP`), Integral time (`TI`), Derivative time (`TD`), Commande (`OUT`).
  - Provide full support for 3 PID forms:
    - **PID Parallel**: $u(t) = K_p \cdot e(t) + \frac{1}{T_i} \int e(t)dt + T_d \frac{de(t)}{dt}$
    - **PID Series (Interacting)**: $C(s) = K_p \left(1 + \frac{1}{T_i s}\right)(1 + T_d s)$
    - **PID Mixed (Ideal / Standard ISA)**: $u(t) = K_p \left[ e(t) + \frac{1}{T_i} \int e(t)dt + T_d \frac{de(t)}{dt} \right]$
    - **PI**: $u(t) = K_p \left[ e(t) + \frac{1}{T_i} \int e(t)dt \right]$
    - **P**: $u(t) = K_p \cdot e(t)$
  - Provide quick toolbar buttons in FBD: `+ PID (Par)`, `+ PID (Ser)`, `+ PID (Mix)`, `+ PI`, `+ P`.
  - Provide an in-card architecture selector dropdown (`pick_list`) on corrector cards so the user can switch between forms without deleting the block or losing wired connections to `SP`, `PV`, and `OUT`.
  - Synchronize parameters bidirectionally between FBD block inputs, the `FbInstance` registry (`symbols/fb_instances.toml`), and the live `PlcRuntime`.

### 2.2 Non-Goals
- Full free-form 2D drag-and-drop node graph canvas for FBD (FerritePLC uses a clean 2-column signal-flow card layout with orthogonal link routing).
- Modifying Ladder (LD) or Sequential Function Chart (SFC) internal solvers beyond standard cross-language corrector introspection.

### 2.3 Anti-Goals
- No loss of wire connections when renaming a block or switching a PID block's architecture.
- No dangling or orphaned file paths on disk when renaming a POU.
- Zero heap allocation on the scan loop tick execution (`PlcRuntime::execute` hot path).

---

## 3. Approaches Considered

### Approach 1: Integrated In-Card Controls & Explorer Actions *(Selected)*
- **POU Management**: Direct ▲/▼ buttons and a Rename button next to each POU in the Explorer sidebar. Renaming uses an inline text field.
- **Block Management**: Direct ▲/▼ and editable ID input in the card header.
- **PID Architecture**: In-card `pick_list` selector plus quick-add toolbar buttons. Dynamically adjusts `TI` and `TD` pins while preserving `SP`, `PV`, `KP`, and `OUT`.
- **Verdict**: Adopted. Fluid, fast, eliminates modal interruption, and matches user workflows established on the `egui` branch.

### Approach 2: Modal Properties Dialogs & Fixed Separate Blocks
- POUs and blocks edited through pop-up modals.
- Separate, immutable block types for Parallel, Series, and Mixed PID (requiring deletion and rewiring to change forms).
- **Verdict**: Rejected. Inconvenient for tuning and controller design; requires repetitive manual rewiring.

---

## 4. Technical Architecture & Implementation

### 4.0 Builder Resolutions & Explicit Decisions
Following the builder assessment, the following decisions are explicitly locked:
1. **POU Renaming Scope**: Local entity rename. `rename_pou` updates `manifest.pous`, `pous` in-memory map, task `pou_order`, active UI workspace tabs, and atomically renames the file on disk (`pous/<old>.<ext>` $\rightarrow$ `pous/<new>.<ext>`). It does not silently rewrite cross-POU expressions; any external references to old POU names are flagged by `ferrite check` as standard diagnostics.
2. **Corrector Output Initialization**: Newly created PID/PI blocks initialize with an empty `outputs` map (`(Internal / Unbound)`). Control outputs are immediately usable for downstream FBD block `Link` wiring, or can be bound to any global writable tag via the card's Target input field. This prevents compilation errors (`UnknownWritableSymbol`) on newly added blocks.
3. **Parameter Synchronization Policy**: Imperative synchronization. When the user edits a numeric parameter or switches corrector architecture, changes are synchronized from the FBD block to the `FbInstance` in `symbols/fb_instances.toml` and active `PlcRuntime`. During scan execution, the FBD AST remains the authoritative input source.
4. **Task Reordering Semantics**: `move_pou_up` and `move_pou_down` swap the POU order in `manifest.pous`. For each configured task, if the swapped POUs both appear in `task.pou_order`, their relative order within that task is updated to match.

### 4.1 Part A: POU Renaming & Reordering in `ferrite-project` & `ferrite-gui`

#### In `crates/ferrite-project/src/lib.rs`:
```rust
impl Project {
    /// Renames a POU across manifest, pous map, task execution orders, and on-disk files.
    pub fn rename_pou(&mut self, old_name: &str, new_name: &str) -> Result<(), ProjectError>;

    /// Moves a POU up in the manifest list and all task execution orders.
    pub fn move_pou_up(&mut self, name: &str) -> bool;

    /// Moves a POU down in the manifest list and all task execution orders.
    pub fn move_pou_down(&mut self, name: &str) -> bool;
}
```

**Implementation Details for `rename_pou`:**
1. Validates that `old_name` exists and `new_name` is non-empty, alphanumeric/underscore, and does not clash with existing POUs.
2. Updates `self.pous`: removes `old_name`, updates inner POU name, inserts under `new_name`.
3. Updates `self.manifest.pous`: updates `name` and modifies `path` (e.g. `pous/pump_ctrl.fbd.json` $\rightarrow$ `pous/boiler_fbd.fbd.json`).
4. Updates all `self.manifest.tasks`: replaces `old_name` with `new_name` in `pou_order`.
5. On disk: if `old_file` exists, renames it to `new_file`.

#### In `crates/ferrite-gui/src/app.rs`:
- State additions:
  ```rust
  pub renaming_pou: Option<(String, String)>, // (old_name, current_edit_string)
  ```
- Messages:
  ```rust
  MovePouUp(String),
  MovePouDown(String),
  StartRenamePou(String),
  PouRenameInputChanged(String),
  ConfirmRenamePou,
  CancelRenamePou,
  ```
- Explorer UI:
  Each POU item displays:
  - Language Badge (`LD`, `SFC`, `ST`, `FBD`)
  - Name (or text input if `renaming_pou` is active)
  - ▲ button (disabled if already at the top)
  - ▼ button (disabled if already at the bottom)
  - ✏ (Rename button)

---

### 4.2 Part B: FBD Block Renaming & Reordering in `ferrite-lang-fbd` & `canvas_fbd.rs`

#### In `crates/ferrite-lang-fbd/src/lib.rs`:
```rust
impl FbdProgram {
    /// Renames a block and updates any downstream links pointing to `old_id`.
    pub fn rename_block(&mut self, old_id: &str, new_id: &str) -> Result<(), FbdError>;

    /// Moves block at `idx` up by one position.
    pub fn move_block_up(&mut self, idx: usize) -> bool;

    /// Moves block at `idx` down by one position.
    pub fn move_block_down(&mut self, idx: usize) -> bool;
}
```

#### Link Rewiring Algorithm:
When `rename_block(old_id, new_id)` is invoked:
1. Verify `new_id` is non-empty and unique among other blocks in `self.blocks`.
2. Set target block's `id = new_id`.
3. For every block in `self.blocks`:
   For every input in `block.inputs`:
   If `FbdInputSource::Link { from_block, from_pin }` has `*from_block == old_id`, update `*from_block = new_id.to_string()`.

#### In `crates/ferrite-gui/src/canvas_fbd.rs`:
- Card header layout:
  - Editable ID text input: triggers `Message::FbdBlockIdChanged(pou_name, b_idx, new_id)`.
  - ▲ button: triggers `Message::FbdMoveBlockUp(pou_name, b_idx)`.
  - ▼ button: triggers `Message::FbdMoveBlockDown(pou_name, b_idx)`.
  - Type / Architecture badge / pick-list.
  - ✕ button: triggers `Message::FbdRemoveBlock(pou_name, b_idx)`.

---

### 4.3 Part C: PID & PI Corrector Suite

#### Supported Corrector Models:
The underlying mathematics in `ferrite-core::StandardFbKind` covers all five modes:
1. **`PidParallel`**: Independent gains for P, I, and D.
2. **`PidSeries`**: Interacting gain structure commonly found in analog and pneumatic legacy systems ($K_p(1 + 1/T_i s)(1 + T_d s)$).
3. **`PidMixed`**: Standard ideal ISA non-interacting algorithm ($K_p [e + \frac{1}{T_i}\int e + T_d \frac{de}{dt}]$).
4. **`Pi`**: Pure proportional-integral ($K_p [e + \frac{1}{T_i}\int e]$).
5. **`P`**: Pure proportional ($K_p \cdot e$).

#### Standardized Industrial Regulation Pins:
| Pin | Label in UI | Type | Description |
|---|---|---|---|
| `SP` | **Consigne (SP)** | `REAL` | Setpoint target (e.g. 60.0°C) |
| `PV` | **Mesure (PV)** | `REAL` | Process variable sensor feedback |
| `KP` | **Gain ($K_p$)** | `REAL` | Proportional gain multiplier |
| `TI` | **$T_i$ (s)** | `REAL`/`TIME` | Integral reset time in seconds (PI, PID Par/Ser/Mix) |
| `TD` | **$T_d$ (s)** | `REAL`/`TIME` | Derivative rate time in seconds (PID Par/Ser/Mix) |
| `OUT` | **Commande (OUT)** | `REAL` | Manipulated variable control output |

#### In-Card Architecture Switcher:
On any corrector card, an architecture selector (`pick_list`) displays the current mode (`Parallel`, `Series`, `Mixed`, `PI`, `P`). Selecting a new mode sends `Message::FbdChangeCorrectorKind(pou_name, b_idx, new_kind)`.
In `StudioApp::update`:
1. The block's `FbdBlockKind::StandardFb { fb_type, instance }` is updated to `Some(new_kind)`.
2. Required pins are reconciled:
   - If `new_kind.has_integral()`: Ensure `"TI"` is in `blk.inputs` (defaults to `"5.00"` if absent). If not, remove `"TI"`.
   - If `new_kind.has_derivative()`: Ensure `"TD"` is in `blk.inputs` (defaults to `"0.50"` if absent). If not, remove `"TD"`.
   - Keep `"SP"`, `"PV"`, `"KP"`, and `"OUT"` intact with all user links and values.
3. The shared `FbInstance` in `self.project.symbols.fb_instances` and `self.runtime` is synchronized.

#### Toolbar Quick-Add Buttons:
In `render_fbd_editor`:
- `+ PID (Par)`
- `+ PID (Ser)`
- `+ PID (Mix)`
- `+ PI`
- `+ P`

Clicking any button adds a pre-configured block with starter values:
`SP: "60.0"`, `PV: "0.0"`, `KP: "2.00"`, `TI: "5.00"` (if applicable), `TD: "0.50"` (if applicable), and `OUT: "PID_Out"`.

---

## 5. Verification & Quality Gates

1. **Unit Tests in `ferrite-project`**:
   - `test_project_rename_pou`: Asserts manifest, `pous` map, task orders, and disk files update properly; asserts error on duplicate name.
   - `test_project_reorder_pous`: Asserts `move_pou_up` and `move_pou_down` update both manifest and task `pou_order`.
2. **Unit Tests in `ferrite-lang-fbd`**:
   - `test_fbd_rename_block_rewires_links`: Verifies renaming `B1` to `PID_Master` automatically updates downstream `Link { from_block: "B1", from_pin: "OUT" }` to `"PID_Master"`.
   - `test_fbd_reorder_blocks`: Verifies moving blocks up and down.
3. **Unit Tests in `ferrite-gui`**:
   - `test_fbd_corrector_architecture_switching`: Verifies switching between Parallel, Series, Mixed, and PI dynamically reconciles $T_i$ and $T_d$ pins while preserving setpoint and process variable links.
   - `test_pou_explorer_rename_and_reorder_messages`: Verifies Elm architecture messages for POU up/down and renaming.
4. **Workspace Gate**:
   - `cargo check --workspace --all-targets`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo run -p ferrite-cli -- check -p examples/my_automation_plant`
   - `cargo run -p ferrite-cli -- sim --headless -p examples/my_automation_plant`
