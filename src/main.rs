#![feature(slice_group_by)]
#![feature(assert_matches)]
#![feature(iter_advance_by)]
use std::fs::File;
use std::path::PathBuf;

use defns::Definitions;
use flexi_logger::{LogSpecification, Logger};

use crate::error::Error;
use crate::exe::ExecutableData;
use crate::symbols::ObjectProperties;

pub mod codegen;
pub mod defns;
pub mod error;
pub mod eval;
pub mod exe;
pub mod patterns;
pub mod symbols;

#[derive(Clone, Debug)]
struct Opts {
    source_path: PathBuf,
    exe_path: PathBuf,
    dwarf_output_path: Option<PathBuf>,
    c_output_path: Option<PathBuf>,
    rust_output_path: Option<PathBuf>,
}

fn opts() -> Opts {
    use bpaf::*;

    let source_path = positional("C_SOURCE").from_str::<PathBuf>();
    let exe_path = positional("EXE").from_str::<PathBuf>();
    let dwarf_output_path = long("dwarf-output")
        .help("DWARF file to write")
        .argument("DWARF")
        .from_str::<PathBuf>()
        .optional();
    let c_output_path = long("c-output")
        .help("C header with offsets to write")
        .argument("C")
        .from_str::<PathBuf>()
        .optional();
    let rust_output_path = long("rust-output")
        .help("Rust file with offsets to write")
        .argument("RUST")
        .from_str::<PathBuf>()
        .optional();

    let parser = construct!(Opts {
        source_path,
        exe_path,
        dwarf_output_path,
        c_output_path,
        rust_output_path
    });

    Info::default().descr("Zoltan").for_parser(parser).run()
}

fn main() {
    Logger::with(LogSpecification::info()).start().unwrap();

    let opts = opts();
    match run(&opts) {
        Ok(()) => log::info!("Finished!"),
        Err(err) => {
            log::error!("{err}");
            std::process::exit(1);
        }
    }
}

fn run(opts: &Opts) -> Result<(), Error> {
    let source = std::fs::read_to_string(&opts.source_path)?;
    let definitions = Definitions::from_source(&source)?;

    let exe_bytes = std::fs::read(&opts.exe_path)?;
    let exe = object::read::File::parse(&*exe_bytes)?;
    let data = ExecutableData::new(&exe)?;
    let (syms, errors) = symbols::resolve(definitions.into_functions(), &data)?;
    log::info!("Found {} symbols", syms.len());
    if !errors.is_empty() {
        let message = errors
            .iter()
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        log::warn!("Some of the patterns have failed:\n{message}",);
    }

    if let Some(path) = &opts.c_output_path {
        codegen::write_c_header(&syms, File::create(path)?)?;
    }
    if let Some(path) = &opts.rust_output_path {
        codegen::write_rust_header(&syms, File::create(path)?)?;
    }
    if let Some(path) = &opts.dwarf_output_path {
        let props = ObjectProperties::from_object(&exe);
        symbols::generate(syms, props, File::create(path)?)?;
    }

    Ok(())
}
