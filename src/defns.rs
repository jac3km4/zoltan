use std::collections::HashMap;
use std::num::ParseIntError;

use saltwater::codespan::LineIndex;
use saltwater::data::types::Type;
use saltwater::hir::Variable;
use saltwater::types::FunctionType;
use saltwater::{check_semantics, Opt, StorageClass};

use crate::error::Error;
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
    typ: FunctionType,
    pattern: Pattern,
    pub(crate) name: String,
    pub(crate) offset: Option<i64>,
    pub(crate) eval: Option<Expr>,
}

impl Function {
    fn new(name: String, typ: FunctionType, mut params: HashMap<&str, &str>) -> Result<Self, Error> {
        let pattern = Pattern::parse(params.remove("pattern").ok_or(Error::MissingPattern)?)?;
        let offset = params
            .remove("offset")
            .map(|str| {
                str.parse()
                    .map_err(|err: ParseIntError| Error::InvalidCommentParam("offset", err.to_string()))
            })
            .transpose()?;
        let eval = params.remove("eval").map(Expr::parse).transpose()?;
        Ok(Self {
            name,
            typ,
            pattern,
            offset,
            eval,
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
