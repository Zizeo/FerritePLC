use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::address::DirectAddress;
use crate::error::CoreError;
use crate::fb::{FbInstance, StandardFbKind};
use crate::types::{IecType, IecValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TagId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FbId(pub usize);

/// User-defined derived type definition: Struct, Array, or Enumeration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DerivedTypeKey {
    Struct {
        name: String,
        fields: Vec<(String, IecType)>,
        #[serde(default)]
        defaults: HashMap<String, IecValue>,
    },
    Array {
        name: String,
        elem_type: IecType,
        size: usize,
    },
    Enumeration {
        name: String,
        members: Vec<String>,
        #[serde(default)]
        values: Vec<i64>,
    },
}

impl DerivedTypeKey {
    pub fn name(&self) -> &str {
        match self {
            Self::Struct { name, .. } => name,
            Self::Array { name, .. } => name,
            Self::Enumeration { name, .. } => name,
        }
    }
}

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
    #[allow(dead_code)]
    derived_types: HashMap<String, DerivedTypeKey>,
    #[allow(dead_code)]
    arrays: HashMap<String, (IecType, usize)>,
}

/// Normalizes an SFC step identifier into canonical `X<N>` form (e.g. `"0"` -> `"X0"`, `"x1"` -> `"X1"`, `"S2"` -> `"X2"`).
pub fn normalize_sfc_step_prefix(step_id: &str) -> String {
    let trimmed = step_id.trim();
    let clean_num = trimmed.trim_start_matches(['X', 'x', 'S', 's']);
    if !clean_num.is_empty() && clean_num.chars().all(|c| c.is_ascii_digit()) {
        format!("X{clean_num}")
    } else if let Some(stripped) = trimmed.strip_prefix(['X', 'x']) {
        format!("X{stripped}")
    } else {
        format!("X{trimmed}")
    }
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
    ///
    /// When `pou_name` is provided, the primary tag name is strictly `<pou_name>.X<i>.state` (or `.t`).
    pub fn register_sfc_step_full(
        &mut self,
        pou_name: &str,
        step_id: &str,
        step_name: &str,
        is_initial: bool,
    ) -> Result<(TagId, TagId), CoreError> {
        let x_id = normalize_sfc_step_prefix(step_id);
        let short_state_sym = format!("{x_id}.state");
        let short_time_sym = format!("{x_id}.t");

        let pou_prefix = pou_name.trim();
        let (primary_state_sym, primary_time_sym) = if !pou_prefix.is_empty() {
            (
                format!("{pou_prefix}.{x_id}.state"),
                format!("{pou_prefix}.{x_id}.t"),
            )
        } else {
            (short_state_sym.clone(), short_time_sym.clone())
        };

        let mut state_entry = TagEntry::new(
            &primary_state_sym,
            IecType::Bool,
            None,
            IecValue::Bool(is_initial),
            if pou_prefix.is_empty() {
                format!("Grafcet step {x_id} active state")
            } else {
                format!("Grafcet {pou_prefix} step {x_id} active state")
            },
        );
        state_entry.read_only_outside_sfc = true;
        let state_id = self.register_tag(state_entry)?;

        let mut time_entry = TagEntry::new(
            &primary_time_sym,
            IecType::Time,
            None,
            IecValue::Time(0),
            if pou_prefix.is_empty() {
                format!("Grafcet step {x_id} elapsed active time")
            } else {
                format!("Grafcet {pou_prefix} step {x_id} elapsed active time")
            },
        );
        time_entry.read_only_outside_sfc = true;
        let time_id = self.register_tag(time_entry)?;

        // Register short aliases if not already taken by an earlier SFC POU
        self.tag_index
            .entry(short_state_sym.clone())
            .or_insert(state_id);
        self.tag_index
            .entry(short_time_sym.clone())
            .or_insert(time_id);

        let trimmed_id = step_id.trim();
        if trimmed_id != x_id {
            self.tag_index
                .entry(format!("{trimmed_id}.state"))
                .or_insert(state_id);
            self.tag_index
                .entry(format!("{trimmed_id}.t"))
                .or_insert(time_id);
        }

        // Register POU-qualified aliases (`<pou>.X<i>.state`, `<pou>.<i>.state`)
        if !pou_prefix.is_empty() {
            self.tag_index
                .insert(format!("{pou_prefix}.{short_state_sym}"), state_id);
            self.tag_index
                .insert(format!("{pou_prefix}.{short_time_sym}"), time_id);
            if trimmed_id != x_id {
                self.tag_index
                    .insert(format!("{pou_prefix}.{trimmed_id}.state"), state_id);
                self.tag_index
                    .insert(format!("{pou_prefix}.{trimmed_id}.t"), time_id);
            }
        }

        // Register step-name aliases (`<step_name>.state`, `<pou>.<step_name>.state`)
        let trimmed_name = step_name.trim();
        if !trimmed_name.is_empty() && trimmed_name != step_id && trimmed_name != x_id {
            self.tag_index
                .entry(format!("{trimmed_name}.state"))
                .or_insert(state_id);
            self.tag_index
                .entry(format!("{trimmed_name}.t"))
                .or_insert(time_id);
            if !pou_prefix.is_empty() {
                self.tag_index
                    .insert(format!("{pou_prefix}.{trimmed_name}.state"), state_id);
                self.tag_index
                    .insert(format!("{pou_prefix}.{trimmed_name}.t"), time_id);
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
        if tag.read_only_outside_sfc {
            return Err(CoreError::ReadOnlySymbol(name.to_string()));
        }
        let coerced = val.coerce_to(tag.data_type)?;
        tag.value = Some(coerced);
        Ok(())
    }

    /// Sets a tag's value and propagates the write to `ProcessImage` if the tag is bound to a `DirectAddress`.
    pub fn set_tag_value_with_image(
        &mut self,
        name: &str,
        val: IecValue,
        image: &mut crate::process_image::ProcessImage,
    ) -> Result<(), CoreError> {
        let id = self
            .tag_id(name)
            .ok_or_else(|| CoreError::UnknownSymbol(name.to_string()))?;
        let tag = self
            .tags
            .get(id.0)
            .ok_or_else(|| CoreError::UnknownSymbol(name.to_string()))?;
        if tag.read_only_outside_sfc {
            return Err(CoreError::ReadOnlySymbol(name.to_string()));
        }
        let coerced = val.coerce_to(tag.data_type)?;
        let addr_opt = tag.address;
        if let Some(addr) = addr_opt {
            if addr.area == crate::address::MemoryArea::Input {
                image.set_hw_input(addr, &coerced)?;
            } else {
                image.write(addr, &coerced)?;
            }
        }
        if let Some(tag) = self.tags.get_mut(id.0) {
            tag.value = Some(coerced);
        }
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
                crate::address::AccessWidth::LWord => IecType::Lword,
            });
        }
        if let Some((inst, pin)) = symbol.split_once('.')
            && let Some(fb) = self.get_fb(inst)
        {
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

    pub fn is_forced(&self, name: &str) -> bool {
        self.get_tag(name).is_some_and(|t| t.forced.is_some())
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

    /// Resets all shared Function Block instance states and accumulators.
    pub fn reset_fbs(&mut self) {
        for fb in &mut self.fbs {
            fb.reset();
        }
    }

    /// Registers a user-defined derived type (Struct, Array, or Enumeration).
    pub fn register_derived_type(&mut self, dt: DerivedTypeKey) -> Result<(), CoreError> {
        self.derived_types.insert(dt.name().to_string(), dt);
        Ok(())
    }

    /// Looks up a registered derived type definition by name.
    pub fn get_derived_type(&self, name: &str) -> Option<&DerivedTypeKey> {
        self.derived_types.get(name)
    }

    /// Returns all registered user-defined derived types.
    pub fn derived_types(&self) -> &HashMap<String, DerivedTypeKey> {
        &self.derived_types
    }

    /// Registers a struct instance flattened into individual member tags:
    /// e.g. instance `Motor` with fields `Cmd` (BOOL) and `Speed` (REAL)
    /// creates tags `Motor.Cmd` and `Motor.Speed`.
    pub fn register_struct_instance(
        &mut self,
        instance_name: &str,
        struct_type_name: &str,
    ) -> Result<Vec<TagId>, CoreError> {
        let dt = self
            .derived_types
            .get(struct_type_name)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSymbol(struct_type_name.to_string()))?;
        match dt {
            DerivedTypeKey::Struct {
                name: _,
                fields,
                defaults,
            } => {
                let mut ids = Vec::with_capacity(fields.len());
                for (field_name, field_ty) in fields {
                    let tag_name = format!("{instance_name}.{field_name}");
                    let initial_val = defaults
                        .get(&field_name)
                        .cloned()
                        .unwrap_or_else(|| field_ty.default_value());
                    let entry = TagEntry::new(
                        tag_name,
                        field_ty,
                        None,
                        initial_val,
                        format!("Struct field {instance_name}.{field_name} of type {struct_type_name}"),
                    );
                    ids.push(self.register_tag(entry)?);
                }
                Ok(ids)
            }
            _ => Err(CoreError::TypeMismatch {
                expected: IecType::String,
                actual: IecType::String,
            }),
        }
    }

    /// Registers an array instance flattened into indexed member tags:
    /// e.g. instance `Motor.Steps` of size 4 creates tags
    /// `Motor.Steps[0]`, `Motor.Steps[1]`, `Motor.Steps[2]`, `Motor.Steps[3]`.
    pub fn register_array_instance(
        &mut self,
        instance_name: &str,
        elem_type: IecType,
        size: usize,
        default_val: Option<IecValue>,
    ) -> Result<Vec<TagId>, CoreError> {
        self.arrays.insert(instance_name.to_string(), (elem_type, size));
        let mut ids = Vec::with_capacity(size);
        for i in 0..size {
            let tag_name = format!("{instance_name}[{i}]");
            let initial_val = default_val
                .clone()
                .unwrap_or_else(|| elem_type.default_value());
            let entry = TagEntry::new(
                tag_name,
                elem_type,
                None,
                initial_val,
                format!("Array element {instance_name}[{i}]"),
            );
            ids.push(self.register_tag(entry)?);
        }
        Ok(ids)
    }

    /// Registers an array instance from a registered `DerivedTypeKey::Array`.
    pub fn register_array_from_type(
        &mut self,
        instance_name: &str,
        array_type_name: &str,
    ) -> Result<Vec<TagId>, CoreError> {
        let dt = self
            .derived_types
            .get(array_type_name)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSymbol(array_type_name.to_string()))?;
        match dt {
            DerivedTypeKey::Array {
                name: _,
                elem_type,
                size,
            } => self.register_array_instance(instance_name, elem_type, size, None),
            _ => Err(CoreError::TypeMismatch {
                expected: IecType::String,
                actual: IecType::String,
            }),
        }
    }

    /// Registers an enumeration instance backed by an INT tag.
    pub fn register_enum_instance(
        &mut self,
        tag_name: &str,
        enum_type_name: &str,
        initial_member: Option<&str>,
    ) -> Result<TagId, CoreError> {
        let dt = self
            .derived_types
            .get(enum_type_name)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSymbol(enum_type_name.to_string()))?;
        match dt {
            DerivedTypeKey::Enumeration {
                name: _,
                members,
                values,
            } => {
                let init_num = if let Some(m) = initial_member {
                    let idx = members
                        .iter()
                        .position(|name| name == m)
                        .ok_or_else(|| CoreError::UnknownSymbol(m.to_string()))?;
                    values.get(idx).copied().unwrap_or(idx as i64)
                } else {
                    values.first().copied().unwrap_or(0)
                };
                let entry = TagEntry::new(
                    tag_name,
                    IecType::Int,
                    None,
                    IecValue::Int(init_num as i16),
                    format!("Enum instance {tag_name} of type {enum_type_name}"),
                );
                self.register_tag(entry)
            }
            _ => Err(CoreError::TypeMismatch {
                expected: IecType::Int,
                actual: IecType::String,
            }),
        }
    }

    /// High-level instantiation helper dispatching on type kind (Struct, Array, Enumeration).
    pub fn instantiate_derived_type(
        &mut self,
        instance_name: &str,
        type_name: &str,
    ) -> Result<Vec<TagId>, CoreError> {
        let dt = self
            .derived_types
            .get(type_name)
            .cloned()
            .ok_or_else(|| CoreError::UnknownSymbol(type_name.to_string()))?;
        match dt {
            DerivedTypeKey::Struct { .. } => {
                self.register_struct_instance(instance_name, type_name)
            }
            DerivedTypeKey::Array { .. } => {
                self.register_array_from_type(instance_name, type_name)
            }
            DerivedTypeKey::Enumeration { .. } => {
                let id = self.register_enum_instance(instance_name, type_name, None)?;
                Ok(vec![id])
            }
        }
    }

    /// Bounds-checked array metadata lookup.
    pub fn array_info(&self, array_name: &str) -> Option<(IecType, usize)> {
        self.arrays.get(array_name).copied()
    }

    /// Bounds-checked array element access returning a reference to the element's `TagEntry`.
    pub fn get_array_element(
        &self,
        array_name: &str,
        index: usize,
    ) -> Result<&TagEntry, CoreError> {
        let (_, size) = self
            .arrays
            .get(array_name)
            .copied()
            .ok_or_else(|| CoreError::UnknownSymbol(array_name.to_string()))?;
        if index >= size {
            return Err(CoreError::IndexOutOfBounds {
                name: array_name.to_string(),
                index,
                size,
            });
        }
        let sym = format!("{array_name}[{index}]");
        self.get_tag(&sym)
            .ok_or(CoreError::UnknownSymbol(sym))
    }

    /// Bounds-checked array element value lookup.
    pub fn get_array_element_value(
        &self,
        array_name: &str,
        index: usize,
    ) -> Result<&IecValue, CoreError> {
        self.get_array_element(array_name, index)
            .map(|t| t.current_value())
    }

    /// Bounds-checked array element mutation.
    pub fn set_array_element_value(
        &mut self,
        array_name: &str,
        index: usize,
        val: IecValue,
    ) -> Result<(), CoreError> {
        let (_, size) = self
            .arrays
            .get(array_name)
            .copied()
            .ok_or_else(|| CoreError::UnknownSymbol(array_name.to_string()))?;
        if index >= size {
            return Err(CoreError::IndexOutOfBounds {
                name: array_name.to_string(),
                index,
                size,
            });
        }
        let sym = format!("{array_name}[{index}]");
        self.set_tag_value(&sym, val)
    }

    /// Resolves an enumeration member name to its integer value.
    pub fn enum_value(&self, enum_type: &str, member: &str) -> Result<i64, CoreError> {
        let dt = self
            .derived_types
            .get(enum_type)
            .ok_or_else(|| CoreError::UnknownSymbol(enum_type.to_string()))?;
        match dt {
            DerivedTypeKey::Enumeration {
                members,
                values,
                ..
            } => {
                let idx = members
                    .iter()
                    .position(|m| m.eq_ignore_ascii_case(member))
                    .ok_or_else(|| CoreError::UnknownSymbol(member.to_string()))?;
                Ok(values.get(idx).copied().unwrap_or(idx as i64))
            }
            _ => Err(CoreError::TypeMismatch {
                expected: IecType::Int,
                actual: IecType::String,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_image::ProcessImage;

    #[test]
    fn test_regression_i2_set_tag_value_protects_sfc_and_propagates_image() -> Result<(), CoreError> {
        let mut tags = TagTable::new();
        let mut pi = ProcessImage::default();

        // 1. SFC introspection tag protection
        tags.register_sfc_step("0")?;
        assert!(matches!(
            tags.set_tag_value("X0.state", IecValue::Bool(true)),
            Err(CoreError::ReadOnlySymbol(_))
        ));

        // 2. Coercion validation path
        tags.register_tag(TagEntry::new(
            "Level",
            IecType::Int,
            None,
            IecValue::Int(0),
            "Level",
        ))?;
        assert!(tags.set_tag_value("Level", IecValue::Dint(42)).is_ok());
        assert_eq!(tags.get_tag_value("Level"), Some(&IecValue::Int(42)));

        // 3. Propagation to bound ProcessImage
        let qx0_0: DirectAddress = "%QX0.0".parse()?;
        tags.register_tag(TagEntry::new(
            "Pump",
            IecType::Bool,
            Some(qx0_0),
            IecValue::Bool(false),
            "Pump output",
        ))?;
        tags.set_tag_value_with_image("Pump", IecValue::Bool(true), &mut pi)?;
        assert_eq!(tags.get_tag_value("Pump"), Some(&IecValue::Bool(true)));
        assert_eq!(pi.read(qx0_0, IecType::Bool)?, IecValue::Bool(true));

        Ok(())
    }

    #[test]
    fn test_derived_type_struct_flattening() -> Result<(), CoreError> {
        let mut tags = TagTable::new();
        let mut defaults = HashMap::new();
        defaults.insert("Speed".to_string(), IecValue::Real(50.0));

        let struct_dt = DerivedTypeKey::Struct {
            name: "Motor_t".to_string(),
            fields: vec![
                ("Cmd".to_string(), IecType::Bool),
                ("Speed".to_string(), IecType::Real),
                ("Status".to_string(), IecType::Word),
            ],
            defaults,
        };
        tags.register_derived_type(struct_dt)?;

        let ids = tags.register_struct_instance("Pump1", "Motor_t")?;
        assert_eq!(ids.len(), 3);

        // Verify flattened tags exist in symbol table
        assert_eq!(tags.symbol_type("Pump1.Cmd")?, IecType::Bool);
        assert_eq!(tags.symbol_type("Pump1.Speed")?, IecType::Real);
        assert_eq!(tags.symbol_type("Pump1.Status")?, IecType::Word);

        // Verify default value applied
        assert_eq!(tags.get_tag_value("Pump1.Speed"), Some(&IecValue::Real(50.0)));
        assert_eq!(tags.get_tag_value("Pump1.Cmd"), Some(&IecValue::Bool(false)));

        // Mutate member
        tags.set_tag_value("Pump1.Cmd", IecValue::Bool(true))?;
        assert_eq!(tags.get_tag_value("Pump1.Cmd"), Some(&IecValue::Bool(true)));

        Ok(())
    }

    #[test]
    fn test_derived_type_array_flattening_and_bounds_access() -> Result<(), CoreError> {
        let mut tags = TagTable::new();

        // Register array Recipe.Steps of 4 elements (INT)
        let ids = tags.register_array_instance("Recipe.Steps", IecType::Int, 4, Some(IecValue::Int(10)))?;
        assert_eq!(ids.len(), 4);

        // Verify flattened tags exist
        assert_eq!(tags.get_tag_value("Recipe.Steps[0]"), Some(&IecValue::Int(10)));
        assert_eq!(tags.get_tag_value("Recipe.Steps[3]"), Some(&IecValue::Int(10)));

        // Test bounds-checked array access API
        assert_eq!(tags.get_array_element_value("Recipe.Steps", 2)?, &IecValue::Int(10));
        tags.set_array_element_value("Recipe.Steps", 2, IecValue::Int(99))?;
        assert_eq!(tags.get_array_element_value("Recipe.Steps", 2)?, &IecValue::Int(99));

        // Test out-of-bounds error
        let err = tags.get_array_element_value("Recipe.Steps", 4);
        assert!(matches!(
            err,
            Err(CoreError::IndexOutOfBounds { index: 4, size: 4, .. })
        ));

        let set_err = tags.set_array_element_value("Recipe.Steps", 10, IecValue::Int(5));
        assert!(matches!(
            set_err,
            Err(CoreError::IndexOutOfBounds { index: 10, size: 4, .. })
        ));

        Ok(())
    }

    #[test]
    fn test_derived_type_enumeration_and_lookup() -> Result<(), CoreError> {
        let mut tags = TagTable::new();

        let enum_dt = DerivedTypeKey::Enumeration {
            name: "MachineState_t".to_string(),
            members: vec![
                "Stopped".to_string(),
                "Starting".to_string(),
                "Running".to_string(),
                "Fault".to_string(),
            ],
            values: vec![0, 10, 20, 99],
        };
        tags.register_derived_type(enum_dt)?;

        // Verify lookup
        assert_eq!(tags.enum_value("MachineState_t", "Running")?, 20);
        assert_eq!(tags.enum_value("MachineState_t", "Fault")?, 99);
        assert!(tags.enum_value("MachineState_t", "Unknown").is_err());

        // Register enum instance
        tags.register_enum_instance("MainState", "MachineState_t", Some("Starting"))?;
        assert_eq!(tags.get_tag_value("MainState"), Some(&IecValue::Int(10)));

        Ok(())
    }
}
