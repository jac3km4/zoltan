use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::exe::ExecutableData;
use crate::patterns::{Pattern, VarType};
use crate::types::POINTER_SIZE;

#[derive(Debug)]
pub enum Expr {
    Deref(Box<Self>),
    Add(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Ident(String),
    Int(u64),
}

impl Expr {
    pub fn parse(str: &str) -> Result<Self, peg::error::ParseError<peg::str::LineCol>> {
        expr::expr(str)
    }

    pub fn eval(&self, ctx: &EvalContext) -> Result<u64> {
        match self {
            Expr::Deref(expr) => ctx.data.resolve_rel_rdata(expr.eval(ctx)?),
            Expr::Add(lhs, rhs) => Ok(lhs.eval(ctx)? + rhs.eval(ctx)?),
            Expr::Sub(lhs, rhs) => Ok(lhs.eval(ctx)? - rhs.eval(ctx)?),
            Expr::Ident(name) => ctx.get_var(name),
            Expr::Int(i) => Ok(*i * POINTER_SIZE as u64),
        }
    }
}

pub struct EvalContext<'a> {
    vars: HashMap<&'a str, u64>,
    data: &'a ExecutableData<'a>,
}

impl<'a> EvalContext<'a> {
    pub fn new(pattern: &'a Pattern, data: &'a ExecutableData, rva: u64) -> Result<Self> {
        let mut vars = HashMap::new();
        for (key, typ, offset) in pattern.groups() {
            let abs = match typ {
                VarType::Rel => data.resolve_rel_text(offset as u64 + rva)?,
            };
            vars.insert(key, abs);
        }
        let instance = Self { vars, data };
        Ok(instance)
    }

    fn get_var(&self, name: &str) -> Result<u64> {
        self.vars
            .get(name)
            .cloned()
            .ok_or_else(|| Error::UnresolvedName(name.to_owned()))
    }
}

peg::parser! {
    grammar expr() for str {
        rule _() =
            quiet!{[' ' | '\t']*}
        rule number() -> u64
            = n:$(['0'..='9']+) {? n.parse().or(Err("u64")) }

        pub rule expr() -> Expr = precedence!{
            x:(@) _ "+" _ y:@ { Expr::Add(x.into(), y.into()) }
            x:(@) _ "-" _ y:@ { Expr::Sub(x.into(), y.into()) }
           --
           "*" e:expr() { Expr::Deref(e.into()) }
           --
            n:number() { Expr::Int(n) }
            "(" e:expr() ")" { e }
            id:$(['a'..='z' | 'A'..='Z' | '_']+) { Expr::Ident(id.to_owned()) }
          }
    }
}
