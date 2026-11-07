use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::Instant;

use ferrite_core::{
    AccessWidth, ActiveAlarm, AlarmCondition, AlarmDefinition, AlarmEventRecord, AlarmSeverity,
    CoreError, DirectAddress, ExecutablePou, IecType, IecValue, MemoryArea, PouId, ProcessImage,
    RuntimeError, ScanContext, TagId, TagTable,
};
use ferrite_project::{Project, ProjectError, SimScenario, TaskKind};

/// Preemptive & Multi-Task Scheduler runtime representation of a scheduled task.
pub struct ScheduledTask {
    pub name: String,
    pub kind: TaskKind,
    pub priority: u32,
    pub interval_ms: u64,
    pub trigger_symbol: Option<String>,
    pub pous: Vec<Box<dyn ExecutablePou>>,
    pub elapsed_ms: u64,
    pub last_trigger_state: bool,
    pub execution_count: u64,
    /// Maximum execution time in microseconds allocated to this task per scan cycle.
    /// Initialized by default to the runtime's global `watchdog_us`.
    /// A value of 0 disables per-task deadline enforcement.
    pub watchdog_us: u64,
    /// Set to `true` if the task's execution time exceeded its configured `watchdog_us` deadline.
    /// Cleared on cold restart and warm restart.
    pub task_faulted: bool,
    /// Configured POU IDs assigned to this task (matching `TaskConfig.pou_order`).
    pub pou_ids: Vec<PouId>,
}

impl ScheduledTask {
    pub fn new(
        name: impl Into<String>,
        kind: TaskKind,
        priority: u32,
        interval_ms: u64,
        trigger_symbol: Option<String>,
        pous: Vec<Box<dyn ExecutablePou>>,
        watchdog_us: u64,
    ) -> Self {
        let pou_ids = pous.iter().map(|p| p.id()).collect();
        Self {
            name: name.into(),
            kind,
            priority,
            interval_ms,
            trigger_symbol,
            pous,
            elapsed_ms: 0,
            last_trigger_state: false,
            execution_count: 0,
            watchdog_us,
            task_faulted: false,
            pou_ids,
        }
    }
}

/// Single time-stamped synchronous trace record of configured tag values sampled during Phase 4.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSample {
    pub scan_count: u64,
    pub virtual_time_ms: u64,
    pub values: Vec<(TagId, IecValue)>,
}

impl TraceSample {
    /// Returns the raw `IecValue` for a traced tag if present.
    pub fn get_value(&self, id: TagId) -> Option<&IecValue> {
        self.values.iter().find(|(k, _)| *k == id).map(|(_, v)| v)
    }

    /// Converts the traced tag value to `f64` for numerical trend plotting or telemetry.
    pub fn get_f64(&self, id: TagId) -> Option<f64> {
        self.get_value(id).and_then(|v| match v {
            IecValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            IecValue::Int(i) => Some(f64::from(*i)),
            IecValue::Dint(d) => Some(f64::from(*d)),
            IecValue::Real(r) => Some(f64::from(*r)),
            IecValue::Time(t) => Some(*t as f64),
            IecValue::String(_) => None,
            _ => v.as_lreal().ok(),
        })
    }
}

/// Bounded FIFO ring buffer for synchronous in-scan tag value tracing.
/// Populated in Phase 4 (Diagnostics) to guarantee zero heap allocations in Phase 2.
#[derive(Debug, Clone, Default)]
pub struct TraceBuffer {
    pub traced_tags: Vec<TagId>,
    pub capacity: usize,
    pub samples: VecDeque<TraceSample>,
}

impl TraceBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            traced_tags: Vec::new(),
            capacity,
            samples: VecDeque::with_capacity(capacity),
        }
    }

    pub fn configure(&mut self, traced: &[TagId], capacity: usize) {
        self.traced_tags = traced.to_vec();
        self.capacity = capacity;
        self.samples.clear();
        if capacity > 0 {
            self.samples.reserve(capacity);
        }
    }

    pub fn push(&mut self, sample: TraceSample) {
        if self.capacity == 0 {
            return;
        }
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub fn drain(&mut self) -> Vec<TraceSample> {
        self.samples.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Diagnostic report returned after state-preserving online POU reloading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReloadReport {
    /// POUs whose executable definitions were updated in matching task slots.
    pub updated_pous: Vec<PouId>,
    /// Newly added POUs registered into task slots.
    pub added_pous: Vec<PouId>,
    /// POUs previously in tasks but omitted from the reload set, whose task slots were emptied.
    pub dropped_pous: Vec<PouId>,
    /// POUs provided in the reload set that could not be matched to any task.
    pub unmatched_pous: Vec<PouId>,
}

impl ReloadReport {
    pub fn is_success(&self) -> bool {
        self.unmatched_pous.is_empty()
    }
}
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("Runtime error during scenario '{scenario}' at scan {scan}: {source}")]
    Runtime {
        scenario: String,
        scan: u64,
        #[source]
        source: RuntimeError,
    },

    #[error(
        "Assertion failed in scenario '{scenario}' at scan {scan} for symbol '{symbol}': expected {expected}, got {actual}"
    )]
    AssertionFailed {
        scenario: String,
        scan: u64,
        symbol: String,
        expected: IecValue,
        actual: IecValue,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Breakpoint {
    ScanCount(u64),
    Pou(PouId),
    SymbolTrue(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanTelemetry {
    pub scan_count: u64,
    pub virtual_time_ms: u64,
    pub scan_duration_us: u64,
    pub max_scan_duration_us: u64,
    pub breakpoint_hit: bool,
    pub hit_breakpoint: Option<Breakpoint>,
    /// Task deadline violations occurring during this specific scan: `(task_name, scan, overrun_us)`.
    pub task_deadline_violations: Vec<(String, u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioReport {
    pub scenario_name: String,
    pub scans_executed: u64,
    pub assertions_verified: usize,
    pub max_scan_duration_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetainSnapshot {
    pub tags: HashMap<String, IecValue>,
    pub markers: Vec<u8>,
}

/// Deterministic PLC runtime executing the 4-phase scan cycle:
/// 1. Input Scan -> 2. Task & POU Execution -> 3. Output Scan -> 4. Diagnostics & Watchdog.
pub struct PlcRuntime {
    pub process_image: ProcessImage,
    pub tags: TagTable,
    pub tasks: Vec<ScheduledTask>,
    pub scan_cycle_ms: u64,
    pub watchdog_us: u64,
    /// When `false` (default IEC 61131-3 warning/diagnostic recording mode), task deadline overruns set
    /// `task.task_faulted = true` and record violations in telemetry without halting the PLC.
    /// When `true` (strict hard real-time fail-stop mode), a task deadline overrun immediately
    /// halts the runtime with `RuntimeError::WatchdogTimeout`.
    pub strict_deadline_mode: bool,
    /// Cumulative record of task deadline violations: `(task_name, scan, overrun_us)`.
    pub task_deadline_violations: Vec<(String, u64, u64)>,
    /// High-frequency synchronous in-scan trace buffer (sampled in Phase 4).
    pub trace_buffer: TraceBuffer,
    pub scan_count: u64,
    pub virtual_time_ms: u64,
    pub last_scan_duration_us: u64,
    pub max_scan_duration_us: u64,
    pub breakpoints: Vec<Breakpoint>,
    pub watch_list: Vec<String>,
    input_tag_bindings: Vec<(TagId, DirectAddress, IecType)>,
    pub alarms: Vec<ActiveAlarm>,
    pub alarm_journal: VecDeque<AlarmEventRecord>,
}

impl PlcRuntime {
    pub fn from_project(project: &Project) -> Result<Self, ProjectError> {
        let (tags, compiled_tasks) = project.compile_tasks()?;
        let tasks = compiled_tasks
            .into_iter()
            .map(|t| {
                let pou_ids = t.pous.iter().map(|p| p.id()).collect();
                ScheduledTask {
                    name: t.name,
                    kind: t.kind,
                    priority: t.priority,
                    interval_ms: t.interval_ms,
                    trigger_symbol: t.trigger_symbol,
                    pous: t.pous,
                    elapsed_ms: 0,
                    last_trigger_state: false,
                    execution_count: 0,
                    watchdog_us: project.manifest.watchdog_us,
                    task_faulted: false,
                    pou_ids,
                }
            })
            .collect();
        let mut process_image = ProcessImage::default();

        // Initialize process image with tag initial values so initial inputs like EStop_OK = TRUE are latched
        let mut input_tag_bindings = Vec::new();
        for (idx, tag) in tags.tags().iter().enumerate() {
            if let Some(addr) = tag.address {
                if addr.area == MemoryArea::Input {
                    process_image.set_hw_input(addr, &tag.initial_value)?;
                    input_tag_bindings.push((TagId(idx), addr, tag.data_type));
                } else {
                    process_image.write(addr, &tag.initial_value)?;
                }
            }
        }
        process_image.latch_inputs();

        let watch_list = tags.tags().iter().map(|t| t.name.clone()).collect();

        // Configure default ISA-18.2 alarms based on standard project tags
        let mut alarms = Vec::new();
        if tags.get_tag("Tank_Temp").is_some() {
            alarms.push(ActiveAlarm::new(AlarmDefinition {
                id: "ALM_HIGH_TEMP".to_string(),
                tag: "Tank_Temp".to_string(),
                condition: AlarmCondition::High { limit: 70.0 },
                severity: AlarmSeverity::Critical,
                message: "Process tank temperature exceeded critical threshold (70.0°C)"
                    .to_string(),
            }));
        }
        if tags.get_tag("Overpressure_Fault").is_some() {
            alarms.push(ActiveAlarm::new(AlarmDefinition {
                id: "ALM_OVERPRESSURE".to_string(),
                tag: "Overpressure_Fault".to_string(),
                condition: AlarmCondition::Digital {
                    trigger_value: true,
                },
                severity: AlarmSeverity::Major,
                message: "Vessel overpressure safety fault detected".to_string(),
            }));
        }
        if tags.get_tag("High_Temp_Alarm").is_some() {
            alarms.push(ActiveAlarm::new(AlarmDefinition {
                id: "ALM_TEMP_WARNING".to_string(),
                tag: "High_Temp_Alarm".to_string(),
                condition: AlarmCondition::Digital {
                    trigger_value: true,
                },
                severity: AlarmSeverity::Warning,
                message: "High temperature warning active".to_string(),
            }));
        }

        Ok(Self {
            process_image,
            tags,
            tasks,
            scan_cycle_ms: project.manifest.scan_cycle_ms,
            watchdog_us: project.manifest.watchdog_us,
            strict_deadline_mode: false,
            task_deadline_violations: Vec::new(),
            trace_buffer: TraceBuffer::default(),
            scan_count: 0,
            virtual_time_ms: 0,
            last_scan_duration_us: 0,
            max_scan_duration_us: 0,
            breakpoints: Vec::new(),
            watch_list,
            input_tag_bindings,
            alarms,
            alarm_journal: VecDeque::new(),
        })
    }

    pub fn tasks(&self) -> &[ScheduledTask] {
        &self.tasks
    }

    pub fn pous(&self) -> Vec<&dyn ExecutablePou> {
        self.tasks
            .iter()
            .flat_map(|t| t.pous.iter().map(|p| p.as_ref()))
            .collect()
    }

    /// Sets a simulated input or tag value prior to the next scan cycle.
    pub fn set_stimulus(
        &mut self,
        symbol_or_addr: &str,
        val: IecValue,
    ) -> Result<(), RuntimeError> {
        let trimmed = symbol_or_addr.trim();
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            if addr.area == MemoryArea::Input {
                self.process_image.set_hw_input(addr, &val)?;
            } else {
                self.process_image.write(addr, &val)?;
            }
            return Ok(());
        }
        if let Some(id) = self.tags.tag_id(trimmed) {
            let (read_only, addr_opt, data_type) = match self.tags.get_tag_by_id(id) {
                Some(t) => (t.read_only_outside_sfc, t.address, t.data_type),
                None => return Err(CoreError::UnknownSymbol(trimmed.to_string()).into()),
            };
            if read_only {
                return Err(CoreError::ReadOnlySymbol(trimmed.to_string()).into());
            }
            let coerced = val.coerce_to(data_type)?;
            if let Some(addr) = addr_opt {
                if addr.area == MemoryArea::Input {
                    self.process_image.set_hw_input(addr, &coerced)?;
                } else {
                    self.process_image.write(addr, &coerced)?;
                }
            }
            if let Some(tag) = self.tags.get_tag_mut_by_id(id) {
                tag.value = Some(coerced);
            }
            return Ok(());
        }
        Err(CoreError::UnknownSymbol(trimmed.to_string()).into())
    }

    /// Sets a tag value, honoring SFC read-only protection and propagating to ProcessImage.
    pub fn set_tag_value(&mut self, name: &str, val: IecValue) -> Result<(), RuntimeError> {
        self.set_stimulus(name, val)
    }

    /// Forces a symbol or direct address to a fixed value.
    pub fn force(&mut self, symbol_or_addr: &str, val: IecValue) -> Result<(), RuntimeError> {
        let trimmed = symbol_or_addr.trim();
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            self.process_image.force(addr, &val)?;
            return Ok(());
        }
        if let Some(tag) = self.tags.get_tag(trimmed)
            && let Some(addr) = tag.address
        {
            self.process_image.force(addr, &val)?;
        }
        self.tags.force_tag(trimmed, val)?;
        Ok(())
    }

    /// Removes an active force on a symbol or direct address.
    pub fn unforce(&mut self, symbol_or_addr: &str) -> Result<(), RuntimeError> {
        let trimmed = symbol_or_addr.trim();
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            self.process_image.unforce(addr)?;
            return Ok(());
        }
        if let Some(tag) = self.tags.get_tag(trimmed)
            && let Some(addr) = tag.address
        {
            self.process_image.unforce(addr)?;
        }
        self.tags.unforce_tag(trimmed)?;
        Ok(())
    }

    /// Reads the current value of any symbol, direct address, or FB pin.
    pub fn read_value(&self, symbol: &str) -> Result<IecValue, RuntimeError> {
        let trimmed = symbol.trim();
        if let Some(id) = self.tags.tag_id(trimmed)
            && let Some(tag) = self.tags.get_tag_by_id(id)
        {
            if tag.forced.is_some() {
                return Ok(tag.current_value().clone());
            }
            if let Some(addr) = tag.address {
                return Ok(self.process_image.read(addr, tag.data_type)?);
            }
            return Ok(tag.current_value().clone());
        }
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            let ty = match addr.width {
                AccessWidth::Bit => IecType::Bool,
                AccessWidth::Byte | AccessWidth::Word => IecType::Int,
                AccessWidth::DWord => IecType::Dint,
                AccessWidth::LWord => IecType::Lword,
            };
            return Ok(self.process_image.read(addr, ty)?);
        }
        if let Some((inst, pin)) = trimmed.split_once('.')
            && let Some(fb) = self.tags.get_fb(inst)
        {
            return Ok(fb.read_pin(pin)?);
        }
        Err(CoreError::UnknownSymbol(trimmed.to_string()).into())
    }

    /// Executes one complete deterministic 4-phase PLC scan cycle.
    /// Returns the total count of forced tags and addresses.
    pub fn active_forces_count(&self) -> usize {
        self.tags.active_forces_count() + self.process_image.active_forces_count()
    }

    /// Clears all active forces in TagTable and ProcessImage.
    pub fn unforce_all(&mut self) -> Result<(), RuntimeError> {
        self.tags.unforce_all();
        self.process_image.unforce_all();
        Ok(())
    }

    /// Performs an IEC 61131-3 Cold Restart: wipes memory, resets all tags to defaults,
    /// clears all active forces, resets alarms and tasks, and returns POUs to initial state.
    pub fn cold_restart(&mut self) -> Result<(), RuntimeError> {
        self.unforce_all()?;
        self.process_image.clear();
        self.tags.reset_values_to_initial();

        // Re-initialize all direct addresses with initial values and latch inputs
        for tag in self.tags.tags() {
            if let Some(addr) = tag.address {
                if addr.area == MemoryArea::Input {
                    self.process_image.set_hw_input(addr, &tag.initial_value)?;
                } else {
                    self.process_image.write(addr, &tag.initial_value)?;
                }
            }
        }
        self.process_image.latch_inputs();
        self.tags.reset_fbs();

        self.alarms.clear();
        self.alarm_journal.clear();
        self.task_deadline_violations.clear();
        self.trace_buffer.samples.clear();
        self.scan_count = 0;
        self.virtual_time_ms = 0;
        self.last_scan_duration_us = 0;
        self.max_scan_duration_us = 0;
        for task in &mut self.tasks {
            task.elapsed_ms = 0;
            task.last_trigger_state = false;
            task.execution_count = 0;
            task.task_faulted = false;
            for pou in &mut task.pous {
                pou.reset();
            }
        }
        Ok(())
    }

    /// Performs an IEC 61131-3 Warm Restart: preserves RETAIN tags, %M markers, and active forces,
    /// resets non-retain tags to defaults, and returns POUs to initial state.
    pub fn warm_restart(&mut self) -> Result<(), RuntimeError> {
        let retain_tags = self.tags.export_retain_values();
        let markers = self.process_image.export_markers();
        self.process_image.clear();
        self.process_image.import_markers(&markers);
        self.tags.reset_values_to_initial();
        self.tags.import_retain_values(&retain_tags);

        // Re-initialize direct address bindings:
        // - Retained tags write their preserved value to process_image if mapped (inputs, outputs, or markers)
        // - Non-retain %I inputs are re-initialized with tag initial values and re-latched
        // - Non-retain %Q outputs are reset to initial values
        // - %M markers are already restored via import_markers
        for tag in self.tags.tags() {
            if let Some(addr) = tag.address {
                if tag.retain {
                    if addr.area == MemoryArea::Input {
                        self.process_image.set_hw_input(addr, tag.current_value())?;
                    } else {
                        self.process_image.write(addr, tag.current_value())?;
                    }
                } else if addr.area == MemoryArea::Input {
                    self.process_image.set_hw_input(addr, &tag.initial_value)?;
                } else if addr.area == MemoryArea::Output {
                    self.process_image.write(addr, &tag.initial_value)?;
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
            task.task_faulted = false;
            for pou in &mut task.pous {
                pou.reset();
            }
        }
        Ok(())
    }

    /// Evaluates an ST expression to a boolean in pure read-only mode using the runtime's symbol table.
    pub fn evaluate_expression_bool(&self, expr_str: &str, pou_name: &str) -> Option<bool> {
        let trimmed = expr_str.trim();
        if trimmed.is_empty() || trimmed == "1" || trimmed.eq_ignore_ascii_case("true") {
            return Some(true);
        }
        if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
            return Some(false);
        }
        let expr = ferrite_lang_st::parse_st_expression(trimmed, pou_name).ok()?;
        let val = expr
            .evaluate_with_resolver(
                &|sym| {
                    if let Ok(v) = self.read_value(sym) {
                        Ok(v)
                    } else {
                        let scoped = format!("{pou_name}.{sym}");
                        self.read_value(&scoped)
                    }
                },
                pou_name,
            )
            .ok()?;
        val.as_bool().ok()
    }

    pub fn step_scan(&mut self) -> Result<ScanTelemetry, RuntimeError> {
        let start = Instant::now();
        self.scan_count = self.scan_count.saturating_add(1);
        self.virtual_time_ms = self.virtual_time_ms.saturating_add(self.scan_cycle_ms);

        // Phase 1: Input Scan (Latch simulated hardware inputs into ProcessImage input buffer)
        self.process_image.latch_inputs();

        // Synchronize only input-mapped tags from ProcessImage without scanning all tags
        for &(id, addr, ty) in &self.input_tag_bindings {
            if let Some(t) = self.tags.get_tag_mut_by_id(id)
                && t.forced.is_none()
            {
                let val = self.process_image.read(addr, ty)?;
                t.value = Some(val);
            }
        }

        // Phase 2: Task & POU Execution
        let mut executed_pous = Vec::new();
        let mut scan_task_violations = Vec::new();
        {
            let dt_ms = self.scan_cycle_ms;
            let scan_count = self.scan_count;
            for task in &mut self.tasks {
                let should_run = match task.kind {
                    TaskKind::Continuous => true,
                    TaskKind::Cyclic => {
                        task.elapsed_ms = task.elapsed_ms.saturating_add(dt_ms);
                        let interval = task.interval_ms.max(1);
                        if task.elapsed_ms >= interval {
                            task.elapsed_ms = task.elapsed_ms.saturating_sub(interval);
                            true
                        } else {
                            false
                        }
                    }
                    TaskKind::Event => {
                        if let Some(ref sym) = task.trigger_symbol {
                            let curr = self
                                .tags
                                .get_tag_value(sym)
                                .and_then(|v| v.as_bool().ok())
                                .unwrap_or(false);
                            let rising_edge = !task.last_trigger_state && curr;
                            task.last_trigger_state = curr;
                            rising_edge
                        } else {
                            false
                        }
                    }
                };

                if should_run {
                    task.execution_count = task.execution_count.saturating_add(1);
                    let task_start = Instant::now();
                    let mut ctx = ScanContext::new(
                        &mut self.process_image,
                        &mut self.tags,
                        dt_ms,
                        scan_count,
                    );
                    for pou in &mut task.pous {
                        pou.execute(&mut ctx)?;
                        executed_pous.push(pou.id());
                    }
                    let task_elapsed_us = task_start.elapsed().as_micros() as u64;
                    if task.watchdog_us > 0 && task_elapsed_us > task.watchdog_us {
                        let overrun_us = task_elapsed_us.saturating_sub(task.watchdog_us);
                        task.task_faulted = true;
                        let violation = (task.name.clone(), scan_count, overrun_us);
                        scan_task_violations.push(violation.clone());
                        self.task_deadline_violations.push(violation);

                        if self.strict_deadline_mode {
                            return Err(RuntimeError::WatchdogTimeout {
                                elapsed_us: task_elapsed_us,
                                limit_us: task.watchdog_us,
                            });
                        }
                    }
                }
            }
        }

        // Phase 3: Output Scan (Commit ProcessImage output buffer to hardware I/O bus)
        self.process_image.commit_outputs();

        // Phase 4: Diagnostics & Watchdog
        let elapsed_us = start.elapsed().as_micros() as u64;
        self.last_scan_duration_us = elapsed_us;
        if elapsed_us > self.max_scan_duration_us {
            self.max_scan_duration_us = elapsed_us;
        }

        if self.watchdog_us > 0 && elapsed_us > self.watchdog_us {
            return Err(RuntimeError::WatchdogTimeout {
                elapsed_us,
                limit_us: self.watchdog_us,
            });
        }

        // High-frequency synchronous in-scan trace buffer sampling (O(traced_tags) complexity)
        if self.trace_buffer.capacity > 0 && !self.trace_buffer.traced_tags.is_empty() {
            let mut values = Vec::with_capacity(self.trace_buffer.traced_tags.len());
            for &id in &self.trace_buffer.traced_tags {
                if let Some(tag) = self.tags.get_tag_by_id(id) {
                    let val = if tag.forced.is_some() {
                        tag.current_value().clone()
                    } else if let Some(addr) = tag.address {
                        self.process_image
                            .read(addr, tag.data_type)
                            .unwrap_or_else(|_| tag.current_value().clone())
                    } else {
                        tag.current_value().clone()
                    };
                    values.push((id, val));
                }
            }
            self.trace_buffer.push(TraceSample {
                scan_count: self.scan_count,
                virtual_time_ms: self.virtual_time_ms,
                values,
            });
        }

        // Evaluate ISA-18.2 Alarms
        self.evaluate_alarms();

        let mut hit_breakpoint = None;
        for bp in &self.breakpoints {
            match bp {
                Breakpoint::ScanCount(target) if *target == self.scan_count => {
                    hit_breakpoint = Some(bp.clone());
                    break;
                }
                Breakpoint::SymbolTrue(sym) => {
                    if let Ok(IecValue::Bool(true)) = self.read_value(sym) {
                        hit_breakpoint = Some(bp.clone());
                        break;
                    }
                }
                Breakpoint::Pou(id) if executed_pous.contains(id) => {
                    hit_breakpoint = Some(bp.clone());
                    break;
                }
                _ => {}
            }
        }
        let breakpoint_hit = hit_breakpoint.is_some();

        Ok(ScanTelemetry {
            scan_count: self.scan_count,
            virtual_time_ms: self.virtual_time_ms,
            scan_duration_us: elapsed_us,
            max_scan_duration_us: self.max_scan_duration_us,
            breakpoint_hit,
            hit_breakpoint,
            task_deadline_violations: scan_task_violations,
        })
    }

    /// Executes up to `batch_size` sequential scan cycles.
    /// Halts early and returns immediately if any breakpoint is hit or on runtime error.
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

    /// Executes an automated headless test scenario from `sim/scenarios.toml`.
    pub fn run_scenario(
        &mut self,
        scenario: &SimScenario,
    ) -> Result<ScenarioReport, ScenarioError> {
        let mut assertions_verified = 0usize;

        for scan_idx in 1..=scenario.total_scans {
            for step in &scenario.steps {
                if step.scan == scan_idx {
                    for (sym, val) in &step.set {
                        self.set_stimulus(sym, val.clone()).map_err(|source| {
                            ScenarioError::Runtime {
                                scenario: scenario.name.clone(),
                                scan: scan_idx,
                                source,
                            }
                        })?;
                    }
                }
            }

            self.step_scan().map_err(|source| ScenarioError::Runtime {
                scenario: scenario.name.clone(),
                scan: scan_idx,
                source,
            })?;

            for step in &scenario.steps {
                if step.scan == scan_idx {
                    for (sym, expected) in &step.expect {
                        let actual =
                            self.read_value(sym)
                                .map_err(|source| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source,
                                })?;
                        let coerced_expected = expected
                            .coerce_to(actual.iec_type())
                            .unwrap_or_else(|_| expected.clone());
                        if actual != coerced_expected {
                            return Err(ScenarioError::AssertionFailed {
                                scenario: scenario.name.clone(),
                                scan: scan_idx,
                                symbol: sym.clone(),
                                expected: coerced_expected,
                                actual,
                            });
                        }
                        assertions_verified += 1;
                    }

                    // Evaluate explicit ScenarioAssert assertions (FB instances, range gates, operators)
                    for assert in &step.asserts {
                        let (target_sym, expected_val) = if let Some((fb, field)) = assert.target_fb_and_field() {
                            let sym = format!("{fb}.{field}");
                            (sym, assert.value.clone())
                        } else if let Some(ref step_name) = assert.sfc_step {
                            let sym = format!("{step_name}.state");
                            (sym, assert.value.clone().or(Some(IecValue::Bool(true))))
                        } else if !assert.fb.is_empty() {
                            (assert.fb.clone(), assert.value.clone())
                        } else {
                            continue;
                        };

                        let actual = self.read_value(&target_sym).map_err(|source| ScenarioError::Runtime {
                            scenario: scenario.name.clone(),
                            scan: scan_idx,
                            source,
                        })?;

                        match assert.op {
                            ferrite_project::AssertOp::Equal => {
                                let expected = expected_val.ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "Assertion requires expected value".to_string(),
                                    },
                                })?;
                                let coerced = expected.coerce_to(actual.iec_type()).unwrap_or(expected.clone());
                                if actual != coerced {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: coerced,
                                        actual,
                                    });
                                }
                            }
                            ferrite_project::AssertOp::NotEqual => {
                                let expected = expected_val.ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "Assertion requires expected value".to_string(),
                                    },
                                })?;
                                let coerced = expected.coerce_to(actual.iec_type()).unwrap_or(expected.clone());
                                if actual == coerced {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: IecValue::String(format!("!= {coerced}")),
                                        actual,
                                    });
                                }
                            }
                            ferrite_project::AssertOp::LessThan => {
                                let expected = expected_val.ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "Assertion requires expected value".to_string(),
                                    },
                                })?;
                                let coerced = expected.coerce_to(actual.iec_type()).unwrap_or(expected.clone());
                                if actual.partial_cmp(&coerced) != Some(std::cmp::Ordering::Less) {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: IecValue::String(format!("< {coerced}")),
                                        actual,
                                    });
                                }
                            }
                            ferrite_project::AssertOp::LessEqual => {
                                let expected = expected_val.ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "Assertion requires expected value".to_string(),
                                    },
                                })?;
                                let coerced = expected.coerce_to(actual.iec_type()).unwrap_or(expected.clone());
                                if !matches!(actual.partial_cmp(&coerced), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)) {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: IecValue::String(format!("<= {coerced}")),
                                        actual,
                                    });
                                }
                            }
                            ferrite_project::AssertOp::GreaterThan => {
                                let expected = expected_val.ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "Assertion requires expected value".to_string(),
                                    },
                                })?;
                                let coerced = expected.coerce_to(actual.iec_type()).unwrap_or(expected.clone());
                                if actual.partial_cmp(&coerced) != Some(std::cmp::Ordering::Greater) {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: IecValue::String(format!("> {coerced}")),
                                        actual,
                                    });
                                }
                            }
                            ferrite_project::AssertOp::GreaterEqual => {
                                let expected = expected_val.ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "Assertion requires expected value".to_string(),
                                    },
                                })?;
                                let coerced = expected.coerce_to(actual.iec_type()).unwrap_or(expected.clone());
                                if !matches!(actual.partial_cmp(&coerced), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)) {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: IecValue::String(format!(">= {coerced}")),
                                        actual,
                                    });
                                }
                            }
                            ferrite_project::AssertOp::InRange => {
                                let min = assert.min.as_ref().ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "InRange requires min".to_string(),
                                    },
                                })?;
                                let max = assert.max.as_ref().ok_or_else(|| ScenarioError::Runtime {
                                    scenario: scenario.name.clone(),
                                    scan: scan_idx,
                                    source: RuntimeError::ExecutionError {
                                        pou: scenario.name.clone(),
                                        message: "InRange requires max".to_string(),
                                    },
                                })?;
                                let coerced_min = min.coerce_to(actual.iec_type()).unwrap_or(min.clone());
                                let coerced_max = max.coerce_to(actual.iec_type()).unwrap_or(max.clone());
                                let in_min = matches!(actual.partial_cmp(&coerced_min), Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal));
                                let in_max = matches!(actual.partial_cmp(&coerced_max), Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal));
                                if !(in_min && in_max) {
                                    return Err(ScenarioError::AssertionFailed {
                                        scenario: scenario.name.clone(),
                                        scan: scan_idx,
                                        symbol: target_sym,
                                        expected: IecValue::String(format!("in [{coerced_min}, {coerced_max}]")),
                                        actual,
                                    });
                                }
                            }
                        }
                        assertions_verified += 1;
                    }
                }
            }
        }

        Ok(ScenarioReport {
            scenario_name: scenario.name.clone(),
            scans_executed: scenario.total_scans,
            assertions_verified,
            max_scan_duration_us: self.max_scan_duration_us,
        })
    }

    /// Registers a new ISA-18.2 alarm definition into the runtime.
    pub fn register_alarm_definition(&mut self, def: AlarmDefinition) {
        self.alarms.push(ActiveAlarm::new(def));
    }

    /// Returns a slice of all currently active (unacknowledged or acknowledged) alarms.
    pub fn active_alarms(&self) -> Vec<&ActiveAlarm> {
        self.alarms.iter().filter(|a| a.state.is_active()).collect()
    }

    /// Returns a reference to the alarm event journal.
    pub fn alarm_journal(&self) -> &VecDeque<AlarmEventRecord> {
        &self.alarm_journal
    }

    /// Evaluates all configured ISA-18.2 alarms against current tag and memory values.
    pub fn evaluate_alarms(&mut self) {
        let time_ms = self.virtual_time_ms;
        for alarm in &mut self.alarms {
            let tag_name = &alarm.definition.tag;
            let val = if let Some(t) = self.tags.get_tag(tag_name) {
                if let Some(addr) = t.address {
                    self.process_image
                        .read(addr, t.data_type)
                        .unwrap_or_else(|_| t.current_value().clone())
                } else {
                    t.current_value().clone()
                }
            } else {
                continue;
            };

            let changed = alarm.evaluate(&val, time_ms);
            if changed {
                self.alarm_journal.push_back(AlarmEventRecord {
                    timestamp_ms: time_ms,
                    alarm_id: alarm.definition.id.clone(),
                    state: alarm.state,
                    severity: alarm.definition.severity,
                    message: alarm.definition.message.clone(),
                });
                if self.alarm_journal.len() > 1000 {
                    self.alarm_journal.pop_front();
                }
            }
        }
    }

    /// Acknowledges a specific active alarm by ID (ISA-18.2 operator acknowledgement).
    pub fn acknowledge_alarm(&mut self, alarm_id: &str) {
        let time_ms = self.virtual_time_ms;
        for alarm in &mut self.alarms {
            if alarm.definition.id == alarm_id {
                let old_state = alarm.state;
                alarm.acknowledge(time_ms);
                if old_state != alarm.state {
                    self.alarm_journal.push_back(AlarmEventRecord {
                        timestamp_ms: time_ms,
                        alarm_id: alarm.definition.id.clone(),
                        state: alarm.state,
                        severity: alarm.definition.severity,
                        message: format!("ACK: {}", alarm.definition.message),
                    });
                    if self.alarm_journal.len() > 1000 {
                        self.alarm_journal.pop_front();
                    }
                }
            }
        }
    }

    /// Acknowledges all active alarms across the system.
    pub fn acknowledge_all_alarms(&mut self) {
        let time_ms = self.virtual_time_ms;
        for alarm in &mut self.alarms {
            let old_state = alarm.state;
            alarm.acknowledge(time_ms);
            if old_state != alarm.state {
                self.alarm_journal.push_back(AlarmEventRecord {
                    timestamp_ms: time_ms,
                    alarm_id: alarm.definition.id.clone(),
                    state: alarm.state,
                    severity: alarm.definition.severity,
                    message: format!("ACK: {}", alarm.definition.message),
                });
                if self.alarm_journal.len() > 1000 {
                    self.alarm_journal.pop_front();
                }
            }
        }
    }

    /// Serializes retentive tags and %M marker memory to disk atomically.
    pub fn save_retain_snapshot(&self, path: &Path) -> std::io::Result<()> {
        let retain_tags = self.tags.export_retain_values();
        let markers = self.process_image.export_markers();
        let payload = RetainSnapshot {
            tags: retain_tags,
            markers,
        };
        let bytes = serde_json::to_vec_pretty(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let bak_path = path.with_extension("bak");

        // Backup existing valid snapshot if present
        if path.exists() {
            let _ = std::fs::copy(path, &bak_path);
        }

        let tmp_file_name = format!(
            ".{}.tmp.{}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("retain"),
            std::process::id()
        );
        let tmp_path = parent.join(tmp_file_name);

        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)?;
            use std::io::Write;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }

        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp_path, path)?;

        Ok(())
    }

    /// Restores retentive tags and %M marker memory from disk, falling back to `.bak` if corrupted.
    pub fn load_retain_snapshot(&mut self, path: &Path) -> std::io::Result<()> {
        let bak_path = path.with_extension("bak");

        let load_from = |p: &Path| -> std::io::Result<RetainSnapshot> {
            let bytes = std::fs::read(p)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        };

        let snapshot = if path.exists() {
            match load_from(path) {
                Ok(s) => s,
                Err(orig_err) => {
                    if bak_path.exists() {
                        load_from(&bak_path)?
                    } else {
                        return Err(orig_err);
                    }
                }
            }
        } else if bak_path.exists() {
            load_from(&bak_path)?
        } else {
            return Ok(());
        };

        self.tags.import_retain_values(&snapshot.tags);
        self.process_image.import_markers(&snapshot.markers);
        Ok(())
    }

    /// Configures high-frequency synchronous in-scan tracing for a subset of tags.
    /// Sampling occurs deterministically during Phase 4 (Diagnostics) with O(traced_tags) complexity.
    pub fn configure_trace(&mut self, traced: &[TagId], capacity: usize) {
        self.trace_buffer.configure(traced, capacity);
    }

    /// Returns the configured capacity of the in-scan trace ring buffer.
    pub fn trace_capacity(&self) -> usize {
        self.trace_buffer.capacity()
    }

    /// Drains and returns all captured trace samples from the ring buffer.
    pub fn drain_trace(&mut self) -> Vec<TraceSample> {
        self.trace_buffer.drain()
    }

    /// Performs an online state-preserving POU reload (IEC 61131-3 dynamic online change).
    ///
    /// Preserves:
    /// - `ProcessImage` (all inputs, outputs, %M markers, forced direct addresses)
    /// - `TagTable` (all tag values, forced tags, and SFC step states like `X<i>.state`)
    /// - `scan_count` and `virtual_time_ms`
    /// - Alarm journal and active ISA-18.2 alarms
    /// - Configured breakpoints and trace buffer
    /// - Scheduled task configurations (cycle times, priorities, triggers)
    ///
    /// Swapping & Semantics:
    /// - Tasks keep their configuration by matching `PouId`.
    /// - POUs present in `new_pous` replace the corresponding POU in matching task slots.
    /// - Newly replaced POUs have their internal state reset via `pou.reset()` (POU-internal
    ///   FB instances, e.g. local timers/counters, are re-initialized).
    /// - POUs not present in `new_pous` are cleanly dropped, emptying their task slots.
    /// - Returns a `ReloadReport` detailing updated, added, dropped, and unmatched POUs.
    pub fn reload_pous_preserving_state<I>(&mut self, new_pous: I) -> ReloadReport
    where
        I: IntoIterator<Item = Box<dyn ExecutablePou>>,
    {
        let mut incoming: HashMap<PouId, Box<dyn ExecutablePou>> = new_pous
            .into_iter()
            .map(|p| (p.id(), p))
            .collect();

        let mut updated_pous = Vec::new();
        let mut added_pous = Vec::new();
        let mut dropped_pous = Vec::new();

        for task in &mut self.tasks {
            if task.pou_ids.is_empty() && !task.pous.is_empty() {
                task.pou_ids = task.pous.iter().map(|p| p.id()).collect();
            }

            let old_pous = std::mem::take(&mut task.pous);
            let mut new_task_pous = Vec::new();

            for pou_id in &task.pou_ids {
                if let Some(mut new_pou) = incoming.remove(pou_id) {
                    new_pou.reset();
                    new_task_pous.push(new_pou);
                    if old_pous.iter().any(|p| p.id() == *pou_id) {
                        updated_pous.push(pou_id.clone());
                    } else {
                        added_pous.push(pou_id.clone());
                    }
                } else if old_pous.iter().any(|p| p.id() == *pou_id) {
                    dropped_pous.push(pou_id.clone());
                }
            }

            // Also check if any old POU was running that wasn't in pou_ids
            for old_pou in old_pous {
                let id = old_pou.id();
                if !task.pou_ids.contains(&id) {
                    if let Some(mut new_pou) = incoming.remove(&id) {
                        new_pou.reset();
                        new_task_pous.push(new_pou);
                        updated_pous.push(id.clone());
                        task.pou_ids.push(id);
                    } else {
                        dropped_pous.push(id);
                    }
                }
            }

            task.pous = new_task_pous;
        }

        // If there's exactly one task and remaining unmatched incoming POUs, attach them to that task
        if self.tasks.len() == 1 && !incoming.is_empty() {
            let task = &mut self.tasks[0];
            let remaining_keys: Vec<PouId> = incoming.keys().cloned().collect();
            for id in remaining_keys {
                if let Some(mut new_pou) = incoming.remove(&id) {
                    new_pou.reset();
                    task.pou_ids.push(id.clone());
                    task.pous.push(new_pou);
                    added_pous.push(id);
                }
            }
        }

        let unmatched_pous: Vec<PouId> = incoming.into_keys().collect();

        ReloadReport {
            updated_pous,
            added_pous,
            dropped_pous,
            unmatched_pous,
        }
    }

    /// Compiles all POUs from an updated `Project` and performs an online state-preserving reload.
    ///
    /// Preserves all live runtime state (`TagTable` values/forces, `ProcessImage`, `scan_count`,
    /// `virtual_time_ms`, alarms, breakpoints) while recompiling POUs and updating task mappings.
    pub fn reload_from_project_preserving_state(
        &mut self,
        project: &Project,
    ) -> Result<ReloadReport, ProjectError> {
        let (new_tags, compiled_tasks) = project.compile_tasks()?;

        // Synchronize any newly declared global tags without overwriting existing tag values or forces
        for tag in new_tags.tags() {
            if self.tags.get_tag(&tag.name).is_none() {
                let _ = self.tags.register_tag(tag.clone());
                if let Some(addr) = tag.address {
                    if addr.area == MemoryArea::Input {
                        let _ = self.process_image.set_hw_input(addr, &tag.initial_value);
                        if let Some(new_id) = self.tags.tag_id(&tag.name) {
                            self.input_tag_bindings.push((new_id, addr, tag.data_type));
                        }
                    } else {
                        let _ = self.process_image.write(addr, &tag.initial_value);
                    }
                }
            }
        }

        // Reconcile task definitions and collect all freshly compiled POUs
        let mut all_new_pous = Vec::new();
        for ct in compiled_tasks {
            if let Some(task) = self.tasks.iter_mut().find(|t| t.name == ct.name) {
                task.kind = ct.kind;
                task.priority = ct.priority;
                task.interval_ms = ct.interval_ms;
                task.trigger_symbol = ct.trigger_symbol;
                task.pou_ids = ct.pous.iter().map(|p| p.id()).collect();
            } else {
                let pou_ids = ct.pous.iter().map(|p| p.id()).collect();
                self.tasks.push(ScheduledTask {
                    name: ct.name,
                    kind: ct.kind,
                    priority: ct.priority,
                    interval_ms: ct.interval_ms,
                    trigger_symbol: ct.trigger_symbol,
                    pous: Vec::new(),
                    elapsed_ms: 0,
                    last_trigger_state: false,
                    execution_count: 0,
                    watchdog_us: self.watchdog_us,
                    task_faulted: false,
                    pou_ids,
                });
            }
            all_new_pous.extend(ct.pous);
        }

        Ok(self.reload_pous_preserving_state(all_new_pous))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::LanguageKind;
    use std::env;
    use std::fs;

    #[test]
    fn test_full_four_language_runtime_and_scenario() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_runtime_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let project = Project::scaffold(&temp_dir, "runtime_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        for scenario in &project.scenarios.scenarios {
            let report = runtime.run_scenario(scenario)?;
            assert_eq!(report.scans_executed, 8);
            assert!(report.assertions_verified >= 20);
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_isa18_alarm_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_alarm_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let project = Project::scaffold(&temp_dir, "alarm_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        // Register an alarm on Tank_Level > 90
        use ferrite_core::alarm::{AlarmCondition, AlarmDefinition, AlarmSeverity, AlarmState};
        let alarm_def = AlarmDefinition {
            id: "ALM_TANK_HIGH".to_string(),
            tag: "Tank_Level".to_string(),
            severity: AlarmSeverity::Critical,
            condition: AlarmCondition::High { limit: 90.0 },
            message: "Tank Level Exceeded 90%!".to_string(),
        };
        runtime.register_alarm_definition(alarm_def);

        // Scan with Tank_Level = 50 -> normal
        runtime.set_stimulus("Tank_Level", IecValue::Int(50))?;
        runtime.step_scan()?;
        assert!(runtime.active_alarms().is_empty());

        // Scan with Tank_Level = 95 -> ActiveUnacknowledged
        // (Note: Tank_Level >= 95 also triggers Overpressure_Fault in pump_ctrl FBD, producing 2 active alarms!)
        runtime.set_stimulus("Tank_Level", IecValue::Int(95))?;
        runtime.step_scan()?;
        let active = runtime.active_alarms();
        assert_eq!(active.len(), 2);
        let tank_alm = active
            .iter()
            .find(|a| a.definition.id == "ALM_TANK_HIGH")
            .ok_or("ALM_TANK_HIGH exists")?;
        assert_eq!(tank_alm.state, AlarmState::ActiveUnacknowledged);

        // Acknowledge alarm -> ActiveAcknowledged
        runtime.acknowledge_alarm("ALM_TANK_HIGH");
        let active = runtime.active_alarms();
        let tank_alm = active
            .iter()
            .find(|a| a.definition.id == "ALM_TANK_HIGH")
            .ok_or("ALM_TANK_HIGH exists")?;
        assert_eq!(tank_alm.state, AlarmState::ActiveAcknowledged);

        // Scan with Tank_Level back to 80 -> cleared / removed from active alarms
        runtime.set_stimulus("Tank_Level", IecValue::Int(80))?;
        runtime.step_scan()?;
        assert!(
            runtime
                .active_alarms()
                .iter()
                .all(|a| a.definition.id != "ALM_TANK_HIGH")
        );

        // Check journal has records
        assert!(runtime.alarm_journal().len() >= 3);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_retain_snapshot_persistence() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_retain_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let project = Project::scaffold(&temp_dir, "retain_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        // Set retain on Tank_Level
        if let Some(entry) = runtime.tags.get_tag_mut("Tank_Level") {
            entry.retain = true;
        }
        runtime.set_stimulus("Tank_Level", IecValue::Int(78))?;
        runtime.process_image.write_marker_word(10, 78)?;

        let snap_path = temp_dir.join("retain.json");
        runtime.save_retain_snapshot(&snap_path)?;

        // Create new fresh runtime from project
        let mut runtime2 = PlcRuntime::from_project(&project)?;
        if let Some(entry) = runtime2.tags.get_tag_mut("Tank_Level") {
            entry.retain = true;
        }
        // Initially it should be default
        assert_ne!(
            runtime2.tags.get_tag_value("Tank_Level"),
            Some(&IecValue::Int(78))
        );

        // Load retain snapshot
        runtime2.load_retain_snapshot(&snap_path)?;
        assert_eq!(
            runtime2.tags.get_tag_value("Tank_Level"),
            Some(&IecValue::Int(78))
        );
        assert_eq!(runtime2.process_image.read_marker_word(10)?, 78);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_multi_task_scheduler_cyclic_and_event() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_multitask_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let mut project = Project::scaffold(&temp_dir, "multitask_plant")?;
        // Reconfigure project tasks:
        // Task 1: Fast cyclic (10ms) with main_safety
        // Task 2: Slow cyclic (50ms) with math_pid
        // Task 3: Event task triggered by Overpressure_Fault with pump_ctrl
        use ferrite_project::{TaskConfig, TaskKind};
        project.manifest.scan_cycle_ms = 10;
        project.manifest.tasks = vec![
            TaskConfig {
                name: "FastTask".to_string(),
                kind: TaskKind::Cyclic,
                priority: 1,
                interval_ms: 10,
                trigger_symbol: None,
                pou_order: vec!["main_safety".to_string()],
            },
            TaskConfig {
                name: "SlowTask".to_string(),
                kind: TaskKind::Cyclic,
                priority: 2,
                interval_ms: 50,
                trigger_symbol: None,
                pou_order: vec!["math_pid".to_string()],
            },
            TaskConfig {
                name: "EventTask".to_string(),
                kind: TaskKind::Event,
                priority: 3,
                interval_ms: 0,
                trigger_symbol: Some("Overpressure_Fault".to_string()),
                pou_order: vec!["pump_ctrl".to_string()],
            },
        ];

        let mut runtime = PlcRuntime::from_project(&project)?;
        assert_eq!(runtime.tasks().len(), 3);

        // Run 5 scans (50ms virtual time)
        for _ in 0..5 {
            runtime.step_scan()?;
        }

        // FastTask (10ms) ran 5 times
        assert_eq!(runtime.tasks()[0].execution_count, 5);
        // SlowTask (50ms) ran 1 time
        assert_eq!(runtime.tasks()[1].execution_count, 1);
        // EventTask ran 0 times (Overpressure_Fault not triggered)
        assert_eq!(runtime.tasks()[2].execution_count, 0);

        // Now trigger rising edge on Overpressure_Fault
        runtime.set_stimulus("Overpressure_Fault", IecValue::Bool(true))?;
        runtime.step_scan()?;
        assert_eq!(runtime.tasks()[2].execution_count, 1);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_runtime_sim_speed_batching_and_breakpoint() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_batch_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "batch_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        runtime.breakpoints.push(Breakpoint::ScanCount(3));
        let tel = runtime.step_scan_batch(10)?;
        assert!(tel.breakpoint_hit);
        assert_eq!(tel.scan_count, 3);
        assert_eq!(tel.hit_breakpoint, Some(Breakpoint::ScanCount(3)));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_runtime_warm_vs_cold_restart_and_unforce_all() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = env::temp_dir().join("ferrite_plc_restart_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "restart_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        if let Some(t) = runtime.tags.get_tag_mut("Temp_Setpoint") {
            t.retain = true;
        }
        let _ = runtime.force("Temp_Setpoint", IecValue::Real(88.5));
        let _ = runtime.force("%IX0.0", IecValue::Bool(true));
        assert!(runtime.active_forces_count() >= 2);

        // Warm restart: preserves forces and retain values
        runtime.warm_restart()?;
        assert!(runtime.active_forces_count() >= 2);
        assert_eq!(runtime.read_value("Temp_Setpoint")?, IecValue::Real(88.5));

        // Cold restart: clears forces and resets Setpoint to initial
        runtime.cold_restart()?;
        assert_eq!(runtime.active_forces_count(), 0);
        assert_ne!(runtime.read_value("Temp_Setpoint")?, IecValue::Real(88.5));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_runtime_compound_receptivity_evaluation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_receptivity_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "rec_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        let _ = runtime.force("Start_Button", IecValue::Bool(true));
        let _ = runtime.force("Fast_Recipe_Mode", IecValue::Bool(true));

        assert_eq!(
            runtime.evaluate_expression_bool("Start_Button AND Fast_Recipe_Mode", "main"),
            Some(true)
        );
        assert_eq!(
            runtime.evaluate_expression_bool("Start_Button AND NOT Fast_Recipe_Mode", "main"),
            Some(false)
        );
        assert_eq!(
            runtime.evaluate_expression_bool("Invalid Syntax &^%", "main"),
            None
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_runtime_warm_restart_relatches_inputs_and_resets_fbs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_warm_restart_relatch_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "warm_relatch_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        // Verify initial input EStop_OK is latched TRUE
        assert_eq!(runtime.read_value("EStop_OK")?, IecValue::Bool(true));

        // Mutate hardware input to FALSE and advance a timer
        let estop_addr: ferrite_core::address::DirectAddress = "%IX0.0".parse()?;
        runtime.process_image.set_hw_input(estop_addr, &IecValue::Bool(false))?;
        runtime.process_image.latch_inputs();
        assert_eq!(runtime.read_value("EStop_OK")?, IecValue::Bool(false));

        if let Some(fb) = runtime.tags.get_fb_mut("Fill_Timer") {
            fb.et_ms = 3500;
            fb.q = true;
        }
        assert_eq!(runtime.read_value("Fill_Timer.ET")?, IecValue::Time(3500));

        // Mark Batch_Count (%MW12) as retain and write 42
        let mw12_addr: ferrite_core::address::DirectAddress = "%MW12".parse()?;
        runtime.process_image.write(mw12_addr, &IecValue::Int(42))?;
        if let Some(tag) = runtime.tags.get_tag_mut("Batch_Count") {
            tag.retain = true;
            tag.value = Some(IecValue::Int(42));
        }
        assert_eq!(runtime.read_value("Batch_Count")?, IecValue::Int(42));

        // Call warm_restart()
        runtime.warm_restart()?;

        // EStop_OK must be re-initialized from tag initial value and re-latched to TRUE
        assert_eq!(runtime.read_value("EStop_OK")?, IecValue::Bool(true));
        // Fill_Timer must have accumulators reset
        assert_eq!(runtime.read_value("Fill_Timer.ET")?, IecValue::Time(0));
        assert_eq!(runtime.read_value("Fill_Timer.Q")?, IecValue::Bool(false));
        // Retained tag Batch_Count at %MW12 must be preserved as 42
        assert_eq!(runtime.read_value("Batch_Count")?, IecValue::Int(42));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_regression_i5_breakpoint_pou_fires_only_when_executed() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_i5_bp_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let mut project = Project::scaffold(&temp_dir, "i5_bp_plant")?;
        use ferrite_project::{TaskConfig, TaskKind};
        project.manifest.scan_cycle_ms = 10;
        project.manifest.tasks = vec![
            TaskConfig {
                name: "SlowTask".to_string(),
                kind: TaskKind::Cyclic,
                priority: 1,
                interval_ms: 30,
                trigger_symbol: None,
                pou_order: vec!["math_pid".to_string()],
            },
        ];
        let mut runtime = PlcRuntime::from_project(&project)?;
        let pou_id = runtime.tasks()[0].pous[0].id();

        runtime.breakpoints.push(Breakpoint::Pou(pou_id.clone()));

        // Scan 1 (10ms): SlowTask elapsed 10 < 30ms -> did not run -> BP must NOT hit
        let tel1 = runtime.step_scan()?;
        assert!(!tel1.breakpoint_hit, "Breakpoint fired when POU was not executed!");

        // Scan 2 (20ms): SlowTask elapsed 20 < 30ms -> did not run -> BP must NOT hit
        let tel2 = runtime.step_scan()?;
        assert!(!tel2.breakpoint_hit, "Breakpoint fired when POU was not executed!");

        // Scan 3 (30ms): SlowTask elapsed 30 >= 30ms -> runs! -> BP hits
        let tel3 = runtime.step_scan()?;
        assert!(tel3.breakpoint_hit, "Breakpoint should hit when POU actually executes!");
        assert_eq!(tel3.hit_breakpoint, Some(Breakpoint::Pou(pou_id)));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_regression_i6_retain_snapshot_atomic_and_bak_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_i6_retain_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "i6_retain_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        if let Some(entry) = runtime.tags.get_tag_mut("Tank_Level") {
            entry.retain = true;
        }
        runtime.set_stimulus("Tank_Level", IecValue::Int(99))?;
        runtime.process_image.write_marker_word(10, 99)?;

        let snap_path = temp_dir.join("retain.json");
        // Save initial snapshot
        runtime.save_retain_snapshot(&snap_path)?;
        assert!(snap_path.exists());

        // Mutate and save second snapshot -> creates .bak
        runtime.set_stimulus("Tank_Level", IecValue::Int(100))?;
        runtime.save_retain_snapshot(&snap_path)?;

        let bak_path = snap_path.with_extension("bak");
        assert!(bak_path.exists(), ".bak backup should exist after subsequent save");

        // Corrupt main snapshot file
        fs::write(&snap_path, b"CORRUPTED_JSON_DATA_GARBAGE")?;

        // Loading should fall back to .bak without failing
        let mut runtime2 = PlcRuntime::from_project(&project)?;
        if let Some(entry) = runtime2.tags.get_tag_mut("Tank_Level") {
            entry.retain = true;
        }
        runtime2.load_retain_snapshot(&snap_path)?;
        assert_eq!(
            runtime2.tags.get_tag_value("Tank_Level"),
            Some(&IecValue::Int(99))
        );

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    struct MockSlowPou {
        id: PouId,
        delay_ms: u64,
    }

    impl ExecutablePou for MockSlowPou {
        fn id(&self) -> PouId {
            self.id.clone()
        }
        fn kind(&self) -> LanguageKind {
            LanguageKind::St
        }
        fn execute(&mut self, _ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
            Ok(())
        }
    }

    #[test]
    fn test_per_task_watchdog_flag_and_strict_stop_mode() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_task_watchdog_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "watchdog_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        // Verify task watchdog is derived by default from project global watchdog
        assert_eq!(runtime.tasks[0].watchdog_us, project.manifest.watchdog_us);

        // Inject a slow mock POU with a tight 50us watchdog deadline
        runtime.tasks[0].watchdog_us = 50;
        runtime.tasks[0].pous = vec![Box::new(MockSlowPou {
            id: PouId::new("slow_mock"),
            delay_ms: 2, // 2000us execution >> 50us deadline
        })];

        // 1. Default Mode (record-and-tag, permissive logging):
        // Does NOT fault-stop the runtime, sets task_faulted, and records overrun telemetry
        assert!(!runtime.strict_deadline_mode);
        assert!(!runtime.tasks[0].task_faulted);
        let tel = runtime.step_scan()?;
        assert!(runtime.tasks[0].task_faulted);
        assert_eq!(tel.task_deadline_violations.len(), 1);
        assert_eq!(tel.task_deadline_violations[0].0, runtime.tasks[0].name);
        assert_eq!(tel.task_deadline_violations[0].1, 1); // scan count 1
        assert!(tel.task_deadline_violations[0].2 > 0); // overrun_us
        assert_eq!(runtime.task_deadline_violations.len(), 1);

        // 2. Strict Deadline Mode:
        // Violating task deadline immediately fault-stops runtime with WatchdogTimeout
        runtime.strict_deadline_mode = true;
        let res = runtime.step_scan();
        match res {
            Err(RuntimeError::WatchdogTimeout { elapsed_us, limit_us }) => {
                assert_eq!(limit_us, 50);
                assert!(elapsed_us >= 50);
            }
            other => panic!("Expected WatchdogTimeout error, got: {other:?}"),
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_trace_ring_buffer_capacity_drain_and_fifo_order() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_trace_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "trace_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        let t_level = runtime.tags.tag_id("Tank_Level").ok_or("Tank_Level tag")?;
        let t_batch = runtime.tags.tag_id("Batch_Count").ok_or("Batch_Count tag")?;

        // Configure trace for 2 tags with capacity of 3 samples
        runtime.configure_trace(&[t_level, t_batch], 3);
        assert_eq!(runtime.trace_capacity(), 3);
        assert_eq!(runtime.trace_buffer.len(), 0);

        // Execute 5 scan cycles with varying tag values
        for i in 1..=5 {
            runtime.set_stimulus("Tank_Level", IecValue::Int(i * 10))?;
            runtime.set_stimulus("Batch_Count", IecValue::Int(i * 100))?;
            runtime.step_scan()?;
        }

        // Bounded ring buffer must only retain the last 3 samples (scans 3, 4, 5)
        assert_eq!(runtime.trace_buffer.len(), 3);

        let samples = runtime.drain_trace();
        assert_eq!(samples.len(), 3);
        assert!(runtime.trace_buffer.is_empty());

        // Check sample FIFO ordering and sampled values
        assert_eq!(samples[0].scan_count, 3);
        assert_eq!(samples[0].get_value(t_level), Some(&IecValue::Int(30)));
        assert_eq!(samples[0].get_f64(t_level), Some(30.0));
        assert_eq!(samples[0].get_f64(t_batch), Some(300.0));

        assert_eq!(samples[1].scan_count, 4);
        assert_eq!(samples[1].get_f64(t_level), Some(40.0));
        assert_eq!(samples[1].get_f64(t_batch), Some(400.0));

        assert_eq!(samples[2].scan_count, 5);
        assert_eq!(samples[2].get_f64(t_level), Some(50.0));
        assert_eq!(samples[2].get_f64(t_batch), Some(500.0));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_online_reload_pous_preserving_state_and_sfc_steps() -> Result<(), Box<dyn std::error::Error>> {
        use ferrite_project::PouSource;

        let temp_dir = env::temp_dir().join("ferrite_plc_online_reload_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "reload_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        // Execute 5 initial scans
        for _ in 0..5 {
            runtime.step_scan()?;
        }
        assert_eq!(runtime.scan_count, 5);

        // Mutate live state: set tag values, apply force, verify SFC initial step
        runtime.set_stimulus("Tank_Level", IecValue::Int(84))?;
        runtime.force("Start_Button", IecValue::Bool(true))?;
        assert!(runtime.tags.get_tag("Start_Button").ok_or("tag")?.forced.is_some());

        // SFC step states live in TagTable (e.g. batch_seq.X0.state)
        assert_eq!(runtime.read_value("batch_seq.X0.state")?, IecValue::Bool(true));
        assert_eq!(runtime.read_value("X0.state")?, IecValue::Bool(true));

        // Recompile only main_safety and math_pid POUs; omit batch_seq and pump_ctrl (they will be dropped)
        let tags = project.build_tag_table()?;
        let ld_exec: Box<dyn ExecutablePou> = match project.pous.get("main_safety").ok_or("main_safety")? {
            PouSource::Ld(p) => Box::new(p.compile(&tags)?),
            _ => unreachable!(),
        };
        let st_exec: Box<dyn ExecutablePou> = match project.pous.get("math_pid").ok_or("math_pid")? {
            PouSource::St(p) => Box::new(p.compile(&tags)?),
            _ => unreachable!(),
        };

        let report = runtime.reload_pous_preserving_state(vec![ld_exec, st_exec]);

        // Verify diagnostics in ReloadReport
        assert!(report.updated_pous.contains(&PouId::new("main_safety")));
        assert!(report.updated_pous.contains(&PouId::new("math_pid")));
        assert!(report.dropped_pous.contains(&PouId::new("batch_seq")));
        assert!(report.dropped_pous.contains(&PouId::new("pump_ctrl")));

        // VERIFY STATE PRESERVATION:
        // 1. Scan count is preserved across online reload
        assert_eq!(runtime.scan_count, 5);
        // 2. ProcessImage & TagTable tag values survive
        assert_eq!(runtime.read_value("Tank_Level")?, IecValue::Int(84));
        // 3. Active forces survive
        assert_eq!(runtime.read_value("Start_Button")?, IecValue::Bool(true));
        assert!(runtime.tags.get_tag("Start_Button").ok_or("tag")?.forced.is_some());
        // 4. SFC active step states in TagTable survive
        assert_eq!(runtime.read_value("batch_seq.X0.state")?, IecValue::Bool(true));
        assert_eq!(runtime.read_value("X0.state")?, IecValue::Bool(true));

        // Subsequent scans increment scan count and execute updated POUs smoothly
        runtime.step_scan()?;
        assert_eq!(runtime.scan_count, 6);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_reload_from_project_preserving_state_full_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_project_reload_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let project = Project::scaffold(&temp_dir, "proj_reload_plant")?;
        let mut runtime = PlcRuntime::from_project(&project)?;

        for _ in 0..3 {
            runtime.step_scan()?;
        }
        assert_eq!(runtime.scan_count, 3);
        runtime.set_stimulus("Tank_Level", IecValue::Int(42))?;
        runtime.force("Fast_Recipe_Mode", IecValue::Bool(true))?;

        let report = runtime.reload_from_project_preserving_state(&project)?;
        assert!(report.is_success());
        assert_eq!(report.dropped_pous.len(), 0);
        assert!(report.updated_pous.len() >= 4);

        // Live state preserved
        assert_eq!(runtime.scan_count, 3);
        assert_eq!(runtime.read_value("Tank_Level")?, IecValue::Int(42));
        assert_eq!(runtime.read_value("Fast_Recipe_Mode")?, IecValue::Bool(true));
        assert!(runtime.tags.get_tag("Fast_Recipe_Mode").ok_or("tag")?.forced.is_some());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
