use std::borrow::Cow;
use std::collections::HashMap;
use std::io;

use gimli::write::{Address, AttributeValue, DwarfUnit, EndianVec, Sections, Unit, UnitEntryId};
use gimli::DwAte;
use object::{Architecture, BinaryFormat, Endianness, Object, SectionKind};
use saltwater::data::types::Type;
use saltwater::hir::Variable;
use saltwater::types::{ArrayType, FunctionType};
use saltwater::{get_str, InternedStr, StructType};

use crate::defns::Function;
use crate::error::{Error, SymbolError};
use crate::eval::EvalContext;
use crate::exe::ExecutableData;
use crate::patterns;

const DWARF_VERSION: u16 = 5;

pub fn resolve(
    functions: Vec<Function>,
    data: &ExecutableData,
) -> Result<(Vec<FunctionSymbol>, Vec<SymbolError>), Error> {
    let mut match_map: HashMap<usize, Vec<u64>> = HashMap::new();
    for mat in patterns::multi_search(functions.iter().map(Function::pattern), data.text()) {
        match_map.entry(mat.pattern).or_default().push(mat.rva);
    }

    let mut syms = vec![];
    let mut errs = vec![];
    for (i, fun) in functions.into_iter().enumerate() {
        match match_map.get(&i).map(|vec| &vec[..]) {
            Some([rva]) => {
                let addr = resolve_address(&fun, data, *rva)?;
                syms.push(fun.into_symbol(addr));
            }
            Some(rvas) => {
                errs.push(SymbolError::MoreThanOneMatch(fun.name, rvas.len()));
            }
            None => {
                errs.push(SymbolError::NoMatches(fun.name));
            }
        }
    }
    Ok((syms, errs))
}

fn resolve_address(fun: &Function, data: &ExecutableData, rva: u64) -> Result<u64, Error> {
    let res = match &fun.eval {
        Some(expr) => expr.eval(&EvalContext::new(fun.pattern(), data, rva)?)?,
        None => data.text_offset() + (rva as i64 - fun.offset.unwrap_or(0) as i64) as u64,
    };
    Ok(res)
}

pub fn generate<W: io::Write>(
    symbols: Vec<FunctionSymbol>,
    props: ObjectProperties,
    out: W,
) -> Result<(), Error> {
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
    let mut processor = DwarfProcessor::new(&mut dwarf.unit);
    for sym in symbols {
        processor.define_function(sym);
    }

    // TODO: handle endianess here
    let mut sections = Sections::new(EndianVec::new(gimli::LittleEndian));
    dwarf.write(&mut sections)?;

    let mut obj = object::write::Object::new(BinaryFormat::Elf, props.architecture, props.endianess);
    sections.for_each_mut(|id, data| {
        let name = id.name().as_bytes().to_vec();
        let id = obj.add_section(b"LOAD".to_vec(), name, SectionKind::Debug);
        obj.set_section_data(id, Cow::Owned(data.take()), 8);
        Ok::<(), Error>(())
    })?;
    obj.write_stream(out)?;

    Ok(())
}

struct DwarfProcessor<'a> {
    unit: &'a mut Unit,
    known: HashMap<Cow<'static, str>, UnitEntryId>,
}

impl<'a> DwarfProcessor<'a> {
    fn new(unit: &'a mut Unit) -> Self {
        Self {
            unit,
            known: HashMap::new(),
        }
    }

    fn get_type(&mut self, typ: &Type) -> UnitEntryId {
        let name = get_type_name(typ);
        self.known.get(&name).cloned().unwrap_or_else(|| {
            let id = self.define_type(typ);
            self.known.insert(name, id);
            id
        })
    }

    fn define_type(&mut self, typ: &Type) -> UnitEntryId {
        match typ {
            Type::Void => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Bool => self.define_base_type(typ, gimli::DW_ATE_boolean),
            Type::Char(true) => self.define_base_type(typ, gimli::DW_ATE_signed_char),
            Type::Char(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned_char),
            Type::Short(true) => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Short(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned),
            Type::Int(true) => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Int(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned),
            Type::Long(true) => self.define_base_type(typ, gimli::DW_ATE_signed),
            Type::Long(false) => self.define_base_type(typ, gimli::DW_ATE_unsigned),
            Type::Float => self.define_base_type(typ, gimli::DW_ATE_float),
            Type::Double => self.define_base_type(typ, gimli::DW_ATE_float),
            Type::Pointer(inner, qual) => {
                let id = self.unit.add(self.unit.root(), gimli::DW_TAG_pointer_type);
                let inner = self.get_type(inner);
                let entry = self.unit.get_mut(id);
                entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(inner));
                let encoding = AttributeValue::Encoding(gimli::DW_ATE_address);
                entry.set(gimli::DW_AT_encoding, encoding);
                entry.set(gimli::DW_AT_byte_size, AttributeValue::Data2(8));
                entry.set(gimli::DW_AT_mutable, AttributeValue::Flag(!qual.c_const));
                id
            }
            Type::Struct(StructType::Named(name, ty_ref)) => {
                self.define_struct(get_str!(name), &ty_ref.get(), typ.sizeof().ok())
            }
            Type::Array(inner, arr_type) => {
                let id = self.unit.add(self.unit.root(), gimli::DW_TAG_array_type);
                let inner = self.get_type(inner);
                let entry = self.unit.get_mut(id);
                entry.set(gimli::DW_AT_type, AttributeValue::UnitRef(inner));
                if let Ok(size) = typ.sizeof() {
                    entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size));
                }
                if let ArrayType::Fixed(len) = arr_type {
                    let range = self.unit.add(id, gimli::DW_TAG_subrange_type);
                    let range = self.unit.get_mut(range);
                    range.set(gimli::DW_AT_upper_bound, AttributeValue::Data8(*len));
                }
                id
            }
            Type::Enum(Some(name), members) => self.define_enum(get_str!(name), members),
            Type::Function(_) => todo!(),
            Type::Union(_) => todo!(),
            _ => unimplemented!(),
        }
    }

    fn define_base_type(&mut self, typ: &Type, encoding: DwAte) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_base_type);
        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(get_type_name(typ).as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        entry.set(gimli::DW_AT_encoding, AttributeValue::Encoding(encoding));

        if typ == &Type::Void {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data1(0));
        } else if let Ok(size) = typ.sizeof() {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size));
        }
        id
    }

    fn define_struct(&mut self, name: &str, members: &[Variable], size: Option<u64>) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_structure_type);
        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(name.as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        if let Some(size) = size {
            entry.set(gimli::DW_AT_byte_size, AttributeValue::Data8(size));
        }

        let mut member_types = vec![];
        for member in members {
            let typ = self.get_type(&member.ctype);
            let size = member.ctype.sizeof().ok();
            let align = member.ctype.alignof().ok();
            member_types.push((member.id, typ, size, align))
        }

        let mut offset = 0;
        for (name, typ_id, size, align) in member_types {
            let param = self.unit.add(id, gimli::DW_TAG_member);
            let param = self.unit.get_mut(param);
            let name = AttributeValue::String(get_str!(name).as_bytes().to_vec());
            param.set(gimli::DW_AT_name, name);
            param.set(gimli::DW_AT_type, AttributeValue::UnitRef(typ_id));
            param.set(gimli::DW_AT_data_member_location, AttributeValue::Data8(offset));

            if let Some(size) = size {
                offset += offset % align.unwrap_or(1);
                offset += size;
            }
        }

        id
    }

    fn define_enum(&mut self, name: &str, members: &[(InternedStr, i64)]) -> UnitEntryId {
        let id = self.unit.add(self.unit.root(), gimli::DW_TAG_enumeration_type);
        let entry = self.unit.get_mut(id);
        let name = AttributeValue::String(name.as_bytes().to_vec());
        entry.set(gimli::DW_AT_name, name);
        entry.set(gimli::DW_AT_byte_size, AttributeValue::Data1(4));

        for (member_name, val) in members {
            let member = self.unit.add(id, gimli::DW_TAG_enumerator);
            let member = self.unit.get_mut(member);
            let name = AttributeValue::String(get_str!(member_name).as_bytes().to_vec());
            member.set(gimli::DW_AT_name, name);
            member.set(gimli::DW_AT_const_value, AttributeValue::Sdata(*val));
        }

        id
    }

    fn define_function(&mut self, fun: FunctionSymbol) {
        let ret_type = self.get_type(&fun.typ.return_type);
        let mut args = vec![];
        for arg_types in fun.typ.params {
            let var = arg_types.get();
            args.push((var.id, self.get_type(&var.ctype)))
        }

        let proc_id = self.unit.add(self.unit.root(), gimli::DW_TAG_subprogram);
        let proc = self.unit.get_mut(proc_id);
        let name = AttributeValue::String(fun.name.as_bytes().to_vec());
        proc.set(gimli::DW_AT_name, name);
        let pc = AttributeValue::Address(Address::Constant(fun.addr));
        proc.set(gimli::DW_AT_low_pc, pc);
        proc.set(gimli::DW_AT_type, AttributeValue::UnitRef(ret_type));

        for (name, typ_id) in args {
            let param = self.unit.add(proc_id, gimli::DW_TAG_formal_parameter);
            let param = self.unit.get_mut(param);
            let name = AttributeValue::String(get_str!(name).as_bytes().to_vec());
            param.set(gimli::DW_AT_name, name);
            param.set(gimli::DW_AT_type, AttributeValue::UnitRef(typ_id));
        }
    }
}

pub fn get_type_name(typ: &Type) -> Cow<'static, str> {
    match typ {
        Type::Void => Cow::Borrowed("void"),
        Type::Bool => Cow::Borrowed("bool"),
        Type::Char(_) => Cow::Borrowed("char"),
        Type::Short(_) => Cow::Borrowed("short"),
        Type::Int(_) => Cow::Borrowed("int"),
        Type::Long(_) => Cow::Borrowed("long"),
        Type::Float => Cow::Borrowed("float"),
        Type::Double => Cow::Borrowed("double"),
        Type::Pointer(inner, _) => Cow::Owned(format!("{}*", get_type_name(inner))),
        Type::Array(inner, _) => Cow::Owned(format!("[{}]", get_type_name(inner))),
        Type::Function(fun) => {
            let ret = get_type_name(&fun.return_type);
            let mut params = String::new();
            for param in &fun.params {
                let var = param.get();
                params.push_str(&get_type_name(&var.ctype));
                params.push_str(", ");
            }
            Cow::Owned(format!("{} ({})", ret, params))
        }
        Type::Union(StructType::Named(name, _)) => Cow::Owned(name.resolve_and_clone()),
        Type::Struct(StructType::Named(name, _)) => Cow::Owned(name.resolve_and_clone()),
        Type::Enum(Some(name), _) => Cow::Owned(name.resolve_and_clone()),
        _ => unimplemented!(),
    }
}

#[derive(Debug)]
pub struct FunctionSymbol {
    name: String,
    typ: FunctionType,
    addr: u64,
}

impl FunctionSymbol {
    pub(crate) fn new(name: String, typ: FunctionType, addr: u64) -> Self {
        Self { name, typ, addr }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }
}

#[derive(Debug)]
pub struct ObjectProperties {
    architecture: Architecture,
    endianess: Endianness,
}

impl ObjectProperties {
    pub fn from_object<'a: 'b, 'b, O: Object<'a, 'b>>(obj: &O) -> Self {
        Self {
            architecture: obj.architecture(),
            endianess: obj.endianness(),
        }
    }

    fn is64bit(&self) -> bool {
        match self.architecture {
            Architecture::X86_64 => true,
            Architecture::X86_64_X32 => false,
            _ => unimplemented!(),
        }
    }

    fn address_size(&self) -> u8 {
        match self.architecture {
            Architecture::X86_64 => 8,
            Architecture::X86_64_X32 => 4,
            _ => unimplemented!(),
        }
    }
}
