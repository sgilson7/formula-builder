//! The math font as integer data. `tools/font/extract.py` turns the vendored
//! STIX Two Math (SIL OFL 1.1) into `data/font/stix.txt`; the core embeds that
//! text when it is compiled and reads it once. Units are font units, 1000 to
//! the em, with y pointing up.

use std::sync::OnceLock;

const DATA: &str = include_str!("../../../data/font/stix.txt");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub key: String,
    pub adv: i32,
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub ital: i32,
    pub path: String,
}

#[derive(Debug, Default)]
pub struct Font {
    /// Sorted by key, so a lookup is a binary search.
    glyphs: Vec<Glyph>,
    consts: Vec<(String, i32)>,
    /// Code point, then the keys of its vertical variants, smallest first.
    variants: Vec<(u32, Vec<String>)>,
}

impl Font {
    pub fn parse(text: &str) -> Font {
        let mut f = Font::default();
        for line in text.lines() {
            let mut parts = line.splitn(9, ' ');
            match parts.next() {
                Some("k") => {
                    let name = parts.next().unwrap_or("").to_string();
                    let v = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    f.consts.push((name, v));
                }
                Some("g") => {
                    let key = parts.next().unwrap_or("").to_string();
                    let mut num = || parts.next().and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                    let (adv, x0, y0, x1, y1, ital) = (num(), num(), num(), num(), num(), num());
                    let path = parts.next().unwrap_or("").to_string();
                    f.glyphs.push(Glyph {
                        key,
                        adv,
                        x0,
                        y0,
                        x1,
                        y1,
                        ital,
                        path,
                    });
                }
                Some("v") => {
                    let cp = u32::from_str_radix(parts.next().unwrap_or(""), 16).unwrap_or(0);
                    let rest: Vec<String> = line.split(' ').skip(2).map(String::from).collect();
                    f.variants.push((cp, rest));
                }
                _ => {}
            }
        }
        f.glyphs.sort_by(|a, b| a.key.cmp(&b.key));
        f
    }

    pub fn glyph(&self, key: &str) -> Option<&Glyph> {
        self.glyphs
            .binary_search_by(|g| g.key.as_str().cmp(key))
            .ok()
            .map(|i| &self.glyphs[i])
    }

    pub fn cp(&self, cp: u32) -> Option<&Glyph> {
        self.glyph(&key_of(cp))
    }

    /// A MATH-table constant; 0 when the font lacks it.
    pub fn k(&self, name: &str) -> i32 {
        self.consts
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    }

    pub fn variants(&self, cp: u32) -> Vec<&Glyph> {
        let keys = self
            .variants
            .iter()
            .find(|(c, _)| *c == cp)
            .map(|(_, k)| k.clone())
            .unwrap_or_default();
        keys.iter().filter_map(|k| self.glyph(k)).collect()
    }
}

pub fn key_of(cp: u32) -> String {
    format!("{cp:X}")
}

/// The font, read from the embedded table the first time it is needed.
pub fn font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(|| Font::parse(DATA))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_table_has_the_constants_and_glyphs_layout_needs() {
        let f = font();
        assert_eq!(f.k("AxisHeight"), 258);
        assert_eq!(f.k("ScriptPercentScaleDown"), 70);
        // italic a, from the font's own numbers
        let a = f.cp(0x1D44E).unwrap();
        assert_eq!((a.adv, a.y0, a.y1), (555, -12, 479));
        assert!(f.variants(0x28).len() > 4, "parentheses grow in several sizes");
        assert!(f.cp(0x2211).is_some() && f.cp(0x221A).is_some());
    }

    #[test]
    fn a_missing_glyph_or_constant_is_none_or_zero() {
        let f = Font::parse("k A 5\ng 41 10 0 0 10 10 0 M0,0\n");
        assert_eq!(f.k("A"), 5);
        assert_eq!(f.k("B"), 0);
        assert!(f.cp(0x42).is_none());
        assert_eq!(f.cp(0x41).unwrap().adv, 10);
    }
}
