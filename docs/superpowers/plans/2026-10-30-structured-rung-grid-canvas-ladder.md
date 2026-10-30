# Structured Rung Grid Canvas for Ladder Diagram Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the Ladder Diagram (LD) editor into a high-performance vector 2D canvas with structured rung grid layout, drag-and-drop repositioning, marquee multi-selection, batch deletion, parallel branch bypass loops, live power-flow visualization, and an interactive bottom inspector.

**Architecture:** Implement an `iced::widget::canvas::Canvas` vector renderer in `crates/ferrite-gui/src/canvas_ld.rs`. Use a two-pass dynamic geometry engine to lay out left/right power rails, rungs, discrete contact/coil slots, and step-down parallel bypass loops. Maintain clean AST indexing with `LdBranchPathSegment` and `LdElementKey` for selection and drag-and-drop, preserving full compatibility with `ferrite-lang-ld` without modifying the `.ld.json` schema.

**Tech Stack:** Rust 2024 Edition, `iced` (Canvas, Geometry, Path, Stroke, Text), `ferrite-core`, `ferrite-lang-ld`, `ferrite-runtime`, `ferrite-gui`.

**Spec:** [`docs/superpowers/specs/2026-10-30-structured-rung-grid-canvas-ladder-design.md`](file:///C:/Users/Elio/prog/FerritePLC/docs/superpowers/specs/2026-10-30-structured-rung-grid-canvas-ladder-design.md)

## Global Constraints
- Rust 2024 Edition.
- Zero `unwrap()` / `expect()` calls in library crates and GUI.
- Clean clippy with zero warnings (`cargo clippy --workspace --all-targets -- -D warnings`).
- All tests pass (`cargo test --workspace`).
- 100% backward compatibility with existing `LdProgram` and `.ld.json` files.

## Review Focus
1. Multi-selection marquee box must correctly intersect contacts, coils, and branches across multiple rungs without off-by-one coordinate errors.
2. Drag-and-drop repositioning must properly update nested branch paths without corrupting AST ordering or causing index out-of-bounds panics.
3. Batch deletion of all elements in a branch path must either leave a valid bare wire bypass or automatically dissolve a 1-path branch back onto the main rung.
4. Dynamic `RAIL_RIGHT_X` calculation must prevent wide rungs with multiple contacts from overflowing past the right power rail or coils.
5. Live simulation power-flow animation must highlight energized wires, closed contacts, and active coils in emerald green without UI frame drops or memory allocations during scan execution.

---

### Task 1: Core Layout Geometry Engine & Addressing Types in `canvas_ld.rs`

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_ld.rs`
- Test: `crates/ferrite-gui/src/canvas_ld.rs` (unit tests module at bottom)

**Interfaces:**
- Produces:
  ```rust
  pub struct LdBranchPathSegment { pub branch_idx: usize, pub path_idx: usize }
  pub enum LdElementKey {
      Rung(usize),
      Contact { rung: usize, branch_path: Vec<LdBranchPathSegment>, index: usize },
      Coil { rung: usize, index: usize },
      Branch { rung: usize, branch_path: Vec<LdBranchPathSegment>, index: usize },
      InlineFb { rung: usize, branch_path: Vec<LdBranchPathSegment>, index: usize },
  }
  pub enum LdDropTarget {
      BeforeElement { rung: usize, branch_path: Vec<LdBranchPathSegment>, index: usize },
      AfterElement { rung: usize, branch_path: Vec<LdBranchPathSegment>, index: usize },
      NewBranch { rung: usize, branch_path: Vec<LdBranchPathSegment> },
      CoilSlot { rung: usize, index: usize },
      NewRung { index: usize },
  }
  pub struct LdLayoutEngine { ... }
  ```

- [ ] **Step 1: Write failing unit tests for layout geometry calculation in `crates/ferrite-gui/src/canvas_ld.rs`**
  - `test_ld_layout_straight_rung_geometry()`: verifies left rail, contact slots, coil area, and right rail positions.
  - `test_ld_layout_parallel_branch_geometry()`: verifies vertical step-down displacement, horizontal sub-tracks, and merge point calculation.
  - `test_ld_layout_multiple_coils_stack_height()`: verifies rung height expands dynamically when a rung has multiple coils.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui --lib test_ld_layout_`
  Expected: FAIL (types and functions not defined).

- [ ] **Step 3: Implement addressing types and `LdLayoutEngine` in `canvas_ld.rs`**
  - Define `LdBranchPathSegment`, `LdElementKey`, `LdDropTarget`, and `LdLayoutEngine`.
  - Implement two-pass layout:
    - Pass 1: compute maximum horizontal contact width across all rungs to fix `RAIL_RIGHT_X`.
    - Pass 2: calculate bounding boxes `iced::Rectangle` for each contact, coil, inline FB, branch loop, and rung.
    - Compute power flow state for each element from `PlcRuntime`.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-gui --lib test_ld_layout_`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git add crates/ferrite-gui/src/canvas_ld.rs
  git commit -m "feat(gui): implement core ladder canvas layout geometry and addressing types"
  ```

---

### Task 2: Vector Canvas Rendering (`Program` Implementation for `LdCanvas`)

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_ld.rs`

**Interfaces:**
- Consumes: `LdLayoutEngine`, `LdElementKey` from Task 1.
- Produces:
  ```rust
  pub struct LdCanvas<'a> {
      pub program: &'a LdProgram,
      pub runtime: Option<&'a PlcRuntime>,
      pub state: &'a LdCanvasState,
      pub lang: UiLanguage,
  }
  impl<'a> iced::widget::canvas::Program<Message> for LdCanvas<'a> { ... }
  ```

- [ ] **Step 1: Write failing unit test for canvas rendering in `canvas_ld.rs`**
  - `test_ld_canvas_draw_without_panic()`: creates sample ladder program with contacts, branches, and coils, and calls `LdCanvas::draw()`, asserting geometry frame produces paths and text without panic.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui --lib test_ld_canvas_draw_`
  Expected: FAIL (`LdCanvas` or `Program` implementation missing).

- [ ] **Step 3: Implement vector `draw()` for `LdCanvas`**
  - Render background grid lines/dots with panning and zoom scale.
  - Render continuous vertical left (emerald) and right (neutral) power rails.
  - Render horizontal rung wires, contact cards (symbols, `--| |--`, `--|/|--`, `--|P|--`, `--|N|--`), and inline FB boxes.
  - Render parallel branch loops: vertical step-down lines, horizontal parallel branch wires, and step-up merge lines.
  - Render coil cards near right rail (`--( )--`, `--[S]--`, `--[R]--`).
  - Render selection cyan glow around `selected_elements`.
  - Render marquee drag rectangle when active (`state.drag_marquee`).
  - Render drop hover indicator when dragging (`state.drop_hover`).

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -p ferrite-gui --lib test_ld_canvas_draw_`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git add crates/ferrite-gui/src/canvas_ld.rs
  git commit -m "feat(gui): implement vector iced canvas rendering for ladder diagrams"
  ```

---

### Task 3: Interactive Selection, Multi-Selection, Marquee & Pan/Zoom

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_ld.rs`, `crates/ferrite-gui/src/app.rs`

**Interfaces:**
- Consumes: `LdCanvas`, `LdLayoutEngine` from Tasks 1 & 2.
- Produces:
  - `LdCanvasState`: pan, zoom, selected, selected_elements, drag_marquee.
  - Messages in `app.rs`: `LdSelectElement`, `LdSelectElements`, `LdCanvasPanChanged`, `LdCanvasZoomChanged`.

- [ ] **Step 1: Write failing unit tests for selection and marquee in `canvas_ld.rs`**
  - `test_ld_hit_test_elements()`: tests hit-testing point on contacts, coils, and branches.
  - `test_ld_marquee_selection_intersection()`: tests dragging a marquee rectangle and asserting all enclosed elements are selected.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui --lib test_ld_marquee_`
  Expected: FAIL (functions missing).

- [ ] **Step 3: Implement interactive mouse handling in `LdCanvas::update()`**
  - Click on element: selects single element (`LdSelectElement`).
  - `Ctrl + Click`: toggles element in `selected_elements` (`LdSelectElements`).
  - Click on background + drag: starts `drag_marquee` selection box.
  - Marquee release: selects all elements intersecting the marquee rectangle.
  - Middle-click / space-drag: pans canvas view.
  - Scroll wheel: zooms canvas view centered at cursor.

- [ ] **Step 4: Connect messages in `crates/ferrite-gui/src/app.rs`**
  - Handle `LdSelectElement`, `LdSelectElements`, `LdCanvasPanChanged`, `LdCanvasZoomChanged` in `app.update()`.

- [ ] **Step 5: Run tests to verify they pass**
  Run: `cargo test -p ferrite-gui --lib test_ld_marquee_`
  Expected: PASS.

- [ ] **Step 6: Commit**
  ```bash
  git add crates/ferrite-gui/src/canvas_ld.rs crates/ferrite-gui/src/app.rs
  git commit -m "feat(gui): implement ladder canvas marquee multi-selection, pan, and zoom"
  ```

---

### Task 4: Drag-and-Drop Repositioning & Parallel Branch Operations

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_ld.rs`, `crates/ferrite-gui/src/app.rs`

**Interfaces:**
- Consumes: `LdDropTarget`, `LdElementKey` from Tasks 1-3.
- Produces:
  - Messages in `app.rs`: `LdMoveElements(String, Vec<LdElementKey>, LdDropTarget)`, `LdAddParallelBranch(String, usize, Vec<LdBranchPathSegment>)`.

- [ ] **Step 1: Write failing unit test for moving elements and branch operations in `app.rs`**
  - `test_ld_move_contact_to_different_slot()`: moves a contact from slot 0 to slot 1 and asserts updated rung elements.
  - `test_ld_add_parallel_branch_wraps_contacts()`: creates a parallel bypass branch around selected contacts.
  - `test_ld_branch_dissolution_on_last_branch_drop()`: verifies removing a leg from a 2-leg branch dissolves the branch into the main rung.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui --lib test_ld_move_`
  Expected: FAIL (`LdMoveElements` handler missing).

- [ ] **Step 3: Implement drag-and-drop state machine and drop target resolution**
  - In `canvas_ld.rs`: on mouse drag with element selected, set `dragging` and calculate nearest `drop_hover: Option<LdDropTarget>`.
  - On mouse release over valid drop target: emit `Message::LdMoveElements`.
  - In `app.rs`: implement `LdMoveElements`:
    - Remove moved element(s) from source rung/branch path.
    - Insert at destination slot (`BeforeElement`, `AfterElement`, `NewBranch`, `CoilSlot`).
    - If a 2-path branch is left with 1 path, unwrap/dissolve remaining elements into parent line.
  - In `app.rs`: implement `LdAddParallelBranch`:
    - Creates a new `LdElement::Branch` enclosing the target element or adds an alternative parallel path.

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-gui --lib test_ld_move_`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git add crates/ferrite-gui/src/canvas_ld.rs crates/ferrite-gui/src/app.rs
  git commit -m "feat(gui): implement ladder drag-and-drop repositioning and parallel branch creation"
  ```

---

### Task 5: Batch Deletion, Bottom Inspector Context Bar & Toolbar Actions

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_ld.rs`, `crates/ferrite-gui/src/app.rs`

**Interfaces:**
- Produces:
  - Message in `app.rs`: `LdDeleteSelected(String)`.
  - Bottom inspector rendering: `render_ld_inspector(&self, pou_name)`.
  - Top toolbar buttons for ladder canvas.

- [ ] **Step 1: Write failing unit test for batch deletion in `app.rs`**
  - `test_ld_batch_delete_selected_elements()`: selects 3 contacts across 2 rungs, calls `Message::LdDeleteSelected`, asserts all 3 are deleted and rungs collapse cleanly.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-gui --lib test_ld_batch_delete_`
  Expected: FAIL (`LdDeleteSelected` handler missing).

- [ ] **Step 3: Implement batch delete in `app.rs`**
  - Sort keys by descending index to prevent index invalidation during removal.
  - Delete selected contacts, coils, branches, and rungs.
  - Perform automatic branch dissolution for any branches that became single-path or empty.
  - Clear selection.

- [ ] **Step 4: Implement Bottom Inspector Bar and Top Toolbar**
  - In `canvas_ld.rs` / `app.rs`:
    - Bottom inspector:
      - Contact selected: tag input, contact type pick list (`NO`, `NC`, `|P|`, `|N|`), delete button.
      - Coil selected: tag input, coil type pick list (`( )`, `[S]`, `[R]`), delete button.
      - Rung selected: comment input, delete rung button.
      - Multiple elements selected: `"N elements selected"` badge, batch delete button.
    - Top toolbar: `+ Rung`, `+ NO`, `+ NC`, `+ |P|`, `+ |N|`, `+ Coil`, `+ Branch`, `Delete Selected`, `Reset View`.

- [ ] **Step 5: Run tests to verify they pass**
  Run: `cargo test -p ferrite-gui --lib test_ld_batch_delete_`
  Expected: PASS.

- [ ] **Step 6: Commit**
  ```bash
  git add crates/ferrite-gui/src/canvas_ld.rs crates/ferrite-gui/src/app.rs
  git commit -m "feat(gui): implement ladder batch deletion, bottom inspector, and toolbar actions"
  ```

---

### Task 6: Full Workspace Verification & Clippy Cleanup

**Files:**
- None (workspace verification)

- [ ] **Step 1: Run `cargo check --workspace --all-targets`**
  Expected: Clean compilation, 0 errors.

- [ ] **Step 2: Run `cargo test --workspace`**
  Expected: All workspace tests pass.

- [ ] **Step 3: Run `cargo clippy --workspace --all-targets -- -D warnings`**
  Expected: Zero warnings.

- [ ] **Step 4: Final commit if needed**
  Clean working tree.
