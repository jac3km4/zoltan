use std::num::ParseIntError;

use aho_corasick::AhoCorasick;
use enum_as_inner::EnumAsInner;

use crate::error::Error;

#[derive(Debug, EnumAsInner)]
pub enum PatItem {
    Byte(u8),
    Any,
}

#[derive(Debug)]
pub struct Pattern(Vec<PatItem>);

impl Pattern {
    pub fn parse(str: &str) -> Result<Self, Error> {
        fn from_part(str: &str, parts: &mut Vec<PatItem>) -> Result<(), Error> {
            if let Some("?") = str.get(0..1) {
                parts.push(PatItem::Any);
                from_part(&str[1..], parts)
            } else if let Some(part) = str.get(0..2) {
                let byte = u8::from_str_radix(part, 16)
                    .map_err(|err: ParseIntError| Error::InvalidCommentParam("pattern", err.to_string()))?;
                parts.push(PatItem::Byte(byte));
                from_part(&str[2..], parts)
            } else {
                Ok(())
            }
        }

        let mut parts = vec![];
        from_part(&str.replace(' ', ""), &mut parts)?;
        Ok(Pattern(parts))
    }

    #[inline]
    fn parts(&self) -> &[PatItem] {
        &self.0
    }

    #[inline]
    fn does_match(&self, bytes: &[u8]) -> bool {
        self.parts().iter().zip(bytes).all(|(pat, val)| match pat {
            PatItem::Byte(expected) => expected == val,
            PatItem::Any => true,
        })
    }

    fn longest_byte_sequence(&self) -> &[PatItem] {
        self.parts()
            .group_by(|a, b| a.as_byte().is_some() && b.as_byte().is_some())
            .max_by_key(|parts| parts.len())
            .unwrap_or_default()
    }
}

pub fn multi_search<'a, I>(patterns: I, haystack: &[u8]) -> Vec<Match>
where
    I: IntoIterator<Item = &'a Pattern>,
{
    let mut items = vec![];
    let mut sequences: Vec<Vec<u8>> = vec![];

    for pat in patterns {
        let seq = pat.longest_byte_sequence();
        let offset = offset_from(pat.parts(), seq);
        items.push((pat, offset));
        sequences.push(seq.iter().filter_map(PatItem::as_byte).cloned().collect());
    }

    let ac = AhoCorasick::new(&sequences);
    let mut matches = vec![];

    for mat in ac.find_overlapping_iter(haystack) {
        let (pat, offset) = items[mat.pattern()];
        let start = mat.start() - offset;
        let slice = &haystack[start..start + pat.parts().len()];

        if pat.does_match(slice) {
            let mat = Match {
                pattern: mat.pattern(),
                rva: start as u64,
            };
            matches.push(mat);
        }
    }
    matches
}

#[derive(Debug)]
pub struct Match {
    pub pattern: usize,
    pub rva: u64,
}

/// Returns the offset of `other` into `slice`.
fn offset_from<T>(slice: &[T], other: &[T]) -> usize {
    ((other.as_ptr() as usize) - (slice.as_ptr() as usize)) / std::mem::size_of::<T>()
}
