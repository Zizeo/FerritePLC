use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::types::{IecType, IecValue};

/// Standard IEC 61131-3 Function Block kinds supported across FBD, LD, SFC, and ST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StandardFbKind {
    /// On-Delay Timer (`IN: BOOL, PT: TIME -> Q: BOOL, ET: TIME`)
    Ton,
    /// Off-Delay Timer (`IN: BOOL, PT: TIME -> Q: BOOL, ET: TIME`)
    Tof,
    /// Pulse Timer (`IN: BOOL, PT: TIME -> Q: BOOL, ET: TIME`)
    Tp,
    /// Count Up (`CU: BOOL, R: BOOL, PV: INT -> Q: BOOL, CV: INT`)
    Ctu,
    /// Count Down (`CD: BOOL, LD: BOOL, PV: INT -> Q: BOOL, CV: INT`)
    Ctd,
    /// Rising Edge Detector (`CLK: BOOL -> Q: BOOL`)
    RTrig,
    /// Falling Edge Detector (`CLK: BOOL -> Q: BOOL`)
    FTrig,
    /// Set-Dominant Bistable (`S1: BOOL, R: BOOL -> Q1: BOOL`)
    Sr,
    /// Reset-Dominant Bistable (`S: BOOL, R1: BOOL -> Q1: BOOL`)
    Rs,
}

impl StandardFbKind {
    pub fn pin_type(self, pin: &str) -> Option<IecType> {
        let p = pin.to_ascii_uppercase();
        match (self, p.as_str()) {
            (Self::Ton | Self::Tof | Self::Tp, "IN" | "Q") => Some(IecType::Bool),
            (Self::Ton | Self::Tof | Self::Tp, "PT" | "ET") => Some(IecType::Time),
            (Self::Ctu, "CU" | "R" | "Q") => Some(IecType::Bool),
            (Self::Ctu, "PV" | "CV") => Some(IecType::Int),
            (Self::Ctd, "CD" | "LD" | "Q") => Some(IecType::Bool),
            (Self::Ctd, "PV" | "CV") => Some(IecType::Int),
            (Self::RTrig | Self::FTrig, "CLK" | "IN" | "Q") => Some(IecType::Bool),
            (Self::Sr, "S1" | "S" | "R" | "Q1" | "Q") => Some(IecType::Bool),
            (Self::Rs, "S" | "R1" | "R" | "Q1" | "Q") => Some(IecType::Bool),
            _ => None,
        }
    }
}

/// Fixed, allocation-free state for a shared Function Block instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FbInstance {
    pub name: String,
    pub kind: StandardFbKind,
    #[serde(default)]
    pub comment: String,
    // Boolean pins
    #[serde(default)]
    pub in_bool: bool,
    #[serde(default)]
    pub r_bool: bool,
    #[serde(default)]
    pub s_bool: bool,
    #[serde(default)]
    pub ld_bool: bool,
    #[serde(default)]
    pub q: bool,
    // Timer pins (ms)
    #[serde(default)]
    pub pt_ms: u64,
    #[serde(default)]
    pub et_ms: u64,
    // Counter pins
    #[serde(default)]
    pub pv: i16,
    #[serde(default)]
    pub cv: i16,
    // Internal edge/pulse memory
    #[serde(default)]
    pub prev_in: bool,
    #[serde(default)]
    pub running: bool,
}

impl FbInstance {
    pub fn new(name: impl Into<String>, kind: StandardFbKind) -> Self {
        let mut inst = Self {
            name: name.into(),
            kind,
            comment: String::new(),
            in_bool: false,
            r_bool: false,
            s_bool: false,
            ld_bool: false,
            q: false,
            pt_ms: 1000,
            et_ms: 0,
            pv: 0,
            cv: 0,
            prev_in: false,
            running: false,
        };
        if kind == StandardFbKind::FTrig {
            inst.prev_in = false;
        }
        inst
    }

    pub fn read_pin(&self, pin: &str) -> Result<IecValue, CoreError> {
        let p = pin.to_ascii_uppercase();
        match (self.kind, p.as_str()) {
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "IN") => {
                Ok(IecValue::Bool(self.in_bool))
            }
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "PT") => {
                Ok(IecValue::Time(self.pt_ms))
            }
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "Q") => {
                Ok(IecValue::Bool(self.q))
            }
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "ET") => {
                Ok(IecValue::Time(self.et_ms))
            }
            (StandardFbKind::Ctu, "CU") => Ok(IecValue::Bool(self.in_bool)),
            (StandardFbKind::Ctu, "R") => Ok(IecValue::Bool(self.r_bool)),
            (StandardFbKind::Ctu, "PV") => Ok(IecValue::Int(self.pv)),
            (StandardFbKind::Ctu, "CV") => Ok(IecValue::Int(self.cv)),
            (StandardFbKind::Ctu, "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::Ctd, "CD") => Ok(IecValue::Bool(self.in_bool)),
            (StandardFbKind::Ctd, "LD") => Ok(IecValue::Bool(self.ld_bool)),
            (StandardFbKind::Ctd, "PV") => Ok(IecValue::Int(self.pv)),
            (StandardFbKind::Ctd, "CV") => Ok(IecValue::Int(self.cv)),
            (StandardFbKind::Ctd, "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::RTrig | StandardFbKind::FTrig, "CLK" | "IN") => {
                Ok(IecValue::Bool(self.in_bool))
            }
            (StandardFbKind::RTrig | StandardFbKind::FTrig, "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::Sr, "S1" | "S") => Ok(IecValue::Bool(self.s_bool)),
            (StandardFbKind::Sr, "R") => Ok(IecValue::Bool(self.r_bool)),
            (StandardFbKind::Sr, "Q1" | "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::Rs, "S") => Ok(IecValue::Bool(self.s_bool)),
            (StandardFbKind::Rs, "R1" | "R") => Ok(IecValue::Bool(self.r_bool)),
            (StandardFbKind::Rs, "Q1" | "Q") => Ok(IecValue::Bool(self.q)),
            _ => Err(CoreError::UnknownFbPin {
                instance: self.name.clone(),
                pin: pin.to_string(),
            }),
        }
    }

    pub fn write_pin(&mut self, pin: &str, val: &IecValue) -> Result<(), CoreError> {
        let p = pin.to_ascii_uppercase();
        match (self.kind, p.as_str()) {
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "IN") => {
                self.in_bool = val.as_bool()?;
            }
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "PT") => {
                self.pt_ms = val.as_time_ms()?;
            }
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp, "ET") => {
                self.et_ms = val.as_time_ms()?;
            }
            (StandardFbKind::Ctu, "CU") => {
                self.in_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctu, "R") => {
                self.r_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctu, "PV") => {
                self.pv = val.as_int()?;
            }
            (StandardFbKind::Ctu, "CV") => {
                self.cv = val.as_int()?;
            }
            (StandardFbKind::Ctu, "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::Ctd, "CD") => {
                self.in_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctd, "LD") => {
                self.ld_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctd, "PV") => {
                self.pv = val.as_int()?;
            }
            (StandardFbKind::Ctd, "CV") => {
                self.cv = val.as_int()?;
            }
            (StandardFbKind::Ctd, "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::RTrig | StandardFbKind::FTrig, "CLK" | "IN") => {
                self.in_bool = val.as_bool()?;
            }
            (StandardFbKind::RTrig | StandardFbKind::FTrig, "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::Sr, "S1" | "S") => {
                self.s_bool = val.as_bool()?;
            }
            (StandardFbKind::Sr, "R") => {
                self.r_bool = val.as_bool()?;
            }
            (StandardFbKind::Sr, "Q1" | "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::Rs, "S") => {
                self.s_bool = val.as_bool()?;
            }
            (StandardFbKind::Rs, "R1" | "R") => {
                self.r_bool = val.as_bool()?;
            }
            (StandardFbKind::Rs, "Q1" | "Q") => {
                self.q = val.as_bool()?;
            }
            _ => {
                return Err(CoreError::UnknownFbPin {
                    instance: self.name.clone(),
                    pin: pin.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Evaluates the Function Block state machine for a scan step of `dt_ms` milliseconds.
    /// Performs zero heap allocations.
    pub fn evaluate(&mut self, dt_ms: u64) {
        match self.kind {
            StandardFbKind::Ton => {
                if !self.in_bool {
                    self.et_ms = 0;
                    self.q = false;
                } else {
                    self.et_ms = (self.et_ms.saturating_add(dt_ms)).min(self.pt_ms);
                    self.q = self.et_ms >= self.pt_ms;
                }
                self.prev_in = self.in_bool;
            }
            StandardFbKind::Tof => {
                if self.in_bool {
                    self.et_ms = 0;
                    self.q = true;
                    self.running = false;
                } else {
                    if self.prev_in {
                        self.running = true;
                    }
                    if self.running {
                        self.et_ms = (self.et_ms.saturating_add(dt_ms)).min(self.pt_ms);
                        if self.et_ms >= self.pt_ms {
                            self.q = false;
                            self.running = false;
                        } else {
                            self.q = true;
                        }
                    } else {
                        self.q = false;
                    }
                }
                self.prev_in = self.in_bool;
            }
            StandardFbKind::Tp => {
                if !self.running {
                    if self.in_bool && !self.prev_in {
                        self.running = true;
                        self.et_ms = 0;
                        self.q = self.pt_ms > 0;
                    } else {
                        self.q = false;
                        if !self.in_bool {
                            self.et_ms = 0;
                        }
                    }
                } else {
                    self.et_ms = (self.et_ms.saturating_add(dt_ms)).min(self.pt_ms);
                    if self.et_ms >= self.pt_ms {
                        self.q = false;
                        self.running = false;
                        if !self.in_bool {
                            self.et_ms = 0;
                        }
                    } else {
                        self.q = true;
                    }
                }
                self.prev_in = self.in_bool;
            }
            StandardFbKind::Ctu => {
                if self.r_bool {
                    self.cv = 0;
                } else if self.in_bool && !self.prev_in {
                    self.cv = self.cv.saturating_add(1);
                }
                self.q = self.cv >= self.pv;
                self.prev_in = self.in_bool;
            }
            StandardFbKind::Ctd => {
                if self.ld_bool {
                    self.cv = self.pv;
                } else if self.in_bool && !self.prev_in {
                    self.cv = self.cv.saturating_sub(1);
                }
                self.q = self.cv <= 0;
                self.prev_in = self.in_bool;
            }
            StandardFbKind::RTrig => {
                self.q = self.in_bool && !self.prev_in;
                self.prev_in = self.in_bool;
            }
            StandardFbKind::FTrig => {
                self.q = !self.in_bool && self.prev_in;
                self.prev_in = self.in_bool;
            }
            StandardFbKind::Sr => {
                // Set-dominant: Q1 := S1 OR (NOT R AND Q1)
                self.q = self.s_bool || (!self.r_bool && self.q);
            }
            StandardFbKind::Rs => {
                // Reset-dominant: Q1 := NOT R1 AND (S OR Q1)
                self.q = !self.r_bool && (self.s_bool || self.q);
            }
        }
    }
}
