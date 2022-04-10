use std::borrow::Cow;
use std::collections::HashMap;
use std::io;

use gimli::write::{Address, AttributeValue, DwarfUnit, EndianVec, Sections, Unit, UnitEntryId};
use gimli::{DwAte, DwTag};
use object::{BinaryFormat, SectionKind};

use crate::error::{Error, Result};
use crate::exe::ExeProperties;
use crate::symbols::FunctionSymbol;
use crate::types::*;

pub fn write_symbol_file<W>(
    output: W,
    symbols: Vec<FunctionSymbol>,
    type_info: &TypeInfo,
    props: ExeProperties,
) -> Result<()>
where
    W: io::Write,
{
    const DWARF_VERSION: u16 = 5;

    let encoding = gimli::Encoding {
        format: if props.is64bit() {
            gimli::Format::Dwarf64
        } else {
            gimli::Format::Dwarf32
        },
        version: DWARF_VERSION,
        address_size: props.address_size(),
    };
    let mut dwarf = DwarfUnit::new(encoding);
    let mut writer = DwarfWriter::new(&mut dwarf.unit, type_info);
    for sym in symbols {
        writer.define_function_symbol(sym, props.image_base());
    }

    // TODO: handle endianess here
    let mut sections = Sections::new(EndianVec::new(gimli::LittleEndian));
    dwarf.write(&mut sections)?;

    let mut obj = props.replicate_object(BinaryFormat::Elf);
    sections.for_each_mut(|id, data| {
        let name = id.name().as_bytes().to_vec();
        let id = obj.add_section(b"LOAD".to_vec(), name, SectionKind::Debug);
        obj.set_section_data(id, Cow::Owned(data.take()), 8);
        Ok::<(), Error>(())
    })?;
    obj.write_stream(output)?;

    Ok(())
}

struct DwarfWriter<'a> {
    unit: &'a mut Unit,
    types: &'a TypeInfo,
    cache: HashMap<Cow<'static, str>, UnitEntryId>,
}

impl<'a> DwarfWriter<'a> {
    fn new(unit: &'a mut Unit, info: &'a TypeInfo) -> Self {
        Self {
            unit,
            types: info,
            cache: HashMap::new(),
        }
    }

    fn get_type(&mut self, typ: &Type) -> UnitEntryId {
        let name = typ.name();
        self.cache.get(&name).cloned().unwrap_or_else(|| {
            let id = self.define_type(typ);
            self.cache.insert(name, id);
            id
        })
    }

    fn define_type(&mut self, typ: &Type) -> UnitEntryId {
        match typ {
            Type::Void => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Bool => self.define_base_type(typ, gimli::DW_ATE_boolean),
            Type::Char(true) => self.define_base_type(typ, gimli::DW_ATE_signed_char),
            Type::Char(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned_char),
            Type::WChar => self.define_base_type(typ, gimli::DW_ATE_unsigned_char),
            Type::Short(true) => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Short(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned),
            Type::Int(true) => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Int(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned),
            Type::Long(true) => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Long(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned),
            Type::Float => self.define_base_type(typ, gimli::DW_ATE_float),
            Type::Double => self.define_base_type(typ, gimli::DW_ATE_float),
            Type::Reference(inner) => self.define_pointer(inner, gimli::DW_TAG_reference_type),
            Type::Pointer(inner) => self.define_pointer(inner, gimli::DW_TAG_pointer_type),
            Type::Array(inner) => self.define_array(inner, typ.size(self.types), None),
            Type::FixedArray(inner, size) => self.define_array(inner, typ.size(self.types), Some(*size)),
            Type::Struct(id) => {
                let struct_ty = self.types.structs.get(id).expect("Unresolved struct");
                self.define_struct(struct_ty)
            }
            Type::Enum(id) => {
                let enum_ty = self.types.enums.get(id).expect("Unresolved enum");
                self.define_enum(enum_ty)
            }
            Type::Union(id) => {
                let union_ty = self.types.unions.get(id).expect("Unresolved union");
                self.define_union(union_ty)
            }
            Type::Function(fun) => self.define_function_type(fun),
        }
    }

    fn define_base_type(&mut self, typ: &Type, encoding: DwAte) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_base_type);
        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(typ.name().as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        entry.set(gimli::DW_AT_encoding, AttributeValue::Encoding(encoding));
        if typ == &Type::Void {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data1(0));
        } else if let Some(size) = typ.size(self.types) {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size as u64));
        }

        id
    }

    fn define_pointer(&mut self, inner: &Type, tag: DwTag) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), tag);
        let inner = self.get_type(inner);
        let entry = self.unit.get_mut(id);
        entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(inner));
        entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(POINTER_SIZE as u64));
        id
    }

    fn define_array(
        &mut self,
        inner: &Type,
        byte_size: Option<usize>,
        array_size: Option<usize>,
    ) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_array_type);
        let inner = self.get_type(inner);
        let entry = self.unit.get_mut(id);
        entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(inner));
        if let Some(size) = byte_size {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size as u64));
        }

        if let Some(array_size) = array_size {
            let range = self.unit.add(id, gimli::DW_TAG_subrange_type);
            let range = self.unit.get_mut(range);
            range.set(gimli::DW_AT_count, AttributeValue::Data8(array_size as u64));
        }

        id
    }

    fn define_struct(&mut self, struct_: &StructType) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_structure_type);
        self.cache.insert(struct_.name.as_str().into(), id);

        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(struct_.name.as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);

        if let Some(size) = struct_.size {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size as u64));
        }

        let mut offset = 0u64;

        if struct_.has_virtual_methods(self.types) {
            let vtable_id = self.define_vtable(struct_);
            let this_pointer_id = self.unit.add(id, gimli::DW_TAG_pointer_type);
            let this_pointer = self.unit.get_mut(this_pointer_id);
            this_pointer.set(gimli::DW_AT_type, AttributeValue::UnitRef(vtable_id));

            let this_param_id = self.unit.add(id, gimli::DW_TAG_member);
            let this_param = self.unit.get_mut(this_param_id);
            let name = AttributeValue::String(get_vtable_field_name(struct_).as_bytes().to_vec());
            this_param.set(gimli::DW_AT_name, name);
            this_param.set(gimli::DW_AT_type, AttributeValue::UnitRef(this_pointer_id));
            this_param.set(gimli::DW_AT_artificial, AttributeValue::Data1(1));
            this_param.set(gimli::DW_AT_data_member_location, AttributeValue::Data8(offset));
            offset += POINTER_SIZE as u64;
        }

        for member in struct_.all_members(self.types) {
            let type_id = self.get_type(&member.typ);
            let member_id = self.unit.add(id, gimli::DW_TAG_member);
            let member_entry = self.unit.get_mut(member_id);
            let name = AttributeValue::String(member.name.as_bytes().to_vec());
            member_entry.set(gimli::DW_AT_name, name);

            if let Some(offset_bits) = member.bit_offset {
                offset = offset_bits as u64 / u8::BITS as u64;
                member_entry.set(gimli::DW_AT_data_member_location, AttributeValue::Data8(offset));
                member_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));
                if member.is_bitfield {
                    member_entry.set(gimli::DW_AT_bit_offset, AttributeValue::Data8(offset_bits as u64));
                    member_entry.set(gimli::DW_AT_bit_size, AttributeValue::Data1(1));
                };
            } else {
                member_entry.set(gimli::DW_AT_data_member_location, AttributeValue::Data8(offset));
                member_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));

                if let Some(size) = member.typ.size(self.types) {
                    let align = size.min(MAX_ALIGN) as u64;
                    offset += offset % align;
                    offset += size as u64;
                }
            }
        }

        id
    }

    fn define_union(&mut self, struct_: &UnionType) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_union_type);
        self.cache.insert(struct_.name.as_str().into(), id);

        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(struct_.name.as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        if let Some(size) = struct_.size {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size as u64));
        }

        for member in &struct_.members {
            let type_id = self.get_type(&member.typ);
            let member_id = self.unit.add(id, gimli::DW_TAG_member);
            let member_entry = self.unit.get_mut(member_id);
            let name = AttributeValue::String(member.name.as_bytes().to_vec());
            member_entry.set(gimli::DW_AT_name, name);
            member_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));
            if let Some(offset_bits) = member.bit_offset {
                let location = AttributeValue::Data8(offset_bits as u64 / u8::BITS as u64);
                member_entry.set(gimli::DW_AT_data_member_location, location);
            } else {
                member_entry.set(gimli::DW_AT_data_member_location, AttributeValue::Data8(0));
            }
        }

        id
    }

    fn define_enum(&mut self, enum_: &EnumType) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_enumeration_type);
        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(enum_.name.as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        if let Some(size) = enum_.size {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size as u64));
        }

        for member in &enum_.members {
            let entry = self.unit.add(id, gimli::DW_TAG_enumerator);
            let entry = self.unit.get_mut(entry);
            let name = AttributeValue::String(member.name.as_bytes().to_vec());
            entry.set(gimli::DW_AT_name, name);
            entry.set(gimli::DW_AT_const_value, AttributeValue::Sdata(member.value));
        }

        id
    }

    fn define_function_type(&mut self, fun: &FunctionType) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_subroutine_type);
        let ret_type = self.get_type(&fun.return_type);
        let entry = self.unit.get_mut(id);
        entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(ret_type));

        for arg in &fun.params {
            let type_id = self.get_type(arg);
            let arg_id = self.unit.add(id, gimli::DW_TAG_formal_parameter);
            let arg_entry = self.unit.get_mut(arg_id);
            arg_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));
        }

        id
    }

    fn define_vtable(&mut self, struct_: &StructType) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_structure_type);
        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(get_vtable_type_name(struct_).as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        let size = struct_.all_virtual_methods(self.types).count() * POINTER_SIZE;
        entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size as u64));

        for (i, method) in struct_.all_virtual_methods(self.types).enumerate() {
            let method_id = self.define_virtual_method(id, struct_.name.into(), i, method);
            let type_id = self.unit.add(id, gimli::DW_TAG_pointer_type);
            let type_entry = self.unit.get_mut(type_id);
            type_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(method_id));

            let member_id = self.unit.add(id, gimli::DW_TAG_member);
            let member_entry = self.unit.get_mut(member_id);
            let name = AttributeValue::String(method.name.as_bytes().to_vec());
            member_entry.set(gimli::DW_AT_name, name);
            member_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));
            let location = AttributeValue::Data8(i as u64 * POINTER_SIZE as u64);
            member_entry.set(gimli::DW_AT_data_member_location, location);
        }

        id
    }

    fn define_virtual_method(
        &mut self,
        parent: UnitEntryId,
        parent_id: StructId,
        index: usize,
        method: &Method,
    ) -> UnitEntryId {
        let id = self.unit.add(parent, gimli::DW_TAG_subroutine_type);
        let this_arg_id = self.unit.add(id, gimli::DW_TAG_formal_parameter);
        let this_type_id = self.get_type(&Type::Pointer(Type::Struct(parent_id).into()));
        let ret_type_id = self.get_type(&method.typ.return_type);

        let entry = self.unit.get_mut(id);
        entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(ret_type_id));
        let location = AttributeValue::Data8((index * POINTER_SIZE) as u64);
        entry.set(gimli::DW_AT_data_member_location, location);
        entry.set(gimli::DW_AT_object_pointer, AttributeValue::UnitRef(this_type_id));

        let this_arg_entry = self.unit.get_mut(this_arg_id);
        this_arg_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(this_type_id));
        this_arg_entry.set(gimli::DW_AT_artificial, AttributeValue::Data1(1));

        for arg in &method.typ.params {
            let type_id = self.get_type(arg);
            let arg_id = self.unit.add(id, gimli::DW_TAG_formal_parameter);
            let arg_entry = self.unit.get_mut(arg_id);
            arg_entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));
        }

        id
    }

    fn define_function_symbol(&mut self, fun: FunctionSymbol, image_base: u64) {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_subprogram);
        let ret_type_id = self.get_type(&fun.function_type().return_type);

        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(fun.name().as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        let pc = AttributeValue::Address(Address::Constant(image_base + fun.rva()));
        entry.set(gimli::DW_AT_low_pc, pc);
        entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(ret_type_id));

        for arg in &fun.function_type().params {
            let type_id = self.get_type(arg);
            let arg_id = self.unit.add(id, gimli::DW_TAG_formal_parameter);
            let param = self.unit.get_mut(arg_id);
            param.set(gimli::DW_AT_type, AttributeValue::UnitRef(type_id));
        }
    }
}

fn get_vtable_type_name(owner: &StructType) -> Cow<'static, str> {
    format!("{}_vft", owner.name).into()
}

fn get_vtable_field_name(_owner: &StructType) -> Cow<'static, str> {
    "vft".into()
}
