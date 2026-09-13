//! The error a command ends with.

use thiserror::Error;

use crate::api::ApiError;
use crate::auth::AuthError;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Other(String),
}

impl CliError {
    /// Process exit code: 2 for usage mistakes, 1 for everything else.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) => 2,
            _ => 1,
        }
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
