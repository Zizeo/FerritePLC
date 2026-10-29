use std::collections::HashMap;
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
    pub scan_count: u64,
    pub virtual_time_ms: u64,
    pub last_scan_duration_us: u64,
    pub max_scan_duration_us: u64,
    pub breakpoints: Vec<Breakpoint>,
    pub watch_list: Vec<String>,
    input_tag_bindings: Vec<(TagId, DirectAddress, IecType)>,
    pub alarms: Vec<ActiveAlarm>,
    pub alarm_journal: Vec<AlarmEventRecord>,
}

impl PlcRuntime {
    pub fn from_project(project: &Project) -> Result<Self, ProjectError> {
        let (tags, compiled_tasks) = project.compile_tasks()?;
        let tasks = compiled_tasks
            .into_iter()
            .map(|t| ScheduledTask {
                name: t.name,
                kind: t.kind,
                priority: t.priority,
                interval_ms: t.interval_ms,
                trigger_symbol: t.trigger_symbol,
                pous: t.pous,
                elapsed_ms: 0,
                last_trigger_state: false,
                execution_count: 0,
            })
            .collect();
        let mut process_image = ProcessImage::default();

        // Initialize process image with tag initial values so initial inputs like EStop_OK = TRUE are latched
        let mut input_tag_bindings = Vec::new();
        for (idx, tag) in tags.tags().iter().enumerate() {
            if let Some(addr) = tag.address {
                if addr.area == MemoryArea::Input {
                    let _ = process_image.set_hw_input(addr, &tag.initial_value);
                    input_tag_bindings.push((TagId(idx), addr, tag.data_type));
                } else {
                    let _ = process_image.write(addr, &tag.initial_value);
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
            scan_count: 0,
            virtual_time_ms: 0,
            last_scan_duration_us: 0,
            max_scan_duration_us: 0,
            breakpoints: Vec::new(),
            watch_list,
            input_tag_bindings,
            alarms,
            alarm_journal: Vec::new(),
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
            let (addr_opt, data_type) = match self.tags.get_tag_by_id(id) {
                Some(t) => (t.address, t.data_type),
                None => return Err(CoreError::UnknownSymbol(trimmed.to_string()).into()),
            };
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
                    let _ = self.process_image.set_hw_input(addr, &tag.initial_value);
                } else {
                    let _ = self.process_image.write(addr, &tag.initial_value);
                }
            }
        }
        self.process_image.latch_inputs();
        self.tags.reset_fbs();

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
        // - %I inputs are re-initialized with tag initial values and re-latched
        // - Retained tags write their preserved value to process_image if mapped
        // - Non-retain %Q outputs are reset to initial values
        // - %M markers are already restored via import_markers
        for tag in self.tags.tags() {
            if let Some(addr) = tag.address {
                if addr.area == MemoryArea::Input {
                    let _ = self.process_image.set_hw_input(addr, &tag.initial_value);
                } else if tag.retain {
                    let _ = self.process_image.write(addr, tag.current_value());
                } else if addr.area == MemoryArea::Output {
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
        {
            let dt_ms = self.scan_cycle_ms;
            let scan_count = self.scan_count;
            for task in &mut self.tasks {
                let should_run = match task.kind {
                    TaskKind::Continuous => true,
                    TaskKind::Cyclic => {
                        task.elapsed_ms = task.elapsed_ms.saturating_add(dt_ms);
                        if task.elapsed_ms >= task.interval_ms.max(1) {
                            task.elapsed_ms = 0;
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
                    let mut ctx = ScanContext::new(
                        &mut self.process_image,
                        &mut self.tags,
                        dt_ms,
                        scan_count,
                    );
                    for pou in &mut task.pous {
                        pou.execute(&mut ctx)?;
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
                Breakpoint::Pou(id)
                    if self
                        .tasks
                        .iter()
                        .any(|t| t.pous.iter().any(|p| p.id() == *id)) =>
                {
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
    pub fn alarm_journal(&self) -> &[AlarmEventRecord] {
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
                self.alarm_journal.push(AlarmEventRecord {
                    timestamp_ms: time_ms,
                    alarm_id: alarm.definition.id.clone(),
                    state: alarm.state,
                    severity: alarm.definition.severity,
                    message: alarm.definition.message.clone(),
                });
                if self.alarm_journal.len() > 1000 {
                    self.alarm_journal.remove(0);
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
                    self.alarm_journal.push(AlarmEventRecord {
                        timestamp_ms: time_ms,
                        alarm_id: alarm.definition.id.clone(),
                        state: alarm.state,
                        severity: alarm.definition.severity,
                        message: format!("ACK: {}", alarm.definition.message),
                    });
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
                self.alarm_journal.push(AlarmEventRecord {
                    timestamp_ms: time_ms,
                    alarm_id: alarm.definition.id.clone(),
                    state: alarm.state,
                    severity: alarm.definition.severity,
                    message: format!("ACK: {}", alarm.definition.message),
                });
            }
        }
    }

    /// Serializes retentive tags and %M marker memory to disk.
    pub fn save_retain_snapshot(&self, path: &Path) -> std::io::Result<()> {
        let retain_tags = self.tags.export_retain_values();
        let markers = self.process_image.export_markers();
        let payload = RetainSnapshot {
            tags: retain_tags,
            markers,
        };
        let bytes = serde_json::to_vec_pretty(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, bytes)
    }

    /// Restores retentive tags and %M marker memory from disk.
    pub fn load_retain_snapshot(&mut self, path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let bytes = std::fs::read(path)?;
        let snapshot: RetainSnapshot = serde_json::from_slice(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.tags.import_retain_values(&snapshot.tags);
        self.process_image.import_markers(&snapshot.markers);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
