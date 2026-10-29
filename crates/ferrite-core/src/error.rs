use thiserror::Error;

use crate::types::IecType;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CoreError {
    #[error("Invalid direct memory address '{0}'")]
    InvalidDirectAddress(String),

    #[error("Memory address out of bounds: {0}")]
    AddressOutOfBounds(String),

    #[error("Type mismatch: expected {expected:?}, got {actual:?}")]
    TypeMismatch { expected: IecType, actual: IecType },

    #[error("Unknown symbol '{0}'")]
    UnknownSymbol(String),

    #[error("Unknown Function Block instance '{0}'")]
    UnknownFbInstance(String),

    #[error("Unknown Function Block pin '{pin}' on '{instance}'")]
    UnknownFbPin { instance: String, pin: String },

    #[error("Symbol '{0}' is read-only outside of the SFC engine")]
    ReadOnlySymbol(String),

    #[error("Invalid literal value '{0}'")]
    InvalidLiteral(String),

    #[error("Duplicate symbol registration '{0}'")]
    DuplicateSymbol(String),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum RuntimeError {
    #[error("Core error: {0}")]
    Core(#[from] CoreError),

    #[error("Division by zero in POU '{pou}'")]
    DivisionByZero { pou: String },

    #[error("Watchdog timeout: scan took {elapsed_us} us (limit {limit_us} us)")]
    WatchdogTimeout { elapsed_us: u64, limit_us: u64 },

    #[error("Loop iteration limit exceeded in POU '{pou}' (max {max_iterations})")]
    IterationLimitExceeded { pou: String, max_iterations: usize },

    #[error("Invalid execution state in POU '{pou}': {message}")]
    ExecutionError { pou: String, message: String },
}
