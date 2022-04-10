use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::rc::Rc;

use auto_enums::auto_enum;
use derive_more::{AsRef, Display, From};
use enum_as_inner::EnumAsInner;
use ustr::{IdentityHasher, Ustr};

pub const POINTER_SIZE: usize = 8;
pub const MAX_ALIGN: usize = 8;

#[derive(Debug, Clone, PartialEq, EnumAsInner)]
pub enum Type {
    Void,
    Bool,
    Char(bool),
    WChar,
    Short(bool),
    Int(bool),
    Long(bool),
    Float,
    Double,
    Pointer(Rc<Type>),
    Reference(Rc<Type>),
    Array(Rc<Type>),
    FixedArray(Rc<Type>, usize),
    Function(Rc<FunctionType>),
    Union(UnionId),
    Struct(StructId),
    Enum(EnumId),
}

impl Type {
    pub fn size(&self, info: &TypeInfo) -> Option<usize> {
        match self {
            Type::Void => Some(0),
            Type::Bool => Some(1),
            Type::Char(_) => Some(1),
            #[cfg(windows)]
            Type::WChar => Some(2),
            #[cfg(unix)]
            Type::WChar => Some(4),
            Type::Short(_) => Some(2),
            Type::Int(_) => Some(4),
            Type::Long(_) => Some(8),
            Type::Float => Some(4),
            Type::Double => Some(8),
            Type::Pointer(_) => Some(POINTER_SIZE),
            Type::Reference(_) => Some(POINTER_SIZE),
            Type::Array(_) => None,
            Type::FixedArray(ty, size) => ty.size(info).map(|v| v * size),
            Type::Function(_) => Some(POINTER_SIZE),
            Type::Union(u) => info.unions.get(u).and_then(|u| u.size),
            Type::Struct(s) => info.structs.get(s).and_then(|s| s.size),
            Type::Enum(e) => info.enums.get(e).and_then(|e| e.size),
        }
    }

    pub fn name(&self) -> Cow<'static, str> {
        match self {
            Type::Void => "void".into(),
            Type::Bool => "bool".into(),
            Type::Char(true) => "char".into(),
            Type::Char(false) => "signed char".into(),
            Type::WChar => "wchar_t".into(),
            Type::Short(true) => "short".into(),
            Type::Short(false) => "unsigned short".into(),
            Type::Int(true) => "int".into(),
            Type::Int(false) => "unsigned int".into(),
            Type::Long(true) => "long".into(),
            Type::Long(false) => "unsigned long".into(),
            Type::Float => "float".into(),
            Type::Double => "double".into(),
            Type::Union(id) => id.as_ref().as_str().into(),
            Type::Struct(id) => id.as_ref().as_str().into(),
            Type::Enum(id) => id.as_ref().as_str().into(),
            Type::Pointer(inner) => format!("{}*", inner.name()).into(),
            Type::Reference(inner) => format!("{}&", inner.name()).into(),
            Type::Array(inner) => format!("{}[]", inner.name()).into(),
            Type::FixedArray(inner, size) => format!("{}[{}]", inner.name(), size).into(),
            Type::Function(fun) => {
                let ret = fun.return_type.name();
                let mut params = String::new();
                for param in &fun.params {
                    params.push_str(&param.name());
                    params.push_str(", ");
                }
                format!("{} ({})", ret, params).into()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRef, From, Display, Hash)]
pub struct StructId(Ustr);

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRef, From, Display, Hash)]
pub struct UnionId(Ustr);

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRef, From, Display, Hash)]
pub struct EnumId(Ustr);

pub type TypeMap<K, V> = HashMap<K, V, BuildHasherDefault<IdentityHasher>>;

#[derive(Debug, PartialEq)]
pub struct FunctionType {
    pub params: Vec<Type>,
    pub return_type: Type,
}

impl FunctionType {
    pub fn new(params: Vec<Type>, return_type: Type) -> Self {
        Self { params, return_type }
    }
}

#[derive(Debug)]
pub struct DataMember {
    pub name: Ustr,
    pub typ: Type,
    pub bit_offset: Option<usize>,
    pub is_bitfield: bool,
}

impl DataMember {
    pub fn basic(name: Ustr, typ: Type) -> Self {
        Self {
            name,
            typ,
            bit_offset: None,
            is_bitfield: false,
        }
    }
}

#[derive(Debug)]
pub struct StructType {
    pub name: Ustr,
    pub base: Option<StructId>,
    pub members: Vec<DataMember>,
    pub virtual_methods: Vec<Method>,
    pub size: Option<usize>,
}

impl StructType {
    pub fn stub(name: Ustr) -> Self {
        Self {
            name,
            base: None,
            members: vec![],
            virtual_methods: vec![],
            size: None,
        }
    }

    pub fn has_virtual_methods(&self, types: &TypeInfo) -> bool {
        !self.virtual_methods.is_empty()
            || self
                .base
                .and_then(|id| types.structs.get(&id))
                .iter()
                .any(|typ| typ.has_virtual_methods(types))
    }

    #[auto_enum(Iterator)]
    pub fn all_members<'a>(&'a self, types: &'a TypeInfo) -> impl Iterator<Item = &'a DataMember> {
        match self.base.and_then(|id| types.structs.get(&id)) {
            Some(typ) => {
                Box::new(typ.all_members(types).chain(self.members.iter())) as Box<dyn Iterator<Item = _>>
            }
            None => self.members.iter(),
        }
    }

    #[auto_enum(Iterator)]
    pub fn all_virtual_methods<'a>(&'a self, types: &'a TypeInfo) -> impl Iterator<Item = &'a Method> {
        match self.base.and_then(|id| types.structs.get(&id)) {
            Some(typ) => Box::new(typ.all_virtual_methods(types).chain(self.virtual_methods.iter()))
                as Box<dyn Iterator<Item = _>>,
            None => self.virtual_methods.iter(),
        }
    }
}

#[derive(Debug)]
pub struct Method {
    pub name: Ustr,
    pub typ: Rc<FunctionType>,
}

#[derive(Debug)]
pub struct UnionType {
    pub name: Ustr,
    pub members: Vec<DataMember>,
    pub size: Option<usize>,
}

#[derive(Debug)]
pub struct EnumType {
    pub name: Ustr,
    pub members: Vec<EnumMember>,
    pub size: Option<usize>,
}

#[derive(Debug)]
pub struct EnumMember {
    pub name: Ustr,
    pub value: i64,
}

impl EnumMember {
    pub fn new(name: Ustr, value: i64) -> Self {
        Self { name, value }
    }
}

#[derive(Debug)]
pub struct TypeInfo {
    pub structs: TypeMap<StructId, StructType>,
    pub unions: TypeMap<UnionId, UnionType>,
    pub enums: TypeMap<EnumId, EnumType>,
}
