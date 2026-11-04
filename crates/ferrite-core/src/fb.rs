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
    /// Proportional Corrector: `OUT = Kp * (SP - PV)`
    P,
    /// Proportional-Integral Corrector: `OUT = Kp * (e + (1/Ti) * ∫e dt)`
    Pi,
    /// Parallel PID Corrector: `OUT = Kp * e + (1/Ti) * ∫e dt + Td * de/dt`
    PidParallel,
    /// Series (Interacting) PID Corrector: `C(s) = Kp * (1 + 1/(Ti*s)) * (1 + Td*s)`
    PidSeries,
    /// Mixed (Ideal / Standard ISA) PID Corrector: `OUT = Kp * (e + (1/Ti) * ∫e dt + Td * de/dt)`
    #[serde(alias = "PID")]
    PidMixed,
    /// Up/Down Counter (`CU: BOOL, CD: BOOL, R: BOOL, LD: BOOL, PV: INT -> QU: BOOL, QD: BOOL, CV: INT`)
    Ctud,
    /// Pulse Generator / Blinker (`ENABLE: BOOL, TIMELOW: TIME, TIMEHIGH: TIME -> OUT: BOOL`)
    Blink,
    /// Hysteresis / Schmitt Trigger (`IN: REAL, HIGH: REAL, LOW: REAL -> OUT: BOOL`)
    Hysteresis,
    /// Slew-Rate Limiter (`IN: REAL, ASCEND: REAL, DESCEND: REAL, RESET: BOOL -> OUT: REAL`)
    RampReal,
    /// Linear Analog Scaler (`IN: REAL, IN_MIN: REAL, IN_MAX: REAL, OUT_MIN: REAL, OUT_MAX: REAL -> OUT: REAL, ERR: BOOL`)
    ScaleLin,
}

impl StandardFbKind {
    pub const ALL: [Self; 19] = [
        Self::Ton,
        Self::Tof,
        Self::Tp,
        Self::Ctu,
        Self::Ctd,
        Self::Ctud,
        Self::RTrig,
        Self::FTrig,
        Self::Sr,
        Self::Rs,
        Self::Blink,
        Self::Hysteresis,
        Self::RampReal,
        Self::ScaleLin,
        Self::P,
        Self::Pi,
        Self::PidParallel,
        Self::PidSeries,
        Self::PidMixed,
    ];

    pub const CORRECTORS: [Self; 5] = [
        Self::P,
        Self::Pi,
        Self::PidParallel,
        Self::PidSeries,
        Self::PidMixed,
    ];

    pub fn is_corrector(self) -> bool {
        matches!(
            self,
            Self::P | Self::Pi | Self::PidParallel | Self::PidSeries | Self::PidMixed
        )
    }

    pub fn has_integral(self) -> bool {
        matches!(
            self,
            Self::Pi | Self::PidParallel | Self::PidSeries | Self::PidMixed
        )
    }

    pub fn has_derivative(self) -> bool {
        matches!(self, Self::PidParallel | Self::PidSeries | Self::PidMixed)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ton => "TON",
            Self::Tof => "TOF",
            Self::Tp => "TP",
            Self::Ctu => "CTU",
            Self::Ctd => "CTD",
            Self::Ctud => "CTUD",
            Self::RTrig => "R_TRIG",
            Self::FTrig => "F_TRIG",
            Self::Sr => "SR",
            Self::Rs => "RS",
            Self::Blink => "BLINK",
            Self::Hysteresis => "HYSTERESIS",
            Self::RampReal => "RAMP_REAL",
            Self::ScaleLin => "SCALE_LIN",
            Self::P => "P",
            Self::Pi => "PI",
            Self::PidParallel => "PID_PARALLEL",
            Self::PidSeries => "PID_SERIES",
            Self::PidMixed => "PID_MIXED",
        }
    }

    pub fn formula_hint(self) -> &'static str {
        match self {
            Self::P => "u(t) = Kp · e(t)",
            Self::Pi => "u(t) = Kp · [ e(t) + (1/Ti)·∫e(t)dt ]",
            Self::PidParallel => "u(t) = Kp·e(t) + (1/Ti)·∫e(t)dt + Td·de(t)/dt",
            Self::PidSeries => "C(s) = Kp · (1 + 1/(Ti·s)) · (1 + Td·s)",
            Self::PidMixed => "u(t) = Kp · [ e(t) + (1/Ti)·∫e(t)dt + Td·de(t)/dt ]",
            Self::Hysteresis => "OUT = 1 if IN > HIGH; 0 if IN < LOW",
            Self::RampReal => "|d(OUT)/dt| <= ASCEND / DESCEND",
            Self::ScaleLin => "OUT = OUT_MIN + ((IN - IN_MIN)/(IN_MAX - IN_MIN))·(OUT_MAX - OUT_MIN)",
            _ => "",
        }
    }

    pub fn pin_type(self, pin: &str) -> Option<IecType> {
        let p = pin.to_ascii_uppercase();
        if self.is_corrector() {
            return match p.as_str() {
                "EN" | "IN" | "R" | "RESET" | "Q" => Some(IecType::Bool),
                "SP" | "PV" | "ERR" | "KP" | "GAIN" | "TI" | "TD" | "OUT_MIN" | "OUT_MAX"
                | "OUT" | "CV" => Some(IecType::Real),
                _ => None,
            };
        }
        match (self, p.as_str()) {
            (Self::Ton | Self::Tof | Self::Tp, "IN" | "Q") => Some(IecType::Bool),
            (Self::Ton | Self::Tof | Self::Tp, "PT" | "ET") => Some(IecType::Time),
            (Self::Ctu, "CU" | "R" | "Q") => Some(IecType::Bool),
            (Self::Ctu, "PV" | "CV") => Some(IecType::Int),
            (Self::Ctd, "CD" | "LD" | "Q") => Some(IecType::Bool),
            (Self::Ctd, "PV" | "CV") => Some(IecType::Int),
            (Self::Ctud, "CU" | "CD" | "R" | "LD" | "QU" | "QD" | "Q") => Some(IecType::Bool),
            (Self::Ctud, "PV" | "CV") => Some(IecType::Int),
            (Self::RTrig | Self::FTrig, "CLK" | "IN" | "Q") => Some(IecType::Bool),
            (Self::Sr, "S1" | "S" | "R" | "Q1" | "Q") => Some(IecType::Bool),
            (Self::Rs, "S" | "R1" | "R" | "Q1" | "Q") => Some(IecType::Bool),
            (Self::Blink, "ENABLE" | "IN" | "OUT" | "Q") => Some(IecType::Bool),
            (Self::Blink, "TIMELOW" | "TIMEHIGH" | "ET") => Some(IecType::Time),
            (Self::Hysteresis, "IN" | "HIGH" | "LOW") => Some(IecType::Real),
            (Self::Hysteresis, "OUT" | "Q") => Some(IecType::Bool),
            (Self::RampReal, "IN" | "ASCEND" | "DESCEND" | "OUT") => Some(IecType::Real),
            (Self::RampReal, "RESET" | "R" | "Q") => Some(IecType::Bool),
            (Self::ScaleLin, "IN" | "IN_MIN" | "IN_MAX" | "OUT_MIN" | "OUT_MAX" | "OUT") => {
                Some(IecType::Real)
            }
            (Self::ScaleLin, "ERR" | "Q") => Some(IecType::Bool),
            _ => None,
        }
    }
}

impl std::fmt::Display for StandardFbKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PidParallel => write!(f, "PID (Parallel)"),
            Self::PidSeries => write!(f, "PID (Series)"),
            Self::PidMixed => write!(f, "PID (Mixed / ISA)"),
            Self::Pi => write!(f, "PI"),
            Self::P => write!(f, "P"),
            other => write!(f, "{}", other.label()),
        }
    }
}

fn default_kp() -> f32 {
    2.0
}

fn default_ti_s() -> f32 {
    5.0
}

fn default_td_s() -> f32 {
    0.5
}

fn default_out_min() -> f32 {
    -1000.0
}

fn default_out_max() -> f32 {
    1000.0
}

fn default_time_half_ms() -> u64 {
    500
}

fn default_in_max() -> f32 {
    100.0
}

fn default_ramp_rate() -> f32 {
    10.0
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
    pub cd_bool: bool,
    #[serde(default)]
    pub q: bool,
    #[serde(default)]
    pub qu: bool,
    #[serde(default)]
    pub qd: bool,
    // Timer / Blink pins (ms)
    #[serde(default)]
    pub pt_ms: u64,
    #[serde(default)]
    pub et_ms: u64,
    #[serde(default = "default_time_half_ms")]
    pub time_low_ms: u64,
    #[serde(default = "default_time_half_ms")]
    pub time_high_ms: u64,
    // Counter pins
    #[serde(default)]
    pub pv: i16,
    #[serde(default)]
    pub cv: i16,
    // Corrector (P / PI / PID Parallel / Series / Mixed) & Analog parameters & signals
    #[serde(default = "default_kp")]
    pub kp: f32,
    #[serde(default = "default_ti_s")]
    pub ti_s: f32,
    #[serde(default = "default_td_s")]
    pub td_s: f32,
    #[serde(default)]
    pub sp_real: f32,
    #[serde(default)]
    pub pv_real: f32,
    #[serde(default)]
    pub err_real: f32,
    #[serde(default)]
    pub out_real: f32,
    #[serde(default = "default_out_min")]
    pub out_min: f32,
    #[serde(default = "default_out_max")]
    pub out_max: f32,
    #[serde(default)]
    pub in_min: f32,
    #[serde(default = "default_in_max")]
    pub in_max: f32,
    #[serde(default = "default_ramp_rate")]
    pub ascend_rate: f32,
    #[serde(default = "default_ramp_rate")]
    pub descend_rate: f32,
    #[serde(default)]
    pub integral_accum: f32,
    #[serde(default)]
    pub prev_err: f32,
    #[serde(default)]
    pub has_prev_sample: bool,
    #[serde(default)]
    pub direct_err_mode: bool,
    // Internal edge/pulse memory
    #[serde(default)]
    pub prev_in: bool,
    #[serde(default)]
    pub prev_cd: bool,
    #[serde(default)]
    pub running: bool,
}

impl FbInstance {
    /// Resets runtime state and accumulators of the Function Block instance to defaults.
    pub fn reset(&mut self) {
        self.in_bool = self.kind.is_corrector();
        self.r_bool = false;
        self.s_bool = false;
        self.ld_bool = false;
        self.cd_bool = false;
        self.q = false;
        self.qu = false;
        self.qd = false;
        self.et_ms = 0;
        self.cv = 0;
        self.err_real = 0.0;
        self.out_real = 0.0;
        self.integral_accum = 0.0;
        self.prev_err = 0.0;
        self.has_prev_sample = false;
        self.prev_in = false;
        self.prev_cd = false;
        self.running = false;
    }

    pub fn new(name: impl Into<String>, kind: StandardFbKind) -> Self {
        let (out_min, out_max) = match kind {
            StandardFbKind::Hysteresis => (20.0, 80.0),
            StandardFbKind::ScaleLin => (0.0, 100.0),
            _ => (default_out_min(), default_out_max()),
        };
        let mut inst = Self {
            name: name.into(),
            kind,
            comment: String::new(),
            in_bool: kind.is_corrector(),
            r_bool: false,
            s_bool: false,
            ld_bool: false,
            cd_bool: false,
            q: false,
            qu: false,
            qd: false,
            pt_ms: 1000,
            et_ms: 0,
            time_low_ms: default_time_half_ms(),
            time_high_ms: default_time_half_ms(),
            pv: 0,
            cv: 0,
            kp: default_kp(),
            ti_s: default_ti_s(),
            td_s: default_td_s(),
            sp_real: 0.0,
            pv_real: 0.0,
            err_real: 0.0,
            out_real: 0.0,
            out_min,
            out_max,
            in_min: 0.0,
            in_max: default_in_max(),
            ascend_rate: default_ramp_rate(),
            descend_rate: default_ramp_rate(),
            integral_accum: 0.0,
            prev_err: 0.0,
            has_prev_sample: false,
            direct_err_mode: false,
            prev_in: false,
            prev_cd: false,
            running: false,
        };
        if kind == StandardFbKind::FTrig {
            inst.prev_in = false;
        }
        inst
    }

    pub fn read_pin(&self, pin: &str) -> Result<IecValue, CoreError> {
        let p = pin.to_ascii_uppercase();
        if self.kind.is_corrector() {
            return match p.as_str() {
                "EN" | "IN" => Ok(IecValue::Bool(self.in_bool)),
                "R" | "RESET" => Ok(IecValue::Bool(self.r_bool)),
                "Q" => Ok(IecValue::Bool(self.q)),
                "SP" => Ok(IecValue::Real(self.sp_real)),
                "PV" => Ok(IecValue::Real(self.pv_real)),
                "ERR" => Ok(IecValue::Real(self.err_real)),
                "KP" | "GAIN" => Ok(IecValue::Real(self.kp)),
                "TI" => Ok(IecValue::Real(self.ti_s)),
                "TD" => Ok(IecValue::Real(self.td_s)),
                "OUT_MIN" => Ok(IecValue::Real(self.out_min)),
                "OUT_MAX" => Ok(IecValue::Real(self.out_max)),
                "OUT" | "CV" => Ok(IecValue::Real(self.out_real)),
                _ => Err(CoreError::UnknownFbPin {
                    instance: self.name.clone(),
                    pin: pin.to_string(),
                }),
            };
        }
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
            (StandardFbKind::Ctud, "CU") => Ok(IecValue::Bool(self.in_bool)),
            (StandardFbKind::Ctud, "CD") => Ok(IecValue::Bool(self.cd_bool)),
            (StandardFbKind::Ctud, "R") => Ok(IecValue::Bool(self.r_bool)),
            (StandardFbKind::Ctud, "LD") => Ok(IecValue::Bool(self.ld_bool)),
            (StandardFbKind::Ctud, "PV") => Ok(IecValue::Int(self.pv)),
            (StandardFbKind::Ctud, "CV") => Ok(IecValue::Int(self.cv)),
            (StandardFbKind::Ctud, "QU" | "Q") => Ok(IecValue::Bool(self.qu)),
            (StandardFbKind::Ctud, "QD") => Ok(IecValue::Bool(self.qd)),
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
            (StandardFbKind::Blink, "ENABLE" | "IN") => Ok(IecValue::Bool(self.in_bool)),
            (StandardFbKind::Blink, "TIMELOW") => Ok(IecValue::Time(self.time_low_ms)),
            (StandardFbKind::Blink, "TIMEHIGH") => Ok(IecValue::Time(self.time_high_ms)),
            (StandardFbKind::Blink, "ET") => Ok(IecValue::Time(self.et_ms)),
            (StandardFbKind::Blink, "OUT" | "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::Hysteresis, "IN") => Ok(IecValue::Real(self.pv_real)),
            (StandardFbKind::Hysteresis, "HIGH") => Ok(IecValue::Real(self.out_max)),
            (StandardFbKind::Hysteresis, "LOW") => Ok(IecValue::Real(self.out_min)),
            (StandardFbKind::Hysteresis, "OUT" | "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::RampReal, "IN") => Ok(IecValue::Real(self.sp_real)),
            (StandardFbKind::RampReal, "ASCEND") => Ok(IecValue::Real(self.ascend_rate)),
            (StandardFbKind::RampReal, "DESCEND") => Ok(IecValue::Real(self.descend_rate)),
            (StandardFbKind::RampReal, "RESET" | "R") => Ok(IecValue::Bool(self.r_bool)),
            (StandardFbKind::RampReal, "OUT") => Ok(IecValue::Real(self.out_real)),
            (StandardFbKind::RampReal, "Q") => Ok(IecValue::Bool(self.q)),
            (StandardFbKind::ScaleLin, "IN") => Ok(IecValue::Real(self.pv_real)),
            (StandardFbKind::ScaleLin, "IN_MIN") => Ok(IecValue::Real(self.in_min)),
            (StandardFbKind::ScaleLin, "IN_MAX") => Ok(IecValue::Real(self.in_max)),
            (StandardFbKind::ScaleLin, "OUT_MIN") => Ok(IecValue::Real(self.out_min)),
            (StandardFbKind::ScaleLin, "OUT_MAX") => Ok(IecValue::Real(self.out_max)),
            (StandardFbKind::ScaleLin, "OUT") => Ok(IecValue::Real(self.out_real)),
            (StandardFbKind::ScaleLin, "ERR" | "Q") => Ok(IecValue::Bool(self.q)),
            _ => Err(CoreError::UnknownFbPin {
                instance: self.name.clone(),
                pin: pin.to_string(),
            }),
        }
    }

    pub fn write_pin(&mut self, pin: &str, val: &IecValue) -> Result<(), CoreError> {
        let p = pin.to_ascii_uppercase();
        if self.kind.is_corrector() {
            match p.as_str() {
                "EN" | "IN" => {
                    self.in_bool = val.as_bool()?;
                }
                "R" | "RESET" => {
                    self.r_bool = val.as_bool()?;
                }
                "Q" => {
                    self.q = val.as_bool()?;
                }
                "SP" => {
                    self.sp_real = val.as_real()?;
                    self.direct_err_mode = false;
                }
                "PV" => {
                    self.pv_real = val.as_real()?;
                    self.direct_err_mode = false;
                }
                "ERR" => {
                    self.err_real = val.as_real()?;
                    self.direct_err_mode = true;
                }
                "KP" | "GAIN" => {
                    self.kp = val.as_real()?;
                }
                "TI" => {
                    self.ti_s = match val {
                        IecValue::Time(ms) => (*ms as f32) / 1000.0,
                        other => other.as_real()?,
                    };
                }
                "TD" => {
                    self.td_s = match val {
                        IecValue::Time(ms) => (*ms as f32) / 1000.0,
                        other => other.as_real()?,
                    };
                }
                "OUT_MIN" => {
                    self.out_min = val.as_real()?;
                }
                "OUT_MAX" => {
                    self.out_max = val.as_real()?;
                }
                "OUT" | "CV" => {
                    self.out_real = val.as_real()?;
                }
                _ => {
                    return Err(CoreError::UnknownFbPin {
                        instance: self.name.clone(),
                        pin: pin.to_string(),
                    });
                }
            }
            return Ok(());
        }
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
            (StandardFbKind::Ctud, "CU") => {
                self.in_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctud, "CD") => {
                self.cd_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctud, "R") => {
                self.r_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctud, "LD") => {
                self.ld_bool = val.as_bool()?;
            }
            (StandardFbKind::Ctud, "PV") => {
                self.pv = val.as_int()?;
            }
            (StandardFbKind::Ctud, "CV") => {
                self.cv = val.as_int()?;
            }
            (StandardFbKind::Ctud, "QU" | "Q") => {
                self.qu = val.as_bool()?;
                self.q = self.qu;
            }
            (StandardFbKind::Ctud, "QD") => {
                self.qd = val.as_bool()?;
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
            (StandardFbKind::Blink, "ENABLE" | "IN") => {
                self.in_bool = val.as_bool()?;
            }
            (StandardFbKind::Blink, "TIMELOW") => {
                self.time_low_ms = val.as_time_ms()?;
            }
            (StandardFbKind::Blink, "TIMEHIGH") => {
                self.time_high_ms = val.as_time_ms()?;
            }
            (StandardFbKind::Blink, "ET") => {
                self.et_ms = val.as_time_ms()?;
            }
            (StandardFbKind::Blink, "OUT" | "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::Hysteresis, "IN") => {
                self.pv_real = val.as_real()?;
            }
            (StandardFbKind::Hysteresis, "HIGH") => {
                self.out_max = val.as_real()?;
            }
            (StandardFbKind::Hysteresis, "LOW") => {
                self.out_min = val.as_real()?;
            }
            (StandardFbKind::Hysteresis, "OUT" | "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::RampReal, "IN") => {
                self.sp_real = val.as_real()?;
            }
            (StandardFbKind::RampReal, "ASCEND") => {
                self.ascend_rate = val.as_real()?;
            }
            (StandardFbKind::RampReal, "DESCEND") => {
                self.descend_rate = val.as_real()?;
            }
            (StandardFbKind::RampReal, "RESET" | "R") => {
                self.r_bool = val.as_bool()?;
            }
            (StandardFbKind::RampReal, "OUT") => {
                self.out_real = val.as_real()?;
            }
            (StandardFbKind::RampReal, "Q") => {
                self.q = val.as_bool()?;
            }
            (StandardFbKind::ScaleLin, "IN") => {
                self.pv_real = val.as_real()?;
            }
            (StandardFbKind::ScaleLin, "IN_MIN") => {
                self.in_min = val.as_real()?;
            }
            (StandardFbKind::ScaleLin, "IN_MAX") => {
                self.in_max = val.as_real()?;
            }
            (StandardFbKind::ScaleLin, "OUT_MIN") => {
                self.out_min = val.as_real()?;
            }
            (StandardFbKind::ScaleLin, "OUT_MAX") => {
                self.out_max = val.as_real()?;
            }
            (StandardFbKind::ScaleLin, "OUT") => {
                self.out_real = val.as_real()?;
            }
            (StandardFbKind::ScaleLin, "ERR" | "Q") => {
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

    /// Resets internal integral/derivative memory of a P/PI/PID corrector.
    pub fn reset_corrector(&mut self) {
        self.integral_accum = 0.0;
        self.prev_err = 0.0;
        self.has_prev_sample = false;
        self.out_real = 0.0;
        self.q = false;
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
            StandardFbKind::Ctud => {
                if self.r_bool {
                    self.cv = 0;
                } else if self.ld_bool {
                    self.cv = self.pv;
                } else {
                    if self.in_bool && !self.prev_in {
                        self.cv = self.cv.saturating_add(1);
                    }
                    if self.cd_bool && !self.prev_cd {
                        self.cv = self.cv.saturating_sub(1);
                    }
                }
                self.qu = self.cv >= self.pv;
                self.qd = self.cv <= 0;
                self.q = self.qu;
                self.prev_in = self.in_bool;
                self.prev_cd = self.cd_bool;
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
            StandardFbKind::Blink => {
                if !self.in_bool {
                    self.q = false;
                    self.et_ms = 0;
                    self.running = false;
                } else {
                    if !self.running {
                        self.running = true;
                        self.q = true;
                        self.et_ms = 0;
                    }
                    self.et_ms = self.et_ms.saturating_add(dt_ms);
                    let phase_limit = if self.q {
                        self.time_high_ms.max(1)
                    } else {
                        self.time_low_ms.max(1)
                    };
                    if self.et_ms >= phase_limit {
                        self.et_ms = self.et_ms.saturating_sub(phase_limit);
                        self.q = !self.q;
                    }
                }
                self.prev_in = self.in_bool;
            }
            StandardFbKind::Hysteresis => {
                if self.pv_real > self.out_max {
                    self.q = true;
                } else if self.pv_real < self.out_min {
                    self.q = false;
                }
            }
            StandardFbKind::RampReal => {
                if self.r_bool {
                    self.out_real = self.sp_real;
                    self.q = true;
                } else {
                    let dt_s = (dt_ms as f32) / 1000.0;
                    let diff = self.sp_real - self.out_real;
                    if diff > 0.0 {
                        let max_step = self.ascend_rate.abs() * dt_s;
                        self.out_real = (self.out_real + max_step).min(self.sp_real);
                    } else if diff < 0.0 {
                        let max_step = self.descend_rate.abs() * dt_s;
                        self.out_real = (self.out_real - max_step).max(self.sp_real);
                    }
                    self.q = (self.sp_real - self.out_real).abs() <= 1e-5;
                }
            }
            StandardFbKind::ScaleLin => {
                let span_in = self.in_max - self.in_min;
                if span_in.abs() < 1e-9 {
                    self.q = true;
                    self.out_real = self.out_min;
                } else {
                    let (in_lo, in_hi) = if self.in_min <= self.in_max {
                        (self.in_min, self.in_max)
                    } else {
                        (self.in_max, self.in_min)
                    };
                    self.q = self.pv_real < in_lo || self.pv_real > in_hi;
                    let ratio = ((self.pv_real - self.in_min) / span_in).clamp(0.0, 1.0);
                    self.out_real = self.out_min + ratio * (self.out_max - self.out_min);
                }
            }
            StandardFbKind::P
            | StandardFbKind::Pi
            | StandardFbKind::PidParallel
            | StandardFbKind::PidSeries
            | StandardFbKind::PidMixed => {
                if self.r_bool || !self.in_bool {
                    self.reset_corrector();
                    return;
                }

                if !self.direct_err_mode {
                    self.err_real = self.sp_real - self.pv_real;
                }
                let e = self.err_real;
                let dt_s = (dt_ms as f32) / 1000.0;

                // Derivative of error de/dt
                let de_dt = if self.has_prev_sample && dt_s > 1e-6 {
                    (e - self.prev_err) / dt_s
                } else {
                    0.0
                };

                // Update integral accumulator ∫e(t)dt with anti-windup clamping
                if self.kind.has_integral() && self.ti_s > 1e-6 && dt_s > 0.0 {
                    self.integral_accum += e * dt_s;
                    let max_span = (self.out_max - self.out_min).abs().max(1.0);
                    let max_int = max_span * self.ti_s / self.kp.abs().max(1e-3);
                    self.integral_accum = self.integral_accum.clamp(-max_int, max_int);
                }

                let i_over_ti = if self.kind.has_integral() && self.ti_s > 1e-6 {
                    self.integral_accum / self.ti_s
                } else {
                    0.0
                };
                let td_de_dt = if self.kind.has_derivative() && self.td_s > 0.0 {
                    self.td_s * de_dt
                } else {
                    0.0
                };

                let raw_out = match self.kind {
                    StandardFbKind::P => self.kp * e,
                    StandardFbKind::Pi => self.kp * (e + i_over_ti),
                    StandardFbKind::PidParallel => self.kp * e + i_over_ti + td_de_dt,
                    StandardFbKind::PidMixed => self.kp * (e + i_over_ti + td_de_dt),
                    StandardFbKind::PidSeries => {
                        // C(s) = Kp * (1 + 1/(Ti*s)) * (1 + Td*s)
                        // u(t) = Kp * [ (1 + Td/Ti)*e(t) + (1/Ti)*∫e(t)dt + Td*de(t)/dt ]
                        let series_p_factor = if self.ti_s > 1e-6 {
                            1.0 + (self.td_s.max(0.0) / self.ti_s)
                        } else {
                            1.0
                        };
                        self.kp * (series_p_factor * e + i_over_ti + td_de_dt)
                    }
                    _ => 0.0,
                };

                let (lo, hi) = if self.out_min <= self.out_max {
                    (self.out_min, self.out_max)
                } else {
                    (self.out_max, self.out_min)
                };
                self.out_real = raw_out.clamp(lo, hi);
                self.prev_err = e;
                self.has_prev_sample = true;
                self.q = true;
            }
        }
    }
}
