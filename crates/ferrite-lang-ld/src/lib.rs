use std::collections::HashSet;

use ferrite_core::{
    ExecutablePou, IecType, IecValue, LanguageKind, PouId, RuntimeError, ScanContext, TagTable,
};
use ferrite_ir::{IrBinaryOp, IrError, PouGraph, SymbolResolver, eval_binary_op, types_compatible};
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
    #[serde(alias = "negated", alias = "not", alias = "/")]
    Negated,
    #[serde(alias = "S", alias = "set", alias = "latch")]
    Set,
    #[serde(alias = "R", alias = "reset", alias = "unlatch")]
    Reset,
    #[serde(alias = "P", alias = "rising_edge", alias = "RISING")]
    RisingEdge,
    #[serde(alias = "N", alias = "falling_edge", alias = "FALLING")]
    FallingEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LdCompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl LdCompareOp {
    pub const ALL: [Self; 6] = [Self::Eq, Self::Ne, Self::Gt, Self::Ge, Self::Lt, Self::Le];

    pub fn label(self) -> &'static str {
        match self {
            Self::Eq => "EQ (==)",
            Self::Ne => "NE (<>)",
            Self::Gt => "GT (>)",
            Self::Ge => "GE (>=)",
            Self::Lt => "LT (<)",
            Self::Le => "LE (<=)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Eq => "EQ",
            Self::Ne => "NE",
            Self::Gt => "GT",
            Self::Ge => "GE",
            Self::Lt => "LT",
            Self::Le => "LE",
        }
    }

    pub fn to_ir_op(self) -> IrBinaryOp {
        match self {
            Self::Eq => IrBinaryOp::Eq,
            Self::Ne => IrBinaryOp::Ne,
            Self::Gt => IrBinaryOp::Gt,
            Self::Ge => IrBinaryOp::Ge,
            Self::Lt => IrBinaryOp::Lt,
            Self::Le => IrBinaryOp::Le,
        }
    }
}

impl std::fmt::Display for LdCompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LdMathOp {
    Move,
    Add,
    Sub,
    Mul,
    Div,
}

impl LdMathOp {
    pub const ALL: [Self; 5] = [Self::Move, Self::Add, Self::Sub, Self::Mul, Self::Div];

    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "MOVE",
            Self::Add => "ADD (+)",
            Self::Sub => "SUB (-)",
            Self::Mul => "MUL (*)",
            Self::Div => "DIV (/)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Move => "MOVE",
            Self::Add => "ADD",
            Self::Sub => "SUB",
            Self::Mul => "MUL",
            Self::Div => "DIV",
        }
    }
}

impl std::fmt::Display for LdMathOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
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
    Compare {
        op: LdCompareOp,
        left: String,
        right: String,
    },
    MathMove {
        op: LdMathOp,
        input1: String,
        #[serde(default)]
        input2: Option<String>,
        dest: String,
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
        let mut rung_ids = HashSet::new();
        for rung in &self.rungs {
            if !rung_ids.insert(&rung.id) {
                return Err(LdError::Ir {
                    pou: self.name.clone(),
                    source: IrError::Validation {
                        pou: self.name.clone(),
                        message: format!("Duplicate rung ID '{}'", rung.id),
                    },
                });
            }
        }

        let mut graph = PouGraph::new();
        for rung in &self.rungs {
            graph.add_node(&rung.id);
        }

        let mut rung_writes: Vec<(&str, HashSet<String>)> = Vec::with_capacity(self.rungs.len());
        let mut rung_reads: Vec<(&str, HashSet<String>)> = Vec::with_capacity(self.rungs.len());

        for rung in &self.rungs {
            let mut writes = HashSet::new();
            let mut reads = HashSet::new();

            for coil in &rung.coils {
                writes.insert(coil.symbol.trim().to_string());
            }
            for elem in &rung.elements {
                Self::collect_element_symbols(elem, &mut reads, &mut writes);
            }

            rung_writes.push((&rung.id, writes));
            rung_reads.push((&rung.id, reads));
        }

        for (id_a, writes_a) in &rung_writes {
            for (id_b, reads_b) in &rung_reads {
                if id_a != id_b {
                    for sym in writes_a {
                        if reads_b.contains(sym) {
                            graph.add_edge(*id_a, *id_b);
                        }
                    }
                }
            }
        }

        graph
            .topological_sort(&self.name)
            .map_err(|source| LdError::Ir {
                pou: self.name.clone(),
                source,
            })?;

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

    fn collect_element_symbols(
        elem: &LdElement,
        reads: &mut HashSet<String>,
        writes: &mut HashSet<String>,
    ) {
        match elem {
            LdElement::Contact { symbol, .. } => {
                reads.insert(symbol.trim().to_string());
            }
            LdElement::Branch { paths } => {
                for path in paths {
                    for e in path {
                        Self::collect_element_symbols(e, reads, writes);
                    }
                }
            }
            LdElement::InlineFb {
                inputs,
                outputs,
                ..
            } => {
                for (_, val_or_sym) in inputs {
                    if IecValue::parse_literal(val_or_sym, None).is_err() {
                        reads.insert(val_or_sym.trim().to_string());
                    }
                }
                for (_, target) in outputs {
                    writes.insert(target.trim().to_string());
                }
            }
            LdElement::Compare { left, right, .. } => {
                if IecValue::parse_literal(left, None).is_err() {
                    reads.insert(left.trim().to_string());
                }
                if IecValue::parse_literal(right, None).is_err() {
                    reads.insert(right.trim().to_string());
                }
            }
            LdElement::MathMove {
                input1,
                input2,
                dest,
                ..
            } => {
                writes.insert(dest.trim().to_string());
                if IecValue::parse_literal(input1, None).is_err() {
                    reads.insert(input1.trim().to_string());
                }
                if let Some(i2) = input2
                    && IecValue::parse_literal(i2, None).is_err()
                {
                    reads.insert(i2.trim().to_string());
                }
            }
        }
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
                enable_pin,
                inputs,
                outputs,
            } => {
                let fb = tags.get_fb(instance).ok_or_else(|| LdError::Ir {
                    pou: self.name.clone(),
                    source: IrError::Core {
                        pou: self.name.clone(),
                        source: ferrite_core::CoreError::UnknownFbInstance(instance.clone()),
                    },
                })?;
                let en_pin = enable_pin.as_deref().unwrap_or("IN");
                let en_pty = fb.kind.pin_type(en_pin).ok_or_else(|| LdError::Ir {
                    pou: self.name.clone(),
                    source: IrError::Core {
                        pou: self.name.clone(),
                        source: ferrite_core::CoreError::UnknownFbPin {
                            instance: instance.clone(),
                            pin: en_pin.to_string(),
                        },
                    },
                })?;
                if en_pty != IecType::Bool {
                    return Err(LdError::Ir {
                        pou: self.name.clone(),
                        source: IrError::TypeMismatch {
                            pou: self.name.clone(),
                            expected: IecType::Bool,
                            actual: en_pty,
                            context: format!("FB '{instance}' enable pin '{en_pin}'"),
                        },
                    });
                }
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
            LdElement::Compare { op, left, right } => {
                let lty = if let Ok(v) = IecValue::parse_literal(left, None) {
                    v.iec_type()
                } else {
                    SymbolResolver::validate_readable_symbol(tags, left, &self.name)
                        .map_err(|source| LdError::Ir {
                            pou: self.name.clone(),
                            source,
                        })?
                };
                let rty = if let Ok(v) = IecValue::parse_literal(right, None) {
                    v.iec_type()
                } else {
                    SymbolResolver::validate_readable_symbol(tags, right, &self.name)
                        .map_err(|source| LdError::Ir {
                            pou: self.name.clone(),
                            source,
                        })?
                };
                if (lty == IecType::Bool && rty != IecType::Bool)
                    || (rty == IecType::Bool && lty != IecType::Bool)
                    || !types_compatible(lty, rty)
                {
                    return Err(LdError::Ir {
                        pou: self.name.clone(),
                        source: IrError::TypeMismatch {
                            pou: self.name.clone(),
                            expected: lty,
                            actual: rty,
                            context: format!("Compare {op:?} '{left}' vs '{right}'"),
                        },
                    });
                }
                Ok(())
            }
            LdElement::MathMove {
                op,
                input1,
                input2,
                dest,
            } => {
                let dest_ty = SymbolResolver::validate_writable_symbol(tags, dest, &self.name, false)
                    .map_err(|source| LdError::Ir {
                        pou: self.name.clone(),
                        source,
                    })?;
                let in1_ty = if let Ok(v) = IecValue::parse_literal(input1, Some(dest_ty)) {
                    v.iec_type()
                } else if let Ok(v) = IecValue::parse_literal(input1, None) {
                    v.iec_type()
                } else {
                    SymbolResolver::validate_readable_symbol(tags, input1, &self.name).map_err(
                        |source| LdError::Ir {
                            pou: self.name.clone(),
                            source,
                        },
                    )?
                };
                if !types_compatible(dest_ty, in1_ty) {
                    return Err(LdError::Ir {
                        pou: self.name.clone(),
                        source: IrError::TypeMismatch {
                            pou: self.name.clone(),
                            expected: dest_ty,
                            actual: in1_ty,
                            context: format!("{} input1 -> '{dest}'", op.short_label()),
                        },
                    });
                }
                if *op == LdMathOp::Div {
                    let Some(raw2) = input2.as_deref() else {
                        return Err(LdError::Ir {
                            pou: self.name.clone(),
                            source: IrError::Validation {
                                pou: self.name.clone(),
                                message: "DIV operation requires input2".to_string(),
                            },
                        });
                    };
                    let trimmed = raw2.trim();
                    if trimmed == "0" || trimmed == "0.0" {
                        return Err(LdError::Ir {
                            pou: self.name.clone(),
                            source: IrError::Validation {
                                pou: self.name.clone(),
                                message: "DIV operation input2 cannot be zero literal".to_string(),
                            },
                        });
                    }
                }
                if *op != LdMathOp::Move {
                    let raw2 = input2.as_deref().unwrap_or("0");
                    let in2_ty = if let Ok(v) = IecValue::parse_literal(raw2, Some(dest_ty)) {
                        v.iec_type()
                    } else if let Ok(v) = IecValue::parse_literal(raw2, None) {
                        v.iec_type()
                    } else {
                        SymbolResolver::validate_readable_symbol(tags, raw2, &self.name).map_err(
                            |source| LdError::Ir {
                                pou: self.name.clone(),
                                source,
                            },
                        )?
                    };
                    if !types_compatible(dest_ty, in2_ty) {
                        return Err(LdError::Ir {
                            pou: self.name.clone(),
                            source: IrError::TypeMismatch {
                                pou: self.name.clone(),
                                expected: dest_ty,
                                actual: in2_ty,
                                context: format!("{} input2 -> '{dest}'", op.short_label()),
                            },
                        });
                    }
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
                coils: r
                    .coils
                    .iter()
                    .map(|c| {
                        let edge_idx = match c.kind {
                            CoilKind::RisingEdge | CoilKind::FallingEdge => {
                                let idx = edge_count;
                                edge_count += 1;
                                Some(idx)
                            }
                            _ => None,
                        };
                        CompiledCoil {
                            symbol: c.symbol.clone(),
                            kind: c.kind,
                            edge_idx,
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(CompiledLdPou {
            id: PouId::new(self.name.clone()),
            rungs,
            edge_states: vec![false; edge_count],
        })
    }

    fn compile_binding(raw: &str, hint: Option<IecType>) -> FbInputBinding {
        if let Ok(val) = IecValue::parse_literal(raw, hint) {
            FbInputBinding::Literal(val)
        } else if let Ok(val) = IecValue::parse_literal(raw, None) {
            FbInputBinding::Literal(val)
        } else {
            FbInputBinding::Symbol(raw.trim().to_string())
        }
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
                        (pin.clone(), Self::compile_binding(raw, pty))
                    })
                    .collect();
                CompiledElement::InlineFb {
                    instance: instance.clone(),
                    enable_pin: enable_pin.clone(),
                    inputs: compiled_inputs,
                    outputs: outputs.clone(),
                }
            }
            LdElement::Compare { op, left, right } => CompiledElement::Compare {
                op: *op,
                left: Self::compile_binding(left, None),
                right: Self::compile_binding(right, None),
            },
            LdElement::MathMove {
                op,
                input1,
                input2,
                dest,
            } => {
                let dest_ty = tags.symbol_type(dest).ok();
                let in1 = Self::compile_binding(input1, dest_ty);
                let in2 = input2
                    .as_deref()
                    .map(|s| Self::compile_binding(s, dest_ty));
                CompiledElement::MathMove {
                    op: *op,
                    input1: in1,
                    input2: in2,
                    dest: dest.clone(),
                    dest_ty,
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

impl FbInputBinding {
    fn resolve(&self, ctx: &ScanContext<'_>) -> Result<IecValue, RuntimeError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Symbol(sym) => ctx.read_symbol(sym),
        }
    }
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
    Compare {
        op: LdCompareOp,
        left: FbInputBinding,
        right: FbInputBinding,
    },
    MathMove {
        op: LdMathOp,
        input1: FbInputBinding,
        input2: Option<FbInputBinding>,
        dest: String,
        dest_ty: Option<IecType>,
    },
}

#[derive(Debug, Clone)]
struct CompiledCoil {
    symbol: String,
    kind: CoilKind,
    edge_idx: Option<usize>,
}

#[derive(Debug, Clone)]
struct CompiledRung {
    elements: Vec<CompiledElement>,
    coils: Vec<CompiledCoil>,
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
        pou_name: &str,
        ctx: &mut ScanContext<'_>,
    ) -> Result<bool, RuntimeError> {
        for elem in elements {
            power = Self::eval_element(elem, power, edge_states, pou_name, ctx)?;
        }
        Ok(power)
    }

    fn eval_element(
        elem: &CompiledElement,
        power_in: bool,
        edge_states: &mut [bool],
        pou_name: &str,
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
                    let path_out = Self::eval_elements(path, power_in, edge_states, pou_name, ctx)?;
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
                    fb.write_pin(en_pin, &IecValue::Bool(power_in))?;
                }
                for (pin, binding) in inputs {
                    let val = binding.resolve(ctx)?;
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
            CompiledElement::Compare { op, left, right } => {
                if !power_in {
                    return Ok(false);
                }
                let lv = left.resolve(ctx)?;
                let rv = right.resolve(ctx)?;
                let res = eval_binary_op(op.to_ir_op(), &lv, &rv, pou_name)?.as_bool()?;
                Ok(res)
            }
            CompiledElement::MathMove {
                op,
                input1,
                input2,
                dest,
                dest_ty,
            } => {
                if !power_in {
                    return Ok(false);
                }
                let v1 = input1.resolve(ctx)?;
                let raw_result = match op {
                    LdMathOp::Move => v1,
                    LdMathOp::Add | LdMathOp::Sub | LdMathOp::Mul | LdMathOp::Div => {
                        let v2 = match input2 {
                            Some(b) => b.resolve(ctx)?,
                            None => IecValue::Int(0),
                        };
                        let bin_op = match op {
                            LdMathOp::Add => IrBinaryOp::Add,
                            LdMathOp::Sub => IrBinaryOp::Sub,
                            LdMathOp::Mul => IrBinaryOp::Mul,
                            _ => IrBinaryOp::Div,
                        };
                        eval_binary_op(bin_op, &v1, &v2, pou_name)?
                    }
                };
                let coerced = match (dest_ty, &raw_result) {
                    (Some(IecType::Int), IecValue::Real(r)) => {
                        IecValue::Int(r.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16)
                    }
                    (Some(IecType::Dint), IecValue::Real(r)) => {
                        IecValue::Dint(r.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32)
                    }
                    (Some(IecType::Int), _) => IecValue::Int(raw_result.as_int()?),
                    (Some(IecType::Dint), _) => IecValue::Dint(raw_result.as_dint()?),
                    (Some(IecType::Real), _) => IecValue::Real(raw_result.as_real()?),
                    (Some(IecType::Time), _) => IecValue::Time(raw_result.as_time_ms()?),
                    _ => raw_result,
                };
                ctx.write_symbol(dest, coerced)?;
                Ok(true)
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
        let pou_name = self.id.as_str();
        for rung in &self.rungs {
            let power =
                Self::eval_elements(&rung.elements, true, &mut self.edge_states, pou_name, ctx)?;
            for coil in &rung.coils {
                match coil.kind {
                    CoilKind::Normal => {
                        ctx.write_symbol(&coil.symbol, IecValue::Bool(power))?;
                    }
                    CoilKind::Negated => {
                        ctx.write_symbol(&coil.symbol, IecValue::Bool(!power))?;
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
                    CoilKind::RisingEdge => {
                        let idx = coil.edge_idx.unwrap_or(0);
                        let prev = self.edge_states[idx];
                        self.edge_states[idx] = power;
                        ctx.write_symbol(&coil.symbol, IecValue::Bool(power && !prev))?;
                    }
                    CoilKind::FallingEdge => {
                        let idx = coil.edge_idx.unwrap_or(0);
                        let prev = self.edge_states[idx];
                        self.edge_states[idx] = power;
                        ctx.write_symbol(&coil.symbol, IecValue::Bool(!power && prev))?;
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

    #[test]
    fn test_ladder_compare_math_move_and_extended_coils() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Enable",
            IecType::Bool,
            None,
            IecValue::Bool(true),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Level",
            IecType::Int,
            None,
            IecValue::Int(60),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Scaled_Level",
            IecType::Int,
            None,
            IecValue::Int(0),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Inv_Coil",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Pulse_P",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Pulse_N",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;

        let ld = LdProgram {
            name: "ld_ext".to_string(),
            comment: String::new(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: String::new(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Enable".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Compare {
                        op: LdCompareOp::Ge,
                        left: "Level".to_string(),
                        right: "50".to_string(),
                    },
                    LdElement::MathMove {
                        op: LdMathOp::Add,
                        input1: "Level".to_string(),
                        input2: Some("15".to_string()),
                        dest: "Scaled_Level".to_string(),
                    },
                ],
                coils: vec![
                    LdCoil {
                        symbol: "Inv_Coil".to_string(),
                        kind: CoilKind::Negated,
                    },
                    LdCoil {
                        symbol: "Pulse_P".to_string(),
                        kind: CoilKind::RisingEdge,
                    },
                    LdCoil {
                        symbol: "Pulse_N".to_string(),
                        kind: CoilKind::FallingEdge,
                    },
                ],
            }],
        };

        let mut compiled = ld.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);

        // Scan 1: Enable=TRUE, Level=60 >= 50 -> rung power TRUE
        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Scaled_Level")?, IecValue::Int(75));
        assert!(!ctx.read_bool("Inv_Coil")?);
        assert!(ctx.read_bool("Pulse_P")?);
        assert!(!ctx.read_bool("Pulse_N")?);

        // Scan 2: rung stays TRUE -> Pulse_P drops to FALSE
        compiled.execute(&mut ctx)?;
        assert!(!ctx.read_bool("Pulse_P")?);
        assert!(!ctx.read_bool("Pulse_N")?);

        // Scan 3: Level drops to 30 (< 50) -> rung power FALSE -> Inv_Coil=TRUE, Pulse_N=TRUE
        ctx.write_symbol("Level", IecValue::Int(30))?;
        compiled.execute(&mut ctx)?;
        assert_eq!(ctx.read_symbol("Scaled_Level")?, IecValue::Int(75)); // unchanged when rung false
        assert!(ctx.read_bool("Inv_Coil")?);
        assert!(!ctx.read_bool("Pulse_P")?);
        assert!(ctx.read_bool("Pulse_N")?);

        Ok(())
    }

    #[test]
    fn test_ld_math_add_with_mixed_int_and_real_numbers() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::default();

        // INT tag Tank_Level (matches user's real project setup)
        tags.register_tag(TagEntry::new(
            "Tank_Level",
            IecType::Int,
            None,
            IecValue::Int(10),
            "",
        ))?;
        // REAL tag Tank_Temp
        tags.register_tag(TagEntry::new(
            "Tank_Temp",
            IecType::Real,
            None,
            IecValue::Real(20.0),
            "",
        ))?;

        let ld = LdProgram {
            name: "main_safety".to_string(),
            comment: String::new(),
            rungs: vec![
                // Rung 0: ADD Tank_Level + "1.0" -> Tank_Level (INT dest with REAL literal)
                LdRung {
                    id: "R0".to_string(),
                    comment: String::new(),
                    elements: vec![LdElement::MathMove {
                        op: LdMathOp::Add,
                        input1: "Tank_Level".to_string(),
                        input2: Some("1.0".to_string()),
                        dest: "Tank_Level".to_string(),
                    }],
                    coils: vec![],
                },
                // Rung 1: ADD Tank_Temp + "2" -> Tank_Temp (REAL dest with INT literal)
                LdRung {
                    id: "R1".to_string(),
                    comment: String::new(),
                    elements: vec![LdElement::MathMove {
                        op: LdMathOp::Add,
                        input1: "Tank_Temp".to_string(),
                        input2: Some("2".to_string()),
                        dest: "Tank_Temp".to_string(),
                    }],
                    coils: vec![],
                },
            ],
        };

        // Must validate and compile without "Unknown symbol '1.0'"
        let mut compiled = ld.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);

        compiled.execute(&mut ctx)?;
        // Tank_Level was 10, plus 1.0 -> 11
        assert_eq!(ctx.read_symbol("Tank_Level")?, IecValue::Int(11));
        // Tank_Temp was 20.0, plus 2 -> 22.0
        assert_eq!(ctx.read_symbol("Tank_Temp")?, IecValue::Real(22.0));

        Ok(())
    }

    #[test]
    fn test_i10_combinational_loop_detection() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new("A", IecType::Bool, None, IecValue::Bool(false), ""))?;
        tags.register_tag(TagEntry::new("B", IecType::Bool, None, IecValue::Bool(false), ""))?;

        let ld = LdProgram {
            name: "loop_prog".to_string(),
            comment: "".to_string(),
            rungs: vec![
                // Rung 1: contact B -> coil A
                LdRung {
                    id: "R1".to_string(),
                    comment: "".to_string(),
                    elements: vec![LdElement::Contact {
                        symbol: "B".to_string(),
                        kind: ContactKind::NormallyOpen,
                    }],
                    coils: vec![LdCoil {
                        symbol: "A".to_string(),
                        kind: CoilKind::Normal,
                    }],
                },
                // Rung 2: contact A -> coil B
                LdRung {
                    id: "R2".to_string(),
                    comment: "".to_string(),
                    elements: vec![LdElement::Contact {
                        symbol: "A".to_string(),
                        kind: ContactKind::NormallyOpen,
                    }],
                    coils: vec![LdCoil {
                        symbol: "B".to_string(),
                        kind: CoilKind::Normal,
                    }],
                },
            ],
        };

        assert!(matches!(
            ld.validate(&tags),
            Err(LdError::Ir {
                source: IrError::CombinationalLoop { .. },
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn test_i11_inline_fb_enable_pin_validation() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.create_fb("TON_1", ferrite_core::StandardFbKind::Ton);

        // Invalid enable_pin "EN" on TON (TON has IN, not EN)
        let ld_bad = LdProgram {
            name: "fb_bad".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::InlineFb {
                    instance: "TON_1".to_string(),
                    enable_pin: Some("EN".to_string()),
                    inputs: vec![],
                    outputs: vec![],
                }],
                coils: vec![],
            }],
        };
        assert!(matches!(
            ld_bad.validate(&tags),
            Err(LdError::Ir {
                source: IrError::Core {
                    source: ferrite_core::CoreError::UnknownFbPin { .. },
                    ..
                },
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn test_ld_compare_bool_numeric_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new("BoolTag", IecType::Bool, None, IecValue::Bool(true), ""))?;

        let ld = LdProgram {
            name: "cmp_mismatch".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::Compare {
                    op: LdCompareOp::Ge,
                    left: "BoolTag".to_string(),
                    right: "50".to_string(),
                }],
                coils: vec![],
            }],
        };
        assert!(matches!(
            ld.validate(&tags),
            Err(LdError::Ir {
                source: IrError::TypeMismatch { .. },
                ..
            })
        ));

        Ok(())
    }

    #[test]
    fn test_ld_div_missing_input2_or_zero() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new("Val", IecType::Int, None, IecValue::Int(10), ""))?;

        // DIV without input2
        let ld_no_in2 = LdProgram {
            name: "div_no_in2".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::MathMove {
                    op: LdMathOp::Div,
                    input1: "Val".to_string(),
                    input2: None,
                    dest: "Val".to_string(),
                }],
                coils: vec![],
            }],
        };
        assert!(matches!(
            ld_no_in2.validate(&tags),
            Err(LdError::Ir {
                source: IrError::Validation { message, .. },
                ..
            }) if message.contains("requires input2")
        ));

        // DIV with literal "0"
        let ld_zero = LdProgram {
            name: "div_zero".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::MathMove {
                    op: LdMathOp::Div,
                    input1: "Val".to_string(),
                    input2: Some("0".to_string()),
                    dest: "Val".to_string(),
                }],
                coils: vec![],
            }],
        };
        assert!(matches!(
            ld_zero.validate(&tags),
            Err(LdError::Ir {
                source: IrError::Validation { message, .. },
                ..
            }) if message.contains("cannot be zero literal")
        ));

        Ok(())
    }

    #[test]
    fn test_ld_real_to_int_rounding_coercion() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new("RealIn", IecType::Real, None, IecValue::Real(3.7), ""))?;
        tags.register_tag(TagEntry::new("IntOut", IecType::Int, None, IecValue::Int(0), ""))?;

        let ld = LdProgram {
            name: "round_test".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R1".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::MathMove {
                    op: LdMathOp::Move,
                    input1: "RealIn".to_string(),
                    input2: None,
                    dest: "IntOut".to_string(),
                }],
                coils: vec![],
            }],
        };

        let mut compiled = ld.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 10, 1);
        compiled.execute(&mut ctx)?;
        // 3.7 rounds to 4, not truncated to 3!
        assert_eq!(ctx.read_symbol("IntOut")?, IecValue::Int(4));

        Ok(())
    }
}
