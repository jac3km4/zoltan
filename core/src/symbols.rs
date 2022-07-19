use std::collections::HashMap;
use std::rc::Rc;

use ustr::Ustr;

use crate::error::{Result, SymbolError};
use crate::eval::EvalContext;
use crate::exe::ExecutableData;
use crate::patterns;
use crate::spec::FunctionSpec;
use crate::types::FunctionType;

pub fn resolve_in_exe(
    specs: Vec<FunctionSpec>,
    exe: &ExecutableData,
) -> Result<(Vec<FunctionSymbol>, Vec<SymbolError>)> {
    let mut match_map: HashMap<usize, Vec<u64>> = HashMap::new();
    for mat in patterns::multi_search(specs.iter().map(|spec| &spec.pattern), exe.text()) {
        match_map.entry(mat.pattern).or_default().push(mat.rva);
    }

    let mut syms = vec![];
    let mut errs = vec![];
    for (i, fun) in specs.into_iter().enumerate() {
        match match_map.get(&i).map(|vec| &vec[..]) {
            Some([addr]) => syms.push(resolve_symbol(fun, exe, *addr)?),
            Some(addrs) => {
                if let Some((n, max)) = fun.nth_entry_of {
                    match addrs.get(n) {
                        Some(rva) if max == addrs.len() => syms.push(resolve_symbol(fun, exe, *rva)?),
                        Some(_) => errs.push(SymbolError::CountMismatch(fun.name, addrs.len())),
                        None => errs.push(SymbolError::NotEnoughMatches(fun.name, addrs.len())),
                    }
                } else {
                    errs.push(SymbolError::MoreThanOneMatch(fun.name, addrs.len()));
                }
            }
            None => errs.push(SymbolError::NoMatches(fun.name)),
        }
    }
    Ok((syms, errs))
}

fn resolve_symbol(spec: FunctionSpec, data: &ExecutableData, rva: u64) -> Result<FunctionSymbol> {
    let res = match &spec.eval {
        Some(expr) => expr.eval(&EvalContext::new(&spec.pattern, data, rva)?)? - data.image_base(),
        None => (rva as i64 - spec.offset.unwrap_or(0) as i64) as u64 + data.text_offset_from_base(),
    };
    Ok(FunctionSymbol::new(spec.name, spec.function_type, res))
}

#[derive(Debug)]
pub struct FunctionSymbol {
    name: Ustr,
    function_type: Rc<FunctionType>,
    rva: u64,
}

impl FunctionSymbol {
    fn new(name: Ustr, function_type: Rc<FunctionType>, rva: u64) -> Self {
        Self {
            name,
            function_type,
            rva,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn function_type(&self) -> Rc<FunctionType> {
        self.function_type.clone()
    }

    pub fn rva(&self) -> u64 {
        self.rva
    }
}
