use aho_corasick::AhoCorasick;
use enum_as_inner::EnumAsInner;

#[derive(Debug, EnumAsInner)]
pub enum PatItem {
    Byte(u8),
    Any,
    Group(String, VarType),
}

impl PatItem {
    #[inline]
    fn size(&self) -> usize {
        match self {
            PatItem::Byte(_) => 1,
            PatItem::Any => 1,
            PatItem::Group(_, VarType::Rel) => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VarType {
    Rel,
}

#[derive(Debug)]
pub struct Pattern {
    parts: Vec<PatItem>,
    size: usize,
}

impl Pattern {
    #[inline]
    fn new(parts: Vec<PatItem>) -> Self {
        Self {
            size: parts.iter().map(PatItem::size).sum(),
            parts,
        }
    }

    pub fn parse(str: &str) -> Result<Self, peg::error::ParseError<peg::str::LineCol>> {
        pattern::pattern(str)
    }

    #[inline]
    fn parts(&self) -> &[PatItem] {
        &self.parts
    }

    #[inline]
    fn size(&self) -> usize {
        self.size
    }

    pub fn groups(&self) -> impl Iterator<Item = (&str, VarType, usize)> {
        self.parts
            .iter()
            .scan(0usize, |offset, it| {
                let pos = *offset;
                *offset += it.size();
                Some((it, pos))
            })
            .filter_map(|(it, offset)| it.as_group().map(|(key, typ)| (key.as_str(), *typ, offset)))
    }

    fn does_match(&self, bytes: &[u8]) -> bool {
        let mut bytes = bytes.iter();
        for pat in self.parts() {
            match pat {
                PatItem::Byte(expected) => {
                    if bytes.next() != Some(expected) {
                        return false;
                    }
                }
                PatItem::Group(_, _) => {
                    if bytes.advance_by(pat.size()).is_err() {
                        return false;
                    }
                }
                PatItem::Any => {
                    bytes.next();
                }
            }
        }
        true
    }

    fn longest_byte_sequence(&self) -> &[PatItem] {
        self.parts()
            .group_by(|a, b| a.as_byte().is_some() && b.as_byte().is_some())
            .max_by_key(|parts| parts.len())
            .unwrap_or_default()
    }
}

peg::parser! {
    grammar pattern() for str {
        rule _() =
            quiet!{[' ' | '\t']*}
        rule byte() -> u8
            = n:$(['0'..='9' | 'A'..='F']*<2>) {? u8::from_str_radix(n, 16).or(Err("byte")) }
        rule any()
            = "?"
        rule ident() -> String
            = id:$(['a'..='z' | 'A'..='Z' | '_']+) { id.to_owned() }
        rule var_type() -> VarType
            = "rel" { VarType::Rel }
        rule item() -> PatItem
            = n:byte() { PatItem::Byte(n) }
            / any() { PatItem::Any }
            / "(" _ id:ident() _ ":" _ typ:var_type() _ ")" { PatItem::Group(id, typ) }
        pub rule pattern() -> Pattern
            = items:item() ** _ { Pattern::new(items) }
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
        let start = offset_from(pat.parts(), seq);
        let offset: usize = pat.parts[0..start].iter().map(PatItem::size).sum();
        items.push((pat, offset));
        sequences.push(seq.iter().filter_map(PatItem::as_byte).cloned().collect());
    }

    let ac = AhoCorasick::new(&sequences);
    let mut matches = vec![];

    for mat in ac.find_overlapping_iter(haystack) {
        let (pat, offset) = items[mat.pattern()];
        let start = mat.start() - offset;
        let slice = &haystack[start..start + pat.size()];

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
#[inline]
fn offset_from<T>(slice: &[T], other: &[T]) -> usize {
    ((other.as_ptr() as usize) - (slice.as_ptr() as usize)) / std::mem::size_of::<T>()
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn parse_valid_patterns() {
        let pat = Pattern::parse("8B 0D ? ? BA 10").unwrap();
        assert_matches!(pat.parts(), &[
            PatItem::Byte(0x8B),
            PatItem::Byte(0x0D),
            PatItem::Any,
            PatItem::Any,
            PatItem::Byte(0xBA),
            PatItem::Byte(0x10),
        ]);

        let pat = Pattern::parse("8BF9E8??").unwrap();
        assert_matches!(pat.parts(), &[
            PatItem::Byte(0x8B),
            PatItem::Byte(0xF9),
            PatItem::Byte(0xe8),
            PatItem::Any,
            PatItem::Any,
        ]);
    }

    #[test]
    fn return_correct_longest_seq() {
        let pat = Pattern::parse("8B ? 0D ? F9 5F 48 B8 ? BA 10").unwrap();
        assert_matches!(pat.longest_byte_sequence(), &[
            PatItem::Byte(0xF9),
            PatItem::Byte(0x5F),
            PatItem::Byte(0x48),
            PatItem::Byte(0xB8)
        ]);
    }

    #[test]
    fn match_valid_patterns() {
        let pat1 = Pattern::parse("FD 98 07 ? ? 49 C5").unwrap();
        let pat2 = Pattern::parse("? BB 5E 83 F1 ? 49").unwrap();
        let pat3 = Pattern::parse("BA (match:rel) 89 BF").unwrap();
        let haystack = [
            0x9C, 0x0D, 0x1C, 0x53, 0x1D, 0x35, 0xFD, 0x98, 0x07, 0x10, 0x22, 0x49, 0xC5, 0xBB, 0x5E, 0x83,
            0xF1, 0xBF, 0x49, 0x8E, 0x78, 0x32, 0x17, 0xC1, 0x6F, 0xBA, 0x83, 0x5B, 0x5D, 0x83, 0x89, 0xBF,
        ];
        assert_matches!(multi_search([&pat1, &pat2, &pat3], &haystack).as_slice(), &[
            Match { pattern: 0, rva: 6 },
            Match { pattern: 1, rva: 12 },
            Match { pattern: 2, rva: 25 },
        ]);
    }

    #[test]
    fn return_correct_groups() {
        let pat = Pattern::parse("BA CC (one:rel) FF 89 BF (two:rel) (three:rel) 56").unwrap();
        assert_matches!(pat.groups().collect::<Vec<_>>().as_slice(), &[
            ("one", VarType::Rel, 2),
            ("two", VarType::Rel, 9),
            ("three", VarType::Rel, 13)
        ]);
    }
}
