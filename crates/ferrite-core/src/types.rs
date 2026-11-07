use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::CoreError;

/// Standard IEC 61131-3 elementary data types supported by FerritePLC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IecType {
    Bool,
    Sint,
    Int,
    Dint,
    Lint,
    Usint,
    Uint,
    Udint,
    Ulint,
    Real,
    Lreal,
    Time,
    Byte,
    Word,
    Dword,
    Lword,
    String,
}

impl IecType {
    pub const ALL_ELEMENTARY: &'static [Self] = &[
        Self::Bool,
        Self::Sint,
        Self::Int,
        Self::Dint,
        Self::Lint,
        Self::Usint,
        Self::Uint,
        Self::Udint,
        Self::Ulint,
        Self::Real,
        Self::Lreal,
        Self::Time,
        Self::Byte,
        Self::Word,
        Self::Dword,
        Self::Lword,
        Self::String,
    ];

    pub fn default_value(self) -> IecValue {
        match self {
            Self::Bool => IecValue::Bool(false),
            Self::Sint => IecValue::Sint(0),
            Self::Int => IecValue::Int(0),
            Self::Dint => IecValue::Dint(0),
            Self::Lint => IecValue::Lint(0),
            Self::Usint => IecValue::Usint(0),
            Self::Uint => IecValue::Uint(0),
            Self::Udint => IecValue::Udint(0),
            Self::Ulint => IecValue::Ulint(0),
            Self::Real => IecValue::Real(0.0),
            Self::Lreal => IecValue::Lreal(0.0),
            Self::Time => IecValue::Time(0),
            Self::Byte => IecValue::Byte(0),
            Self::Word => IecValue::Word(0),
            Self::Dword => IecValue::Dword(0),
            Self::Lword => IecValue::Lword(0),
            Self::String => IecValue::String(String::new()),
        }
    }

    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Sint
                | Self::Int
                | Self::Dint
                | Self::Lint
                | Self::Usint
                | Self::Uint
                | Self::Udint
                | Self::Ulint
                | Self::Byte
                | Self::Word
                | Self::Dword
                | Self::Lword
                | Self::Real
                | Self::Lreal
                | Self::Time
        )
    }

    pub fn is_signed_integer(self) -> bool {
        matches!(self, Self::Sint | Self::Int | Self::Dint | Self::Lint)
    }

    pub fn is_unsigned_integer(self) -> bool {
        matches!(self, Self::Usint | Self::Uint | Self::Udint | Self::Ulint)
    }

    pub fn is_bit_string(self) -> bool {
        matches!(self, Self::Byte | Self::Word | Self::Dword | Self::Lword)
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::Real | Self::Lreal)
    }

    pub fn byte_size(self) -> usize {
        match self {
            Self::Bool => 1,
            Self::Sint | Self::Usint | Self::Byte => 1,
            Self::Int | Self::Uint | Self::Word => 2,
            Self::Dint | Self::Udint | Self::Dword | Self::Real => 4,
            Self::Lint | Self::Ulint | Self::Lword | Self::Lreal | Self::Time => 8,
            Self::String => 0,
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        s.parse::<Self>().ok()
    }
}

impl FromStr for IecType {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "BOOL" => Ok(Self::Bool),
            "SINT" => Ok(Self::Sint),
            "INT" => Ok(Self::Int),
            "DINT" => Ok(Self::Dint),
            "LINT" => Ok(Self::Lint),
            "USINT" => Ok(Self::Usint),
            "UINT" => Ok(Self::Uint),
            "UDINT" => Ok(Self::Udint),
            "ULINT" => Ok(Self::Ulint),
            "REAL" => Ok(Self::Real),
            "LREAL" => Ok(Self::Lreal),
            "TIME" => Ok(Self::Time),
            "BYTE" => Ok(Self::Byte),
            "WORD" => Ok(Self::Word),
            "DWORD" => Ok(Self::Dword),
            "LWORD" => Ok(Self::Lword),
            "STRING" => Ok(Self::String),
            _ => Err(CoreError::InvalidLiteral(format!("Unknown IEC type '{s}'"))),
        }
    }
}

impl<'de> Deserialize<'de> for IecType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<Self>().map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for IecType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Bool => "BOOL",
            Self::Sint => "SINT",
            Self::Int => "INT",
            Self::Dint => "DINT",
            Self::Lint => "LINT",
            Self::Usint => "USINT",
            Self::Uint => "UINT",
            Self::Udint => "UDINT",
            Self::Ulint => "ULINT",
            Self::Real => "REAL",
            Self::Lreal => "LREAL",
            Self::Time => "TIME",
            Self::Byte => "BYTE",
            Self::Word => "WORD",
            Self::Dword => "DWORD",
            Self::Lword => "LWORD",
            Self::String => "STRING",
        };
        write!(f, "{s}")
    }
}

mod lword_serde {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct LwordMap {
        #[serde(alias = "LWORD", alias = "lword_val")]
        lword: u64,
    }

    pub fn serialize<S>(val: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        LwordMap { lword: *val }.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        LwordMap::deserialize(deserializer).map(|m| m.lword)
    }
}

mod lreal_serde {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct LrealMap {
        #[serde(alias = "LREAL", alias = "lreal_val")]
        lreal: f64,
    }

    pub fn serialize<S>(val: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        LrealMap { lreal: *val }.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        LrealMap::deserialize(deserializer).map(|m| m.lreal)
    }
}

/// Runtime value for an IEC 61131-3 variable or expression.
/// Note: `Time` is stored in milliseconds (`u64`).
///
/// Unambiguous JSON serialization:
/// - Elementary numbers (`Int`, `Dint`, `Real`, `Sint`, `Lint`, `Usint`, `Uint`, `Udint`, `Ulint`, `Byte`, `Word`, `Dword`)
///   serialize as untagged JSON numbers.
/// - `Lword` and `Lreal` serialize as `{ "lword": u64 }` and `{ "lreal": f64 }` respectively,
///   guaranteeing unambiguous deserialization in `serde_json` without collisions with `Time` or `Real`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IecValue {
    Bool(bool),
    Int(i16),
    Dint(i32),
    Real(f32),
    Time(u64),
    String(String),
    Sint(i8),
    Lint(i64),
    Usint(u8),
    Uint(u16),
    Udint(u32),
    Ulint(u64),
    Byte(u8),
    Word(u16),
    Dword(u32),
    #[serde(with = "lword_serde")]
    Lword(u64),
    #[serde(with = "lreal_serde")]
    Lreal(f64),
}

impl IecValue {
    pub fn iec_type(&self) -> IecType {
        match self {
            Self::Bool(_) => IecType::Bool,
            Self::Sint(_) => IecType::Sint,
            Self::Int(_) => IecType::Int,
            Self::Dint(_) => IecType::Dint,
            Self::Lint(_) => IecType::Lint,
            Self::Usint(_) => IecType::Usint,
            Self::Uint(_) => IecType::Uint,
            Self::Udint(_) => IecType::Udint,
            Self::Ulint(_) => IecType::Ulint,
            Self::Real(_) => IecType::Real,
            Self::Lreal(_) => IecType::Lreal,
            Self::Time(_) => IecType::Time,
            Self::Byte(_) => IecType::Byte,
            Self::Word(_) => IecType::Word,
            Self::Dword(_) => IecType::Dword,
            Self::Lword(_) => IecType::Lword,
            Self::String(_) => IecType::String,
        }
    }

    pub fn as_bool(&self) -> Result<bool, CoreError> {
        match self {
            Self::Bool(b) => Ok(*b),
            Self::Sint(v) => Ok(*v != 0),
            Self::Int(v) => Ok(*v != 0),
            Self::Dint(v) => Ok(*v != 0),
            Self::Lint(v) => Ok(*v != 0),
            Self::Usint(v) => Ok(*v != 0),
            Self::Uint(v) => Ok(*v != 0),
            Self::Udint(v) => Ok(*v != 0),
            Self::Ulint(v) => Ok(*v != 0),
            Self::Byte(v) => Ok((*v & 1) != 0),
            Self::Word(v) => Ok((*v & 1) != 0),
            Self::Dword(v) => Ok((*v & 1) != 0),
            Self::Lword(v) => Ok((*v & 1) != 0),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::Bool,
                actual: other.iec_type(),
            }),
        }
    }

    pub fn as_sint(&self) -> Result<i8, CoreError> {
        to_i8(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Sint,
            actual: self.iec_type(),
        })
    }

    pub fn as_int(&self) -> Result<i16, CoreError> {
        to_i16(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Int,
            actual: self.iec_type(),
        })
    }

    pub fn as_dint(&self) -> Result<i32, CoreError> {
        to_i32(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Dint,
            actual: self.iec_type(),
        })
    }

    pub fn as_lint(&self) -> Result<i64, CoreError> {
        to_i64(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Lint,
            actual: self.iec_type(),
        })
    }

    pub fn as_usint(&self) -> Result<u8, CoreError> {
        to_u8(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Usint,
            actual: self.iec_type(),
        })
    }

    pub fn as_uint(&self) -> Result<u16, CoreError> {
        to_u16(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Uint,
            actual: self.iec_type(),
        })
    }

    pub fn as_udint(&self) -> Result<u32, CoreError> {
        to_u32(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Udint,
            actual: self.iec_type(),
        })
    }

    pub fn as_ulint(&self) -> Result<u64, CoreError> {
        to_u64(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Ulint,
            actual: self.iec_type(),
        })
    }

    pub fn as_byte(&self) -> Result<u8, CoreError> {
        to_u8(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Byte,
            actual: self.iec_type(),
        })
    }

    pub fn as_word(&self) -> Result<u16, CoreError> {
        to_u16(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Word,
            actual: self.iec_type(),
        })
    }

    pub fn as_dword(&self) -> Result<u32, CoreError> {
        to_u32(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Dword,
            actual: self.iec_type(),
        })
    }

    pub fn as_lword(&self) -> Result<u64, CoreError> {
        to_u64(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Lword,
            actual: self.iec_type(),
        })
    }

    pub fn as_real(&self) -> Result<f32, CoreError> {
        to_f32(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Real,
            actual: self.iec_type(),
        })
    }

    pub fn as_lreal(&self) -> Result<f64, CoreError> {
        to_f64(self).ok_or_else(|| CoreError::TypeMismatch {
            expected: IecType::Lreal,
            actual: self.iec_type(),
        })
    }

    pub fn as_time_ms(&self) -> Result<u64, CoreError> {
        match self {
            Self::Time(ms) => Ok(*ms),
            other => to_u64(other).ok_or_else(|| CoreError::TypeMismatch {
                expected: IecType::Time,
                actual: other.iec_type(),
            }),
        }
    }

    pub fn as_string(&self) -> Result<&str, CoreError> {
        match self {
            Self::String(s) => Ok(s.as_str()),
            other => Err(CoreError::TypeMismatch {
                expected: IecType::String,
                actual: other.iec_type(),
            }),
        }
    }

    /// Coerces this value into the target `IecType` if compatible.
    ///
    /// Deterministic error-on-impossible policy:
    /// - Signed to unsigned: negative values fail (`CoreError::TypeMismatch`).
    /// - Range overflow: values exceeding target type range fail.
    /// - Float to integer: rounds to nearest (IEC 61131-3 `*_TO_INT`).
    /// - Bit-strings coerce to/from BOOL via LSB (`val & 1 != 0`).
    pub fn coerce_to(&self, target: IecType) -> Result<Self, CoreError> {
        let err = || CoreError::TypeMismatch {
            expected: target,
            actual: self.iec_type(),
        };
        match target {
            IecType::Bool => {
                if let Self::String(s) = self {
                    return match s.trim().to_ascii_uppercase().as_str() {
                        "TRUE" | "1" => Ok(Self::Bool(true)),
                        "FALSE" | "0" => Ok(Self::Bool(false)),
                        _ => Err(err()),
                    };
                }
                self.as_bool().map(Self::Bool)
            }
            IecType::Sint => to_i8(self).map(Self::Sint).ok_or_else(err),
            IecType::Int => to_i16(self).map(Self::Int).ok_or_else(err),
            IecType::Dint => to_i32(self).map(Self::Dint).ok_or_else(err),
            IecType::Lint => to_i64(self).map(Self::Lint).ok_or_else(err),
            IecType::Usint => to_u8(self).map(Self::Usint).ok_or_else(err),
            IecType::Uint => to_u16(self).map(Self::Uint).ok_or_else(err),
            IecType::Udint => to_u32(self).map(Self::Udint).ok_or_else(err),
            IecType::Ulint => to_u64(self).map(Self::Ulint).ok_or_else(err),
            IecType::Byte => to_u8(self).map(Self::Byte).ok_or_else(err),
            IecType::Word => to_u16(self).map(Self::Word).ok_or_else(err),
            IecType::Dword => to_u32(self).map(Self::Dword).ok_or_else(err),
            IecType::Lword => to_u64(self).map(Self::Lword).ok_or_else(err),
            IecType::Real => to_f32(self).map(Self::Real).ok_or_else(err),
            IecType::Lreal => to_f64(self).map(Self::Lreal).ok_or_else(err),
            IecType::Time => {
                if let Self::String(s) = self {
                    parse_iec_time_literal(s)
                        .map(Self::Time)
                        .ok_or_else(|| CoreError::InvalidLiteral(s.clone()))
                } else {
                    to_u64(self).map(Self::Time).ok_or_else(err)
                }
            }
            IecType::String => match self {
                Self::String(s) => Ok(Self::String(s.clone())),
                other => Ok(Self::String(other.to_string())),
            },
        }
    }

    /// Parses a literal string given an expected `IecType` (or infers it).
    pub fn parse_literal(raw: &str, expected: Option<IecType>) -> Result<Self, CoreError> {
        let trimmed = raw.trim();

        // Check if there is an explicit typed prefix e.g. "BYTE#16#FF", "LREAL#3.14", "TIME#500ms"
        if let Some((prefix, body)) = trimmed.split_once('#') {
            let p_upper = prefix.trim().to_ascii_uppercase();
            if let Ok(prefix_ty) = p_upper.parse::<IecType>() {
                return parse_typed_body(body, prefix_ty, raw);
            }
        }

        if let Some(ty) = expected {
            return parse_typed_body(trimmed, ty, raw);
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
        if (trimmed.starts_with("16#") || trimmed.starts_with("0x") || trimmed.starts_with("0X"))
            && let Some(u) = parse_iec_uint(trimmed)
        {
            return Ok(if u <= 0xFF {
                Self::Byte(u as u8)
            } else if u <= 0xFFFF {
                Self::Word(u as u16)
            } else if u <= 0xFFFF_FFFF {
                Self::Dword(u as u32)
            } else {
                Self::Lword(u)
            });
        }
        if (trimmed.starts_with("2#") || trimmed.starts_with("0b") || trimmed.starts_with("0B"))
            && let Some(u) = parse_iec_uint(trimmed)
        {
            return Ok(if u <= 0xFF {
                Self::Byte(u as u8)
            } else if u <= 0xFFFF {
                Self::Word(u as u16)
            } else if u <= 0xFFFF_FFFF {
                Self::Dword(u as u32)
            } else {
                Self::Lword(u)
            });
        }
        if trimmed.contains('.') {
            if let Ok(r) = trimmed.parse::<f32>() {
                return Ok(Self::Real(r));
            }
            if let Ok(lr) = trimmed.parse::<f64>() {
                return Ok(Self::Lreal(lr));
            }
        }
        if let Ok(i) = trimmed.parse::<i16>() {
            return Ok(Self::Int(i));
        }
        if let Ok(d) = trimmed.parse::<i32>() {
            return Ok(Self::Dint(d));
        }
        if let Ok(li) = trimmed.parse::<i64>() {
            return Ok(Self::Lint(li));
        }
        if let Ok(ul) = trimmed.parse::<u64>() {
            return Ok(Self::Ulint(ul));
        }

        Err(CoreError::InvalidLiteral(raw.to_string()))
    }
}

fn parse_typed_body(body: &str, ty: IecType, raw: &str) -> Result<IecValue, CoreError> {
    let trimmed = body.trim();
    match ty {
        IecType::Bool => match trimmed.to_ascii_uppercase().as_str() {
            "TRUE" | "1" => Ok(IecValue::Bool(true)),
            "FALSE" | "0" => Ok(IecValue::Bool(false)),
            _ => Err(CoreError::InvalidLiteral(raw.to_string())),
        },
        IecType::Sint => parse_iec_sint(trimmed)
            .map(IecValue::Sint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Int => parse_iec_sint(trimmed)
            .map(IecValue::Int)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Dint => parse_iec_sint(trimmed)
            .map(IecValue::Dint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Lint => parse_iec_sint(trimmed)
            .map(IecValue::Lint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Usint => parse_iec_uint(trimmed)
            .and_then(|u| u8::try_from(u).ok())
            .map(IecValue::Usint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Uint => parse_iec_uint(trimmed)
            .and_then(|u| u16::try_from(u).ok())
            .map(IecValue::Uint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Udint => parse_iec_uint(trimmed)
            .and_then(|u| u32::try_from(u).ok())
            .map(IecValue::Udint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Ulint => parse_iec_uint(trimmed)
            .map(IecValue::Ulint)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Byte => parse_iec_uint(trimmed)
            .and_then(|u| u8::try_from(u).ok())
            .map(IecValue::Byte)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Word => parse_iec_uint(trimmed)
            .and_then(|u| u16::try_from(u).ok())
            .map(IecValue::Word)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Dword => parse_iec_uint(trimmed)
            .and_then(|u| u32::try_from(u).ok())
            .map(IecValue::Dword)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Lword => parse_iec_uint(trimmed)
            .map(IecValue::Lword)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Real => trimmed
            .parse::<f32>()
            .map(IecValue::Real)
            .map_err(|_| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Lreal => trimmed
            .parse::<f64>()
            .map(IecValue::Lreal)
            .map_err(|_| CoreError::InvalidLiteral(raw.to_string())),
        IecType::Time => parse_iec_time_literal(trimmed)
            .map(IecValue::Time)
            .ok_or_else(|| CoreError::InvalidLiteral(raw.to_string())),
        IecType::String => {
            let unquoted = trimmed
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .or_else(|| trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
                .unwrap_or(trimmed);
            Ok(IecValue::String(unquoted.to_string()))
        }
    }
}

fn parse_iec_uint(s: &str) -> Option<u64> {
    let clean = s.replace('_', "");
    let upper = clean.to_ascii_uppercase();
    if let Some(rest) = upper.strip_prefix("16#").or_else(|| upper.strip_prefix("0X")) {
        u64::from_str_radix(rest, 16).ok()
    } else if let Some(rest) = upper.strip_prefix("2#").or_else(|| upper.strip_prefix("0B")) {
        u64::from_str_radix(rest, 2).ok()
    } else if let Some(rest) = upper.strip_prefix("8#").or_else(|| upper.strip_prefix("0O")) {
        u64::from_str_radix(rest, 8).ok()
    } else {
        clean.parse::<u64>().ok()
    }
}

fn parse_iec_sint<T: TryFrom<i64> + TryFrom<u64>>(s: &str) -> Option<T> {
    let clean = s.replace('_', "");
    let upper = clean.to_ascii_uppercase();
    if upper.starts_with("16#")
        || upper.starts_with("0X")
        || upper.starts_with("2#")
        || upper.starts_with("0B")
        || upper.starts_with("8#")
        || upper.starts_with("0O")
    {
        return parse_iec_uint(s).and_then(|u| T::try_from(u).ok());
    }
    if let Ok(i) = clean.parse::<i64>() {
        return T::try_from(i).ok();
    }
    if let Ok(f) = clean.parse::<f64>() {
        let rounded = f.round();
        if rounded >= (i64::MIN as f64) && rounded <= (i64::MAX as f64) {
            return T::try_from(rounded as i64).ok();
        }
    }
    None
}

fn to_i8(val: &IecValue) -> Option<i8> {
    match val {
        IecValue::Sint(v) => Some(*v),
        IecValue::Int(v) => i8::try_from(*v).ok(),
        IecValue::Dint(v) => i8::try_from(*v).ok(),
        IecValue::Lint(v) => i8::try_from(*v).ok(),
        IecValue::Usint(v) | IecValue::Byte(v) => i8::try_from(*v).ok(),
        IecValue::Uint(v) | IecValue::Word(v) => i8::try_from(*v).ok(),
        IecValue::Udint(v) | IecValue::Dword(v) => i8::try_from(*v).ok(),
        IecValue::Ulint(v) | IecValue::Lword(v) => i8::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if (i8::MIN as f32..=i8::MAX as f32).contains(&r) {
                Some(r as i8)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if (i8::MIN as f64..=i8::MAX as f64).contains(&r) {
                Some(r as i8)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => i8::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_i16(val: &IecValue) -> Option<i16> {
    match val {
        IecValue::Sint(v) => Some(i16::from(*v)),
        IecValue::Int(v) => Some(*v),
        IecValue::Dint(v) => i16::try_from(*v).ok(),
        IecValue::Lint(v) => i16::try_from(*v).ok(),
        IecValue::Usint(v) | IecValue::Byte(v) => Some(i16::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => i16::try_from(*v).ok(),
        IecValue::Udint(v) | IecValue::Dword(v) => i16::try_from(*v).ok(),
        IecValue::Ulint(v) | IecValue::Lword(v) => i16::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if (i16::MIN as f32..=i16::MAX as f32).contains(&r) {
                Some(r as i16)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if (i16::MIN as f64..=i16::MAX as f64).contains(&r) {
                Some(r as i16)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => i16::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_i32(val: &IecValue) -> Option<i32> {
    match val {
        IecValue::Sint(v) => Some(i32::from(*v)),
        IecValue::Int(v) => Some(i32::from(*v)),
        IecValue::Dint(v) => Some(*v),
        IecValue::Lint(v) => i32::try_from(*v).ok(),
        IecValue::Usint(v) | IecValue::Byte(v) => Some(i32::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(i32::from(*v)),
        IecValue::Udint(v) | IecValue::Dword(v) => i32::try_from(*v).ok(),
        IecValue::Ulint(v) | IecValue::Lword(v) => i32::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if (i32::MIN as f32..=i32::MAX as f32).contains(&r) {
                Some(r as i32)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if (i32::MIN as f64..=i32::MAX as f64).contains(&r) {
                Some(r as i32)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => i32::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_i64(val: &IecValue) -> Option<i64> {
    match val {
        IecValue::Sint(v) => Some(i64::from(*v)),
        IecValue::Int(v) => Some(i64::from(*v)),
        IecValue::Dint(v) => Some(i64::from(*v)),
        IecValue::Lint(v) => Some(*v),
        IecValue::Usint(v) | IecValue::Byte(v) => Some(i64::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(i64::from(*v)),
        IecValue::Udint(v) | IecValue::Dword(v) => Some(i64::from(*v)),
        IecValue::Ulint(v) | IecValue::Lword(v) => i64::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            Some(r as i64)
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            Some(r as i64)
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => i64::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_u8(val: &IecValue) -> Option<u8> {
    match val {
        IecValue::Usint(v) | IecValue::Byte(v) => Some(*v),
        IecValue::Uint(v) | IecValue::Word(v) => u8::try_from(*v).ok(),
        IecValue::Udint(v) | IecValue::Dword(v) => u8::try_from(*v).ok(),
        IecValue::Ulint(v) | IecValue::Lword(v) => u8::try_from(*v).ok(),
        IecValue::Sint(v) => u8::try_from(*v).ok(),
        IecValue::Int(v) => u8::try_from(*v).ok(),
        IecValue::Dint(v) => u8::try_from(*v).ok(),
        IecValue::Lint(v) => u8::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if (0.0..=255.0).contains(&r) {
                Some(r as u8)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if (0.0..=255.0).contains(&r) {
                Some(r as u8)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => u8::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_u16(val: &IecValue) -> Option<u16> {
    match val {
        IecValue::Usint(v) | IecValue::Byte(v) => Some(u16::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(*v),
        IecValue::Udint(v) | IecValue::Dword(v) => u16::try_from(*v).ok(),
        IecValue::Ulint(v) | IecValue::Lword(v) => u16::try_from(*v).ok(),
        IecValue::Sint(v) => u16::try_from(*v).ok(),
        IecValue::Int(v) => u16::try_from(*v).ok(),
        IecValue::Dint(v) => u16::try_from(*v).ok(),
        IecValue::Lint(v) => u16::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if (0.0..=65535.0).contains(&r) {
                Some(r as u16)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if (0.0..=65535.0).contains(&r) {
                Some(r as u16)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => u16::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_u32(val: &IecValue) -> Option<u32> {
    match val {
        IecValue::Usint(v) | IecValue::Byte(v) => Some(u32::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(u32::from(*v)),
        IecValue::Udint(v) | IecValue::Dword(v) => Some(*v),
        IecValue::Ulint(v) | IecValue::Lword(v) => u32::try_from(*v).ok(),
        IecValue::Sint(v) => u32::try_from(*v).ok(),
        IecValue::Int(v) => u32::try_from(*v).ok(),
        IecValue::Dint(v) => u32::try_from(*v).ok(),
        IecValue::Lint(v) => u32::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if r >= 0.0 && r <= u32::MAX as f32 {
                Some(r as u32)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if r >= 0.0 && r <= u32::MAX as f64 {
                Some(r as u32)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => u32::try_from(*ms).ok(),
        IecValue::String(_) => None,
    }
}

fn to_u64(val: &IecValue) -> Option<u64> {
    match val {
        IecValue::Usint(v) | IecValue::Byte(v) => Some(u64::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(u64::from(*v)),
        IecValue::Udint(v) | IecValue::Dword(v) => Some(u64::from(*v)),
        IecValue::Ulint(v) | IecValue::Lword(v) => Some(*v),
        IecValue::Sint(v) => u64::try_from(*v).ok(),
        IecValue::Int(v) => u64::try_from(*v).ok(),
        IecValue::Dint(v) => u64::try_from(*v).ok(),
        IecValue::Lint(v) => u64::try_from(*v).ok(),
        IecValue::Real(v) => {
            let r = v.round();
            if r >= 0.0 && r <= u64::MAX as f32 {
                Some(r as u64)
            } else {
                None
            }
        }
        IecValue::Lreal(v) => {
            let r = v.round();
            if r >= 0.0 && r <= u64::MAX as f64 {
                Some(r as u64)
            } else {
                None
            }
        }
        IecValue::Bool(b) => Some(if *b { 1 } else { 0 }),
        IecValue::Time(ms) => Some(*ms),
        IecValue::String(_) => None,
    }
}

fn to_f32(val: &IecValue) -> Option<f32> {
    match val {
        IecValue::Real(v) => Some(*v),
        IecValue::Lreal(v) => Some(*v as f32),
        IecValue::Sint(v) => Some(f32::from(*v)),
        IecValue::Int(v) => Some(f32::from(*v)),
        IecValue::Dint(v) => Some(*v as f32),
        IecValue::Lint(v) => Some(*v as f32),
        IecValue::Usint(v) | IecValue::Byte(v) => Some(f32::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(f32::from(*v)),
        IecValue::Udint(v) | IecValue::Dword(v) => Some(*v as f32),
        IecValue::Ulint(v) | IecValue::Lword(v) => Some(*v as f32),
        IecValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        IecValue::Time(ms) => Some(*ms as f32),
        IecValue::String(_) => None,
    }
}

fn to_f64(val: &IecValue) -> Option<f64> {
    match val {
        IecValue::Lreal(v) => Some(*v),
        IecValue::Real(v) => Some(f64::from(*v)),
        IecValue::Sint(v) => Some(f64::from(*v)),
        IecValue::Int(v) => Some(f64::from(*v)),
        IecValue::Dint(v) => Some(f64::from(*v)),
        IecValue::Lint(v) => Some(*v as f64),
        IecValue::Usint(v) | IecValue::Byte(v) => Some(f64::from(*v)),
        IecValue::Uint(v) | IecValue::Word(v) => Some(f64::from(*v)),
        IecValue::Udint(v) | IecValue::Dword(v) => Some(f64::from(*v)),
        IecValue::Ulint(v) | IecValue::Lword(v) => Some(*v as f64),
        IecValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        IecValue::Time(ms) => Some(*ms as f64),
        IecValue::String(_) => None,
    }
}

impl PartialOrd for IecValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if (self.iec_type().is_float() || other.iec_type().is_float())
            && self.iec_type().is_numeric()
            && other.iec_type().is_numeric()
        {
            let lf = self.as_lreal().ok()?;
            let rf = other.as_lreal().ok()?;
            return lf.partial_cmp(&rf);
        }

        if let (Self::String(a), Self::String(b)) = (self, other) {
            return a.partial_cmp(b);
        }

        if let (Self::Bool(a), Self::Bool(b)) = (self, other) {
            return a.partial_cmp(b);
        }

        if let (Self::Time(a), Self::Time(b)) = (self, other) {
            return a.partial_cmp(b);
        }

        if self.iec_type().is_signed_integer() && other.iec_type().is_signed_integer() {
            let li = self.as_lint().ok()?;
            let ri = other.as_lint().ok()?;
            return li.partial_cmp(&ri);
        }

        if (self.iec_type().is_unsigned_integer() || self.iec_type().is_bit_string())
            && (other.iec_type().is_unsigned_integer() || other.iec_type().is_bit_string())
        {
            let lu = self.as_lword().ok()?;
            let ru = other.as_lword().ok()?;
            return lu.partial_cmp(&ru);
        }

        if self.iec_type().is_signed_integer()
            && (other.iec_type().is_unsigned_integer() || other.iec_type().is_bit_string())
        {
            let li = self.as_lint().ok()?;
            let ru = other.as_lword().ok()?;
            if li < 0 {
                return Some(std::cmp::Ordering::Less);
            }
            return (li as u64).partial_cmp(&ru);
        }

        if (self.iec_type().is_unsigned_integer() || self.iec_type().is_bit_string())
            && other.iec_type().is_signed_integer()
        {
            let lu = self.as_lword().ok()?;
            let ri = other.as_lint().ok()?;
            if ri < 0 {
                return Some(std::cmp::Ordering::Greater);
            }
            return lu.partial_cmp(&(ri as u64));
        }

        if matches!(self, Self::Time(_)) && other.iec_type().is_numeric() {
            let t = self.as_time_ms().ok()?;
            let ru = other.as_lword().ok()?;
            return t.partial_cmp(&ru);
        }
        if self.iec_type().is_numeric() && matches!(other, Self::Time(_)) {
            let lu = self.as_lword().ok()?;
            let t = other.as_time_ms().ok()?;
            return lu.partial_cmp(&t);
        }

        None
    }
}

/// Parses IEC 61131-3 time literals such as `T#500ms`, `T#5s`, `TIME#2m`,
/// `T#1h30m`, `TIME#1d2h3m4s5ms`, or raw ms numbers after a `T#` prefix.
pub fn parse_iec_time_literal(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    let upper = trimmed.to_ascii_uppercase();
    let has_prefix = upper.starts_with("TIME#") || upper.starts_with("T#");
    let body = upper
        .strip_prefix("TIME#")
        .or_else(|| upper.strip_prefix("T#"))
        .unwrap_or(&upper)
        .trim();
    if body.is_empty() {
        return None;
    }

    // Scan a sequence of <number><unit> components (ms|s|m|h|d), e.g. "1h30m".
    let bytes = body.as_bytes();
    let mut total: u64 = 0;
    let mut i = 0usize;
    while i < bytes.len() {
        // Number part (integer or fractional).
        let num_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
        if num_start == i {
            return None;
        }
        let value: f64 = body[num_start..i].parse().ok()?;

        // Unit part.
        let unit_start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let factor_ms: f64 = match &body[unit_start..i] {
            "MS" => 1.0,
            "S" => 1_000.0,
            "M" => 60_000.0,
            "H" => 3_600_000.0,
            "D" => 86_400_000.0,
            // Bare `T#<number>` means raw milliseconds.
            "" if has_prefix => 1.0,
            _ => return None,
        };
        total += (value * factor_ms).round() as u64;
    }
    Some(total)
}

impl fmt::Display for IecValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Self::Sint(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Dint(v) => write!(f, "{v}"),
            Self::Lint(v) => write!(f, "{v}"),
            Self::Usint(v) => write!(f, "{v}"),
            Self::Uint(v) => write!(f, "{v}"),
            Self::Udint(v) => write!(f, "{v}"),
            Self::Ulint(v) => write!(f, "{v}"),
            Self::Byte(v) => write!(f, "16#{v:02X}"),
            Self::Word(v) => write!(f, "16#{v:04X}"),
            Self::Dword(v) => write!(f, "16#{v:08X}"),
            Self::Lword(v) => write!(f, "16#{v:016X}"),
            Self::Real(v) => write!(f, "{v:.3}"),
            Self::Lreal(v) => write!(f, "{v:.3}"),
            Self::Time(ms) => write!(f, "T#{ms}ms"),
            Self::String(s) => write!(f, "'{s}'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iec_type_serde_toml_roundtrip_and_case_insensitivity() -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Wrapper {
            ty: IecType,
        }

        for &ty in IecType::ALL_ELEMENTARY {
            let toml_str = toml::to_string(&Wrapper { ty })?;
            let roundtripped: Wrapper = toml::from_str(&toml_str)?;
            assert_eq!(ty, roundtripped.ty);

            // Test lower-case name parsing
            let lower_toml = format!("ty = \"{s}\"\n", s = ty.to_string().to_ascii_lowercase());
            let parsed_lower: Wrapper = toml::from_str(&lower_toml)?;
            assert_eq!(ty, parsed_lower.ty);
        }
        Ok(())
    }

    #[test]
    fn test_iec_value_serde_json_unambiguous() -> Result<(), Box<dyn std::error::Error>> {
        // LREAL and LWORD must deserialize unambiguously in JSON without colliding with Real or Time
        let lreal_val = IecValue::Lreal(123.456789012345);
        let lreal_json = serde_json::to_string(&lreal_val)?;
        assert!(lreal_json.contains("\"lreal\""));
        let lreal_back: IecValue = serde_json::from_str(&lreal_json)?;
        assert_eq!(lreal_val, lreal_back);

        let lword_val = IecValue::Lword(0xFEDC_BA98_7654_3210);
        let lword_json = serde_json::to_string(&lword_val)?;
        assert!(lword_json.contains("\"lword\""));
        let lword_back: IecValue = serde_json::from_str(&lword_json)?;
        assert_eq!(lword_val, lword_back);

        // Standard JSON numbers still deserialize into elementary types
        let int_val = IecValue::Int(42);
        let int_json = serde_json::to_string(&int_val)?;
        assert_eq!(int_json, "42");
        let int_back: IecValue = serde_json::from_str(&int_json)?;
        assert_eq!(int_val, int_back);

        let real_val = IecValue::Real(2.5);
        let real_json = serde_json::to_string(&real_val)?;
        assert_eq!(real_json, "2.5");
        let real_back: IecValue = serde_json::from_str(&real_json)?;
        assert_eq!(real_val, real_back);

        Ok(())
    }

    #[test]
    fn test_coercion_matrix_signed_unsigned_and_bitstrings() {
        // Deterministic error-on-impossible: negative signed to unsigned fails
        let neg_sint = IecValue::Sint(-10);
        assert!(neg_sint.coerce_to(IecType::Usint).is_err());
        assert!(neg_sint.coerce_to(IecType::Uint).is_err());
        assert!(neg_sint.coerce_to(IecType::Byte).is_err());
        assert!(neg_sint.coerce_to(IecType::Lword).is_err());

        // Range overflow: unsigned to signed when exceeding signed MAX fails
        let large_byte = IecValue::Byte(200);
        assert!(large_byte.coerce_to(IecType::Sint).is_err()); // 200 > 127
        assert_eq!(large_byte.coerce_to(IecType::Int), Ok(IecValue::Int(200)));
        assert_eq!(large_byte.coerce_to(IecType::Dint), Ok(IecValue::Dint(200)));

        // Bitstrings coerce to/from BOOL via LSB
        assert_eq!(IecValue::Byte(0xFE).as_bool(), Ok(false)); // LSB is 0
        assert_eq!(IecValue::Byte(0x01).as_bool(), Ok(true));  // LSB is 1
        assert_eq!(IecValue::Word(0x100).as_bool(), Ok(false));
        assert_eq!(IecValue::Word(0x101).as_bool(), Ok(true));
        assert_eq!(IecValue::Bool(true).coerce_to(IecType::Byte), Ok(IecValue::Byte(1)));
        assert_eq!(IecValue::Bool(false).coerce_to(IecType::Word), Ok(IecValue::Word(0)));

        // Real/Lreal rounding to nearest integer
        assert_eq!(IecValue::Real(1.6).coerce_to(IecType::Sint), Ok(IecValue::Sint(2)));
        assert_eq!(IecValue::Lreal(99.4).coerce_to(IecType::Udint), Ok(IecValue::Udint(99)));
        assert_eq!(IecValue::Lreal(-5.5).coerce_to(IecType::Lint), Ok(IecValue::Lint(-6)));

        // Wider unsigned to smaller unsigned
        assert_eq!(IecValue::Dword(100).coerce_to(IecType::Byte), Ok(IecValue::Byte(100)));
        assert!(IecValue::Dword(1000).coerce_to(IecType::Byte).is_err()); // 1000 > 255
    }

    #[test]
    fn test_parse_literal_new_elementary_types() {
        // With explicit hints
        assert_eq!(IecValue::parse_literal("-120", Some(IecType::Sint)), Ok(IecValue::Sint(-120)));
        assert_eq!(IecValue::parse_literal("250", Some(IecType::Usint)), Ok(IecValue::Usint(250)));
        assert_eq!(IecValue::parse_literal("60000", Some(IecType::Uint)), Ok(IecValue::Uint(60000)));
        assert_eq!(IecValue::parse_literal("16#FF", Some(IecType::Byte)), Ok(IecValue::Byte(255)));
        assert_eq!(IecValue::parse_literal("2#1010", Some(IecType::Word)), Ok(IecValue::Word(10)));
        assert_eq!(IecValue::parse_literal("16#CAFE_BABE", Some(IecType::Dword)), Ok(IecValue::Dword(0xCAFE_BABE)));
        assert_eq!(IecValue::parse_literal("16#0123_4567_89AB_CDEF", Some(IecType::Lword)), Ok(IecValue::Lword(0x0123_4567_89AB_CDEF)));
        assert_eq!(IecValue::parse_literal("12.3456789012", Some(IecType::Lreal)), Ok(IecValue::Lreal(12.3456789012)));

        // With typed prefixes
        assert_eq!(IecValue::parse_literal("BYTE#16#AB", None), Ok(IecValue::Byte(0xAB)));
        assert_eq!(IecValue::parse_literal("SINT#-5", None), Ok(IecValue::Sint(-5)));
        assert_eq!(IecValue::parse_literal("LREAL#98.76543", None), Ok(IecValue::Lreal(98.76543)));
        assert_eq!(IecValue::parse_literal("BOOL#TRUE", None), Ok(IecValue::Bool(true)));
    }

    #[test]
    fn test_iec_value_partial_ord() {
        assert!(IecValue::Sint(-5) < IecValue::Int(10));
        assert!(IecValue::Byte(200) > IecValue::Int(100));
        assert!(IecValue::Sint(-1) < IecValue::Byte(0));
        assert_eq!(IecValue::Int(42).partial_cmp(&IecValue::Dint(42)), Some(std::cmp::Ordering::Equal));
        assert!(IecValue::Real(1.5) < IecValue::Lreal(2.5));
        assert_eq!(IecValue::String("a".to_string()).partial_cmp(&IecValue::Int(1)), None);
    }
}
