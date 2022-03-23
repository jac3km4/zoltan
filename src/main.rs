#![feature(slice_group_by)]
#![feature(assert_matches)]
use std::fs::File;
use std::path::Path;

use defns::Definitions;
use flexi_logger::{LogSpecification, Logger};
use object::{Object, ObjectSection};

use crate::error::Error;
use crate::symbols::ObjectProperties;

pub mod defns;
pub mod error;
pub mod patterns;
pub mod symbols;

const CODE_SECTION: &str = ".text";

fn main() {
    Logger::with(LogSpecification::info()).start().unwrap();

    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [source_path, exe_path, out_path] => {
            match run(source_path.as_ref(), exe_path.as_ref(), out_path.as_ref()) {
                Ok(()) => log::info!("Finished!"),
                Err(err) => {
                    log::error!("{err}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            println!("Usage: zoltan [C header] [executable] [DWARF output]")
        }
    }
}

fn run(source_path: &Path, exe_path: &Path, out_path: &Path) -> Result<(), Error> {
    let source = std::fs::read_to_string(source_path)?;
    let definitions = Definitions::from_source(&source)?;

    let exe_bytes = std::fs::read(exe_path)?;
    let exe = object::read::File::parse(&*exe_bytes)?;
    let code = exe.section_by_name(CODE_SECTION);

    if let Some(code) = code {
        let props = ObjectProperties::from_object(&exe);
        let (syms, errors) = symbols::resolve(definitions.into_functions(), code.data()?, code.address());
        log::info!("Found {} symbols", syms.len());
        if !errors.is_empty() {
            let message = errors
                .iter()
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            log::warn!("Some of the patterns have failed:\n{message}",);
        }

        symbols::generate(syms, props, File::create(out_path)?)?;
        log::info!("Written the debug symbols to {}", out_path.display());
    } else {
        log::error!("Code section missing from the executable, nothing to do")
    }
    Ok(())
}
