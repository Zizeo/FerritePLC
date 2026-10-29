use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

use ferrite_core::{CoreError, DirectAddress, IecType, IecValue, MemoryArea, ProcessImage};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Core memory error: {0}")]
    Core(#[from] CoreError),

    #[error("Invalid GPIO pin {0}")]
    InvalidPin(u32),

    #[error("Permission denied accessing GPIO {0}: run as root/dialout or check udev rules")]
    PermissionDenied(u32),

    #[error("Driver initialization failed: {0}")]
    InitFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PinPull {
    #[default]
    None,
    PullUp,
    PullDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpioPinMapping {
    pub bcm_pin: u32,
    pub label: String,
    pub address: DirectAddress,
    pub direction: PinDirection,
    #[serde(default)]
    pub active_low: bool,
    #[serde(default)]
    pub pull: PinPull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GpioDriverKind {
    #[default]
    Mock,
    LinuxSysfs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpioHalConfig {
    pub driver_kind: GpioDriverKind,
    pub mappings: Vec<GpioPinMapping>,
}

impl Default for GpioHalConfig {
    fn default() -> Self {
        Self {
            driver_kind: GpioDriverKind::Mock,
            mappings: vec![
                GpioPinMapping {
                    bcm_pin: 17,
                    label: "Start_Button (Pin 11)".to_string(),
                    address: DirectAddress::bit(MemoryArea::Input, 0, 0),
                    direction: PinDirection::Input,
                    active_low: false,
                    pull: PinPull::PullDown,
                },
                GpioPinMapping {
                    bcm_pin: 27,
                    label: "Stop_Button (Pin 13)".to_string(),
                    address: DirectAddress::bit(MemoryArea::Input, 0, 1),
                    direction: PinDirection::Input,
                    active_low: true,
                    pull: PinPull::PullUp,
                },
                GpioPinMapping {
                    bcm_pin: 22,
                    label: "Sensor_1 (Pin 15)".to_string(),
                    address: DirectAddress::bit(MemoryArea::Input, 0, 2),
                    direction: PinDirection::Input,
                    active_low: false,
                    pull: PinPull::None,
                },
                GpioPinMapping {
                    bcm_pin: 23,
                    label: "Pump_Contactor (Pin 16)".to_string(),
                    address: DirectAddress::bit(MemoryArea::Output, 0, 0),
                    direction: PinDirection::Output,
                    active_low: false,
                    pull: PinPull::None,
                },
                GpioPinMapping {
                    bcm_pin: 24,
                    label: "Valve_Actuator (Pin 18)".to_string(),
                    address: DirectAddress::bit(MemoryArea::Output, 0, 1),
                    direction: PinDirection::Output,
                    active_low: false,
                    pull: PinPull::None,
                },
                GpioPinMapping {
                    bcm_pin: 25,
                    label: "Alarm_LED (Pin 22)".to_string(),
                    address: DirectAddress::bit(MemoryArea::Output, 0, 2),
                    direction: PinDirection::Output,
                    active_low: false,
                    pull: PinPull::None,
                },
            ],
        }
    }
}

/// Generic interface for physical or simulated hardware I/O adapters.
pub trait HardwareDriver: Send + Sync {
    fn name(&self) -> &str;

    /// Phase 1 (Input Scan): Read physical pins and update `ProcessImage` `%IX` buffers.
    fn read_inputs(&mut self, pi: &mut ProcessImage) -> Result<(), HalError>;

    /// Phase 3 (Output Scan): Read `ProcessImage` `%QX` buffers and drive physical output pins.
    fn write_outputs(&mut self, pi: &ProcessImage) -> Result<(), HalError>;

    /// Live pin logic level states (true = HIGH / 3.3V, false = LOW / 0V).
    fn pin_levels(&self) -> HashMap<u32, bool>;

    /// Injected mock input stimulus for simulation and tests.
    fn set_mock_input(&mut self, bcm: u32, level: bool) -> Result<(), HalError>;
}

/// In-memory mock GPIO driver for all host operating systems and offline development.
pub struct MockGpioDriver {
    pub config: GpioHalConfig,
    pub levels: HashMap<u32, bool>,
}

impl MockGpioDriver {
    pub fn new(config: GpioHalConfig) -> Self {
        let mut levels = HashMap::new();
        for m in &config.mappings {
            levels.insert(m.bcm_pin, false);
        }
        Self { config, levels }
    }
}

impl HardwareDriver for MockGpioDriver {
    fn name(&self) -> &str {
        "Mock / Simulated GPIO Driver"
    }

    fn read_inputs(&mut self, pi: &mut ProcessImage) -> Result<(), HalError> {
        for m in &self.config.mappings {
            if m.direction == PinDirection::Input {
                let raw_high = self.levels.get(&m.bcm_pin).copied().unwrap_or(false);
                let plc_val = if m.active_low { !raw_high } else { raw_high };
                pi.set_hw_input(m.address, &IecValue::Bool(plc_val))?;
            }
        }
        Ok(())
    }

    fn write_outputs(&mut self, pi: &ProcessImage) -> Result<(), HalError> {
        for m in &self.config.mappings {
            if m.direction == PinDirection::Output {
                let plc_val = pi
                    .read_hw_output(m.address, IecType::Bool)?
                    .as_bool()
                    .unwrap_or(false);
                let raw_high = if m.active_low { !plc_val } else { plc_val };
                self.levels.insert(m.bcm_pin, raw_high);
            }
        }
        Ok(())
    }

    fn pin_levels(&self) -> HashMap<u32, bool> {
        self.levels.clone()
    }

    fn set_mock_input(&mut self, bcm: u32, level: bool) -> Result<(), HalError> {
        self.levels.insert(bcm, level);
        Ok(())
    }
}

/// Linux standard Sysfs GPIO Driver (`/sys/class/gpio`).
pub struct LinuxSysfsGpioDriver {
    pub config: GpioHalConfig,
    pub levels: HashMap<u32, bool>,
}

impl LinuxSysfsGpioDriver {
    pub fn new(config: GpioHalConfig) -> Result<Self, HalError> {
        let mut levels = HashMap::new();
        for m in &config.mappings {
            levels.insert(m.bcm_pin, false);
            Self::export_pin(m.bcm_pin)?;
            Self::set_direction(m.bcm_pin, m.direction)?;
        }
        Ok(Self { config, levels })
    }

    fn export_pin(pin: u32) -> Result<(), HalError> {
        let gpio_dir = format!("/sys/class/gpio/gpio{pin}");
        if !Path::new(&gpio_dir).exists() {
            let export_path = "/sys/class/gpio/export";
            if Path::new(export_path).exists() {
                let mut f = OpenOptions::new()
                    .write(true)
                    .open(export_path)
                    .map_err(|_| HalError::PermissionDenied(pin))?;
                let _ = f.write_all(format!("{pin}").as_bytes());
            }
        }
        Ok(())
    }

    fn set_direction(pin: u32, dir: PinDirection) -> Result<(), HalError> {
        let dir_path = format!("/sys/class/gpio/gpio{pin}/direction");
        if Path::new(&dir_path).exists() {
            let mut f = OpenOptions::new()
                .write(true)
                .open(&dir_path)
                .map_err(|_| HalError::PermissionDenied(pin))?;
            let val = match dir {
                PinDirection::Input => "in",
                PinDirection::Output => "out",
            };
            let _ = f.write_all(val.as_bytes());
        }
        Ok(())
    }

    fn read_pin_sysfs(pin: u32) -> Result<bool, HalError> {
        let val_path = format!("/sys/class/gpio/gpio{pin}/value");
        if Path::new(&val_path).exists() {
            let mut buf = String::new();
            let mut f = OpenOptions::new()
                .read(true)
                .open(&val_path)
                .map_err(|_| HalError::PermissionDenied(pin))?;
            f.read_to_string(&mut buf)?;
            Ok(buf.trim() == "1")
        } else {
            Ok(false)
        }
    }

    fn write_pin_sysfs(pin: u32, high: bool) -> Result<(), HalError> {
        let val_path = format!("/sys/class/gpio/gpio{pin}/value");
        if Path::new(&val_path).exists() {
            let mut f = OpenOptions::new()
                .write(true)
                .open(&val_path)
                .map_err(|_| HalError::PermissionDenied(pin))?;
            f.write_all(if high { b"1" } else { b"0" })?;
        }
        Ok(())
    }
}

impl HardwareDriver for LinuxSysfsGpioDriver {
    fn name(&self) -> &str {
        "Linux Sysfs GPIO Driver (/sys/class/gpio)"
    }

    fn read_inputs(&mut self, pi: &mut ProcessImage) -> Result<(), HalError> {
        for m in &self.config.mappings {
            if m.direction == PinDirection::Input {
                let raw_high = Self::read_pin_sysfs(m.bcm_pin).unwrap_or(false);
                self.levels.insert(m.bcm_pin, raw_high);
                let plc_val = if m.active_low { !raw_high } else { raw_high };
                pi.set_hw_input(m.address, &IecValue::Bool(plc_val))?;
            }
        }
        Ok(())
    }

    fn write_outputs(&mut self, pi: &ProcessImage) -> Result<(), HalError> {
        for m in &self.config.mappings {
            if m.direction == PinDirection::Output {
                let plc_val = pi
                    .read_hw_output(m.address, IecType::Bool)?
                    .as_bool()
                    .unwrap_or(false);
                let raw_high = if m.active_low { !plc_val } else { plc_val };
                let _ = Self::write_pin_sysfs(m.bcm_pin, raw_high);
                self.levels.insert(m.bcm_pin, raw_high);
            }
        }
        Ok(())
    }

    fn pin_levels(&self) -> HashMap<u32, bool> {
        self.levels.clone()
    }

    fn set_mock_input(&mut self, bcm: u32, level: bool) -> Result<(), HalError> {
        self.levels.insert(bcm, level);
        Ok(())
    }
}

/// Creates the appropriate `HardwareDriver` according to `GpioHalConfig`.
pub fn create_hardware_driver(config: GpioHalConfig) -> Box<dyn HardwareDriver> {
    match config.driver_kind {
        GpioDriverKind::LinuxSysfs => {
            if cfg!(target_os = "linux") && Path::new("/sys/class/gpio").exists() {
                match LinuxSysfsGpioDriver::new(config.clone()) {
                    Ok(drv) => Box::new(drv),
                    Err(_) => Box::new(MockGpioDriver::new(config)),
                }
            } else {
                Box::new(MockGpioDriver::new(config))
            }
        }
        GpioDriverKind::Mock => Box::new(MockGpioDriver::new(config)),
    }
}

/// Static description of a Raspberry Pi 40-pin header connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RPiPinInfo {
    pub pin_number: u8,
    pub bcm: Option<u32>,
    pub name: &'static str,
}

pub const RPI_40PIN_HEADER: [RPiPinInfo; 40] = [
    RPiPinInfo {
        pin_number: 1,
        bcm: None,
        name: "3V3 Power",
    },
    RPiPinInfo {
        pin_number: 2,
        bcm: None,
        name: "5V Power",
    },
    RPiPinInfo {
        pin_number: 3,
        bcm: Some(2),
        name: "GPIO 2 (SDA)",
    },
    RPiPinInfo {
        pin_number: 4,
        bcm: None,
        name: "5V Power",
    },
    RPiPinInfo {
        pin_number: 5,
        bcm: Some(3),
        name: "GPIO 3 (SCL)",
    },
    RPiPinInfo {
        pin_number: 6,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 7,
        bcm: Some(4),
        name: "GPIO 4 (GPCLK0)",
    },
    RPiPinInfo {
        pin_number: 8,
        bcm: Some(14),
        name: "GPIO 14 (TXD)",
    },
    RPiPinInfo {
        pin_number: 9,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 10,
        bcm: Some(15),
        name: "GPIO 15 (RXD)",
    },
    RPiPinInfo {
        pin_number: 11,
        bcm: Some(17),
        name: "GPIO 17",
    },
    RPiPinInfo {
        pin_number: 12,
        bcm: Some(18),
        name: "GPIO 18 (PWM0)",
    },
    RPiPinInfo {
        pin_number: 13,
        bcm: Some(27),
        name: "GPIO 27",
    },
    RPiPinInfo {
        pin_number: 14,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 15,
        bcm: Some(22),
        name: "GPIO 22",
    },
    RPiPinInfo {
        pin_number: 16,
        bcm: Some(23),
        name: "GPIO 23",
    },
    RPiPinInfo {
        pin_number: 17,
        bcm: None,
        name: "3V3 Power",
    },
    RPiPinInfo {
        pin_number: 18,
        bcm: Some(24),
        name: "GPIO 24",
    },
    RPiPinInfo {
        pin_number: 19,
        bcm: Some(10),
        name: "GPIO 10 (MOSI)",
    },
    RPiPinInfo {
        pin_number: 20,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 21,
        bcm: Some(9),
        name: "GPIO 9 (MISO)",
    },
    RPiPinInfo {
        pin_number: 22,
        bcm: Some(25),
        name: "GPIO 25",
    },
    RPiPinInfo {
        pin_number: 23,
        bcm: Some(11),
        name: "GPIO 11 (SCLK)",
    },
    RPiPinInfo {
        pin_number: 24,
        bcm: Some(8),
        name: "GPIO 8 (CE0)",
    },
    RPiPinInfo {
        pin_number: 25,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 26,
        bcm: Some(7),
        name: "GPIO 7 (CE1)",
    },
    RPiPinInfo {
        pin_number: 27,
        bcm: Some(0),
        name: "GPIO 0 (ID_SD)",
    },
    RPiPinInfo {
        pin_number: 28,
        bcm: Some(1),
        name: "GPIO 1 (ID_SC)",
    },
    RPiPinInfo {
        pin_number: 29,
        bcm: Some(5),
        name: "GPIO 5",
    },
    RPiPinInfo {
        pin_number: 30,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 31,
        bcm: Some(6),
        name: "GPIO 6",
    },
    RPiPinInfo {
        pin_number: 32,
        bcm: Some(12),
        name: "GPIO 12 (PWM0)",
    },
    RPiPinInfo {
        pin_number: 33,
        bcm: Some(13),
        name: "GPIO 13 (PWM1)",
    },
    RPiPinInfo {
        pin_number: 34,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 35,
        bcm: Some(19),
        name: "GPIO 19 (MISO)",
    },
    RPiPinInfo {
        pin_number: 36,
        bcm: Some(16),
        name: "GPIO 16",
    },
    RPiPinInfo {
        pin_number: 37,
        bcm: Some(26),
        name: "GPIO 26",
    },
    RPiPinInfo {
        pin_number: 38,
        bcm: Some(20),
        name: "GPIO 20 (MOSI)",
    },
    RPiPinInfo {
        pin_number: 39,
        bcm: None,
        name: "Ground",
    },
    RPiPinInfo {
        pin_number: 40,
        bcm: Some(21),
        name: "GPIO 21 (SCLK)",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_gpio_driver_input_output_cycle() -> Result<(), Box<dyn std::error::Error>> {
        let config = GpioHalConfig::default();
        let mut drv = MockGpioDriver::new(config);
        let mut pi = ProcessImage::default();

        // 1. Set physical input (BCM 17: active high, maps to %IX0.0)
        drv.set_mock_input(17, true)?;
        drv.read_inputs(&mut pi)?;
        pi.latch_inputs();

        let ix0_0 = DirectAddress::bit(MemoryArea::Input, 0, 0);
        assert_eq!(pi.read(ix0_0, IecType::Bool)?, IecValue::Bool(true));

        // 2. Drive physical output (BCM 23: maps to %QX0.0)
        let qx0_0 = DirectAddress::bit(MemoryArea::Output, 0, 0);
        pi.write(qx0_0, &IecValue::Bool(true))?;
        pi.commit_outputs();
        drv.write_outputs(&pi)?;

        let levels = drv.pin_levels();
        assert_eq!(levels.get(&23).copied(), Some(true));
        Ok(())
    }
}
