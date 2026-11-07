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
        if offset.checked_add(len).is_none_or(|end| end > mask_slice.len() || end > force_val_slice.len()) {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
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
        let len = addr.byte_len();
        if offset.checked_add(len).is_none_or(|end| end > mask_slice.len() || end > slice.len()) {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
        match addr.width {
            AccessWidth::Bit => {
                let bit_mask = 1u8 << addr.bit;
                mask_slice[offset] |= bit_mask;
                slice[offset] = (slice[offset] & !bit_mask) | (force_val_slice[offset] & bit_mask);
            }
            AccessWidth::Byte | AccessWidth::Word | AccessWidth::DWord | AccessWidth::LWord => {
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
        if offset.checked_add(len).is_none_or(|end| end > mask_slice.len()) {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
        match addr.width {
            AccessWidth::Bit => {
                mask_slice[offset] &= !(1u8 << addr.bit);
            }
            AccessWidth::Byte | AccessWidth::Word | AccessWidth::DWord | AccessWidth::LWord => {
                for b in &mut mask_slice[offset..(offset + len)] {
                    *b = 0;
                }
            }
        }
        Ok(())
    }

    /// Returns the total count of forced bits across all memory areas (%I, %Q, %M).
    pub fn active_forces_count(&self) -> usize {
        let count_bits =
            |slice: &[u8]| -> usize { slice.iter().map(|b| b.count_ones() as usize).sum() };
        count_bits(&self.force_mask_inputs)
            + count_bits(&self.force_mask_outputs)
            + count_bits(&self.force_mask_markers)
    }

    /// Clears all active force bitmasks and forced values across %I, %Q, %M.
    pub fn unforce_all(&mut self) {
        self.force_mask_inputs.fill(0);
        self.force_val_inputs.fill(0);
        self.force_mask_outputs.fill(0);
        self.force_val_outputs.fill(0);
        self.force_mask_markers.fill(0);
        self.force_val_markers.fill(0);
    }

    /// Zeroes all memory buffers (%I, %Q, %M).
    pub fn clear(&mut self) {
        self.inputs.fill(0);
        self.hw_inputs.fill(0);
        self.outputs.fill(0);
        self.hw_outputs.fill(0);
        self.markers.fill(0);
    }

    fn read_from_slice(
        slice: &[u8],
        addr: DirectAddress,
        ty: IecType,
    ) -> Result<IecValue, CoreError> {
        let offset = addr.byte_offset();
        let len = addr.byte_len();
        if offset.checked_add(len).is_none_or(|end| end > slice.len()) {
            return Err(CoreError::AddressOutOfBounds(addr.to_string()));
        }
        match addr.width {
            AccessWidth::Bit => {
                if ty != IecType::Bool {
                    return Err(CoreError::TypeMismatch {
                        expected: IecType::Bool,
                        actual: ty,
                    });
                }
                let bit_val = (slice[offset] & (1u8 << addr.bit)) != 0;
                Ok(IecValue::Bool(bit_val))
            }
            AccessWidth::Byte => {
                let b = slice[offset];
                match ty {
                    IecType::Bool => Ok(IecValue::Bool(b != 0)),
                    IecType::Int => Ok(IecValue::Int(i16::from(b))),
                    IecType::Dint => Ok(IecValue::Dint(i32::from(b))),
                    IecType::Sint => Ok(IecValue::Sint(b as i8)),
                    IecType::Usint => Ok(IecValue::Usint(b)),
                    IecType::Byte => Ok(IecValue::Byte(b)),
                    other => Err(CoreError::TypeMismatch {
                        expected: IecType::Int,
                        actual: other,
                    }),
                }
            }
            AccessWidth::Word => {
                let bytes = [slice[offset], slice[offset + 1]];
                let w = i16::from_le_bytes(bytes);
                match ty {
                    IecType::Int => Ok(IecValue::Int(w)),
                    IecType::Dint => Ok(IecValue::Dint(i32::from(w))),
                    IecType::Bool => Ok(IecValue::Bool(w != 0)),
                    IecType::Uint => Ok(IecValue::Uint(u16::from_le_bytes(bytes))),
                    IecType::Word => Ok(IecValue::Word(u16::from_le_bytes(bytes))),
                    other => Err(CoreError::TypeMismatch {
                        expected: IecType::Int,
                        actual: other,
                    }),
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
                    IecType::Dint => Ok(IecValue::Dint(i32::from_le_bytes(bytes))),
                    IecType::Int => Ok(IecValue::Int(i32::from_le_bytes(bytes) as i16)),
                    IecType::Bool => Ok(IecValue::Bool(u32::from_le_bytes(bytes) != 0)),
                    IecType::Udint => Ok(IecValue::Udint(u32::from_le_bytes(bytes))),
                    IecType::Dword => Ok(IecValue::Dword(u32::from_le_bytes(bytes))),
                    other => Err(CoreError::TypeMismatch {
                        expected: IecType::Dint,
                        actual: other,
                    }),
                }
            }
            AccessWidth::LWord => {
                let bytes = [
                    slice[offset],
                    slice[offset + 1],
                    slice[offset + 2],
                    slice[offset + 3],
                    slice[offset + 4],
                    slice[offset + 5],
                    slice[offset + 6],
                    slice[offset + 7],
                ];
                match ty {
                    IecType::Lreal => Ok(IecValue::Lreal(f64::from_le_bytes(bytes))),
                    IecType::Lword => Ok(IecValue::Lword(u64::from_le_bytes(bytes))),
                    IecType::Lint => Ok(IecValue::Lint(i64::from_le_bytes(bytes))),
                    IecType::Ulint => Ok(IecValue::Ulint(u64::from_le_bytes(bytes))),
                    IecType::Time => Ok(IecValue::Time(u64::from_le_bytes(bytes))),
                    IecType::Real => Ok(IecValue::Real(f64::from_le_bytes(bytes) as f32)),
                    IecType::Udint => Ok(IecValue::Udint(u64::from_le_bytes(bytes) as u32)),
                    IecType::Dword => Ok(IecValue::Dword(u64::from_le_bytes(bytes) as u32)),
                    IecType::Dint => Ok(IecValue::Dint(i64::from_le_bytes(bytes) as i32)),
                    IecType::Int => Ok(IecValue::Int(i64::from_le_bytes(bytes) as i16)),
                    IecType::Bool => Ok(IecValue::Bool(u64::from_le_bytes(bytes) != 0)),
                    other => Err(CoreError::TypeMismatch {
                        expected: IecType::Lword,
                        actual: other,
                    }),
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
        if offset.checked_add(len).is_none_or(|end| end > slice.len()) {
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
            AccessWidth::Byte => match val {
                IecValue::Byte(b) => {
                    slice[offset] = *b;
                }
                IecValue::Usint(u) => {
                    slice[offset] = *u;
                }
                IecValue::Sint(s) => {
                    slice[offset] = *s as u8;
                }
                IecValue::Int(v) => {
                    slice[offset] = *v as u8;
                }
                IecValue::Bool(b) => {
                    slice[offset] = if *b { 1 } else { 0 };
                }
                other => {
                    return Err(CoreError::TypeMismatch {
                        expected: IecType::Int,
                        actual: other.iec_type(),
                    });
                }
            },
            AccessWidth::Word => match val {
                IecValue::Word(w) => {
                    let bytes = w.to_le_bytes();
                    slice[offset..offset + 2].copy_from_slice(&bytes);
                }
                IecValue::Uint(u) => {
                    let bytes = u.to_le_bytes();
                    slice[offset..offset + 2].copy_from_slice(&bytes);
                }
                IecValue::Int(v) => {
                    let bytes = v.to_le_bytes();
                    slice[offset] = bytes[0];
                    slice[offset + 1] = bytes[1];
                }
                IecValue::Bool(b) => {
                    let v = if *b { 1i16 } else { 0i16 };
                    let bytes = v.to_le_bytes();
                    slice[offset] = bytes[0];
                    slice[offset + 1] = bytes[1];
                }
                other => {
                    return Err(CoreError::TypeMismatch {
                        expected: IecType::Int,
                        actual: other.iec_type(),
                    });
                }
            },
            AccessWidth::DWord => match val {
                IecValue::Dword(dw) => {
                    let bytes = dw.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Udint(u) => {
                    let bytes = u.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Real(r) => {
                    let bytes = r.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Time(ms) => {
                    let u = u32::try_from(*ms).map_err(|_| CoreError::TypeMismatch {
                        expected: IecType::Time,
                        actual: IecType::Time,
                    })?;
                    let bytes = u.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Dint(d) => {
                    let bytes = d.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Int(v) => {
                    let bytes = i32::from(*v).to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                IecValue::Bool(b) => {
                    let d = if *b { 1i32 } else { 0i32 };
                    let bytes = d.to_le_bytes();
                    slice[offset..offset + 4].copy_from_slice(&bytes);
                }
                other => {
                    return Err(CoreError::TypeMismatch {
                        expected: IecType::Dint,
                        actual: other.iec_type(),
                    });
                }
            },
            AccessWidth::LWord => match val {
                IecValue::Lreal(r) => {
                    slice[offset..offset + 8].copy_from_slice(&r.to_le_bytes());
                }
                IecValue::Lword(lw) => {
                    slice[offset..offset + 8].copy_from_slice(&lw.to_le_bytes());
                }
                IecValue::Lint(li) => {
                    slice[offset..offset + 8].copy_from_slice(&li.to_le_bytes());
                }
                IecValue::Ulint(ul) => {
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Time(ms) => {
                    slice[offset..offset + 8].copy_from_slice(&ms.to_le_bytes());
                }
                IecValue::Real(r) => {
                    let f = f64::from(*r);
                    slice[offset..offset + 8].copy_from_slice(&f.to_le_bytes());
                }
                IecValue::Dint(d) => {
                    let li = i64::from(*d);
                    slice[offset..offset + 8].copy_from_slice(&li.to_le_bytes());
                }
                IecValue::Udint(u) => {
                    let ul = u64::from(*u);
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Dword(dw) => {
                    let ul = u64::from(*dw);
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Int(i) => {
                    let li = i64::from(*i);
                    slice[offset..offset + 8].copy_from_slice(&li.to_le_bytes());
                }
                IecValue::Uint(u) => {
                    let ul = u64::from(*u);
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Word(w) => {
                    let ul = u64::from(*w);
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Sint(s) => {
                    let li = i64::from(*s);
                    slice[offset..offset + 8].copy_from_slice(&li.to_le_bytes());
                }
                IecValue::Usint(u) => {
                    let ul = u64::from(*u);
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Byte(b) => {
                    let ul = u64::from(*b);
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                IecValue::Bool(b) => {
                    let ul = if *b { 1u64 } else { 0u64 };
                    slice[offset..offset + 8].copy_from_slice(&ul.to_le_bytes());
                }
                other => {
                    return Err(CoreError::TypeMismatch {
                        expected: IecType::Lword,
                        actual: other.iec_type(),
                    });
                }
            },
        }
        Ok(())
    }

    /// Returns a copy of the %M internal marker byte buffer for retentive persistence.
    pub fn export_markers(&self) -> Vec<u8> {
        self.markers.clone()
    }

    /// Restores the %M internal marker byte buffer from a retentive snapshot.
    pub fn import_markers(&mut self, src: &[u8]) {
        let len = self.markers.len().min(src.len());
        self.markers[..len].copy_from_slice(&src[..len]);
    }

    /// Convenience helper to write a 16-bit word into %MW<word_idx>.
    pub fn write_marker_word(&mut self, word_idx: usize, val: u16) -> Result<(), CoreError> {
        let offset = word_idx * 2;
        if offset + 2 > self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%MW{word_idx}")));
        }
        self.markers[offset..offset + 2].copy_from_slice(&val.to_le_bytes());
        Ok(())
    }

    /// Convenience helper to read a 16-bit word from %MW<word_idx>.
    pub fn read_marker_word(&self, word_idx: usize) -> Result<u16, CoreError> {
        let offset = word_idx * 2;
        if offset + 2 > self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%MW{word_idx}")));
        }
        let bytes: [u8; 2] = [self.markers[offset], self.markers[offset + 1]];
        Ok(u16::from_le_bytes(bytes))
    }

    /// Convenience helper to write an 8-bit byte into %MB<byte_idx>.
    pub fn write_marker_byte(&mut self, byte_idx: usize, val: u8) -> Result<(), CoreError> {
        if byte_idx >= self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%MB{byte_idx}")));
        }
        self.markers[byte_idx] = val;
        Ok(())
    }

    /// Convenience helper to read an 8-bit byte from %MB<byte_idx>.
    pub fn read_marker_byte(&self, byte_idx: usize) -> Result<u8, CoreError> {
        if byte_idx >= self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%MB{byte_idx}")));
        }
        Ok(self.markers[byte_idx])
    }

    /// Convenience helper to write a 32-bit double word into %MD<dword_idx>.
    pub fn write_marker_dword(&mut self, dword_idx: usize, val: u32) -> Result<(), CoreError> {
        let offset = dword_idx.checked_mul(4).ok_or_else(|| {
            CoreError::AddressOutOfBounds(format!("%MD{dword_idx}"))
        })?;
        if offset + 4 > self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%MD{dword_idx}")));
        }
        self.markers[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
        Ok(())
    }

    /// Convenience helper to read a 32-bit double word from %MD<dword_idx>.
    pub fn read_marker_dword(&self, dword_idx: usize) -> Result<u32, CoreError> {
        let offset = dword_idx.checked_mul(4).ok_or_else(|| {
            CoreError::AddressOutOfBounds(format!("%MD{dword_idx}"))
        })?;
        if offset + 4 > self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%MD{dword_idx}")));
        }
        let bytes: [u8; 4] = self.markers[offset..offset + 4]
            .try_into()
            .map_err(|_| CoreError::AddressOutOfBounds(format!("%MD{dword_idx}")))?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Convenience helper to write a 64-bit long word into %ML<lword_idx>.
    pub fn write_marker_lword(&mut self, lword_idx: usize, val: u64) -> Result<(), CoreError> {
        let offset = lword_idx.checked_mul(8).ok_or_else(|| {
            CoreError::AddressOutOfBounds(format!("%ML{lword_idx}"))
        })?;
        if offset + 8 > self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%ML{lword_idx}")));
        }
        self.markers[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
        Ok(())
    }

    /// Convenience helper to read a 64-bit long word from %ML<lword_idx>.
    pub fn read_marker_lword(&self, lword_idx: usize) -> Result<u64, CoreError> {
        let offset = lword_idx.checked_mul(8).ok_or_else(|| {
            CoreError::AddressOutOfBounds(format!("%ML{lword_idx}"))
        })?;
        if offset + 8 > self.markers.len() {
            return Err(CoreError::AddressOutOfBounds(format!("%ML{lword_idx}")));
        }
        let bytes: [u8; 8] = self.markers[offset..offset + 8]
            .try_into()
            .map_err(|_| CoreError::AddressOutOfBounds(format!("%ML{lword_idx}")))?;
        Ok(u64::from_le_bytes(bytes))
    }

    /// Convenience helper to write a 64-bit double precision float into %ML<lword_idx>.
    pub fn write_marker_lreal(&mut self, lword_idx: usize, val: f64) -> Result<(), CoreError> {
        self.write_marker_lword(lword_idx, val.to_bits())
    }

    /// Convenience helper to read a 64-bit double precision float from %ML<lword_idx>.
    pub fn read_marker_lreal(&self, lword_idx: usize) -> Result<f64, CoreError> {
        self.read_marker_lword(lword_idx).map(f64::from_bits)
    }
}

pub type MemoryMap = ProcessImage;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_image_forces_count_and_unforce_all() -> Result<(), Box<dyn std::error::Error>> {
        let mut pi = ProcessImage::new(64);
        let in_addr: DirectAddress = "%IX0.0".parse()?;
        let out_addr: DirectAddress = "%QX0.1".parse()?;
        pi.force(in_addr, &IecValue::Bool(true))?;
        pi.force(out_addr, &IecValue::Bool(true))?;
        assert_eq!(pi.active_forces_count(), 2);
        pi.unforce_all();
        assert_eq!(pi.active_forces_count(), 0);
        Ok(())
    }

    #[test]
    fn test_regression_i3_word_byte_write_truncation_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let mut pi = ProcessImage::new(64);
        let word_addr: DirectAddress = "%MW0".parse()?;
        let byte_addr: DirectAddress = "%MB0".parse()?;
        let dword_addr: DirectAddress = "%MD0".parse()?;

        // Writing Real, Dint, Time to Word must return TypeMismatch, not truncate silently
        assert!(matches!(
            pi.write(word_addr, &IecValue::Real(3.75)),
            Err(CoreError::TypeMismatch { expected: IecType::Int, actual: IecType::Real })
        ));
        assert!(matches!(
            pi.write(word_addr, &IecValue::Dint(70000)),
            Err(CoreError::TypeMismatch { expected: IecType::Int, actual: IecType::Dint })
        ));
        assert!(matches!(
            pi.write(word_addr, &IecValue::Time(1000)),
            Err(CoreError::TypeMismatch { expected: IecType::Int, actual: IecType::Time })
        ));

        // Writing Real, Dint, Time to Byte must also return TypeMismatch
        assert!(matches!(
            pi.write(byte_addr, &IecValue::Real(1.5)),
            Err(CoreError::TypeMismatch { expected: IecType::Int, actual: IecType::Real })
        ));
        assert!(matches!(
            pi.write(byte_addr, &IecValue::Dint(300)),
            Err(CoreError::TypeMismatch { expected: IecType::Int, actual: IecType::Dint })
        ));

        // Writing Time > u32::MAX ms (> 49.7 days) to DWord must error, not truncate
        let huge_time = (u32::MAX as u64) + 10;
        assert!(matches!(
            pi.write(dword_addr, &IecValue::Time(huge_time)),
            Err(CoreError::TypeMismatch { expected: IecType::Time, actual: IecType::Time })
        ));

        // Normal Word and DWord writes still succeed
        assert!(pi.write(word_addr, &IecValue::Int(1234)).is_ok());
        assert_eq!(pi.read(word_addr, IecType::Int)?, IecValue::Int(1234));

        assert!(pi.write(dword_addr, &IecValue::Real(3.75)).is_ok());
        assert_eq!(pi.read(dword_addr, IecType::Real)?, IecValue::Real(3.75));

        Ok(())
    }

    #[test]
    fn test_process_image_64bit_lword_lreal_and_bounds() -> Result<(), Box<dyn std::error::Error>> {
        let mut pi = ProcessImage::new(128);

        // 1. LWord write/read
        let ml0: DirectAddress = "%ML0".parse()?;
        let lword_val = IecValue::Lword(0x0123_4567_89AB_CDEF);
        pi.write(ml0, &lword_val)?;
        assert_eq!(pi.read(ml0, IecType::Lword)?, lword_val);

        // 2. LReal write/read
        let ml1: DirectAddress = "%ML1".parse()?;
        let lreal_val = IecValue::Lreal(123.456789012345);
        pi.write(ml1, &lreal_val)?;
        assert_eq!(pi.read(ml1, IecType::Lreal)?, lreal_val);

        // 3. Convenience marker helpers
        pi.write_marker_lreal(2, 456.789012345678)?;
        assert_eq!(pi.read_marker_lreal(2)?, 456.789012345678);

        pi.write_marker_lword(3, 0xAABB_CCDD_EEFF_0011)?;
        assert_eq!(pi.read_marker_lword(3)?, 0xAABB_CCDD_EEFF_0011);

        pi.write_marker_byte(40, 0xAB)?;
        assert_eq!(pi.read_marker_byte(40)?, 0xAB);

        pi.write_marker_dword(11, 0x1234_5678)?;
        assert_eq!(pi.read_marker_dword(11)?, 0x1234_5678);

        // 4. Force on L-width
        let ql0: DirectAddress = "%QL0".parse()?;
        pi.force(ql0, &IecValue::Lword(0xFFFF_FFFF_FFFF_FFFF))?;
        assert_eq!(pi.read(ql0, IecType::Lword)?, IecValue::Lword(0xFFFF_FFFF_FFFF_FFFF));
        assert_eq!(pi.active_forces_count(), 64);
        pi.unforce(ql0)?;
        assert_eq!(pi.active_forces_count(), 0);

        // 5. Bounds check
        let ml_out: DirectAddress = "%ML20".parse()?; // 20 * 8 = 160 > 128
        assert!(matches!(pi.read(ml_out, IecType::Lword), Err(CoreError::AddressOutOfBounds(_))));
        assert!(matches!(pi.write(ml_out, &lword_val), Err(CoreError::AddressOutOfBounds(_))));

        Ok(())
    }
}
