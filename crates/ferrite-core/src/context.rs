use std::fmt;

use serde::{Deserialize, Serialize};

use crate::address::DirectAddress;
use crate::error::{CoreError, RuntimeError};
use crate::fb::FbInstance;
use crate::process_image::ProcessImage;
use crate::tag_table::{FbId, TagId, TagTable};
use crate::types::{IecType, IecValue};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PouId(pub String);

impl PouId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PouId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageKind {
    Ld,
    Sfc,
    St,
    Fbd,
}

impl fmt::Display for LanguageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ld => "LD",
            Self::Sfc => "SFC",
            Self::St => "ST",
            Self::Fbd => "FBD",
        };
        write!(f, "{s}")
    }
}

/// Common inter-language execution contract implemented by all compiled POUs (`.ld`, `.sfc`, `.st`, `.fbd`).
pub trait ExecutablePou: Send {
    fn id(&self) -> PouId;
    fn kind(&self) -> LanguageKind;
    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError>;
    /// Resets internal POU state (e.g. sequence steps and timers) on cold/warm restart.
    fn reset(&mut self) {}
}

/// Execution context passed to every `ExecutablePou` during a deterministic scan cycle.
pub struct ScanContext<'a> {
    pub process_image: &'a mut ProcessImage,
    pub tags: &'a mut TagTable,
    /// Elapsed scan delta in milliseconds.
    pub dt_ms: u64,
    /// Monotonically increasing scan cycle counter.
    pub scan_count: u64,
    /// True only while executing inside the SFC evolution engine (allows mutating `X<i>.state` / `X<i>.t`).
    in_sfc_engine: bool,
}

impl<'a> ScanContext<'a> {
    pub fn new(
        process_image: &'a mut ProcessImage,
        tags: &'a mut TagTable,
        dt_ms: u64,
        scan_count: u64,
    ) -> Self {
        Self {
            process_image,
            tags,
            dt_ms,
            scan_count,
            in_sfc_engine: false,
        }
    }

    /// Synchronizes all `TagTable` entries that are bound to a `DirectAddress` from the `ProcessImage`.
    /// Performs zero heap allocations.
    pub fn sync_from_process_image(&mut self) -> Result<(), RuntimeError> {
        let count = self.tags.tags().len();
        for idx in 0..count {
            let id = TagId(idx);
            let (addr_opt, ty, is_forced) = match self.tags.get_tag_by_id(id) {
                Some(t) => (t.address, t.data_type, t.forced.is_some()),
                None => continue,
            };
            if is_forced {
                continue;
            }
            if let Some(addr) = addr_opt {
                let val = self.process_image.read(addr, ty)?;
                if let Some(tag) = self.tags.get_tag_mut_by_id(id) {
                    tag.value = Some(val);
                }
            }
        }
        Ok(())
    }

    /// Runs a closure with SFC engine privilege so `X<i>.state` and `X<i>.t` can be updated.
    pub fn with_sfc_engine<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let prev = self.in_sfc_engine;
        self.in_sfc_engine = true;
        let res = f(self);
        self.in_sfc_engine = prev;
        res
    }

    /// Zero-allocation read by pre-resolved `TagId`.
    pub fn read_tag_id(&self, id: TagId) -> Result<IecValue, RuntimeError> {
        let tag = self
            .tags
            .get_tag_by_id(id)
            .ok_or_else(|| CoreError::UnknownSymbol(format!("TagId({})", id.0)))?;
        if tag.forced.is_some() {
            return Ok(tag.current_value().clone());
        }
        if let Some(addr) = tag.address {
            return Ok(self.process_image.read(addr, tag.data_type)?);
        }
        Ok(tag.current_value().clone())
    }

    /// Zero-allocation write by pre-resolved `TagId`.
    pub fn write_tag_id(&mut self, id: TagId, val: IecValue) -> Result<(), RuntimeError> {
        let (name, read_only, data_type, addr_opt, is_forced) = {
            let tag = self
                .tags
                .get_tag_by_id(id)
                .ok_or_else(|| CoreError::UnknownSymbol(format!("TagId({})", id.0)))?;
            (
                tag.name.clone(),
                tag.read_only_outside_sfc,
                tag.data_type,
                tag.address,
                tag.forced.is_some(),
            )
        };

        if read_only && !self.in_sfc_engine {
            return Err(CoreError::ReadOnlySymbol(name).into());
        }
        let coerced = val.coerce_to(data_type)?;
        if !is_forced {
            if let Some(addr) = addr_opt {
                self.process_image.write(addr, &coerced)?;
            }
            if let Some(tag) = self.tags.get_tag_mut_by_id(id) {
                tag.value = Some(coerced);
            }
        }
        Ok(())
    }

    /// Reads any symbol across the Unified Symbol Table & Shared Process Image:
    /// - Named symbolic tags (`Start_Button`, `X0.state`, `X0.t`)
    /// - Direct hardware/marker addresses (`%IX0.0`, `%QW0`, `%MW10`)
    /// - Function Block pins (`Timer_1.Q`, `Timer_1.ET`, `Counter_1.CV`)
    pub fn read_symbol(&self, symbol: &str) -> Result<IecValue, RuntimeError> {
        let trimmed = symbol.trim();
        if let Some(id) = self.tags.tag_id(trimmed) {
            return self.read_tag_id(id);
        }
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            let default_ty = match addr.width {
                crate::address::AccessWidth::Bit => IecType::Bool,
                crate::address::AccessWidth::Byte | crate::address::AccessWidth::Word => {
                    IecType::Int
                }
                crate::address::AccessWidth::DWord => IecType::Dint,
            };
            return Ok(self.process_image.read(addr, default_ty)?);
        }
        if let Some((inst, pin)) = trimmed.split_once('.')
            && let Some(fb) = self.tags.get_fb(inst)
        {
            return Ok(fb.read_pin(pin)?);
        }
        Err(CoreError::UnknownSymbol(trimmed.to_string()).into())
    }

    /// Convenience helper to read a boolean symbol (`BOOL`).
    pub fn read_bool(&self, symbol: &str) -> Result<bool, RuntimeError> {
        Ok(self.read_symbol(symbol)?.as_bool()?)
    }

    /// Writes any symbol across the Unified Symbol Table & Shared Process Image.
    pub fn write_symbol(&mut self, symbol: &str, val: IecValue) -> Result<(), RuntimeError> {
        let trimmed = symbol.trim();
        if let Some(id) = self.tags.tag_id(trimmed) {
            return self.write_tag_id(id, val);
        }
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            self.process_image.write(addr, &val)?;
            // Keep any tag bound to this exact address in sync
            let count = self.tags.tags().len();
            for idx in 0..count {
                let id = TagId(idx);
                if let Some(t) = self.tags.get_tag_by_id(id)
                    && t.address == Some(addr)
                    && t.forced.is_none()
                    && let Ok(coerced) = val.coerce_to(t.data_type)
                    && let Some(tm) = self.tags.get_tag_mut_by_id(id)
                {
                    tm.value = Some(coerced);
                }
            }
            return Ok(());
        }
        if let Some((inst, pin)) = trimmed.split_once('.')
            && let Some(fb) = self.tags.get_fb_mut(inst)
        {
            fb.write_pin(pin, &val)?;
            return Ok(());
        }
        Err(CoreError::UnknownSymbol(trimmed.to_string()).into())
    }

    /// Forces a symbol or direct address in the `TagTable` / `ProcessImage` (used by SFC forcing orders).
    pub fn force_symbol(&mut self, symbol: &str, val: IecValue) -> Result<(), RuntimeError> {
        let trimmed = symbol.trim();
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            self.process_image.force(addr, &val)?;
            return Ok(());
        }
        if let Some(tag) = self.tags.get_tag(trimmed)
            && let Some(addr) = tag.address
        {
            self.process_image.force(addr, &val)?;
        }
        self.tags.force_tag(trimmed, val)?;
        Ok(())
    }

    /// Unforces a symbol or direct address in the `TagTable` / `ProcessImage`.
    pub fn unforce_symbol(&mut self, symbol: &str) -> Result<(), RuntimeError> {
        let trimmed = symbol.trim();
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse()?;
            self.process_image.unforce(addr)?;
            return Ok(());
        }
        if let Some(tag) = self.tags.get_tag(trimmed)
            && let Some(addr) = tag.address
        {
            self.process_image.unforce(addr)?;
        }
        self.tags.unforce_tag(trimmed)?;
        Ok(())
    }

    /// Evaluates a shared Function Block instance by `FbId` using `self.dt_ms`.
    pub fn evaluate_fb_id(&mut self, id: FbId) -> Result<(), RuntimeError> {
        let dt = self.dt_ms;
        let fb = self
            .tags
            .get_fb_mut_by_id(id)
            .ok_or_else(|| CoreError::UnknownFbInstance(format!("FbId({})", id.0)))?;
        fb.evaluate(dt);
        Ok(())
    }

    /// Evaluates a shared Function Block instance by name using `self.dt_ms`.
    pub fn evaluate_fb(&mut self, instance_name: &str) -> Result<(), RuntimeError> {
        let dt = self.dt_ms;
        let fb: &mut FbInstance = self
            .tags
            .get_fb_mut(instance_name)
            .ok_or_else(|| CoreError::UnknownFbInstance(instance_name.to_string()))?;
        fb.evaluate(dt);
        Ok(())
    }
}
