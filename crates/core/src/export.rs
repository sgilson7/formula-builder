//! Exports that are text: LaTeX for papers and slides, and the address
//! fragment that carries a whole document into an embed or a shared link.

use crate::doc::{Doc, Model};
use crate::palette::COLOUR;
use crate::sheet::{Pattern, Side};

fn macro_name(id: &str) -> String {
    let letters: String = id.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    format!("fb{letters}")
}

fn ulem(p: Pattern) -> &'static str {
    match p {
        Pattern::Solid => "\\uline",
        Pattern::Dashed => "\\dashuline",
        Pattern::Dotted => "\\dotuline",
        Pattern::Wave => "\\uwave",
        Pattern::Double => "\\uuline",
    }
}

/// LaTeX for every formula: a preamble that defines one macro per concept
/// (colour and underline pattern, from `ulem`), then each formula with its
/// concepts wrapped, labels as `\overset`/`\underset` and braces as
/// `\underbrace`/`\overbrace`. The formula stays readable: the styling lives in
/// the preamble (the paper's R6).
pub fn latex(m: &Model, header: &str) -> String {
    let mut out = styled(m, header);
    out += "\n% Plain version, for a document that takes no new macros: colour only.\n";
    out += &formulas(m, true);
    out
}

fn styled(m: &Model, header: &str) -> String {
    let mut out = format!("% {header}\n% Preamble: needs xcolor, amsmath and ulem.\n\\usepackage{{xcolor,amsmath}}\n\\usepackage[normalem]{{ulem}}\n");
    for c in &m.sheet.concepts {
        let colour = if c.colour {
            COLOUR.get(c.num - 1).map(|p| &p.0[1..])
        } else {
            None
        };
        let mac = macro_name(&c.id);
        let line = ulem(c.pattern);
        match colour {
            Some(hex) => {
                out += &format!("\\definecolor{{{mac}}}{{HTML}}{{{hex}}}\n");
                out += &format!("\\newcommand{{\\{mac}}}[1]{{\\textcolor{{{mac}}}{{\\text{{{line}{{$#1$}}}}}}}} % {} {}\n", c.num, c.name);
            }
            None => {
                out += &format!(
                    "\\newcommand{{\\{mac}}}[1]{{\\text{{{line}{{$#1$}}}}}} % {} {}\n",
                    c.num, c.name
                )
            }
        }
    }
    out += "\n";
    out + &formulas(m, false)
}

/// Each formula with its concepts wrapped: in their macros, or, when `plain`,
/// in `\textcolor` alone.
fn formulas(m: &Model, plain: bool) -> String {
    let mut out = String::new();
    for (fi, src) in m.lines.iter().enumerate() {
        let Some(root) = &m.roots[fi] else { continue };
        // (position, order, text): opening marks sort before closing ones at
        // the same place, outer before inner when opening
        let mut marks: Vec<(usize, i64, String)> = Vec::new();
        let mut taken: Vec<(usize, usize)> = Vec::new();
        let mut occs: Vec<_> = m.occs.iter().filter(|o| o.formula == fi).collect();
        occs.sort_by_key(|o| o.depth);
        for o in occs {
            let Some(row) = root.find(o.row) else { continue };
            let (a, b) = (row.kids[o.first].span.0, row.kids[o.first + o.len - 1].span.1);
            if taken
                .iter()
                .any(|(x, y)| (a < *x && *x < b && b < *y) || (*x < a && a < *y && *y < b))
            {
                continue;
            }
            taken.push((a, b));
            let c = &m.sheet.concepts[o.concept];
            let width = (b - a) as i64;
            if plain {
                let open = match COLOUR.get(c.num - 1).filter(|_| c.colour) {
                    Some((hex, _)) => format!("\\textcolor[HTML]{{{}}}{{", &hex[1..]),
                    None => "{".to_string(),
                };
                marks.push((a, -width, open));
                marks.push((b, width, "}".to_string()));
                continue;
            }
            let mut open = format!("\\{}{{", macro_name(&c.id));
            let mut close = "}".to_string();
            if let Some((side, text)) = &c.brace {
                match side {
                    Side::Below => {
                        open = format!("\\underbrace{{{open}");
                        close = format!("{close}}}_{{\\text{{{text}}}}}");
                    }
                    Side::Above => {
                        open = format!("\\overbrace{{{open}");
                        close = format!("{close}}}^{{\\text{{{text}}}}}");
                    }
                }
            }
            if let Some((side, text)) = &c.label {
                let cmd = if *side == Side::Above {
                    "overset"
                } else {
                    "underset"
                };
                open = format!("\\{cmd}{{\\text{{\\scriptsize {text}}}}}{{{open}");
                close = format!("{close}}}");
            }
            marks.push((a, -width, open));
            marks.push((b, width, close));
        }
        let mut text = String::new();
        let mut at = 0;
        // closings before openings at the same position
        marks.sort_by(|x, y| {
            x.0.cmp(&y.0)
                .then_with(|| (x.1 < 0).cmp(&(y.1 < 0)))
                .then(x.1.cmp(&y.1))
        });
        for (pos, _, mark) in marks {
            text += &src[at..pos];
            text += &mark;
            at = pos;
        }
        text += &src[at..];
        out += &format!("\\[\n  {text}\n\\]\n");
    }
    out
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64(bytes: &[u8]) -> String {
    let mut out = String::new();
    for ch in bytes.chunks(3) {
        let n =
            (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for i in 0..=ch.len() {
            out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn unb64(text: &str) -> Option<Vec<u8>> {
    let vals: Vec<u32> = text
        .bytes()
        .map(|c| B64.iter().position(|b| *b == c).map(|p| p as u32))
        .collect::<Option<_>>()?;
    let mut out = Vec::new();
    for ch in vals.chunks(4) {
        if ch.len() < 2 {
            return None;
        }
        let n = ch
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, v)| n | v << (18 - 6 * i));
        for i in 0..ch.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// The address fragment for a document: `#f=` and the document as base64url
/// JSON. A fragment is never sent to a server.
pub fn fragment(doc: &Doc) -> String {
    let json = serde_json::to_string(doc).unwrap_or_default();
    format!("#f={}", b64(json.as_bytes()))
}

pub fn from_fragment(hash: &str) -> Result<Doc, String> {
    let body = hash
        .trim_start_matches('#')
        .strip_prefix("f=")
        .ok_or("This link does not hold a formula.")?;
    let bytes = unb64(body)
        .ok_or("This link's formula is damaged; it may have been cut short when it was copied.")?;
    let text = String::from_utf8(bytes).map_err(|_| "This link's formula is damaged.".to_string())?;
    serde_json::from_str(&text).map_err(|_| "This link's formula is damaged.".to_string())
}

/// A saved document file.
pub fn from_file(text: &str) -> Result<Doc, String> {
    serde_json::from_str(text).map_err(|e| format!("This file is not a Formula Builder document: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(f: &str, s: &str) -> Model {
        Model::build(&Doc {
            title: "t".into(),
            formulas: f.into(),
            sheet: s.into(),
            prose: String::new(),
        })
    }

    #[test]
    fn latex_wraps_concepts_in_macros_defined_once() {
        let m = model(
            "y = m x + b",
            "concept slope: m\n  label above\nconcept intercept: b\n  brace below\n",
        );
        let t = latex(&m, "Formula Builder");
        assert!(t.contains("\\definecolor{fbslope}{HTML}{1D6FA3}"));
        assert!(t.contains("\\newcommand{\\fbslope}[1]{\\textcolor{fbslope}{\\text{\\uline{$#1$}}}}"));
        assert!(
            t.contains("\\newcommand{\\fbintercept}[1]{\\textcolor{fbintercept}{\\text{\\dashuline{$#1$}}}}")
        );
        assert!(
            t.contains("y = \\overset{\\text{\\scriptsize slope}}{\\fbslope{m}} x + \\underbrace{\\fbintercept{b}}_{\\text{intercept}}"),
            "{t}"
        );
    }

    #[test]
    fn nested_concepts_nest_their_macros() {
        let m = model(
            "w - \\nabla f(w_k)",
            "concept g: \\nabla f(w_k)\nconcept c: w_k\n",
        );
        let t = latex(&m, "h");
        assert!(t.contains("w - \\fbg{\\nabla f(\\fbc{w_k})}"), "{t}");
    }

    #[test]
    fn concepts_that_start_together_open_outer_first() {
        // both start at x: the longer run opens first and closes last
        let m = model("x + y = 1", "concept pair: x + y\nconcept first: x\n");
        let t = latex(&m, "h");
        assert!(t.contains("\\fbpair{\\fbfirst{x} + y} = 1"), "{t}");
    }

    #[test]
    fn a_fragment_round_trips_any_text() {
        let d = Doc {
            title: "Ünïcode ∑".into(),
            formulas: "y = m x + b".into(),
            sheet: "concept s: m".into(),
            prose: "a".into(),
        };
        let f = fragment(&d);
        assert!(f.starts_with("#f=") && f.is_ascii());
        assert_eq!(from_fragment(&f).unwrap(), d);
        assert_eq!(
            from_fragment("#x=1").unwrap_err(),
            "This link does not hold a formula."
        );
        assert!(from_fragment(&f[..f.len() - 7]).is_err());
    }
}
