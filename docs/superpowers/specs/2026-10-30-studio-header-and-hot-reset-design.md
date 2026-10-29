# Studio Header Controls & Warm Restart Fix Design

## Intent & The Why
Enhance usability of the Ferrite Studio header bar by reclaiming horizontal space, ensuring responsiveness on small or narrow viewports, refining standard desktop aesthetics, and fixing the simulation Warm Restart ("hot reset") to properly reset logic and hardware memory according to IEC 61131-3 while updating the UI state immediately.

---

## Goals & Non-Goals

### Goals
1. **Compact Speed Dropdown**: Replace the 6 horizontal buttons (`0.1x` to `Max`) with an Iced `pick_list` dropdown menu matching the studio theme.
2. **Scrollable Top Menu Bar**: Wrap the top header controls in a horizontal `scrollable` to prevent clipping or overflow when the window is narrow.
3. **Clean Menu Labels**: Remove glyph indicators (`" ▾"`) from `"File"`, `"PLC"`, `"Panels"`, and `"Options"`.
4. **Functional Warm Restart ("Hot Reset")**:
   - In `PlcRuntime::warm_restart()`: preserve RETAIN variables and active forces; reset non-retain variables; re-initialize direct addresses bound to non-retain tags in `process_image` (re-latching inputs like E-Stop / Permissive); reset Function Block accumulators (`TagTable::reset_fbs()`); reset tasks and POUs.
   - In `ferrite-gui`: auto-pause the simulation (`self.sim_running = false`), and immediately sync `shared_io` and `modbus_shared_pi` with fresh values so the canvas, Digital Twin, and I/O rack update visually even when paused.

### Non-Goals
- Changing cold restart semantics (cold restart already clears all memory and forces).
- Redesigning the bottom dock or workspace tabs.

---

## Constraints
- **Rust 2024 Edition** across all crates.
- **Zero unwrap/expect in library crates** (`ferrite-core`, `ferrite-runtime`, `ferrite-gui`).
- **Real-Time Safety**: Zero heap allocations in the scan loop hot path.

---

## Approaches Evaluated

- **Approach A (Selected): True IEC 61131-3 Warm Restart with Instant State Sync**:
  - Granularly resets non-retain tags and Function Blocks in the existing runtime instance without reallocating tasks or arenas.
  - Re-latches configured initial hardware inputs in `process_image`.
  - Auto-pauses and synchronizes `shared_io` and `modbus_shared_pi` in GUI.
  - Replaces speed buttons with a styled `pick_list` and wraps header in a horizontal `scrollable`.
  - *Trade-off*: Touches `ferrite-core`, `ferrite-runtime`, and `ferrite-gui`, but guarantees allocation-free deterministic operation and immediate UI reflection.

- **Approach B (Rejected): Snapshot + Recompilation Fallback**:
  - Re-instantiates `PlcRuntime::from_project(&self.project)` on warm reset and restores retain values.
  - *Trade-off*: Recompilation re-allocates memory and risks discarding active manual forces.

---

## Detailed Implementation Specifics

### 1. `SimSpeed` Display & Dropdown (`ferrite-gui`)
- Implement `std::fmt::Display` for `SimSpeed` returning `self.label()`.
- Define `SimSpeed::ALL` containing `[Slow0_1x, Realtime1x, Fast2x, Fast5x, Fast10x, Max]`.
- Replace inline button loop with:
  ```rust
  pick_list(
      &SimSpeed::ALL[..],
      Some(self.sim_speed),
      Message::SetSimSpeed,
  )
  .style(StudioTheme::styled_pick_list)
  .width(72)
  ```

### 2. Scrollable Header Bar (`ferrite-gui`)
- In `view_header(&self)`:
  Wrap the main row of controls in `scrollable(header_row).direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::default())).width(Length::Fill)`.

### 3. Header Menu Labels (`ferrite-gui`)
- Change button labels:
  - `self.ui_lang.tr("File ▾", "Fichier ▾")` -> `self.ui_lang.tr("File", "Fichier")`
  - `self.ui_lang.tr("PLC ▾", "API ▾")` -> `self.ui_lang.tr("PLC", "API")`
  - `self.ui_lang.tr("Panels ▾", "Panneaux ▾")` -> `self.ui_lang.tr("Panels", "Panneaux")`
  - `self.ui_lang.tr("Options ▾", "Options ▾")` -> `self.ui_lang.tr("Options", "Options")`

### 4. Warm Restart Core & Runtime (`ferrite-core` & `ferrite-runtime`)
- In `crates/ferrite-core/src/fb.rs` and `tag_table.rs`:
  Add `pub fn reset(&mut self)` to `FbInstance` and `pub fn reset_fbs(&mut self)` to `TagTable` to reset timer `et_ms`/`q` and counter `cv`/`q` to default initial states.
- In `crates/ferrite-runtime/src/lib.rs`:
  Update `PlcRuntime::warm_restart()`:
  1. Export retain tags.
  2. Clear process image, then re-initialize process image with initial values of all tags that have an address (preserving active forces).
  3. Reset non-retain tags to initial values, then re-import retain values.
  4. Latch inputs (`process_image.latch_inputs()`).
  5. Reset Function Blocks (`self.tags.reset_fbs()`).
  6. Reset tasks (`elapsed_ms = 0`, `execution_count = 0`, `last_trigger_state = false`) and POUs (`pou.reset()`).
  7. Reset `scan_count = 0`, `virtual_time_ms = 0`, `last_scan_duration_us = 0`.

### 5. Warm Restart UI Integration (`ferrite-gui`)
- In `Message::ResetSimWarm`:
  1. Call `rt.warm_restart()`.
  2. Set `self.sim_running = false;` (auto-pause).
  3. Sync `shared_io` and `modbus_shared_pi` with current `rt` tag and process image values.
  4. Clear `self.trend_plotter.clear_all()`.
