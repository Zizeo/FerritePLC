use std::collections::{HashMap, HashSet, VecDeque};

use ferrite_core::{
    CoreError, DirectAddress, IecType, IecValue, LanguageKind, PouId, RuntimeError, ScanContext,
    TagTable,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_LOOP_ITERATIONS: usize = 10_000;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum IrError {
    #[error("Core error in POU '{pou}': {source}")]
    Core { pou: String, source: CoreError },

    #[error("Type mismatch in POU '{pou}': expected {expected}, got {actual} ({context})")]
    TypeMismatch {
        pou: String,
        expected: IecType,
        actual: IecType,
        context: String,
    },

    #[error("Combinational loop detected in '{scope}': {cycle:?}")]
    CombinationalLoop { scope: String, cycle: Vec<String> },

    #[error("Validation error in POU '{pou}': {message}")]
    Validation { pou: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationDiagnostic {
    pub pou: String,
    pub language: LanguageKind,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrUnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrExpr {
    Literal(IecValue),
    Symbol(String),
    Unary {
        op: IrUnaryOp,
        expr: Box<IrExpr>,
    },
    Binary {
        op: IrBinaryOp,
        left: Box<IrExpr>,
        right: Box<IrExpr>,
    },
    Call {
        function_name: String,
        args: Vec<IrExpr>,
    },
}

impl IrExpr {
    pub fn evaluate(&self, ctx: &ScanContext<'_>, pou_name: &str) -> Result<IecValue, RuntimeError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Symbol(sym) => ctx.read_symbol(sym),
            Self::Unary { op, expr } => {
                let val = expr.evaluate(ctx, pou_name)?;
                match op {
                    IrUnaryOp::Not => match val {
                        IecValue::Bool(b) => Ok(IecValue::Bool(!b)),
                        IecValue::Int(i) => Ok(IecValue::Int(!i)),
                        IecValue::Dint(d) => Ok(IecValue::Dint(!d)),
                        other => Err(CoreError::TypeMismatch {
                            expected: IecType::Bool,
                            actual: other.iec_type(),
                        }
                        .into()),
                    },
                    IrUnaryOp::Neg => match val {
                        IecValue::Int(i) => Ok(IecValue::Int(i.wrapping_neg())),
                        IecValue::Dint(d) => Ok(IecValue::Dint(d.wrapping_neg())),
                        IecValue::Real(r) => Ok(IecValue::Real(-r)),
                        other => Err(CoreError::TypeMismatch {
                            expected: IecType::Int,
                            actual: other.iec_type(),
                        }
                        .into()),
                    },
                }
            }
            Self::Binary { op, left, right } => {
                let lv = left.evaluate(ctx, pou_name)?;
                let rv = right.evaluate(ctx, pou_name)?;
                eval_binary_op(*op, &lv, &rv, pou_name)
            }
            Self::Call { function_name, args } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(a.evaluate(ctx, pou_name)?);
                }
                eval_math_function(function_name, &evaluated_args, pou_name)
            }
        }
    }

    pub fn infer_type(&self, tags: &TagTable, pou_name: &str) -> Result<IecType, IrError> {
        match self {
            Self::Literal(v) => Ok(v.iec_type()),
            Self::Symbol(sym) => tags.symbol_type(sym).map_err(|source| IrError::Core {
                pou: pou_name.to_string(),
                source,
            }),
            Self::Call { function_name, args } => {
                let upper = function_name.to_ascii_uppercase();
                match upper.as_str() {
                    "ABS" | "MIN" | "MAX" => {
                        if let Some(first) = args.first() {
                            first.infer_type(tags, pou_name)
                        } else {
                            Ok(IecType::Real)
                        }
                    }
                    "LIMIT" => {
                        if args.len() >= 2 {
                            args[1].infer_type(tags, pou_name)
                        } else {
                            Ok(IecType::Real)
                        }
                    }
                    _ => Ok(IecType::Real),
                }
            }
            Self::Unary { op, expr } => {
                let ty = expr.infer_type(tags, pou_name)?;
                match op {
                    IrUnaryOp::Not => {
                        if matches!(ty, IecType::Bool | IecType::Int | IecType::Dint) {
                            Ok(ty)
                        } else {
                            Err(IrError::TypeMismatch {
                                pou: pou_name.to_string(),
                                expected: IecType::Bool,
                                actual: ty,
                                context: "unary NOT".to_string(),
                            })
                        }
                    }
                    IrUnaryOp::Neg => {
                        if matches!(ty, IecType::Int | IecType::Dint | IecType::Real) {
                            Ok(ty)
                        } else {
                            Err(IrError::TypeMismatch {
                                pou: pou_name.to_string(),
                                expected: IecType::Int,
                                actual: ty,
                                context: "unary NEG".to_string(),
                            })
                        }
                    }
                }
            }
            Self::Binary { op, left, right } => {
                let lt = left.infer_type(tags, pou_name)?;
                let rt = right.infer_type(tags, pou_name)?;
                match op {
                    IrBinaryOp::And | IrBinaryOp::Or | IrBinaryOp::Xor => {
                        if lt == IecType::Bool && rt == IecType::Bool {
                            Ok(IecType::Bool)
                        } else if lt.is_numeric() && rt.is_numeric() {
                            Ok(if lt == IecType::Dint || rt == IecType::Dint {
                                IecType::Dint
                            } else {
                                IecType::Int
                            })
                        } else {
                            Err(IrError::TypeMismatch {
                                pou: pou_name.to_string(),
                                expected: lt,
                                actual: rt,
                                context: format!("bitwise/boolean {op:?}"),
                            })
                        }
                    }
                    IrBinaryOp::Eq
                    | IrBinaryOp::Ne
                    | IrBinaryOp::Lt
                    | IrBinaryOp::Le
                    | IrBinaryOp::Gt
                    | IrBinaryOp::Ge => {
                        if lt == rt || (lt.is_numeric() && rt.is_numeric()) {
                            Ok(IecType::Bool)
                        } else {
                            Err(IrError::TypeMismatch {
                                pou: pou_name.to_string(),
                                expected: lt,
                                actual: rt,
                                context: format!("comparison {op:?}"),
                            })
                        }
                    }
                    IrBinaryOp::Add
                    | IrBinaryOp::Sub
                    | IrBinaryOp::Mul
                    | IrBinaryOp::Div
                    | IrBinaryOp::Mod => {
                        if lt == IecType::Real || rt == IecType::Real {
                            Ok(IecType::Real)
                        } else if lt == IecType::Time || rt == IecType::Time {
                            Ok(IecType::Time)
                        } else if lt == IecType::Dint || rt == IecType::Dint {
                            Ok(IecType::Dint)
                        } else if lt == IecType::Int && rt == IecType::Int {
                            Ok(IecType::Int)
                        } else {
                            Err(IrError::TypeMismatch {
                                pou: pou_name.to_string(),
                                expected: lt,
                                actual: rt,
                                context: format!("arithmetic {op:?}"),
                            })
                        }
                    }
                }
            }
        }
    }

    pub fn referenced_symbols(&self, out: &mut Vec<String>) {
        match self {
            Self::Literal(_) => {}
            Self::Symbol(s) => out.push(s.clone()),
            Self::Unary { expr, .. } => expr.referenced_symbols(out),
            Self::Binary { left, right, .. } => {
                left.referenced_symbols(out);
                right.referenced_symbols(out);
            }
            Self::Call { args, .. } => {
                for a in args {
                    a.referenced_symbols(out);
                }
            }
        }
    }
}

fn eval_math_function(
    func: &str,
    args: &[IecValue],
    pou_name: &str,
) -> Result<IecValue, RuntimeError> {
    let name = func.to_ascii_uppercase();
    match name.as_str() {
        "ABS" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "ABS requires 1 argument".to_string(),
            })?;
            match v {
                IecValue::Int(i) => Ok(IecValue::Int(i.abs())),
                IecValue::Dint(d) => Ok(IecValue::Dint(d.abs())),
                IecValue::Real(r) => Ok(IecValue::Real(r.abs())),
                _ => Ok(IecValue::Real(v.as_real()?.abs())),
            }
        }
        "SQRT" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "SQRT requires 1 argument".to_string(),
            })?;
            let r = v.as_real()?;
            if r < 0.0 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("SQRT domain error: negative value {r}"),
                });
            }
            Ok(IecValue::Real(r.sqrt()))
        }
        "SIN" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "SIN requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.sin()))
        }
        "COS" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "COS requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.cos()))
        }
        "TAN" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "TAN requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.tan()))
        }
        "LN" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "LN requires 1 argument".to_string(),
            })?;
            let r = v.as_real()?;
            if r <= 0.0 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("LN domain error: non-positive value {r}"),
                });
            }
            Ok(IecValue::Real(r.ln()))
        }
        "EXP" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "EXP requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.exp()))
        }
        "EXPT" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "EXPT requires 2 arguments (base, exponent)".to_string(),
                });
            }
            let base = args[0].as_real()?;
            let exp = args[1].as_real()?;
            Ok(IecValue::Real(base.powf(exp)))
        }
        "MIN" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "MIN requires at least 2 arguments".to_string(),
                });
            }
            let mut min_val = args[0].clone();
            for item in &args[1..] {
                let a = min_val.as_real()?;
                let b = item.as_real()?;
                if b < a {
                    min_val = item.clone();
                }
            }
            Ok(min_val)
        }
        "MAX" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "MAX requires at least 2 arguments".to_string(),
                });
            }
            let mut max_val = args[0].clone();
            for item in &args[1..] {
                let a = max_val.as_real()?;
                let b = item.as_real()?;
                if b > a {
                    max_val = item.clone();
                }
            }
            Ok(max_val)
        }
        "LIMIT" => {
            if args.len() < 3 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "LIMIT requires 3 arguments (min, val, max)".to_string(),
                });
            }
            let min = args[0].as_real()?;
            let val = args[1].as_real()?;
            let max = args[2].as_real()?;
            let clamped = val.clamp(min, max);
            Ok(IecValue::Real(clamped))
        }
        "TRUNC" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "TRUNC requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.trunc()))
        }
        "ROUND" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "ROUND requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.round()))
        }
        other => Err(RuntimeError::ExecutionError {
            pou: pou_name.to_string(),
            message: format!("Unknown standard function '{other}'"),
        }),
    }
}

pub fn eval_binary_op(
    op: IrBinaryOp,
    lv: &IecValue,
    rv: &IecValue,
    pou_name: &str,
) -> Result<IecValue, RuntimeError> {
    match op {
        IrBinaryOp::And => match (lv, rv) {
            (IecValue::Bool(a), IecValue::Bool(b)) => Ok(IecValue::Bool(*a && *b)),
            _ => Ok(IecValue::Dint(lv.as_dint()? & rv.as_dint()?)),
        },
        IrBinaryOp::Or => match (lv, rv) {
            (IecValue::Bool(a), IecValue::Bool(b)) => Ok(IecValue::Bool(*a || *b)),
            _ => Ok(IecValue::Dint(lv.as_dint()? | rv.as_dint()?)),
        },
        IrBinaryOp::Xor => match (lv, rv) {
            (IecValue::Bool(a), IecValue::Bool(b)) => Ok(IecValue::Bool(*a ^ *b)),
            _ => Ok(IecValue::Dint(lv.as_dint()? ^ rv.as_dint()?)),
        },
        IrBinaryOp::Eq => {
            if matches!(lv, IecValue::Real(_)) || matches!(rv, IecValue::Real(_)) {
                let diff = (lv.as_real()? - rv.as_real()?).abs();
                Ok(IecValue::Bool(diff < 1e-5))
            } else if lv.iec_type().is_numeric() && rv.iec_type().is_numeric() {
                Ok(IecValue::Bool(lv.as_dint()? == rv.as_dint()?))
            } else {
                Ok(IecValue::Bool(lv == rv))
            }
        }
        IrBinaryOp::Ne => {
            let eq = eval_binary_op(IrBinaryOp::Eq, lv, rv, pou_name)?.as_bool()?;
            Ok(IecValue::Bool(!eq))
        }
        IrBinaryOp::Lt | IrBinaryOp::Le | IrBinaryOp::Gt | IrBinaryOp::Ge => {
            if matches!(lv, IecValue::Real(_)) || matches!(rv, IecValue::Real(_)) {
                let a = lv.as_real()?;
                let b = rv.as_real()?;
                let res = match op {
                    IrBinaryOp::Lt => a < b,
                    IrBinaryOp::Le => a <= b,
                    IrBinaryOp::Gt => a > b,
                    IrBinaryOp::Ge => a >= b,
                    _ => false,
                };
                Ok(IecValue::Bool(res))
            } else if matches!(lv, IecValue::Time(_)) && matches!(rv, IecValue::Time(_)) {
                let a = lv.as_time_ms()?;
                let b = rv.as_time_ms()?;
                let res = match op {
                    IrBinaryOp::Lt => a < b,
                    IrBinaryOp::Le => a <= b,
                    IrBinaryOp::Gt => a > b,
                    IrBinaryOp::Ge => a >= b,
                    _ => false,
                };
                Ok(IecValue::Bool(res))
            } else {
                let a = lv.as_dint()?;
                let b = rv.as_dint()?;
                let res = match op {
                    IrBinaryOp::Lt => a < b,
                    IrBinaryOp::Le => a <= b,
                    IrBinaryOp::Gt => a > b,
                    IrBinaryOp::Ge => a >= b,
                    _ => false,
                };
                Ok(IecValue::Bool(res))
            }
        }
        IrBinaryOp::Add | IrBinaryOp::Sub | IrBinaryOp::Mul | IrBinaryOp::Div | IrBinaryOp::Mod => {
            if matches!(lv, IecValue::Real(_)) || matches!(rv, IecValue::Real(_)) {
                let a = lv.as_real()?;
                let b = rv.as_real()?;
                let res = match op {
                    IrBinaryOp::Add => a + b,
                    IrBinaryOp::Sub => a - b,
                    IrBinaryOp::Mul => a * b,
                    IrBinaryOp::Div => {
                        if b.abs() < f32::EPSILON {
                            return Err(RuntimeError::DivisionByZero {
                                pou: pou_name.to_string(),
                            });
                        }
                        a / b
                    }
                    IrBinaryOp::Mod => {
                        if b.abs() < f32::EPSILON {
                            return Err(RuntimeError::DivisionByZero {
                                pou: pou_name.to_string(),
                            });
                        }
                        a % b
                    }
                    _ => 0.0,
                };
                Ok(IecValue::Real(res))
            } else if matches!(lv, IecValue::Dint(_)) || matches!(rv, IecValue::Dint(_)) {
                let a = lv.as_dint()?;
                let b = rv.as_dint()?;
                let res = match op {
                    IrBinaryOp::Add => a.wrapping_add(b),
                    IrBinaryOp::Sub => a.wrapping_sub(b),
                    IrBinaryOp::Mul => a.wrapping_mul(b),
                    IrBinaryOp::Div => {
                        if b == 0 {
                            return Err(RuntimeError::DivisionByZero {
                                pou: pou_name.to_string(),
                            });
                        }
                        a.wrapping_div(b)
                    }
                    IrBinaryOp::Mod => {
                        if b == 0 {
                            return Err(RuntimeError::DivisionByZero {
                                pou: pou_name.to_string(),
                            });
                        }
                        a.wrapping_rem(b)
                    }
                    _ => 0,
                };
                Ok(IecValue::Dint(res))
            } else {
                let a = lv.as_int()?;
                let b = rv.as_int()?;
                let res = match op {
                    IrBinaryOp::Add => a.wrapping_add(b),
                    IrBinaryOp::Sub => a.wrapping_sub(b),
                    IrBinaryOp::Mul => a.wrapping_mul(b),
                    IrBinaryOp::Div => {
                        if b == 0 {
                            return Err(RuntimeError::DivisionByZero {
                                pou: pou_name.to_string(),
                            });
                        }
                        a.wrapping_div(b)
                    }
                    IrBinaryOp::Mod => {
                        if b == 0 {
                            return Err(RuntimeError::DivisionByZero {
                                pou: pou_name.to_string(),
                            });
                        }
                        a.wrapping_rem(b)
                    }
                    _ => 0,
                };
                Ok(IecValue::Int(res))
            }
        }
    }
}

/// Statements in the common FerriteIR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrStmt {
    Assign {
        target: String,
        expr: IrExpr,
    },
    If {
        branches: Vec<(IrExpr, Vec<IrStmt>)>,
        else_body: Vec<IrStmt>,
    },
    Case {
        selector: IrExpr,
        arms: Vec<(Vec<i32>, Vec<IrStmt>)>,
        else_body: Vec<IrStmt>,
    },
    For {
        var: String,
        start: IrExpr,
        end: IrExpr,
        step: Option<IrExpr>,
        body: Vec<IrStmt>,
    },
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    FbCall {
        instance: String,
        inputs: Vec<(String, IrExpr)>,
        outputs: Vec<(String, String)>,
    },
}

impl IrStmt {
    pub fn execute(&self, ctx: &mut ScanContext<'_>, pou_name: &str) -> Result<(), RuntimeError> {
        match self {
            Self::Assign { target, expr } => {
                let val = expr.evaluate(ctx, pou_name)?;
                ctx.write_symbol(target, val)
            }
            Self::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    if cond.evaluate(ctx, pou_name)?.as_bool()? {
                        for stmt in body {
                            stmt.execute(ctx, pou_name)?;
                        }
                        return Ok(());
                    }
                }
                for stmt in else_body {
                    stmt.execute(ctx, pou_name)?;
                }
                Ok(())
            }
            Self::Case {
                selector,
                arms,
                else_body,
            } => {
                let sel_val = selector.evaluate(ctx, pou_name)?.as_dint()?;
                for (labels, body) in arms {
                    if labels.contains(&sel_val) {
                        for stmt in body {
                            stmt.execute(ctx, pou_name)?;
                        }
                        return Ok(());
                    }
                }
                for stmt in else_body {
                    stmt.execute(ctx, pou_name)?;
                }
                Ok(())
            }
            Self::For {
                var,
                start,
                end,
                step,
                body,
            } => {
                let start_val = start.evaluate(ctx, pou_name)?.as_dint()?;
                let end_val = end.evaluate(ctx, pou_name)?.as_dint()?;
                let step_val = match step {
                    Some(s) => s.evaluate(ctx, pou_name)?.as_dint()?,
                    None => 1,
                };
                if step_val == 0 {
                    return Err(RuntimeError::IterationLimitExceeded {
                        pou: pou_name.to_string(),
                        max_iterations: MAX_LOOP_ITERATIONS,
                    });
                }
                let mut cur = start_val;
                let mut iters = 0usize;
                while (step_val > 0 && cur <= end_val) || (step_val < 0 && cur >= end_val) {
                    if iters >= MAX_LOOP_ITERATIONS {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: MAX_LOOP_ITERATIONS,
                        });
                    }
                    ctx.write_symbol(var, IecValue::Dint(cur))?;
                    for stmt in body {
                        stmt.execute(ctx, pou_name)?;
                    }
                    cur = cur.wrapping_add(step_val);
                    iters += 1;
                }
                Ok(())
            }
            Self::While { cond, body } => {
                let mut iters = 0usize;
                while cond.evaluate(ctx, pou_name)?.as_bool()? {
                    if iters >= MAX_LOOP_ITERATIONS {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: MAX_LOOP_ITERATIONS,
                        });
                    }
                    for stmt in body {
                        stmt.execute(ctx, pou_name)?;
                    }
                    iters += 1;
                }
                Ok(())
            }
            Self::FbCall {
                instance,
                inputs,
                outputs,
            } => {
                for (pin, expr) in inputs {
                    let val = expr.evaluate(ctx, pou_name)?;
                    let sym = format!("{instance}.{pin}");
                    ctx.write_symbol(&sym, val)?;
                }
                ctx.evaluate_fb(instance)?;
                for (pin, target_sym) in outputs {
                    let sym = format!("{instance}.{pin}");
                    let out_val = ctx.read_symbol(&sym)?;
                    ctx.write_symbol(target_sym, out_val)?;
                }
                Ok(())
            }
        }
    }

    pub fn validate(&self, tags: &TagTable, pou_name: &str, in_sfc: bool) -> Result<(), IrError> {
        match self {
            Self::Assign { target, expr } => {
                let target_ty = SymbolResolver::validate_writable_symbol(tags, target, pou_name, in_sfc)?;
                let expr_ty = expr.infer_type(tags, pou_name)?;
                if !types_compatible(target_ty, expr_ty) {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: target_ty,
                        actual: expr_ty,
                        context: format!("assignment to '{target}'"),
                    });
                }
                Ok(())
            }
            Self::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    let cty = cond.infer_type(tags, pou_name)?;
                    if cty != IecType::Bool {
                        return Err(IrError::TypeMismatch {
                            pou: pou_name.to_string(),
                            expected: IecType::Bool,
                            actual: cty,
                            context: "IF condition".to_string(),
                        });
                    }
                    for s in body {
                        s.validate(tags, pou_name, in_sfc)?;
                    }
                }
                for s in else_body {
                    s.validate(tags, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::Case {
                selector,
                arms,
                else_body,
            } => {
                let sty = selector.infer_type(tags, pou_name)?;
                if !matches!(sty, IecType::Int | IecType::Dint) {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Int,
                        actual: sty,
                        context: "CASE selector".to_string(),
                    });
                }
                for (_, body) in arms {
                    for s in body {
                        s.validate(tags, pou_name, in_sfc)?;
                    }
                }
                for s in else_body {
                    s.validate(tags, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::For {
                var,
                start,
                end,
                step,
                body,
            } => {
                let vty = SymbolResolver::validate_writable_symbol(tags, var, pou_name, in_sfc)?;
                if !matches!(vty, IecType::Int | IecType::Dint) {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Int,
                        actual: vty,
                        context: "FOR loop variable".to_string(),
                    });
                }
                let _ = start.infer_type(tags, pou_name)?;
                let _ = end.infer_type(tags, pou_name)?;
                if let Some(s) = step {
                    let _ = s.infer_type(tags, pou_name)?;
                }
                for s in body {
                    s.validate(tags, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::While { cond, body } => {
                let cty = cond.infer_type(tags, pou_name)?;
                if cty != IecType::Bool {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Bool,
                        actual: cty,
                        context: "WHILE condition".to_string(),
                    });
                }
                for s in body {
                    s.validate(tags, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::FbCall {
                instance,
                inputs,
                outputs,
            } => {
                let fb = tags.get_fb(instance).ok_or_else(|| IrError::Core {
                    pou: pou_name.to_string(),
                    source: CoreError::UnknownFbInstance(instance.clone()),
                })?;
                let kind = fb.kind;
                for (pin, expr) in inputs {
                    let pty = kind.pin_type(pin).ok_or_else(|| IrError::Core {
                        pou: pou_name.to_string(),
                        source: CoreError::UnknownFbPin {
                            instance: instance.clone(),
                            pin: pin.clone(),
                        },
                    })?;
                    let ety = expr.infer_type(tags, pou_name)?;
                    if !types_compatible(pty, ety) {
                        return Err(IrError::TypeMismatch {
                            pou: pou_name.to_string(),
                            expected: pty,
                            actual: ety,
                            context: format!("FB input {instance}.{pin}"),
                        });
                    }
                }
                for (pin, target_sym) in outputs {
                    let pty = kind.pin_type(pin).ok_or_else(|| IrError::Core {
                        pou: pou_name.to_string(),
                        source: CoreError::UnknownFbPin {
                            instance: instance.clone(),
                            pin: pin.clone(),
                        },
                    })?;
                    let tty =
                        SymbolResolver::validate_writable_symbol(tags, target_sym, pou_name, in_sfc)?;
                    if !types_compatible(tty, pty) {
                        return Err(IrError::TypeMismatch {
                            pou: pou_name.to_string(),
                            expected: tty,
                            actual: pty,
                            context: format!("FB output {instance}.{pin} -> {target_sym}"),
                        });
                    }
                }
                Ok(())
            }
        }
    }
}

pub fn types_compatible(expected: IecType, actual: IecType) -> bool {
    if expected == actual {
        return true;
    }
    matches!(
        (expected, actual),
        (IecType::Int, IecType::Dint)
            | (IecType::Dint, IecType::Int)
            | (IecType::Real, IecType::Int)
            | (IecType::Real, IecType::Dint)
            | (IecType::Time, IecType::Int)
            | (IecType::Time, IecType::Dint)
    )
}

/// Cross-language symbol resolver and validator.
pub struct SymbolResolver;

impl SymbolResolver {
    pub fn validate_readable_symbol(
        tags: &TagTable,
        symbol: &str,
        pou_name: &str,
    ) -> Result<IecType, IrError> {
        tags.symbol_type(symbol.trim()).map_err(|source| IrError::Core {
            pou: pou_name.to_string(),
            source,
        })
    }

    pub fn validate_writable_symbol(
        tags: &TagTable,
        symbol: &str,
        pou_name: &str,
        in_sfc: bool,
    ) -> Result<IecType, IrError> {
        let trimmed = symbol.trim();
        if let Some(tag) = tags.get_tag(trimmed) {
            if tag.read_only_outside_sfc && !in_sfc {
                return Err(IrError::Core {
                    pou: pou_name.to_string(),
                    source: CoreError::ReadOnlySymbol(trimmed.to_string()),
                });
            }
            return Ok(tag.data_type);
        }
        if trimmed.starts_with('%') {
            let addr: DirectAddress = trimmed.parse().map_err(|source| IrError::Core {
                pou: pou_name.to_string(),
                source,
            })?;
            return Ok(match addr.width {
                ferrite_core::AccessWidth::Bit => IecType::Bool,
                ferrite_core::AccessWidth::Byte | ferrite_core::AccessWidth::Word => IecType::Int,
                ferrite_core::AccessWidth::DWord => IecType::Dint,
            });
        }
        if let Some((inst, pin)) = trimmed.split_once('.')
            && let Some(fb) = tags.get_fb(inst) {
                if let Some(ty) = fb.kind.pin_type(pin) {
                    return Ok(ty);
                }
                return Err(IrError::Core {
                    pou: pou_name.to_string(),
                    source: CoreError::UnknownFbPin {
                        instance: inst.to_string(),
                        pin: pin.to_string(),
                    },
                });
            }
        Err(IrError::Core {
            pou: pou_name.to_string(),
            source: CoreError::UnknownSymbol(trimmed.to_string()),
        })
    }
}

/// Directed graph for POU dependencies and FBD/Ladder signal networks, supporting
/// topological sorting and combinational loop detection (Kahn's algorithm + cycle extraction).
#[derive(Debug, Clone, Default)]
pub struct PouGraph {
    nodes: Vec<String>,
    edges: HashMap<String, Vec<String>>,
}

impl PouGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: impl Into<String>) {
        let n = node.into();
        if !self.nodes.contains(&n) {
            self.nodes.push(n.clone());
        }
        self.edges.entry(n).or_default();
    }

    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        let f = from.into();
        let t = to.into();
        self.add_node(f.clone());
        self.add_node(t.clone());
        let list = self.edges.entry(f).or_default();
        if !list.contains(&t) {
            list.push(t);
        }
    }

    /// Returns a deterministic topological sort of the nodes, or `IrError::CombinationalLoop`
    /// if a cycle exists.
    pub fn topological_sort(&self, scope: &str) -> Result<Vec<String>, IrError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for n in &self.nodes {
            in_degree.insert(n.clone(), 0);
        }
        for targets in self.edges.values() {
            for t in targets {
                if let Some(deg) = in_degree.get_mut(t) {
                    *deg += 1;
                }
            }
        }

        let mut queue: VecDeque<String> = VecDeque::new();
        for n in &self.nodes {
            if in_degree.get(n).copied().unwrap_or(0) == 0 {
                queue.push_back(n.clone());
            }
        }

        let mut ordered = Vec::with_capacity(self.nodes.len());
        while let Some(u) = queue.pop_front() {
            ordered.push(u.clone());
            if let Some(neighbors) = self.edges.get(&u) {
                for v in neighbors {
                    if let Some(deg) = in_degree.get_mut(v) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(v.clone());
                        }
                    }
                }
            }
        }

        if ordered.len() != self.nodes.len() {
            let visited: HashSet<&String> = ordered.iter().collect();
            let cycle: Vec<String> = self
                .nodes
                .iter()
                .filter(|n| !visited.contains(n))
                .cloned()
                .collect();
            return Err(IrError::CombinationalLoop {
                scope: scope.to_string(),
                cycle,
            });
        }

        Ok(ordered)
    }

    pub fn pou_ids_in_order(&self, scope: &str) -> Result<Vec<PouId>, IrError> {
        let sorted = self.topological_sort(scope)?;
        Ok(sorted.into_iter().map(PouId).collect())
    }
}
