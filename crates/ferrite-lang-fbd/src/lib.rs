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

    #[error("Duplicate block ID '{0}'")]
    DuplicateBlockId(String),

    #[error("Invalid block ID '{0}'")]
    InvalidBlockId(String),

    #[error("Validation error: {0}")]
    Validation(String),
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
    #[serde(default)]
    pub description: String,
    pub input_pins: Vec<String>,
    pub output_pins: Vec<String>,
    #[serde(default)]
    pub st_code: Option<String>,
    #[serde(default)]
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

    /// Renames a block within the FBD POU and automatically rewires all downstream
    /// `FbdInputSource::Link` references targeting `old_id` to `new_id`.
    pub fn rename_block(&mut self, old_id: &str, new_id: &str) -> Result<(), FbdError> {
        let trimmed_new = new_id.trim();
        if trimmed_new.is_empty()
            || trimmed_new.contains('.')
            || !trimmed_new.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(FbdError::InvalidBlockId(new_id.to_string()));
        }
        if old_id == trimmed_new {
            return Ok(());
        }
        if self.blocks.iter().any(|b| b.id == trimmed_new) {
            return Err(FbdError::DuplicateBlockId(trimmed_new.to_string()));
        }
        let Some(block) = self.blocks.iter_mut().find(|b| b.id == old_id) else {
            return Err(FbdError::UnknownBlock {
                pou: self.name.clone(),
                block: old_id.to_string(),
            });
        };
        block.id = trimmed_new.to_string();

        // Rewire all downstream links referencing old_id
        for blk in &mut self.blocks {
            for src in blk.inputs.values_mut() {
                if let FbdInputSource::Link { from_block, .. } = src
                    && from_block == old_id
                {
                    *from_block = trimmed_new.to_string();
                }
            }
        }
        for comp in &mut self.composites {
            for blk in &mut comp.blocks {
                for src in blk.inputs.values_mut() {
                    if let FbdInputSource::Link { from_block, .. } = src
                        && from_block == old_id
                    {
                        *from_block = trimmed_new.to_string();
                    }
                }
            }
        }

        Ok(())
    }

    /// Moves a block up one position in the evaluation order. Returns true if swapped.
    pub fn move_block_up(&mut self, index: usize) -> bool {
        if index > 0 && index < self.blocks.len() {
            self.blocks.swap(index, index - 1);
            true
        } else {
            false
        }
    }

    /// Moves a block down one position in the evaluation order. Returns true if swapped.
    pub fn move_block_down(&mut self, index: usize) -> bool {
        if index + 1 < self.blocks.len() {
            self.blocks.swap(index, index + 1);
            true
        } else {
            false
        }
    }

    pub fn validate(&self, tags: &TagTable) -> Result<Vec<String>, FbdError> {
        let mut block_ids: HashMap<String, &FbdBlock> = HashMap::new();
        let mut graph = PouGraph::new();
        for blk in &self.blocks {
            if block_ids.insert(blk.id.clone(), blk).is_some() {
                return Err(FbdError::DuplicateBlockId(blk.id.clone()));
            }
            graph.add_node(blk.id.clone());
        }

        // I-9: Reject composite FBs with sub-network blocks if st_code is not provided
        for comp in &self.composites {
            if comp.st_code.is_none() && !comp.blocks.is_empty() {
                return Err(FbdError::Validation(format!(
                    "Composite FB '{}': graphical sub-network execution is not supported; provide st_code implementation",
                    comp.name
                )));
            }
        }

        for blk in &self.blocks {
            match &blk.kind {
                FbdBlockKind::Not => {
                    if blk.inputs.len() != 1 {
                        return Err(FbdError::Validation(format!(
                            "NOT block '{}' expects exactly 1 input, found {}",
                            blk.id,
                            blk.inputs.len()
                        )));
                    }
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
                    if blk.inputs.len() < 2 {
                        return Err(FbdError::Validation(format!(
                            "Binary operation block '{}' ({:?}) expects at least 2 inputs, found {}",
                            blk.id,
                            blk.kind,
                            blk.inputs.len()
                        )));
                    }
                }
                FbdBlockKind::StandardFb { instance, .. } => {
                    let Some(fb) = tags.get_fb(instance) else {
                        return Err(FbdError::Ir {
                            pou: self.name.clone(),
                            source: IrError::Core {
                                pou: self.name.clone(),
                                source: CoreError::UnknownFbInstance(instance.clone()),
                            },
                        });
                    };
                    let has_pin = |pin: &str| {
                        blk.inputs.keys().any(|k| k.eq_ignore_ascii_case(pin))
                    };
                    let missing = match fb.kind {
                        StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp => {
                            if !has_pin("IN") { Some("IN") } else { None }
                        }
                        StandardFbKind::Ctu => {
                            if !has_pin("CU") { Some("CU") } else { None }
                        }
                        StandardFbKind::Ctd => {
                            if !has_pin("CD") { Some("CD") } else { None }
                        }
                        StandardFbKind::Ctud => {
                            if !has_pin("CU") {
                                Some("CU")
                            } else if !has_pin("CD") {
                                Some("CD")
                            } else {
                                None
                            }
                        }
                        StandardFbKind::RTrig | StandardFbKind::FTrig => {
                            if !has_pin("CLK") && !has_pin("IN") { Some("CLK") } else { None }
                        }
                        StandardFbKind::Sr => {
                            if !has_pin("S1") && !has_pin("S") {
                                Some("S1")
                            } else if !has_pin("R") {
                                Some("R")
                            } else {
                                None
                            }
                        }
                        StandardFbKind::Rs => {
                            if !has_pin("S") {
                                Some("S")
                            } else if !has_pin("R1") && !has_pin("R") {
                                Some("R1")
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some(missing_pin) = missing {
                        return Err(FbdError::Validation(format!(
                            "Block '{}' ({}) missing required input pin '{missing_pin}'",
                            blk.id,
                            fb.kind.label()
                        )));
                    }
                }
                FbdBlockKind::Composite { type_name } => {
                    if !self.composites.iter().any(|c| &c.name == type_name) {
                        return Err(FbdError::UnknownCompositeFb {
                            pou: self.name.clone(),
                            name: type_name.clone(),
                        });
                    }
                }
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

            for (pin_name, target_sym) in &blk.outputs {
                let pin_upper = pin_name.to_ascii_uppercase();
                let is_valid_pin = match &blk.kind {
                    FbdBlockKind::Not
                    | FbdBlockKind::And
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
                        matches!(pin_upper.as_str(), "OUT" | "Q" | "Q1")
                    }
                    FbdBlockKind::StandardFb { instance, .. } => {
                        if let Some(fb) = tags.get_fb(instance) {
                            fb.kind.pin_type(&pin_upper).is_some()
                        } else {
                            false
                        }
                    }
                    FbdBlockKind::Composite { type_name } => {
                        if let Some(comp) = self.composites.iter().find(|c| &c.name == type_name) {
                            comp.output_pins.iter().any(|p| p.eq_ignore_ascii_case(&pin_upper))
                        } else {
                            false
                        }
                    }
                };
                if !is_valid_pin {
                    return Err(FbdError::Validation(format!(
                        "Block '{}' has unknown or invalid output pin '{pin_name}'",
                        blk.id
                    )));
                }

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

            let compiled_st_stmts = if let Some(ref def) = composite_local_fbs {
                if let Some(ref code) = def.st_code {
                    let st_stmts = ferrite_lang_st::parse_st_statements(code, &def.name)
                        .map_err(|e| FbdError::Validation(format!("ST syntax error in composite block '{}': {e}", def.name)))?;
                    Some(st_stmts)
                } else {
                    None
                }
            } else {
                None
            };

            compiled_blocks.push(CompiledFbdBlock {
                kind: blk.kind.clone(),
                inputs: compiled_inputs,
                outputs: compiled_outputs,
                composite_def: composite_local_fbs,
                compiled_st_stmts,
                slot_out: IecValue::Bool(false),
                slot_q: IecValue::Bool(false),
                slot_qu: IecValue::Bool(false),
                slot_qd: IecValue::Bool(false),
                slot_et: IecValue::Time(0),
                slot_cv: IecValue::Int(0),
                slot_err: IecValue::Real(0.0),
                custom_slots: HashMap::new(),
                composite_env: HashMap::new(),
            });
        }

        Ok(CompiledFbdPou {
            id: PouId::new(self.name.clone()),
            blocks: compiled_blocks,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PinSlot {
    PrimaryOut,   // OUT (Real for Correctors/RampReal/ScaleLin, Bool for logic/timers)
    BoolOut,      // Q, Q1
    CountUpOut,   // QU
    CountDownOut, // QD
    ElapsedTime,  // ET
    CountValue,   // CV
    ErrorOut,     // ERR
    Named(String),
}

impl PinSlot {
    fn from_name(pin: &str) -> Self {
        match pin.to_ascii_uppercase().as_str() {
            "ET" => Self::ElapsedTime,
            "CV" => Self::CountValue,
            "ERR" => Self::ErrorOut,
            "QU" => Self::CountUpOut,
            "QD" => Self::CountDownOut,
            "Q" | "Q1" => Self::BoolOut,
            "OUT" => Self::PrimaryOut,
            _ => Self::Named(pin.to_string()),
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
    compiled_st_stmts: Option<Vec<ferrite_ir::IrStmt>>,
    slot_out: IecValue,
    slot_q: IecValue,
    slot_qu: IecValue,
    slot_qd: IecValue,
    slot_et: IecValue,
    slot_cv: IecValue,
    slot_err: IecValue,
    custom_slots: HashMap<String, IecValue>,
    composite_env: HashMap<String, IecValue>,
}

impl CompiledFbdBlock {
    fn read_slot(&self, slot: &PinSlot) -> IecValue {
        match slot {
            PinSlot::PrimaryOut => self.slot_out.clone(),
            PinSlot::BoolOut => self.slot_q.clone(),
            PinSlot::CountUpOut => self.slot_qu.clone(),
            PinSlot::CountDownOut => self.slot_qd.clone(),
            PinSlot::ElapsedTime => self.slot_et.clone(),
            PinSlot::CountValue => self.slot_cv.clone(),
            PinSlot::ErrorOut => self.slot_err.clone(),
            PinSlot::Named(name) => {
                for (k, v) in &self.custom_slots {
                    if k.eq_ignore_ascii_case(name) {
                        return v.clone();
                    }
                }
                self.slot_out.clone()
            }
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
            CompiledInputSource::Wire { block_idx, pin } => Ok(blocks[*block_idx].read_slot(pin)),
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
            let (prev_blocks, rest) = self.blocks.split_at_mut(idx);
            let Some(current_block) = rest.first_mut() else {
                continue;
            };
            match &current_block.kind {
                FbdBlockKind::Not => {
                    let v = if let Some((_, src)) = current_block.inputs.first() {
                        Self::read_input(prev_blocks, src, ctx)?.as_bool()?
                    } else {
                        false
                    };
                    current_block.slot_out = IecValue::Bool(!v);
                    current_block.slot_q = IecValue::Bool(!v);
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
                    let bin_op = match current_block.kind {
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
                    let in_len = current_block.inputs.len();
                    if in_len >= 2 {
                        let v1 =
                            Self::read_input(prev_blocks, &current_block.inputs[0].1, ctx)?;
                        let mut acc = v1;
                        for inp_idx in 1..in_len {
                            let vn = Self::read_input(
                                prev_blocks,
                                &current_block.inputs[inp_idx].1,
                                ctx,
                            )?;
                            acc = eval_binary_op(bin_op, &acc, &vn, pou_name)?;
                        }
                        current_block.slot_q = acc.clone();
                        current_block.slot_out = acc;
                    }
                }
                FbdBlockKind::StandardFb { instance, .. } => {
                    let in_len = current_block.inputs.len();
                    for inp_idx in 0..in_len {
                        let val = Self::read_input(
                            prev_blocks,
                            &current_block.inputs[inp_idx].1,
                            ctx,
                        )?;
                        let pin_name = &current_block.inputs[inp_idx].0;
                        if let Some(fb) = ctx.tags.get_fb_mut(instance) {
                            fb.write_pin(pin_name, &val)?;
                        }
                    }
                    ctx.evaluate_fb(instance)?;
                    if let Some(fb) = ctx.tags.get_fb(instance) {
                        if fb.kind.is_corrector()
                            || matches!(
                                fb.kind,
                                StandardFbKind::RampReal | StandardFbKind::ScaleLin
                            )
                        {
                            current_block.slot_out = IecValue::Real(fb.out_real);
                        } else {
                            current_block.slot_out = IecValue::Bool(fb.q);
                        }
                        current_block.slot_q = IecValue::Bool(fb.q);
                        current_block.slot_qu = IecValue::Bool(fb.qu);
                        current_block.slot_qd = IecValue::Bool(fb.qd);
                        current_block.slot_et = IecValue::Time(fb.et_ms);
                        current_block.slot_cv = IecValue::Int(fb.cv);
                        current_block.slot_err = if fb.kind == StandardFbKind::ScaleLin {
                            IecValue::Bool(fb.q)
                        } else {
                            IecValue::Real(fb.err_real)
                        };
                    }
                }
                FbdBlockKind::Composite { .. } => {
                    if let Some(ref st_stmts) = current_block.compiled_st_stmts {
                        current_block.composite_env.clear();
                        let in_len = current_block.inputs.len();
                        for inp_idx in 0..in_len {
                            let (ref pin_name, ref src) = current_block.inputs[inp_idx];
                            let val = Self::read_input(prev_blocks, src, ctx)?;
                            current_block.composite_env.insert(pin_name.clone(), val);
                        }
                        let output_pins = current_block
                            .composite_def
                            .as_ref()
                            .map(|d| d.output_pins.clone())
                            .unwrap_or_default();
                        for out_pin in &output_pins {
                            current_block
                                .composite_env
                                .entry(out_pin.clone())
                                .or_insert(IecValue::Bool(false));
                        }
                        for stmt in st_stmts {
                            stmt.execute_with_env(ctx, &mut current_block.composite_env, pou_name)?;
                        }
                        let mut primary_set = false;
                        let out_count = output_pins.len();
                        for out_pin in &output_pins {
                            let mut out_val = None;
                            for (k, v) in &current_block.composite_env {
                                if k.eq_ignore_ascii_case(out_pin) {
                                    out_val = Some(v.clone());
                                    break;
                                }
                            }
                            if let Some(v) = out_val {
                                current_block.custom_slots.insert(out_pin.clone(), v.clone());
                                if out_pin.eq_ignore_ascii_case("OUT")
                                    || (!primary_set && out_count == 1)
                                {
                                    current_block.slot_out = v.clone();
                                    primary_set = true;
                                }
                                if out_pin.eq_ignore_ascii_case("Q") {
                                    current_block.slot_q = v.clone();
                                }
                            }
                        }
                        if !primary_set {
                            if let Some(v) = current_block.composite_env.get("OUT") {
                                current_block.slot_out = v.clone();
                            } else if let Some(v) = current_block.composite_env.get("Q") {
                                current_block.slot_out = v.clone();
                                current_block.slot_q = v.clone();
                            }
                        }
                    } else {
                        // Evaluate composite FB as boolean AND/OR reducer or custom sub-network
                        if let Some(ref def) = current_block.composite_def {
                            let _ = FbInstance::new(&def.name, StandardFbKind::Sr);
                        }
                        let mut result = true;
                        let in_len = current_block.inputs.len();
                        for inp_idx in 0..in_len {
                            let v = Self::read_input(
                                prev_blocks,
                                &current_block.inputs[inp_idx].1,
                                ctx,
                            )?
                            .as_bool()?;
                            result = result && v;
                        }
                        current_block.slot_out = IecValue::Bool(result);
                        current_block.slot_q = IecValue::Bool(result);
                    }
                }
            }

            for (slot, target_sym) in &current_block.outputs {
                let val = current_block.read_slot(slot);
                ctx.write_symbol(target_sym, val)?;
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

    #[test]
    fn test_fbd_rename_block_rewires_links() {
        let mut blk_b_inputs = HashMap::new();
        blk_b_inputs.insert(
            "IN1".to_string(),
            FbdInputSource::Link {
                from_block: "A".to_string(),
                from_pin: "OUT".to_string(),
            },
        );

        let mut prog = FbdProgram {
            name: "test_pou".to_string(),
            comment: "".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "A".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
                FbdBlock {
                    id: "B".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: blk_b_inputs,
                    outputs: HashMap::new(),
                },
            ],
        };

        // Error on empty/whitespace ID
        assert!(matches!(
            prog.rename_block("A", "   "),
            Err(FbdError::InvalidBlockId(_))
        ));

        // Error on ID containing dot or invalid characters
        assert!(matches!(
            prog.rename_block("A", "Invalid.Dot"),
            Err(FbdError::InvalidBlockId(_))
        ));
        assert!(matches!(
            prog.rename_block("A", "Invalid/Slash"),
            Err(FbdError::InvalidBlockId(_))
        ));

        // Error on duplicate ID
        assert!(matches!(
            prog.rename_block("A", "B"),
            Err(FbdError::DuplicateBlockId(_))
        ));

        // Error on unknown block
        assert!(matches!(
            prog.rename_block("NonExistent", "NewName"),
            Err(FbdError::UnknownBlock { .. })
        ));

        // Successful rename and link rewiring
        assert!(prog.rename_block("A", "Sensor_Block").is_ok());
        assert_eq!(prog.blocks[0].id, "Sensor_Block");

        // Verify downstream link was rewired
        match &prog.blocks[1].inputs["IN1"] {
            FbdInputSource::Link { from_block, from_pin } => {
                assert_eq!(from_block, "Sensor_Block");
                assert_eq!(from_pin, "OUT");
            }
            _ => panic!("Expected FbdInputSource::Link"),
        }

        // Renaming to same name is a no-op Ok(())
        assert!(prog.rename_block("Sensor_Block", "Sensor_Block").is_ok());
    }

    #[test]
    fn test_fbd_reorder_blocks() {
        let mut prog = FbdProgram {
            name: "test_order".to_string(),
            comment: "".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "A".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
                FbdBlock {
                    id: "B".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
                FbdBlock {
                    id: "C".to_string(),
                    kind: FbdBlockKind::Or,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
            ],
        };

        // Move A down: [A, B, C] -> [B, A, C]
        assert!(prog.move_block_down(0));
        assert_eq!(prog.blocks[0].id, "B");
        assert_eq!(prog.blocks[1].id, "A");
        assert_eq!(prog.blocks[2].id, "C");

        // Move C up: [B, A, C] -> [B, C, A]
        assert!(prog.move_block_up(2));
        assert_eq!(prog.blocks[0].id, "B");
        assert_eq!(prog.blocks[1].id, "C");
        assert_eq!(prog.blocks[2].id, "A");

        // Out of bound moves return false
        assert!(!prog.move_block_up(0));
        assert!(!prog.move_block_down(2));
        assert!(!prog.move_block_up(99));
        assert!(!prog.move_block_down(99));
    }

    #[test]
    fn test_composite_st_block_compilation_and_execution() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        tags.register_tag(TagEntry::new(
            "Permissive_A",
            IecType::Bool,
            None,
            IecValue::Bool(true),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Permissive_B",
            IecType::Bool,
            None,
            IecValue::Bool(true),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Safety_Status",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;

        let custom_def = CompositeFbDef {
            name: "SafetyGate".to_string(),
            description: "2-channel safety validator".to_string(),
            input_pins: vec!["CH1".to_string(), "CH2".to_string()],
            output_pins: vec!["OK".to_string()],
            st_code: Some("IF CH1 AND CH2 THEN\n    OK := TRUE;\nELSE\n    OK := FALSE;\nEND_IF;".to_string()),
            blocks: vec![],
        };

        let mut block = FbdBlock {
            id: "GATE1".to_string(),
            kind: FbdBlockKind::Composite {
                type_name: "SafetyGate".to_string(),
            },
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        };
        block.inputs.insert("CH1".to_string(), FbdInputSource::Value { expr: "Permissive_A".to_string() });
        block.inputs.insert("CH2".to_string(), FbdInputSource::Value { expr: "Permissive_B".to_string() });
        block.outputs.insert("OK".to_string(), "Safety_Status".to_string());

        let prog = FbdProgram {
            name: "SafetyCheck".to_string(),
            comment: "".to_string(),
            composites: vec![custom_def],
            blocks: vec![block],
        };

        let mut compiled = prog.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);

        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Safety_Status")?, IecValue::Bool(true));

        // Now change Permissive_B to FALSE and re-execute
        ctx.write_symbol("Permissive_B", IecValue::Bool(false))?;
        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Safety_Status")?, IecValue::Bool(false));

        Ok(())
    }

    #[test]
    fn test_i8_fbd_duplicate_block_id_rejected() {
        let tags = TagTable::new();
        let mut inp1 = HashMap::new();
        inp1.insert("IN".to_string(), FbdInputSource::Value { expr: "TRUE".to_string() });
        let mut inp2 = HashMap::new();
        inp2.insert("IN".to_string(), FbdInputSource::Value { expr: "FALSE".to_string() });

        let prog = FbdProgram {
            name: "dup_test".to_string(),
            comment: "".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "BLK1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: inp1,
                    outputs: HashMap::new(),
                },
                FbdBlock {
                    id: "BLK1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: inp2,
                    outputs: HashMap::new(),
                },
            ],
        };
        assert_eq!(prog.validate(&tags), Err(FbdError::DuplicateBlockId("BLK1".to_string())));
    }

    #[test]
    fn test_i9_composite_subnetwork_without_st_code_rejected() {
        let tags = TagTable::new();
        let composite = CompositeFbDef {
            name: "SubNetComp".to_string(),
            description: "".to_string(),
            input_pins: vec!["IN1".to_string()],
            output_pins: vec!["OUT".to_string()],
            st_code: None,
            blocks: vec![
                FbdBlock {
                    id: "SUB1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
            ],
        };
        let prog = FbdProgram {
            name: "comp_test".to_string(),
            comment: "".to_string(),
            composites: vec![composite],
            blocks: vec![],
        };
        let res = prog.validate(&tags);
        assert!(matches!(res, Err(FbdError::Validation(msg)) if msg.contains("graphical sub-network execution is not supported")));
    }

    #[test]
    fn test_fbd_arity_and_pin_validation() {
        let tags = TagTable::new();

        // NOT block with 0 inputs
        let not_zero = FbdProgram {
            name: "test_not".to_string(),
            comment: "".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "N1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
            ],
        };
        assert!(matches!(not_zero.validate(&tags), Err(FbdError::Validation(msg)) if msg.contains("expects exactly 1 input")));

        // AND block with 1 input
        let mut and_inputs = HashMap::new();
        and_inputs.insert("IN1".to_string(), FbdInputSource::Value { expr: "TRUE".to_string() });
        let and_one = FbdProgram {
            name: "test_and".to_string(),
            comment: "".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "A1".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: and_inputs,
                    outputs: HashMap::new(),
                },
            ],
        };
        assert!(matches!(and_one.validate(&tags), Err(FbdError::Validation(msg)) if msg.contains("expects at least 2 inputs")));

        // Block with unknown output pin
        let mut not_valid_in = HashMap::new();
        not_valid_in.insert("IN".to_string(), FbdInputSource::Value { expr: "TRUE".to_string() });
        let mut bad_out = HashMap::new();
        bad_out.insert("INVALID_OUT_PIN".to_string(), "Some_Tag".to_string());
        let bad_out_prog = FbdProgram {
            name: "test_bad_out".to_string(),
            comment: "".to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "N2".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: not_valid_in,
                    outputs: bad_out,
                },
            ],
        };
        assert!(matches!(bad_out_prog.validate(&tags), Err(FbdError::Validation(msg)) if msg.contains("unknown or invalid output pin")));
    }
}
