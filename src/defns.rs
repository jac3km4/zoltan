use std::num::ParseIntError;

use enum_as_inner::EnumAsInner;
use saltwater::codespan::LineIndex;
use saltwater::data::types::Type;
use saltwater::hir::Variable;
use saltwater::types::FunctionType;
use saltwater::{check_semantics, Opt, StorageClass};

use crate::error::Error;
use crate::patterns::Pattern;
use crate::symbols::FunctionSymbol;

#[derive(Debug)]
pub struct Definitions {
    functions: Vec<Function>,
}

impl Definitions {
    pub fn from_source<S: AsRef<str>>(source: S) -> Result<Self, Error> {
        let prog = check_semantics(source.as_ref(), Opt::default());
        let mut functions = vec![];

        for decl in prog.result.map_err(Error::CompileError)? {
            let var = decl.data.symbol.get();
            if let Variable {
                ctype: Type::Function(fun_typ),
                storage_class: StorageClass::Typedef,
                ..
            } = &*var
            {
                let file = decl.location.file;
                let line = prog.files.line_index(file, decl.location.span.start);
                let mut params = Vec::new();
                for li in (0..line.0).rev() {
                    let span = prog.files.line_span(file, LineIndex(li)).unwrap();
                    let slice = prog.files.source_slice(file, span).unwrap();
                    if let Some(kv) = DefnParam::from_comment(slice) {
                        params.push(kv?);
                    }
                }

                if !params.is_empty() {
                    let func = Function::new(var.id.resolve_and_clone(), fun_typ.clone(), params)?;
                    functions.push(func);
                }
            }
        }

        Ok(Definitions { functions })
    }

    pub fn into_functions(self) -> Vec<Function> {
        self.functions
    }
}

#[derive(Debug)]
pub struct Function {
    name: String,
    typ: FunctionType,
    pattern: Pattern,
    offset: Option<i64>,
}

impl Function {
    fn new(name: String, typ: FunctionType, params: Vec<DefnParam>) -> Result<Self, Error> {
        let offset = params.iter().find_map(DefnParam::as_offset).cloned();
        let pattern = params
            .into_iter()
            .find_map(|param| param.into_pattern().ok())
            .ok_or(Error::MissingPattern)?;
        Ok(Self {
            name,
            typ,
            pattern,
            offset,
        })
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn into_symbol(self, addr: u64) -> FunctionSymbol {
        let addr = (addr as i64 + self.offset.unwrap_or(0)) as u64;
        FunctionSymbol::new(self.name, self.typ, addr)
    }

    pub fn into_name(self) -> String {
        self.name
    }
}

#[derive(Debug, EnumAsInner)]
enum DefnParam {
    Pattern(Pattern),
    Offset(i64),
}

impl DefnParam {
    fn from_key_val(key: &str, val: &str) -> Result<Self, Error> {
        match key {
            "pattern" => Ok(DefnParam::Pattern(Pattern::parse(val)?)),
            "offset" => {
                let offset = val
                    .parse()
                    .map_err(|err: ParseIntError| Error::InvalidCommentParam("offset", err.to_string()))?;
                Ok(DefnParam::Offset(offset))
            }
            _ => Err(Error::UnknownCommentParam(key.to_owned())),
        }
    }

    fn from_comment(line: &str) -> Option<Result<Self, Error>> {
        let (key, val) = line
            .trim_start()
            .strip_prefix("///")?
            .trim_start()
            .strip_prefix('@')?
            .split_once(' ')?;

        Some(Self::from_key_val(key, val.trim()))
    }
}
