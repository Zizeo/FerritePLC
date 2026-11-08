use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ferrite_core::{AccessWidth, DirectAddress, IecType, IecValue, MemoryArea, ProcessImage};
use thiserror::Error;

pub mod rtu;
pub use rtu::*;

/// High-level selection of fieldbus transport engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransportEngine {
    Tcp,
    Rtu,
}

#[derive(Debug, Error)]
pub enum ModbusError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Frame too short ({got} bytes, min {min})")]
    FrameTooShort { got: usize, min: usize },

    #[error("Invalid Modbus protocol identifier: {0} (expected 0)")]
    InvalidProtocolId(u16),

    #[error("Illegal function code: 0x{0:02X}")]
    IllegalFunction(u8),

    #[error("Modbus exception response: code {0:?}")]
    Exception(ModbusException),

    #[error("Server not running")]
    NotRunning,

    #[error("Invalid data length: {0}")]
    InvalidDataLength(usize),

    #[error("Transaction ID mismatch (expected {expected}, got {got})")]
    TransactionIdMismatch { expected: u16, got: u16 },

    #[error("Connection poisoned due to previous I/O timeout or error")]
    ConnectionPoisoned,

    #[error("CRC mismatch: expected 0x{expected:04X}, got 0x{got:04X}")]
    CrcMismatch { expected: u16, got: u16 },

    #[error("Invalid or mismatched slave address: {0}")]
    InvalidSlaveAddress(u8),

    #[error("Serial port error: {0}")]
    Serial(String),

    #[error("Response timeout")]
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[repr(u8)]
pub enum ModbusException {
    #[error("Illegal function (0x01)")]
    IllegalFunction = 0x01,
    #[error("Illegal data address (0x02)")]
    IllegalDataAddress = 0x02,
    #[error("Illegal data value (0x03)")]
    IllegalDataValue = 0x03,
    #[error("Server device failure (0x04)")]
    ServerDeviceFailure = 0x04,
}

impl ModbusException {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(Self::IllegalFunction),
            0x02 => Some(Self::IllegalDataAddress),
            0x03 => Some(Self::IllegalDataValue),
            0x04 => Some(Self::ServerDeviceFailure),
            _ => None,
        }
    }
}

/// Modbus Application Protocol (MBAP) Header (7 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbapHeader {
    pub transaction_id: u16,
    pub protocol_id: u16,
    pub length: u16,
    pub unit_id: u8,
}

impl MbapHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, ModbusError> {
        if buf.len() < 7 {
            return Err(ModbusError::FrameTooShort {
                got: buf.len(),
                min: 7,
            });
        }
        let transaction_id = u16::from_be_bytes([buf[0], buf[1]]);
        let protocol_id = u16::from_be_bytes([buf[2], buf[3]]);
        if protocol_id != 0 {
            return Err(ModbusError::InvalidProtocolId(protocol_id));
        }
        let length = u16::from_be_bytes([buf[4], buf[5]]);
        if !(2..=256).contains(&length) {
            return Err(ModbusError::InvalidDataLength(length as usize));
        }
        let unit_id = buf[6];
        Ok(Self {
            transaction_id,
            protocol_id,
            length,
            unit_id,
        })
    }

    pub fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.transaction_id.to_be_bytes());
        out.extend_from_slice(&self.protocol_id.to_be_bytes());
        out.extend_from_slice(&self.length.to_be_bytes());
        out.push(self.unit_id);
    }
}

/// Supported Modbus standard request types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModbusRequest {
    /// FC 0x01: Read Coils
    ReadCoils { address: u16, count: u16 },
    /// FC 0x02: Read Discrete Inputs
    ReadDiscreteInputs { address: u16, count: u16 },
    /// FC 0x03: Read Holding Registers
    ReadHoldingRegisters { address: u16, count: u16 },
    /// FC 0x04: Read Input Registers
    ReadInputRegisters { address: u16, count: u16 },
    /// FC 0x05: Write Single Coil
    WriteSingleCoil { address: u16, value: bool },
    /// FC 0x06: Write Single Register
    WriteSingleRegister { address: u16, value: u16 },
    /// FC 0x0F (15): Write Multiple Coils
    WriteMultipleCoils { address: u16, values: Vec<bool> },
    /// FC 0x10 (16): Write Multiple Registers
    WriteMultipleRegisters { address: u16, values: Vec<u16> },
}

impl ModbusRequest {
    pub fn function_code(&self) -> u8 {
        match self {
            Self::ReadCoils { .. } => 0x01,
            Self::ReadDiscreteInputs { .. } => 0x02,
            Self::ReadHoldingRegisters { .. } => 0x03,
            Self::ReadInputRegisters { .. } => 0x04,
            Self::WriteSingleCoil { .. } => 0x05,
            Self::WriteSingleRegister { .. } => 0x06,
            Self::WriteMultipleCoils { .. } => 0x0F,
            Self::WriteMultipleRegisters { .. } => 0x10,
        }
    }

    pub fn parse(pdu: &[u8]) -> Result<Self, ModbusError> {
        if pdu.is_empty() {
            return Err(ModbusError::FrameTooShort { got: 0, min: 1 });
        }
        let fc = pdu[0];
        match fc {
            0x01 => {
                if pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let count = u16::from_be_bytes([pdu[3], pdu[4]]);
                Ok(Self::ReadCoils { address, count })
            }
            0x02 => {
                if pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let count = u16::from_be_bytes([pdu[3], pdu[4]]);
                Ok(Self::ReadDiscreteInputs { address, count })
            }
            0x03 => {
                if pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let count = u16::from_be_bytes([pdu[3], pdu[4]]);
                Ok(Self::ReadHoldingRegisters { address, count })
            }
            0x04 => {
                if pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let count = u16::from_be_bytes([pdu[3], pdu[4]]);
                Ok(Self::ReadInputRegisters { address, count })
            }
            0x05 => {
                if pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let raw_val = u16::from_be_bytes([pdu[3], pdu[4]]);
                let value = raw_val == 0xFF00;
                Ok(Self::WriteSingleCoil { address, value })
            }
            0x06 => {
                if pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let value = u16::from_be_bytes([pdu[3], pdu[4]]);
                Ok(Self::WriteSingleRegister { address, value })
            }
            0x0F => {
                if pdu.len() < 6 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 6,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let count = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
                let byte_count = pdu[5] as usize;
                let expected_bytes = count.div_ceil(8);
                if byte_count != expected_bytes || pdu.len() < 6 + byte_count {
                    return Err(ModbusError::InvalidDataLength(pdu.len()));
                }
                let mut values = Vec::with_capacity(count);
                for i in 0..count {
                    let byte_idx = 6 + (i / 8);
                    let bit_idx = i % 8;
                    let bit_val = (pdu[byte_idx] & (1 << bit_idx)) != 0;
                    values.push(bit_val);
                }
                Ok(Self::WriteMultipleCoils { address, values })
            }
            0x10 => {
                if pdu.len() < 6 {
                    return Err(ModbusError::FrameTooShort {
                        got: pdu.len(),
                        min: 6,
                    });
                }
                let address = u16::from_be_bytes([pdu[1], pdu[2]]);
                let count = u16::from_be_bytes([pdu[3], pdu[4]]) as usize;
                let byte_count = pdu[5] as usize;
                if byte_count != count * 2 || pdu.len() < 6 + byte_count {
                    return Err(ModbusError::InvalidDataLength(pdu.len()));
                }
                let mut values = Vec::with_capacity(count);
                for i in 0..count {
                    let offset = 6 + i * 2;
                    let reg = u16::from_be_bytes([pdu[offset], pdu[offset + 1]]);
                    values.push(reg);
                }
                Ok(Self::WriteMultipleRegisters { address, values })
            }
            unknown => Err(ModbusError::IllegalFunction(unknown)),
        }
    }

    pub fn encode_pdu(&self, out: &mut Vec<u8>) {
        out.push(self.function_code());
        match self {
            Self::ReadCoils { address, count }
            | Self::ReadDiscreteInputs { address, count }
            | Self::ReadHoldingRegisters { address, count }
            | Self::ReadInputRegisters { address, count } => {
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&count.to_be_bytes());
            }
            Self::WriteSingleCoil { address, value } => {
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(if *value { &[0xFF, 0x00] } else { &[0x00, 0x00] });
            }
            Self::WriteSingleRegister { address, value } => {
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::WriteMultipleCoils { address, values } => {
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&(values.len() as u16).to_be_bytes());
                let byte_count = values.len().div_ceil(8);
                out.push(byte_count as u8);
                let mut current_byte = 0u8;
                for (i, &b) in values.iter().enumerate() {
                    let bit = i % 8;
                    if b {
                        current_byte |= 1 << bit;
                    }
                    if bit == 7 || i + 1 == values.len() {
                        out.push(current_byte);
                        current_byte = 0;
                    }
                }
            }
            Self::WriteMultipleRegisters { address, values } => {
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&(values.len() as u16).to_be_bytes());
                out.push((values.len() * 2) as u8);
                for v in values {
                    out.extend_from_slice(&v.to_be_bytes());
                }
            }
        }
    }
}

/// Supported Modbus response types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModbusResponse {
    ReadBits(Vec<bool>),
    ReadRegisters(Vec<u16>),
    WriteSingleCoil { address: u16, value: bool },
    WriteSingleRegister { address: u16, value: u16 },
    WriteMultipleCoils { address: u16, count: u16 },
    WriteMultipleRegisters { address: u16, count: u16 },
    Exception { function: u8, code: ModbusException },
}

impl ModbusResponse {
    pub fn encode_pdu(&self, function: u8, out: &mut Vec<u8>) {
        match self {
            Self::ReadBits(bits) => {
                out.push(function);
                let byte_count = bits.len().div_ceil(8);
                out.push(byte_count as u8);
                let mut current_byte = 0u8;
                for (i, &b) in bits.iter().enumerate() {
                    let bit = i % 8;
                    if b {
                        current_byte |= 1 << bit;
                    }
                    if bit == 7 || i + 1 == bits.len() {
                        out.push(current_byte);
                        current_byte = 0;
                    }
                }
            }
            Self::ReadRegisters(regs) => {
                out.push(function);
                out.push((regs.len() * 2) as u8);
                for r in regs {
                    out.extend_from_slice(&r.to_be_bytes());
                }
            }
            Self::WriteSingleCoil { address, value } => {
                out.push(function);
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(if *value { &[0xFF, 0x00] } else { &[0x00, 0x00] });
            }
            Self::WriteSingleRegister { address, value } => {
                out.push(function);
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&value.to_be_bytes());
            }
            Self::WriteMultipleCoils { address, count } => {
                out.push(function);
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&count.to_be_bytes());
            }
            Self::WriteMultipleRegisters { address, count } => {
                out.push(function);
                out.extend_from_slice(&address.to_be_bytes());
                out.extend_from_slice(&count.to_be_bytes());
            }
            Self::Exception { function, code } => {
                out.push(function | 0x80);
                out.push(*code as u8);
            }
        }
    }

    /// Parse a raw Modbus response PDU into a typed `ModbusResponse`.
    pub fn parse_pdu(resp_pdu: &[u8]) -> Result<Self, ModbusError> {
        if resp_pdu.is_empty() {
            return Err(ModbusError::FrameTooShort { got: 0, min: 1 });
        }
        let resp_fc = resp_pdu[0];
        if (resp_fc & 0x80) != 0 {
            let exc_code = resp_pdu.get(1).copied().unwrap_or(4);
            let exc =
                ModbusException::from_u8(exc_code).unwrap_or(ModbusException::ServerDeviceFailure);
            return Err(ModbusError::Exception(exc));
        }

        match resp_fc {
            0x01 | 0x02 => {
                if resp_pdu.len() < 2 {
                    return Err(ModbusError::FrameTooShort {
                        got: resp_pdu.len(),
                        min: 2,
                    });
                }
                let byte_cnt = resp_pdu[1] as usize;
                if resp_pdu.len() < 2 + byte_cnt {
                    return Err(ModbusError::InvalidDataLength(resp_pdu.len()));
                }
                let mut bits = Vec::with_capacity(byte_cnt * 8);
                for b_idx in 0..byte_cnt {
                    let byte = resp_pdu[2 + b_idx];
                    for bit in 0..8 {
                        bits.push((byte & (1 << bit)) != 0);
                    }
                }
                Ok(Self::ReadBits(bits))
            }
            0x03 | 0x04 => {
                if resp_pdu.len() < 2 {
                    return Err(ModbusError::FrameTooShort {
                        got: resp_pdu.len(),
                        min: 2,
                    });
                }
                let byte_cnt = resp_pdu[1] as usize;
                let reg_cnt = byte_cnt / 2;
                if resp_pdu.len() < 2 + byte_cnt {
                    return Err(ModbusError::InvalidDataLength(resp_pdu.len()));
                }
                let mut regs = Vec::with_capacity(reg_cnt);
                for i in 0..reg_cnt {
                    let off = 2 + i * 2;
                    regs.push(u16::from_be_bytes([resp_pdu[off], resp_pdu[off + 1]]));
                }
                Ok(Self::ReadRegisters(regs))
            }
            0x05 => {
                if resp_pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: resp_pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([resp_pdu[1], resp_pdu[2]]);
                let val_code = u16::from_be_bytes([resp_pdu[3], resp_pdu[4]]);
                Ok(Self::WriteSingleCoil {
                    address,
                    value: val_code == 0xFF00,
                })
            }
            0x06 => {
                if resp_pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: resp_pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([resp_pdu[1], resp_pdu[2]]);
                let value = u16::from_be_bytes([resp_pdu[3], resp_pdu[4]]);
                Ok(Self::WriteSingleRegister { address, value })
            }
            0x0F => {
                if resp_pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: resp_pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([resp_pdu[1], resp_pdu[2]]);
                let count = u16::from_be_bytes([resp_pdu[3], resp_pdu[4]]);
                Ok(Self::WriteMultipleCoils { address, count })
            }
            0x10 => {
                if resp_pdu.len() < 5 {
                    return Err(ModbusError::FrameTooShort {
                        got: resp_pdu.len(),
                        min: 5,
                    });
                }
                let address = u16::from_be_bytes([resp_pdu[1], resp_pdu[2]]);
                let count = u16::from_be_bytes([resp_pdu[3], resp_pdu[4]]);
                Ok(Self::WriteMultipleRegisters { address, count })
            }
            unknown => Err(ModbusError::IllegalFunction(unknown)),
        }
    }
}

/// Standard mapping between Modbus address space and FerritePLC's `ProcessImage`:
/// - Coils (0xxxx):
///   - 0..999: `%QX0.0` .. `%QX124.7`
///   - 1000..1999: `%MX0.0` .. `%MX124.7`
/// - Discrete Inputs (1xxxx):
///   - 0..999: `%IX0.0` .. `%IX124.7`
/// - Holding Registers (4xxxx):
///   - 0..511: `%MW0` .. `%MW1022` (16-bit words)
///   - 1000..1511: `%QW0` .. `%QW1022` (16-bit words)
/// - Input Registers (3xxxx):
///   - 0..511: `%IW0` .. `%IW1022` (16-bit words)
pub struct ModbusProcessImageHandler;

impl ModbusProcessImageHandler {
    pub fn handle_request(
        pi: &mut ProcessImage,
        req: &ModbusRequest,
    ) -> Result<ModbusResponse, ModbusException> {
        match req {
            ModbusRequest::ReadCoils { address, count } => {
                let count = *count as usize;
                if count == 0 || count > 2000 {
                    return Err(ModbusException::IllegalDataValue);
                }
                if (*address as u32) + (count as u32) > 0x10000 {
                    return Err(ModbusException::IllegalDataAddress);
                }
                let mut bits = Vec::with_capacity(count);
                for i in 0..count {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    let val = Self::read_coil(pi, addr)?;
                    bits.push(val);
                }
                Ok(ModbusResponse::ReadBits(bits))
            }
            ModbusRequest::ReadDiscreteInputs { address, count } => {
                let count = *count as usize;
                if count == 0 || count > 2000 {
                    return Err(ModbusException::IllegalDataValue);
                }
                if (*address as u32) + (count as u32) > 0x10000 {
                    return Err(ModbusException::IllegalDataAddress);
                }
                let mut bits = Vec::with_capacity(count);
                for i in 0..count {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    let val = Self::read_discrete_input(pi, addr)?;
                    bits.push(val);
                }
                Ok(ModbusResponse::ReadBits(bits))
            }
            ModbusRequest::ReadHoldingRegisters { address, count } => {
                let count = *count as usize;
                if count == 0 || count > 125 {
                    return Err(ModbusException::IllegalDataValue);
                }
                if (*address as u32) + (count as u32) > 0x10000 {
                    return Err(ModbusException::IllegalDataAddress);
                }
                let mut regs = Vec::with_capacity(count);
                for i in 0..count {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    let val = Self::read_holding_register(pi, addr)?;
                    regs.push(val);
                }
                Ok(ModbusResponse::ReadRegisters(regs))
            }
            ModbusRequest::ReadInputRegisters { address, count } => {
                let count = *count as usize;
                if count == 0 || count > 125 {
                    return Err(ModbusException::IllegalDataValue);
                }
                if (*address as u32) + (count as u32) > 0x10000 {
                    return Err(ModbusException::IllegalDataAddress);
                }
                let mut regs = Vec::with_capacity(count);
                for i in 0..count {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    let val = Self::read_input_register(pi, addr)?;
                    regs.push(val);
                }
                Ok(ModbusResponse::ReadRegisters(regs))
            }
            ModbusRequest::WriteSingleCoil { address, value } => {
                Self::write_coil(pi, *address, *value)?;
                Ok(ModbusResponse::WriteSingleCoil {
                    address: *address,
                    value: *value,
                })
            }
            ModbusRequest::WriteSingleRegister { address, value } => {
                Self::write_holding_register(pi, *address, *value)?;
                Ok(ModbusResponse::WriteSingleRegister {
                    address: *address,
                    value: *value,
                })
            }
            ModbusRequest::WriteMultipleCoils { address, values } => {
                if values.is_empty() || values.len() > 1968 {
                    return Err(ModbusException::IllegalDataValue);
                }
                if (*address as u32) + (values.len() as u32) > 0x10000 {
                    return Err(ModbusException::IllegalDataAddress);
                }
                // Pre-validate all target addresses to avoid partial mutations
                for i in 0..values.len() {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    if addr >= 2000 {
                        return Err(ModbusException::IllegalDataAddress);
                    }
                }
                for (i, &val) in values.iter().enumerate() {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    Self::write_coil(pi, addr, val)?;
                }
                Ok(ModbusResponse::WriteMultipleCoils {
                    address: *address,
                    count: values.len() as u16,
                })
            }
            ModbusRequest::WriteMultipleRegisters { address, values } => {
                if values.is_empty() || values.len() > 123 {
                    return Err(ModbusException::IllegalDataValue);
                }
                if (*address as u32) + (values.len() as u32) > 0x10000 {
                    return Err(ModbusException::IllegalDataAddress);
                }
                // Pre-validate all target addresses to avoid partial mutations
                for i in 0..values.len() {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    if !((addr < 512) || (1000..1512).contains(&addr)) {
                        return Err(ModbusException::IllegalDataAddress);
                    }
                }
                for (i, &val) in values.iter().enumerate() {
                    let addr = address
                        .checked_add(i as u16)
                        .ok_or(ModbusException::IllegalDataAddress)?;
                    Self::write_holding_register(pi, addr, val)?;
                }
                Ok(ModbusResponse::WriteMultipleRegisters {
                    address: *address,
                    count: values.len() as u16,
                })
            }
        }
    }

    fn read_coil(pi: &ProcessImage, addr: u16) -> Result<bool, ModbusException> {
        let direct_addr = if addr < 1000 {
            DirectAddress {
                area: MemoryArea::Output,
                width: AccessWidth::Bit,
                index: (addr / 8) as usize,
                bit: (addr % 8) as u8,
            }
        } else if addr < 2000 {
            let offset = addr - 1000;
            DirectAddress {
                area: MemoryArea::Marker,
                width: AccessWidth::Bit,
                index: (offset / 8) as usize,
                bit: (offset % 8) as u8,
            }
        } else {
            return Err(ModbusException::IllegalDataAddress);
        };
        pi.read(direct_addr, IecType::Bool)
            .map(|v| v.as_bool().unwrap_or(false))
            .map_err(|_| ModbusException::IllegalDataAddress)
    }

    fn write_coil(pi: &mut ProcessImage, addr: u16, val: bool) -> Result<(), ModbusException> {
        let direct_addr = if addr < 1000 {
            DirectAddress {
                area: MemoryArea::Output,
                width: AccessWidth::Bit,
                index: (addr / 8) as usize,
                bit: (addr % 8) as u8,
            }
        } else if addr < 2000 {
            let offset = addr - 1000;
            DirectAddress {
                area: MemoryArea::Marker,
                width: AccessWidth::Bit,
                index: (offset / 8) as usize,
                bit: (offset % 8) as u8,
            }
        } else {
            return Err(ModbusException::IllegalDataAddress);
        };
        pi.write(direct_addr, &IecValue::Bool(val))
            .map_err(|_| ModbusException::IllegalDataAddress)
    }

    fn read_discrete_input(pi: &ProcessImage, addr: u16) -> Result<bool, ModbusException> {
        if addr >= 1000 {
            return Err(ModbusException::IllegalDataAddress);
        }
        let direct_addr = DirectAddress {
            area: MemoryArea::Input,
            width: AccessWidth::Bit,
            index: (addr / 8) as usize,
            bit: (addr % 8) as u8,
        };
        pi.read(direct_addr, IecType::Bool)
            .map(|v| v.as_bool().unwrap_or(false))
            .map_err(|_| ModbusException::IllegalDataAddress)
    }

    fn read_input_register(pi: &ProcessImage, addr: u16) -> Result<u16, ModbusException> {
        if addr >= 512 {
            return Err(ModbusException::IllegalDataAddress);
        }
        let direct_addr = DirectAddress {
            area: MemoryArea::Input,
            width: AccessWidth::Word,
            index: addr as usize,
            bit: 0,
        };
        pi.read(direct_addr, IecType::Int)
            .map(|v| v.as_int().unwrap_or(0) as u16)
            .map_err(|_| ModbusException::IllegalDataAddress)
    }

    fn read_holding_register(pi: &ProcessImage, addr: u16) -> Result<u16, ModbusException> {
        let direct_addr = if addr < 512 {
            DirectAddress {
                area: MemoryArea::Marker,
                width: AccessWidth::Word,
                index: addr as usize,
                bit: 0,
            }
        } else if (1000..1512).contains(&addr) {
            let offset = addr - 1000;
            DirectAddress {
                area: MemoryArea::Output,
                width: AccessWidth::Word,
                index: offset as usize,
                bit: 0,
            }
        } else {
            return Err(ModbusException::IllegalDataAddress);
        };
        pi.read(direct_addr, IecType::Int)
            .map(|v| v.as_int().unwrap_or(0) as u16)
            .map_err(|_| ModbusException::IllegalDataAddress)
    }

    fn write_holding_register(
        pi: &mut ProcessImage,
        addr: u16,
        val: u16,
    ) -> Result<(), ModbusException> {
        let direct_addr = if addr < 512 {
            DirectAddress {
                area: MemoryArea::Marker,
                width: AccessWidth::Word,
                index: addr as usize,
                bit: 0,
            }
        } else if (1000..1512).contains(&addr) {
            let offset = addr - 1000;
            DirectAddress {
                area: MemoryArea::Output,
                width: AccessWidth::Word,
                index: offset as usize,
                bit: 0,
            }
        } else {
            return Err(ModbusException::IllegalDataAddress);
        };
        pi.write(direct_addr, &IecValue::Int(val as i16))
            .map_err(|_| ModbusException::IllegalDataAddress)
    }
}

/// Statistics and telemetry for the live Modbus TCP Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModbusServerTelemetry {
    pub requests_total: u64,
    pub exceptions_total: u64,
    pub active_clients: usize,
    pub is_running: bool,
}

struct ConnectedClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

/// Dedicated, deterministic Modbus TCP Server running in a background worker thread.
pub struct ModbusTcpServer {
    bind_addr: SocketAddr,
    running: Arc<AtomicBool>,
    requests_count: Arc<AtomicU64>,
    exceptions_count: Arc<AtomicU64>,
    active_clients: Arc<AtomicUsize>,
    worker_handle: Option<JoinHandle<()>>,
}

impl ModbusTcpServer {
    pub fn start(
        bind_addr: SocketAddr,
        shared_pi: Arc<RwLock<ProcessImage>>,
    ) -> Result<Self, ModbusError> {
        let listener = TcpListener::bind(bind_addr)?;
        let actual_addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;

        let running = Arc::new(AtomicBool::new(true));
        let requests_count = Arc::new(AtomicU64::new(0));
        let exceptions_count = Arc::new(AtomicU64::new(0));
        let active_clients = Arc::new(AtomicUsize::new(0));

        let r_flag = Arc::clone(&running);
        let req_cnt = Arc::clone(&requests_count);
        let exc_cnt = Arc::clone(&exceptions_count);
        let act_cnt = Arc::clone(&active_clients);

        let worker_handle = thread::spawn(move || {
            const MAX_CLIENTS: usize = 64;
            let mut clients: Vec<ConnectedClient> = Vec::new();
            let mut scratch = [0u8; 1024];

            while r_flag.load(Ordering::Relaxed) {

                // Non-blocking accept of new connections
                match listener.accept() {
                    Ok((stream, _)) => {
                        if clients.len() < MAX_CLIENTS {
                            let _ = stream.set_nonblocking(true);
                            let _ = stream.set_nodelay(true);
                            clients.push(ConnectedClient {
                                stream,
                                buf: Vec::with_capacity(512),
                            });
                            act_cnt.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => {}
                }

                let mut to_remove = Vec::new();
                for (idx, client) in clients.iter_mut().enumerate() {
                    match client.stream.read(&mut scratch) {
                        Ok(0) => {
                            // Client disconnected
                            to_remove.push(idx);
                            continue;
                        }
                        Ok(n) => {
                            client.buf.extend_from_slice(&scratch[..n]);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(_) => {
                            to_remove.push(idx);
                            continue;
                        }
                    }

                    // Process all complete MBAP frames in client.buf
                    while client.buf.len() >= 7 {
                        match MbapHeader::parse(&client.buf[..7]) {
                            Ok(header) => {
                                // Protocol ID must be 0 for Modbus TCP
                                if header.protocol_id != 0 {
                                    to_remove.push(idx);
                                    break;
                                }
                                let frame_len = 6 + (header.length as usize);
                                // Modbus TCP ADU max length is 260 bytes; length must be >= 2 (unit_id + FC)
                                if header.length < 2 || frame_len > 260 {
                                    to_remove.push(idx);
                                    break;
                                }
                                if client.buf.len() < frame_len {
                                    // Incomplete frame, wait for remainder
                                    break;
                                }

                                // Valid complete frame received
                                req_cnt.fetch_add(1, Ordering::Relaxed);

                                let pdu_len = (header.length as usize).saturating_sub(1);
                                let pdu = client.buf[7..7 + pdu_len].to_vec();
                                let req_fc = pdu.first().copied().unwrap_or(0);

                                let resp = match ModbusRequest::parse(&pdu) {
                                    Ok(req) => {
                                        if let Ok(mut pi) = shared_pi.write() {
                                            match ModbusProcessImageHandler::handle_request(
                                                &mut pi, &req,
                                            ) {
                                                Ok(r) => r,
                                                Err(exc) => {
                                                    exc_cnt.fetch_add(1, Ordering::Relaxed);
                                                    ModbusResponse::Exception {
                                                        function: req.function_code(),
                                                        code: exc,
                                                    }
                                                }
                                            }
                                        } else {
                                            ModbusResponse::Exception {
                                                function: req.function_code(),
                                                code: ModbusException::ServerDeviceFailure,
                                            }
                                        }
                                    }
                                    Err(ModbusError::IllegalFunction(fc)) => {
                                        exc_cnt.fetch_add(1, Ordering::Relaxed);
                                        ModbusResponse::Exception {
                                            function: fc,
                                            code: ModbusException::IllegalFunction,
                                        }
                                    }
                                    Err(_) => {
                                        exc_cnt.fetch_add(1, Ordering::Relaxed);
                                        ModbusResponse::Exception {
                                            function: req_fc,
                                            code: ModbusException::IllegalDataValue,
                                        }
                                    }
                                };

                                let mut resp_pdu = Vec::with_capacity(256);
                                resp.encode_pdu(req_fc, &mut resp_pdu);

                                let resp_header = MbapHeader {
                                    transaction_id: header.transaction_id,
                                    protocol_id: 0,
                                    length: (resp_pdu.len() + 1) as u16,
                                    unit_id: header.unit_id,
                                };

                                let mut out_frame = Vec::with_capacity(7 + resp_pdu.len());
                                resp_header.write_bytes(&mut out_frame);
                                out_frame.extend_from_slice(&resp_pdu);

                                if client.stream.write_all(&out_frame).is_err()
                                    || client.stream.flush().is_err()
                                {
                                    to_remove.push(idx);
                                    break;
                                }

                                client.buf.drain(..frame_len);
                            }
                            Err(_) => {
                                to_remove.push(idx);
                                break;
                            }
                        }
                    }

                    // Reject accumulator overflow from junk data (> 260 bytes without a valid frame)
                    if client.buf.len() > 260 {
                        to_remove.push(idx);
                    }
                }

                if !to_remove.is_empty() {
                    to_remove.sort_unstable();
                    to_remove.dedup();
                    for &idx in to_remove.iter().rev() {
                        if idx < clients.len() {
                            clients.swap_remove(idx);
                            act_cnt.fetch_sub(1, Ordering::Relaxed);
                        }
                    }
                }

                thread::sleep(Duration::from_millis(5));
            }
        });

        Ok(Self {
            bind_addr: actual_addr,
            running,
            requests_count,
            exceptions_count,
            active_clients,
            worker_handle: Some(worker_handle),
        })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn telemetry(&self) -> ModbusServerTelemetry {
        ModbusServerTelemetry {
            requests_total: self.requests_count.load(Ordering::Relaxed),
            exceptions_total: self.exceptions_count.load(Ordering::Relaxed),
            active_clients: self.active_clients.load(Ordering::Relaxed),
            is_running: self.running.load(Ordering::Relaxed),
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.worker_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for ModbusTcpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Modbus TCP Client for remote I/O scanning and fieldbus polling.
pub struct ModbusTcpClient {
    stream: TcpStream,
    transaction_id: u16,
    poisoned: bool,
}

impl ModbusTcpClient {
    pub fn connect(addr: SocketAddr) -> Result<Self, ModbusError> {
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        Ok(Self {
            stream,
            transaction_id: 1,
            poisoned: false,
        })
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn send_request(
        &mut self,
        unit_id: u8,
        req: &ModbusRequest,
    ) -> Result<ModbusResponse, ModbusError> {
        if self.poisoned {
            return Err(ModbusError::ConnectionPoisoned);
        }

        let expected_tid = self.transaction_id;
        let mut pdu = Vec::new();
        req.encode_pdu(&mut pdu);

        let header = MbapHeader {
            transaction_id: expected_tid,
            protocol_id: 0,
            length: (pdu.len() + 1) as u16,
            unit_id,
        };
        self.transaction_id = self.transaction_id.wrapping_add(1);

        let mut out = Vec::with_capacity(7 + pdu.len());
        header.write_bytes(&mut out);
        out.extend_from_slice(&pdu);

        if let Err(e) = self.stream.write_all(&out) {
            self.poisoned = true;
            return Err(ModbusError::Io(e));
        }
        if let Err(e) = self.stream.flush() {
            self.poisoned = true;
            return Err(ModbusError::Io(e));
        }

        let mut resp_header_buf = [0u8; 7];
        if let Err(e) = self.stream.read_exact(&mut resp_header_buf) {
            self.poisoned = true;
            return Err(ModbusError::Io(e));
        }
        let resp_header = match MbapHeader::parse(&resp_header_buf) {
            Ok(h) => h,
            Err(e) => {
                self.poisoned = true;
                return Err(e);
            }
        };

        if resp_header.transaction_id != expected_tid {
            self.poisoned = true;
            return Err(ModbusError::TransactionIdMismatch {
                expected: expected_tid,
                got: resp_header.transaction_id,
            });
        }

        let resp_pdu_len = (resp_header.length as usize).saturating_sub(1);
        let mut resp_pdu = vec![0u8; resp_pdu_len];
        if let Err(e) = self.stream.read_exact(&mut resp_pdu) {
            self.poisoned = true;
            return Err(ModbusError::Io(e));
        }

        ModbusResponse::parse_pdu(&resp_pdu).map_err(|e| {
            // Exceptions from the remote server are protocol responses, not link corruption
            if !matches!(e, ModbusError::Exception(_)) {
                self.poisoned = true;
            }
            e
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbap_header_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let header = MbapHeader {
            transaction_id: 1234,
            protocol_id: 0,
            length: 6,
            unit_id: 1,
        };
        let mut buf = Vec::new();
        header.write_bytes(&mut buf);
        let parsed = MbapHeader::parse(&buf)?;
        assert_eq!(header, parsed);
        Ok(())
    }

    #[test]
    fn test_mbap_header_invalid_length() {
        let zero_len = [0, 1, 0, 0, 0, 0, 1];
        assert!(matches!(
            MbapHeader::parse(&zero_len),
            Err(ModbusError::InvalidDataLength(0))
        ));

        let one_len = [0, 1, 0, 0, 0, 1, 1];
        assert!(matches!(
            MbapHeader::parse(&one_len),
            Err(ModbusError::InvalidDataLength(1))
        ));

        let huge_len = [0, 1, 0, 0, 1, 1, 1]; // length = 257
        assert!(matches!(
            MbapHeader::parse(&huge_len),
            Err(ModbusError::InvalidDataLength(257))
        ));
    }

    #[test]
    fn test_request_pdu_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let req = ModbusRequest::ReadHoldingRegisters {
            address: 10,
            count: 4,
        };
        let mut pdu = Vec::new();
        req.encode_pdu(&mut pdu);
        let parsed = ModbusRequest::parse(&pdu)?;
        assert_eq!(req, parsed);

        let req_coil = ModbusRequest::WriteSingleCoil {
            address: 5,
            value: true,
        };
        pdu.clear();
        req_coil.encode_pdu(&mut pdu);
        let parsed_coil = ModbusRequest::parse(&pdu)?;
        assert_eq!(req_coil, parsed_coil);
        Ok(())
    }

    #[test]
    fn test_process_image_modbus_handler() -> Result<(), Box<dyn std::error::Error>> {
        let mut pi = ProcessImage::default();
        // Write single coil at address 0 (%QX0.0)
        let resp = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::WriteSingleCoil {
                address: 0,
                value: true,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            resp,
            ModbusResponse::WriteSingleCoil {
                address: 0,
                value: true
            }
        );

        // Read coils back
        let read_resp = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::ReadCoils {
                address: 0,
                count: 8,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        if let ModbusResponse::ReadBits(bits) = read_resp {
            assert!(bits[0]);
            assert!(!bits[1]);
        } else {
            return Err("unexpected response".into());
        }

        // Write holding register at 0 (%MW0)
        ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::WriteSingleRegister {
                address: 0,
                value: 4200,
            },
        )
        .map_err(|e| format!("{e:?}"))?;

        let read_reg_resp = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::ReadHoldingRegisters {
                address: 0,
                count: 1,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        if let ModbusResponse::ReadRegisters(regs) = read_reg_resp {
            assert_eq!(regs[0], 4200);
        } else {
            return Err("unexpected response".into());
        }
        Ok(())
    }

    #[test]
    fn test_modbus_tcp_client_server_integration() -> Result<(), Box<dyn std::error::Error>> {
        let pi = Arc::new(RwLock::new(ProcessImage::default()));
        let bind_addr: SocketAddr = "127.0.0.1:5029".parse()?;
        let mut server = ModbusTcpServer::start(bind_addr, Arc::clone(&pi))?;

        thread::sleep(Duration::from_millis(50));

        let mut client = ModbusTcpClient::connect(bind_addr)?;
        let write_resp = client.send_request(
            1,
            &ModbusRequest::WriteSingleCoil {
                address: 2,
                value: true,
            },
        )?;
        assert_eq!(
            write_resp,
            ModbusResponse::WriteSingleCoil {
                address: 2,
                value: true
            }
        );

        let read_resp = client.send_request(
            1,
            &ModbusRequest::ReadCoils {
                address: 0,
                count: 8,
            },
        )?;
        if let ModbusResponse::ReadBits(bits) = read_resp {
            assert!(!bits[0]);
            assert!(!bits[1]);
            assert!(bits[2]);
        } else {
            return Err("expected bits response".into());
        }

        let telem = server.telemetry();
        assert!(telem.requests_total >= 2);
        server.stop();
        Ok(())
    }

    #[test]
    fn test_modbus_address_overflow_guard_cr1() -> Result<(), Box<dyn std::error::Error>> {
        let mut pi = ProcessImage::default();

        // CR-1: WriteMultipleCoils with address 0xFFF0 and 16 values should error with IllegalDataAddress
        let err = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::WriteMultipleCoils {
                address: 0xFFF0,
                values: vec![true; 16],
            },
        );
        assert_eq!(err, Err(ModbusException::IllegalDataAddress));

        // Ensure process image output and memory coils are untouched (%QX0.0 is false)
        let q0 = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::ReadCoils {
                address: 0,
                count: 1,
            },
        )?;
        assert_eq!(q0, ModbusResponse::ReadBits(vec![false]));

        // Address overflow (address + count > 0x10000) for ReadCoils
        let err_read = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::ReadCoils {
                address: 0xFFF0,
                count: 20,
            },
        );
        assert_eq!(err_read, Err(ModbusException::IllegalDataAddress));

        // Address overflow for ReadHoldingRegisters
        let err_hr = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::ReadHoldingRegisters {
                address: 0xFFF0,
                count: 20,
            },
        );
        assert_eq!(err_hr, Err(ModbusException::IllegalDataAddress));

        // Address overflow for WriteMultipleRegisters
        let err_wmr = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::WriteMultipleRegisters {
                address: 0xFFF0,
                values: vec![42; 20],
            },
        );
        assert_eq!(err_wmr, Err(ModbusException::IllegalDataAddress));

        // All-or-nothing check: invalid coil address in range does not partially mutate
        let err_partial = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::WriteMultipleCoils {
                address: 1999,
                values: vec![true, true],
            },
        );
        assert_eq!(err_partial, Err(ModbusException::IllegalDataAddress));
        let m_coil = ModbusProcessImageHandler::handle_request(
            &mut pi,
            &ModbusRequest::ReadCoils {
                address: 1999,
                count: 1,
            },
        )?;
        assert_eq!(m_coil, ModbusResponse::ReadBits(vec![false]));

        Ok(())
    }

    #[test]
    fn test_modbus_server_frame_accumulator_coalesced_and_fragmented_i29(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pi = Arc::new(RwLock::new(ProcessImage::default()));
        let mut server = ModbusTcpServer::start("127.0.0.1:0".parse()?, Arc::clone(&pi))?;
        let addr = server.bind_addr();

        thread::sleep(Duration::from_millis(30));

        let mut stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;

        // 1. Coalesced requests in a single TCP write
        let req1 = ModbusRequest::ReadHoldingRegisters {
            address: 0,
            count: 1,
        };
        let req2 = ModbusRequest::ReadHoldingRegisters {
            address: 0,
            count: 1,
        };
        let mut pdu1 = Vec::new();
        req1.encode_pdu(&mut pdu1);
        let mut pdu2 = Vec::new();
        req2.encode_pdu(&mut pdu2);

        let h1 = MbapHeader {
            transaction_id: 100,
            protocol_id: 0,
            length: (pdu1.len() + 1) as u16,
            unit_id: 1,
        };
        let h2 = MbapHeader {
            transaction_id: 101,
            protocol_id: 0,
            length: (pdu2.len() + 1) as u16,
            unit_id: 1,
        };

        let mut coalesced = Vec::new();
        h1.write_bytes(&mut coalesced);
        coalesced.extend_from_slice(&pdu1);
        h2.write_bytes(&mut coalesced);
        coalesced.extend_from_slice(&pdu2);

        stream.write_all(&coalesced)?;
        stream.flush()?;

        // Expect two responses back-to-back
        let mut resp_hdr1 = [0u8; 7];
        stream.read_exact(&mut resp_hdr1)?;
        let parsed_h1 = MbapHeader::parse(&resp_hdr1)?;
        assert_eq!(parsed_h1.transaction_id, 100);
        let mut pdu_buf1 = vec![0u8; (parsed_h1.length as usize).saturating_sub(1)];
        stream.read_exact(&mut pdu_buf1)?;

        let mut resp_hdr2 = [0u8; 7];
        stream.read_exact(&mut resp_hdr2)?;
        let parsed_h2 = MbapHeader::parse(&resp_hdr2)?;
        assert_eq!(parsed_h2.transaction_id, 101);
        let mut pdu_buf2 = vec![0u8; (parsed_h2.length as usize).saturating_sub(1)];
        stream.read_exact(&mut pdu_buf2)?;

        // 2. Fragmented request split into two writes
        let req3 = ModbusRequest::ReadHoldingRegisters {
            address: 0,
            count: 1,
        };
        let mut pdu3 = Vec::new();
        req3.encode_pdu(&mut pdu3);
        let h3 = MbapHeader {
            transaction_id: 102,
            protocol_id: 0,
            length: (pdu3.len() + 1) as u16,
            unit_id: 1,
        };
        let mut frame3 = Vec::new();
        h3.write_bytes(&mut frame3);
        frame3.extend_from_slice(&pdu3);

        // Send first 4 bytes
        stream.write_all(&frame3[..4])?;
        stream.flush()?;
        thread::sleep(Duration::from_millis(20));
        // Send remainder
        stream.write_all(&frame3[4..])?;
        stream.flush()?;

        let mut resp_hdr3 = [0u8; 7];
        stream.read_exact(&mut resp_hdr3)?;
        let parsed_h3 = MbapHeader::parse(&resp_hdr3)?;
        assert_eq!(parsed_h3.transaction_id, 102);
        let mut pdu_buf3 = vec![0u8; (parsed_h3.length as usize).saturating_sub(1)];
        stream.read_exact(&mut pdu_buf3)?;

        server.stop();
        Ok(())
    }

    #[test]
    fn test_modbus_client_transaction_id_mismatch_and_poisoning_p1_p2(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        let handle = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut buf = [0u8; 12];
            stream.read_exact(&mut buf)?;
            // Reply with mismatched transaction_id = 999 instead of expected 1
            let resp_h = MbapHeader {
                transaction_id: 999,
                protocol_id: 0,
                length: 3,
                unit_id: 1,
            };
            let mut out = Vec::new();
            resp_h.write_bytes(&mut out);
            out.extend_from_slice(&[0x03, 0x02, 0x00, 0x00]); // FC 03 response
            stream.write_all(&out)?;
            stream.flush()?;
            Ok(())
        });

        let mut client = ModbusTcpClient::connect(addr)?;
        let res = client.send_request(
            1,
            &ModbusRequest::ReadHoldingRegisters {
                address: 0,
                count: 1,
            },
        );
        match res {
            Err(ModbusError::TransactionIdMismatch { expected, got }) => {
                assert_eq!(expected, 1);
                assert_eq!(got, 999);
            }
            other => panic!("expected TransactionIdMismatch, got {other:?}"),
        }
        assert!(client.is_poisoned());

        // Subsequent call on poisoned client fails immediately with ConnectionPoisoned
        let res_poisoned = client.send_request(
            1,
            &ModbusRequest::ReadHoldingRegisters {
                address: 0,
                count: 1,
            },
        );
        match res_poisoned {
            Err(ModbusError::ConnectionPoisoned) => {}
            other => panic!("expected ConnectionPoisoned, got {other:?}"),
        }

        let _ = handle.join();
        Ok(())
    }

    #[test]
    fn test_modbus_server_junk_traffic_not_counted_p3() -> Result<(), Box<dyn std::error::Error>> {
        let pi = Arc::new(RwLock::new(ProcessImage::default()));
        let mut server = ModbusTcpServer::start("127.0.0.1:0".parse()?, Arc::clone(&pi))?;
        let addr = server.bind_addr();

        thread::sleep(Duration::from_millis(30));

        let mut stream = TcpStream::connect(addr)?;
        // Send 10 bytes of junk data (not valid Modbus MBAP)
        stream.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05])?;
        stream.flush()?;

        thread::sleep(Duration::from_millis(50));

        let telem = server.telemetry();
        assert_eq!(telem.requests_total, 0);

        server.stop();
        Ok(())
    }
}
