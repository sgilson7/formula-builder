//! The worked line: a formula that defines a name, written again with the
//! reader's numbers in place of the names that have values, then its result.
//! `m = \frac{a + b + c}{3}` with sliders at 4, 7 and 10 works out as
//! `m = \frac{4 + 7 + 10}{3} = 7`. The line is new LaTeX built from the
//! formula's own source, so it lays out like any formula. Two factors that
//! sat side by side as letters (`a d`) would run together as digits (`3 2`),
//! so a times sign goes between them.

use crate::doc::Model;
use crate::render::value_text;
use crate::tex::{canon, parse, Class, Node, K};
use crate::value::Fx;

/// A worked line and, for each number put into it, the bytes it spans in
/// `source` and the concept whose value it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worked {
    pub source: String,
    pub root: Node,
    pub marks: Vec<(usize, usize, Option<usize>)>,
}

impl Worked {
    /// The concept whose number the node belongs to, if any.
    pub fn concept_of(&self, node: usize) -> Option<usize> {
        let span = self.root.find(node)?.span;
        self.marks
            .iter()
            .find(|(a, b, _)| *a <= span.0 && span.1 <= *b)
            .and_then(|(_, _, c)| *c)
    }
}

/// A value as the worked line writes it: three decimals at most, trailing
/// zeros dropped, and whether that is the value exactly.
pub fn number(v: Fx) -> (String, bool) {
    let t = v.format(3);
    let t = if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        t
    };
    let exact = Fx::parse(&t) == Some(v);
    (t, exact)
}

struct Ctx<'a> {
    m: &'a Model,
    values: &'a [(String, Fx)],
    scope: crate::eval::Env<'a>,
    own: String,
    line: &'a str,
}

impl Ctx<'_> {
    /// The number a name stands for, and its concept.
    fn valued(&self, name: &str) -> Option<(String, Option<usize>)> {
        let concept = self.m.sheet.concepts.iter().position(|c| c.value_name() == name);
        let v = match self.values.iter().find(|(n, _)| n == name) {
            Some((_, v)) => {
                let step = concept
                    .and_then(|c| self.m.sheet.concepts[c].control.as_ref())
                    .map(|k| k.step)
                    .or_else(|| {
                        self.m
                            .sheet
                            .hidden
                            .iter()
                            .find(|h| h.name == name)
                            .map(|h| h.control.step)
                    });
                let text = match step {
                    Some(s) => value_text(*v, s),
                    None => number(*v).0,
                };
                return Some((bracket(text, *v), concept));
            }
            None if name != self.own && self.scope.defs.iter().any(|(n, _)| n == name) => {
                self.scope.value_of(name, &[]).ok()?
            }
            None => return None,
        };
        Some((bracket(number(v).0, v), concept))
    }

    /// The number that replaces this node, if it is a name with a value.
    fn replace(&self, n: &Node) -> Option<(String, Option<usize>)> {
        match &n.k {
            K::Glyph { cp, .. } if crate::eval::is_name(*cp) => self.valued(&canon(n)),
            K::Scripts {
                sub: true,
                sup: false,
            } => self.valued(&canon(n)),
            _ => None,
        }
    }

    fn starts_number(&self, n: &Node) -> bool {
        match &n.k {
            K::Num(_) => true,
            K::Scripts { sub: false, .. } => self.replace(&n.kids[0]).is_some(),
            _ => self.replace(n).is_some(),
        }
    }

    fn ends_number(&self, n: &Node) -> bool {
        match &n.k {
            K::Num(_) => true,
            K::Scripts { sub: false, .. } => self.replace(&n.kids[0]).is_some(),
            _ => self.replace(n).is_some(),
        }
    }

    /// Collects replacements (start, end, text, concept) and times signs
    /// (position) in a row of siblings.
    fn row(&self, kids: &[Node], out: &mut Vec<(usize, usize, String, Option<usize>)>) {
        for (i, k) in kids.iter().enumerate() {
            if i > 0 {
                let a = &kids[i - 1];
                let touching =
                    (self.starts_number(k) && factor_end(a)) || (self.ends_number(a) && factor_start(k));
                let replaced = self.replace(a).is_some()
                    || self.replace(k).is_some()
                    || self.starts_number(k) && !matches!(k.k, K::Num(_))
                    || self.ends_number(a) && !matches!(a.k, K::Num(_));
                if touching && replaced {
                    out.push((k.span.0, k.span.0, "\\times ".into(), None));
                }
            }
            self.node(k, out);
        }
    }

    fn node(&self, n: &Node, out: &mut Vec<(usize, usize, String, Option<usize>)>) {
        if let Some((text, c)) = self.replace(n) {
            // a command's span takes the space after it; keep that space
            let end = self
                .line
                .get(n.span.0..n.span.1)
                .map(|t| n.span.0 + t.trim_end().len())
                .unwrap_or(n.span.1);
            out.push((n.span.0, end, text, c));
            return;
        }
        for k in &n.kids {
            if k.k == K::Row {
                self.row(&k.kids, out);
            } else {
                self.node(k, out);
            }
        }
    }
}

/// A negative number in brackets, so `3 \times (-2)` reads right.
fn bracket(text: String, v: Fx) -> String {
    if v < Fx::ZERO {
        format!("({text})")
    } else {
        text
    }
}

fn factor_end(n: &Node) -> bool {
    !matches!(n.k, K::Space(_))
        && !matches!(
            n.class(),
            Class::Bin | Class::Rel | Class::Open | Class::Punct | Class::Op
        )
}

fn factor_start(n: &Node) -> bool {
    !matches!(n.k, K::Space(_)) && !matches!(n.class(), Class::Bin | Class::Rel | Class::Close | Class::Punct)
}

/// The worked line for formula `fi`, when it defines one name, every name on
/// its right side has a value, and at least one number goes in.
pub fn worked(m: &Model, fi: usize, values: &[(String, Fx)]) -> Option<Worked> {
    let root = m.roots.get(fi)?.as_ref()?;
    let line = m.lines.get(fi)?;
    let kids = &root.kids;
    let eq = kids
        .iter()
        .position(|k| matches!(k.k, K::Glyph { cp: 0x3D, .. }))?;
    if eq != 1 || kids[eq + 1..].iter().any(|k| k.class() == Class::Rel) || eq + 1 >= kids.len() {
        return None;
    }
    let own = canon(&kids[0]);
    let scope = m.scope(values);
    let result = scope.value_of(&own, &[]).ok()?;
    let cx = Ctx {
        m,
        values,
        scope,
        own: own.clone(),
        line,
    };
    let mut edits = Vec::new();
    cx.row(&kids[eq + 1..], &mut edits);
    if !edits.iter().any(|e| !e.2.starts_with("\\times")) {
        return None;
    }
    edits.sort_by_key(|e| (e.0, e.1));
    let mut source = String::new();
    let mut marks = Vec::new();
    let mut at = 0;
    for (a, b, text, c) in edits {
        source += line.get(at..a)?;
        let start = source.len();
        source += &text;
        if !text.starts_with("\\times") {
            marks.push((start, source.len(), c));
        }
        at = b;
    }
    source += line.get(at..)?;
    let (t, exact) = number(result);
    source += if exact { " = " } else { " \\approx " };
    let start = source.len();
    source += &t;
    let lhs_concept = m
        .sheet
        .concepts
        .iter()
        .position(|c| c.value_name() == own)
        .or_else(|| {
            m.occs
                .iter()
                .filter(|o| o.formula == fi && o.lo <= kids[0].id && kids[0].id <= o.hi)
                .min_by_key(|o| o.depth)
                .map(|o| o.concept)
        });
    marks.push((start, source.len(), lhs_concept));
    let root = parse(&source).ok()?;
    Some(Worked { source, root, marks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    fn line(formulas: &str, sheet: &str, fi: usize) -> Option<String> {
        let doc = Doc {
            title: String::new(),
            formulas: formulas.into(),
            sheet: sheet.into(),
            prose: String::new(),
            ..Doc::default()
        };
        let m = Model::build(&doc);
        assert!(m.errors.is_empty(), "{:?}", m.errors);
        let values = m.values(&[], None);
        worked(&m, fi, &values).map(|w| w.source)
    }

    #[test]
    fn the_mean_works_out_with_the_reader_numbers_in_place() {
        let sheet = "concept first: a\n  slider 0 .. 20 step 1 = 4\nconcept second: b\n  slider 0 .. 20 step 1 = 7\nconcept third: c\n  slider 0 .. 20 step 1 = 10\n";
        assert_eq!(
            line("m = \\frac{a + b + c}{3}", sheet, 0).unwrap(),
            "m = \\frac{4 + 7 + 10}{3} = 7"
        );
        // 6 + 7 + 10 = 23, and 23 / 3 is 7.666..., shown to three places
        let sheet6 = sheet.replace("= 4\n", "= 6\n");
        assert_eq!(
            line("m = \\frac{a + b + c}{3}", &sheet6, 0).unwrap(),
            "m = \\frac{6 + 7 + 10}{3} \\approx 7.667"
        );
    }

    #[test]
    fn letters_side_by_side_get_a_times_sign_and_a_negative_number_brackets() {
        let sheet = "hidden a 0 .. 5 step 1 = 3 a\nhidden b 0 .. 5 step 1 = 1 b\nhidden c 0 .. 5 step 1 = 1 c\nhidden d 0 .. 5 step 1 = 2 d\n";
        assert_eq!(
            line("D = a d - b c", sheet, 0).unwrap(),
            "D = 3 \\times 2 - 1 \\times 1 = 5"
        );
        let sheet =
            "concept l: \\ell\n  slider 1 .. 10 step 1 = 5\nconcept w: w\n  slider 1 .. 10 step 1 = 2\n";
        assert_eq!(
            line("P = 2 \\ell + 2 w", sheet, 0).unwrap(),
            "P = 2 \\times 5 + 2 \\times 2 = 14"
        );
        let sheet = "concept n: n\n  slider -5 .. 5 step 1 = -2\n";
        assert_eq!(
            line("y = 3 n(n + 1)", sheet, 0).unwrap(),
            "y = 3 \\times (-2)\\times ((-2) + 1) = 6"
        );
    }

    #[test]
    fn a_name_defined_by_another_formula_goes_in_as_its_value() {
        let sheet = "concept big: \\lambda_n\n  slider 1 .. 100 step 1 = 50\nconcept small: \\lambda_1\n  slider 1 .. 10 step 1 = 2\n";
        let f = "\\kappa = \\frac{\\lambda_n}{\\lambda_1}\nq = 2 \\kappa";
        assert_eq!(line(f, sheet, 0).unwrap(), "\\kappa = \\frac{50}{2} = 25");
        assert_eq!(line(f, sheet, 1).unwrap(), "q = 2 \\times 25 = 50");
    }

    #[test]
    fn no_worked_line_without_a_value_for_every_name_or_for_a_function() {
        let sheet = "concept slope: m\n  slider -3 .. 3 step 1 = 2\n";
        assert_eq!(line("y = m x + 1", sheet, 0), None, "x has no value");
        assert_eq!(line("f(x) = m x", sheet, 0), None, "a function is not worked out");
        assert_eq!(line("y = 2 + 3", "", 0), None, "no number goes in");
    }
}
