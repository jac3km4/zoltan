use object::{Object, ObjectSection};

use crate::error::Error;

pub struct ExecutableData<'a> {
    text: &'a [u8],
    rdata: &'a [u8],
    rdata_offset: u64,
    text_offset: u64,
}

impl<'a> ExecutableData<'a> {
    const TEXT_SECTION: &'static str = ".text";
    const RDATA_SECTION: &'static str = ".rdata";

    pub fn new(exe: &'a object::read::File<'a>) -> Result<Self, Error> {
        let text = exe
            .section_by_name(Self::TEXT_SECTION)
            .ok_or(Error::MissingSection("text"))?;
        let rdata = exe
            .section_by_name(Self::RDATA_SECTION)
            .ok_or(Error::MissingSection("rdata"))?;

        let res = Self {
            text: text.data()?,
            rdata: rdata.data()?,
            rdata_offset: rdata.address(),
            text_offset: text.address(),
        };
        Ok(res)
    }

    pub fn resolve_rel_text(&self, addr: u64) -> Result<u64, Error> {
        let addr = addr as usize;
        let bytes = self
            .text
            .get(addr..addr + std::mem::size_of::<i32>())
            .ok_or(Error::InvalidAccess(addr))?
            .try_into()
            .unwrap();
        let rel = i32::from_ne_bytes(bytes);
        let abs = self.text_offset as i64 + addr as i64 + std::mem::size_of::<i32>() as i64 + rel as i64;
        Ok(abs as u64)
    }

    pub fn resolve_rel_rdata(&self, addr: u64) -> Result<u64, Error> {
        let addr = addr as usize - self.rdata_offset as usize;
        let bytes = self
            .rdata
            .get(addr..addr + std::mem::size_of::<u64>())
            .ok_or(Error::InvalidAccess(addr))?
            .try_into()
            .unwrap();
        Ok(u64::from_ne_bytes(bytes))
    }

    pub fn text(&'a self) -> &'a [u8] {
        self.text
    }

    pub fn text_offset(&'a self) -> u64 {
        self.text_offset
    }
}
