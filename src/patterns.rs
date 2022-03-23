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

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn parse_valid_patterns() -> Result<(), Error> {
        let pat = Pattern::parse("8B 0D ? ? BA 10")?;
        assert_matches!(pat.0.as_slice(), &[
            PatItem::Byte(0x8B),
            PatItem::Byte(0x0D),
            PatItem::Any,
            PatItem::Any,
            PatItem::Byte(0xBA),
            PatItem::Byte(0x10),
        ]);

        let pat = Pattern::parse("8bf9e8??")?;
        assert_matches!(pat.0.as_slice(), &[
            PatItem::Byte(0x8B),
            PatItem::Byte(0xF9),
            PatItem::Byte(0xe8),
            PatItem::Any,
            PatItem::Any,
        ]);
        Ok(())
    }

    #[test]
    fn return_correct_longest_seq() -> Result<(), Error> {
        let pat = Pattern::parse("8B ? 0D ? F9 5F 48 B8 ? BA 10")?;
        assert_matches!(pat.longest_byte_sequence(), &[
            PatItem::Byte(0xF9),
            PatItem::Byte(0x5F),
            PatItem::Byte(0x48),
            PatItem::Byte(0xB8)
        ]);
        Ok(())
    }

    #[test]
    fn match_valid_patterns() -> Result<(), Error> {
        let pat1 = Pattern::parse("FD 98 07 ? ? 49 C5")?;
        let pat2 = Pattern::parse("? BB 5E 83 F1 ? 49")?;
        let haystack = [
            0x9C, 0x0D, 0x1C, 0x53, 0x1D, 0x35, 0xFD, 0x98, 0x07, 0x10, 0x22, 0x49, 0xC5, 0xBB, 0x5E, 0x83,
            0xF1, 0xBF, 0x49, 0x8E, 0x78, 0x32, 0x17, 0xC1, 0x6F, 0xBA, 0x83, 0x5B, 0x5D, 0x83, 0x89, 0xBF,
        ];
        assert_matches!(multi_search([&pat1, &pat2], &haystack).as_slice(), &[
            Match { pattern: 0, rva: 6 },
            Match { pattern: 1, rva: 12 }
        ]);
        Ok(())
    }
}
