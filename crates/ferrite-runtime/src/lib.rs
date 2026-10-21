use std::time::Instant;

use ferrite_core::{
    AccessWidth, CoreError, DirectAddress, ExecutablePou, IecType, IecValue, MemoryArea, PouId,
    ProcessImage, RuntimeError, ScanContext, TagId, TagTable,
};
use ferrite_project::{Project, ProjectError, SimScenario};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanTelemetry {
    pub scan_count: u64,
    pub virtual_time_ms: u64,
    pub scan_duration_us: u64,
    pub max_scan_duration_us: u64,
    pub breakpoint_hit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioReport {
    pub scenario_name: String,
    pub scans_executed: u64,
    pub assertions_verified: usize,
    pub max_scan_duration_us: u64,
}

/// Deterministic PLC runtime executing the 4-phase scan cycle:
/// 1. Input Scan -> 2. Task & POU Execution -> 3. Output Scan -> 4. Diagnostics & Watchdog.
pub struct PlcRuntime {
    pub process_image: ProcessImage,
    pub tags: TagTable,
    pous: Vec<Box<dyn ExecutablePou>>,
    pub scan_cycle_ms: u64,
    pub watchdog_us: u64,
    pub scan_count: u64,
    pub virtual_time_ms: u64,
    pub last_scan_duration_us: u64,
    pub max_scan_duration_us: u64,
    pub breakpoints: Vec<Breakpoint>,
    pub watch_list: Vec<String>,
}

impl PlcRuntime {
    pub fn from_project(project: &Project) -> Result<Self, ProjectError> {
        let (tags, pous) = project.compile()?;
        let mut process_image = ProcessImage::default();

        // Initialize process image with tag initial values so initial inputs like EStop_OK = TRUE are latched
        for tag in tags.tags() {
            if let Some(addr) = tag.address {
                if addr.area == MemoryArea::Input {
                    let _ = process_image.set_hw_input(addr, &tag.initial_value);
                } else {
                    let _ = process_image.write(addr, &tag.initial_value);
                }
            }
        }
        process_image.latch_inputs();

        let watch_list = tags.tags().iter().map(|t| t.name.clone()).collect();

        Ok(Self {
            process_image,
            tags,
            pous,
            scan_cycle_ms: project.manifest.scan_cycle_ms,
            watchdog_us: project.manifest.watchdog_us,
            scan_count: 0,
            virtual_time_ms: 0,
            last_scan_duration_us: 0,
            max_scan_duration_us: 0,
            breakpoints: Vec::new(),
            watch_list,
        })
    }

    pub fn pous(&self) -> &[Box<dyn ExecutablePou>] {
        &self.pous
    }

    /// Sets a simulated input or tag value prior to the next scan cycle.
    pub fn set_stimulus(&mut self, symbol_or_addr: &str, val: IecValue) -> Result<(), RuntimeError> {
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
            && let Some(addr) = tag.address {
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
            && let Some(addr) = tag.address {
                self.process_image.unforce(addr)?;
            }
        self.tags.unforce_tag(trimmed)?;
        Ok(())
    }

    /// Reads the current value of any symbol, direct address, or FB pin.
    pub fn read_value(&self, symbol: &str) -> Result<IecValue, RuntimeError> {
        let trimmed = symbol.trim();
        if let Some(id) = self.tags.tag_id(trimmed)
            && let Some(tag) = self.tags.get_tag_by_id(id) {
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
            && let Some(fb) = self.tags.get_fb(inst) {
                return Ok(fb.read_pin(pin)?);
            }
        Err(CoreError::UnknownSymbol(trimmed.to_string()).into())
    }

    /// Executes one complete deterministic 4-phase PLC scan cycle.
    pub fn step_scan(&mut self) -> Result<ScanTelemetry, RuntimeError> {
        let start = Instant::now();
        self.scan_count = self.scan_count.saturating_add(1);
        self.virtual_time_ms = self.virtual_time_ms.saturating_add(self.scan_cycle_ms);

        // Phase 1: Input Scan (Latch simulated hardware inputs into ProcessImage input buffer)
        self.process_image.latch_inputs();

        // Synchronize input-mapped tags from ProcessImage without heap allocation
        let tag_len = self.tags.tags().len();
        for idx in 0..tag_len {
            let id = TagId(idx);
            let (addr_opt, ty, is_forced) = match self.tags.get_tag_by_id(id) {
                Some(t) => (t.address, t.data_type, t.forced.is_some()),
                None => continue,
            };
            if !is_forced
                && let Some(addr) = addr_opt
                    && addr.area == MemoryArea::Input {
                        let val = self.process_image.read(addr, ty)?;
                        if let Some(t) = self.tags.get_tag_mut_by_id(id) {
                            t.value = Some(val);
                        }
                    }
        }

        // Phase 2: Task & POU Execution
        {
            let mut ctx = ScanContext::new(
                &mut self.process_image,
                &mut self.tags,
                self.scan_cycle_ms,
                self.scan_count,
            );
            for pou in &mut self.pous {
                pou.execute(&mut ctx)?;
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

        let mut breakpoint_hit = false;
        for bp in &self.breakpoints {
            match bp {
                Breakpoint::ScanCount(target) if *target == self.scan_count => {
                    breakpoint_hit = true;
                }
                Breakpoint::SymbolTrue(sym) => {
                    if let Ok(IecValue::Bool(true)) = self.read_value(sym) {
                        breakpoint_hit = true;
                    }
                }
                _ => {}
            }
        }

        Ok(ScanTelemetry {
            scan_count: self.scan_count,
            virtual_time_ms: self.virtual_time_ms,
            scan_duration_us: elapsed_us,
            max_scan_duration_us: self.max_scan_duration_us,
            breakpoint_hit,
        })
    }

    /// Executes an automated headless test scenario from `sim/scenarios.toml`.
    pub fn run_scenario(&mut self, scenario: &SimScenario) -> Result<ScenarioReport, ScenarioError> {
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
                            self.read_value(sym).map_err(|source| ScenarioError::Runtime {
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
}
