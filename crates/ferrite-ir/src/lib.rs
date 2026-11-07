use std::collections::{HashMap, HashSet, VecDeque};

use ferrite_core::{
    CoreError, DirectAddress, IecType, IecValue, LanguageKind, PouId, RuntimeError, ScanContext,
    TagTable,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_MAX_LOOP_ITERATIONS: usize = 10_000;
pub const MAX_LOOP_ITERATIONS: usize = DEFAULT_MAX_LOOP_ITERATIONS;

static MAX_LOOP_LIMIT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(DEFAULT_MAX_LOOP_ITERATIONS);

pub fn set_max_loop_iterations(limit: usize) {
    MAX_LOOP_LIMIT.store(limit, std::sync::atomic::Ordering::Relaxed);
}

pub fn max_loop_iterations() -> usize {
    MAX_LOOP_LIMIT.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardFunctionInfo {
    pub name: &'static str,
    pub min_arity: usize,
    pub max_arity: Option<usize>,
}

pub const STANDARD_FUNCTIONS: &[StandardFunctionInfo] = &[
    StandardFunctionInfo { name: "ABS", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SQRT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SIN", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "COS", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "TAN", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LN", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "EXP", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "EXPT", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "MIN", min_arity: 2, max_arity: None },
    StandardFunctionInfo { name: "MAX", min_arity: 2, max_arity: None },
    StandardFunctionInfo { name: "LIMIT", min_arity: 3, max_arity: Some(3) },
    StandardFunctionInfo { name: "TRUNC", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "ROUND", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "CEIL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "FLOOR", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "ASIN", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "ACOS", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "ATAN", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LOG", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SEL", min_arity: 3, max_arity: Some(3) },
    StandardFunctionInfo { name: "MUX", min_arity: 2, max_arity: None },
    StandardFunctionInfo { name: "SHL", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "SHR", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "ROL", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "ROR", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "INT_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BOOL_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "TIME_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "REAL_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BOOL_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "REAL_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BOOL_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "TIME_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_BOOL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_BOOL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "REAL_TO_BOOL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "REAL_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_LINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LINT_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_BOOL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BOOL_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_TIME", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "TIME_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_SINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SINT_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_SINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SINT_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LINT_TO_SINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SINT_TO_LINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "SINT_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "REAL_TO_SINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_BYTE", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BYTE_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_BYTE", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BYTE_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BYTE_TO_USINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "USINT_TO_BYTE", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BOOL_TO_BYTE", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BYTE_TO_BOOL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_WORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "WORD_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_WORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "WORD_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "WORD_TO_UINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "UINT_TO_WORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_DWORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DWORD_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DWORD_TO_UDINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "UDINT_TO_DWORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LINT_TO_LWORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LWORD_TO_LINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "ULINT_TO_LWORD", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LWORD_TO_ULINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "USINT_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_USINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "UINT_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_UINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "UINT_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_UINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "UDINT_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_UDINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "ULINT_TO_LINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LINT_TO_ULINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_LINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LINT_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_LINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LINT_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "INT_TO_STRING", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "STRING_TO_INT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "DINT_TO_STRING", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "STRING_TO_DINT", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "REAL_TO_STRING", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "STRING_TO_REAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LREAL_TO_STRING", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "STRING_TO_LREAL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "BOOL_TO_STRING", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "STRING_TO_BOOL", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "TIME_TO_STRING", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "STRING_TO_TIME", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "LEN", min_arity: 1, max_arity: Some(1) },
    StandardFunctionInfo { name: "CONCAT", min_arity: 2, max_arity: None },
    StandardFunctionInfo { name: "LEFT", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "RIGHT", min_arity: 2, max_arity: Some(2) },
    StandardFunctionInfo { name: "MID", min_arity: 3, max_arity: Some(3) },
    StandardFunctionInfo { name: "INSERT", min_arity: 3, max_arity: Some(3) },
    StandardFunctionInfo { name: "DELETE", min_arity: 3, max_arity: Some(3) },
    StandardFunctionInfo { name: "REPLACE", min_arity: 4, max_arity: Some(4) },
    StandardFunctionInfo { name: "FIND", min_arity: 2, max_arity: Some(2) },
];

pub fn lookup_standard_function(name: &str) -> Option<&'static StandardFunctionInfo> {
    STANDARD_FUNCTIONS
        .iter()
        .find(|info| info.name.eq_ignore_ascii_case(name))
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VarScope {
    Local,
    Input,
    Output,
    Temp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDecl {
    pub name: String,
    pub data_type: IecType,
    pub scope: VarScope,
    pub initial_value: Option<IrExpr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionParam {
    pub name: String,
    pub data_type: IecType,
    pub default_value: Option<IecValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserFunction {
    pub name: String,
    pub return_type: IecType,
    pub inputs: Vec<FunctionParam>,
    pub locals: Vec<FunctionParam>,
    pub body: Vec<IrStmt>,
}

thread_local! {
    static CALL_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl UserFunction {
    pub fn new(
        name: impl Into<String>,
        return_type: IecType,
        inputs: Vec<FunctionParam>,
        locals: Vec<FunctionParam>,
        body: Vec<IrStmt>,
    ) -> Self {
        Self {
            name: name.into(),
            return_type,
            inputs,
            locals,
            body,
        }
    }

    pub fn validate(&self, tags: &TagTable) -> Result<(), IrError> {
        let mut local_types = HashMap::new();
        for param in &self.inputs {
            local_types.insert(param.name.clone(), param.data_type);
        }
        for local in &self.locals {
            local_types.insert(local.name.clone(), local.data_type);
        }
        local_types.insert(self.name.clone(), self.return_type);

        for stmt in &self.body {
            stmt.validate_with_locals(tags, &local_types, &self.name, false)?;
        }
        Ok(())
    }

    pub fn call_with_resolver(
        &self,
        args: &[IecValue],
        resolver: &dyn Fn(&str) -> Result<IecValue, RuntimeError>,
        caller_pou: &str,
    ) -> Result<IecValue, RuntimeError> {
        const MAX_CALL_DEPTH: usize = 64;
        let depth = CALL_DEPTH.with(|d| d.get());
        if depth >= MAX_CALL_DEPTH {
            return Err(RuntimeError::ExecutionError {
                pou: caller_pou.to_string(),
                message: format!(
                    "Maximum function call depth ({MAX_CALL_DEPTH}) exceeded in '{}'",
                    self.name
                ),
            });
        }
        CALL_DEPTH.with(|d| d.set(depth + 1));
        let res = self.execute_internal(args, resolver, caller_pou);
        CALL_DEPTH.with(|d| d.set(depth));
        res
    }

    pub fn call_named_with_resolver(
        &self,
        args: &[(String, IecValue)],
        resolver: &dyn Fn(&str) -> Result<IecValue, RuntimeError>,
        caller_pou: &str,
    ) -> Result<IecValue, RuntimeError> {
        let mut positional = Vec::with_capacity(self.inputs.len());
        for input in &self.inputs {
            let mut found = None;
            for (name, val) in args {
                if name.eq_ignore_ascii_case(&input.name) {
                    found = Some(val.clone());
                    break;
                }
            }
            if let Some(val) = found {
                positional.push(val);
            } else if let Some(def) = &input.default_value {
                positional.push(def.clone());
            } else {
                positional.push(input.data_type.default_value());
            }
        }
        self.call_with_resolver(&positional, resolver, caller_pou)
    }

    fn execute_internal(
        &self,
        args: &[IecValue],
        resolver: &dyn Fn(&str) -> Result<IecValue, RuntimeError>,
        caller_pou: &str,
    ) -> Result<IecValue, RuntimeError> {
        let mut env = HashMap::new();

        for (i, param) in self.inputs.iter().enumerate() {
            let val = if i < args.len() {
                args[i].coerce_to(param.data_type).map_err(|e| RuntimeError::ExecutionError {
                    pou: caller_pou.to_string(),
                    message: format!("Argument type mismatch for '{}': {e}", param.name),
                })?
            } else if let Some(def) = &param.default_value {
                def.clone()
            } else {
                param.data_type.default_value()
            };
            env.insert(param.name.clone(), val);
        }

        for local in &self.locals {
            let val = if let Some(def) = &local.default_value {
                def.clone()
            } else {
                local.data_type.default_value()
            };
            env.insert(local.name.clone(), val);
        }

        env.insert(self.name.clone(), self.return_type.default_value());

        for stmt in &self.body {
            let flow = stmt.execute_with_resolver_and_env(resolver, &mut env, &self.name)?;
            if flow == StmtControl::Return {
                break;
            }
        }

        let ret = env
            .get(&self.name)
            .cloned()
            .or_else(|| {
                for (k, v) in &env {
                    if k.eq_ignore_ascii_case(&self.name) {
                        return Some(v.clone());
                    }
                }
                None
            })
            .unwrap_or_else(|| self.return_type.default_value());

        ret.coerce_to(self.return_type).map_err(|e| RuntimeError::ExecutionError {
            pou: self.name.clone(),
            message: format!("Function return value coercion error: {e}"),
        })
    }
}

static GLOBAL_USER_FUNCTIONS: std::sync::RwLock<Option<HashMap<String, UserFunction>>> =
    std::sync::RwLock::new(None);

pub fn register_user_function(func: UserFunction) {
    if let Ok(mut lock) = GLOBAL_USER_FUNCTIONS.write() {
        let map = lock.get_or_insert_with(HashMap::new);
        map.insert(func.name.to_ascii_uppercase(), func);
    }
}

pub fn get_user_function(name: &str) -> Option<UserFunction> {
    if let Ok(lock) = GLOBAL_USER_FUNCTIONS.read()
        && let Some(map) = lock.as_ref()
    {
        return map.get(&name.to_ascii_uppercase()).cloned();
    }
    None
}

pub fn clear_user_functions() {
    if let Ok(mut lock) = GLOBAL_USER_FUNCTIONS.write()
        && let Some(map) = lock.as_mut()
    {
        map.clear();
    }
}

pub fn all_user_functions() -> Vec<UserFunction> {
    if let Ok(lock) = GLOBAL_USER_FUNCTIONS.read()
        && let Some(map) = lock.as_ref()
    {
        return map.values().cloned().collect();
    }
    Vec::new()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserFunctionRegistry {
    functions: HashMap<String, UserFunction>,
}

impl UserFunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, func: UserFunction) {
        self.functions.insert(func.name.to_ascii_uppercase(), func);
    }

    pub fn get(&self, name: &str) -> Option<&UserFunction> {
        self.functions.get(&name.to_ascii_uppercase())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_ascii_uppercase())
    }

    pub fn clear(&mut self) {
        self.functions.clear();
    }
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
    NamedCall {
        function_name: String,
        args: Vec<(String, IrExpr)>,
    },
}

impl IrExpr {
    /// Pure, read-only evaluation of an expression using an arbitrary symbol resolver closure.
    pub fn evaluate_with_resolver(
        &self,
        resolver: &dyn Fn(&str) -> Result<IecValue, RuntimeError>,
        pou_name: &str,
    ) -> Result<IecValue, RuntimeError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Symbol(sym) => resolver(sym),
            Self::Unary { op, expr } => {
                let val = expr.evaluate_with_resolver(resolver, pou_name)?;
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
                let lv = left.evaluate_with_resolver(resolver, pou_name)?;
                let rv = right.evaluate_with_resolver(resolver, pou_name)?;
                eval_binary_op(*op, &lv, &rv, pou_name)
            }
            Self::Call {
                function_name,
                args,
            } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(a.evaluate_with_resolver(resolver, pou_name)?);
                }
                if lookup_standard_function(function_name).is_some() {
                    eval_math_function(function_name, &evaluated_args, pou_name)
                } else if let Some(user_fn) = get_user_function(function_name) {
                    user_fn.call_with_resolver(&evaluated_args, resolver, pou_name)
                } else {
                    eval_math_function(function_name, &evaluated_args, pou_name)
                }
            }
            Self::NamedCall {
                function_name,
                args,
            } => {
                let mut evaluated_named = Vec::with_capacity(args.len());
                for (name, a) in args {
                    let val = a.evaluate_with_resolver(resolver, pou_name)?;
                    evaluated_named.push((name.clone(), val));
                }
                if let Some(user_fn) = get_user_function(function_name) {
                    user_fn.call_named_with_resolver(&evaluated_named, resolver, pou_name)
                } else {
                    Err(RuntimeError::ExecutionError {
                        pou: pou_name.to_string(),
                        message: format!("Unknown user function '{function_name}' for named argument call"),
                    })
                }
            }
        }
    }

    pub fn evaluate(
        &self,
        ctx: &ScanContext<'_>,
        pou_name: &str,
    ) -> Result<IecValue, RuntimeError> {
        self.evaluate_with_resolver(&|sym| ctx.read_symbol(sym), pou_name)
    }

    pub fn evaluate_with_env(
        &self,
        ctx: &ScanContext<'_>,
        env: &HashMap<String, IecValue>,
        pou_name: &str,
    ) -> Result<IecValue, RuntimeError> {
        self.evaluate_with_resolver(
            &|sym| {
                if let Some(val) = env.get(sym) {
                    return Ok(val.clone());
                }
                for (k, v) in env.iter() {
                    if k.eq_ignore_ascii_case(sym) {
                        return Ok(v.clone());
                    }
                }
                ctx.read_symbol(sym)
            },
            pou_name,
        )
    }

    pub fn infer_type(&self, tags: &TagTable, pou_name: &str) -> Result<IecType, IrError> {
        self.infer_type_with_locals(tags, &HashMap::new(), pou_name)
    }

    pub fn infer_type_with_locals(
        &self,
        tags: &TagTable,
        locals: &HashMap<String, IecType>,
        pou_name: &str,
    ) -> Result<IecType, IrError> {
        match self {
            Self::Literal(v) => Ok(v.iec_type()),
            Self::Symbol(sym) => {
                for (k, ty) in locals {
                    if k.eq_ignore_ascii_case(sym) {
                        return Ok(*ty);
                    }
                }
                tags.symbol_type(sym).map_err(|source| IrError::Core {
                    pou: pou_name.to_string(),
                    source,
                })
            }
            Self::Call {
                function_name,
                args,
            } => {
                if let Some(info) = lookup_standard_function(function_name) {
                    if args.len() < info.min_arity {
                        return Err(IrError::Validation {
                            pou: pou_name.to_string(),
                            message: format!(
                                "Standard function '{}' expects at least {} arguments, found {}",
                                info.name,
                                info.min_arity,
                                args.len()
                            ),
                        });
                    }
                    if let Some(max) = info.max_arity
                        && args.len() > max
                    {
                        return Err(IrError::Validation {
                            pou: pou_name.to_string(),
                            message: format!(
                                "Standard function '{}' expects at most {} arguments, found {}",
                                info.name,
                                max,
                                args.len()
                            ),
                        });
                    }
                    if let Some((_src, dst)) = info.name.split_once("_TO_")
                        && let Ok(target_ty) = dst.parse::<IecType>()
                    {
                        for a in args {
                            let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                        }
                        return Ok(target_ty);
                    }
                    match info.name {
                        "ABS" => {
                            let ty = args[0].infer_type_with_locals(tags, locals, pou_name)?;
                            if ty.is_numeric() {
                                Ok(ty)
                            } else {
                                Err(IrError::TypeMismatch {
                                    pou: pou_name.to_string(),
                                    expected: IecType::Real,
                                    actual: ty,
                                    context: "ABS argument".to_string(),
                                })
                            }
                        }
                        "MIN" | "MAX" => {
                            let first_ty = args[0].infer_type_with_locals(tags, locals, pou_name)?;
                            for a in &args[1..] {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(first_ty)
                        }
                        "LIMIT" => {
                            let _ = args[0].infer_type_with_locals(tags, locals, pou_name)?;
                            let val_ty = args[1].infer_type_with_locals(tags, locals, pou_name)?;
                            let _ = args[2].infer_type_with_locals(tags, locals, pou_name)?;
                            Ok(val_ty)
                        }
                        "SEL" => {
                            let _ = args[0].infer_type_with_locals(tags, locals, pou_name)?;
                            let in0_ty = args[1].infer_type_with_locals(tags, locals, pou_name)?;
                            let _ = args[2].infer_type_with_locals(tags, locals, pou_name)?;
                            Ok(in0_ty)
                        }
                        "MUX" => {
                            let _ = args[0].infer_type_with_locals(tags, locals, pou_name)?;
                            let in0_ty = args[1].infer_type_with_locals(tags, locals, pou_name)?;
                            for a in &args[2..] {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(in0_ty)
                        }
                        "SHL" | "SHR" | "ROL" | "ROR" => {
                            let in_ty = args[0].infer_type_with_locals(tags, locals, pou_name)?;
                            let _ = args[1].infer_type_with_locals(tags, locals, pou_name)?;
                            Ok(in_ty)
                        }
                        "INT_TO_REAL" | "DINT_TO_REAL" | "BOOL_TO_REAL" | "TIME_TO_REAL"
                        | "SQRT" | "SIN" | "COS" | "TAN" | "LN" | "EXP" | "EXPT"
                        | "TRUNC" | "ROUND" | "CEIL" | "FLOOR" | "ASIN" | "ACOS" | "ATAN" | "LOG" => {
                            for a in args {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(IecType::Real)
                        }
                        "REAL_TO_INT" | "DINT_TO_INT" | "BOOL_TO_INT" | "LEN" | "FIND" => {
                            for a in args {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(IecType::Int)
                        }
                        "REAL_TO_DINT" | "INT_TO_DINT" | "BOOL_TO_DINT" | "TIME_TO_DINT" => {
                            for a in args {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(IecType::Dint)
                        }
                        "INT_TO_BOOL" | "DINT_TO_BOOL" | "REAL_TO_BOOL" => {
                            for a in args {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(IecType::Bool)
                        }
                        "CONCAT" | "LEFT" | "RIGHT" | "MID" | "INSERT" | "DELETE" | "REPLACE" => {
                            for a in args {
                                let _ = a.infer_type_with_locals(tags, locals, pou_name)?;
                            }
                            Ok(IecType::String)
                        }
                        _ => Ok(IecType::Real),
                    }
                } else if let Some(user_fn) = get_user_function(function_name) {
                    if args.len() != user_fn.inputs.len() {
                        return Err(IrError::Validation {
                            pou: pou_name.to_string(),
                            message: format!(
                                "Function '{}' expects {} arguments, found {}",
                                user_fn.name,
                                user_fn.inputs.len(),
                                args.len()
                            ),
                        });
                    }
                    for (i, a) in args.iter().enumerate() {
                        let aty = a.infer_type_with_locals(tags, locals, pou_name)?;
                        let pty = user_fn.inputs[i].data_type;
                        if !types_compatible(pty, aty) {
                            return Err(IrError::TypeMismatch {
                                pou: pou_name.to_string(),
                                expected: pty,
                                actual: aty,
                                context: format!("Argument {} of function '{}'", i + 1, user_fn.name),
                            });
                        }
                    }
                    Ok(user_fn.return_type)
                } else {
                    Err(IrError::Validation {
                        pou: pou_name.to_string(),
                        message: format!("Unknown standard function '{function_name}'"),
                    })
                }
            }
            Self::NamedCall {
                function_name,
                args,
            } => {
                let user_fn = get_user_function(function_name).ok_or_else(|| {
                    IrError::Validation {
                        pou: pou_name.to_string(),
                        message: format!("Unknown user function '{function_name}'"),
                    }
                })?;
                for (name, a) in args {
                    let aty = a.infer_type_with_locals(tags, locals, pou_name)?;
                    let param = user_fn
                        .inputs
                        .iter()
                        .find(|p| p.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| IrError::Validation {
                            pou: pou_name.to_string(),
                            message: format!("Unknown parameter '{name}' in function '{}'", user_fn.name),
                        })?;
                    if !types_compatible(param.data_type, aty) {
                        return Err(IrError::TypeMismatch {
                            pou: pou_name.to_string(),
                            expected: param.data_type,
                            actual: aty,
                            context: format!("Parameter '{name}' of function '{}'", user_fn.name),
                        });
                    }
                }
                Ok(user_fn.return_type)
            }
            Self::Unary { op, expr } => {
                let ty = expr.infer_type_with_locals(tags, locals, pou_name)?;
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
                        if ty.is_numeric() {
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
                let lt = left.infer_type_with_locals(tags, locals, pou_name)?;
                let rt = right.infer_type_with_locals(tags, locals, pou_name)?;
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
                        if lt == rt || (lt.is_numeric() && rt.is_numeric()) || (lt.is_bit_string() && rt.is_bit_string()) {
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
                        if lt == IecType::Lreal || rt == IecType::Lreal {
                            Ok(IecType::Lreal)
                        } else if lt == IecType::Real || rt == IecType::Real {
                            Ok(IecType::Real)
                        } else if lt == IecType::Time || rt == IecType::Time {
                            Ok(IecType::Time)
                        } else if lt == IecType::Lint || rt == IecType::Lint {
                            Ok(IecType::Lint)
                        } else if lt == IecType::Dint || rt == IecType::Dint {
                            Ok(IecType::Dint)
                        } else if lt == IecType::Int && rt == IecType::Int {
                            Ok(IecType::Int)
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
            Self::NamedCall { args, .. } => {
                for (_, a) in args {
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
    let info = lookup_standard_function(func).ok_or_else(|| RuntimeError::ExecutionError {
        pou: pou_name.to_string(),
        message: format!("Unknown standard function '{func}'"),
    })?;
    if args.len() < info.min_arity {
        return Err(RuntimeError::ExecutionError {
            pou: pou_name.to_string(),
            message: format!(
                "{} requires at least {} argument{}",
                info.name,
                info.min_arity,
                if info.min_arity > 1 { "s" } else { "" }
            ),
        });
    }
    if let Some(max) = info.max_arity
        && args.len() > max
    {
        return Err(RuntimeError::ExecutionError {
            pou: pou_name.to_string(),
            message: format!(
                "{} requires at most {} argument{}",
                info.name,
                max,
                if max > 1 { "s" } else { "" }
            ),
        });
    }
    let name = info.name;
    match name {
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
        "CEIL" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "CEIL requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.ceil()))
        }
        "FLOOR" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "FLOOR requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.floor()))
        }
        "ASIN" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "ASIN requires 1 argument".to_string(),
            })?;
            let r = v.as_real()?;
            if !(-1.0..=1.0).contains(&r) {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("ASIN domain error: value {r} out of [-1, 1]"),
                });
            }
            Ok(IecValue::Real(r.asin()))
        }
        "ACOS" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "ACOS requires 1 argument".to_string(),
            })?;
            let r = v.as_real()?;
            if !(-1.0..=1.0).contains(&r) {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("ACOS domain error: value {r} out of [-1, 1]"),
                });
            }
            Ok(IecValue::Real(r.acos()))
        }
        "ATAN" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "ATAN requires 1 argument".to_string(),
            })?;
            Ok(IecValue::Real(v.as_real()?.atan()))
        }
        "LOG" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "LOG requires 1 argument".to_string(),
            })?;
            let r = v.as_real()?;
            if r <= 0.0 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("LOG domain error: non-positive value {r}"),
                });
            }
            Ok(IecValue::Real(r.log10()))
        }
        "SEL" => {
            if args.len() < 3 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "SEL requires 3 arguments (G, IN0, IN1)".to_string(),
                });
            }
            let g = args[0].as_bool()?;
            Ok(if g { args[2].clone() } else { args[1].clone() })
        }
        "MUX" => {
            let k_raw = args[0].as_dint()?;
            let num_inputs = args.len() - 1;
            if k_raw < 0 || (k_raw as usize) >= num_inputs {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!(
                        "MUX selector K={k_raw} out of range [0, {}]",
                        num_inputs - 1
                    ),
                });
            }
            let idx = 1 + (k_raw as usize);
            Ok(args[idx].clone())
        }
        "SHL" | "SHR" | "ROL" | "ROR" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("{name} requires 2 arguments (IN, N)"),
                });
            }
            let n = (args[1].as_dint()?.max(0) as u32) & 31;
            match &args[0] {
                IecValue::Int(v) => {
                    let u = *v as u16;
                    let n16 = n & 15;
                    let res = match name {
                        "SHL" => u.wrapping_shl(n16),
                        "SHR" => u.wrapping_shr(n16),
                        "ROL" => u.rotate_left(n16),
                        _ => u.rotate_right(n16),
                    };
                    Ok(IecValue::Int(res as i16))
                }
                other => {
                    let u = other.as_dint()? as u32;
                    let res = match name {
                        "SHL" => u.wrapping_shl(n),
                        "SHR" => u.wrapping_shr(n),
                        "ROL" => u.rotate_left(n),
                        _ => u.rotate_right(n),
                    };
                    Ok(IecValue::Dint(res as i32))
                }
            }
        }
        "INT_TO_REAL" | "DINT_TO_REAL" | "BOOL_TO_REAL" | "TIME_TO_REAL" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: format!("{name} requires 1 argument"),
            })?;
            let r = match v {
                IecValue::Bool(b) => {
                    if *b {
                        1.0
                    } else {
                        0.0
                    }
                }
                IecValue::Time(ms) => *ms as f32,
                other => other.as_real()?,
            };
            Ok(IecValue::Real(r))
        }
        "REAL_TO_INT" | "DINT_TO_INT" | "BOOL_TO_INT" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: format!("{name} requires 1 argument"),
            })?;
            let i = match v {
                IecValue::Real(r) => r.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
                IecValue::Bool(b) => {
                    if *b {
                        1
                    } else {
                        0
                    }
                }
                other => other.as_int()?,
            };
            Ok(IecValue::Int(i))
        }
        "REAL_TO_DINT" | "INT_TO_DINT" | "BOOL_TO_DINT" | "TIME_TO_DINT" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: format!("{name} requires 1 argument"),
            })?;
            let d = match v {
                IecValue::Real(r) => r.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32,
                IecValue::Bool(b) => {
                    if *b {
                        1
                    } else {
                        0
                    }
                }
                IecValue::Time(ms) => *ms as i32,
                other => other.as_dint()?,
            };
            Ok(IecValue::Dint(d))
        }
        "INT_TO_BOOL" | "DINT_TO_BOOL" | "REAL_TO_BOOL" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: format!("{name} requires 1 argument"),
            })?;
            let b = match v {
                IecValue::Bool(b) => *b,
                IecValue::Real(r) => r.abs() > f32::EPSILON,
                other => other.as_dint()? != 0,
            };
            Ok(IecValue::Bool(b))
        }
        "LEN" => {
            let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "LEN requires 1 argument".to_string(),
            })?;
            let len = match v {
                IecValue::String(s) => s.chars().count() as i16,
                other => other.to_string().chars().count() as i16,
            };
            Ok(IecValue::Int(len))
        }
        "CONCAT" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "CONCAT requires at least 2 arguments".to_string(),
                });
            }
            let mut out = String::new();
            for a in args {
                match a {
                    IecValue::String(s) => out.push_str(s),
                    other => out.push_str(&other.to_string()),
                }
            }
            Ok(IecValue::String(out))
        }
        "LEFT" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "LEFT requires 2 arguments (STR, L)".to_string(),
                });
            }
            let s = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let count = args[1].as_dint()?.max(0) as usize;
            let res: String = s.chars().take(count).collect();
            Ok(IecValue::String(res))
        }
        "RIGHT" => {
            if args.len() < 2 {
                return Err(RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: "RIGHT requires 2 arguments (STR, L)".to_string(),
                });
            }
            let s = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let count = args[1].as_dint()?.max(0) as usize;
            let chars: Vec<char> = s.chars().collect();
            let start = chars.len().saturating_sub(count);
            let res: String = chars[start..].iter().collect();
            Ok(IecValue::String(res))
        }
        "MID" => {
            let s = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let l = args[1].as_dint()?.max(0) as usize;
            let p = args[2].as_dint()?;
            let start = if p <= 0 { 0 } else { (p as usize).saturating_sub(1) };
            let chars: Vec<char> = s.chars().collect();
            let res: String = if start < chars.len() {
                chars[start..].iter().take(l).collect()
            } else {
                String::new()
            };
            Ok(IecValue::String(res))
        }
        "INSERT" => {
            let s1 = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let s2 = match &args[1] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let p = args[2].as_dint()?.max(0) as usize;
            let chars: Vec<char> = s1.chars().collect();
            let split_pos = p.min(chars.len());
            let prefix: String = chars[..split_pos].iter().collect();
            let suffix: String = chars[split_pos..].iter().collect();
            Ok(IecValue::String(format!("{prefix}{s2}{suffix}")))
        }
        "DELETE" => {
            let s = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let l = args[1].as_dint()?.max(0) as usize;
            let p = args[2].as_dint()?;
            let start = if p <= 0 { 0 } else { (p as usize).saturating_sub(1) };
            let chars: Vec<char> = s.chars().collect();
            if start >= chars.len() {
                return Ok(IecValue::String(s));
            }
            let end = (start + l).min(chars.len());
            let prefix: String = chars[..start].iter().collect();
            let suffix: String = chars[end..].iter().collect();
            Ok(IecValue::String(format!("{prefix}{suffix}")))
        }
        "REPLACE" => {
            let s1 = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let s2 = match &args[1] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let l = args[2].as_dint()?.max(0) as usize;
            let p = args[3].as_dint()?;
            let start = if p <= 0 { 0 } else { (p as usize).saturating_sub(1) };
            let chars: Vec<char> = s1.chars().collect();
            if start >= chars.len() {
                return Ok(IecValue::String(format!("{s1}{s2}")));
            }
            let end = (start + l).min(chars.len());
            let prefix: String = chars[..start].iter().collect();
            let suffix: String = chars[end..].iter().collect();
            Ok(IecValue::String(format!("{prefix}{s2}{suffix}")))
        }
        "FIND" => {
            let s1 = match &args[0] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let s2 = match &args[1] {
                IecValue::String(s) => s.clone(),
                other => other.to_string(),
            };
            let pos = if let Some(byte_idx) = s1.find(&s2) {
                let char_count = s1[..byte_idx].chars().count();
                (char_count as i16) + 1
            } else {
                0
            };
            Ok(IecValue::Int(pos))
        }
        other => {
            if let Some((_src, dst_str)) = other.split_once("_TO_")
                && let Ok(target_type) = dst_str.parse::<IecType>()
            {
                let v = args.first().ok_or_else(|| RuntimeError::ExecutionError {
                    pou: pou_name.to_string(),
                    message: format!("{other} requires 1 argument"),
                })?;
                let coerced = v.coerce_to(target_type).map_err(RuntimeError::Core)?;
                return Ok(coerced);
            }
            Err(RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: format!("Unknown standard function '{other}'"),
            })
        }
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

/// Control-flow signal returned during statement execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StmtControl {
    None,
    Break,
    Continue,
    Return,
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
    Repeat {
        body: Vec<IrStmt>,
        until_cond: IrExpr,
    },
    Exit,
    Continue,
    Return,
    FbCall {
        instance: String,
        inputs: Vec<(String, IrExpr)>,
        outputs: Vec<(String, String)>,
    },
}

impl IrStmt {
    pub fn execute(&self, ctx: &mut ScanContext<'_>, pou_name: &str) -> Result<(), RuntimeError> {
        let mut empty_env = HashMap::new();
        let _ = self.execute_with_env(ctx, &mut empty_env, pou_name)?;
        Ok(())
    }

    pub fn execute_block(
        stmts: &[Self],
        ctx: &mut ScanContext<'_>,
        pou_name: &str,
    ) -> Result<StmtControl, RuntimeError> {
        let mut env = HashMap::new();
        Self::execute_block_with_env(stmts, ctx, &mut env, pou_name)
    }

    pub fn execute_block_with_env(
        stmts: &[Self],
        ctx: &mut ScanContext<'_>,
        env: &mut HashMap<String, IecValue>,
        pou_name: &str,
    ) -> Result<StmtControl, RuntimeError> {
        for stmt in stmts {
            let flow = stmt.execute_with_env(ctx, env, pou_name)?;
            if flow != StmtControl::None {
                return Ok(flow);
            }
        }
        Ok(StmtControl::None)
    }

    pub fn execute_with_env(
        &self,
        ctx: &mut ScanContext<'_>,
        env: &mut HashMap<String, IecValue>,
        pou_name: &str,
    ) -> Result<StmtControl, RuntimeError> {
        match self {
            Self::Assign { target, expr } => {
                let val = expr.evaluate_with_env(ctx, env, pou_name)?;
                let mut matched_key = None;
                for k in env.keys() {
                    if k.eq_ignore_ascii_case(target) {
                        matched_key = Some(k.clone());
                        break;
                    }
                }
                if let Some(k) = matched_key {
                    env.insert(k, val);
                    return Ok(StmtControl::None);
                }
                if ctx.tags.tag_id(target).is_some()
                    || target.starts_with('%')
                    || target.contains('.')
                {
                    ctx.write_symbol(target, val)?;
                } else {
                    env.insert(target.clone(), val);
                }
                Ok(StmtControl::None)
            }
            Self::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    if cond.evaluate_with_env(ctx, env, pou_name)?.as_bool()? {
                        for stmt in body {
                            let flow = stmt.execute_with_env(ctx, env, pou_name)?;
                            if flow != StmtControl::None {
                                return Ok(flow);
                            }
                        }
                        return Ok(StmtControl::None);
                    }
                }
                for stmt in else_body {
                    let flow = stmt.execute_with_env(ctx, env, pou_name)?;
                    if flow != StmtControl::None {
                        return Ok(flow);
                    }
                }
                Ok(StmtControl::None)
            }
            Self::Case {
                selector,
                arms,
                else_body,
            } => {
                let sel_val = selector.evaluate_with_env(ctx, env, pou_name)?.as_dint()?;
                for (labels, body) in arms {
                    if labels.contains(&sel_val) {
                        for stmt in body {
                            let flow = stmt.execute_with_env(ctx, env, pou_name)?;
                            if flow != StmtControl::None {
                                return Ok(flow);
                            }
                        }
                        return Ok(StmtControl::None);
                    }
                }
                for stmt in else_body {
                    let flow = stmt.execute_with_env(ctx, env, pou_name)?;
                    if flow != StmtControl::None {
                        return Ok(flow);
                    }
                }
                Ok(StmtControl::None)
            }
            Self::For {
                var,
                start,
                end,
                step,
                body,
            } => {
                let start_val = start.evaluate_with_env(ctx, env, pou_name)?.as_dint()?;
                let end_val = end.evaluate_with_env(ctx, env, pou_name)?.as_dint()?;
                let step_val = match step {
                    Some(s) => s.evaluate_with_env(ctx, env, pou_name)?.as_dint()?,
                    None => 1,
                };
                if step_val == 0 {
                    return Err(RuntimeError::ExecutionError {
                        pou: pou_name.to_string(),
                        message: "FOR loop step cannot be zero".to_string(),
                    });
                }
                let mut cur = start_val;
                let mut iters = 0usize;
                let max_iters = max_loop_iterations();
                while (step_val > 0 && cur <= end_val) || (step_val < 0 && cur >= end_val) {
                    if iters >= max_iters {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: max_iters,
                        });
                    }
                    env.insert(var.clone(), IecValue::Dint(cur));
                    let mut broke = false;
                    for stmt in body {
                        match stmt.execute_with_env(ctx, env, pou_name)? {
                            StmtControl::None => {}
                            StmtControl::Break => {
                                broke = true;
                                break;
                            }
                            StmtControl::Continue => break,
                            StmtControl::Return => return Ok(StmtControl::Return),
                        }
                    }
                    if broke {
                        break;
                    }
                    cur = cur.wrapping_add(step_val);
                    iters += 1;
                }
                Ok(StmtControl::None)
            }
            Self::While { cond, body } => {
                let mut iters = 0usize;
                let max_iters = max_loop_iterations();
                while cond.evaluate_with_env(ctx, env, pou_name)?.as_bool()? {
                    if iters >= max_iters {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: max_iters,
                        });
                    }
                    let mut broke = false;
                    for stmt in body {
                        match stmt.execute_with_env(ctx, env, pou_name)? {
                            StmtControl::None => {}
                            StmtControl::Break => {
                                broke = true;
                                break;
                            }
                            StmtControl::Continue => break,
                            StmtControl::Return => return Ok(StmtControl::Return),
                        }
                    }
                    if broke {
                        break;
                    }
                    iters += 1;
                }
                Ok(StmtControl::None)
            }
            Self::Repeat { body, until_cond } => {
                let mut iters = 0usize;
                let max_iters = max_loop_iterations();
                loop {
                    if iters >= max_iters {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: max_iters,
                        });
                    }
                    let mut broke = false;
                    for stmt in body {
                        match stmt.execute_with_env(ctx, env, pou_name)? {
                            StmtControl::None => {}
                            StmtControl::Break => {
                                broke = true;
                                break;
                            }
                            StmtControl::Continue => break,
                            StmtControl::Return => return Ok(StmtControl::Return),
                        }
                    }
                    if broke {
                        break;
                    }
                    iters += 1;
                    if until_cond.evaluate_with_env(ctx, env, pou_name)?.as_bool()? {
                        break;
                    }
                }
                Ok(StmtControl::None)
            }
            Self::Exit => Ok(StmtControl::Break),
            Self::Continue => Ok(StmtControl::Continue),
            Self::Return => Ok(StmtControl::Return),
            Self::FbCall {
                instance,
                inputs,
                outputs,
            } => {
                for (pin, expr) in inputs {
                    let val = expr.evaluate_with_env(ctx, env, pou_name)?;
                    let sym = format!("{instance}.{pin}");
                    ctx.write_symbol(&sym, val)?;
                }
                ctx.evaluate_fb(instance)?;
                for (pin, target_sym) in outputs {
                    let sym = format!("{instance}.{pin}");
                    let out_val = ctx.read_symbol(&sym)?;
                    let mut matched_key = None;
                    for k in env.keys() {
                        if k.eq_ignore_ascii_case(target_sym) {
                            matched_key = Some(k.clone());
                            break;
                        }
                    }
                    if let Some(k) = matched_key {
                        env.insert(k, out_val);
                    } else {
                        ctx.write_symbol(target_sym, out_val)?;
                    }
                }
                Ok(StmtControl::None)
            }
        }
    }

    pub fn execute_with_resolver_and_env(
        &self,
        resolver: &dyn Fn(&str) -> Result<IecValue, RuntimeError>,
        env: &mut HashMap<String, IecValue>,
        pou_name: &str,
    ) -> Result<StmtControl, RuntimeError> {
        match self {
            Self::Assign { target, expr } => {
                let val = expr.evaluate_with_resolver(
                    &|sym| {
                        if let Some(v) = env.get(sym) {
                            return Ok(v.clone());
                        }
                        for (k, v) in env.iter() {
                            if k.eq_ignore_ascii_case(sym) {
                                return Ok(v.clone());
                            }
                        }
                        resolver(sym)
                    },
                    pou_name,
                )?;
                let mut matched_key = None;
                for k in env.keys() {
                    if k.eq_ignore_ascii_case(target) {
                        matched_key = Some(k.clone());
                        break;
                    }
                }
                if let Some(k) = matched_key {
                    env.insert(k, val);
                } else {
                    env.insert(target.clone(), val);
                }
                Ok(StmtControl::None)
            }
            Self::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    let cond_val = cond.evaluate_with_resolver(
                        &|sym| {
                            if let Some(v) = env.get(sym) {
                                return Ok(v.clone());
                            }
                            for (k, v) in env.iter() {
                                if k.eq_ignore_ascii_case(sym) {
                                    return Ok(v.clone());
                                }
                            }
                            resolver(sym)
                        },
                        pou_name,
                    )?;
                    if cond_val.as_bool()? {
                        for stmt in body {
                            let flow = stmt.execute_with_resolver_and_env(resolver, env, pou_name)?;
                            if flow != StmtControl::None {
                                return Ok(flow);
                            }
                        }
                        return Ok(StmtControl::None);
                    }
                }
                for stmt in else_body {
                    let flow = stmt.execute_with_resolver_and_env(resolver, env, pou_name)?;
                    if flow != StmtControl::None {
                        return Ok(flow);
                    }
                }
                Ok(StmtControl::None)
            }
            Self::Case {
                selector,
                arms,
                else_body,
            } => {
                let sel_val = selector.evaluate_with_resolver(
                    &|sym| {
                        if let Some(v) = env.get(sym) {
                            return Ok(v.clone());
                        }
                        for (k, v) in env.iter() {
                            if k.eq_ignore_ascii_case(sym) {
                                return Ok(v.clone());
                            }
                        }
                        resolver(sym)
                    },
                    pou_name,
                )?.as_dint()?;
                for (labels, body) in arms {
                    if labels.contains(&sel_val) {
                        for stmt in body {
                            let flow = stmt.execute_with_resolver_and_env(resolver, env, pou_name)?;
                            if flow != StmtControl::None {
                                return Ok(flow);
                            }
                        }
                        return Ok(StmtControl::None);
                    }
                }
                for stmt in else_body {
                    let flow = stmt.execute_with_resolver_and_env(resolver, env, pou_name)?;
                    if flow != StmtControl::None {
                        return Ok(flow);
                    }
                }
                Ok(StmtControl::None)
            }
            Self::For {
                var,
                start,
                end,
                step,
                body,
            } => {
                let eval_env = |e: &IrExpr| e.evaluate_with_resolver(
                    &|sym| {
                        if let Some(v) = env.get(sym) {
                            return Ok(v.clone());
                        }
                        for (k, v) in env.iter() {
                            if k.eq_ignore_ascii_case(sym) {
                                return Ok(v.clone());
                            }
                        }
                        resolver(sym)
                    },
                    pou_name,
                );
                let start_val = eval_env(start)?.as_dint()?;
                let end_val = eval_env(end)?.as_dint()?;
                let step_val = match step {
                    Some(s) => eval_env(s)?.as_dint()?,
                    None => 1,
                };
                if step_val == 0 {
                    return Err(RuntimeError::ExecutionError {
                        pou: pou_name.to_string(),
                        message: "FOR loop step cannot be zero".to_string(),
                    });
                }
                let mut cur = start_val;
                let mut iters = 0usize;
                let max_iters = max_loop_iterations();
                while (step_val > 0 && cur <= end_val) || (step_val < 0 && cur >= end_val) {
                    if iters >= max_iters {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: max_iters,
                        });
                    }
                    env.insert(var.clone(), IecValue::Dint(cur));
                    let mut broke = false;
                    for stmt in body {
                        match stmt.execute_with_resolver_and_env(resolver, env, pou_name)? {
                            StmtControl::None => {}
                            StmtControl::Break => {
                                broke = true;
                                break;
                            }
                            StmtControl::Continue => break,
                            StmtControl::Return => return Ok(StmtControl::Return),
                        }
                    }
                    if broke {
                        break;
                    }
                    cur = cur.wrapping_add(step_val);
                    iters += 1;
                }
                Ok(StmtControl::None)
            }
            Self::While { cond, body } => {
                let mut iters = 0usize;
                let max_iters = max_loop_iterations();
                while cond.evaluate_with_resolver(
                    &|sym| {
                        if let Some(v) = env.get(sym) {
                            return Ok(v.clone());
                        }
                        for (k, v) in env.iter() {
                            if k.eq_ignore_ascii_case(sym) {
                                return Ok(v.clone());
                            }
                        }
                        resolver(sym)
                    },
                    pou_name,
                )?.as_bool()? {
                    if iters >= max_iters {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: max_iters,
                        });
                    }
                    let mut broke = false;
                    for stmt in body {
                        match stmt.execute_with_resolver_and_env(resolver, env, pou_name)? {
                            StmtControl::None => {}
                            StmtControl::Break => {
                                broke = true;
                                break;
                            }
                            StmtControl::Continue => break,
                            StmtControl::Return => return Ok(StmtControl::Return),
                        }
                    }
                    if broke {
                        break;
                    }
                    iters += 1;
                }
                Ok(StmtControl::None)
            }
            Self::Repeat { body, until_cond } => {
                let mut iters = 0usize;
                let max_iters = max_loop_iterations();
                loop {
                    if iters >= max_iters {
                        return Err(RuntimeError::IterationLimitExceeded {
                            pou: pou_name.to_string(),
                            max_iterations: max_iters,
                        });
                    }
                    let mut broke = false;
                    for stmt in body {
                        match stmt.execute_with_resolver_and_env(resolver, env, pou_name)? {
                            StmtControl::None => {}
                            StmtControl::Break => {
                                broke = true;
                                break;
                            }
                            StmtControl::Continue => break,
                            StmtControl::Return => return Ok(StmtControl::Return),
                        }
                    }
                    if broke {
                        break;
                    }
                    iters += 1;
                    if until_cond.evaluate_with_resolver(
                        &|sym| {
                            if let Some(v) = env.get(sym) {
                                return Ok(v.clone());
                            }
                            for (k, v) in env.iter() {
                                if k.eq_ignore_ascii_case(sym) {
                                    return Ok(v.clone());
                                }
                            }
                            resolver(sym)
                        },
                        pou_name,
                    )?.as_bool()? {
                        break;
                    }
                }
                Ok(StmtControl::None)
            }
            Self::Exit => Ok(StmtControl::Break),
            Self::Continue => Ok(StmtControl::Continue),
            Self::Return => Ok(StmtControl::Return),
            Self::FbCall { .. } => Err(RuntimeError::ExecutionError {
                pou: pou_name.to_string(),
                message: "FB calls inside pure functions without ScanContext are not supported".to_string(),
            }),
        }
    }

    pub fn validate(&self, tags: &TagTable, pou_name: &str, in_sfc: bool) -> Result<(), IrError> {
        self.validate_with_locals(tags, &HashMap::new(), pou_name, in_sfc)
    }

    pub fn validate_with_locals(
        &self,
        tags: &TagTable,
        locals: &HashMap<String, IecType>,
        pou_name: &str,
        in_sfc: bool,
    ) -> Result<(), IrError> {
        match self {
            Self::Assign { target, expr } => {
                let target_ty = {
                    let mut local_ty = None;
                    for (k, ty) in locals {
                        if k.eq_ignore_ascii_case(target) {
                            local_ty = Some(*ty);
                            break;
                        }
                    }
                    if let Some(ty) = local_ty {
                        ty
                    } else {
                        SymbolResolver::validate_writable_symbol(tags, target, pou_name, in_sfc)?
                    }
                };
                let expr_ty = expr.infer_type_with_locals(tags, locals, pou_name)?;
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
                    let cty = cond.infer_type_with_locals(tags, locals, pou_name)?;
                    if cty != IecType::Bool {
                        return Err(IrError::TypeMismatch {
                            pou: pou_name.to_string(),
                            expected: IecType::Bool,
                            actual: cty,
                            context: "IF condition".to_string(),
                        });
                    }
                    for s in body {
                        s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
                    }
                }
                for s in else_body {
                    s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::Case {
                selector,
                arms,
                else_body,
            } => {
                let sty = selector.infer_type_with_locals(tags, locals, pou_name)?;
                if !matches!(sty, IecType::Int | IecType::Dint) {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Int,
                        actual: sty,
                        context: "CASE selector".to_string(),
                    });
                }
                let mut seen_labels = HashSet::new();
                for (labels, body) in arms {
                    for label in labels {
                        if !seen_labels.insert(*label) {
                            return Err(IrError::Validation {
                                pou: pou_name.to_string(),
                                message: format!("Duplicate CASE label '{label}'"),
                            });
                        }
                    }
                    for s in body {
                        s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
                    }
                }
                for s in else_body {
                    s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
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
                let vty = {
                    let mut local_ty = None;
                    for (k, ty) in locals {
                        if k.eq_ignore_ascii_case(var) {
                            local_ty = Some(*ty);
                            break;
                        }
                    }
                    if let Some(ty) = local_ty {
                        ty
                    } else {
                        SymbolResolver::validate_writable_symbol(tags, var, pou_name, in_sfc)?
                    }
                };
                if !matches!(vty, IecType::Int | IecType::Dint) {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Int,
                        actual: vty,
                        context: "FOR loop variable".to_string(),
                    });
                }
                let _ = start.infer_type_with_locals(tags, locals, pou_name)?;
                let _ = end.infer_type_with_locals(tags, locals, pou_name)?;
                if let Some(s) = step {
                    let _ = s.infer_type_with_locals(tags, locals, pou_name)?;
                }
                for s in body {
                    s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::While { cond, body } => {
                let cty = cond.infer_type_with_locals(tags, locals, pou_name)?;
                if cty != IecType::Bool {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Bool,
                        actual: cty,
                        context: "WHILE condition".to_string(),
                    });
                }
                for s in body {
                    s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
                }
                Ok(())
            }
            Self::Repeat { body, until_cond } => {
                for s in body {
                    s.validate_with_locals(tags, locals, pou_name, in_sfc)?;
                }
                let cty = until_cond.infer_type_with_locals(tags, locals, pou_name)?;
                if cty != IecType::Bool {
                    return Err(IrError::TypeMismatch {
                        pou: pou_name.to_string(),
                        expected: IecType::Bool,
                        actual: cty,
                        context: "REPEAT UNTIL condition".to_string(),
                    });
                }
                Ok(())
            }
            Self::Exit | Self::Continue | Self::Return => Ok(()),
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
                    let ety = expr.infer_type_with_locals(tags, locals, pou_name)?;
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
                    let tty = {
                        let mut local_ty = None;
                        for (k, ty) in locals {
                            if k.eq_ignore_ascii_case(target_sym) {
                                local_ty = Some(*ty);
                                break;
                            }
                        }
                        if let Some(ty) = local_ty {
                            ty
                        } else {
                            SymbolResolver::validate_writable_symbol(
                                tags, target_sym, pou_name, in_sfc,
                            )?
                        }
                    };
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
    if expected.is_numeric() && actual.is_numeric() {
        return true;
    }
    if expected.is_bit_string() && actual.is_bit_string() {
        return true;
    }
    if (expected.is_bit_string() && actual.is_numeric())
        || (expected.is_numeric() && actual.is_bit_string())
    {
        return true;
    }
    if (expected == IecType::Time && (actual.is_numeric() || actual == IecType::Time))
        || (actual == IecType::Time && expected.is_numeric())
    {
        return true;
    }
    false
}

/// Cross-language symbol resolver and validator.
pub struct SymbolResolver;

impl SymbolResolver {
    pub fn validate_readable_symbol(
        tags: &TagTable,
        symbol: &str,
        pou_name: &str,
    ) -> Result<IecType, IrError> {
        tags.symbol_type(symbol.trim())
            .map_err(|source| IrError::Core {
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
                ferrite_core::AccessWidth::LWord => IecType::Lword,
            });
        }
        if let Some((inst, pin)) = trimmed.split_once('.')
            && let Some(fb) = tags.get_fb(inst)
        {
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
            let unsorted: Vec<String> = self
                .nodes
                .iter()
                .filter(|n| !visited.contains(n))
                .cloned()
                .collect();
            let cycle = self.extract_cycle_nodes(&unsorted);
            return Err(IrError::CombinationalLoop {
                scope: scope.to_string(),
                cycle,
            });
        }

        Ok(ordered)
    }

    fn extract_cycle_nodes(&self, unsorted: &[String]) -> Vec<String> {
        let unsorted_set: HashSet<&str> = unsorted.iter().map(|s| s.as_str()).collect();
        let mut index = 0usize;
        let mut indices: HashMap<&str, usize> = HashMap::new();
        let mut lowlink: HashMap<&str, usize> = HashMap::new();
        let mut on_stack: HashSet<&str> = HashSet::new();
        let mut stack: Vec<&str> = Vec::new();
        let mut cycle_nodes = Vec::new();

        #[allow(clippy::too_many_arguments)]
        fn scc<'a>(
            u: &'a str,
            edges: &'a HashMap<String, Vec<String>>,
            unsorted_set: &HashSet<&'a str>,
            index: &mut usize,
            indices: &mut HashMap<&'a str, usize>,
            lowlink: &mut HashMap<&'a str, usize>,
            on_stack: &mut HashSet<&'a str>,
            stack: &mut Vec<&'a str>,
            cycle_nodes: &mut Vec<String>,
        ) {
            indices.insert(u, *index);
            lowlink.insert(u, *index);
            *index += 1;
            stack.push(u);
            on_stack.insert(u);

            if let Some(neighbors) = edges.get(u) {
                for neighbor in neighbors {
                    let v = neighbor.as_str();
                    if !unsorted_set.contains(v) {
                        continue;
                    }
                    if !indices.contains_key(v) {
                        scc(
                            v,
                            edges,
                            unsorted_set,
                            index,
                            indices,
                            lowlink,
                            on_stack,
                            stack,
                            cycle_nodes,
                        );
                        let v_low = lowlink.get(v).copied().unwrap_or(0);
                        if let Some(u_low) = lowlink.get_mut(u) {
                            *u_low = (*u_low).min(v_low);
                        }
                    } else if on_stack.contains(v) {
                        let v_idx = indices.get(v).copied().unwrap_or(0);
                        if let Some(u_low) = lowlink.get_mut(u) {
                            *u_low = (*u_low).min(v_idx);
                        }
                    }
                }
            }

            let u_low = lowlink.get(u).copied().unwrap_or(0);
            let u_idx = indices.get(u).copied().unwrap_or(0);
            if u_low == u_idx {
                let mut component = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack.remove(w);
                    component.push(w.to_string());
                    if w == u {
                        break;
                    }
                }
                let is_cycle = if component.len() > 1 {
                    true
                } else if component.len() == 1 {
                    let node = &component[0];
                    edges
                        .get(node)
                        .is_some_and(|targets| targets.iter().any(|t| t == node))
                } else {
                    false
                };
                if is_cycle && cycle_nodes.is_empty() {
                    component.sort();
                    *cycle_nodes = component;
                }
            }
        }

        for n in unsorted {
            if !indices.contains_key(n.as_str()) {
                scc(
                    n.as_str(),
                    &self.edges,
                    &unsorted_set,
                    &mut index,
                    &mut indices,
                    &mut lowlink,
                    &mut on_stack,
                    &mut stack,
                    &mut cycle_nodes,
                );
            }
        }

        if cycle_nodes.is_empty() {
            unsorted.to_vec()
        } else {
            cycle_nodes
        }
    }

    pub fn pou_ids_in_order(&self, scope: &str) -> Result<Vec<PouId>, IrError> {
        let sorted = self.topological_sort(scope)?;
        Ok(sorted.into_iter().map(PouId).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::CoreError;

    #[test]
    fn test_ir_expr_evaluate_with_resolver() -> Result<(), Box<dyn std::error::Error>> {
        let expr = IrExpr::Binary {
            op: IrBinaryOp::And,
            left: Box::new(IrExpr::Symbol("Start_PB".to_string())),
            right: Box::new(IrExpr::Symbol("Safety_OK".to_string())),
        };
        let lookup = |sym: &str| match sym {
            "Start_PB" => Ok(IecValue::Bool(true)),
            "Safety_OK" => Ok(IecValue::Bool(true)),
            _ => Err(CoreError::UnknownSymbol(sym.to_string()).into()),
        };
        let res = expr.evaluate_with_resolver(&lookup, "main")?;
        assert_eq!(res, IecValue::Bool(true));
        Ok(())
    }

    #[test]
    fn test_i7_unknown_function_and_bad_arity_validation() {
        let tags = TagTable::new();
        // Unknown function
        let unknown = IrExpr::Call {
            function_name: "SQRTT".to_string(),
            args: vec![IrExpr::Literal(IecValue::Real(4.0))],
        };
        assert!(matches!(
            unknown.infer_type(&tags, "main"),
            Err(IrError::Validation { message, .. }) if message.contains("Unknown standard function 'SQRTT'")
        ));

        // Bad arity: ABS with 0 arguments
        let abs_zero = IrExpr::Call {
            function_name: "ABS".to_string(),
            args: vec![],
        };
        assert!(matches!(
            abs_zero.infer_type(&tags, "main"),
            Err(IrError::Validation { message, .. }) if message.contains("expects at least 1 arguments")
        ));
    }

    #[test]
    fn test_mux_out_of_range_k() {
        let k_neg = IrExpr::Call {
            function_name: "MUX".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::Dint(-1)),
                IrExpr::Literal(IecValue::Int(10)),
                IrExpr::Literal(IecValue::Int(20)),
            ],
        };
        let empty_lookup = |sym: &str| Err(CoreError::UnknownSymbol(sym.to_string()).into());
        let res = k_neg.evaluate_with_resolver(&empty_lookup, "main");
        assert!(matches!(res, Err(RuntimeError::ExecutionError { message, .. }) if message.contains("out of range")));

        let k_high = IrExpr::Call {
            function_name: "MUX".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::Dint(5)),
                IrExpr::Literal(IecValue::Int(10)),
                IrExpr::Literal(IecValue::Int(20)),
            ],
        };
        let res2 = k_high.evaluate_with_resolver(&empty_lookup, "main");
        assert!(matches!(res2, Err(RuntimeError::ExecutionError { message, .. }) if message.contains("out of range")));
    }

    #[test]
    fn test_case_duplicate_label_detection() {
        let tags = TagTable::new();
        let case_dup = IrStmt::Case {
            selector: IrExpr::Literal(IecValue::Int(1)),
            arms: vec![
                (vec![1, 2], vec![]),
                (vec![2, 3], vec![]),
            ],
            else_body: vec![],
        };
        let res = case_dup.validate(&tags, "main", false);
        assert!(matches!(
            res,
            Err(IrError::Validation { message, .. }) if message.contains("Duplicate CASE label '2'")
        ));
    }

    #[test]
    fn test_pou_graph_cycle_reporting_only_real_cycle_members() {
        let mut graph = PouGraph::new();
        // A -> B -> A, and B -> C
        graph.add_edge("A", "B");
        graph.add_edge("B", "A");
        graph.add_edge("B", "C");

        let Err(err) = graph.topological_sort("test_scope") else {
            panic!("topological_sort should fail on a cyclic graph");
        };
        match err {
            IrError::CombinationalLoop { cycle, .. } => {
                assert!(cycle.contains(&"A".to_string()));
                assert!(cycle.contains(&"B".to_string()));
                assert!(!cycle.contains(&"C".to_string()), "Downstream node C must not be in cycle report: {cycle:?}");
            }
            other => panic!("Expected CombinationalLoop, got {other:?}"),
        }
    }

    #[test]
    fn test_extended_string_functions() -> Result<(), Box<dyn std::error::Error>> {
        let empty_lookup = |sym: &str| Err(CoreError::UnknownSymbol(sym.to_string()).into());

        // MID(IN, L, P)
        let mid_expr = IrExpr::Call {
            function_name: "MID".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::String("ABCDEF".to_string())),
                IrExpr::Literal(IecValue::Int(3)),
                IrExpr::Literal(IecValue::Int(2)),
            ],
        };
        let res = mid_expr.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::String("BCD".to_string()));

        // INSERT(IN1, IN2, P)
        let ins_expr = IrExpr::Call {
            function_name: "INSERT".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::String("ABC".to_string())),
                IrExpr::Literal(IecValue::String("XY".to_string())),
                IrExpr::Literal(IecValue::Int(1)),
            ],
        };
        let res = ins_expr.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::String("AXYBC".to_string()));

        // DELETE(IN, L, P)
        let del_expr = IrExpr::Call {
            function_name: "DELETE".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::String("ABCDEF".to_string())),
                IrExpr::Literal(IecValue::Int(2)),
                IrExpr::Literal(IecValue::Int(3)),
            ],
        };
        let res = del_expr.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::String("ABEF".to_string()));

        // REPLACE(IN1, IN2, L, P)
        let rep_expr = IrExpr::Call {
            function_name: "REPLACE".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::String("ABCDEF".to_string())),
                IrExpr::Literal(IecValue::String("ZZ".to_string())),
                IrExpr::Literal(IecValue::Int(2)),
                IrExpr::Literal(IecValue::Int(3)),
            ],
        };
        let res = rep_expr.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::String("ABZZEF".to_string()));

        // FIND(IN1, IN2)
        let find_found = IrExpr::Call {
            function_name: "FIND".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::String("ABCDEF".to_string())),
                IrExpr::Literal(IecValue::String("CD".to_string())),
            ],
        };
        let res = find_found.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Int(3));

        let find_not_found = IrExpr::Call {
            function_name: "FIND".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::String("ABCDEF".to_string())),
                IrExpr::Literal(IecValue::String("ZZ".to_string())),
            ],
        };
        let res = find_not_found.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Int(0));

        Ok(())
    }

    #[test]
    fn test_extended_conversion_functions() -> Result<(), Box<dyn std::error::Error>> {
        let empty_lookup = |sym: &str| Err(CoreError::UnknownSymbol(sym.to_string()).into());

        // LREAL_TO_REAL
        let call_lr_to_r = IrExpr::Call {
            function_name: "LREAL_TO_REAL".to_string(),
            args: vec![IrExpr::Literal(IecValue::Lreal(123.456))],
        };
        let res = call_lr_to_r.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Real(123.456));

        // REAL_TO_LREAL
        let call_r_to_lr = IrExpr::Call {
            function_name: "REAL_TO_LREAL".to_string(),
            args: vec![IrExpr::Literal(IecValue::Real(45.5))],
        };
        let res = call_r_to_lr.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Lreal(45.5));

        // LREAL_TO_INT (rounds)
        let call_lr_to_i = IrExpr::Call {
            function_name: "LREAL_TO_INT".to_string(),
            args: vec![IrExpr::Literal(IecValue::Lreal(42.6))],
        };
        let res = call_lr_to_i.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Int(43));

        // INT_TO_BYTE
        let call_i_to_b = IrExpr::Call {
            function_name: "INT_TO_BYTE".to_string(),
            args: vec![IrExpr::Literal(IecValue::Int(250))],
        };
        let res = call_i_to_b.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Byte(250));

        // DINT_TO_DWORD
        let call_d_to_dw = IrExpr::Call {
            function_name: "DINT_TO_DWORD".to_string(),
            args: vec![IrExpr::Literal(IecValue::Dint(0x1234))],
        };
        let res = call_d_to_dw.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Dword(0x1234));

        // INT_TO_SINT
        let call_i_to_sint = IrExpr::Call {
            function_name: "INT_TO_SINT".to_string(),
            args: vec![IrExpr::Literal(IecValue::Int(127))],
        };
        let res = call_i_to_sint.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Sint(127));

        Ok(())
    }

    #[test]
    fn test_user_function_registration_and_invocation() -> Result<(), Box<dyn std::error::Error>> {
        // FUNCTION MultiplyAdd : INT (a: INT, b: INT, c: INT) -> (a * b) + c
        let func = UserFunction::new(
            "MultiplyAdd",
            IecType::Int,
            vec![
                FunctionParam {
                    name: "a".to_string(),
                    data_type: IecType::Int,
                    default_value: None,
                },
                FunctionParam {
                    name: "b".to_string(),
                    data_type: IecType::Int,
                    default_value: None,
                },
                FunctionParam {
                    name: "c".to_string(),
                    data_type: IecType::Int,
                    default_value: Some(IecValue::Int(0)),
                },
            ],
            vec![FunctionParam {
                name: "temp".to_string(),
                data_type: IecType::Int,
                default_value: Some(IecValue::Int(0)),
            }],
            vec![
                IrStmt::Assign {
                    target: "temp".to_string(),
                    expr: IrExpr::Binary {
                        op: IrBinaryOp::Mul,
                        left: Box::new(IrExpr::Symbol("a".to_string())),
                        right: Box::new(IrExpr::Symbol("b".to_string())),
                    },
                },
                IrStmt::Assign {
                    target: "MultiplyAdd".to_string(),
                    expr: IrExpr::Binary {
                        op: IrBinaryOp::Add,
                        left: Box::new(IrExpr::Symbol("temp".to_string())),
                        right: Box::new(IrExpr::Symbol("c".to_string())),
                    },
                },
            ],
        );

        register_user_function(func);

        let empty_lookup = |sym: &str| Err(CoreError::UnknownSymbol(sym.to_string()).into());

        // Positional call: MultiplyAdd(3, 4, 5) -> 17
        let call_expr = IrExpr::Call {
            function_name: "MultiplyAdd".to_string(),
            args: vec![
                IrExpr::Literal(IecValue::Int(3)),
                IrExpr::Literal(IecValue::Int(4)),
                IrExpr::Literal(IecValue::Int(5)),
            ],
        };
        let res = call_expr.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res, IecValue::Int(17));

        // Named call: MultiplyAdd(c := 10, b := 2, a := 6) -> (6 * 2) + 10 = 22
        let named_call = IrExpr::NamedCall {
            function_name: "MultiplyAdd".to_string(),
            args: vec![
                ("c".to_string(), IrExpr::Literal(IecValue::Int(10))),
                ("b".to_string(), IrExpr::Literal(IecValue::Int(2))),
                ("a".to_string(), IrExpr::Literal(IecValue::Int(6))),
            ],
        };
        let res_named = named_call.evaluate_with_resolver(&empty_lookup, "main")?;
        assert_eq!(res_named, IecValue::Int(22));

        // Type inference
        let tags = TagTable::new();
        let inferred = call_expr.infer_type(&tags, "main")?;
        assert_eq!(inferred, IecType::Int);

        let inferred_named = named_call.infer_type(&tags, "main")?;
        assert_eq!(inferred_named, IecType::Int);

        Ok(())
    }
}
