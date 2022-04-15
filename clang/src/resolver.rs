use std::hash::BuildHasherDefault;

use quickscope::ScopeMap;
use zoltan::types::*;
use zoltan::ustr::{IdentityHasher, Ustr};

use crate::error::{Error, Result};

pub struct TypeResolver {
    structs: TypeMap<StructId, StructType>,
    unions: TypeMap<UnionId, UnionType>,
    enums: TypeMap<EnumId, EnumType>,
    local_types: ScopeMap<Ustr, Type, BuildHasherDefault<IdentityHasher>>,
    name_allocator: NameAllocator,
    strip_namespaces: bool,
}

impl TypeResolver {
    pub fn new(strip_namespaces: bool) -> Self {
        Self {
            structs: TypeMap::default(),
            unions: TypeMap::default(),
            enums: TypeMap::default(),
            local_types: ScopeMap::default(),
            name_allocator: NameAllocator::default(),
            strip_namespaces,
        }
    }

    pub fn into_types(self) -> TypeInfo {
        TypeInfo {
            structs: self.structs,
            unions: self.unions,
            enums: self.enums,
        }
    }

    pub fn resolve_decl(&mut self, entity: clang::Entity) -> Result<Type> {
        let name: Ustr = self.generate_type_name(entity);

        match entity.get_kind() {
            clang::EntityKind::StructDecl
            | clang::EntityKind::ClassDecl
            | clang::EntityKind::ClassTemplate => {
                if !self.structs.contains_key(&name.into()) {
                    self.structs.insert(name.into(), StructType::stub(name));

                    let size = entity.get_type().and_then(|t| t.get_sizeof().ok());
                    let res = if let Some(template) = entity.get_template() {
                        self.resolve_struct(name, template, size)?
                    } else {
                        self.resolve_struct(name, entity, size)?
                    };
                    self.structs.insert(name.into(), res);
                }
                Ok(Type::Struct(name.into()))
            }
            clang::EntityKind::EnumDecl => {
                if !self.enums.contains_key(&name.into()) {
                    let res = self.resolve_enum(name, entity)?;
                    self.enums.insert(name.into(), res);
                }
                Ok(Type::Enum(name.into()))
            }
            clang::EntityKind::UnionDecl => {
                if !self.unions.contains_key(&name.into()) {
                    let res = self.resolve_union(name, entity)?;
                    self.unions.insert(name.into(), res);
                }

                Ok(Type::Union(name.into()))
            }
            other => Err(Error::UnexpectedKind(other)),
        }
    }

    pub fn resolve_type(&mut self, typ: clang::Type) -> Result<Type> {
        // populate template arguments if available
        if let Some(args) = typ.get_template_argument_types() {
            self.local_types.push_layer();

            let decl = typ.get_declaration().unwrap();
            let template = if decl.get_kind() == clang::EntityKind::ClassTemplate {
                decl
            } else {
                decl.get_template().unwrap()
            };

            for (ent, typ) in template
                .get_children()
                .iter()
                .take_while(|ent| ent.get_kind() == clang::EntityKind::TemplateTypeParameter)
                .zip(&args)
            {
                if let Some(typ) = typ {
                    let typ = self.resolve_type(*typ)?;
                    self.local_types
                        .define(ent.get_name_raw().unwrap().as_str().into(), typ);
                }
            }
        }

        let res = match typ.get_kind() {
            clang::TypeKind::Void => Type::Void,
            clang::TypeKind::Bool => Type::Bool,
            clang::TypeKind::CharS | clang::TypeKind::SChar => Type::Char(true),
            clang::TypeKind::CharU | clang::TypeKind::UChar => Type::Char(false),
            clang::TypeKind::WChar => Type::WChar,
            clang::TypeKind::Short => Type::Short(true),
            clang::TypeKind::UShort => Type::Short(false),
            clang::TypeKind::Int => Type::Int(true),
            clang::TypeKind::UInt => Type::Int(false),
            clang::TypeKind::Long | clang::TypeKind::LongLong => Type::Long(true),
            clang::TypeKind::ULong | clang::TypeKind::ULongLong => Type::Long(false),
            clang::TypeKind::Float => Type::Float,
            clang::TypeKind::Double => Type::Double,
            clang::TypeKind::Pointer => {
                let inner = self.resolve_type(typ.get_pointee_type().unwrap())?;
                Type::Pointer(inner.into())
            }
            clang::TypeKind::LValueReference | clang::TypeKind::RValueReference => {
                let inner = self.resolve_type(typ.get_pointee_type().unwrap())?;
                Type::Reference(inner.into())
            }
            clang::TypeKind::Enum => self.resolve_decl(typ.get_declaration().unwrap())?,
            clang::TypeKind::Record => self.resolve_decl(typ.get_declaration().unwrap())?,
            clang::TypeKind::Typedef => self.resolve_type(typ.get_canonical_type())?,
            clang::TypeKind::FunctionPrototype => {
                let fun = self.resolve_function(typ)?;
                Type::Function(fun.into())
            }
            clang::TypeKind::FunctionNoPrototype => todo!(),
            clang::TypeKind::ConstantArray => {
                let inner = self.resolve_type(typ.get_element_type().unwrap())?;
                Type::FixedArray(inner.into(), typ.get_size().unwrap())
            }
            clang::TypeKind::DependentSizedArray => {
                let inner = self.resolve_type(typ.get_element_type().unwrap())?;
                Type::FixedArray(inner.into(), typ.get_size().unwrap())
            }
            clang::TypeKind::Elaborated => self.resolve_type(typ.get_elaborated_type().unwrap())?,
            clang::TypeKind::Unexposed => {
                if typ.get_template_argument_types().is_some() {
                    // type with template arguments
                    self.resolve_decl(typ.get_declaration().unwrap())?
                } else {
                    // template argument
                    let name = typ.get_display_name().into();
                    self.local_types
                        .get(&name)
                        .ok_or(Error::UnresolvedType(name))?
                        .clone()
                }
            }
            other => return Err(Error::UnexpectedType(other)),
        };

        if typ.get_template_argument_types().is_some() {
            self.local_types.pop_layer();
        }
        Ok(res)
    }

    fn resolve_struct(
        &mut self,
        name: Ustr,
        entity: clang::Entity,
        size: Option<usize>,
    ) -> Result<StructType> {
        let children = entity.get_children();
        let base = children
            .iter()
            .find(|ent| ent.get_kind() == clang::EntityKind::BaseSpecifier)
            .and_then(|ent| ent.get_definition())
            .map(|ent| self.resolve_decl(ent))
            .transpose()?
            .and_then(|ty| ty.into_struct().ok());

        let mut members = vec![];
        let mut virtual_methods = vec![];

        for child in children {
            match child.get_kind() {
                clang::EntityKind::FieldDecl => {
                    let name = self.get_entity_name(child);
                    let typ = self.resolve_type(child.get_type().unwrap())?;
                    let bit_offset = child.get_offset_of_field().ok();
                    members.push(DataMember {
                        name,
                        typ,
                        bit_offset,
                        is_bitfield: child.is_bit_field(),
                    })
                }
                clang::EntityKind::Method | clang::EntityKind::Destructor if child.is_virtual_method() => {
                    let name = self.get_entity_name(child);
                    if let Type::Function(typ) = self.resolve_type(child.get_type().unwrap())? {
                        virtual_methods.push(Method {
                            name,
                            typ: typ.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(StructType {
            name,
            base,
            members,
            virtual_methods,
            size,
        })
    }

    fn resolve_enum(&mut self, name: Ustr, entity: clang::Entity) -> Result<EnumType> {
        let children = entity.get_children();
        let mut members = vec![];

        for child in children {
            if child.get_kind() == clang::EntityKind::EnumConstantDecl {
                let name = self.get_entity_name(child);
                let (value, _) = child.get_enum_constant_value().unwrap();
                members.push(EnumMember { name, value });
            }
        }

        let size = entity.get_type().unwrap().get_sizeof().ok();
        Ok(EnumType { name, members, size })
    }

    fn resolve_union(&mut self, name: Ustr, entity: clang::Entity) -> Result<UnionType> {
        let children = entity.get_children();
        let mut members = vec![];

        for child in children {
            if child.get_kind() == clang::EntityKind::FieldDecl {
                let name = self.get_entity_name(child);
                let typ = self.resolve_type(child.get_type().unwrap())?;
                let bit_offset = child.get_offset_of_field().ok();
                members.push(DataMember {
                    name,
                    typ,
                    bit_offset,
                    is_bitfield: false,
                })
            }
        }

        let size = entity.get_type().unwrap().get_sizeof().ok();
        Ok(UnionType { name, members, size })
    }

    fn resolve_function(&mut self, typ: clang::Type) -> Result<FunctionType> {
        let return_type = self.resolve_type(typ.get_result_type().unwrap())?;
        let mut params = vec![];

        for typ in typ.get_argument_types().unwrap() {
            params.push(self.resolve_type(typ)?);
        }
        Ok(FunctionType { return_type, params })
    }

    fn generate_type_name(&mut self, entity: clang::Entity) -> Ustr {
        let mut cur = entity;
        let mut full_name = entity
            .get_display_name()
            .unwrap_or_else(|| self.name_allocator.allocate());

        while let Some(parent) = cur.get_semantic_parent() {
            match parent.get_kind() {
                clang::EntityKind::TranslationUnit => {}
                clang::EntityKind::Namespace if self.strip_namespaces => {}
                _ => {
                    let parent_name = parent.get_name();
                    let prefix = parent_name.as_deref().unwrap_or("__unnamed");
                    full_name = format!("{}::{}", prefix, full_name);
                }
            }
            cur = parent;
        }

        full_name.into()
    }

    fn get_entity_name(&mut self, entity: clang::Entity) -> Ustr {
        entity
            .get_name_raw()
            .map(|str| str.as_str().into())
            .unwrap_or_else(|| self.name_allocator.allocate().into())
    }
}
