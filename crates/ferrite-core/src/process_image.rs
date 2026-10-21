use crate::address::{AccessWidth, DirectAddress, MemoryArea};
use crate::error::CoreError;
use crate::types::{IecType, IecValue};

pub const DEFAULT_MEMORY_AREA_BYTES: usize = 1024;

/// Deterministic shared Process Image (`MemoryMap`) for `%I`, `%Q`, and `%M`.
///
/// Pre-allocates all buffers at creation so `latch_inputs()`, `commit_outputs()`,
/// and all direct memory reads/writes during the PLC scan loop perform zero heap allocations.
#[derive(Debug, Clone)]
pub struct ProcessImage {
    /// Simulated hardware input bus written by scenarios/UI before Phase 1 (Input Scan).
    hw_inputs: Vec<u8>,
    /// Latched `%I` buffer read by POUs during Phase 2 (Task Execution).
    inputs: Vec<u8>,
    /// Working `%Q` buffer mutated by POUs during Phase 2.
    outputs: Vec<u8>,
    /// Committed hardware `%Q` bus updated during Phase 3 (Output Scan).
    hw_outputs: Vec<u8>,
    /// Internal `%M` marker memory shared across all POUs.
    markers: Vec<u8>,

    /// Bit-level force masks & values for `%I`, `%Q`, `%M`.
    force_mask_inputs: Vec<u8>,
    force_val_inputs: Vec<u8>,
    force_mask_outputs: Vec<u8>,
    force_val_outputs: Vec<u8>,
    force_mask_markers: Vec<u8>,
    force_val_markers: Vec<u8>,
}

impl Default for ProcessImage {
    fn default() -> Self {
        Self::new(DEFAULT_MEMORY_AREA_BYTES)
    }
}

impl ProcessImage {
    pub fn new(area_bytes: usize) -> Self {
        Self {
            hw_inputs: vec![0; area_bytes],
            inputs: vec![0; area_bytes],
            outputs: vec![0; area_bytes],
            hw_outputs: vec![0; area_bytes],
            markers: vec![0; area_bytes],
            force_mask_inputs: vec![0; area_bytes],
            force_val_inputs: vec![0; area_bytes],
            force_mask_outputs: vec![0; area_bytes],
            force_val_outputs: vec![0; area_bytes],
            force_mask_markers: vec![0; area_bytes],
            force_val_markers: vec![0; area_bytes],
        }
    }

    /// Phase 1 (Input Scan): Latches simulated hardware inputs into the `%I` input buffer,
    /// applying any active input forces. Zero heap allocations.
    pub fn latch_inputs(&mut self) {
        for i in 0..self.inputs.len() {
            let mask = self.force_mask_inputs[i];
            let forced = self.force_val_inputs[i];
            self.inputs[i] = (self.hw_inputs[i] & !mask) | (forced & mask);
        }
    }

    /// Phase 3 (Output Scan): Applies any active output forces and commits the `%Q` output
    /// buffer to the simulated hardware output bus. Zero heap allocations.
    pub fn commit_outputs(&mut self) {
        for i in 0..self.outputs.len() {
            let mask = self.force_mask_outputs[i];
            let forced = self.force_val_outputs[i];
            let effective = (self.outputs[i] & !mask) | (forced & mask);
            self.outputs[i] = effective;
            self.hw_outputs[i] = effective;
        }
    }

    /// Sets simulated hardware input value (picked up on next `latch_inputs()`).
    pub fn set_hw_input(&mut self, addr: DirectAddress, val: &IecValue) -> Result<(), CoreError> {
        if addr.area != MemoryArea::Input {
            return Err(CoreError::InvalidDirectAddress(addr.to_string()));
        }
        Self::write_to_slice(&mut self.hw_inputs, addr, val)
    }

    /// Reads committed hardware output value (`hw_outputs`).
    pub fn read_hw_output(&self, addr: DirectAddress, ty: IecType) -> Result<IecValue, CoreError> {
        if addr.area != MemoryArea::Output {
            return Err(CoreError::InvalidDirectAddress(addr.to_string()));
        }
        Self::read_from_slice(&self.hw_outputs, addr, ty)
    }

    /// Reads from the active process image (`%I`, `%Q`, or `%M`).
    pub fn read(&self, addr: DirectAddress, ty: IecType) -> Result<IecValue, CoreError> {
        let slice = match addr.area {
            MemoryArea::Input => &self.inputs,
            MemoryArea::Output => &self.outputs,
            MemoryArea::Marker => &self.markers,
        };
        Self::read_from_slice(slice, addr, ty)
    }

    /// Writes to the active process image (`%I`, `%Q`, or `%M`), respecting active force masks.
    pub fn write(&mut self, addr: DirectAddress, val: &IecValue) -> Result<(), CoreError> {
        let (slice, mask_slice, force_val_slice) = match addr.area {
            MemoryArea::Input => (
                &mut self.inputs,
                &self.force_mask_inputs,
                &self.force_val_inputs,
            ),
            MemoryArea::Output => (
                &mut self.outputs,
                &self.force_mask_outputs,
                &self.force_val_outputs,
            ),
            MemoryArea::Marker => (
                &mut self.markers,
                &self.force_mask_markers,
                &self.force_val_markers,
            ),
        };
        Self::write_to_slice(slice, addr, val)?;
        let offset = addr.byte_offset();
        let len = addr.byte_len();
        for i in offset..(offset + len) {
            let mask = mask_slice[i];
            if mask != 0 {
                slice[i] = (slice[i] & !mask) | (force_val_slice[i] & mask);
            }
        }
        Ok(())
    }

    /// Forces a direct address to a fixed value until `unforce` is called.
    pub fn force(&mut self, addr: DirectAddress, val: &IecValue) -> Result<(), CoreError> {
        let (slice, mask_slice, force_val_slice) = match addr.area {
            MemoryArea::Input => (
                &mut self.inputs,
                &mut self.force_mask_inputs,
                &mut self.force_val_inputs,
            ),
            MemoryArea::Output => (
                &mut self.outputs,
                &mut self.force_mask_outputs,
                &mut self.force_val_outputs,
            ),
            MemoryArea::Marker => (
                &mut self.markers,
                &mut self.force_mask_markers,
                &mut self.force_val_markers,
            ),
        };
        Self::write_to_slice(force_val_slice, addr, val)?;
        let offset = addr.byte_offset();
        match addr.width {
            AccessWidth::Bit => {
                let bit_mask = 1u8 << addr.bit;
                mask_slice[offset] |= bit_mask;
                slice[offset] =
                    (slice[offset] & !bit_mask) | (force_val_slice[offset] & bit_mask);
            }
            AccessWidth::Byte | AccessWidth::Word | AccessWidth::DWord => {
                let len = addr.byte_len();
                for i in offset..(offset + len) {
                    mask_slice[i] = 0xFF;
                    slice[i] = force_val_slice[i];
                }
            }
        }
        Ok(())
    }

    /// Clears a force on a direct address.
    pub fn unforce(&mut self, addr: DirectAddress) -> Result<(), CoreError> {
        let mask_slice = match addr.area {
            MemoryArea::Input => &mut self.force_mask_inputs,
            MemoryArea::Output => &mut self.force_mask_outputs,
            MemoryArea::Marker => &mut self.force_mask_markers,
        };
        let offset = addr.byte_offset();
        let len = addr.byte_len();
        if offset + len > mask_slice.len() {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
        match addr.width {
            AccessWidth::Bit => {
                mask_slice[offset] &= !(1u8 << addr.bit);
            }
            AccessWidth::Byte | AccessWidth::Word | AccessWidth::DWord => {
                for b in &mut mask_slice[offset..(offset + len)] {
                    *b = 0;
                }
            }
        }
        Ok(())
    }

    fn read_from_slice(
        slice: &[u8],
        addr: DirectAddress,
        ty: IecType,
    ) -> Result<IecValue, CoreError> {
        let offset = addr.byte_offset();
        let len = addr.byte_len();
        if offset + len > slice.len() {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
        match addr.width {
            AccessWidth::Bit => {
                let bit_val = (slice[offset] & (1u8 << addr.bit)) != 0;
                Ok(IecValue::Bool(bit_val))
            }
            AccessWidth::Byte => {
                let b = slice[offset];
                match ty {
                    IecType::Bool => Ok(IecValue::Bool(b != 0)),
                    IecType::Int => Ok(IecValue::Int(i16::from(b))),
                    IecType::Dint => Ok(IecValue::Dint(i32::from(b))),
                    _ => Ok(IecValue::Int(i16::from(b))),
                }
            }
            AccessWidth::Word => {
                let bytes = [slice[offset], slice[offset + 1]];
                let w = i16::from_le_bytes(bytes);
                match ty {
                    IecType::Int => Ok(IecValue::Int(w)),
                    IecType::Dint => Ok(IecValue::Dint(i32::from(w))),
                    IecType::Real => Ok(IecValue::Real(f32::from(w))),
                    IecType::Time => Ok(IecValue::Time(w.max(0) as u64)),
                    IecType::Bool => Ok(IecValue::Bool(w != 0)),
                    IecType::String => Ok(IecValue::Int(w)),
                }
            }
            AccessWidth::DWord => {
                let bytes = [
                    slice[offset],
                    slice[offset + 1],
                    slice[offset + 2],
                    slice[offset + 3],
                ];
                match ty {
                    IecType::Real => Ok(IecValue::Real(f32::from_le_bytes(bytes))),
                    IecType::Time => Ok(IecValue::Time(u32::from_le_bytes(bytes) as u64)),
                    IecType::Int => Ok(IecValue::Int(i32::from_le_bytes(bytes) as i16)),
                    _ => Ok(IecValue::Dint(i32::from_le_bytes(bytes))),
                }
            }
        }
    }

    fn write_to_slice(
        slice: &mut [u8],
        addr: DirectAddress,
        val: &IecValue,
    ) -> Result<(), CoreError> {
        let offset = addr.byte_offset();
        let len = addr.byte_len();
        if offset + len > slice.len() {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
        match addr.width {
            AccessWidth::Bit => {
                let b = val.as_bool()?;
                if b {
                    slice[offset] |= 1u8 << addr.bit;
                } else {
                    slice[offset] &= !(1u8 << addr.bit);
                }
            }
            AccessWidth::Byte => {
                let v = val.as_int()? as u8;
                slice[offset] = v;
            }
            AccessWidth::Word => {
                let v = val.as_int()?;
                let bytes = v.to_le_bytes();
                slice[offset] = bytes[0];
                slice[offset + 1] = bytes[1];
            }
            AccessWidth::DWord => match val {
                IecValue::Real(r) => {
                    let bytes = r.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Time(ms) => {
                    let bytes = (*ms as u32).to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                _ => {
                    let d = val.as_dint()?;
                    let bytes = d.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
            },
        }
        Ok(())
    }
}

pub type MemoryMap = ProcessImage;
