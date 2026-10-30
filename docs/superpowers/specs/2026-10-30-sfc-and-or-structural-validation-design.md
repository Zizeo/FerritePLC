# SFC IEC 60848 Structural Validation Design

## 1. Intent & The Why
FerritePLC Sequential Function Charts (SFC / Grafcet) must follow strict IEC 60848 structural grammar and alternation rules. The compiler and validator must reject ill-formed divergence and convergence patterns to guarantee deterministic execution, deadlock prevention, and standard compliance.

Specifically:
- **AND Divergence (`===`)**: Must follow `Transition => Divergence => multiple States` (a single transition precedes the parallel divergence into 2+ target states).
- **AND Convergence (`===`)**: Must follow `multiple States => Convergence => Transition => State` (2+ states synchronize before a single transition clears them).
- **OR Divergence (`—`)**: Must follow `State => Divergence => Transition on each branch => State` (sequence selection where each alternative path has its own independent transition condition).
- **OR Convergence (`—`)**: Must follow `multiple Transitions => Convergence => State` (alternative branches each end with their own transition and reunite into a single destination state).

---

## 2. Goals, Non-Goals, & Anti-Goals
- **Goals**:
  - Implement comprehensive structural checks in `validate_sfc_topology(&SfcProgram) -> SfcValidationReport` in `ferrite-lang-sfc`.
  - Add explicit diagnostic error variants to `SfcValidationError`:
    - `InvalidAndDivergence`: flags AND divergence not preceded by a transition or lacking target steps.
    - `InvalidAndConvergence`: flags AND convergence not succeeded by a transition or lacking source steps.
    - `InvalidOrDivergence`: flags invalid sequence selection (e.g., a single transition declaring OR divergence to multiple target steps).
    - `InvalidOrConvergence`: flags invalid sequence reunion (e.g., multiple states connecting to a single transition under OR convergence).
  - Enforce compile-time blocking (`SfcProgram::compile()` returns `Err(SfcError::TopologyValidation)`).
  - Update any legacy test fixtures or sample POU definitions that used non-standard `from_steps.len() > 1` with `ConvergenceKind::Or` to conform to true Grafcet OR convergence (`multiple transitions -> single target step`).
  - Provide clear UI feedback (canvas error badges, node error status, and hover tooltips).
- **Non-Goals**:
  - Automatically rewriting user diagrams without explicit user actions.
  - Changing LD, FBD, or ST validation logic.
- **Anti-Goals**:
  - Runtime panic or `unwrap()` calls on invalid topologies.
  - Breaking valid Grafcet programs (e.g. valid parallel branches, loops, or alternative paths).

---

## 3. Constraints
- Rust 2024 Edition.
- Zero `unwrap()` / `expect()` in library crates (`ferrite-core`, `ferrite-lang-sfc`, `ferrite-project`, `ferrite-runtime`).
- Maintain full serialization compatibility (`serde`) for `.sfc.json` files.

---

## 4. Approaches Considered
- **Approach A: Strict Compile-Time Validation & Canvas Diagnostics (Picked)**
  - Fails `compile()` and `check()`.
  - Marks offending elements in GUI canvas.
  - Ensures industrial reliability and conformance to IEC 60848.
- **Approach B: Non-Blocking Lint Warnings & Canvas Highlighting (Rejected)**
  - Allows ill-formed charts to execute in simulator, violating deterministic scan rules.
- **Approach C: Dual Configurable Mode (Rejected)**
  - Unnecessary complexity for what is a core language syntax requirement.

---

## 5. Detailed Specification

### 5.1 Validation Rules in `validate_sfc_topology`

1. **AND Divergence (`===`)**:
   - Condition: A transition `tr` has `divergence == DivergenceKind::And` and `tr.to_steps.len() > 1`.
   - Valid if:
     - `tr` has $\ge 1$ upstream source step (`!tr.from_steps.is_empty()`).
     - Every target step exists and is a distinct valid step.
   - Invalid if:
     - Target steps $< 2$ when marked as multi-branch divergence.
     - Upstream element is missing (orphan divergence).

2. **AND Convergence (`===`)**:
   - Condition: A transition `tr` has `convergence == ConvergenceKind::And` and `tr.from_steps.len() > 1`.
   - Valid if:
     - `tr` has $\ge 1$ downstream target step (`!tr.to_steps.is_empty()`).
     - Every source step exists and is a distinct valid step.
   - Invalid if:
     - Source steps $< 2$ when marked as multi-step convergence.
     - Downstream element is missing (dead-end convergence).

3. **OR Divergence (`—`)**:
   - Standard: A state splits into multiple alternative branches. Each alternative branch MUST have its own transition.
   - Detection:
     - If any single transition has `divergence == DivergenceKind::Or` and `to_steps.len() > 1`: Flag `InvalidOrDivergence`. (A single transition cannot branch to multiple alternative steps without independent transition conditions on each branch).
     - If a step `S` has multiple outgoing transitions, each outgoing transition must independently define its own receptivity and target step(s).

4. **OR Convergence (`—`)**:
   - Standard: Multiple alternative branches reunite into a single state. Each branch MUST end in its own transition, and those transitions converge into the destination state.
   - Detection:
     - If any single transition has `convergence == ConvergenceKind::Or` and `from_steps.len() > 1`: Flag `InvalidOrConvergence`. (Multiple steps cannot converge into a single transition under OR; each step must have its own transition leading to the common target step).
     - Valid OR convergence is identified when multiple distinct transitions share the same target step in their `to_steps`.

### 5.2 Error Types in `crates/ferrite-lang-sfc/src/lib.rs`
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfcValidationError {
    // Existing variants...
    MissingInitialStepInTree { tree_id: usize, step_ids: Vec<String> },
    DanglingStepReference { transition_id: String, referenced_step: String, is_from_step: bool },
    InvalidAlternation { element_id: String, reason: String },
    OrphanElement { element_id: String, is_step: bool },
    UnclosedAndBranch { transition_id: String, reason: String },

    // New structural variants:
    InvalidAndDivergence { transition_id: String, reason: String },
    InvalidAndConvergence { transition_id: String, reason: String },
    InvalidOrDivergence { element_id: String, reason: String },
    InvalidOrConvergence { element_id: String, reason: String },
}
```

### 5.3 Test Fixtures & Codebase Alignment
- Update `crates/ferrite-project/src/lib.rs` default sample SFC (`sample_batch_sfc`) and any GUI canvas tests where `from_steps: vec!["4", "5"], convergence: ConvergenceKind::Or` was used:
  - Transform to valid Grafcet OR convergence: two separate transitions (`T4_To_6` and `T5_To_6`) targeting Step `6`.
- Add unit tests verifying each of the 4 valid patterns pass validation, and each invalid pattern fails with the respective error variant.
