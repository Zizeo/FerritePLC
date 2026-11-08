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
    /// 64-bit long word (`L`, e.g. `%IL0`, `%QL1`, `%ML10`)
    LWord,
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

    /// Creates a validated bit-addressed direct address, returning an error on overflow or bit > 7.
    pub fn try_bit(area: MemoryArea, index: usize, bit: u8) -> Result<Self, CoreError> {
        if bit > 7 {
            return Err(CoreError::InvalidDirectAddress(format!("{area:?}X{index}.{bit}")));
        }
        Self::bit(area, index, bit).validate()
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

    /// Creates a validated byte-addressed direct address, returning an error on overflow.
    pub fn try_byte(area: MemoryArea, index: usize) -> Result<Self, CoreError> {
        Self::byte(area, index).validate()
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

    /// Creates a validated 16-bit word-addressed direct address, returning an error on overflow.
    pub fn try_word(area: MemoryArea, index: usize) -> Result<Self, CoreError> {
        Self::word(area, index).validate()
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

    /// Creates a validated 32-bit double-word-addressed direct address, returning an error on overflow.
    pub fn try_dword(area: MemoryArea, index: usize) -> Result<Self, CoreError> {
        Self::dword(area, index).validate()
    }

    /// Creates a 64-bit long-word-addressed direct address (%IL0, %QL1, %ML10).
    pub const fn lword(area: MemoryArea, index: usize) -> Self {
        Self {
            area,
            width: AccessWidth::LWord,
            index,
            bit: 0,
        }
    }

    /// Creates a validated 64-bit long-word-addressed direct address, returning an error on overflow.
    pub fn try_lword(area: MemoryArea, index: usize) -> Result<Self, CoreError> {
        Self::lword(area, index).validate()
    }

    /// Returns the starting byte offset in the area's byte buffer, or `None` if
    /// `index * width_in_bytes` does not fit in `usize` (no wrap, no panic).
    pub fn checked_byte_offset(self) -> Option<usize> {
        match self.width {
            AccessWidth::Bit | AccessWidth::Byte => Some(self.index),
            AccessWidth::Word => self.index.checked_mul(2),
            AccessWidth::DWord => self.index.checked_mul(4),
            AccessWidth::LWord => self.index.checked_mul(8),
        }
    }

    /// Returns the starting byte offset in the area's byte buffer.
    ///
    /// Saturates to `usize::MAX` on overflow (never wraps and never panics), so any
    /// subsequent bounds check against a real buffer length fails. Prefer
    /// [`DirectAddress::checked_byte_offset`] when an explicit error is required.
    pub fn byte_offset(self) -> usize {
        self.checked_byte_offset().unwrap_or(usize::MAX)
    }

    /// Validates that this address' byte offset (and its end offset) are representable.
    pub fn validate(self) -> Result<Self, CoreError> {
        match self.checked_byte_offset() {
            Some(off) if off.checked_add(self.byte_len()).is_some() => Ok(self),
            _ => Err(CoreError::AddressOutOfBounds(self.to_string())),
        }
    }

    /// Returns the number of bytes spanned by this address.
    pub fn byte_len(self) -> usize {
        match self.width {
            AccessWidth::Bit | AccessWidth::Byte => 1,
            AccessWidth::Word => 2,
            AccessWidth::DWord => 4,
            AccessWidth::LWord => 8,
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
            'L' => AccessWidth::LWord,
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
                Self {
                    area,
                    width,
                    index,
                    bit,
                }
                .validate()
                .map_err(|_| CoreError::InvalidDirectAddress(s.to_string()))
            }
            AccessWidth::Byte | AccessWidth::Word | AccessWidth::DWord | AccessWidth::LWord => {
                let index = tail
                    .parse::<usize>()
                    .map_err(|_| CoreError::InvalidDirectAddress(s.to_string()))?;
                Self {
                    area,
                    width,
                    index,
                    bit: 0,
                }
                .validate()
                .map_err(|_| CoreError::InvalidDirectAddress(s.to_string()))
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
            AccessWidth::LWord => write!(f, "%{area_ch}L{}", self.index),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regression_i1_direct_address_overflow_rejected() {
        // Normal valid addresses
        assert!("%IW0".parse::<DirectAddress>().is_ok());
        assert!("%IX0.7".parse::<DirectAddress>().is_ok());
        assert!("%QD100".parse::<DirectAddress>().is_ok());

        // Bit out of range (> 7)
        assert!("%IX0.8".parse::<DirectAddress>().is_err());

        // Address overflow on parse must be rejected (no wrap, no panic)
        let overflow_word_str = format!("%IW{}", usize::MAX);
        assert!(overflow_word_str.parse::<DirectAddress>().is_err());

        let overflow_dword_str = format!("%ID{}", usize::MAX / 2);
        assert!(overflow_dword_str.parse::<DirectAddress>().is_err());

        // Validating constructors reject overflow
        assert!(DirectAddress::try_word(MemoryArea::Input, usize::MAX).is_err());
        assert!(DirectAddress::try_dword(MemoryArea::Input, usize::MAX / 2).is_err());
        assert!(DirectAddress::try_bit(MemoryArea::Input, 0, 8).is_err());

        // DirectAddress::byte_offset saturates instead of wrapping
        let unsafe_addr = DirectAddress::word(MemoryArea::Input, usize::MAX);
        assert_eq!(unsafe_addr.byte_offset(), usize::MAX);
        assert_eq!(unsafe_addr.checked_byte_offset(), None);
        assert!(unsafe_addr.validate().is_err());
    }

    #[test]
    fn test_lword_direct_address_parsing_and_properties() -> Result<(), Box<dyn std::error::Error>> {
        let il0: DirectAddress = "%IL0".parse()?;
        assert_eq!(il0.area, MemoryArea::Input);
        assert_eq!(il0.width, AccessWidth::LWord);
        assert_eq!(il0.index, 0);
        assert_eq!(il0.byte_len(), 8);
        assert_eq!(il0.checked_byte_offset(), Some(0));
        assert_eq!(il0.to_string(), "%IL0");

        let ql2: DirectAddress = "%QL2".parse()?;
        assert_eq!(ql2.area, MemoryArea::Output);
        assert_eq!(ql2.width, AccessWidth::LWord);
        assert_eq!(ql2.index, 2);
        assert_eq!(ql2.checked_byte_offset(), Some(16));
        assert_eq!(ql2.to_string(), "%QL2");

        let ml10: DirectAddress = "%ML10".parse()?;
        assert_eq!(ml10.area, MemoryArea::Marker);
        assert_eq!(ml10.width, AccessWidth::LWord);
        assert_eq!(ml10.checked_byte_offset(), Some(80));
        assert_eq!(ml10.to_string(), "%ML10");

        assert!(DirectAddress::try_lword(MemoryArea::Input, usize::MAX / 4).is_err());
        Ok(())
    }
}
