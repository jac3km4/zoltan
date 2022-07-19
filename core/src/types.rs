use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::rc::Rc;

use auto_enums::auto_enum;
use derive_more::{AsRef, Display, From};
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
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

    fn name_right(&self) -> Option<Cow<'static, str>> {
        match self {
            Type::Pointer(inner) => inner.name_right(),
            Type::Reference(inner) => inner.name_right(),
            Type::Array(inner) => Some(format!("{}[]", inner.name_right().unwrap_or_default()).into()),
            Type::FixedArray(inner, size) => {
                Some(format!("{}[{size}]", inner.name_right().unwrap_or_default()).into())
            }
            Type::Function(ty) => {
                let params = ty.params.iter().map(Type::name).format(", ");
                Some(format!("({params})").into())
            }
            _ => None,
        }
    }

    fn name_left(&self) -> Cow<'static, str> {
        match self {
            Type::Void => "void".into(),
            Type::Bool => "bool".into(),
            Type::Char(true) => "char".into(),
            Type::Char(false) => "signed char".into(),
            Type::WChar => "wchar_t".into(),
            Type::Short(true) => "int16_t".into(),
            Type::Short(false) => "uint16_t".into(),
            Type::Int(true) => "int32_t".into(),
            Type::Int(false) => "uint32_t".into(),
            Type::Long(true) => "int64_t".into(),
            Type::Long(false) => "uint64_t".into(),
            Type::Float => "float".into(),
            Type::Double => "double".into(),
            Type::Union(id) => id.as_ref().as_str().into(),
            Type::Struct(id) => id.as_ref().as_str().into(),
            Type::Enum(id) => id.as_ref().as_str().into(),
            Type::Pointer(inner) if matches!(inner.as_ref(), Type::Function(_)) => {
                format!("{}(*)", inner.name_left()).into()
            }
            Type::Pointer(inner) => format!("{}*", inner.name_left()).into(),
            Type::Reference(inner) if matches!(inner.as_ref(), Type::Function(_)) => {
                format!("{}(&)", inner.name_left()).into()
            }
            Type::Reference(inner) => format!("{}&", inner.name_left()).into(),
            Type::Array(inner) => inner.name_left(),
            Type::FixedArray(inner, _) => inner.name_left(),
            Type::Function(fun) => fun.return_type.name(),
        }
    }

    pub fn name(&self) -> Cow<'static, str> {
        match self.name_right() {
            Some(right) => format!("{}{right}", self.name_left()).into(),
            None => self.name_left(),
        }
    }

    pub fn name_with_id(&self, id: &str) -> Cow<'static, str> {
        match self.name_right() {
            Some(right) => format!("{} {id}{right}", self.name_left()).into(),
            None => format!("{} {id}", self.name_left()).into(),
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

    pub fn has_direct_virtual_methods(&self) -> bool {
        !self.virtual_methods.is_empty()
    }

    pub fn has_indirect_virtual_methods(&self, types: &TypeInfo) -> bool {
        self.base
            .and_then(|id| types.structs.get(&id))
            .iter()
            .any(|typ| typ.has_virtual_methods(types))
    }

    pub fn has_virtual_methods(&self, types: &TypeInfo) -> bool {
        self.has_direct_virtual_methods() || self.has_indirect_virtual_methods(types)
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

#[derive(Debug, Default)]
pub struct NameAllocator {
    name_count: usize,
}

impl NameAllocator {
    pub fn allocate(&mut self) -> String {
        let i = self.name_count;
        self.name_count += 1;
        format!("__anonymous{}", i)
    }
}
