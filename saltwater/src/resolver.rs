use saltwater::types::ArrayType;
use saltwater::{get_str, InternedStr};
use zoltan::types::*;
use zoltan::ustr::Ustr;

use crate::error::{Error, Result};

#[derive(Default)]
pub struct TypeResolver {
    structs: TypeMap<StructId, StructType>,
    unions: TypeMap<UnionId, UnionType>,
    enums: TypeMap<EnumId, EnumType>,
    name_allocator: NameAllocator,
}

impl TypeResolver {
    pub fn into_types(self) -> TypeInfo {
        TypeInfo {
            structs: self.structs,
            unions: self.unions,
            enums: self.enums,
        }
    }

    pub fn resolve_type(&mut self, typ: &saltwater::Type) -> Result<Type> {
        match typ {
            saltwater::Type::Void => Ok(Type::Void),
            saltwater::Type::Bool => Ok(Type::Bool),
            saltwater::Type::Char(signed) => Ok(Type::Char(*signed)),
            saltwater::Type::Short(signed) => Ok(Type::Short(*signed)),
            saltwater::Type::Int(signed) => Ok(Type::Int(*signed)),
            saltwater::Type::Long(signed) => Ok(Type::Long(*signed)),
            saltwater::Type::Float => Ok(Type::Float),
            saltwater::Type::Double => Ok(Type::Double),
            saltwater::Type::Pointer(inner, _) => Ok(Type::Pointer(self.resolve_type(inner)?.into())),
            saltwater::Type::Array(inner, ArrayType::Unbounded) => {
                Ok(Type::Array(self.resolve_type(inner)?.into()))
            }
            saltwater::Type::Array(inner, ArrayType::Fixed(size)) => {
                Ok(Type::FixedArray(self.resolve_type(inner)?.into(), *size as usize))
            }
            saltwater::Type::Function(fn_type) => {
                let args = fn_type
                    .params
                    .iter()
                    .map(|arg| self.resolve_type(&arg.get().ctype))
                    .collect::<Result<Vec<_>>>()?;
                let ret_type = self.resolve_type(&fn_type.return_type)?;
                Ok(Type::Function(FunctionType::new(args, ret_type).into()))
            }
            saltwater::Type::Union(saltwater::StructType::Anonymous(vars)) => {
                let id = self.resolve_union(None, vars, typ.sizeof().ok())?;
                Ok(Type::Union(id))
            }
            saltwater::Type::Union(saltwater::StructType::Named(name, vars)) => {
                let id = self.resolve_union(Some(get_str!(name)), &vars.get(), typ.sizeof().ok())?;
                Ok(Type::Union(id))
            }
            saltwater::Type::Struct(saltwater::StructType::Anonymous(vars)) => {
                let id = self.resolve_struct(None, vars, typ.sizeof().ok())?;
                Ok(Type::Struct(id))
            }
            saltwater::Type::Struct(saltwater::StructType::Named(name, vars)) => {
                let id = self.resolve_struct(Some(get_str!(name)), &vars.get(), typ.sizeof().ok())?;
                Ok(Type::Struct(id))
            }
            saltwater::Type::Enum(Some(name), vars) => {
                let id = self.resolve_enum(Some(get_str!(name)), vars, typ.sizeof().ok())?;
                Ok(Type::Enum(id))
            }
            saltwater::Type::Enum(None, vars) => {
                let id = self.resolve_enum(None, vars, typ.sizeof().ok())?;
                Ok(Type::Enum(id))
            }
            saltwater::Type::VaList => Err(Error::VarArgNotSupported),
            saltwater::Type::Error => Err(Error::InvalidType),
        }
    }

    fn resolve_union(
        &mut self,
        name: Option<&str>,
        vars: &[saltwater::hir::Variable],
        size: Option<u64>,
    ) -> Result<UnionId> {
        let name: Ustr = name
            .map(Into::into)
            .unwrap_or_else(|| self.name_allocator.allocate().into());

        if !self.unions.contains_key(&name.into()) {
            let mut members = vec![];
            for var in vars {
                let typ = self.resolve_type(&var.ctype)?;
                members.push(DataMember::basic(name, typ));
            }
            let union = UnionType {
                name,
                members,
                size: size.map(|s| s as usize),
            };
            self.unions.insert(name.into(), union);
        }
        Ok(name.into())
    }

    fn resolve_struct(
        &mut self,
        name: Option<&str>,
        vars: &[saltwater::hir::Variable],
        size: Option<u64>,
    ) -> Result<StructId> {
        let name: Ustr = name
            .map(Into::into)
            .unwrap_or_else(|| self.name_allocator.allocate().into());
        if !self.structs.contains_key(&name.into()) {
            self.structs.insert(name.into(), StructType::stub(name));

            let mut members = vec![];
            for var in vars {
                let typ = self.resolve_type(&var.ctype)?;
                members.push(DataMember::basic(name, typ));
            }
            let struct_ = StructType {
                name,
                base: None,
                members,
                virtual_methods: vec![],
                size: size.map(|s| s as usize),
            };
            self.structs.insert(name.into(), struct_);
        }
        Ok(name.into())
    }

    fn resolve_enum(
        &mut self,
        name: Option<&str>,
        vars: &[(InternedStr, i64)],
        size: Option<u64>,
    ) -> Result<EnumId> {
        let name: Ustr = name
            .map(Into::into)
            .unwrap_or_else(|| self.name_allocator.allocate().into());
        if !self.enums.contains_key(&name.into()) {
            let mut members = vec![];
            for (str, val) in vars {
                members.push(EnumMember::new(get_str!(str).into(), *val));
            }
            let enum_ = EnumType {
                name,
                members,
                size: size.map(|s| s as usize),
            };
            self.enums.insert(name.into(), enum_);
        }
        Ok(name.into())
    }
}
