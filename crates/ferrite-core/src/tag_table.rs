use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::address::DirectAddress;
use crate::error::CoreError;
use crate::fb::{FbInstance, StandardFbKind};
use crate::types::{IecType, IecValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TagId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FbId(pub usize);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagEntry {
    pub name: String,
    pub data_type: IecType,
    #[serde(default)]
    pub address: Option<DirectAddress>,
    pub initial_value: IecValue,
    #[serde(skip)]
    pub value: Option<IecValue>,
    #[serde(skip)]
    pub forced: Option<IecValue>,
    #[serde(default)]
    pub read_only_outside_sfc: bool,
    #[serde(default)]
    pub retain: bool,
    #[serde(default)]
    pub comment: String,
}

impl TagEntry {
    pub fn new(
        name: impl Into<String>,
        data_type: IecType,
        address: Option<DirectAddress>,
        initial_value: IecValue,
        comment: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            data_type,
            address,
            value: Some(initial_value.clone()),
            initial_value,
            forced: None,
            read_only_outside_sfc: false,
            retain: false,
            comment: comment.into(),
        }
    }

    pub fn with_retain(mut self, retain: bool) -> Self {
        self.retain = retain;
        self
    }

    pub fn current_value(&self) -> &IecValue {
        if let Some(ref f) = self.forced {
            return f;
        }
        self.value.as_ref().unwrap_or(&self.initial_value)
    }
}

/// Unified Symbol Table (`TagTable`) storing global symbolic variables, `%I/%Q/%M` bindings,
/// shared Function Block instances, and Grafcet introspection symbols (`X<i>.state`, `X<i>.t`).
#[derive(Debug, Clone, Default)]
pub struct TagTable {
    tags: Vec<TagEntry>,
    tag_index: HashMap<String, TagId>,
    fbs: Vec<FbInstance>,
    fb_index: HashMap<String, FbId>,
}

impl TagTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a symbolic tag in the table and returns its pre-allocated `TagId`.
    pub fn register_tag(&mut self, mut entry: TagEntry) -> Result<TagId, CoreError> {
        let coerced = entry.initial_value.coerce_to(entry.data_type)?;
        entry.initial_value = coerced.clone();
        entry.value = Some(coerced);
        if let Some(&existing_id) = self.tag_index.get(&entry.name) {
            self.tags[existing_id.0] = entry;
            return Ok(existing_id);
        }
        let id = TagId(self.tags.len());
        self.tag_index.insert(entry.name.clone(), id);
        self.tags.push(entry);
        Ok(id)
    }

    /// Registers Grafcet introspection symbols `X<i>.state` (`BOOL`) and `X<i>.t` (`TIME`)
    /// as read-only outside of the SFC engine, returning `(state_id, time_id)`.
    pub fn register_sfc_step(&mut self, step_id: &str) -> Result<(TagId, TagId), CoreError> {
        self.register_sfc_step_full("", step_id, "", false)
    }

    /// Registers Grafcet introspection symbols for a step in `pou_name`, binding both short (`X<i>.state`,
    /// `<step_name>.state`) and POU-qualified (`<pou_name>.X<i>.state`, `<pou_name>.<step_name>.state`)
    /// aliases to the same underlying `TagId`.
    pub fn register_sfc_step_full(
        &mut self,
        pou_name: &str,
        step_id: &str,
        step_name: &str,
        is_initial: bool,
    ) -> Result<(TagId, TagId), CoreError> {
        let prefix = if step_id.starts_with('X') || step_id.starts_with('x') {
            step_id.to_string()
        } else {
            format!("X{step_id}")
        };
        let short_state_sym = format!("{prefix}.state");
        let short_time_sym = format!("{prefix}.t");

        let (primary_state_sym, primary_time_sym) =
            if !pou_name.is_empty() && self.tag_index.contains_key(&short_state_sym) {
                (
                    format!("{pou_name}.{short_state_sym}"),
                    format!("{pou_name}.{short_time_sym}"),
                )
            } else {
                (short_state_sym.clone(), short_time_sym.clone())
            };

        let mut state_entry = TagEntry::new(
            &primary_state_sym,
            IecType::Bool,
            None,
            IecValue::Bool(is_initial),
            if pou_name.is_empty() {
                format!("Grafcet step {step_id} active state")
            } else {
                format!("Grafcet {pou_name} step {step_id} active state")
            },
        );
        state_entry.read_only_outside_sfc = true;
        let state_id = self.register_tag(state_entry)?;

        let mut time_entry = TagEntry::new(
            &primary_time_sym,
            IecType::Time,
            None,
            IecValue::Time(0),
            if pou_name.is_empty() {
                format!("Grafcet step {step_id} elapsed active time")
            } else {
                format!("Grafcet {pou_name} step {step_id} elapsed active time")
            },
        );
        time_entry.read_only_outside_sfc = true;
        let time_id = self.register_tag(time_entry)?;

        // Register short aliases if not already taken by an earlier SFC POU
        self.tag_index.entry(short_state_sym.clone()).or_insert(state_id);
        self.tag_index.entry(short_time_sym.clone()).or_insert(time_id);

        if !step_id.starts_with('X') && !step_id.starts_with('x') {
            self.tag_index
                .entry(format!("{step_id}.state"))
                .or_insert(state_id);
            self.tag_index
                .entry(format!("{step_id}.t"))
                .or_insert(time_id);
        }

        // Register POU-qualified aliases (`<pou>.X<i>.state`, `<pou>.<i>.state`)
        if !pou_name.is_empty() {
            self.tag_index
                .insert(format!("{pou_name}.{short_state_sym}"), state_id);
            self.tag_index
                .insert(format!("{pou_name}.{short_time_sym}"), time_id);
            if !step_id.starts_with('X') && !step_id.starts_with('x') {
                self.tag_index
                    .insert(format!("{pou_name}.{step_id}.state"), state_id);
                self.tag_index
                    .insert(format!("{pou_name}.{step_id}.t"), time_id);
            }
        }

        // Register step-name aliases (`<step_name>.state`, `<pou>.<step_name>.state`)
        if !step_name.is_empty() && step_name != step_id {
            self.tag_index
                .entry(format!("{step_name}.state"))
                .or_insert(state_id);
            self.tag_index
                .entry(format!("{step_name}.t"))
                .or_insert(time_id);
            if !pou_name.is_empty() {
                self.tag_index
                    .insert(format!("{pou_name}.{step_name}.state"), state_id);
                self.tag_index
                    .insert(format!("{pou_name}.{step_name}.t"), time_id);
            }
        }

        Ok((state_id, time_id))
    }

    /// Registers a shared Function Block instance and returns its `FbId`.
    pub fn register_fb(&mut self, fb: FbInstance) -> FbId {
        if let Some(&id) = self.fb_index.get(&fb.name) {
            self.fbs[id.0] = fb;
            return id;
        }
        let id = FbId(self.fbs.len());
        self.fb_index.insert(fb.name.clone(), id);
        self.fbs.push(fb);
        id
    }

    pub fn tag_id(&self, name: &str) -> Option<TagId> {
        self.tag_index.get(name).copied()
    }

    pub fn fb_id(&self, name: &str) -> Option<FbId> {
        self.fb_index.get(name).copied()
    }

    pub fn get_tag_by_id(&self, id: TagId) -> Option<&TagEntry> {
        self.tags.get(id.0)
    }

    pub fn get_tag_mut_by_id(&mut self, id: TagId) -> Option<&mut TagEntry> {
        self.tags.get_mut(id.0)
    }

    pub fn get_tag(&self, name: &str) -> Option<&TagEntry> {
        let id = self.tag_id(name)?;
        self.tags.get(id.0)
    }

    pub fn get_tag_mut(&mut self, name: &str) -> Option<&mut TagEntry> {
        let id = self.tag_id(name)?;
        self.tags.get_mut(id.0)
    }

    pub fn get_tag_value(&self, name: &str) -> Option<&IecValue> {
        self.get_tag(name).map(|t| t.current_value())
    }

    pub fn set_tag_value(&mut self, name: &str, val: IecValue) -> Result<(), CoreError> {
        let id = self
            .tag_id(name)
            .ok_or_else(|| CoreError::UnknownSymbol(name.to_string()))?;
        let tag = &mut self.tags[id.0];
        let coerced = val.coerce_to(tag.data_type)?;
        tag.value = Some(coerced);
        Ok(())
    }

    pub fn get_fb_by_id(&self, id: FbId) -> Option<&FbInstance> {
        self.fbs.get(id.0)
    }

    pub fn get_fb_mut_by_id(&mut self, id: FbId) -> Option<&mut FbInstance> {
        self.fbs.get_mut(id.0)
    }

    pub fn get_fb(&self, name: &str) -> Option<&FbInstance> {
        let id = self.fb_id(name)?;
        self.fbs.get(id.0)
    }

    pub fn get_fb_mut(&mut self, name: &str) -> Option<&mut FbInstance> {
        let id = self.fb_id(name)?;
        self.fbs.get_mut(id.0)
    }

    pub fn tags(&self) -> &[TagEntry] {
        &self.tags
    }

    pub fn fbs(&self) -> &[FbInstance] {
        &self.fbs
    }

    pub fn fbs_mut(&mut self) -> &mut [FbInstance] {
        &mut self.fbs
    }

    /// Resolves the `IecType` of any symbol: direct address (`%IX0.0`), Grafcet symbol (`X0.state`),
    /// FB pin (`Timer_1.Q`), or named tag (`Start_Button`).
    pub fn symbol_type(&self, symbol: &str) -> Result<IecType, CoreError> {
        if let Some(tag) = self.get_tag(symbol) {
            return Ok(tag.data_type);
        }
        if symbol.starts_with('%') {
            let addr: DirectAddress = symbol.parse()?;
            return Ok(match addr.width {
                crate::address::AccessWidth::Bit => IecType::Bool,
                crate::address::AccessWidth::Byte | crate::address::AccessWidth::Word => {
                    IecType::Int
                }
                crate::address::AccessWidth::DWord => IecType::Dint,
            });
        }
        if let Some((inst, pin)) = symbol.split_once('.')
            && let Some(fb) = self.get_fb(inst) {
                if let Some(ty) = fb.kind.pin_type(pin) {
                    return Ok(ty);
                }
                return Err(CoreError::UnknownFbPin {
                    instance: inst.to_string(),
                    pin: pin.to_string(),
                });
            }
        Err(CoreError::UnknownSymbol(symbol.to_string()))
    }

    pub fn force_tag(&mut self, name: &str, val: IecValue) -> Result<(), CoreError> {
        let id = self
            .tag_id(name)
            .ok_or_else(|| CoreError::UnknownSymbol(name.to_string()))?;
        let tag = &mut self.tags[id.0];
        let coerced = val.coerce_to(tag.data_type)?;
        tag.forced = Some(coerced);
        Ok(())
    }

    pub fn unforce_tag(&mut self, name: &str) -> Result<(), CoreError> {
        let id = self
            .tag_id(name)
            .ok_or_else(|| CoreError::UnknownSymbol(name.to_string()))?;
        self.tags[id.0].forced = None;
        Ok(())
    }

    pub fn create_fb(&mut self, name: impl Into<String>, kind: StandardFbKind) -> FbId {
        self.register_fb(FbInstance::new(name, kind))
    }

    pub fn export_retain_values(&self) -> HashMap<String, IecValue> {
        let mut map = HashMap::new();
        for tag in &self.tags {
            if tag.retain {
                map.insert(tag.name.clone(), tag.current_value().clone());
            }
        }
        map
    }

    pub fn import_retain_values(&mut self, map: &HashMap<String, IecValue>) {
        for (name, val) in map {
            if let Some(id) = self.tag_id(name) {
                let tag = &mut self.tags[id.0];
                if tag.retain
                    && let Ok(coerced) = val.coerce_to(tag.data_type)
                {
                    tag.value = Some(coerced);
                }
            }
        }
    }

    /// Returns the total count of forced tags in this TagTable.
    pub fn active_forces_count(&self) -> usize {
        self.tags.iter().filter(|t| t.forced.is_some()).count()
    }

    /// Clears all tag forces.
    pub fn unforce_all(&mut self) {
        for t in &mut self.tags {
            t.forced = None;
        }
    }

    /// Resets all tags back to their initial configured values.
    pub fn reset_values_to_initial(&mut self) {
        for t in &mut self.tags {
            t.value = Some(t.initial_value.clone());
        }
    }
}
