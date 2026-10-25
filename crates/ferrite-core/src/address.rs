use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryArea {
    /// Physical/simulated inputs (`%I`)
    Input,
    /// Physical/simulated outputs (`%Q`)
    Output,
    /// Internal marker memory (`%M`)
    Marker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessWidth {
    /// Single bit (`X`, e.g. `%IX0.0`)
    Bit,
    /// 8-bit byte (`B`, e.g. `%IB0`)
    Byte,
    /// 16-bit word (`W`, e.g. `%IW0`)
    Word,
    /// 32-bit double word (`D`, e.g. `%ID0`)
    DWord,
}

/// Represents a direct IEC 61131-3 memory address such as `%IX0.0`, `%QW2`, or `%MW10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DirectAddress {
    pub area: MemoryArea,
    pub width: AccessWidth,
    /// Byte, Word, or DWord index depending on `width`
    pub index: usize,
    /// Bit offset (0..=7) when `width == AccessWidth::Bit`
    pub bit: u8,
}

impl DirectAddress {
    /// Creates a bit-addressed direct address (%IX0.0, %QX1.2, %MX10.7).
    pub const fn bit(area: MemoryArea, index: usize, bit: u8) -> Self {
        Self {
            area,
            width: AccessWidth::Bit,
            index,
            bit,
        }
    }

    /// Creates a byte-addressed direct address (%IB0, %QB1, %MB10).
    pub const fn byte(area: MemoryArea, index: usize) -> Self {
        Self {
            area,
            width: AccessWidth::Byte,
            index,
            bit: 0,
        }
    }

    /// Creates a 16-bit word-addressed direct address (%IW0, %QW1, %MW10).
    pub const fn word(area: MemoryArea, index: usize) -> Self {
        Self {
            area,
            width: AccessWidth::Word,
            index,
            bit: 0,
        }
    }

    /// Creates a 32-bit double-word-addressed direct address (%ID0, %QD1, %MD10).
    pub const fn dword(area: MemoryArea, index: usize) -> Self {
        Self {
            area,
            width: AccessWidth::DWord,
            index,
            bit: 0,
        }
    }

    /// Returns the starting byte offset in the area's byte buffer.
    pub fn byte_offset(self) -> usize {
        match self.width {
            AccessWidth::Bit | AccessWidth::Byte => self.index,
            AccessWidth::Word => self.index * 2,
            AccessWidth::DWord => self.index * 4,
        }
    }

    /// Returns the number of bytes spanned by this address.
    pub fn byte_len(self) -> usize {
        match self.width {
            AccessWidth::Bit | AccessWidth::Byte => 1,
            AccessWidth::Word => 2,
            AccessWidth::DWord => 4,
        }
    }
}

impl FromStr for DirectAddress {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let rest = trimmed
            .strip_prefix('%')
            .ok_or_else(|| CoreError::InvalidDirectAddress(s.to_string()))?;

        let mut chars = rest.chars();
        let area_ch = chars
            .next()
            .ok_or_else(|| CoreError::InvalidDirectAddress(s.to_string()))?
            .to_ascii_uppercase();
        let area = match area_ch {
            'I' => MemoryArea::Input,
            'Q' => MemoryArea::Output,
            'M' => MemoryArea::Marker,
            _ => return Err(CoreError::InvalidDirectAddress(s.to_string())),
        };

        let width_ch = chars
            .next()
            .ok_or_else(|| CoreError::InvalidDirectAddress(s.to_string()))?
            .to_ascii_uppercase();
        let width = match width_ch {
            'X' => AccessWidth::Bit,
            'B' => AccessWidth::Byte,
            'W' => AccessWidth::Word,
            'D' => AccessWidth::DWord,
            _ => return Err(CoreError::InvalidDirectAddress(s.to_string())),
        };

        let tail = chars.as_str();
        match width {
            AccessWidth::Bit => {
                let (byte_str, bit_str) = tail
                    .split_once('.')
                    .ok_or_else(|| CoreError::InvalidDirectAddress(s.to_string()))?;
                let index = byte_str
                    .parse::<usize>()
                    .map_err(|_| CoreError::InvalidDirectAddress(s.to_string()))?;
                let bit = bit_str
                    .parse::<u8>()
                    .map_err(|_| CoreError::InvalidDirectAddress(s.to_string()))?;
                if bit > 7 {
                    return Err(CoreError::InvalidDirectAddress(s.to_string()));
                }
                Ok(Self {
                    area,
                    width,
                    index,
                    bit,
                })
            }
            AccessWidth::Byte | AccessWidth::Word | AccessWidth::DWord => {
                let index = tail
                    .parse::<usize>()
                    .map_err(|_| CoreError::InvalidDirectAddress(s.to_string()))?;
                Ok(Self {
                    area,
                    width,
                    index,
                    bit: 0,
                })
            }
        }
    }
}

impl fmt::Display for DirectAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let area_ch = match self.area {
            MemoryArea::Input => 'I',
            MemoryArea::Output => 'Q',
            MemoryArea::Marker => 'M',
        };
        match self.width {
            AccessWidth::Bit => write!(f, "%{area_ch}X{}.{}", self.index, self.bit),
            AccessWidth::Byte => write!(f, "%{area_ch}B{}", self.index),
            AccessWidth::Word => write!(f, "%{area_ch}W{}", self.index),
            AccessWidth::DWord => write!(f, "%{area_ch}D{}", self.index),
        }
    }
}

impl Serialize for DirectAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DirectAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}
