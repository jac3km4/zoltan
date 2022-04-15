use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Opts {
    pub source_path: PathBuf,
    pub exe_path: PathBuf,
    pub dwarf_output_path: Option<PathBuf>,
    pub c_output_path: Option<PathBuf>,
    pub rust_output_path: Option<PathBuf>,
    pub compiler_flags: Vec<String>,
    pub strip_namespaces: bool,
    pub eager_type_export: bool,
}

impl Opts {
    pub fn load(header: &'static str) -> Self {
        use bpaf::*;

        let source_path = positional("SOURCE").from_str::<PathBuf>();
        let exe_path = positional("EXE").from_str::<PathBuf>();
        let dwarf_output_path = long("dwarf-output")
            .short('o')
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
        let compiler_flags = long("compiler-flag")
            .short('f')
            .help("Flags to pass to the compiler")
            .argument("FLAGS")
            .many()
            .map(|flags| flags.into_iter().map(|flag| "-".to_owned() + &flag).collect());
        let strip_namespaces = long("strip-namespaces")
            .help("Strip namespaces from type names")
            .switch()
            .optional()
            .map(|val| val.unwrap_or(false));
        let eager_type_export = long("eager-type-export")
            .help("Export all types found in the sources")
            .switch()
            .optional()
            .map(|val| val.unwrap_or(false));

        let parser = construct!(Opts {
            source_path,
            exe_path,
            dwarf_output_path,
            c_output_path,
            rust_output_path,
            compiler_flags,
            strip_namespaces,
            eager_type_export
        });

        Info::default().descr(header).for_parser(parser).run()
    }
}
