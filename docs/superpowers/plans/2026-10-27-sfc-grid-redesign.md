# SFC Grid-Based Drag-and-Drop Canvas & Multi-Tree Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the SFC (Sequential Function Chart) editor into a discrete 2D grid-based canvas featuring free drag-and-drop element placement, port-to-port connection wire dragging, IEC 60848 AND divergence/convergence double-bar routing, multi-tree parallel sequence execution, and deferred compile-time syntactic validation.

**Architecture:** Use `iced::widget::canvas::Canvas` with `canvas::Program<Message>` for high-performance 60 FPS vector rendering and smooth mouse drag-and-drop. Store discrete `grid_pos: (i32, i32)` directly in `SfcStep` and `SfcTransition` in `ferrite-lang-sfc`. Implement a deferred topological graph analyzer for multi-tree discovery and syntactic validation (alternation, initial steps, closed branches) while allowing unconstrained intermediate states during canvas editing.

**Tech Stack:** Rust (2024 edition), Iced 0.13 (`iced::widget::canvas`, `iced::mouse`), `ferrite-lang-sfc`, `ferrite-core`, `serde`, `serde_json`.

**Spec:** [`sfc-grid-redesign.md`](file:///C:/Users/Elio/.gemini/antigravity-cli/brain/3fd04e3e-0625-4131-81bc-07416c4d2877/sfc-grid-redesign.md)

## Global Constraints
- `#![forbid(unsafe_code)]` across all crates.
- Zero `unwrap()` / `expect()` in library crates (`thiserror` for error handling).
- Non-allocating / zero heap allocation in real-time PLC scan loop hot path.
- 100% backward compatibility for existing `.sfc.json` project files (`#[serde(default)]`).
- Full bilingual English/French support via `lang.tr(...)`.

## Review Focus
1. Backward compatibility: existing `.sfc.json` without `grid_pos` must automatically compute clean initial grid positions without crashing or overlapping.
2. Port-to-port linking: dragging from a step to a transition must populate `transition.from_steps`, while dragging from a transition to a step must populate `transition.to_steps`.
3. Strict alternation: direct Step-to-Step or Transition-to-Transition connections must be rejected.
4. Multi-tree parallel execution: two independent initial steps and disjoint sequence paths must execute concurrently without blocking each other.
5. In-place double-click editing: double-clicking a step or transition must commit edits cleanly on Enter or blur without corrupting canvas coordinates.

---

### Task 1: AST Grid Coordinates & Multi-Tree Topological Validator

**Files:**
- Modify: `crates/ferrite-lang-sfc/src/lib.rs`
- Test: `crates/ferrite-lang-sfc/src/lib.rs` (unit tests in `mod tests`)

**Requirements:**
- Add `pub grid_pos: Option<(i32, i32)>` (defaulting to `None`) to `SfcStep` and `SfcTransition` with `#[serde(default)]`.
- Implement helper method `SfcProgram::assign_default_grid_positions(&mut self)` so any program lacking explicit coordinates arranges its elements into an orderly grid.
- Implement `SfcValidationReport` and `SfcValidationError` enum:
  - `MissingInitialStepInTree { tree_index: usize }`
  - `InvalidAlternation { source_id: String, target_id: String, reason: String }`
  - `DanglingReference { reference_id: String }`
  - `UnclosedAndBranch { transition_id: String }`
- Implement `validate_sfc_topology(program: &SfcProgram) -> SfcValidationReport`:
  - Computes connected components (independent parallel trees).
  - Verifies each tree has at least one initial step.
  - Verifies every step connects only to transitions, and every transition connects only to steps.
  - Verifies all step IDs exist.
- Update `SfcProgram::compile()` to integrate with `validate_sfc_topology()`.

**Steps:**
- [ ] Add `grid_pos: Option<(i32, i32)>` to `SfcStep` and `SfcTransition` in `crates/ferrite-lang-sfc/src/lib.rs`.
- [ ] Implement `assign_default_grid_positions` in `crates/ferrite-lang-sfc/src/lib.rs`.
- [ ] Implement `SfcValidationReport` and `validate_sfc_topology` in `crates/ferrite-lang-sfc/src/lib.rs`.
- [ ] Write unit tests: `test_sfc_multi_tree_detection_and_validation` and `test_sfc_grid_pos_json_roundtrip`.
- [ ] Verify with `cargo test -p ferrite-lang-sfc`.
- [ ] Commit Task 1.

---

### Task 2: Interactive 2D Grid Canvas & Wire Dragging Engine

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_sfc.rs`
- Modify: `crates/ferrite-gui/src/app.rs`
- Test: `crates/ferrite-gui/src/canvas_sfc.rs`

**Requirements:**
- Implement `SfcCanvasProgram` with `iced::widget::canvas::Canvas` and `canvas::Program<Message>`:
  - Discrete 20px grid background dots/crosshairs.
  - Elements positioned at snapped grid coordinates:
    - Step Box: $180\text{px} \times 60\text{px}$ ($9 \times 3$ grid cells).
    - Initial Step: Authentic double border.
    - Transition Bar: $80\text{px} \times 20\text{px}$ ($4 \times 1$ grid cells) with amber tick crossbar.
  - Top and bottom anchor ports:
    - Step top (input), step bottom (output).
    - Transition top (input), transition bottom (output).
  - Port-to-port wire dragging:
    - Mouse down within 10px of an output port begins wire dragging.
    - Active rubber-band line drawn to current cursor position.
    - Mouse up over a valid complementary input port creates the link and triggers `Message::SfcConnectPorts`.
  - Element dragging:
    - Mouse down on element body drags the element.
    - Moving cursor updates position, snapping to `GRID_CELL = 20.0`.
    - Mouse up commits `Message::SfcElementMoved`.
  - Directed wire routing:
    - Renders lines between steps and transitions.
    - When `to_steps.len() > 1` (AND divergence), renders IEC 60848 double horizontal bar (`===`) spanning target steps with drop wires.
    - When `from_steps.len() > 1` (AND convergence), renders double horizontal bar (`===`) spanning source steps.
- Add double-click in-place editing:
  - Double-clicking step name or transition receptivity toggles inline floating editor.
  - Commits via `Message::SfcStepNameChanged` / `Message::SfcTransitionReceptivityChanged`.

**Steps:**
- [ ] Define canvas interaction state (`SfcCanvasState`: selected element, dragging element, dragging wire, inline edit state).
- [ ] Implement `canvas::Program` in `crates/ferrite-gui/src/canvas_sfc.rs`.
- [ ] Implement drawing passes: grid, wires, AND double bars, step boxes, transition bars, port handles, dragging wire, active halos.
- [ ] Implement event handling: click selection, element dragging with snap-to-grid, port wire dragging, double-click inline edit.
- [ ] Add messages and handlers in `crates/ferrite-gui/src/app.rs`.
- [ ] Write unit tests verifying canvas math and port connection logic.
- [ ] Verify with `cargo test -p ferrite-gui`.
- [ ] Commit Task 2.

---

### Task 3: Multi-Tree Visual Management & Deferred Compile Feedback

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_sfc.rs`
- Modify: `crates/ferrite-gui/src/app.rs`
- Test: `crates/ferrite-gui/src/lib.rs`

**Requirements:**
- Run `validate_sfc_topology()` asynchronously / on demand and cache results in `app`.
- If an element has validation errors (e.g. missing initial step in tree, alternation error, unclosed branch), render a red/amber diagnostic halo or warning badge on the canvas.
- Display a tree identifier (e.g., `[Tree 1]`, `[Tree 2]`) for elements belonging to independent parallel trees.
- Header toolbar buttons for quick placement:
  - `+ Step`, `+ Transition`, `+ Parallel Branch`, `Check / Validate`.
- Update `FerriteStudioApp::check_project()` to report SFC topological diagnostics.

**Steps:**
- [ ] Add validation diagnostic caching to `FerriteStudioApp`.
- [ ] Render error halos and tree badges in `canvas_sfc.rs`.
- [ ] Add toolbar actions and status diagnostics bar.
- [ ] Add integration test in `crates/ferrite-gui/src/lib.rs` proving multi-tree SFC editing and deferred compile errors.
- [ ] Verify with `cargo test -p ferrite-gui`.
- [ ] Commit Task 3.

---

### Task 4: Full Workspace Verification & Multi-Language Integration Test

**Files:**
- Test: `crates/ferrite-runtime/tests/` or `crates/ferrite-gui/tests/`

**Requirements:**
- Full verification pass across all targets:
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- Verify that a multi-tree SFC POU compiles and executes in `ferrite-runtime` alongside Ladder and ST routines.

**Steps:**
- [ ] Run full workspace check.
- [ ] Run full workspace tests.
- [ ] Run workspace clippy with `-D warnings`.
- [ ] Commit Task 4 and document results.
