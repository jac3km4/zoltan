use error::{Error, Result};
use flexi_logger::{LogSpecification, Logger};
use resolver::TypeResolver;
use saltwater::codespan::LineIndex;
use saltwater::hir::Variable;
use saltwater::{check_semantics, get_str, Opt, StorageClass};
use zoltan::opts::Opts;
use zoltan::spec::FunctionSpec;
use zoltan::types::Type;

mod error;
mod resolver;

fn main() {
    Logger::with(LogSpecification::info()).start().unwrap();

    let opts = Opts::load("Zoltan Saltwater frontend for C");
    match run(&opts) {
        Ok(()) => log::info!("Finished!"),
        Err(err) => {
            log::error!("{err}");
            std::process::exit(1);
        }
    }
}

fn run(opts: &Opts) -> Result<()> {
    let source = std::fs::read_to_string(&opts.source_path)?;
    let program = check_semantics(source.as_ref(), Opt::default());

    let mut resolver = TypeResolver::default();
    let mut specs = vec![];

    for decl in program
        .result
        .map_err(|errs| Error::from_compile_errors(errs, &program.files))?
    {
        let var = decl.data.symbol.get();
        if let Variable {
            ctype: function_type,
            storage_class: StorageClass::Typedef,
            ..
        } = &*var
        {
            let file = decl.location.file;
            let line = program.files.line_index(file, decl.location.span.start);
            let comments = (0..line.0)
                .rev()
                .map(|li| {
                    let span = program.files.line_span(file, LineIndex(li)).unwrap();
                    program.files.source_slice(file, span).unwrap()
                })
                .take_while(|str| str.starts_with("///"));

            if let Type::Function(fn_type) = resolver.resolve_type(function_type)? {
                if let Some(spec) = FunctionSpec::new(get_str!(var.id).into(), fn_type, comments) {
                    specs.push(spec?);
                }
            }
        } else if opts.eager_type_export {
            resolver.resolve_type(&var.ctype)?;
        }
    }

    zoltan::process_specs(specs, &resolver.into_types(), opts)?;

    Ok(())
}
