use std::collections::VecDeque;
use std::io;

use peg::str::LineCol;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid typedef parameter {0}: {1}")]
    InvalidCommentParam(&'static str, String),
    #[error("unknown typedef parameter: {0}")]
    UnknownCommentParam(String),
    #[error("missing pattern parameter")]
    MissingPattern,
    #[error("failed to parse a typedef parameter: {0}")]
    PegError(#[from] peg::error::ParseError<LineCol>),
    #[error("invalid rdata access at {0}")]
    InvalidAccess(usize),
    #[error("unresolved name {0}")]
    UnresolvedName(String),
    #[error("compile errors:\n{0}")]
    CompileError(String),
    #[error("object file error: {0}")]
    ObjectError(#[from] object::Error),
    #[error("DWARF error: {0}")]
    DwarfError(#[from] gimli::write::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("missing {0} section")]
    MissingSection(&'static str),
    #[error("{0}")]
    OtherError(#[from] Box<dyn std::error::Error>),
}

impl Error {
    pub fn from_compile_errors(errs: VecDeque<saltwater::CompileError>, files: &saltwater::Files) -> Self {
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

#[derive(Debug, Error)]
pub enum SymbolError {
    #[error("too many matches for {0} ({1})")]
    MoreThanOneMatch(String, usize),
    #[error("no matches for {0}")]
    NoMatches(String),
    #[error("not enough matches for {0} ({1})")]
    NotEnoughMatches(String, usize),
    #[error("count mismatch for {0} ({1})")]
    CountMismatch(String, usize),
}
