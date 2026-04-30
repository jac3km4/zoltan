use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Opts {
    pub source_path: PathBuf,
    pub exe_path: PathBuf,
    pub dwarf_output_path: Option<PathBuf>,
    pub c_output_path: Option<PathBuf>,
    pub rust_output_path: Option<PathBuf>,
    pub strip_namespaces: bool,
    pub eager_type_export: bool,
    pub compiler_flags: Vec<String>,
}

impl Opts {
    pub fn load(header: &'static str) -> Self {
        use bpaf::*;

        let source_path = positional::<std::path::PathBuf>("SOURCE");
        let exe_path = positional::<std::path::PathBuf>("EXE");
        let dwarf_output_path = long("dwarf-output")
            .short('o')
            .help("DWARF file to write")
            .argument::<std::path::PathBuf>("DWARF")
            .optional();
        let c_output_path = long("c-output")
            .help("C header with offsets to write")
            .argument::<std::path::PathBuf>("C")
            .optional();
        let rust_output_path = long("rust-output")
            .help("Rust file with offsets to write")
            .argument::<std::path::PathBuf>("RUST")
            .optional();
        let strip_namespaces = long("strip-namespaces")
            .help("Strip namespaces from type names")
            .switch();
        let eager_type_export = long("eager-type-export")
            .help("Export all types found in the sources")
            .switch();
        let compiler_flags = long("compiler-flag")
            .short('f')
            .help("Flags to pass to the compiler")
            .argument::<String>("FLAGS")
            .map(|flag| format!("-{}", flag))
            .many();

        let parser = construct!(Opts {
            source_path,
            exe_path,
            dwarf_output_path,
            c_output_path,
            rust_output_path,
            strip_namespaces,
            eager_type_export,
            compiler_flags,
        });

        parser.to_options().descr(header).run()
    }
}
