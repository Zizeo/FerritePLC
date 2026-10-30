# SFC IEC 60848 Structural Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement strict compile-time structural validation for Sequential Function Charts (SFC / Grafcet) to verify AND/OR divergence and convergence rules according to IEC 60848.

**Architecture:** Extend `validate_sfc_topology` in `ferrite-lang-sfc` with detection for ill-formed branching and reunion topologies, introducing descriptive `SfcValidationError` variants. Enforce these checks in `SfcProgram::compile()` and `ferrite check`, align sample fixtures in `ferrite-project`, update GUI canvas error rendering, and correct the GUI SFC builder helpers to construct standard-compliant Grafcet structures.

**Tech Stack:** Rust 2024 Edition, `ferrite-core`, `ferrite-lang-sfc`, `ferrite-project`, `ferrite-gui`, `iced`.

**Spec:** [`docs/superpowers/specs/2026-10-30-sfc-and-or-structural-validation-design.md`](file:///C:/Users/Elio/prog/FerritePLC/docs/superpowers/specs/2026-10-30-sfc-and-or-structural-validation-design.md)

## Global Constraints
- Rust 2024 Edition.
- Zero `unwrap()` / `expect()` in library crates (`ferrite-core`, `ferrite-lang-sfc`, `ferrite-project`, `ferrite-runtime`).
- Clean clippy (`cargo clippy --workspace --all-targets -- -D warnings`).
- All tests pass (`cargo test --workspace`).

## Review Focus
1. Single transition with `divergence == DivergenceKind::Or` and `to_steps.len() > 1` must be rejected with `InvalidOrDivergence`.
2. Single transition with `convergence == ConvergenceKind::Or` and `from_steps.len() > 1` must be rejected with `InvalidOrConvergence`.
3. AND Divergence (`===`) with `divergence == DivergenceKind::And` and `to_steps.len() > 1` missing upstream transition or source steps must be rejected with `InvalidAndDivergence`.
4. AND Convergence (`===`) with `convergence == ConvergenceKind::And` and `from_steps.len() > 1` missing downstream transition or target steps must be rejected with `InvalidAndConvergence`.
5. Valid multi-branch Grafcet programs (e.g. valid parallel AND branches and valid multi-transition OR selection/reunion) must continue to compile and pass validation without regressions.

---

### Task 1: Structural Validation Engine & Error Types in `ferrite-lang-sfc`

**Files:**
- Modify: `crates/ferrite-lang-sfc/src/lib.rs:194-263` (Error enum & Display), `crates/ferrite-lang-sfc/src/lib.rs:278-467` (`validate_sfc_topology`)
- Test: `crates/ferrite-lang-sfc/src/lib.rs` (unit tests at bottom of file)

**Interfaces:**
- Produces:
  ```rust
  pub enum SfcValidationError {
      // Existing variants...
      InvalidAndDivergence { transition_id: String, reason: String },
      InvalidAndConvergence { transition_id: String, reason: String },
      InvalidOrDivergence { element_id: String, reason: String },
      InvalidOrConvergence { element_id: String, reason: String },
  }
  ```
  and updated `pub fn validate_sfc_topology(program: &SfcProgram) -> SfcValidationReport`.

- [ ] **Step 1: Write failing unit tests for the 4 structural rules in `crates/ferrite-lang-sfc/src/lib.rs`**
  Add unit tests:
  - `test_sfc_validation_and_divergence_rule()`: verifies valid `Transition => AND Divergence => multiple States` passes, and invalid (no upstream step or empty targets) produces `InvalidAndDivergence`.
  - `test_sfc_validation_and_convergence_rule()`: verifies valid `multiple States => AND Convergence => Transition => State` passes, and invalid (no downstream target step) produces `InvalidAndConvergence`.
  - `test_sfc_validation_or_divergence_rule()`: verifies single transition with `divergence == Or` and `to_steps.len() > 1` produces `InvalidOrDivergence`.
  - `test_sfc_validation_or_convergence_rule()`: verifies single transition with `convergence == Or` and `from_steps.len() > 1` produces `InvalidOrConvergence`.

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -p ferrite-lang-sfc --lib test_sfc_validation_`
  Expected: FAIL with missing error variants or unimplemented validation checks.

- [ ] **Step 3: Add error variants and implement structural rules in `validate_sfc_topology`**
  - Add `InvalidAndDivergence`, `InvalidAndConvergence`, `InvalidOrDivergence`, and `InvalidOrConvergence` to `SfcValidationError` and implement `std::fmt::Display`.
  - In `validate_sfc_topology`:
    - Loop over `program.transitions`:
      - If `tr.divergence == DivergenceKind::And && tr.to_steps.len() > 1`: verify `!tr.from_steps.is_empty()`. If empty, push `InvalidAndDivergence`.
      - If `tr.convergence == ConvergenceKind::And && tr.from_steps.len() > 1`: verify `!tr.to_steps.is_empty()`. If empty, push `InvalidAndConvergence`.
      - If `tr.divergence == DivergenceKind::Or && tr.to_steps.len() > 1`: push `InvalidOrDivergence` (`"IEC 60848: OR divergence cannot be driven by a single transition; each alternative branch must have its own transition"`).
      - If `tr.convergence == ConvergenceKind::Or && tr.from_steps.len() > 1`: push `InvalidOrConvergence` (`"IEC 60848: OR convergence cannot merge multiple steps into a single transition; each alternative branch must end in its own transition into the destination step"`).

- [ ] **Step 4: Run tests to verify they pass**
  Run: `cargo test -p ferrite-lang-sfc --lib test_sfc_validation_`
  Expected: PASS.

- [ ] **Step 5: Commit**
  ```bash
  git add crates/ferrite-lang-sfc/src/lib.rs
  git commit -m "feat(sfc): implement IEC 60848 divergence and convergence structural validation"
  ```

---

### Task 2: Align Project Sample Fixtures & SFC Test POUs

**Files:**
- Modify: `crates/ferrite-project/src/lib.rs:655-685` (sample batch SFC POU), `crates/ferrite-project/src/lib.rs:730-760`
- Modify: `crates/ferrite-lang-sfc/src/lib.rs:1560-1580` (test fixture `T_ToForce`)

**Interfaces:**
- Consumes: `SfcValidationError`, `validate_sfc_topology` from Task 1.

- [ ] **Step 1: Check existing project tests for validation failures**
  Run: `cargo test -p ferrite-project`
  Expected: Detect failures where `T5_OrConv` violated the new OR convergence rule.

- [ ] **Step 2: Update sample batch SFC in `crates/ferrite-project/src/lib.rs` to valid Grafcet OR convergence**
  Replace `T5_OrConv` (which had `from_steps: ["4", "5"]`) with two valid alternative transitions targeting step `6`:
  - `T5a_Merge_Fast`: `from_steps: ["4"]`, `to_steps: ["6"]`, `receptivity: "Mix_Done"`.
  - `T5b_Merge_Slow`: `from_steps: ["5"]`, `to_steps: ["6"]`, `receptivity: "Tank_Temp >= 60.0"`.
  Update corresponding transition list in tests in `crates/ferrite-project/src/lib.rs`.

- [ ] **Step 3: Update any other invalid fixtures in `crates/ferrite-lang-sfc/src/lib.rs`**
  Ensure test fixture `T_ToForce` in `ferrite-lang-sfc` has valid connections.

- [ ] **Step 4: Run tests to verify project tests pass**
  Run: `cargo test -p ferrite-project -p ferrite-lang-sfc`
  Expected: PASS (all tests pass).

- [ ] **Step 5: Commit**
  ```bash
  git add crates/ferrite-project/src/lib.rs crates/ferrite-lang-sfc/src/lib.rs
  git commit -m "refactor(project): update sample SFC POU to conform to standard Grafcet OR convergence"
  ```

---

### Task 3: Update GUI Canvas Tests & Diagnostic Tooltip Formatting

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_sfc.rs` (error mapping / tooltips and test fixtures)

**Interfaces:**
- Consumes: New `SfcValidationError` variants from `ferrite-lang-sfc`.

- [ ] **Step 1: Inspect and update test fixtures in `crates/ferrite-gui/src/canvas_sfc.rs`**
  Update any test cases in `canvas_sfc.rs` (e.g. `test_canvas_or_convergence` at line 3312) that manually constructed `from_steps: vec!["0", "1"]` with `ConvergenceKind::Or` to test valid multi-transition OR convergence or verify the error is correctly reported.

- [ ] **Step 2: Ensure canvas node error badges identify elements affected by structural validation errors**
  In `crates/ferrite-gui/src/canvas_sfc.rs` (`render_step_box` and `render_transition`):
  Check that errors matching `element_id` or `transition_id` in `validation_report.errors` display red warning/error halos and tooltip reasons for `InvalidAndDivergence`, `InvalidAndConvergence`, `InvalidOrDivergence`, and `InvalidOrConvergence`.

- [ ] **Step 3: Run GUI crate unit tests**
  Run: `cargo test -p ferrite-gui --lib`
  Expected: PASS.

- [ ] **Step 4: Commit**
  ```bash
  git add crates/ferrite-gui/src/canvas_sfc.rs
  git commit -m "fix(gui): update canvas SFC error handling and tests for structural validation"
  ```

---

### Task 4: Correct GUI Action Handlers for Adding OR Divergence / Convergence

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:2440-2536` (`SfcAddOrDivergence`, `SfcAddOrConvergence`)

**Interfaces:**
- Consumes: `SfcTransition`, `SfcStep` from `ferrite-lang-sfc`.

- [ ] **Step 1: Refactor `Message::SfcAddOrDivergence` in `crates/ferrite-gui/src/app.rs`**
  When adding an OR divergence from step `S`:
  Instead of creating a single transition with `divergence: Or` and `to_steps: [StepA, StepB]`, create separate transitions for each branch originating from step `S`:
  - `T_branch1`: `from_steps: [S]`, `to_steps: [StepA]`.
  - `T_branch2`: `from_steps: [S]`, `to_steps: [StepB]`.
  Each branch has its own transition, satisfying `State => Divergence => Transition on each branch => State`.

- [ ] **Step 2: Refactor `Message::SfcAddOrConvergence` in `crates/ferrite-gui/src/app.rs`**
  When adding an OR convergence into destination step `Dest`:
  Instead of creating a single transition with `from_steps: [StepA, StepB]`, create or update transitions from each source branch to target `Dest`:
  - Each incoming branch has its own transition targeting `Dest`, satisfying `multiple Transitions => Convergence => State`.

- [ ] **Step 3: Run GUI tests to ensure no regressions**
  Run: `cargo test -p ferrite-gui`
  Expected: PASS.

- [ ] **Step 4: Commit**
  ```bash
  git add crates/ferrite-gui/src/app.rs
  git commit -m "fix(gui): generate valid Grafcet per-branch transitions for OR divergence and convergence"
  ```

---

### Task 5: Full Workspace Quality Gate & Verification

**Files:**
- None (workspace verification)

- [ ] **Step 1: Run `cargo check --workspace --all-targets`**
  Expected: Success, 0 errors.

- [ ] **Step 2: Run `cargo test --workspace`**
  Expected: All tests in all crates pass.

- [ ] **Step 3: Run `cargo clippy --workspace --all-targets -- -D warnings`**
  Expected: Zero clippy warnings.

- [ ] **Step 4: Final verification commit if needed**
  Ensure working directory is clean.
