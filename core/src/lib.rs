#![feature(slice_group_by)]
#![feature(assert_matches)]
#![feature(iter_advance_by)]

pub mod codegen;
pub mod dwarf;
pub mod error;
pub mod eval;
pub mod exe;
pub mod opts;
pub mod patterns;
pub mod spec;
pub mod symbols;
pub mod types;

use std::fs::File;

use error::Result;
use exe::ExecutableData;
use opts::Opts;
use spec::FunctionSpec;
use types::TypeInfo;
pub use ustr;

use crate::exe::ExeProperties;

pub fn process_specs(specs: Vec<FunctionSpec>, type_info: &TypeInfo, opts: &Opts) -> Result<()> {
    let exe_bytes = std::fs::read(&opts.exe_path)?;
    let exe = object::read::File::parse(&*exe_bytes)?;
    let data = ExecutableData::new(&exe)?;

    log::info!("Searching for symbols...");
    let (syms, errors) = symbols::resolve_in_exe(specs, &data)?;
    log::info!("Found {} symbol(s)", syms.len());

    if !errors.is_empty() {
        let message = errors
            .iter()
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        log::warn!("Some of the patterns have failed:\n{message}",);
    }

    if opts.c_output_path.is_none() && opts.rust_output_path.is_none() && opts.dwarf_output_path.is_none() {
        log::error!("No output option specified, nothing to do")
    }

    if let Some(path) = &opts.c_output_path {
        codegen::write_c_header(File::create(path)?, &syms)?;
    }
    if let Some(path) = &opts.rust_output_path {
        codegen::write_rust_header(File::create(path)?, &syms)?;
    }
    if let Some(path) = &opts.dwarf_output_path {
        let props = ExeProperties::from_object(&exe);
        dwarf::write_symbol_file(File::create(path)?, syms, type_info, props)?;
    }

    Ok(())
}
