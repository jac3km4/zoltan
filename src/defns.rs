use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;

use saltwater::codespan::LineIndex;
use saltwater::data::types::Type;
use saltwater::hir::Variable;
use saltwater::types::FunctionType;
use saltwater::{check_semantics, Opt, StorageClass};

use crate::error::{Error, ParamError};
use crate::eval::Expr;
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

        for decl in prog
            .result
            .map_err(|errs| Error::from_compile_errors(errs, &prog.files))?
        {
            let var = decl.data.symbol.get();
            if let Variable {
                ctype: Type::Function(fun_typ),
                storage_class: StorageClass::Typedef,
                ..
            } = &*var
            {
                let file = decl.location.file;
                let line = prog.files.line_index(file, decl.location.span.start);
                let mut params = HashMap::new();
                for li in (0..line.0).rev() {
                    let span = prog.files.line_span(file, LineIndex(li)).unwrap();
                    let slice = prog.files.source_slice(file, span).unwrap();
                    if let Some((key, val)) = parse_typedef_comment(slice) {
                        params.insert(key, val);
                    } else {
                        break;
                    }
                }

                if !params.is_empty() {
                    let func = Function::new(var.id.resolve_and_clone(), fun_typ.clone(), params)
                        .map_err(|err| Error::TypedefParamError(var.id.resolve_and_clone(), err))?;
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
    typ: FunctionType,
    pattern: Pattern,
    pub(crate) name: String,
    pub(crate) offset: Option<i64>,
    pub(crate) eval: Option<Expr>,
    pub(crate) nth_entry_of: Option<(usize, usize)>,
}

impl Function {
    fn new(name: String, typ: FunctionType, mut params: HashMap<&str, &str>) -> Result<Self, ParamError> {
        let pattern = Pattern::parse(params.remove("pattern").ok_or(ParamError::MissingPattern)?)
            .map_err(|err| ParamError::ParseError("pattern", err))?;
        let offset = params
            .remove("offset")
            .map(|str| parse_from_str(str, "offset"))
            .transpose()?;
        let eval = params
            .remove("eval")
            .map(Expr::parse)
            .transpose()
            .map_err(|err| ParamError::ParseError("eval", err))?;
        let nth_entry_of = params.remove("nth").map(parse_index_specifier).transpose()?;
        if let Some(str) = params.keys().next() {
            return Err(ParamError::UnknownParam(str.deref().to_owned()));
        }
        Ok(Self {
            name,
            typ,
            pattern,
            offset,
            eval,
            nth_entry_of,
        })
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn into_symbol(self, addr: u64) -> FunctionSymbol {
        FunctionSymbol::new(self.name, self.typ, addr)
    }
}

fn parse_typedef_comment(line: &str) -> Option<(&str, &str)> {
    let (key, val) = line
        .trim_start()
        .strip_prefix("///")?
        .trim_start()
        .strip_prefix('@')?
        .split_once(' ')?;

    Some((key, val.trim()))
}

fn parse_index_specifier(str: &str) -> Result<(usize, usize), ParamError> {
    let (n, max) = str
        .split_once('/')
        .ok_or_else(|| ParamError::InvalidParam("nth", "invalid format".to_string()))?;
    Ok((
        parse_from_str(n.trim(), "nth")?,
        parse_from_str(max.trim(), "nth")?,
    ))
}

fn parse_from_str<F: FromStr>(str: &str, field: &'static str) -> Result<F, ParamError>
where
    F::Err: std::error::Error,
{
    str.parse()
        .map_err(|err: F::Err| ParamError::InvalidParam(field, err.to_string()))
}
