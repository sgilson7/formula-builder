//! The parts a tap can select, smallest first. A row in the parse tree is
//! flat (`A = \frac{1}{2} b h` is five siblings), so a part larger than one
//! node is a run of siblings: a term (split at `+` and `-`), a side of an
//! equation (split at `=` and other relations), then the whole row. Standard
//! 6.EE.A.2b asks students to see such a part as a single thing; this is
//! where the tool finds them (R2.3).

use crate::tex::{Class, Node, K};

/// A run of `len` siblings starting at `first` in the row `row` of formula `f`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub f: usize,
    pub row: usize,
    pub first: usize,
    pub len: usize,
}

impl Run {
    /// The ids of the nodes the run covers, outermost only.
    pub fn nodes(&self, root: &Node) -> Vec<usize> {
        root.find(self.row)
            .and_then(|r| r.kids.get(self.first..self.first + self.len))
            .map(|kids| kids.iter().map(|k| k.id).collect())
            .unwrap_or_default()
    }

    /// The source text the run spans in its formula line.
    pub fn source<'a>(&self, root: &Node, line: &'a str) -> Option<&'a str> {
        let r = root.find(self.row)?;
        let a = r.kids.get(self.first)?.span.0;
        let b = r.kids.get(self.first + self.len - 1)?.span.1;
        line.get(a..b).map(str::trim)
    }

    fn reach(&self, root: &Node) -> Option<(usize, usize)> {
        let r = root.find(self.row)?;
        let kids = r.kids.get(self.first..self.first + self.len)?;
        Some((kids.first()?.id, kids.last()?.end))
    }
}

fn is_cp(n: &Node, cps: &[u32]) -> bool {
    matches!(n.k, K::Glyph { cp, .. } if cps.contains(&cp))
}

/// `+` and `-` (and the minus sign) split terms.
fn splits_terms(n: &Node) -> bool {
    is_cp(n, &[0x2B, 0x2D, 0x2212, 0xB1])
}

fn splits_sides(n: &Node) -> bool {
    n.class() == Class::Rel
}

/// The run of siblings around `i` bounded by nodes where `split` holds.
fn around(kids: &[Node], i: usize, split: fn(&Node) -> bool) -> (usize, usize) {
    let mut a = i;
    while a > 0 && !split(&kids[a - 1]) {
        a -= 1;
    }
    let mut b = i + 1;
    while b < kids.len() && !split(&kids[b]) {
        b += 1;
    }
    (a, b - a)
}

/// Every part a tap on node `id` can select, smallest first, with no two
/// covering the same nodes.
pub fn choices(root: &Node, f: usize, id: usize) -> Vec<Run> {
    let path = root.path_to(id);
    let mut out: Vec<Run> = Vec::new();
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for w in path.windows(2).rev() {
        let Some(row) = root.find(w[0]) else { continue };
        if matches!(row.k, K::Scripts { .. }) && row.kids.first().map(|b| b.id) == Some(w[1]) {
            // the base of a power on its own: `s` in `s^3` (as concepts match it)
            let run = Run {
                f,
                row: row.id,
                first: 0,
                len: 1,
            };
            if let Some(reach) = run.reach(root) {
                seen.push(reach);
                out.push(run);
            }
            continue;
        }
        if row.k != K::Row {
            continue;
        }
        let Some(i) = row.kids.iter().position(|k| k.id == w[1]) else {
            continue;
        };
        let kid = &row.kids[i];
        // a tap on an operator or relation selects nothing of its own
        let single = if splits_terms(kid) || splits_sides(kid) {
            None
        } else {
            Some((i, 1))
        };
        let term = (!splits_terms(kid) && !splits_sides(kid))
            .then(|| around(&row.kids, i, |n| splits_terms(n) || splits_sides(n)));
        let side = (!splits_sides(kid)).then(|| around(&row.kids, i, splits_sides));
        let whole = Some((0, row.kids.len()));
        for (first, len) in [single, term, side, whole].into_iter().flatten() {
            if len == 0 {
                continue;
            }
            let run = Run {
                f,
                row: row.id,
                first,
                len,
            };
            let Some(reach) = run.reach(root) else { continue };
            if !seen.contains(&reach) {
                seen.push(reach);
                out.push(run);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tex::parse;

    fn texts(src: &str, tap: usize) -> Vec<String> {
        let root = parse(src).unwrap();
        choices(&root, 0, tap)
            .iter()
            .map(|r| r.source(&root, src).unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_tap_grows_through_the_term_and_the_side_to_the_whole_line() {
        // y = m x + b: root 0, y 1, = 2, m 3, x 4, + 5, b 6
        assert_eq!(texts("y = m x + b", 3), ["m", "m x", "m x + b", "y = m x + b"]);
        // A = \frac{1}{2} b h: h is node 9 (frac 3, its rows and digits 4 to 7, b 8)
        assert_eq!(
            texts("A = \\frac{1}{2} b h", 9),
            ["h", "\\frac{1}{2} b h", "A = \\frac{1}{2} b h"]
        );
    }

    #[test]
    fn a_tap_inside_brackets_reaches_the_bracket_then_the_product_outside() {
        // 3(2 + x) = 6 + 3x, the distributive property of 6.EE.A.3
        let src = "3 \\left(2 + x\\right) = 6 + 3 x";
        let root = parse(src).unwrap();
        let mut all = Vec::new();
        root.walk(&mut all);
        let x = all
            .iter()
            .find(|n| matches!(n.k, K::Glyph { cp, .. } if cp == crate::tex::italic('x')))
            .unwrap()
            .id;
        let got: Vec<String> = choices(&root, 0, x)
            .iter()
            .map(|r| r.source(&root, src).unwrap().to_string())
            .collect();
        assert_eq!(
            got,
            [
                "x",
                "2 + x",
                "\\left(2 + x\\right)",
                "3 \\left(2 + x\\right)",
                src
            ]
        );
    }

    #[test]
    fn a_tap_on_the_base_of_a_power_selects_the_base_first() {
        // V = s^{3}: root 0, V 1, = 2, scripts 3, s 4, the 3's row 5, 3 6
        assert_eq!(texts("V = s^{3}", 4), ["s", "s^{3}", "V = s^{3}"]);
    }

    #[test]
    fn a_run_past_the_end_of_its_row_covers_nothing() {
        let root = parse("A = b").unwrap();
        let stale = Run {
            f: 0,
            row: 0,
            first: 2,
            len: 3,
        };
        assert!(stale.nodes(&root).is_empty());
        assert_eq!(stale.source(&root, "A = b"), None);
    }

    #[test]
    fn a_tap_on_a_sign_starts_at_the_side_it_belongs_to() {
        assert_eq!(texts("y = m x + b", 5), ["m x + b", "y = m x + b"]);
    }
}
