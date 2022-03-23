use std::io::Write;

use crate::error::Error;
use crate::symbols::FunctionSymbol;

pub fn write_header<W: Write>(symbols: &[FunctionSymbol], mut out: W) -> Result<(), Error> {
    for symbol in symbols {
        writeln!(
            out,
            "#define {}_ADDR 0x{:X}",
            symbol.name().to_uppercase(),
            symbol.addr()
        )?;
    }

    Ok(())
}
