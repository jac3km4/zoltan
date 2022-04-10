use std::collections::VecDeque;
use std::io;

use thiserror::Error;

pub type Result<A, E = Error> = std::result::Result<A, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("compile errors: \n{0}")]
    CompileErrors(String),
    #[error("I/O error: {0}")]
    IoFailure(#[from] io::Error),
    #[error("invalid type")]
    InvalidType,
    #[error("vararg not supported")]
    VarArgNotSupported,
    #[error("{0}")]
    CoreFailure(#[from] zoltan::error::Error),
}

impl Error {
    pub fn from_compile_errors(errs: VecDeque<saltwater::CompileError>, files: &saltwater::Files) -> Self {
        let message = errs
            .iter()
            .map(|err| {
                let loc = files
                    .location(err.location.file, err.location.span.start)
                    .unwrap();
                format!("at {}:{}: {}", loc.line.0 + 1, loc.column.0 + 1, err.data)
            })
            .collect::<Vec<_>>()
            .join("\n");

        Self::CompileErrors(message)
    }
}
