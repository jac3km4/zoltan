use std::collections::VecDeque;
use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid comment parameter {0}: {1}")]
    InvalidCommentParam(&'static str, String),
    #[error("unknown comment parameter: {0}")]
    UnknownCommentParam(String),
    #[error("missing pattern parameter")]
    MissingPattern,
    #[error("compile errors:\n{0}")]
    CompileError(String),
    #[error("object file error: {0}")]
    ObjectError(#[from] object::Error),
    #[error("DWARF error: {0}")]
    DwarfError(#[from] gimli::write::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("{0}")]
    OtherError(#[from] Box<dyn std::error::Error>),
}

impl Error {
    pub fn from_compile_error(errs: VecDeque<saltwater::CompileError>, files: &saltwater::Files) -> Self {
        let message = errs
            .iter()
            .map(|err| {
                let loc = files
                    .location(err.location.file, err.location.span.start)
                    .unwrap();
                format!("at {}:{}: {}", loc.line, loc.column, err.data)
            })
            .collect::<Vec<_>>()
            .join("\n");

        Self::CompileError(message)
    }
}

#[derive(Debug)]
pub enum SymbolError {
    MoreThanOneMatch(String),
    NoMatches(String),
}
