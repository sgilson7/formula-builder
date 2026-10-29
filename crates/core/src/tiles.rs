//! Writing a formula with tiles and keys instead of LaTeX (journal K1). A
//! line is a list of tiles: symbols, fractions with a top and a bottom,
//! powers, and brackets. A key goes in at the cursor only where it fits, so
//! no key sequence makes a line the parser refuses. The tiles are read from
//! the formula's LaTeX and written back to it after every key; the LaTeX
//! stays the saved form and is shown only in the Teacher mode.

use crate::render::esc;
use crate::tex::{canon, parse, Class, Node, K};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tile {
    /// one symbol, as its LaTeX spelling: `x`, `2`, `+`, `\times`
    Sym(String),
    /// top, bottom
    Frac(Vec<Tile>, Vec<Tile>),
    /// a power on the tile before it
    Pow(Vec<Tile>),
    Paren(Vec<Tile>),
}

/// Where the next key goes: a chain of (tile index, which list inside it),
/// then a position in the innermost list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cursor {
    pub path: Vec<(usize, usize)>,
    pub pos: usize,
}

impl Cursor {
    pub fn write(&self) -> String {
        let p: Vec<String> = self.path.iter().map(|(i, c)| format!("{i}.{c}")).collect();
        format!("{};{}", p.join(","), self.pos)
    }

    pub fn read(text: &str) -> Option<Cursor> {
        let (p, pos) = text.split_once(';')?;
        let mut path = Vec::new();
        for part in p.split(',').filter(|s| !s.is_empty()) {
            let (i, c) = part.split_once('.')?;
            path.push((i.parse().ok()?, c.parse().ok()?));
        }
        Some(Cursor {
            path,
            pos: pos.parse().ok()?,
        })
    }
}

/// The tiles of a parsed line, or `None` when the line holds something tiles
/// cannot show (a root, a subscript, a sum): such a line stays the teacher's.
pub fn from_line(src: &str) -> Option<Vec<Tile>> {
    let root = parse(src).ok()?;
    from_row(&root)
}

fn from_row(n: &Node) -> Option<Vec<Tile>> {
    let mut out = Vec::new();
    if n.k == K::Row {
        for k in &n.kids {
            out.extend(from_row(k)?);
        }
        return Some(out);
    }
    match &n.k {
        K::Glyph { .. } => out.push(Tile::Sym(canon(n))),
        K::Num(t) => out.extend(t.chars().map(|c| Tile::Sym(c.to_string()))),
        K::Frac { .. } => out.push(Tile::Frac(from_row(&n.kids[0])?, from_row(&n.kids[1])?)),
        K::Scripts {
            sub: false,
            sup: true,
        } => {
            out.extend(from_row(&n.kids[0])?);
            out.push(Tile::Pow(from_row(&n.kids[1])?));
        }
        K::Delim {
            left: 0x28,
            right: 0x29,
        } => out.push(Tile::Paren(from_row(&n.kids[0])?)),
        _ => return None,
    }
    Some(out)
}

fn is_digit(t: &Tile) -> bool {
    matches!(t, Tile::Sym(s) if s.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

/// The LaTeX for a list of tiles. Digits run together so `1` `2` reads as 12.
pub fn to_latex(tiles: &[Tile]) -> String {
    let mut out = String::new();
    for (i, t) in tiles.iter().enumerate() {
        let glue = i > 0 && !(is_digit(t) && is_digit(&tiles[i - 1])) && !matches!(t, Tile::Pow(_));
        if glue {
            out.push(' ');
        }
        match t {
            Tile::Sym(s) => out += s,
            Tile::Frac(a, b) => out += &format!("\\frac{{{}}}{{{}}}", to_latex(a), to_latex(b)),
            Tile::Pow(a) => out += &format!("^{{{}}}", to_latex(a)),
            Tile::Paren(a) => out += &format!("\\left({}\\right)", to_latex(a)),
        }
    }
    out
}

fn list_at<'a>(tiles: &'a mut Vec<Tile>, path: &[(usize, usize)]) -> Option<&'a mut Vec<Tile>> {
    let Some(((i, c), rest)) = path.split_first() else {
        return Some(tiles);
    };
    let inner = match (tiles.get_mut(*i)?, c) {
        (Tile::Frac(a, _), 0) => a,
        (Tile::Frac(_, b), 1) => b,
        (Tile::Pow(a), 0) | (Tile::Paren(a), 0) => a,
        _ => return None,
    };
    list_at(inner, rest)
}

fn class_of(sym: &str) -> Class {
    parse(sym)
        .ok()
        .and_then(|r| r.kids.first().map(Node::class))
        .unwrap_or(Class::Ord)
}

fn is_op(t: &Tile) -> bool {
    matches!(t, Tile::Sym(s) if matches!(class_of(s), Class::Bin | Class::Rel))
}

/// A key a student can press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key<'a> {
    /// a letter, a digit or a point
    Put(&'a str),
    /// `+`, `-`, `\times`, `\div`
    Op(&'a str),
    Equals,
    Frac,
    Pow,
    Paren,
    Left,
    Right,
    Back,
}

impl<'a> Key<'a> {
    /// What a typed key means: `*` is times, `/` makes a fraction, `^` a power.
    pub fn from_typed(k: &'a str) -> Option<Key<'a>> {
        Some(match k {
            "+" => Key::Op("+"),
            "-" => Key::Op("-"),
            "*" | "x*" => Key::Op("\\times"),
            "=" => Key::Equals,
            "/" => Key::Frac,
            "^" => Key::Pow,
            "(" | ")" => Key::Paren,
            "ArrowLeft" => Key::Left,
            "ArrowRight" => Key::Right,
            "Backspace" => Key::Back,
            "." => Key::Put("."),
            k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => Key::Put(k),
            _ => return None,
        })
    }
}

/// Applies a key to a line at the cursor. `None` when the key does not fit
/// there; the line and cursor are then unchanged.
pub fn press(line: &[Tile], cur: &Cursor, key: Key) -> Option<(Vec<Tile>, Cursor)> {
    let mut tiles = line.to_vec();
    let mut c = cur.clone();
    let top = c.path.is_empty();
    let has_equals = line.iter().any(|t| matches!(t, Tile::Sym(s) if s == "="));
    let list = list_at(&mut tiles, &c.path)?;
    if c.pos > list.len() {
        return None;
    }
    let before = if c.pos > 0 { list.get(c.pos - 1) } else { None };
    match key {
        Key::Put(s) => {
            if s == "." && !before.map(is_digit).unwrap_or(false) {
                return None;
            }
            if s == "." {
                // one point to a number: walk back over the digits
                let mut i = c.pos;
                while i > 0 && is_digit(&list[i - 1]) {
                    if list[i - 1] == Tile::Sym(".".into()) {
                        return None;
                    }
                    i -= 1;
                }
            }
            list.insert(c.pos, Tile::Sym(s.into()));
            c.pos += 1;
        }
        Key::Op(s) => {
            let after_op = before.map(is_op).unwrap_or(true);
            if after_op && s != "-" {
                return None;
            }
            if before
                .map(|b| matches!(b, Tile::Sym(x) if x == "-" || x == "+"))
                .unwrap_or(false)
            {
                return None;
            }
            list.insert(c.pos, Tile::Sym(s.into()));
            c.pos += 1;
        }
        Key::Equals => {
            if !top || has_equals || before.map(is_op).unwrap_or(true) {
                return None;
            }
            list.insert(c.pos, Tile::Sym("=".into()));
            c.pos += 1;
        }
        Key::Frac => {
            list.insert(c.pos, Tile::Frac(vec![], vec![]));
            c.path.push((c.pos, 0));
            c.pos = 0;
        }
        Key::Pow => {
            match before {
                Some(Tile::Sym(_)) | Some(Tile::Paren(_)) if !before.map(is_op).unwrap_or(false) => {}
                _ => return None,
            }
            if list
                .get(c.pos)
                .map(|t| matches!(t, Tile::Pow(_)))
                .unwrap_or(false)
            {
                return None;
            }
            list.insert(c.pos, Tile::Pow(vec![]));
            c.path.push((c.pos, 0));
            c.pos = 0;
        }
        Key::Paren => {
            list.insert(c.pos, Tile::Paren(vec![]));
            c.path.push((c.pos, 0));
            c.pos = 0;
        }
        Key::Left => return Some((tiles.clone(), step(&mut tiles, c, false)?)),
        Key::Right => return Some((tiles.clone(), step(&mut tiles, c, true)?)),
        Key::Back => {
            if c.pos > 0 {
                list.remove(c.pos - 1);
                c.pos -= 1;
            } else {
                // at the start of an empty box: remove the tile that holds it
                let (i, _) = *c.path.last()?;
                let empty = match list_at(&mut tiles, &c.path[..c.path.len() - 1])?.get(i)? {
                    Tile::Frac(a, b) => a.is_empty() && b.is_empty(),
                    Tile::Pow(a) | Tile::Paren(a) => a.is_empty(),
                    Tile::Sym(_) => false,
                };
                c.path.pop();
                c.pos = i;
                if empty {
                    list_at(&mut tiles, &c.path)?.remove(i);
                }
            }
        }
    }
    Some((tiles, c))
}

/// Moves the cursor one place, into and out of fractions, powers and brackets.
fn step(tiles: &mut Vec<Tile>, mut c: Cursor, right: bool) -> Option<Cursor> {
    let len = list_at(tiles, &c.path)?.len();
    if right && c.pos < len {
        let next = list_at(tiles, &c.path)?[c.pos].clone();
        if matches!(next, Tile::Sym(_)) {
            c.pos += 1;
        } else {
            c.path.push((c.pos, 0));
            c.pos = 0;
        }
        return Some(c);
    }
    if !right && c.pos > 0 {
        let prev = list_at(tiles, &c.path)?[c.pos - 1].clone();
        match prev {
            Tile::Sym(_) => c.pos -= 1,
            Tile::Frac(_, b) => {
                c.path.push((c.pos - 1, 1));
                c.pos = b.len();
            }
            Tile::Pow(a) | Tile::Paren(a) => {
                c.path.push((c.pos - 1, 0));
                c.pos = a.len();
            }
        }
        return Some(c);
    }
    // at an end of an inner list: a fraction's top leads to its bottom
    let (i, which) = c.path.pop()?;
    let holder = list_at(tiles, &c.path)?.get(i)?.clone();
    if let Tile::Frac(a, _) = &holder {
        if right && which == 0 {
            c.path.push((i, 1));
            c.pos = 0;
            return Some(c);
        }
        if !right && which == 1 {
            c.path.push((i, 0));
            c.pos = a.len();
            return Some(c);
        }
    }
    c.pos = if right { i + 1 } else { i };
    Some(c)
}

/// What a tile looks like on its key or in the line.
pub fn face(sym: &str) -> String {
    if sym
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '=' || c == '+')
    {
        return sym.into();
    }
    if sym == "-" {
        return "\u{2212}".into();
    }
    parse(sym)
        .ok()
        .and_then(|r| match r.kids.first().map(|k| &k.k) {
            Some(K::Glyph { cp, .. }) => char::from_u32(*cp).map(String::from),
            _ => None,
        })
        .unwrap_or_else(|| sym.into())
}

/// The line as HTML: each tile a span that puts the cursor after it when
/// tapped, each empty box a span that puts the cursor in it, and the cursor.
pub fn html(tiles: &[Tile], line: usize, cur: Option<&Cursor>, label: &str) -> String {
    let mut out = String::new();
    list_html(tiles, line, &[], cur, &mut out, label);
    out
}

fn at(line: usize, path: &[(usize, usize)], pos: usize) -> String {
    format!(
        "{line}|{}",
        Cursor {
            path: path.to_vec(),
            pos
        }
        .write()
    )
}

fn list_html(
    tiles: &[Tile],
    line: usize,
    path: &[(usize, usize)],
    cur: Option<&Cursor>,
    out: &mut String,
    label: &str,
) {
    let here = cur.filter(|c| c.path == path);
    let caret = r#"<span class="caret" aria-hidden="true"></span>"#;
    if tiles.is_empty() {
        out.push_str(&format!(
            r#"<span class="tslot" data-at="{}" role="button" aria-label="{}">{}</span>"#,
            at(line, path, 0),
            esc(label),
            if here.is_some() { caret } else { "" }
        ));
        return;
    }
    out.push_str(&format!(
        r#"<span class="tstart" data-at="{}"></span>"#,
        at(line, path, 0)
    ));
    if here.map(|c| c.pos == 0).unwrap_or(false) {
        out.push_str(caret);
    }
    for (i, t) in tiles.iter().enumerate() {
        let inner = |which: usize, list: &[Tile], out: &mut String| {
            let mut p = path.to_vec();
            p.push((i, which));
            list_html(list, line, &p, cur, out, label);
        };
        match t {
            Tile::Sym(s) => {
                let cls = if s.chars().all(|c| c.is_ascii_alphabetic()) {
                    "tile letter"
                } else {
                    "tile"
                };
                out.push_str(&format!(
                    r#"<span class="{cls}" data-at="{}">{}</span>"#,
                    at(line, path, i + 1),
                    esc(&face(s))
                ));
            }
            Tile::Frac(a, b) => {
                out.push_str(r#"<span class="tfrac"><span class="ttop">"#);
                inner(0, a, out);
                out.push_str(r#"</span><span class="tbottom">"#);
                inner(1, b, out);
                out.push_str("</span></span>");
            }
            Tile::Pow(a) => {
                out.push_str(r#"<span class="tpow">"#);
                inner(0, a, out);
                out.push_str("</span>");
            }
            Tile::Paren(a) => {
                out.push_str(&format!(
                    r#"<span class="tparen"><span class="tbr" data-at="{}">(</span>"#,
                    at(line, path, i)
                ));
                inner(0, a, out);
                out.push_str(&format!(
                    r#"<span class="tbr" data-at="{}">)</span></span>"#,
                    at(line, path, i + 1)
                ));
            }
        }
        if here.map(|c| c.pos == i + 1).unwrap_or(false) {
            out.push_str(caret);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &[Key]) -> (Vec<Tile>, Cursor) {
        let mut line = vec![];
        let mut c = Cursor::default();
        for k in keys {
            if let Some((l, n)) = press(&line, &c, *k) {
                line = l;
                c = n;
            }
        }
        (line, c)
    }

    #[test]
    fn keys_build_the_triangle_area_formula() {
        use Key::*;
        let (line, _) = typed(&[
            Put("A"),
            Equals,
            Frac,
            Put("1"),
            Right,
            Put("2"),
            Right,
            Put("b"),
            Put("h"),
        ]);
        assert_eq!(to_latex(&line), "A = \\frac{1}{2} b h");
        assert!(parse(&to_latex(&line)).is_ok());
    }

    #[test]
    fn a_key_that_does_not_fit_is_refused() {
        use Key::*;
        let empty = Cursor::default();
        assert!(press(&[], &empty, Equals).is_none(), "a line cannot start with =");
        assert!(
            press(&[], &empty, Pow).is_none(),
            "a power needs something under it"
        );
        assert!(
            press(&[], &empty, Op("+")).is_none(),
            "a line cannot start with +"
        );
        assert!(
            press(&[], &empty, Op("-")).is_some(),
            "a line can start with a minus"
        );
        let (line, c) = typed(&[Put("y"), Equals, Put("2")]);
        assert!(press(&line, &c, Equals).is_none(), "one = to a line");
        let (line, c) = typed(&[Put("2"), Op("+")]);
        assert!(
            press(&line, &c, Op("\\times")).is_none(),
            "two signs cannot sit together"
        );
        let (line, c) = typed(&[Put("1"), Put("."), Put("5")]);
        assert!(press(&line, &c, Put(".")).is_none(), "one point to a number");
        let (line, c) = typed(&[Frac]);
        assert!(
            press(&line, &c, Equals).is_none(),
            "= goes on the line, not inside a fraction"
        );
    }

    #[test]
    fn digits_typed_apart_make_one_number() {
        use Key::*;
        let (line, _) = typed(&[Put("V"), Equals, Put("1"), Put("2"), Put("s")]);
        assert_eq!(to_latex(&line), "V = 12 s");
    }

    #[test]
    fn every_line_the_shelf_needs_round_trips_through_tiles() {
        for src in [
            "A = \\ell \\times w",
            "A = \\frac{1}{2} b h",
            "V = s^{3}",
            "A = 6 s^{2}",
            "d = r \\times t",
            "3 \\left(2 + x\\right) = 6 + 3 x",
            "3 \\times 2 + 3 \\times x",
        ] {
            let tiles = from_line(src).unwrap_or_else(|| panic!("no tiles for {src}"));
            assert_eq!(to_latex(&tiles), src);
        }
        assert!(
            from_line("c = \\sqrt{a^2 + b^2}").is_none(),
            "a root stays the teacher's"
        );
        assert!(
            from_line("w_{k+1} = w_k").is_none(),
            "a subscript stays the teacher's"
        );
    }

    #[test]
    fn arrows_go_into_a_fraction_top_then_bottom_then_out() {
        use Key::*;
        let (line, _) = typed(&[Put("a"), Frac, Put("1"), Right, Put("2"), Right, Right]);
        // a, frac(1, 2): the cursor ends after the fraction, at the end of the line
        let mut c = Cursor::default();
        let mut seen = vec![];
        for _ in 0..6 {
            c = press(&line, &c, Right).map(|(_, n)| n).unwrap_or(c);
            seen.push(c.write());
        }
        assert_eq!(seen, [";1", "1.0;0", "1.0;1", "1.1;0", "1.1;1", ";2"]);
        let back = press(&line, &Cursor { path: vec![], pos: 2 }, Left).unwrap().1;
        assert_eq!(back.write(), "1.1;1", "left from the end goes into the bottom");
    }

    #[test]
    fn backspace_removes_the_tile_before_and_an_empty_box_with_its_holder() {
        use Key::*;
        let (line, c) = typed(&[Put("a"), Pow]);
        let (line, c) = press(&line, &c, Back).unwrap();
        assert_eq!(to_latex(&line), "a");
        assert_eq!(c.write(), ";1");
        let (line, _) = press(&line, &c, Back).unwrap();
        assert!(line.is_empty());
    }

    #[test]
    fn a_half_typed_line_still_parses() {
        use Key::*;
        for keys in [
            vec![Frac],
            vec![Put("a"), Pow],
            vec![Paren],
            vec![Put("y"), Equals],
        ] {
            let (line, _) = typed(&keys);
            let src = to_latex(&line);
            assert!(parse(&src).is_ok(), "{src} does not parse");
        }
    }

    #[test]
    fn typed_keys_mean_what_a_calculator_means() {
        assert_eq!(Key::from_typed("*"), Some(Key::Op("\\times")));
        assert_eq!(Key::from_typed("/"), Some(Key::Frac));
        assert_eq!(Key::from_typed("Tab"), None);
    }
}
