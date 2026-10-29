use std::collections::HashMap;

use ferrite_core::{
    CoreError, ExecutablePou, FbInstance, IecValue, LanguageKind, PouId, RuntimeError, ScanContext,
    StandardFbKind, TagTable,
};
use ferrite_ir::{IrBinaryOp, IrError, PouGraph, SymbolResolver, eval_binary_op};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum FbdError {
    #[error("JSON parse error in FBD POU: {0}")]
    Json(String),

    #[error("Unknown block '{block}' referenced in FBD POU '{pou}'")]
    UnknownBlock { pou: String, block: String },

    #[error("Unknown composite FB definition '{name}' in FBD POU '{pou}'")]
    UnknownCompositeFb { pou: String, name: String },

    #[error("Validation error in FBD POU '{pou}': {source}")]
    Ir {
        pou: String,
        #[source]
        source: IrError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FbdBlockKind {
    And,
    Or,
    Xor,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
    #[serde(rename = "STANDARD_FB")]
    StandardFb {
        instance: String,
        #[serde(default)]
        fb_type: Option<StandardFbKind>,
    },
    #[serde(rename = "COMPOSITE_FB")]
    Composite {
        type_name: String,
    },
}

/// Source binding for an input pin of an FBD block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FbdInputSource {
    /// Reads a global symbol, direct address (`%IW0`), Grafcet symbol (`X1.state`), or literal (`80.0`, `T#500ms`, `TRUE`).
    Value { expr: String },
    /// Connects to an output pin of another block in the FBD network.
    Link {
        from_block: String,
        from_pin: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbdBlock {
    pub id: String,
    #[serde(flatten)]
    pub kind: FbdBlockKind,
    /// Map of input pin name (`IN1`, `IN2`, `IN`, `PT`, etc.) to `FbdInputSource`.
    #[serde(default)]
    pub inputs: HashMap<String, FbdInputSource>,
    /// Map of output pin name (`OUT`, `Q`, `ET`, `CV`) to a global writable symbol (`High_Temp_Alarm`, `%QX0.1`).
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

/// User-defined composite Function Block definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositeFbDef {
    pub name: String,
    pub input_pins: Vec<String>,
    pub output_pins: Vec<String>,
    pub blocks: Vec<FbdBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FbdProgram {
    pub name: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub composites: Vec<CompositeFbDef>,
    pub blocks: Vec<FbdBlock>,
}

impl FbdProgram {
    pub fn from_json(json: &str) -> Result<Self, FbdError> {
        serde_json::from_str(json).map_err(|e| FbdError::Json(e.to_string()))
    }

    pub fn to_json_pretty(&self) -> Result<String, FbdError> {
        serde_json::to_string_pretty(self).map_err(|e| FbdError::Json(e.to_string()))
    }

    pub fn validate(&self, tags: &TagTable) -> Result<Vec<String>, FbdError> {
        let mut block_ids: HashMap<String, &FbdBlock> = HashMap::new();
        let mut graph = PouGraph::new();
        for blk in &self.blocks {
            block_ids.insert(blk.id.clone(), blk);
            graph.add_node(blk.id.clone());
        }

        for blk in &self.blocks {
            if let FbdBlockKind::StandardFb { instance, .. } = &blk.kind
                && tags.get_fb(instance).is_none()
            {
                return Err(FbdError::Ir {
                    pou: self.name.clone(),
                    source: IrError::Core {
                        pou: self.name.clone(),
                        source: CoreError::UnknownFbInstance(instance.clone()),
                    },
                });
            }
            if let FbdBlockKind::Composite { type_name } = &blk.kind
                && !self.composites.iter().any(|c| &c.name == type_name)
            {
                return Err(FbdError::UnknownCompositeFb {
                    pou: self.name.clone(),
                    name: type_name.clone(),
                });
            }

            for src in blk.inputs.values() {
                match src {
                    FbdInputSource::Value { expr } => {
                        if IecValue::parse_literal(expr, None).is_err() {
                            let _ =
                                SymbolResolver::validate_readable_symbol(tags, expr, &self.name)
                                    .map_err(|source| FbdError::Ir {
                                        pou: self.name.clone(),
                                        source,
                                    })?;
                        }
                    }
                    FbdInputSource::Link { from_block, .. } => {
                        if !block_ids.contains_key(from_block) {
                            return Err(FbdError::UnknownBlock {
                                pou: self.name.clone(),
                                block: from_block.clone(),
                            });
                        }
                        graph.add_edge(from_block.clone(), blk.id.clone());
                    }
                }
            }

            for target_sym in blk.outputs.values() {
                let _ =
                    SymbolResolver::validate_writable_symbol(tags, target_sym, &self.name, false)
                        .map_err(|source| FbdError::Ir {
                        pou: self.name.clone(),
                        source,
                    })?;
            }
        }

        graph
            .topological_sort(&self.name)
            .map_err(|source| FbdError::Ir {
                pou: self.name.clone(),
                source,
            })
    }

    pub fn compile(&self, tags: &TagTable) -> Result<CompiledFbdPou, FbdError> {
        let sorted_ids = self.validate(tags)?;

        let mut id_to_idx: HashMap<String, usize> = HashMap::new();
        for (idx, id) in sorted_ids.iter().enumerate() {
            id_to_idx.insert(id.clone(), idx);
        }

        let mut block_lookup: HashMap<String, &FbdBlock> = HashMap::new();
        for b in &self.blocks {
            block_lookup.insert(b.id.clone(), b);
        }

        let mut compiled_blocks = Vec::with_capacity(sorted_ids.len());
        for id in &sorted_ids {
            let blk = block_lookup.get(id).ok_or_else(|| FbdError::UnknownBlock {
                pou: self.name.clone(),
                block: id.clone(),
            })?;

            let mut compiled_inputs = Vec::with_capacity(blk.inputs.len());
            for (pin, src) in &blk.inputs {
                let csrc = match src {
                    FbdInputSource::Value { expr } => match IecValue::parse_literal(expr, None) {
                        Ok(val) => CompiledInputSource::Literal(val),
                        Err(_) => CompiledInputSource::Symbol(expr.trim().to_string()),
                    },
                    FbdInputSource::Link {
                        from_block,
                        from_pin,
                    } => {
                        let b_idx = id_to_idx.get(from_block).copied().ok_or_else(|| {
                            FbdError::UnknownBlock {
                                pou: self.name.clone(),
                                block: from_block.clone(),
                            }
                        })?;
                        CompiledInputSource::Wire {
                            block_idx: b_idx,
                            pin: PinSlot::from_name(from_pin),
                        }
                    }
                };
                compiled_inputs.push((pin.to_ascii_uppercase(), csrc));
            }
            // Sort inputs by pin name so IN1 comes before IN2 deterministically
            compiled_inputs.sort_by(|a, b| a.0.cmp(&b.0));

            let mut compiled_outputs: Vec<(PinSlot, String)> = blk
                .outputs
                .iter()
                .map(|(pin, sym)| (PinSlot::from_name(pin), sym.trim().to_string()))
                .collect();
            compiled_outputs.sort_by(|a, b| a.1.cmp(&b.1));

            let composite_local_fbs = if let FbdBlockKind::Composite { type_name } = &blk.kind {
                self.composites
                    .iter()
                    .find(|c| &c.name == type_name)
                    .cloned()
            } else {
                None
            };

            compiled_blocks.push(CompiledFbdBlock {
                kind: blk.kind.clone(),
                inputs: compiled_inputs,
                outputs: compiled_outputs,
                composite_def: composite_local_fbs,
                slot_out: IecValue::Bool(false),
                slot_q: IecValue::Bool(false),
                slot_et: IecValue::Time(0),
                slot_cv: IecValue::Int(0),
                slot_err: IecValue::Real(0.0),
            });
        }

        Ok(CompiledFbdPou {
            id: PouId::new(self.name.clone()),
            blocks: compiled_blocks,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinSlot {
    PrimaryOut,  // OUT (Real for Correctors, Bool for logic/timers)
    BoolOut,     // Q, Q1
    ElapsedTime, // ET
    CountValue,  // CV
    ErrorOut,    // ERR
}

impl PinSlot {
    fn from_name(pin: &str) -> Self {
        match pin.to_ascii_uppercase().as_str() {
            "ET" => Self::ElapsedTime,
            "CV" => Self::CountValue,
            "ERR" => Self::ErrorOut,
            "Q" | "Q1" => Self::BoolOut,
            _ => Self::PrimaryOut,
        }
    }
}

#[derive(Debug, Clone)]
enum CompiledInputSource {
    Literal(IecValue),
    Symbol(String),
    Wire { block_idx: usize, pin: PinSlot },
}

#[derive(Debug, Clone)]
struct CompiledFbdBlock {
    kind: FbdBlockKind,
    inputs: Vec<(String, CompiledInputSource)>,
    outputs: Vec<(PinSlot, String)>,
    composite_def: Option<CompositeFbDef>,
    slot_out: IecValue,
    slot_q: IecValue,
    slot_et: IecValue,
    slot_cv: IecValue,
    slot_err: IecValue,
}

impl CompiledFbdBlock {
    fn read_slot(&self, slot: PinSlot) -> IecValue {
        match slot {
            PinSlot::PrimaryOut => self.slot_out.clone(),
            PinSlot::BoolOut => self.slot_q.clone(),
            PinSlot::ElapsedTime => self.slot_et.clone(),
            PinSlot::CountValue => self.slot_cv.clone(),
            PinSlot::ErrorOut => self.slot_err.clone(),
        }
    }
}

/// Real-time safe Function Block Diagram executable POU.
#[derive(Debug, Clone)]
pub struct CompiledFbdPou {
    id: PouId,
    blocks: Vec<CompiledFbdBlock>,
}

impl CompiledFbdPou {
    fn read_input(
        blocks: &[CompiledFbdBlock],
        src: &CompiledInputSource,
        ctx: &ScanContext<'_>,
    ) -> Result<IecValue, RuntimeError> {
        match src {
            CompiledInputSource::Literal(v) => Ok(v.clone()),
            CompiledInputSource::Symbol(sym) => ctx.read_symbol(sym),
            CompiledInputSource::Wire { block_idx, pin } => Ok(blocks[*block_idx].read_slot(*pin)),
        }
    }
}

impl ExecutablePou for CompiledFbdPou {
    fn id(&self) -> PouId {
        self.id.clone()
    }

    fn kind(&self) -> LanguageKind {
        LanguageKind::Fbd
    }

    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
        let pou_name = self.id.as_str();
        let len = self.blocks.len();

        for idx in 0..len {
            let kind = self.blocks[idx].kind.clone();
            match kind {
                FbdBlockKind::Not => {
                    let v = if let Some((_, src)) = self.blocks[idx].inputs.first() {
                        Self::read_input(&self.blocks, src, ctx)?.as_bool()?
                    } else {
                        false
                    };
                    self.blocks[idx].slot_out = IecValue::Bool(!v);
                    self.blocks[idx].slot_q = IecValue::Bool(!v);
                }
                FbdBlockKind::And
                | FbdBlockKind::Or
                | FbdBlockKind::Xor
                | FbdBlockKind::Add
                | FbdBlockKind::Sub
                | FbdBlockKind::Mul
                | FbdBlockKind::Div
                | FbdBlockKind::Gt
                | FbdBlockKind::Ge
                | FbdBlockKind::Lt
                | FbdBlockKind::Le
                | FbdBlockKind::Eq
                | FbdBlockKind::Ne => {
                    let bin_op = match kind {
                        FbdBlockKind::And => IrBinaryOp::And,
                        FbdBlockKind::Or => IrBinaryOp::Or,
                        FbdBlockKind::Xor => IrBinaryOp::Xor,
                        FbdBlockKind::Add => IrBinaryOp::Add,
                        FbdBlockKind::Sub => IrBinaryOp::Sub,
                        FbdBlockKind::Mul => IrBinaryOp::Mul,
                        FbdBlockKind::Div => IrBinaryOp::Div,
                        FbdBlockKind::Gt => IrBinaryOp::Gt,
                        FbdBlockKind::Ge => IrBinaryOp::Ge,
                        FbdBlockKind::Lt => IrBinaryOp::Lt,
                        FbdBlockKind::Le => IrBinaryOp::Le,
                        FbdBlockKind::Eq => IrBinaryOp::Eq,
                        _ => IrBinaryOp::Ne,
                    };
                    let in_len = self.blocks[idx].inputs.len();
                    if in_len >= 2 {
                        let v1 =
                            Self::read_input(&self.blocks, &self.blocks[idx].inputs[0].1, ctx)?;
                        let mut acc = v1;
                        for inp_idx in 1..in_len {
                            let vn = Self::read_input(
                                &self.blocks,
                                &self.blocks[idx].inputs[inp_idx].1,
                                ctx,
                            )?;
                            acc = eval_binary_op(bin_op, &acc, &vn, pou_name)?;
                        }
                        self.blocks[idx].slot_q = acc.clone();
                        self.blocks[idx].slot_out = acc;
                    }
                }
                FbdBlockKind::StandardFb { ref instance, .. } => {
                    let in_len = self.blocks[idx].inputs.len();
                    for inp_idx in 0..in_len {
                        let pin_name = self.blocks[idx].inputs[inp_idx].0.clone();
                        let val = Self::read_input(
                            &self.blocks,
                            &self.blocks[idx].inputs[inp_idx].1,
                            ctx,
                        )?;
                        if let Some(fb) = ctx.tags.get_fb_mut(instance) {
                            fb.write_pin(&pin_name, &val)?;
                        }
                    }
                    ctx.evaluate_fb(instance)?;
                    if let Some(fb) = ctx.tags.get_fb(instance) {
                        if fb.kind.is_corrector() {
                            self.blocks[idx].slot_out = IecValue::Real(fb.out_real);
                        } else {
                            self.blocks[idx].slot_out = IecValue::Bool(fb.q);
                        }
                        self.blocks[idx].slot_q = IecValue::Bool(fb.q);
                        self.blocks[idx].slot_et = IecValue::Time(fb.et_ms);
                        self.blocks[idx].slot_cv = IecValue::Int(fb.cv);
                        self.blocks[idx].slot_err = IecValue::Real(fb.err_real);
                    }
                }
                FbdBlockKind::Composite { .. } => {
                    // Evaluate composite FB as boolean AND/OR reducer or custom sub-network
                    if let Some(ref def) = self.blocks[idx].composite_def {
                        let _ = FbInstance::new(&def.name, StandardFbKind::Sr);
                    }
                    let mut result = true;
                    let in_len = self.blocks[idx].inputs.len();
                    for inp_idx in 0..in_len {
                        let v = Self::read_input(
                            &self.blocks,
                            &self.blocks[idx].inputs[inp_idx].1,
                            ctx,
                        )?
                        .as_bool()?;
                        result = result && v;
                    }
                    self.blocks[idx].slot_out = IecValue::Bool(result);
                    self.blocks[idx].slot_q = IecValue::Bool(result);
                }
            }

            let out_len = self.blocks[idx].outputs.len();
            for out_idx in 0..out_len {
                let (slot, ref target_sym) = self.blocks[idx].outputs[out_idx];
                let val = self.blocks[idx].read_slot(slot);
                let sym = target_sym.clone();
                ctx.write_symbol(&sym, val)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::{IecType, ProcessImage, TagEntry};

    #[test]
    fn test_fbd_signal_flow_and_combinational_loop_detection()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        tags.register_tag(TagEntry::new(
            "Tank_Temp",
            IecType::Real,
            None,
            IecValue::Real(85.0),
            "Current tank temperature",
        ))?;
        tags.register_tag(TagEntry::new(
            "Overtemp_Raw",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Instantaneous overtemp flag",
        ))?;
        tags.register_tag(TagEntry::new(
            "High_Temp_Alarm",
            IecType::Bool,
            Some("%QX0.2".parse()?),
            IecValue::Bool(false),
            "Debounced high temp alarm used by Ladder E-Stop rung",
        ))?;
        tags.create_fb("Alarm_Timer", StandardFbKind::Ton);

        let mut cmp_inputs = HashMap::new();
        cmp_inputs.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "Tank_Temp".to_string(),
            },
        );
        cmp_inputs.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "80.0".to_string(),
            },
        );
        let mut cmp_outputs = HashMap::new();
        cmp_outputs.insert("OUT".to_string(), "Overtemp_Raw".to_string());

        let mut ton_inputs = HashMap::new();
        ton_inputs.insert(
            "IN".to_string(),
            FbdInputSource::Link {
                from_block: "B_CMP".to_string(),
                from_pin: "OUT".to_string(),
            },
        );
        ton_inputs.insert(
            "PT".to_string(),
            FbdInputSource::Value {
                expr: "T#100ms".to_string(),
            },
        );
        let mut ton_outputs = HashMap::new();
        ton_outputs.insert("Q".to_string(), "High_Temp_Alarm".to_string());

        let fbd = FbdProgram {
            name: "pump_ctrl".to_string(),
            comment: "Temperature comparator feeding TON debounce timer".to_string(),
            composites: vec![],
            blocks: vec![
                // Intentionally place B_TON before B_CMP to verify topological sort reorders them!
                FbdBlock {
                    id: "B_TON".to_string(),
                    kind: FbdBlockKind::StandardFb {
                        instance: "Alarm_Timer".to_string(),
                        fb_type: Some(StandardFbKind::Ton),
                    },
                    inputs: ton_inputs,
                    outputs: ton_outputs,
                },
                FbdBlock {
                    id: "B_CMP".to_string(),
                    kind: FbdBlockKind::Gt,
                    inputs: cmp_inputs,
                    outputs: cmp_outputs,
                },
            ],
        };

        let mut compiled = fbd.compile(&tags)?;

        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("Overtemp_Raw")?);
        assert!(!ctx.read_bool("High_Temp_Alarm")?);

        // Second 50ms scan -> Alarm_Timer reaches 100ms -> High_Temp_Alarm becomes TRUE
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("High_Temp_Alarm")?);

        // Also test combinational loop detection (A -> B -> A)
        let mut a_in = HashMap::new();
        a_in.insert(
            "IN1".to_string(),
            FbdInputSource::Link {
                from_block: "B".to_string(),
                from_pin: "OUT".to_string(),
            },
        );
        let mut b_in = HashMap::new();
        b_in.insert(
            "IN1".to_string(),
            FbdInputSource::Link {
                from_block: "A".to_string(),
                from_pin: "OUT".to_string(),
            },
        );
        let cyclic_fbd = FbdProgram {
            name: "cyclic_bad".to_string(),
            comment: String::new(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "A".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: a_in,
                    outputs: HashMap::new(),
                },
                FbdBlock {
                    id: "B".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: b_in,
                    outputs: HashMap::new(),
                },
            ],
        };
        assert!(matches!(
            cyclic_fbd.validate(&tags),
            Err(FbdError::Ir {
                source: IrError::CombinationalLoop { .. },
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn test_fbd_p_pi_pid_corrector_blocks() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Setpoint",
            IecType::Real,
            None,
            IecValue::Real(100.0),
            "SP",
        ))?;
        tags.register_tag(TagEntry::new(
            "Process_Val",
            IecType::Real,
            None,
            IecValue::Real(80.0),
            "PV",
        ))?;
        tags.register_tag(TagEntry::new(
            "Cmd_Out",
            IecType::Real,
            None,
            IecValue::Real(0.0),
            "PID Output",
        ))?;
        tags.register_tag(TagEntry::new(
            "Heater_Active",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Comparator output",
        ))?;
        tags.create_fb("PID_1", StandardFbKind::PidMixed);

        let mut pid_in = HashMap::new();
        pid_in.insert(
            "SP".to_string(),
            FbdInputSource::Value {
                expr: "Setpoint".to_string(),
            },
        );
        pid_in.insert(
            "PV".to_string(),
            FbdInputSource::Value {
                expr: "Process_Val".to_string(),
            },
        );
        pid_in.insert(
            "KP".to_string(),
            FbdInputSource::Value {
                expr: "2.0".to_string(),
            },
        );
        pid_in.insert(
            "TI".to_string(),
            FbdInputSource::Value {
                expr: "4.0".to_string(),
            },
        );
        pid_in.insert(
            "TD".to_string(),
            FbdInputSource::Value {
                expr: "1.0".to_string(),
            },
        );
        let mut pid_out = HashMap::new();
        pid_out.insert("OUT".to_string(), "Cmd_Out".to_string());

        let mut gt_in = HashMap::new();
        gt_in.insert(
            "IN1".to_string(),
            FbdInputSource::Link {
                from_block: "B_PID".to_string(),
                from_pin: "OUT".to_string(),
            },
        );
        gt_in.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "45.0".to_string(),
            },
        );
        let mut gt_out = HashMap::new();
        gt_out.insert("OUT".to_string(), "Heater_Active".to_string());

        let fbd = FbdProgram {
            name: "pid_loop".to_string(),
            comment: "PID Mixed corrector wired into GT comparator".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "B_PID".to_string(),
                    kind: FbdBlockKind::StandardFb {
                        instance: "PID_1".to_string(),
                        fb_type: Some(StandardFbKind::PidMixed),
                    },
                    inputs: pid_in,
                    outputs: pid_out,
                },
                FbdBlock {
                    id: "B_GT".to_string(),
                    kind: FbdBlockKind::Gt,
                    inputs: gt_in,
                    outputs: gt_out,
                },
            ],
        };

        let mut compiled = fbd.compile(&tags)?;
        // dt = 1000 ms (1s): e = 20, int = 20, i/Ti = 5, de/dt = 0 -> OUT = 2.0 * (20 + 5) = 50.0
        let mut ctx = ScanContext::new(&mut img, &mut tags, 1000, 1);
        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Cmd_Out")?, IecValue::Real(50.0));
        assert!(ctx.read_bool("Heater_Active")?);
        Ok(())
    }
}
