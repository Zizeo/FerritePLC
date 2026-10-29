# Studio Header Controls & Warm Restart Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compact the speed selection into a styled dropdown menu, make the header menu bar horizontally scrollable, remove tab icons/glyphs, and fix warm restart ("hot reset") to re-initialize process image inputs, reset Function Blocks, auto-pause simulation, and synchronize UI state.

**Architecture:** Extend `ferrite-core` with FB accumulator resetting, update `ferrite-runtime` `warm_restart` to re-initialize process image direct addresses and re-latch inputs while preserving retain tags and active forces, and upgrade `ferrite-gui` header controls with an Iced `pick_list` dropdown, horizontal `scrollable`, and immediate UI synchronization on warm restart.

**Tech Stack:** Rust 2024 Edition, Iced 0.13, `thiserror`, `ferrite-core`, `ferrite-runtime`, `ferrite-gui`.

**Spec:** [`docs/superpowers/specs/2026-10-30-studio-header-and-hot-reset-design.md`](file:///C:/Users/Elio/prog/FerritePLC/docs/superpowers/specs/2026-10-30-studio-header-and-hot-reset-design.md)

## Global Constraints
- Rust 2024 Edition across all crates.
- Zero `unwrap()` / `expect()` in library crates (`thiserror` throughout).
- Real-time safety: zero heap allocations in the scan loop hot path.
- All file links must be clickable `file:///` URLs.

## Review Focus
1. `SimSpeed` serialization and `Display` implementation must match existing `.label()` strings exactly (`"0.1x"`, `"1x"`, `"2x"`, `"5x"`, `"10x"`, `"Max"`).
2. `PlcRuntime::warm_restart()` must preserve active direct address forces (`%IX`, `%QX`, `%MX`) and symbolic tag forces while still re-latching initial hardware inputs.
3. Standard Function Blocks (`TON`, `TOF`, `TP`, `CTU`, `CTD`, `SR`, `RS`) must have internal elapsed times and counts cleared on warm reset unless retentive.
4. `Message::ResetSimWarm` in `ferrite-gui` must set `self.sim_running = false` (auto-pause) and update `self.shared_io` and `self.modbus_shared_pi` synchronously so the UI reflects the reset state without requiring an extra tick.
5. The horizontal scrollable header must not clip or vertically constrain child dropdowns (`pick_list` overlay).

---

### Task 1: Function Block & Tag Table Accumulator Reset (`ferrite-core`)

**Files:**
- Modify: `crates/ferrite-core/src/fb.rs:160-220`
- Modify: `crates/ferrite-core/src/tag_table.rs:380-386`
- Test: `crates/ferrite-core/src/lib.rs:80-120`

**Interfaces:**
- Consumes: `FbInstance`, `TagTable`
- Produces:
  - `FbInstance::reset(&mut self)`
  - `TagTable::reset_fbs(&mut self)`

- [ ] **Step 1: Write the failing test**
In `crates/ferrite-core/src/lib.rs`:
```rust
#[test]
fn test_fb_instance_and_tag_table_reset() -> Result<(), Box<dyn std::error::Error>> {
    let mut table = TagTable::new();
    let mut fb = FbInstance::new("Timer_1", StandardFbKind::Ton);
    fb.et_ms = 4500;
    fb.q = true;
    fb.in_bool = true;
    table.register_fb(fb);

    assert_eq!(table.get_fb("Timer_1").map(|f| f.et_ms), Some(4500));
    table.reset_fbs();
    assert_eq!(table.get_fb("Timer_1").map(|f| f.et_ms), Some(0));
    assert_eq!(table.get_fb("Timer_1").map(|f| f.q), Some(false));
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p ferrite-core --lib test_fb_instance_and_tag_table_reset`
Expected: FAIL with "no method named `reset_fbs` found for struct `TagTable`"

- [ ] **Step 3: Implement `FbInstance::reset` and `TagTable::reset_fbs`**
In `crates/ferrite-core/src/fb.rs`:
```rust
impl FbInstance {
    pub fn reset(&mut self) {
        self.in_bool = false;
        self.r_bool = false;
        self.s_bool = false;
        self.ld_bool = false;
        self.q = false;
        self.et_ms = 0;
        self.cv = 0;
        self.prev_in = false;
        self.prev_cd = false;
        self.prev_clk = false;
        self.start_time_ms = 0;
        self.running = false;
    }
}
```
In `crates/ferrite-core/src/tag_table.rs`:
```rust
impl TagTable {
    pub fn reset_fbs(&mut self) {
        for fb in &mut self.fbs {
            fb.reset();
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p ferrite-core --lib test_fb_instance_and_tag_table_reset`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add crates/ferrite-core/src/fb.rs crates/ferrite-core/src/tag_table.rs crates/ferrite-core/src/lib.rs
git commit -m "feat(core): add accumulator and state reset for Function Blocks in TagTable"
```

---

### Task 2: Warm Restart Logic & Input Re-Latching (`ferrite-runtime`)

**Files:**
- Modify: `crates/ferrite-runtime/src/lib.rs:336-360`
- Test: `crates/ferrite-runtime/src/lib.rs:930-960`

**Interfaces:**
- Consumes: `TagTable::reset_fbs`, `ProcessImage::latch_inputs`
- Produces: `PlcRuntime::warm_restart(&mut self) -> Result<(), RuntimeError>`

- [ ] **Step 1: Write the failing test**
In `crates/ferrite-runtime/src/lib.rs`:
Add test `test_runtime_warm_restart_relatches_inputs_and_resets_fbs` verifying:
1. An initial hardware input `%IX0.0` bound to a tag with initial value `TRUE` is forced or mutated to `FALSE`.
2. A timer FB has `et_ms = 5000`.
3. `warm_restart()` is called.
4. `%IX0.0` is re-latched to `TRUE` (initial value), timer `et_ms` is 0, while RETAIN tags and manual forces are preserved.

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p ferrite-runtime --lib test_runtime_warm_restart_relatches_inputs_and_resets_fbs`
Expected: FAIL

- [ ] **Step 3: Implement re-initialization in `PlcRuntime::warm_restart`**
In `crates/ferrite-runtime/src/lib.rs`:
```rust
    pub fn warm_restart(&mut self) -> Result<(), RuntimeError> {
        let retain_tags = self.tags.export_retain_values();
        let markers = self.process_image.export_markers();
        self.process_image.clear();
        self.process_image.import_markers(&markers);
        self.tags.reset_values_to_initial();
        self.tags.import_retain_values(&retain_tags);

        // Re-initialize direct address bindings with tag initial values and re-latch inputs
        for tag in self.tags.tags() {
            if let Some(addr) = tag.address {
                if addr.area == MemoryArea::Input {
                    let _ = self.process_image.set_hw_input(addr, &tag.initial_value);
                } else {
                    let _ = self.process_image.write(addr, &tag.initial_value);
                }
            }
        }
        self.process_image.latch_inputs();

        self.tags.reset_fbs();
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
```

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p ferrite-runtime --lib test_runtime_warm_restart_relatches_inputs_and_resets_fbs`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add crates/ferrite-runtime/src/lib.rs
git commit -m "fix(runtime): re-initialize process image inputs and reset function blocks on warm restart"
```

---

### Task 3: SimSpeed Dropdown, Scrollable Header, and Glyph Removal (`ferrite-gui`)

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:70-105, 3620-3770`
- Test: `crates/ferrite-gui/src/lib.rs:770-800`

**Interfaces:**
- Consumes: `SimSpeed`, `StudioTheme::styled_pick_list`, `scrollable`
- Produces: `SimSpeed::ALL`, `impl Display for SimSpeed`

- [ ] **Step 1: Write the failing test**
In `crates/ferrite-gui/src/lib.rs`:
```rust
#[test]
fn test_sim_speed_display_and_options() {
    use crate::app::SimSpeed;
    assert_eq!(SimSpeed::ALL.len(), 6);
    assert_eq!(SimSpeed::Slow0_1x.to_string(), "0.1x");
    assert_eq!(SimSpeed::Realtime1x.to_string(), "1x");
    assert_eq!(SimSpeed::Fast2x.to_string(), "2x");
    assert_eq!(SimSpeed::Fast5x.to_string(), "5x");
    assert_eq!(SimSpeed::Fast10x.to_string(), "10x");
    assert_eq!(SimSpeed::Max.to_string(), "Max");
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p ferrite-gui --lib test_sim_speed_display_and_options`
Expected: FAIL with missing `SimSpeed::ALL` and `Display` implementation.

- [ ] **Step 3: Implement `Display`, `SimSpeed::ALL`, `pick_list`, horizontal `scrollable`, and clean labels**
1. In `crates/ferrite-gui/src/app.rs`:
```rust
impl SimSpeed {
    pub const ALL: [SimSpeed; 6] = [
        SimSpeed::Slow0_1x,
        SimSpeed::Realtime1x,
        SimSpeed::Fast2x,
        SimSpeed::Fast5x,
        SimSpeed::Fast10x,
        SimSpeed::Max,
    ];
    ...
}

impl std::fmt::Display for SimSpeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}
```
2. In `view_header`:
- Replace speed button row with:
  ```rust
  let speed_group = row![
      text(self.ui_lang.tr("Speed:", "Vitesse:"))
          .size(11)
          .color(StudioTheme::TEXT_MUTED),
      pick_list(&SimSpeed::ALL[..], Some(self.sim_speed), Message::SetSimSpeed)
          .style(StudioTheme::styled_pick_list)
          .padding([3, 6])
          .width(72),
  ]
  .spacing(4)
  .align_y(Alignment::Center);
  ```
- Change menu button labels from `"File ▾"` to `"File"`, `"PLC ▾"` to `"PLC"`, `"Panels ▾"` to `"Panels"`, `"Options ▾"` to `"Options"`.
- Wrap header contents row in:
  ```rust
  container(
      scrollable(
          row![
              text("FerritePLC").size(15),
              menu_bar,
              telemetry_pill,
              forces_banner,
              iced::widget::Space::with_width(12),
              run_btn,
              step_btn,
              reset_warm_btn,
              reset_cold_btn,
              iced::widget::Space::with_width(6),
              speed_group,
              iced::widget::Space::with_width(8),
              save_btn,
              check_btn,
              undo_btn,
              redo_btn,
              scenarios_btn,
              iced::widget::Space::with_width(Length::Fill),
              lang_btn,
              explorer_toggle,
              dock_toggle,
          ]
          .spacing(6)
          .align_y(Alignment::Center)
      )
      .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::default()))
      .width(Length::Fill)
  )
  ```

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p ferrite-gui --lib test_sim_speed_display_and_options`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add crates/ferrite-gui/src/app.rs crates/ferrite-gui/src/lib.rs
git commit -m "feat(gui): convert speed control to dropdown, make header scrollable, and clean menu labels"
```

---

### Task 4: Auto-Pause and Instant UI State Synchronization on Warm Restart (`ferrite-gui`)

**Files:**
- Modify: `crates/ferrite-gui/src/app.rs:1018-1035`
- Test: `crates/ferrite-gui/src/lib.rs:780-820`

**Interfaces:**
- Consumes: `PlcRuntime::warm_restart`, `SharedIoHandle`
- Produces: `Message::ResetSimWarm` handling with auto-pause and `shared_io` sync.

- [ ] **Step 1: Write the failing test**
In `crates/ferrite-gui/src/lib.rs`:
```rust
#[test]
fn test_warm_restart_auto_pauses_and_syncs_shared_io() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join("ferrite_test_warm_restart_ui");
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_warm")?;
    let mut app = FerriteStudioApp::new(scaffold);

    app.sim_running = true;
    let _ = app.update(Message::ResetSimWarm);
    assert!(!app.sim_running, "Warm restart must auto-pause the simulation");

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p ferrite-gui --lib test_warm_restart_auto_pauses_and_syncs_shared_io`
Expected: FAIL with "assertion failed: !app.sim_running"

- [ ] **Step 3: Update `Message::ResetSimWarm` in `crates/ferrite-gui/src/app.rs`**
In `crates/ferrite-gui/src/app.rs`:
```rust
            Message::ResetSimWarm => {
                self.sim_running = false;
                if let Some(rt) = &mut self.runtime {
                    let _ = rt.warm_restart();
                    if let Ok(mut snap) = self.shared_io.write() {
                        for tag in rt.tags.tags() {
                            if let Ok(v) = rt.read_value(&tag.name) {
                                snap.values.insert(tag.name.clone(), v);
                            }
                        }
                    }
                    if self.modbus_server.is_some()
                        && let Ok(mut pi_lock) = self.modbus_shared_pi.write()
                    {
                        *pi_lock = rt.process_image.clone();
                    }
                } else {
                    self.recompile_runtime();
                }
                self.trend_plotter.clear_all();
                self.status_banner = self
                    .ui_lang
                    .tr(
                        "Warm restart performed (simulation paused, retain memory preserved).",
                        "Redémarrage à chaud effectué (simulation en pause, mémoire rémanente conservée).",
                    )
                    .to_string();
                Task::none()
            }
```

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p ferrite-gui --lib test_warm_restart_auto_pauses_and_syncs_shared_io`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add crates/ferrite-gui/src/app.rs crates/ferrite-gui/src/lib.rs
git commit -m "fix(gui): auto-pause simulation and sync shared I/O immediately on warm restart"
```

---

### Task 5: Full Verification Loop Across Workspace

**Files:**
- Test: All workspace crates

- [ ] **Step 1: Run `cargo check --workspace --all-targets`**
Expected: PASS (0 errors)

- [ ] **Step 2: Run `cargo test --workspace`**
Expected: PASS (all tests pass)

- [ ] **Step 3: Run `cargo clippy --workspace --all-targets -- -D warnings`**
Expected: PASS (0 warnings)

- [ ] **Step 4: Commit any cleanup or polish**
```bash
git commit --allow-empty -m "chore(release): verify studio header controls and hot reset fix across workspace"
```
