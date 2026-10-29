# Simulation Execution & Commissioning Controls Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform FerritePLC into an agile, highly observable, and safety-compliant virtual commissioning simulator by implementing variable simulation speed with runtime batching, safety-critical force monitoring and global release, reactive and authorable breakpoints, live compound SFC receptivity evaluation, and IEC 61131-3 Cold vs. Warm restart.

**Architecture:** Extend `ferrite-core` and `ferrite-ir` with POU reset hooks, process image force counts, and a pure read-only ST expression evaluator (`evaluate_with_resolver`). In `ferrite-runtime`, implement multi-cycle scan batching (`step_scan_batch`), breakpoint metadata telemetry, active force unforcing, and Cold/Warm restart with retain memory preservation. In `ferrite-gui`, introduce `SimSpeed` clock controls, a global amber/red "FORCES ACTIVE" safety banner, an analog tag forcing tool, a Diagnostics breakpoint manager, and animated SFC compound receptivity wires and elapsed step timers.

**Tech Stack:** Rust 2024 Edition, `iced` 0.13, `thiserror`, `ferrite-core`, `ferrite-ir`, `ferrite-lang-st`, `ferrite-lang-sfc`, `ferrite-runtime`, `ferrite-gui`.

**Spec:** [docs/superpowers/specs/2026-10-29-simulation-commissioning-controls-design.md](file:///C:/Users/Elio/prog/FerritePLC/docs/superpowers/specs/2026-10-29-simulation-commissioning-controls-design.md)

## Global Constraints

- Rust 2024 Edition across the workspace.
- Zero `unwrap()` / `expect()` in library crates (`ferrite-core`, `ferrite-ir`, `ferrite-lang-*`, `ferrite-runtime`).
- Zero-heap allocation in the `step_scan()` and `step_scan_batch()` execution hot path.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass with 0 warnings.
- All variable forces and temporary overrides must remain strictly in memory and never mutate `.fplc` or source files on disk.

## Review Focus

1. **Division by zero or invalid syntax in live SFC receptivities:** When the user types an incomplete or invalid expression (e.g. `Start_PB AND`), `parse_st_expression` returns an error; `evaluate_expression_bool` must return `None` (rendering in inactive slate) without panicking.
2. **Breakpoint hit in the middle of a scan batch:** When running at `Fast10x` (batch size 10) or `Max` (batch size 50), if a breakpoint triggers at cycle 3, `step_scan_batch` must halt immediately, return cycle 3 telemetry with `breakpoint_hit = true`, and not execute cycles 4..50.
3. **Simultaneous forces across `ProcessImage` and `TagTable`:** When both a direct address `%IX0.0` and a symbolic tag `Start_PB` are forced, `active_forces_count()` must report 2, and `unforce_all()` must clear both.
4. **Warm restart retain tag and `%M` marker preservation:** Tags with `retain: true` and all `%M` markers must survive `warm_restart()` along with active forces, while non-retain tags revert to `initial_value` and SFC steps reset to initial steps.
5. **Cold restart full clean-up:** `cold_restart()` must reset all tags (retain and non-retain) to defaults, clear all `%I`, `%Q`, `%M` process image memory, wipe all active forces, and reset all SFC steps to initial steps.

---

### Task 1: Core Trait Reset Hook and SFC POU Reset Implementation

**Files:**
- Modify: `crates/ferrite-core/src/context.rs:52-58`
- Modify: `crates/ferrite-lang-sfc/src/lib.rs:1050-1070`
- Test: `crates/ferrite-lang-sfc/src/lib.rs:2000-2050`

**Interfaces:**
- Consumes: `ExecutablePou` trait in `ferrite-core::context`.
- Produces: `ExecutablePou::reset(&mut self)` method with default no-op implementation; `CompiledSfcPou::reset(&mut self)` resetting active steps back to `initial`, zeroing `elapsed_ms`, and clearing scratch buffers.

- [ ] **Step 1: Write failing unit test in `ferrite-lang-sfc`**

Add unit test `test_compiled_sfc_pou_reset()` in `crates/ferrite-lang-sfc/src/lib.rs` verifying that calling `pou.reset()` resets steps to initial configuration and clears elapsed time after execution:
```rust
#[test]
fn test_compiled_sfc_pou_reset() -> Result<(), Box<dyn std::error::Error>> {
    let mut sfc = SfcProgram::new("test_reset");
    sfc.add_step(SfcStep::initial("0"));
    sfc.add_step(SfcStep::normal("1"));
    sfc.add_transition(SfcTransition::new("T0", "TRUE").with_from("0").with_to("1"));
    let mut tags = TagTable::new();
    let mut compiled = sfc.compile(&mut tags)?;
    let mut pi = ProcessImage::new(128);
    let mut ctx = ScanContext::new(&mut pi, &mut tags, 50, 1);
    compiled.execute(&mut ctx)?;
    assert!(ctx.read_bool("test_reset.X1.state")?);
    compiled.reset();
    let mut ctx2 = ScanContext::new(&mut pi, &mut tags, 50, 2);
    compiled.execute(&mut ctx2)?;
    assert!(ctx2.read_bool("test_reset.X0.state")?);
    assert!(!ctx2.read_bool("test_reset.X1.state")?);
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-lang-sfc -- test_compiled_sfc_pou_reset`
Expected: FAIL (`no method named reset found for struct CompiledSfcPou`).

- [ ] **Step 3: Implement `reset(&mut self)` in `ExecutablePou` and `CompiledSfcPou`**

In `crates/ferrite-core/src/context.rs`:
```rust
pub trait ExecutablePou: Send {
    fn id(&self) -> PouId;
    fn kind(&self) -> LanguageKind;
    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError>;
    /// Resets internal POU state (e.g. sequence steps and timers) on cold/warm restart.
    fn reset(&mut self) {}
}
```

In `crates/ferrite-lang-sfc/src/lib.rs` under `impl ExecutablePou for CompiledSfcPou`:
```rust
    fn reset(&mut self) {
        self.initialized = false;
        for step in &mut self.steps {
            step.active = step.initial;
            step.prev_active = false;
            step.elapsed_ms = 0;
        }
        self.fired_scratch.fill(false);
        self.clear_scratch.fill(false);
        self.activate_scratch.fill(false);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ferrite-lang-sfc -- test_compiled_sfc_pou_reset`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-core/src/context.rs crates/ferrite-lang-sfc/src/lib.rs
git commit -m "feat(core,sfc): add ExecutablePou::reset and implement for CompiledSfcPou"
```

---

### Task 2: Core Force Helpers and Pure Read-Only Expression Evaluation

**Files:**
- Modify: `crates/ferrite-core/src/process_image.rs:170-195`
- Modify: `crates/ferrite-core/src/tag_table.rs:330-362`
- Modify: `crates/ferrite-ir/src/lib.rs:88-140`
- Test: `crates/ferrite-ir/src/lib.rs:1000-1050`
- Test: `crates/ferrite-core/src/process_image.rs:250-280`

**Interfaces:**
- Consumes: `TagTable`, `ProcessImage`, `IrExpr`.
- Produces:
  - `ProcessImage::active_forces_count(&self) -> usize`
  - `ProcessImage::unforce_all(&mut self)`
  - `TagTable::active_forces_count(&self) -> usize`
  - `TagTable::unforce_all(&mut self)`
  - `TagTable::reset_values_to_initial(&mut self)`
  - `IrExpr::evaluate_with_resolver<F>(&self, resolver: &F, pou_name: &str) -> Result<IecValue, RuntimeError> where F: Fn(&str) -> Result<IecValue, RuntimeError>`

- [ ] **Step 1: Write failing unit tests for force helpers and expression resolver**

In `crates/ferrite-core/src/process_image.rs`:
```rust
#[test]
fn test_process_image_forces_count_and_unforce_all() -> Result<(), Box<dyn std::error::Error>> {
    let mut pi = ProcessImage::new(64);
    let in_addr: DirectAddress = "%IX0.0".parse()?;
    let out_addr: DirectAddress = "%QX0.1".parse()?;
    pi.force(in_addr, &IecValue::Bool(true))?;
    pi.force(out_addr, &IecValue::Bool(true))?;
    assert_eq!(pi.active_forces_count(), 2);
    pi.unforce_all();
    assert_eq!(pi.active_forces_count(), 0);
    Ok(())
}
```

In `crates/ferrite-ir/src/lib.rs`:
```rust
#[test]
fn test_ir_expr_evaluate_with_resolver() -> Result<(), Box<dyn std::error::Error>> {
    let expr = IrExpr::Binary {
        op: IrBinaryOp::And,
        left: Box::new(IrExpr::Symbol("Start_PB".to_string())),
        right: Box::new(IrExpr::Symbol("Safety_OK".to_string())),
    };
    let lookup = |sym: &str| match sym {
        "Start_PB" => Ok(IecValue::Bool(true)),
        "Safety_OK" => Ok(IecValue::Bool(true)),
        _ => Err(CoreError::UnknownSymbol(sym.to_string()).into()),
    };
    let res = expr.evaluate_with_resolver(&lookup, "main")?;
    assert_eq!(res, IecValue::Bool(true));
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p ferrite-core -- test_process_image_forces_count_and_unforce_all`
Expected: FAIL (`no method named active_forces_count found for struct ProcessImage`).

- [ ] **Step 3: Implement force helpers and `evaluate_with_resolver`**

In `crates/ferrite-core/src/process_image.rs`:
```rust
    pub fn active_forces_count(&self) -> usize {
        let count_bits = |slice: &[u8]| -> usize {
            slice.iter().map(|b| b.count_ones() as usize).sum()
        };
        count_bits(&self.force_mask_inputs)
            + count_bits(&self.force_mask_outputs)
            + count_bits(&self.force_mask_markers)
    }

    pub fn unforce_all(&mut self) {
        self.force_mask_inputs.fill(0);
        self.force_val_inputs.fill(0);
        self.force_mask_outputs.fill(0);
        self.force_val_outputs.fill(0);
        self.force_mask_markers.fill(0);
        self.force_val_markers.fill(0);
    }
```

In `crates/ferrite-core/src/tag_table.rs`:
```rust
    pub fn active_forces_count(&self) -> usize {
        self.tags.iter().filter(|t| t.forced.is_some()).count()
    }

    pub fn unforce_all(&mut self) {
        for t in &mut self.tags {
            t.forced = None;
        }
    }

    pub fn reset_values_to_initial(&mut self) {
        for t in &mut self.tags {
            t.value = Some(t.initial_value.clone());
        }
    }
```

In `crates/ferrite-ir/src/lib.rs`:
Implement `pub fn evaluate_with_resolver<F>(&self, resolver: &F, pou_name: &str) -> Result<IecValue, RuntimeError> where F: Fn(&str) -> Result<IecValue, RuntimeError>`.
Refactor existing `IrExpr::evaluate(&self, ctx: &ScanContext<'_>, pou_name: &str)` to:
```rust
self.evaluate_with_resolver(&|sym| ctx.read_symbol(sym), pou_name)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ferrite-core -p ferrite-ir`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-core/src/process_image.rs crates/ferrite-core/src/tag_table.rs crates/ferrite-ir/src/lib.rs
git commit -m "feat(core,ir): add active force helpers and IrExpr::evaluate_with_resolver"
```

---

### Task 3: Runtime Scan Batching & Breakpoint Telemetry

**Files:**
- Modify: `crates/ferrite-runtime/src/lib.rs:56-65, 280-340`
- Test: `crates/ferrite-runtime/src/lib.rs:650-700`

**Interfaces:**
- Consumes: `ScanTelemetry`, `Breakpoint`, `PlcRuntime`.
- Produces:
  - `ScanTelemetry.hit_breakpoint: Option<Breakpoint>`
  - `PlcRuntime::step_scan_batch(&mut self, batch_size: usize) -> Result<ScanTelemetry, RuntimeError>`

- [ ] **Step 1: Write failing unit tests for `step_scan_batch` and breakpoint telemetry**

In `crates/ferrite-runtime/src/lib.rs`:
```rust
#[test]
fn test_runtime_sim_speed_batching_and_breakpoint() -> Result<(), Box<dyn std::error::Error>> {
    let mut rt = create_test_runtime(); // helper with counter POU
    rt.breakpoints.push(Breakpoint::ScanCount(3));
    let tel = rt.step_scan_batch(10)?;
    assert!(tel.breakpoint_hit);
    assert_eq!(tel.scan_count, 3);
    assert_eq!(tel.hit_breakpoint, Some(Breakpoint::ScanCount(3)));
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ferrite-runtime -- test_runtime_sim_speed_batching_and_breakpoint`
Expected: FAIL (`no method named step_scan_batch found for struct PlcRuntime`).

- [ ] **Step 3: Implement `hit_breakpoint` in `ScanTelemetry` and `step_scan_batch` in `PlcRuntime`**

In `crates/ferrite-runtime/src/lib.rs`:
1. Add `pub hit_breakpoint: Option<Breakpoint>` to `ScanTelemetry`.
2. In `step_scan()`:
   When evaluating `self.breakpoints`, on match:
   ```rust
   let hit_bp = self.breakpoints.iter().find(|bp| match bp {
       Breakpoint::ScanCount(n) => self.scan_count >= *n,
       Breakpoint::Pou(id) => self.tasks.iter().any(|t| t.pous.iter().any(|p| p.id() == *id)),
       Breakpoint::SymbolTrue(sym) => self.read_value(sym).map(|v| v.as_bool().unwrap_or(false)).unwrap_or(false),
   }).cloned();
   let breakpoint_hit = hit_bp.is_some();
   ```
3. Implement `step_scan_batch`:
   ```rust
   pub fn step_scan_batch(&mut self, batch_size: usize) -> Result<ScanTelemetry, RuntimeError> {
       let count = batch_size.max(1);
       let mut latest_tel = self.step_scan()?;
       if latest_tel.breakpoint_hit {
           return Ok(latest_tel);
       }
       for _ in 1..count {
           latest_tel = self.step_scan()?;
           if latest_tel.breakpoint_hit {
               break;
           }
       }
       Ok(latest_tel)
   }
   ```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p ferrite-runtime -- test_runtime_sim_speed_batching_and_breakpoint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-runtime/src/lib.rs
git commit -m "feat(runtime): implement step_scan_batch and enrich ScanTelemetry with hit_breakpoint"
```

---

### Task 4: Runtime Safety Forces & IEC Cold/Warm Restart

**Files:**
- Modify: `crates/ferrite-runtime/src/lib.rs:200-270, 500-565`
- Test: `crates/ferrite-runtime/src/lib.rs:700-750`

**Interfaces:**
- Consumes: `ProcessImage`, `TagTable`, `ExecutablePou::reset`.
- Produces:
  - `PlcRuntime::active_forces_count(&self) -> usize`
  - `PlcRuntime::unforce_all(&mut self) -> Result<(), RuntimeError>`
  - `PlcRuntime::cold_restart(&mut self) -> Result<(), RuntimeError>`
  - `PlcRuntime::warm_restart(&mut self) -> Result<(), RuntimeError>`
  - `PlcRuntime::evaluate_expression_bool(&self, expr_str: &str, pou_name: &str) -> Option<bool>`

- [ ] **Step 1: Write failing unit tests for Cold vs. Warm restart and active forces**

In `crates/ferrite-runtime/src/lib.rs`:
```rust
#[test]
fn test_runtime_warm_vs_cold_restart_and_unforce_all() -> Result<(), Box<dyn std::error::Error>> {
    let mut rt = create_test_runtime();
    rt.tags.get_tag_mut("Counter_Val").map(|t| t.retain = true);
    let _ = rt.force("Counter_Val", IecValue::Int(42));
    let _ = rt.force("%IX0.0", IecValue::Bool(true));
    assert_eq!(rt.active_forces_count(), 2);

    // Warm restart: retains Counter_Val, preserves forces
    rt.warm_restart()?;
    assert_eq!(rt.active_forces_count(), 2);
    assert_eq!(rt.read_value("Counter_Val")?, IecValue::Int(42));

    // Cold restart: clears forces and resets Counter_Val
    rt.cold_restart()?;
    assert_eq!(rt.active_forces_count(), 0);
    assert_ne!(rt.read_value("Counter_Val")?, IecValue::Int(42));
    Ok(())
}

#[test]
fn test_runtime_compound_receptivity_evaluation() -> Result<(), Box<dyn std::error::Error>> {
    let mut rt = create_test_runtime();
    let _ = rt.force("Start_PB", IecValue::Bool(true));
    let _ = rt.force("Safety_OK", IecValue::Bool(true));
    assert_eq!(rt.evaluate_expression_bool("Start_PB AND Safety_OK", "main"), Some(true));
    assert_eq!(rt.evaluate_expression_bool("Start_PB AND NOT Safety_OK", "main"), Some(false));
    assert_eq!(rt.evaluate_expression_bool("Invalid Syntax &^%", "main"), None);
    Ok(())
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p ferrite-runtime -- test_runtime_warm_vs_cold_restart_and_unforce_all`
Expected: FAIL (`no method named warm_restart found for struct PlcRuntime`).

- [ ] **Step 3: Implement `active_forces_count`, `unforce_all`, `cold_restart`, `warm_restart`, and `evaluate_expression_bool`**

In `crates/ferrite-runtime/src/lib.rs`:
```rust
    pub fn active_forces_count(&self) -> usize {
        self.tags.active_forces_count() + self.process_image.active_forces_count()
    }

    pub fn unforce_all(&mut self) -> Result<(), RuntimeError> {
        self.tags.unforce_all();
        self.process_image.unforce_all();
        Ok(())
    }

    pub fn cold_restart(&mut self) -> Result<(), RuntimeError> {
        self.unforce_all()?;
        self.process_image.clear();
        self.tags.reset_values_to_initial();
        self.alarms.clear();
        self.alarm_journal.clear();
        self.scan_count = 0;
        self.virtual_time_ms = 0;
        self.last_scan_duration_us = 0;
        self.max_scan_duration_us = 0;
        for task in &mut self.tasks {
            task.elapsed_ms = 0;
            task.last_trigger_state = false;
            task.execution_count = 0;
            for pou in &mut task.pous {
                pou.reset();
            }
        }
        Ok(())
    }

    pub fn warm_restart(&mut self) -> Result<(), RuntimeError> {
        let retain_tags = self.tags.export_retain_values();
        let markers = self.process_image.export_markers();
        self.process_image.clear();
        self.process_image.import_markers(&markers);
        self.tags.reset_values_to_initial();
        self.tags.import_retain_values(&retain_tags);
        self.scan_count = 0;
        self.virtual_time_ms = 0;
        self.last_scan_duration_us = 0;
        for task in &mut self.tasks {
            task.elapsed_ms = 0;
            task.last_trigger_state = false;
            task.execution_count = 0;
            for pou in &mut task.pous {
                pou.reset();
            }
        }
        Ok(())
    }

    pub fn evaluate_expression_bool(&self, expr_str: &str, pou_name: &str) -> Option<bool> {
        let trimmed = expr_str.trim();
        if trimmed.is_empty() || trimmed == "1" || trimmed.eq_ignore_ascii_case("true") {
            return Some(true);
        }
        if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
            return Some(false);
        }
        let expr = ferrite_lang_st::parse_st_expression(trimmed, pou_name).ok()?;
        let val = expr.evaluate_with_resolver(&|sym| self.read_value(sym), pou_name).ok()?;
        val.as_bool().ok()
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ferrite-runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-runtime/src/lib.rs
git commit -m "feat(runtime): implement cold_restart, warm_restart, unforce_all, and evaluate_expression_bool"
```

---

### Task 5: GUI Simulation Clock, Scan Batching, and Reactive Breakpoint Pausing

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:80-120, 420-450, 520-565, 800-840, 2470-2540`
- Test: Manual verification with headless check + `cargo check -p ferrite-gui`

**Interfaces:**
- Consumes: `PlcRuntime::step_scan_batch`, `ScanTelemetry.hit_breakpoint`, `SimSpeed`.
- Produces:
  - `SimSpeed` enum (`Slow0_1x`, `Realtime1x`, `Fast2x`, `Fast5x`, `Fast10x`, `Max`).
  - Toolbar speed selection buttons.
  - Automatic simulation pausing and diagnostic logging on breakpoint hit.

- [ ] **Step 1: Define `SimSpeed` and messages in `ferrite-gui::app`**

In `crates/ferrite-gui/src/app.rs`:
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

impl SimSpeed {
    pub fn batch_size(self) -> usize {
        match self {
            Self::Slow0_1x | Self::Realtime1x => 1,
            Self::Fast2x => 2,
            Self::Fast5x => 5,
            Self::Fast10x => 10,
            Self::Max => 50,
        }
    }
}
```
Add to `FerriteStudioApp`:
`pub sim_speed: SimSpeed,`
Add to `Message`:
`SetSimSpeed(SimSpeed),`

- [ ] **Step 2: Update `subscription(&self)` with speed intervals**

In `crates/ferrite-gui/src/app.rs`:
```rust
let tick_sub = if self.sim_running {
    let scan_ms = self.project.manifest.scan_cycle_ms;
    let period = match self.sim_speed {
        SimSpeed::Slow0_1x => Duration::from_millis((scan_ms * 10).max(50)),
        SimSpeed::Realtime1x | SimSpeed::Fast2x | SimSpeed::Fast5x | SimSpeed::Fast10x => {
            Duration::from_millis(scan_ms.max(5))
        }
        SimSpeed::Max => Duration::from_millis(10),
    };
    iced::time::every(period).map(|_| Message::PlcTickTimer)
} else {
    iced::Subscription::none()
};
```

- [ ] **Step 3: Update `execute_plc_scan_tick` and `Message::PlcTickTimer` for batch execution and reactive pause**

In `crates/ferrite-gui/src/app.rs`:
In `execute_plc_scan_tick(&mut self) -> Result<ScanTelemetry, RuntimeError>`:
Replace `rt.step_scan()?` with `rt.step_scan_batch(self.sim_speed.batch_size())?`.

In `update`:
```rust
Message::PlcTickTimer => {
    match self.execute_plc_scan_tick() {
        Ok(tel) => {
            if tel.breakpoint_hit {
                self.sim_running = false;
                let bp_info = match &tel.hit_breakpoint {
                    Some(Breakpoint::ScanCount(n)) => format!("Scan count #{n} reached"),
                    Some(Breakpoint::SymbolTrue(s)) => format!("Symbol '{s}' is TRUE"),
                    Some(Breakpoint::Pou(id)) => format!("POU '{id}' invoked"),
                    None => "Condition satisfied".to_string(),
                };
                self.status_banner = format!("⚠️ Breakpoint hit at scan #{} (virtual time {}ms): {}", tel.scan_count, tel.virtual_time_ms, bp_info);
                self.bottom_dock_tab = BottomDockTab::Diagnostics;
            }
        }
        Err(e) => {
            self.sim_running = false;
            self.status_banner = format!("Runtime error: {e}");
        }
    }
    Task::none()
}
Message::SetSimSpeed(speed) => {
    self.sim_speed = speed;
    Task::none()
}
```

- [ ] **Step 4: Add speed selector segmented buttons in Toolbar**

In `view_toolbar()`:
Render buttons `[0.1x]`, `[1x]`, `[2x]`, `[5x]`, `[10x]`, `[Max]`, highlighting the active speed using `StudioTheme::accent_button` vs `secondary_button`.

- [ ] **Step 5: Verify build with cargo check**

Run: `cargo check -p ferrite-gui`
Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add crates/ferrite-gui/src/app.rs
git commit -m "feat(gui): add SimSpeed batching controls and reactive breakpoint pause"
```

---

### Task 6: GUI Global Force Banner, Analog Forcing, and Breakpoint Authoring

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:80-140, 2450-2520, 3010-3050`
- Test: `cargo check -p ferrite-gui`

**Interfaces:**
- Consumes: `rt.active_forces_count()`, `rt.unforce_all()`, `Breakpoint`.
- Produces:
  - Header warning banner: `⚠️ [N] FORCES ACTIVE` + `[Unforce All]`.
  - Diagnostics Breakpoint authoring list and add form.
  - Analog tag force input in I/O Rack panel.

- [ ] **Step 1: Add Messages and State for Breakpoint Authoring & Analog Forcing**

In `crates/ferrite-gui/src/app.rs`:
Add to `FerriteStudioApp`:
- `pub bp_type_scan: bool` (true = scan count, false = symbol true)
- `pub bp_input_val: String`
- `pub force_analog_input: String`
Add to `Message`:
- `UnforceAllTags`
- `ResetSimCold`
- `ResetSimWarm`
- `AddBreakpoint(Breakpoint)`
- `RemoveBreakpoint(usize)`
- `SetBpTypeScan(bool)`
- `SetBpInputVal(String)`
- `ForceNumericTag(String, String)`

- [ ] **Step 2: Implement Handlers in `update()`**

```rust
Message::UnforceAllTags => {
    if let Some(rt) = &mut self.runtime {
        let _ = rt.unforce_all();
        self.status_banner = self.ui_lang.tr("All forces cleared.", "Tous les forçages ont été annulés.").to_string();
    }
    Task::none()
}
Message::ResetSimCold => {
    if let Some(rt) = &mut self.runtime {
        let _ = rt.cold_restart();
    } else {
        self.recompile_runtime();
    }
    self.trend_plotter.clear_all();
    self.status_banner = self.ui_lang.tr("Cold restart performed (memory wiped).", "Redémarrage à froid effectué (mémoire effacée).").to_string();
    Task::none()
}
Message::ResetSimWarm => {
    if let Some(rt) = &mut self.runtime {
        let _ = rt.warm_restart();
    } else {
        self.recompile_runtime();
    }
    self.trend_plotter.clear_all();
    self.status_banner = self.ui_lang.tr("Warm restart performed (retain memory preserved).", "Redémarrage à chaud effectué (mémoire rémanente conservée).").to_string();
    Task::none()
}
Message::AddBreakpoint(bp) => {
    if let Some(rt) = &mut self.runtime {
        rt.breakpoints.push(bp);
    }
    Task::none()
}
Message::RemoveBreakpoint(idx) => {
    if let Some(rt) = &mut self.runtime && idx < rt.breakpoints.len() {
        rt.breakpoints.remove(idx);
    }
    Task::none()
}
```

- [ ] **Step 3: Render Header Safety Banner & Diagnostics Breakpoint UI**

1. In `view_header()`:
   If `self.runtime.as_ref().map(|rt| rt.active_forces_count()).unwrap_or(0) > 0`:
   Render amber badge with count and an `[Unforce All]` button with red outline.
2. In `view_bottom_dock_content()` for `BottomDockTab::Diagnostics`:
   Render Breakpoints subsection with list of `rt.breakpoints`, each with a `[Remove]` button, and a new breakpoint input form.
3. In toolbar:
   Split reset into `[Reset (Warm)]` and `[Reset (Cold)]`.

- [ ] **Step 4: Verify build with cargo check**

Run: `cargo check -p ferrite-gui`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/ferrite-gui/src/app.rs
git commit -m "feat(gui): add global force banner, breakpoint authoring UI, and cold/warm restart buttons"
```

---

### Task 7: GUI Live Compound SFC Receptivities & Step Elapsed Timers

**Files:**
- Modify: `crates/ferrite-gui/src/canvas_sfc.rs:230-260, 1100-1160`
- Test: `cargo check -p ferrite-gui`

**Interfaces:**
- Consumes: `rt.evaluate_expression_bool`, `SfcStep.elapsed_ms`.
- Produces: Live green/slate wire illumination on compound ST receptivities; `⏱ 4.2s` badges on active steps.

- [ ] **Step 1: Update `evaluate_receptivity` to delegate to `rt.evaluate_expression_bool`**

In `crates/ferrite-gui/src/canvas_sfc.rs`:
```rust
pub fn evaluate_receptivity(rt: &PlcRuntime, pou_name: &str, receptivity: &str) -> Option<bool> {
    rt.evaluate_expression_bool(receptivity, pou_name)
}
```

- [ ] **Step 2: Render elapsed duration badge on active step boxes**

In `crates/ferrite-gui/src/canvas_sfc.rs` where step cards are rendered:
When `is_active && elapsed_ms > 0`:
Format duration:
```rust
let timer_str = if elapsed_ms < 1000 {
    format!("⏱ {elapsed_ms}ms")
} else {
    format!("⏱ {:.1}s", elapsed_ms as f32 / 1000.0)
};
```
Draw a clean badge below the step name with `COLOR_ACTIVE_STEP` border.

- [ ] **Step 3: Verify build with cargo check**

Run: `cargo check -p ferrite-gui`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add crates/ferrite-gui/src/canvas_sfc.rs
git commit -m "feat(gui): enable live compound SFC receptivity evaluation and active step timer badges"
```

---

### Task 8: Full Workspace Validation & Verification

**Files:**
- Modify: none (verification only)

**Interfaces:**
- Consumes: All 14 workspace crates.
- Produces: 100% green compilation, test suite, and linter gates.

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
git commit --allow-empty -m "chore(release): verify simulation commissioning suite across all workspace crates"
```
