use clang::diagnostic::{Diagnostic, Severity};
use thiserror::Error;
use zoltan::ustr::Ustr;

pub type Result<A, E = Error> = std::result::Result<A, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid kind: {0:?}")]
    UnexpectedKind(clang::EntityKind),
    #[error("unexpected type {0:?}")]
    UnexpectedType(clang::TypeKind),
    #[error("unresolved type {0}")]
    UnresolvedType(Ustr),
    #[error("parse error: {0}")]
    ParseFailure(#[from] clang::SourceError),
    #[error("compilation errors: \n{0}")]
    CompilerErrors(String),
    #[error("{0}")]
    CoreFailure(#[from] zoltan::error::Error),
}

impl Error {
    pub fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        let msg = diagnostics
            .iter()
            .filter(|err| err.get_severity() == Severity::Error)
            .map(|err| err.formatter().format())
            .collect::<Vec<_>>()
            .join("\n");
        Error::CompilerErrors(msg)
    }
}
