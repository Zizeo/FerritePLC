use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Standard IEC 61131-3 elementary data types supported by FerritePLC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IecType {
    Bool,
    Int,
    Dint,
    Real,
    Time,
    String,
}

impl IecType {
    pub fn default_value(self) -> IecValue {
        match self {
            Self::Bool => IecValue::Bool(false),
            Self::Int => IecValue::Int(0),
            Self::Dint => IecValue::Dint(0),
            Self::Real => IecValue::Real(0.0),
            Self::Time => IecValue::Time(0),
            Self::String => IecValue::String(String::new()),
        }
    }

    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Dint | Self::Real | Self::Time)
    }
}

impl fmt::Display for IecType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Bool => "BOOL",
            Self::Int => "INT",
            Self::Dint => "DINT",
            Self::Real => "REAL",
            Self::Time => "TIME",
            Self::String => "STRING",
        };
        write!(f, "{s}")
    }
}

/// Runtime value for an IEC 61131-3 variable or expression.
/// Note: `Time` is stored in milliseconds (`u64`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IecValue {
    Bool(bool),
    Int(i16),
    Dint(i32),
    Real(f32),
    Time(u64),
    String(String),
}

impl IecValue {
    pub fn iec_type(&self) -> IecType {
        match self {
            Self::Bool(_) => IecType::Bool,
            Self::Int(_) => IecType::Int,
            Self::Dint(_) => IecType::Dint,
            Self::Real(_) => IecType::Real,
            Self::Time(_) => IecType::Time,
            Self::String(_) => IecType::String,
        }
    }

    pub fn as_bool(&self) -> Result<bool, CoreError> {
        match self {
            Self::Bool(b) => Ok(*b),
            Self::Int(v) => Ok(*v != 0),
            Self::Dint(v) => Ok(*v != 0),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::Bool,
                actual: other.iec_type(),
            }),
        }
    }

    pub fn as_int(&self) -> Result<i16, CoreError> {
        match self {
            Self::Int(v) => Ok(*v),
            Self::Dint(v) => Ok(*v as i16),
            Self::Real(v) => Ok(*v as i16),
            Self::Bool(b) => Ok(if *b { 1 } else { 0 }),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::Int,
                actual: other.iec_type(),
            }),
        }
    }

    pub fn as_dint(&self) -> Result<i32, CoreError> {
        match self {
            Self::Dint(v) => Ok(*v),
            Self::Int(v) => Ok(i32::from(*v)),
            Self::Real(v) => Ok(*v as i32),
            Self::Time(v) => Ok(*v as i32),
            Self::Bool(b) => Ok(if *b { 1 } else { 0 }),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::Dint,
                actual: other.iec_type(),
            }),
        }
    }

    pub fn as_real(&self) -> Result<f32, CoreError> {
        match self {
            Self::Real(v) => Ok(*v),
            Self::Int(v) => Ok(f32::from(*v)),
            Self::Dint(v) => Ok(*v as f32),
            Self::Time(v) => Ok(*v as f32),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::Real,
                actual: other.iec_type(),
            }),
        }
    }

    pub fn as_time_ms(&self) -> Result<u64, CoreError> {
        match self {
            Self::Time(ms) => Ok(*ms),
            Self::Int(v) if *v >= 0 => Ok(*v as u64),
            Self::Dint(v) if *v >= 0 => Ok(*v as u64),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::Time,
                actual: other.iec_type(),
            }),
        }
    }

    /// Coerces this value into the target `IecType` if compatible.
    pub fn coerce_to(&self, target: IecType) -> Result<Self, CoreError> {
        match (self, target) {
            (Self::Bool(b), IecType::Bool) => Ok(Self::Bool(*b)),
            (Self::Int(v), IecType::Int) => Ok(Self::Int(*v)),
            (Self::Dint(v), IecType::Int) => Ok(Self::Int(*v as i16)),
            (Self::Real(v), IecType::Int) => Ok(Self::Int(*v as i16)),
            (Self::Int(v), IecType::Dint) => Ok(Self::Dint(i32::from(*v))),
            (Self::Dint(v), IecType::Dint) => Ok(Self::Dint(*v)),
            (Self::Real(v), IecType::Dint) => Ok(Self::Dint(*v as i32)),
            (Self::Time(v), IecType::Dint) => Ok(Self::Dint(*v as i32)),
            (Self::Int(v), IecType::Real) => Ok(Self::Real(f32::from(*v))),
            (Self::Dint(v), IecType::Real) => Ok(Self::Real(*v as f32)),
            (Self::Real(v), IecType::Real) => Ok(Self::Real(*v)),
            (Self::Time(ms), IecType::Time) => Ok(Self::Time(*ms)),
            (Self::Int(v), IecType::Time) if *v >= 0 => Ok(Self::Time(*v as u64)),
            (Self::Dint(v), IecType::Time) if *v >= 0 => Ok(Self::Time(*v as u64)),
            (Self::String(s), IecType::String) => {
                if let Some(ms) = parse_iec_time_literal(s) {
                    let _ = ms;
                }
                Ok(Self::String(s.clone()))
            }
            (Self::String(s), IecType::Time) => {
                parse_iec_time_literal(s)
                    .map(Self::Time)
                    .ok_or_else(|| CoreError::InvalidLiteral(s.clone()))
            }
            _ => Err(CoreError::TypeMismatch {
                expected: target,
                actual: self.iec_type(),
            }),
        }
    }

    /// Parses a literal string given an expected `IecType` (or infers it).
    pub fn parse_literal(raw: &str, expected: Option<IecType>) -> Result<Self, CoreError> {
        let trimmed = raw.trim();
        if let Some(ty) = expected {
            return match ty {
                IecType::Bool => match trimmed.to_ascii_uppercase().as_str() {
                    "TRUE" | "1" => Ok(Self::Bool(true)),
                    "FALSE" | "0" => Ok(Self::Bool(false)),
                    _ => Err(CoreError::InvalidLiteral(raw.to_string())),
                },
                IecType::Int => trimmed
                    .parse::<i16>()
                    .map(Self::Int)
                    .map_err(|_| CoreError::InvalidLiteral(raw.to_string())),
                IecType::Dint => trimmed
                    .parse::<i32>()
                    .map(Self::Dint)
                    .map_err(|_| CoreError::InvalidLiteral(raw.to_string())),
                IecType::Real => trimmed
                    .parse::<f32>()
                    .map(Self::Real)
                    .map_err(|_| CoreError::InvalidLiteral(raw.to_string())),
                IecType::Time => parse_iec_time_literal(trimmed)
                    .map(Self::Time)
                    .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
                IecType::String => {
                    let unquoted = trimmed
                        .strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                        .or_else(|| trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
                        .unwrap_or(trimmed);
                    Ok(Self::String(unquoted.to_string()))
                }
            };
        }

        // Infer type from literal format
        let upper = trimmed.to_ascii_uppercase();
        if upper == "TRUE" {
            return Ok(Self::Bool(true));
        }
        if upper == "FALSE" {
            return Ok(Self::Bool(false));
        }
        if let Some(ms) = parse_iec_time_literal(trimmed) {
            return Ok(Self::Time(ms));
        }
        if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        {
            return Ok(Self::String(trimmed[1..trimmed.len() - 1].to_string()));
        }
        if trimmed.contains('.')
            && let Ok(r) = trimmed.parse::<f32>() {
                return Ok(Self::Real(r));
            }
        if let Ok(i) = trimmed.parse::<i16>() {
            return Ok(Self::Int(i));
        }
        if let Ok(d) = trimmed.parse::<i32>() {
            return Ok(Self::Dint(d));
        }
        Err(CoreError::InvalidLiteral(raw.to_string()))
    }
}

/// Parses IEC 61131-3 time literals such as `T#500ms`, `T#5s`, `TIME#2m`, or raw ms numbers.
pub fn parse_iec_time_literal(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    let upper = trimmed.to_ascii_uppercase();
    let body = upper
        .strip_prefix("TIME#")
        .or_else(|| upper.strip_prefix("T#"))
        .unwrap_or(&upper);

    if let Some(ms_str) = body.strip_suffix("MS") {
        return ms_str.trim().parse::<u64>().ok();
    }
    if let Some(s_str) = body.strip_suffix('S')
        && let Ok(secs) = s_str.trim().parse::<f64>()
            && secs >= 0.0 {
                return Some((secs * 1000.0).round() as u64);
            }
    if let Some(m_str) = body.strip_suffix('M')
        && let Ok(mins) = m_str.trim().parse::<f64>()
            && mins >= 0.0 {
                return Some((mins * 60_000.0).round() as u64);
            }
    if upper.starts_with("T#") || upper.starts_with("TIME#") {
        return body.trim().parse::<u64>().ok();
    }
    None
}

impl fmt::Display for IecValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Self::Int(v) => write!(f, "{v}"),
            Self::Dint(v) => write!(f, "{v}"),
            Self::Real(v) => write!(f, "{v:.3}"),
            Self::Time(ms) => write!(f, "T#{ms}ms"),
            Self::String(s) => write!(f, "'{s}'"),
        }
    }
}
