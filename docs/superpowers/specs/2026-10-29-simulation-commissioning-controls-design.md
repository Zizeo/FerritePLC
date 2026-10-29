# Design Specification: Simulation Execution & Commissioning Controls Suite

**Date**: 2026-10-29  
**Status**: Ready for Approval  
**Author**: Antigravity  
**Domain**: Industrial PLC Simulation, Real-Time Runtime, Visual Commissioning  

---

## 1. Intent & Purpose

In industrial automation engineering (e.g. Siemens TIA Portal, Beckhoff TwinCAT, Rockwell Studio 5000), a PLC simulator is the primary commissioning environment where control programs, safety interlocks, and state sequences are validated prior to deployment on physical hardware.

Currently, FerritePLC possesses a high-performance deterministic execution engine, but its simulation controls suffer from key usability limitations:
1. Scan execution is rigidly tied to wall-clock time (`scan_cycle_ms`), forcing engineers to wait in real time for long industrial timers (`T#10s`, `T#5m`).
2. Breakpoints implemented in `ferrite-runtime` are ignored by the GUI and have no authoring interface.
3. Active forces (`TagTable::force_tag` and `ProcessImage::force`) have no global visibility banner or single-click release, violating a core industrial safety principle.
4. Grafcet / SFC transition receptivity wires only evaluate single symbol names, failing to illuminate bright green on compound logic like `Start_PB AND Safety_OK`.
5. Reset does not distinguish between IEC 61131-3 Cold Restart vs. Warm Restart (preserving `RETAIN` data like batch counters and calibration setpoints).

This project implements the **Integrated Studio Commissioning Suite**, transforming FerritePLC into an agile, highly observable, and safety-compliant virtual commissioning platform.

---

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- **Variable Simulation Speed with Batching**: Provide runtime clock scaling (`0.1x`, `1x`, `2x`, `5x`, `10x`, `Max / Uncapped`) via deterministic batch execution in `ferrite-runtime` to slow down fast transients or fast-forward long process cycles.
- **Global Force Safety Banner & Unforce All**: Display a prominent amber/red indicator whenever any tag or address is forced, with an instant `[Unforce All]` release button and support for forcing numerical/analog tags in the I/O rack.
- **Interactive Breakpoints in Runtime & GUI**: Extend `ScanTelemetry` to report the exact `hit_breakpoint`, pause simulation immediately upon hit, and provide an authoring interface in the Diagnostics tab to add/remove breakpoints on scan counts or symbol conditions.
- **Live Compound SFC Receptivities & Step Timers**: Dynamically evaluate arbitrary ST boolean expressions on the SFC canvas (`TRUE` = bright emerald green, `FALSE` = muted slate), and display live elapsed active timers (`X1.t`) directly on step cards.
- **IEC 61131-3 Cold vs. Warm Restart**: Distinguish full factory reset (Cold, wiping memory, resetting tags, clearing forces) from sequence reset with `RETAIN` variable and `%M` marker preservation (Warm, keeping active forces and retain values).

### Non-Goals
- Closed-loop plant physics simulation (pumps filling tanks automatically; handled in the Digital Twin milestone).
- Physical fieldbus hardware couplers (EtherCAT or Profinet physical PCI cards).
- Online hot-code patching (modifying POU source code in memory without pausing).

### Anti-Goals
- No heap allocations in the `step_scan()` hot loop.
- No GUI thread freezing during `Max` uncapped simulation mode.
- No persistent file corruption: variable forces and temporary overrides must remain strictly in memory and never mutate project `.fplc` or POU source files on disk.

---

## 3. Constraints

- **Language & Workspace**: Rust 2024 Edition.
- **Linter & Code Quality**: `unwrap_used = "deny"`, `expect_used = "deny"`, zero clippy warnings with `-D warnings`.
- **UI Framework**: Iced 0.13 with multi-window canvas docking.
- **Deterministic Memory**: Zero allocation during scan ticks; maintain real-time safety.

---

## 4. Approaches Considered

1. **Approach 1: Integrated Studio Commissioning Suite (Selected)**
   - Delivers time acceleration with runtime batching, active force safety, interactive breakpoints with authoring UI, live compound SFC receptivities, and Cold/Warm restart in one unified update.
   - *Trade-off*: Touches `ferrite-core`, `ferrite-runtime`, and `ferrite-gui`, but directly addresses all identified simulator friction points.

2. **Approach 2: Lightweight Speed & Visual Feedback Slice (Rejected)**
   - Implements only speed multipliers and visual wire colors.
   - *Reason for Rejection*: Leaves breakpoints broken in the GUI, omits the safety-critical Force banner, and skips Warm restart data retention.

3. **Approach 3: Deep Debugger & Inspection Tooling (Rejected)**
   - Focuses only on a complex low-level breakpoint matrix and step-by-step POU execution.
   - *Reason for Rejection*: Overly academic; omits the time acceleration and on-canvas visual feedback that automation engineers rely on during daily testing.

---

## 5. Architectural & Technical Specification

### 5.1 Runtime & Core Enhancements (`ferrite-core` & `ferrite-runtime`)

#### 1. Simulation Batch Execution (`step_scan_batch`)
- In `ferrite-runtime::PlcRuntime`:
  ```rust
  pub fn step_scan_batch(&mut self, batch_size: usize) -> Result<ScanTelemetry, RuntimeError> {
      let mut latest_tel = self.step_scan()?;
      if latest_tel.breakpoint_hit {
          return Ok(latest_tel);
      }
      for _ in 1..batch_size {
          latest_tel = self.step_scan()?;
          if latest_tel.breakpoint_hit {
              break;
          }
      }
      Ok(latest_tel)
  }
  ```
  - Loops up to `batch_size`.
  - Halts early on breakpoint hit or error.
  - Zero heap allocation; preserves deterministic sequence semantics.

#### 2. Enhanced Breakpoint Telemetry
- In `ScanTelemetry`:
  ```rust
  pub struct ScanTelemetry {
      pub scan_count: u64,
      pub virtual_time_ms: u64,
      pub scan_duration_us: u64,
      pub max_scan_duration_us: u64,
      pub breakpoint_hit: bool,
      pub hit_breakpoint: Option<Breakpoint>,
  }
  ```

#### 3. Cold vs. Warm Restart State Boundaries
- In `ferrite-core::context::ExecutablePou`:
  - Add optional method: `fn reset(&mut self) {}` (default no-op).
  - Implement on `CompiledSfcPou` in `ferrite-lang-sfc` to re-initialize active steps back to initial configuration and clear elapsed durations.
- In `ferrite-runtime::PlcRuntime`:
  - `pub fn cold_restart(&mut self) -> Result<(), RuntimeError>`:
    - Clears all `%I`, `%Q`, `%M` process image memory.
    - Reinitializes all tags (both retain and non-retain) to their `initial_value`.
    - Resets all tasks (`elapsed_ms = 0`, `last_trigger_state = false`, `execution_count = 0`).
    - Resets all POUs via `pou.reset()`.
    - Wipes all active forces (`self.unforce_all()?`).
    - Clears virtual clock and scan counters (`scan_count = 0`, `virtual_time_ms = 0`).
  - `pub fn warm_restart(&mut self) -> Result<(), RuntimeError>`:
    - Captures snapshot of `TagTable::export_retain_values()` and `%M` markers (`ProcessImage::export_markers()`).
    - Resets non-retain tags to `initial_value`.
    - Restores retained tag values and `%M` markers.
    - Resets all tasks and all POUs via `pou.reset()`.
    - **Preserves active forces** (field commissioning overrides remain in effect).
    - Resets virtual clock and scan counters (`scan_count = 0`, `virtual_time_ms = 0`).

#### 4. Active Force Query & Unforce All
- In `ferrite-runtime::PlcRuntime`:
  - `pub fn active_forces_count(&self) -> usize`:
    Returns total count of forced tags in `TagTable` + forced addresses in `ProcessImage`.
  - `pub fn unforce_all(&mut self) -> Result<(), RuntimeError>`:
    Clears all active forces in `TagTable` and clears all force bitmasks in `ProcessImage`.

#### 5. Pure Compound Expression Evaluation for SFC Visuals
- In `ferrite-ir`:
  - Add `pub fn evaluate_with_resolver<F>(&self, resolver: &F, pou_name: &str) -> Result<IecValue, RuntimeError> where F: Fn(&str) -> Result<IecValue, RuntimeError>`.
  - Refactor existing `IrExpr::evaluate(&self, ctx: &ScanContext, pou_name: &str)` to delegate to `evaluate_with_resolver`.
- In `ferrite-runtime::PlcRuntime`:
  - `pub fn evaluate_expression_bool(&self, expr_str: &str, pou_name: &str) -> Option<bool>`:
    Parses `expr_str` via `ferrite_lang_st::parse_st_expression` and evaluates it with `&|sym| self.read_value(sym)`.

---

### 5.2 Desktop Studio Enhancements (`ferrite-gui`)

#### 1. Simulation Speed Controller
- Introduce `SimSpeed`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum SimSpeed {
      Slow0_1x,
      #[default]
      Realtime1x,
      Fast2x,
      Fast5x,
      Fast10x,
      Max,
  }
  ```
- In `FerriteStudioApp`:
  - Add `pub sim_speed: SimSpeed`.
  - In `subscription()`:
    - `Slow0_1x`: timer interval = `(scan_cycle_ms * 10).max(50) ms`, batch = 1.
    - `Realtime1x`: timer interval = `scan_cycle_ms.max(5) ms`, batch = 1.
    - `Fast2x`: timer interval = `scan_cycle_ms.max(5) ms`, batch = 2.
    - `Fast5x`: timer interval = `scan_cycle_ms.max(5) ms`, batch = 5.
    - `Fast10x`: timer interval = `scan_cycle_ms.max(5) ms`, batch = 10.
    - `Max`: timer interval = `10 ms`, batch = 50.
  - In `Message::PlcTickTimer`:
    - Calls `rt.step_scan_batch(batch_size)`.
    - If `breakpoint_hit`: automatically sets `self.sim_running = false`, formats status banner, and switches dock to Diagnostics.
  - In Toolbar: Place segmented buttons `[Speed: 0.1x | 1x | 2x | 5x | 10x | Max]`.

#### 2. Global "FORCES ACTIVE" Safety Banner
- In header bar of `FerriteStudioApp`:
  - Query `runtime.active_forces_count()`.
  - If > 0, render high-visibility amber/red banner:
    ```
    [ ⚠️ 3 FORCES ACTIVE ]   [ Unforce All ]
    ```
  - Clicking `[Unforce All]` emits `Message::UnforceAllTags`, immediately clearing all forces.
- In `I/O Rack Panel`:
  - For non-boolean tags (`REAL`, `INT`, `DINT`), provide numeric input with `[Force]` button.

#### 3. Interactive Breakpoint Authoring & Hit Alerts
- In Diagnostics Bottom Tab:
  - Add "Breakpoints" section:
    - Active breakpoint list with condition, parameters, and `[Remove]` button.
    - Add form: Condition selector (`Scan Count`, `Symbol True`), Value input, and `[+ Add Breakpoint]` button.
- On Breakpoint Hit:
  - Immediately pauses simulation.
  - Displays banner: `⚠️ Breakpoint Hit! Scan #142 (virtual time 1420ms): Symbol 'Emergency_Stop' is TRUE`.

#### 4. SFC Canvas Live Compound Evaluation & Step Timers
- In `canvas_sfc.rs`:
  - Update `evaluate_receptivity`:
    Delegate to `rt.evaluate_expression_bool(receptivity, pou_name)`.
    - If `Some(true)`: render wire and cross-bar in bright emerald green (`COLOR_ACTIVE_STEP`).
    - If `Some(false)`: render in muted slate (`COLOR_INACTIVE_STEP`).
  - Update step box drawing:
    When a step is active and has an elapsed duration (`elapsed_ms > 0`), render a timer badge: `⏱ 4.2s`.

#### 5. Cold vs. Warm Restart Controls
- On the `Reset` toolbar button:
  - Split into `Reset (Warm)` (default quick restart preserving retain data and active forces) and `Reset (Cold)` (complete memory and force wipe).

---

## 6. Verification Plan

1. **Automated Unit Tests**:
   - `test_runtime_sim_speed_batching`: Verify multi-tick batch execution without state drift and early exit on breakpoint hit.
   - `test_runtime_warm_vs_cold_restart`: Verify retain tags and %M markers survive warm restart while non-retain tags revert; verify cold restart resets everything.
   - `test_runtime_active_forces_count_and_unforce_all`: Verify all force bits in `ProcessImage` and `TagTable` clear on `unforce_all()`.
   - `test_runtime_compound_receptivity_evaluation`: Verify complex ST expressions like `Start_PB AND (Level >= 50)` evaluate accurately in read-only mode.
   - `test_breakpoint_hit_telemetry`: Verify `ScanTelemetry` accurately populates `hit_breakpoint` and pauses execution.
2. **Quality & Workspace Gates**:
   - `cargo check --workspace --all-targets` (0 errors)
   - `cargo test --workspace` (all tests passing)
   - `cargo clippy --workspace --all-targets -- -D warnings` (clean, 0 warnings)
