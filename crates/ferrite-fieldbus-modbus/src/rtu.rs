use std::io::{Read, Write};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use ferrite_core::ProcessImage;
use serde::{Deserialize, Serialize};

use crate::{
    ModbusError, ModbusException, ModbusProcessImageHandler, ModbusRequest, ModbusResponse,
};

/// Maximum length of a Modbus RTU Application Data Unit (ADU) in bytes.
pub const RTU_MAX_ADU_LEN: usize = 256;

/// Minimum length of a Modbus RTU ADU in bytes:
/// 1 byte slave address + 1 byte function code + 2 bytes CRC16.
pub const RTU_MIN_ADU_LEN: usize = 4;

/// Serial data bits configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SerialDataBits {
    Five,
    Six,
    Seven,
    #[default]
    Eight,
}

impl From<SerialDataBits> for serialport::DataBits {
    fn from(d: SerialDataBits) -> Self {
        match d {
            SerialDataBits::Five => serialport::DataBits::Five,
            SerialDataBits::Six => serialport::DataBits::Six,
            SerialDataBits::Seven => serialport::DataBits::Seven,
            SerialDataBits::Eight => serialport::DataBits::Eight,
        }
    }
}

/// Serial parity bit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SerialParity {
    #[default]
    None,
    Odd,
    Even,
}

impl From<SerialParity> for serialport::Parity {
    fn from(p: SerialParity) -> Self {
        match p {
            SerialParity::None => serialport::Parity::None,
            SerialParity::Odd => serialport::Parity::Odd,
            SerialParity::Even => serialport::Parity::Even,
        }
    }
}

/// Serial stop bits configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SerialStopBits {
    #[default]
    One,
    Two,
}

impl From<SerialStopBits> for serialport::StopBits {
    fn from(s: SerialStopBits) -> Self {
        match s {
            SerialStopBits::One => serialport::StopBits::One,
            SerialStopBits::Two => serialport::StopBits::Two,
        }
    }
}

/// Serial flow control configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SerialFlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

impl From<SerialFlowControl> for serialport::FlowControl {
    fn from(f: SerialFlowControl) -> Self {
        match f {
            SerialFlowControl::None => serialport::FlowControl::None,
            SerialFlowControl::Software => serialport::FlowControl::Software,
            SerialFlowControl::Hardware => serialport::FlowControl::Hardware,
        }
    }
}

/// Configuration parameters for Modbus RTU serial ports (RS-485 / RS-232).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialPortConfig {
    pub path: String,
    pub baud_rate: u32,
    pub data_bits: SerialDataBits,
    pub parity: SerialParity,
    pub stop_bits: SerialStopBits,
    pub flow_control: SerialFlowControl,
    pub timeout_ms: u64,
}

impl Default for SerialPortConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            baud_rate: 19200,
            data_bits: SerialDataBits::Eight,
            parity: SerialParity::None,
            stop_bits: SerialStopBits::One,
            flow_control: SerialFlowControl::None,
            timeout_ms: 1000,
        }
    }
}

impl SerialPortConfig {
    pub fn new(path: impl Into<String>, baud_rate: u32) -> Self {
        Self {
            path: path.into(),
            baud_rate,
            ..Default::default()
        }
    }

    pub fn with_parity(mut self, parity: SerialParity) -> Self {
        self.parity = parity;
        self
    }

    pub fn with_stop_bits(mut self, stop_bits: SerialStopBits) -> Self {
        self.stop_bits = stop_bits;
        self
    }

    pub fn with_data_bits(mut self, data_bits: SerialDataBits) -> Self {
        self.data_bits = data_bits;
        self
    }

    pub fn with_flow_control(mut self, flow_control: SerialFlowControl) -> Self {
        self.flow_control = flow_control;
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Open and configure the native serial port.
    pub fn open(&self) -> Result<Box<dyn serialport::SerialPort>, ModbusError> {
        serialport::new(&self.path, self.baud_rate)
            .data_bits(self.data_bits.into())
            .parity(self.parity.into())
            .stop_bits(self.stop_bits.into())
            .flow_control(self.flow_control.into())
            .timeout(Duration::from_millis(self.timeout_ms))
            .open()
            .map_err(|e| ModbusError::Serial(e.to_string()))
    }
}

// -----------------------------------------------------------------------------
// CRC-16 (Modbus Polynomial 0xA001)
// -----------------------------------------------------------------------------

const fn make_crc16_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u16;
        let mut j = 0;
        while j < 8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Precomputed compile-time lookup table for Modbus CRC-16 (polynomial 0xA001).
pub const CRC16_TABLE: [u16; 256] = make_crc16_table();

/// Calculate Modbus RTU 16-bit CRC using the precomputed lookup table.
///
/// Modbus uses initial value 0xFFFF and polynomial 0xA001 (reversed representation of 0x8005).
#[inline]
pub fn calculate_crc(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        let table_idx = ((crc ^ (byte as u16)) & 0x00FF) as usize;
        crc = (crc >> 8) ^ CRC16_TABLE[table_idx];
    }
    crc
}

/// Bitwise reference implementation of Modbus CRC-16 (for validation and verification).
pub fn calculate_crc_bitwise(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= byte as u16;
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

// -----------------------------------------------------------------------------
// Inter-frame gap timing helpers (T3.5 / T1.5)
// -----------------------------------------------------------------------------

/// Calculate T3.5 inter-frame gap duration per Modbus Serial Line spec.
///
/// - For baud rates > 19200 bps: fixed at 1750 µs (1.75 ms).
/// - For baud rates <= 19200 bps: 3.5 character times (38.5 bit periods).
pub fn t3_5_duration(baud_rate: u32) -> Duration {
    if baud_rate > 19200 {
        Duration::from_micros(1750)
    } else if baud_rate == 0 {
        Duration::from_millis(10)
    } else {
        let micros = 38_500_000u64 / (baud_rate as u64);
        Duration::from_micros(micros.max(1750))
    }
}

/// Calculate T1.5 inter-character gap timeout per Modbus Serial Line spec.
///
/// - For baud rates > 19200 bps: fixed at 750 µs.
/// - For baud rates <= 19200 bps: 1.5 character times (16.5 bit periods).
pub fn t1_5_duration(baud_rate: u32) -> Duration {
    if baud_rate > 19200 {
        Duration::from_micros(750)
    } else if baud_rate == 0 {
        Duration::from_millis(5)
    } else {
        let micros = 16_500_000u64 / (baud_rate as u64);
        Duration::from_micros(micros.max(750))
    }
}

// -----------------------------------------------------------------------------
// Frame Construction & Parsing
// -----------------------------------------------------------------------------

/// Build an RTU Application Data Unit (ADU) frame:
/// `[slave_address, PDU..., crc_lo, crc_hi]`
pub fn build_rtu_frame(slave_address: u8, pdu: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(1 + pdu.len() + 2);
    frame.push(slave_address);
    frame.extend_from_slice(pdu);
    let crc = calculate_crc(&frame);
    frame.extend_from_slice(&crc.to_le_bytes());
    frame
}

/// A parsed Modbus RTU frame with validated CRC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRtuFrame<'a> {
    pub slave_address: u8,
    pub pdu: &'a [u8],
}

/// Parse and validate an RTU frame from raw bytes.
///
/// Verifies:
/// 1. Length is between 4 and 256 bytes.
/// 2. 16-bit CRC matches `calculate_crc` over the address and PDU bytes.
pub fn parse_rtu_frame(raw: &[u8]) -> Result<ParsedRtuFrame<'_>, ModbusError> {
    if raw.len() < RTU_MIN_ADU_LEN {
        return Err(ModbusError::FrameTooShort {
            got: raw.len(),
            min: RTU_MIN_ADU_LEN,
        });
    }
    if raw.len() > RTU_MAX_ADU_LEN {
        return Err(ModbusError::InvalidDataLength(raw.len()));
    }
    let payload_len = raw.len() - 2;
    let expected_crc = calculate_crc(&raw[..payload_len]);
    let got_crc = u16::from_le_bytes([raw[payload_len], raw[payload_len + 1]]);
    if expected_crc != got_crc {
        return Err(ModbusError::CrcMismatch {
            expected: expected_crc,
            got: got_crc,
        });
    }
    Ok(ParsedRtuFrame {
        slave_address: raw[0],
        pdu: &raw[1..payload_len],
    })
}

// -----------------------------------------------------------------------------
// RTU Transport Abstraction
// -----------------------------------------------------------------------------

/// Abstraction for serial communication stream, supporting timeout adjustment.
pub trait RtuTransport: Read + Write + Send {
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), ModbusError>;
}

impl RtuTransport for Box<dyn serialport::SerialPort> {
    fn set_timeout(&mut self, timeout: Duration) -> Result<(), ModbusError> {
        (**self)
            .set_timeout(timeout)
            .map_err(|e| ModbusError::Serial(e.to_string()))
    }
}

// -----------------------------------------------------------------------------
// Modbus RTU Master
// -----------------------------------------------------------------------------

/// Modbus RTU Master client for serial RS-485 / RS-232 networks.
///
/// Enforces T3.5 inter-frame pacing, timeout handling with read-until-quiet heuristic,
/// CRC validation on received responses, and exception decoding.
pub struct RtuMaster<T: RtuTransport = Box<dyn serialport::SerialPort>> {
    transport: T,
    baud_rate: u32,
    timeout: Duration,
    last_activity: Option<Instant>,
}

impl RtuMaster<Box<dyn serialport::SerialPort>> {
    /// Open a new Modbus RTU master from a `SerialPortConfig`.
    pub fn open(config: &SerialPortConfig) -> Result<Self, ModbusError> {
        let port = config.open()?;
        Ok(Self {
            transport: port,
            baud_rate: config.baud_rate,
            timeout: Duration::from_millis(config.timeout_ms),
            last_activity: None,
        })
    }
}

impl<T: RtuTransport> RtuMaster<T> {
    /// Create an RTU master from an existing `RtuTransport` implementation.
    pub fn from_transport(transport: T, baud_rate: u32, timeout: Duration) -> Self {
        Self {
            transport,
            baud_rate,
            timeout,
            last_activity: None,
        }
    }

    /// Access the underlying transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Mutably access the underlying transport.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Send a request to a slave device and await the response.
    pub fn send_request(
        &mut self,
        slave: u8,
        req: &ModbusRequest,
    ) -> Result<ModbusResponse, ModbusError> {
        // 1. Inter-frame pacing: wait for T3.5 since previous activity
        let t3_5 = t3_5_duration(self.baud_rate);
        if let Some(last) = self.last_activity {
            let elapsed = last.elapsed();
            if elapsed < t3_5 {
                std::thread::sleep(t3_5 - elapsed);
            }
        }

        // 2. Encode and transmit request frame
        let mut pdu = Vec::new();
        req.encode_pdu(&mut pdu);
        let frame = build_rtu_frame(slave, &pdu);

        self.transport.write_all(&frame)?;
        self.transport.flush()?;
        self.last_activity = Some(Instant::now());

        // Broadcast requests (address 0) do not elicit responses per Modbus spec
        if slave == 0 {
            return Ok(ModbusResponse::WriteMultipleRegisters { address: 0, count: 0 });
        }

        // 3. Read response using read-until-quiet heuristic
        self.transport.set_timeout(self.timeout)?;

        let mut buf = Vec::with_capacity(RTU_MAX_ADU_LEN);
        let mut first_byte = [0u8; 1];
        match self.transport.read(&mut first_byte) {
            Ok(0) => return Err(ModbusError::Timeout),
            Ok(_) => buf.push(first_byte[0]),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                return Err(ModbusError::Timeout);
            }
            Err(e) => return Err(ModbusError::Io(e)),
        }

        // After the first character arrives, switch to T3.5 inter-frame quiet detection
        let quiet_timeout = t3_5.max(Duration::from_millis(3));
        let _ = self.transport.set_timeout(quiet_timeout);

        let mut scratch = [0u8; 64];
        loop {
            match self.transport.read(&mut scratch) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&scratch[..n]);
                    if buf.len() >= RTU_MAX_ADU_LEN {
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    // Inter-frame quiet gap detected: frame transmission completed
                    break;
                }
                Err(e) => return Err(ModbusError::Io(e)),
            }
        }

        // Restore default timeout
        let _ = self.transport.set_timeout(self.timeout);
        self.last_activity = Some(Instant::now());

        // 4. Validate CRC and frame boundary
        let parsed = parse_rtu_frame(&buf)?;

        // Filter responses by slave address
        if parsed.slave_address != slave {
            return Err(ModbusError::InvalidSlaveAddress(parsed.slave_address));
        }

        // 5. Decode response PDU
        ModbusResponse::parse_pdu(parsed.pdu)
    }

    /// Read discrete coils (FC 0x01).
    pub fn read_coils(
        &mut self,
        slave: u8,
        address: u16,
        count: u16,
    ) -> Result<Vec<bool>, ModbusError> {
        match self.send_request(slave, &ModbusRequest::ReadCoils { address, count })? {
            ModbusResponse::ReadBits(bits) => Ok(bits),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Read discrete inputs (FC 0x02).
    pub fn read_discrete_inputs(
        &mut self,
        slave: u8,
        address: u16,
        count: u16,
    ) -> Result<Vec<bool>, ModbusError> {
        match self.send_request(slave, &ModbusRequest::ReadDiscreteInputs { address, count })? {
            ModbusResponse::ReadBits(bits) => Ok(bits),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Read holding registers (FC 0x03).
    pub fn read_holding_registers(
        &mut self,
        slave: u8,
        address: u16,
        count: u16,
    ) -> Result<Vec<u16>, ModbusError> {
        match self.send_request(slave, &ModbusRequest::ReadHoldingRegisters { address, count })? {
            ModbusResponse::ReadRegisters(regs) => Ok(regs),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Read input registers (FC 0x04).
    pub fn read_input_registers(
        &mut self,
        slave: u8,
        address: u16,
        count: u16,
    ) -> Result<Vec<u16>, ModbusError> {
        match self.send_request(slave, &ModbusRequest::ReadInputRegisters { address, count })? {
            ModbusResponse::ReadRegisters(regs) => Ok(regs),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Write a single coil (FC 0x05).
    pub fn write_coil(&mut self, slave: u8, address: u16, value: bool) -> Result<bool, ModbusError> {
        match self.send_request(slave, &ModbusRequest::WriteSingleCoil { address, value })? {
            ModbusResponse::WriteSingleCoil { value, .. } => Ok(value),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Write multiple coils (FC 0x0F).
    pub fn write_coils(
        &mut self,
        slave: u8,
        address: u16,
        values: Vec<bool>,
    ) -> Result<u16, ModbusError> {
        match self.send_request(slave, &ModbusRequest::WriteMultipleCoils { address, values })? {
            ModbusResponse::WriteMultipleCoils { count, .. } => Ok(count),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Write a single holding register (FC 0x06).
    pub fn write_register(
        &mut self,
        slave: u8,
        address: u16,
        value: u16,
    ) -> Result<u16, ModbusError> {
        match self.send_request(slave, &ModbusRequest::WriteSingleRegister { address, value })? {
            ModbusResponse::WriteSingleRegister { value, .. } => Ok(value),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }

    /// Write multiple holding registers (FC 0x10).
    pub fn write_registers(
        &mut self,
        slave: u8,
        address: u16,
        values: Vec<u16>,
    ) -> Result<u16, ModbusError> {
        match self.send_request(slave, &ModbusRequest::WriteMultipleRegisters { address, values })? {
            ModbusResponse::WriteMultipleRegisters { count, .. } => Ok(count),
            _ => Err(ModbusError::InvalidDataLength(0)),
        }
    }
}

// -----------------------------------------------------------------------------
// Modbus RTU Slave
// -----------------------------------------------------------------------------

/// Modbus RTU Slave (device) implementation.
///
/// Designed with an iterative / low-overhead service model suitable for calling
/// inside `ferrite-runtime`'s scan cycle or on a dedicated serial thread without
/// busy blocking loops.
///
/// # Example Scanning Integration in Ferrite Runtime
/// ```no_run
/// use std::sync::{Arc, RwLock};
/// use std::time::Duration;
/// use ferrite_core::ProcessImage;
/// use ferrite_fieldbus_modbus::rtu::{RtuSlave, SerialPortConfig};
///
/// fn run_slave_in_runtime_loop(shared_pi: Arc<RwLock<ProcessImage>>) -> Result<(), Box<dyn std::error::Error>> {
///     let config = SerialPortConfig::new("COM2", 19200);
///     let mut port = config.open()?;
///     let mut slave = RtuSlave::new(1, 19200, shared_pi);
///
///     // Polling loop inside runtime or task runner (e.g. 10ms cycle)
///     loop {
///         let _ = slave.poll_port(&mut *port)?;
///         std::thread::sleep(Duration::from_millis(10));
///     }
/// }
/// ```
pub struct RtuSlave {
    slave_address: u8,
    baud_rate: u32,
    shared_pi: Arc<RwLock<ProcessImage>>,
    rx_buffer: Vec<u8>,
    last_rx: Option<Instant>,
}

impl RtuSlave {
    /// Create a new Modbus RTU slave instance.
    pub fn new(slave_address: u8, baud_rate: u32, shared_pi: Arc<RwLock<ProcessImage>>) -> Self {
        Self {
            slave_address,
            baud_rate,
            shared_pi,
            rx_buffer: Vec::with_capacity(RTU_MAX_ADU_LEN),
            last_rx: None,
        }
    }

    /// The configured slave address (1..247).
    pub fn slave_address(&self) -> u8 {
        self.slave_address
    }

    /// Pure iterative service function: checks elapsed quiet interval and processes complete frame.
    ///
    /// If `elapsed_quiet >= T3.5`, the buffered bytes are evaluated.
    /// If the frame is valid and addressed to this slave, returns `Some(tx_response_frame)`.
    /// On CRC error, non-matching slave address, or broadcast, returns `None`.
    pub fn service(&self, rx_buffer_bytes: &[u8], elapsed_quiet: Duration) -> Option<Vec<u8>> {
        let t3_5 = t3_5_duration(self.baud_rate);
        if elapsed_quiet < t3_5 {
            return None;
        }
        self.handle_frame(rx_buffer_bytes)
    }

    /// Process a verified raw RTU frame and generate the corresponding RTU response frame.
    pub fn handle_frame(&self, frame: &[u8]) -> Option<Vec<u8>> {
        let parsed = parse_rtu_frame(frame).ok()?;

        // Slave address filtering: ignore frames addressed to other slaves
        let is_broadcast = parsed.slave_address == 0;
        if parsed.slave_address != self.slave_address && !is_broadcast {
            return None;
        }

        let req_fc = parsed.pdu.first().copied().unwrap_or(0);
        let resp = match ModbusRequest::parse(parsed.pdu) {
            Ok(req) => {
                if let Ok(mut pi) = self.shared_pi.write() {
                    match ModbusProcessImageHandler::handle_request(&mut pi, &req) {
                        Ok(r) => r,
                        Err(exc) => ModbusResponse::Exception {
                            function: req.function_code(),
                            code: exc,
                        },
                    }
                } else {
                    ModbusResponse::Exception {
                        function: req.function_code(),
                        code: ModbusException::ServerDeviceFailure,
                    }
                }
            }
            Err(ModbusError::IllegalFunction(fc)) => ModbusResponse::Exception {
                function: fc,
                code: ModbusException::IllegalFunction,
            },
            Err(_) => ModbusResponse::Exception {
                function: req_fc,
                code: ModbusException::IllegalDataValue,
            },
        };

        // Broadcast requests are processed by slaves, but slaves MUST NOT respond
        if is_broadcast {
            return None;
        }

        let mut resp_pdu = Vec::with_capacity(256);
        resp.encode_pdu(req_fc, &mut resp_pdu);

        Some(build_rtu_frame(self.slave_address, &resp_pdu))
    }

    /// Non-blocking polling API on an open serial stream.
    ///
    /// Reads newly arrived bytes without blocking. When the T3.5 inter-frame gap
    /// is detected, processes the request against the shared `ProcessImage` and transmits
    /// the response.
    pub fn poll_port<T: Read + Write + ?Sized>(
        &mut self,
        port: &mut T,
    ) -> Result<Option<usize>, ModbusError> {
        let mut scratch = [0u8; 128];
        match port.read(&mut scratch) {
            Ok(0) => {}
            Ok(n) => {
                self.rx_buffer.extend_from_slice(&scratch[..n]);
                self.last_rx = Some(Instant::now());
                if self.rx_buffer.len() > RTU_MAX_ADU_LEN {
                    // Prevent memory overflow from line noise or framing loss
                    self.rx_buffer.clear();
                    self.last_rx = None;
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(ModbusError::Io(e)),
        }

        if !self.rx_buffer.is_empty()
            && let Some(last) = self.last_rx
        {
            let elapsed = last.elapsed();
            let t3_5 = t3_5_duration(self.baud_rate);
            if elapsed >= t3_5 {
                let maybe_resp = self.handle_frame(&self.rx_buffer);
                self.rx_buffer.clear();
                self.last_rx = None;

                if let Some(tx_frame) = maybe_resp {
                    port.write_all(&tx_frame)?;
                    port.flush()?;
                    return Ok(Some(tx_frame.len()));
                }
            }
        }

        Ok(None)
    }
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
pub struct MockRtuTransport {
    pub rx_data: std::io::Cursor<Vec<u8>>,
    pub tx_data: Vec<u8>,
}

#[cfg(test)]
impl MockRtuTransport {
    pub fn new(rx_bytes: Vec<u8>) -> Self {
        Self {
            rx_data: std::io::Cursor::new(rx_bytes),
            tx_data: Vec::new(),
        }
    }
}

#[cfg(test)]
impl Read for MockRtuTransport {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.rx_data.read(buf)
    }
}

#[cfg(test)]
impl Write for MockRtuTransport {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.tx_data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
impl RtuTransport for MockRtuTransport {
    fn set_timeout(&mut self, _timeout: Duration) -> Result<(), ModbusError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modbus_rtu_crc_known_vectors() {
        // Spec test vector: 01 03 00 00 00 0A -> CRC C5 CD
        let frame1 = [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A];
        let crc1 = calculate_crc(&frame1);
        let bytes1 = crc1.to_le_bytes();
        assert_eq!(bytes1, [0xC5, 0xCD]);
        assert_eq!(calculate_crc_bitwise(&frame1), crc1);

        // Vector 2: 02 07 -> CRC 41 12
        let frame2 = [0x02, 0x07];
        let crc2 = calculate_crc(&frame2);
        assert_eq!(crc2.to_le_bytes(), [0x41, 0x12]);
        assert_eq!(calculate_crc_bitwise(&frame2), crc2);

        // Vector 3: 11 03 00 6B 00 03 -> CRC 76 87
        let frame3 = [0x11, 0x03, 0x00, 0x6B, 0x00, 0x03];
        let crc3 = calculate_crc(&frame3);
        assert_eq!(crc3.to_le_bytes(), [0x76, 0x87]);
        assert_eq!(calculate_crc_bitwise(&frame3), crc3);
    }

    #[test]
    fn test_modbus_rtu_frame_build_and_parse() -> Result<(), ModbusError> {
        let pdu = [0x03, 0x00, 0x00, 0x00, 0x0A];
        let frame = build_rtu_frame(1, &pdu);
        assert_eq!(frame, [0x01, 0x03, 0x00, 0x00, 0x00, 0x0A, 0xC5, 0xCD]);

        let parsed = parse_rtu_frame(&frame)?;
        assert_eq!(parsed.slave_address, 1);
        assert_eq!(parsed.pdu, &pdu);

        // Corrupted CRC check
        let mut bad_frame = frame.clone();
        bad_frame[7] ^= 0xFF;
        match parse_rtu_frame(&bad_frame) {
            Err(ModbusError::CrcMismatch { expected, got }) => {
                assert_eq!(expected, 0xCDC5);
                assert_ne!(expected, got);
            }
            other => panic!("expected CrcMismatch, got {other:?}"),
        }

        // Frame too short
        match parse_rtu_frame(&[0x01, 0x03, 0x00]) {
            Err(ModbusError::FrameTooShort { got: 3, min: 4 }) => {}
            other => panic!("expected FrameTooShort, got {other:?}"),
        }

        // Frame too long
        let too_long = vec![0u8; 257];
        match parse_rtu_frame(&too_long) {
            Err(ModbusError::InvalidDataLength(257)) => {}
            other => panic!("expected InvalidDataLength, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_timing_calculations() {
        // 9600 baud: 38.5 / 9600 = ~4.01ms (4010 µs)
        let t3_5_9600 = t3_5_duration(9600);
        assert!(t3_5_9600.as_micros() >= 4000 && t3_5_9600.as_micros() <= 4050);

        // 19200 baud: 38.5 / 19200 = ~2.00ms (2005 µs)
        let t3_5_19200 = t3_5_duration(19200);
        assert!(t3_5_19200.as_micros() >= 2000 && t3_5_19200.as_micros() <= 2010);

        // 38400 baud: fixed at 1750 µs
        assert_eq!(t3_5_duration(38400), Duration::from_micros(1750));
        assert_eq!(t1_5_duration(38400), Duration::from_micros(750));
    }

    #[test]
    fn test_modbus_rtu_slave_service_and_filtering() -> Result<(), Box<dyn std::error::Error>> {
        let pi = Arc::new(RwLock::new(ProcessImage::default()));
        // Pre-write register 0 to 4242 in process image (%MW0)
        {
            let mut w = pi.write().map_err(|e| format!("{e:?}"))?;
            let _ = ModbusProcessImageHandler::handle_request(
                &mut w,
                &ModbusRequest::WriteSingleRegister {
                    address: 0,
                    value: 4242,
                },
            );
        }

        let slave = RtuSlave::new(1, 19200, Arc::clone(&pi));

        // 1. Valid request to slave 1: ReadHoldingRegisters address 0, count 1
        let req = ModbusRequest::ReadHoldingRegisters {
            address: 0,
            count: 1,
        };
        let mut pdu = Vec::new();
        req.encode_pdu(&mut pdu);
        let rx_frame = build_rtu_frame(1, &pdu);

        // Elapsed quiet < T3.5 -> None
        let resp_too_soon = slave.service(&rx_frame, Duration::from_micros(500));
        assert!(resp_too_soon.is_none());

        // Elapsed quiet >= T3.5 -> Response frame
        let resp_frame = slave
            .service(&rx_frame, Duration::from_millis(5))
            .ok_or("expected response frame")?;

        let parsed_resp = parse_rtu_frame(&resp_frame)?;
        assert_eq!(parsed_resp.slave_address, 1);
        let resp = ModbusResponse::parse_pdu(parsed_resp.pdu)?;
        assert_eq!(resp, ModbusResponse::ReadRegisters(vec![4242]));

        // 2. Request addressed to different slave (slave 2) -> ignored (None)
        let rx_frame_slave2 = build_rtu_frame(2, &pdu);
        let resp_other = slave.service(&rx_frame_slave2, Duration::from_millis(5));
        assert!(resp_other.is_none());

        // 3. Request with corrupted CRC -> silent discard (None)
        let mut corrupted = rx_frame;
        let last_idx = corrupted.len() - 1;
        corrupted[last_idx] ^= 0xFF;
        let resp_corrupt = slave.service(&corrupted, Duration::from_millis(5));
        assert!(resp_corrupt.is_none());

        // 4. Request with illegal function code -> exception response FC|0x80
        let illegal_frame = build_rtu_frame(1, &[0x42, 0x00, 0x00]);
        let resp_illegal = slave
            .service(&illegal_frame, Duration::from_millis(5))
            .ok_or("expected exception response")?;
        let parsed_illegal = parse_rtu_frame(&resp_illegal)?;
        assert_eq!(parsed_illegal.slave_address, 1);
        assert_eq!(parsed_illegal.pdu, &[0xC2, 0x01]); // 0x42 | 0x80, IllegalFunction (0x01)

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_master_mock_transport() -> Result<(), Box<dyn std::error::Error>> {
        // Pre-build a valid response for ReadHoldingRegisters address 10 -> values [100, 200]
        let mut resp_pdu = Vec::new();
        ModbusResponse::ReadRegisters(vec![100, 200]).encode_pdu(0x03, &mut resp_pdu);
        let resp_frame = build_rtu_frame(1, &resp_pdu);

        let transport = MockRtuTransport::new(resp_frame);
        let mut master = RtuMaster::from_transport(transport, 19200, Duration::from_millis(100));

        let regs = master.read_holding_registers(1, 10, 2)?;
        assert_eq!(regs, vec![100, 200]);

        // Verify master transmitted the expected request frame
        let tx = &master.transport().tx_data;
        let parsed_tx = parse_rtu_frame(tx)?;
        assert_eq!(parsed_tx.slave_address, 1);
        assert_eq!(
            ModbusRequest::parse(parsed_tx.pdu)?,
            ModbusRequest::ReadHoldingRegisters {
                address: 10,
                count: 2
            }
        );

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_master_exception_response() -> Result<(), Box<dyn std::error::Error>> {
        // Build exception response: FC 0x83 (ReadHoldingRegisters | 0x80), code 0x02 (IllegalDataAddress)
        let resp_frame = build_rtu_frame(1, &[0x83, 0x02]);
        let transport = MockRtuTransport::new(resp_frame);
        let mut master = RtuMaster::from_transport(transport, 19200, Duration::from_millis(100));

        let res = master.read_holding_registers(1, 9999, 1);
        match res {
            Err(ModbusError::Exception(ModbusException::IllegalDataAddress)) => {}
            other => panic!("expected IllegalDataAddress exception, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_master_crc_error_and_slave_mismatch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Response with wrong CRC
        let mut bad_crc_frame = build_rtu_frame(1, &[0x03, 0x02, 0x00, 0x05]);
        let last_idx = bad_crc_frame.len() - 1;
        bad_crc_frame[last_idx] ^= 0x55;
        let mut master_bad_crc = RtuMaster::from_transport(
            MockRtuTransport::new(bad_crc_frame),
            19200,
            Duration::from_millis(100),
        );
        match master_bad_crc.read_holding_registers(1, 0, 1) {
            Err(ModbusError::CrcMismatch { .. }) => {}
            other => panic!("expected CrcMismatch, got {other:?}"),
        }

        // 2. Response from wrong slave address (expected 1, got 2)
        let wrong_slave_frame = build_rtu_frame(2, &[0x03, 0x02, 0x00, 0x05]);
        let mut master_wrong_slave = RtuMaster::from_transport(
            MockRtuTransport::new(wrong_slave_frame),
            19200,
            Duration::from_millis(100),
        );
        match master_wrong_slave.read_holding_registers(1, 0, 1) {
            Err(ModbusError::InvalidSlaveAddress(2)) => {}
            other => panic!("expected InvalidSlaveAddress(2), got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_slave_broadcast_write_no_response(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pi = Arc::new(RwLock::new(ProcessImage::default()));
        let slave = RtuSlave::new(1, 19200, Arc::clone(&pi));

        // Broadcast WriteSingleCoil: slave 0, address 0, value true (%QX0.0)
        let req = ModbusRequest::WriteSingleCoil {
            address: 0,
            value: true,
        };
        let mut pdu = Vec::new();
        req.encode_pdu(&mut pdu);
        let broadcast_frame = build_rtu_frame(0, &pdu);

        let resp = slave.service(&broadcast_frame, Duration::from_millis(5));
        assert!(resp.is_none(), "broadcast must not return any response");

        // Verify ProcessImage was mutated by the broadcast write
        let r = pi.read().map_err(|e| format!("{e:?}"))?;
        let coil_val = ModbusProcessImageHandler::handle_request(
            &mut r.clone(),
            &ModbusRequest::ReadCoils {
                address: 0,
                count: 1,
            },
        )?;
        assert_eq!(coil_val, ModbusResponse::ReadBits(vec![true]));

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_slave_poll_port() -> Result<(), Box<dyn std::error::Error>> {
        let pi = Arc::new(RwLock::new(ProcessImage::default()));
        let mut slave = RtuSlave::new(1, 19200, Arc::clone(&pi));

        // Create a mock stream preloaded with a WriteSingleRegister request frame
        let mut req_pdu = Vec::new();
        ModbusRequest::WriteSingleRegister {
            address: 5,
            value: 999,
        }
        .encode_pdu(&mut req_pdu);
        let rx_frame = build_rtu_frame(1, &req_pdu);

        let mut transport = MockRtuTransport::new(rx_frame);

        // First poll: reads the bytes into rx_buffer
        let res1 = slave.poll_port(&mut transport)?;
        // Immediately after read, quiet interval hasn't passed (elapsed is ~0ms)
        assert_eq!(res1, None);

        // Sleep to exceed T3.5 (2.0ms at 19200 baud)
        std::thread::sleep(Duration::from_millis(5));

        // Second poll with no new bytes: detects quiet interval and transmits response
        let res2 = slave.poll_port(&mut transport)?;
        assert!(res2.is_some());

        // Verify response in transport tx_data
        let parsed_tx = parse_rtu_frame(&transport.tx_data)?;
        assert_eq!(parsed_tx.slave_address, 1);
        let resp = ModbusResponse::parse_pdu(parsed_tx.pdu)?;
        assert_eq!(
            resp,
            ModbusResponse::WriteSingleRegister {
                address: 5,
                value: 999
            }
        );

        Ok(())
    }

    #[test]
    fn test_modbus_rtu_master_all_poll_methods() -> Result<(), Box<dyn std::error::Error>> {
        // 1. read_coils
        let mut pdu = Vec::new();
        ModbusResponse::ReadBits(vec![true, false, true]).encode_pdu(0x01, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let coils = master.read_coils(1, 0, 3)?;
        assert_eq!(coils[..3], [true, false, true]);

        // 2. read_discrete_inputs
        pdu.clear();
        ModbusResponse::ReadBits(vec![false, true]).encode_pdu(0x02, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let inputs = master.read_discrete_inputs(1, 0, 2)?;
        assert_eq!(inputs[..2], [false, true]);

        // 3. read_input_registers
        pdu.clear();
        ModbusResponse::ReadRegisters(vec![555, 666]).encode_pdu(0x04, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let in_regs = master.read_input_registers(1, 0, 2)?;
        assert_eq!(in_regs, vec![555, 666]);

        // 4. write_coil
        pdu.clear();
        ModbusResponse::WriteSingleCoil {
            address: 10,
            value: true,
        }
        .encode_pdu(0x05, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let coil_res = master.write_coil(1, 10, true)?;
        assert!(coil_res);

        // 5. write_coils
        pdu.clear();
        ModbusResponse::WriteMultipleCoils {
            address: 0,
            count: 4,
        }
        .encode_pdu(0x0F, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let written_coils = master.write_coils(1, 0, vec![true, true, false, false])?;
        assert_eq!(written_coils, 4);

        // 6. write_register
        pdu.clear();
        ModbusResponse::WriteSingleRegister {
            address: 20,
            value: 777,
        }
        .encode_pdu(0x06, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let reg_res = master.write_register(1, 20, 777)?;
        assert_eq!(reg_res, 777);

        // 7. write_registers
        pdu.clear();
        ModbusResponse::WriteMultipleRegisters {
            address: 30,
            count: 3,
        }
        .encode_pdu(0x10, &mut pdu);
        let mut master = RtuMaster::from_transport(
            MockRtuTransport::new(build_rtu_frame(1, &pdu)),
            19200,
            Duration::from_millis(100),
        );
        let written_regs = master.write_registers(1, 30, vec![1, 2, 3])?;
        assert_eq!(written_regs, 3);

        Ok(())
    }

    #[test]
    #[ignore = "requires physical serial loopback or virtual COM port (FERRITE_TEST_SERIAL_PORT)"]
    fn test_rtu_hardware_serial_loopback() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(port_name) = std::env::var("FERRITE_TEST_SERIAL_PORT") else {
            return Ok(());
        };

        let config = SerialPortConfig::new(port_name, 19200).with_timeout_ms(200);
        let mut master = RtuMaster::open(&config)?;
        // Ping slave 1 with holding register read
        let _ = master.read_holding_registers(1, 0, 1);
        Ok(())
    }
}
