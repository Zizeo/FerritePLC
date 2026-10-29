use ferrite_core::{
    ExecutablePou, IecType, IecValue, LanguageKind, PouId, RuntimeError, ScanContext, TagTable,
};
use ferrite_ir::{IrError, SymbolResolver};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum LdError {
    #[error("JSON parse error in Ladder POU: {0}")]
    Json(String),

    #[error("IR/Validation error in Ladder POU '{pou}': {source}")]
    Ir {
        pou: String,
        #[source]
        source: IrError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContactKind {
    #[serde(alias = "NO", alias = "no", alias = "normally_open")]
    NormallyOpen,
    #[serde(alias = "NC", alias = "nc", alias = "normally_closed")]
    NormallyClosed,
    #[serde(alias = "P", alias = "rising_edge", alias = "RISING")]
    RisingEdge,
    #[serde(alias = "N", alias = "falling_edge", alias = "FALLING")]
    FallingEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoilKind {
    #[serde(alias = "normal", alias = "out")]
    Normal,
    #[serde(alias = "S", alias = "set", alias = "latch")]
    Set,
    #[serde(alias = "R", alias = "reset", alias = "unlatch")]
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LdElement {
    Contact {
        symbol: String,
        kind: ContactKind,
    },
    Branch {
        paths: Vec<Vec<LdElement>>,
    },
    InlineFb {
        instance: String,
        #[serde(default)]
        enable_pin: Option<String>,
        #[serde(default)]
        inputs: Vec<(String, String)>,
        #[serde(default)]
        outputs: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdCoil {
    pub symbol: String,
    pub kind: CoilKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdRung {
    pub id: String,
    #[serde(default)]
    pub comment: String,
    pub elements: Vec<LdElement>,
    pub coils: Vec<LdCoil>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdProgram {
    pub name: String,
    #[serde(default)]
    pub comment: String,
    pub rungs: Vec<LdRung>,
}

impl LdProgram {
    pub fn from_json(json: &str) -> Result<Self, LdError> {
        serde_json::from_str(json).map_err(|e| LdError::Json(e.to_string()))
    }

    pub fn to_json_pretty(&self) -> Result<String, LdError> {
        serde_json::to_string_pretty(self).map_err(|e| LdError::Json(e.to_string()))
    }

    pub fn validate(&self, tags: &TagTable) -> Result<(), LdError> {
        for rung in &self.rungs {
            for elem in &rung.elements {
                self.validate_element(elem, tags)?;
            }
            for coil in &rung.coils {
                let ty =
                    SymbolResolver::validate_writable_symbol(tags, &coil.symbol, &self.name, false)
                        .map_err(|source| LdError::Ir {
                            pou: self.name.clone(),
                            source,
                        })?;
                if ty != IecType::Bool {
                    return Err(LdError::Ir {
                        pou: self.name.clone(),
                        source: IrError::TypeMismatch {
                            pou: self.name.clone(),
                            expected: IecType::Bool,
                            actual: ty,
                            context: format!("coil '{}'", coil.symbol),
                        },
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_element(&self, elem: &LdElement, tags: &TagTable) -> Result<(), LdError> {
        match elem {
            LdElement::Contact { symbol, .. } => {
                let ty = SymbolResolver::validate_readable_symbol(tags, symbol, &self.name)
                    .map_err(|source| LdError::Ir {
                        pou: self.name.clone(),
                        source,
                    })?;
                if ty != IecType::Bool {
                    return Err(LdError::Ir {
                        pou: self.name.clone(),
                        source: IrError::TypeMismatch {
                            pou: self.name.clone(),
                            expected: IecType::Bool,
                            actual: ty,
                            context: format!("contact '{symbol}'"),
                        },
                    });
                }
                Ok(())
            }
            LdElement::Branch { paths } => {
                for path in paths {
                    for e in path {
                        self.validate_element(e, tags)?;
                    }
                }
                Ok(())
            }
            LdElement::InlineFb {
                instance,
                inputs,
                outputs,
                ..
            } => {
                let fb = tags.get_fb(instance).ok_or_else(|| LdError::Ir {
                    pou: self.name.clone(),
                    source: IrError::Core {
                        pou: self.name.clone(),
                        source: ferrite_core::CoreError::UnknownFbInstance(instance.clone()),
                    },
                })?;
                for (pin, val_or_sym) in inputs {
                    let pty = fb.kind.pin_type(pin).ok_or_else(|| LdError::Ir {
                        pou: self.name.clone(),
                        source: IrError::Core {
                            pou: self.name.clone(),
                            source: ferrite_core::CoreError::UnknownFbPin {
                                instance: instance.clone(),
                                pin: pin.clone(),
                            },
                        },
                    })?;
                    if IecValue::parse_literal(val_or_sym, Some(pty)).is_err() {
                        let _ =
                            SymbolResolver::validate_readable_symbol(tags, val_or_sym, &self.name)
                                .map_err(|source| LdError::Ir {
                                    pou: self.name.clone(),
                                    source,
                                })?;
                    }
                }
                for (pin, target) in outputs {
                    if fb.kind.pin_type(pin).is_none() {
                        return Err(LdError::Ir {
                            pou: self.name.clone(),
                            source: IrError::Core {
                                pou: self.name.clone(),
                                source: ferrite_core::CoreError::UnknownFbPin {
                                    instance: instance.clone(),
                                    pin: pin.clone(),
                                },
                            },
                        });
                    }
                    let _ =
                        SymbolResolver::validate_writable_symbol(tags, target, &self.name, false)
                            .map_err(|source| LdError::Ir {
                            pou: self.name.clone(),
                            source,
                        })?;
                }
                Ok(())
            }
        }
    }

    pub fn compile(&self, tags: &TagTable) -> Result<CompiledLdPou, LdError> {
        self.validate(tags)?;
        let mut edge_count = 0usize;
        let rungs = self
            .rungs
            .iter()
            .map(|r| CompiledRung {
                elements: r
                    .elements
                    .iter()
                    .map(|e| Self::compile_element(e, tags, &mut edge_count))
                    .collect(),
                coils: r.coils.clone(),
            })
            .collect();
        Ok(CompiledLdPou {
            id: PouId::new(self.name.clone()),
            rungs,
            edge_states: vec![false; edge_count],
        })
    }

    fn compile_element(
        elem: &LdElement,
        tags: &TagTable,
        edge_count: &mut usize,
    ) -> CompiledElement {
        match elem {
            LdElement::Contact { symbol, kind } => {
                let edge_idx = match kind {
                    ContactKind::RisingEdge | ContactKind::FallingEdge => {
                        let idx = *edge_count;
                        *edge_count += 1;
                        Some(idx)
                    }
                    _ => None,
                };
                CompiledElement::Contact {
                    symbol: symbol.clone(),
                    kind: *kind,
                    edge_idx,
                }
            }
            LdElement::Branch { paths } => CompiledElement::Branch {
                paths: paths
                    .iter()
                    .map(|p| {
                        p.iter()
                            .map(|e| Self::compile_element(e, tags, edge_count))
                            .collect()
                    })
                    .collect(),
            },
            LdElement::InlineFb {
                instance,
                enable_pin,
                inputs,
                outputs,
            } => {
                let fb_kind = tags.get_fb(instance).map(|f| f.kind);
                let compiled_inputs = inputs
                    .iter()
                    .map(|(pin, raw)| {
                        let pty = fb_kind.and_then(|k| k.pin_type(pin));
                        let bound = match IecValue::parse_literal(raw, pty) {
                            Ok(val) => FbInputBinding::Literal(val),
                            Err(_) => FbInputBinding::Symbol(raw.clone()),
                        };
                        (pin.clone(), bound)
                    })
                    .collect();
                CompiledElement::InlineFb {
                    instance: instance.clone(),
                    enable_pin: enable_pin.clone(),
                    inputs: compiled_inputs,
                    outputs: outputs.clone(),
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum FbInputBinding {
    Literal(IecValue),
    Symbol(String),
}

#[derive(Debug, Clone)]
enum CompiledElement {
    Contact {
        symbol: String,
        kind: ContactKind,
        edge_idx: Option<usize>,
    },
    Branch {
        paths: Vec<Vec<CompiledElement>>,
    },
    InlineFb {
        instance: String,
        enable_pin: Option<String>,
        inputs: Vec<(String, FbInputBinding)>,
        outputs: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone)]
struct CompiledRung {
    elements: Vec<CompiledElement>,
    coils: Vec<LdCoil>,
}

/// Pre-allocated, real-time safe Ladder Diagram executable POU.
#[derive(Debug, Clone)]
pub struct CompiledLdPou {
    id: PouId,
    rungs: Vec<CompiledRung>,
    edge_states: Vec<bool>,
}

impl CompiledLdPou {
    fn eval_elements(
        elements: &[CompiledElement],
        mut power: bool,
        edge_states: &mut [bool],
        ctx: &mut ScanContext<'_>,
    ) -> Result<bool, RuntimeError> {
        for elem in elements {
            power = Self::eval_element(elem, power, edge_states, ctx)?;
        }
        Ok(power)
    }

    fn eval_element(
        elem: &CompiledElement,
        power_in: bool,
        edge_states: &mut [bool],
        ctx: &mut ScanContext<'_>,
    ) -> Result<bool, RuntimeError> {
        match elem {
            CompiledElement::Contact {
                symbol,
                kind,
                edge_idx,
            } => {
                let sig = ctx.read_bool(symbol)?;
                let cond = match kind {
                    ContactKind::NormallyOpen => sig,
                    ContactKind::NormallyClosed => !sig,
                    ContactKind::RisingEdge => {
                        let idx = edge_idx.unwrap_or(0);
                        let prev = edge_states[idx];
                        edge_states[idx] = sig;
                        sig && !prev
                    }
                    ContactKind::FallingEdge => {
                        let idx = edge_idx.unwrap_or(0);
                        let prev = edge_states[idx];
                        edge_states[idx] = sig;
                        !sig && prev
                    }
                };
                Ok(power_in && cond)
            }
            CompiledElement::Branch { paths } => {
                let mut any_active = false;
                for path in paths {
                    let path_out = Self::eval_elements(path, power_in, edge_states, ctx)?;
                    if path_out {
                        any_active = true;
                    }
                }
                Ok(any_active)
            }
            CompiledElement::InlineFb {
                instance,
                enable_pin,
                inputs,
                outputs,
            } => {
                let en_pin = enable_pin.as_deref().unwrap_or("IN");
                if let Some(fb) = ctx.tags.get_fb_mut(instance) {
                    let _ = fb.write_pin(en_pin, &IecValue::Bool(power_in));
                }
                for (pin, binding) in inputs {
                    let val = match binding {
                        FbInputBinding::Literal(v) => v.clone(),
                        FbInputBinding::Symbol(sym) => ctx.read_symbol(sym)?,
                    };
                    if let Some(fb) = ctx.tags.get_fb_mut(instance) {
                        fb.write_pin(pin, &val)?;
                    }
                }
                ctx.evaluate_fb(instance)?;
                for (pin, target) in outputs {
                    let out_val = match ctx.tags.get_fb(instance) {
                        Some(fb) => fb.read_pin(pin)?,
                        None => IecValue::Bool(false),
                    };
                    ctx.write_symbol(target, out_val)?;
                }
                let q_out = match ctx.tags.get_fb(instance) {
                    Some(fb) => fb.q,
                    None => power_in,
                };
                Ok(q_out)
            }
        }
    }
}

impl ExecutablePou for CompiledLdPou {
    fn id(&self) -> PouId {
        self.id.clone()
    }

    fn kind(&self) -> LanguageKind {
        LanguageKind::Ld
    }

    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
        for rung in &self.rungs {
            let power = Self::eval_elements(&rung.elements, true, &mut self.edge_states, ctx)?;
            for coil in &rung.coils {
                match coil.kind {
                    CoilKind::Normal => {
                        ctx.write_symbol(&coil.symbol, IecValue::Bool(power))?;
                    }
                    CoilKind::Set => {
                        if power {
                            ctx.write_symbol(&coil.symbol, IecValue::Bool(true))?;
                        }
                    }
                    CoilKind::Reset => {
                        if power {
                            ctx.write_symbol(&coil.symbol, IecValue::Bool(false))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::{ProcessImage, TagEntry};

    #[test]
    fn test_ladder_interlock_and_rising_edge_mutation() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "EStop_OK",
            IecType::Bool,
            Some("%IX0.0".parse()?),
            IecValue::Bool(false),
            "NC E-Stop input",
        ))?;
        tags.register_tag(TagEntry::new(
            "Guard_Closed",
            IecType::Bool,
            Some("%IX0.1".parse()?),
            IecValue::Bool(false),
            "Safety guard switch",
        ))?;
        tags.register_tag(TagEntry::new(
            "High_Temp_Alarm",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Alarm from FBD",
        ))?;
        tags.register_tag(TagEntry::new(
            "Permissive_OK",
            IecType::Bool,
            Some("%MX0.0".parse()?),
            IecValue::Bool(false),
            "Safety permissive output for Grafcet",
        ))?;

        let ld = LdProgram {
            name: "main_safety".to_string(),
            comment: "Safety interlocks".to_string(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: "Permissive_OK := EStop_OK AND Guard_Closed AND NOT High_Temp_Alarm"
                    .to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "EStop_OK".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Guard_Closed".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "High_Temp_Alarm".to_string(),
                        kind: ContactKind::NormallyClosed,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Permissive_OK".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let mut compiled = ld.compile(&tags)?;

        img.set_hw_input("%IX0.0".parse()?, &IecValue::Bool(true))?;
        img.set_hw_input("%IX0.1".parse()?, &IecValue::Bool(true))?;
        img.latch_inputs();

        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);
        ctx.sync_from_process_image()?;
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("Permissive_OK")?);

        // Now simulate FBD setting High_Temp_Alarm = TRUE -> breaks NC contact in LD rung
        ctx.write_symbol("High_Temp_Alarm", IecValue::Bool(true))?;
        compiled.execute(&mut ctx)?;
        assert!(!ctx.read_bool("Permissive_OK")?);

        Ok(())
    }
}
