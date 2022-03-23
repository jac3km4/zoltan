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
    #[error("compile errors: {0:?}")]
    CompileError(VecDeque<saltwater::Locatable<saltwater::data::Error>>),
    #[error("object file error: {0}")]
    ObjectError(#[from] object::Error),
    #[error("DWARF error: {0}")]
    DwarfError(#[from] gimli::write::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("{0}")]
    OtherError(#[from] Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub enum SymbolError {
    MoreThanOneMatch(String),
    NoMatches(String),
}
