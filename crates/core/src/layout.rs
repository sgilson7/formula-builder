//! TeX-style layout in integer font units (1000 to the em at the formula's
//! size, y up, the baseline at 0). The spacing rules follow TeX's table of
//! atom classes; the sizes and shifts come from the font's MATH table. Every
//! glyph and rule remembers the node that produced it, and every node gets a
//! box, so concepts can colour, underline and hit-test any part.

use crate::font::{font, key_of, Glyph};
use crate::tex::{Acc, Class, Node, K};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum It {
    Glyph {
        key: String,
        x: i32,
        y: i32,
        scale: i32,
    },
    /// y is the bottom edge
    Rule {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// a curly brace over or under a span; y is the edge nearest the body
    Brace {
        x: i32,
        y: i32,
        w: i32,
        below: bool,
        scale: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub it: It,
    pub node: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeBox {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub d: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Laid {
    pub w: i32,
    pub h: i32,
    pub d: i32,
    pub ital: i32,
    pub items: Vec<Item>,
    pub boxes: Vec<NodeBox>,
}

impl Laid {
    fn place(&mut self, child: Laid, dx: i32, dy: i32) {
        for mut it in child.items {
            match &mut it.it {
                It::Glyph { x, y, .. } | It::Rule { x, y, .. } | It::Brace { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
            }
            self.items.push(it);
        }
        for mut b in child.boxes {
            b.x += dx;
            b.y += dy;
            self.boxes.push(b);
        }
    }

    pub fn find(&self, id: usize) -> Option<NodeBox> {
        self.boxes.iter().find(|b| b.id == id).copied()
    }
}

#[derive(Debug, Clone, Copy)]
struct Style {
    level: u8,
    display: bool,
}

fn scale_of(level: u8) -> i32 {
    match level {
        0 => 1000,
        1 => font().k("ScriptPercentScaleDown") * 10,
        _ => font().k("ScriptScriptPercentScaleDown") * 10,
    }
}

fn sc(v: i32, s: i32) -> i32 {
    v * s / 1000
}

fn k(name: &str, s: i32) -> i32 {
    sc(font().k(name), s)
}

fn glyph_for(cp: u32) -> &'static Glyph {
    font()
        .cp(cp)
        .or_else(|| font().cp(0x3F))
        .expect("the font has a question mark")
}

fn one_glyph(g: &Glyph, s: i32, node: usize) -> Laid {
    Laid {
        w: sc(g.adv, s),
        h: sc(g.y1.max(0), s),
        d: sc((-g.y0).max(0), s),
        ital: sc(g.ital, s),
        items: vec![Item {
            it: It::Glyph {
                key: g.key.clone(),
                x: 0,
                y: 0,
                scale: s,
            },
            node,
        }],
        boxes: vec![],
    }
}

fn text_run(text: &str, s: i32, node: usize, upright: bool) -> Laid {
    let mut out = Laid::default();
    for c in text.chars() {
        let cp = match c {
            '-' => 0x2212,
            c if !upright && c.is_ascii_alphabetic() => crate::tex::italic(c),
            c => c as u32,
        };
        let g = glyph_for(cp);
        let one = one_glyph(g, s, node);
        let x = out.w;
        out.h = out.h.max(one.h);
        out.d = out.d.max(one.d);
        out.w += one.w;
        out.ital = one.ital;
        out.place(one, x, 0);
    }
    out
}

/// TeX's inter-atom spacing, in thousandths of an em: 0, thin, medium, thick.
fn space(left: Class, right: Class, level: u8) -> i32 {
    use Class::*;
    let (thin, med, thick) = (167, 222, 278);
    let v = match (left, right) {
        (Ord, Op) | (Ord, Inner) | (Op, Ord) | (Op, Op) | (Op, Inner) => thin,
        (Close, Op) | (Close, Inner) | (Inner, Ord) | (Inner, Op) | (Inner, Open) | (Inner, Inner) => thin,
        (Inner, Punct) | (Punct, _) => thin,
        (Ord, Bin) | (Bin, Ord) | (Bin, Op) | (Bin, Open) | (Bin, Inner) | (Close, Bin) | (Inner, Bin) => med,
        (Ord, Rel)
        | (Op, Rel)
        | (Rel, Ord)
        | (Rel, Op)
        | (Rel, Open)
        | (Rel, Inner)
        | (Close, Rel)
        | (Inner, Rel) => thick,
        _ => 0,
    };
    if level > 0 && v != thin {
        0
    } else {
        v
    }
}

/// Extra room, in thousandths of an em, that the `space` augmentation puts
/// around a concept.
pub const SPACE_PAD: i32 = 330;
/// A `subst` entry with this text puts `SPACE_PAD` before the node.
pub const PAD_BEFORE: &str = "\u{1}<";
/// A `subst` entry with this text puts `SPACE_PAD` after the node.
pub const PAD_AFTER: &str = "\u{1}>";

/// Lays out one formula in display style. `subst` replaces a node with text
/// (a scrubbed value shown in place of its symbol), or with `PAD_BEFORE` and
/// `PAD_AFTER` asks for room around it (the paper's space augmentation).
pub fn layout(root: &Node, subst: &[(usize, String)]) -> Laid {
    lay(
        root,
        Style {
            level: 0,
            display: true,
        },
        subst,
    )
}

fn lay(n: &Node, st: Style, subst: &[(usize, String)]) -> Laid {
    let s = scale_of(st.level);
    let mut out = if let Some((_, text)) = subst
        .iter()
        .find(|(id, t)| *id == n.id && !t.starts_with('\u{1}'))
    {
        text_run(text, s, n.id, true)
    } else {
        lay_kind(n, st, s, subst)
    };
    out.boxes.push(NodeBox {
        id: n.id,
        x: 0,
        y: 0,
        w: out.w,
        h: out.h,
        d: out.d,
    });
    out
}

fn lay_kind(n: &Node, st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    match &n.k {
        K::Glyph { cp, .. } => one_glyph(glyph_for(*cp), s, n.id),
        K::Num(t) => text_run(t, s, n.id, true),
        K::Word { text, .. } => text_run(text, s, n.id, true),
        K::Space(w) => Laid {
            w: sc(*w, s),
            ..Laid::default()
        },
        K::Row => row(&n.kids, st, s, subst),
        K::Frac { display } => frac(n, display.unwrap_or(st.display), st, s, subst),
        K::Scripts { sub, sup } => scripts(n, *sub, *sup, st, s, subst),
        K::Sqrt => sqrt(n, st, s, subst),
        K::BigOp { cp, .. } => big_op(*cp, st, s, n.id),
        K::Delim { left, right } => delim(n, *left, *right, st, s, subst),
        K::Accent(a) => accent(n, *a, st, s, subst),
        K::Brace { below } => {
            let body = lay(&n.kids[0], st, subst);
            let mut out = Laid {
                w: body.w,
                h: body.h,
                d: body.d,
                ..Laid::default()
            };
            let gap = sc(60, s);
            let tall = sc(220, s);
            let y = if *below { -body.d - gap } else { body.h + gap };
            out.place(body, 0, 0);
            out.items.push(Item {
                it: It::Brace {
                    x: 0,
                    y,
                    w: out.w,
                    below: *below,
                    scale: s,
                },
                node: n.id,
            });
            if *below {
                out.d += gap + tall;
            } else {
                out.h += gap + tall;
            }
            out
        }
    }
}

fn row(kids: &[Node], st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    let mut classes: Vec<Option<Class>> = kids
        .iter()
        .map(|k| {
            if matches!(k.k, K::Space(_)) {
                None
            } else {
                Some(k.class())
            }
        })
        .collect();
    // A binary operator at the start, or after another operator, is unary (Ord).
    for i in 0..classes.len() {
        if classes[i] == Some(Class::Bin) {
            let prev = classes[..i].iter().rev().flatten().next().copied();
            let next = classes[i + 1..].iter().flatten().next().copied();
            let unary_before = matches!(
                prev,
                None | Some(Class::Bin | Class::Op | Class::Rel | Class::Open | Class::Punct)
            );
            let unary_after = matches!(next, None | Some(Class::Rel | Class::Close | Class::Punct));
            if unary_before || unary_after {
                classes[i] = Some(Class::Ord);
            }
        }
    }
    let mut out = Laid::default();
    let mut prev: Option<Class> = None;
    for (kid, class) in kids.iter().zip(classes) {
        if let (Some(p), Some(c)) = (prev, class) {
            out.w += sc(space(p, c, st.level), s);
        }
        let pad = |mark: &str| subst.iter().any(|(id, t)| *id == kid.id && t == mark);
        if pad(PAD_BEFORE) {
            out.w += sc(SPACE_PAD, s);
        }
        let one = lay(kid, st, subst);
        let x = out.w;
        out.h = out.h.max(one.h);
        out.d = out.d.max(one.d);
        out.w += one.w;
        out.ital = one.ital;
        out.place(one, x, 0);
        if pad(PAD_AFTER) {
            out.w += sc(SPACE_PAD, s);
        }
        if class.is_some() {
            prev = class;
        }
    }
    out
}

fn smaller(st: Style) -> Style {
    Style {
        level: (st.level + 1).min(2),
        display: false,
    }
}

fn frac(n: &Node, display: bool, st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    let inner = if display {
        Style {
            level: st.level,
            display: false,
        }
    } else {
        smaller(st)
    };
    let num = lay(&n.kids[0], inner, subst);
    let den = lay(&n.kids[1], inner, subst);
    let t = k("FractionRuleThickness", s);
    let a = k("AxisHeight", s);
    let (up, down, gn, gd) = if display {
        (
            k("FractionNumeratorDisplayStyleShiftUp", s),
            k("FractionDenominatorDisplayStyleShiftDown", s),
            k("FractionNumDisplayStyleGapMin", s),
            k("FractionDenomDisplayStyleGapMin", s),
        )
    } else {
        (
            k("FractionNumeratorShiftUp", s),
            k("FractionDenominatorShiftDown", s),
            k("FractionNumeratorGapMin", s),
            k("FractionDenominatorGapMin", s),
        )
    };
    let u = up.max(a + t / 2 + gn + num.d);
    let v = down.max(den.h + gd - (a - t / 2));
    let pad = sc(110, s);
    let w = num.w.max(den.w) + 2 * pad;
    let mut out = Laid {
        w,
        h: u + num.h,
        d: v + den.d,
        ..Laid::default()
    };
    let (nh, nw, dw) = (num.h, num.w, den.w);
    let _ = nh;
    out.place(num, (w - nw) / 2, u);
    out.place(den, (w - dw) / 2, -v);
    out.items.push(Item {
        it: It::Rule {
            x: pad / 2,
            y: a - t / 2,
            w: w - pad,
            h: t,
        },
        node: n.id,
    });
    out
}

fn limits(base: Laid, sub: Option<Laid>, sup: Option<Laid>, s: i32) -> Laid {
    let w = base
        .w
        .max(sub.as_ref().map(|l| l.w).unwrap_or(0))
        .max(sup.as_ref().map(|l| l.w).unwrap_or(0));
    let mut out = Laid {
        w,
        h: base.h,
        d: base.d,
        ..Laid::default()
    };
    let (bh, bd, bw) = (base.h, base.d, base.w);
    out.place(base, (w - bw) / 2, 0);
    if let Some(up) = sup {
        let y = bh + k("UpperLimitGapMin", s) + up.d;
        out.h = y + up.h;
        let uw = up.w;
        out.place(up, (w - uw) / 2, y);
    }
    if let Some(dn) = sub {
        let y = bd + k("LowerLimitGapMin", s) + dn.h;
        out.d = y + dn.d;
        let dw = dn.w;
        out.place(dn, (w - dw) / 2, -y);
    }
    out
}

fn scripts(n: &Node, has_sub: bool, has_sup: bool, st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    let base_node = &n.kids[0];
    let base = lay(base_node, st, subst);
    let small = smaller(st);
    let sub = if has_sub {
        Some(lay(&n.kids[1], small, subst))
    } else {
        None
    };
    let sup = if has_sup {
        Some(lay(n.kids.last().expect("a superscript"), small, subst))
    } else {
        None
    };
    let wants_limits = match &base_node.k {
        K::BigOp { limits, .. } => *limits && st.display,
        K::Word { limits, .. } => *limits && st.display,
        _ => false,
    };
    if wants_limits {
        return limits(base, sub, sup, s);
    }
    let single = matches!(base_node.k, K::Glyph { .. } | K::Num(_) | K::Word { .. });
    let drop = if single { 0 } else { sc(250, s) };
    let mut out = Laid {
        w: base.w,
        h: base.h,
        d: base.d,
        ..Laid::default()
    };
    let (bw, bh, bd, ital) = (base.w, base.h, base.d, base.ital);
    let is_integral = matches!(base_node.k, K::BigOp { limits: false, .. });
    out.place(base, 0, 0);
    let mut u = 0;
    let mut v = 0;
    if let Some(up) = &sup {
        let from_base = if single { 0 } else { bh - drop };
        u = k("SuperscriptShiftUp", s)
            .max(from_base)
            .max(up.d + k("SuperscriptBottomMin", s));
    }
    if let Some(dn) = &sub {
        v = k("SubscriptShiftDown", s).max(dn.h - k("SubscriptTopMax", s));
        if !single {
            v = v.max(bd + sc(80, s));
        }
    }
    if let (Some(up), Some(dn)) = (&sup, &sub) {
        let gap = (u - up.d) - (dn.h - v);
        let need = k("SubSuperscriptGapMin", s);
        if gap < need {
            v += need - gap;
        }
    }
    let after = k("SpaceAfterScript", s);
    let mut w = bw;
    if let Some(dn) = sub {
        let x = if is_integral { bw - ital } else { bw };
        out.d = out.d.max(v + dn.d);
        w = w.max(x + dn.w);
        out.place(dn, x, -v);
    }
    if let Some(up) = sup {
        let x = bw + if is_integral { 0 } else { ital };
        out.h = out.h.max(u + up.h);
        w = w.max(x + up.w);
        out.place(up, x, u);
    }
    out.w = w + after;
    out
}

fn pick_variant(cp: u32, need: i32, s: i32) -> &'static Glyph {
    let vs = font().variants(cp);
    vs.iter()
        .find(|g| sc(g.y1 - g.y0, s) >= need)
        .or(vs.last())
        .copied()
        .unwrap_or_else(|| glyph_for(cp))
}

fn sqrt(n: &Node, st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    let body = lay(
        &n.kids[0],
        Style {
            level: st.level,
            display: false,
        },
        subst,
    );
    let t = k("RadicalRuleThickness", s);
    let gap = if st.display {
        k("RadicalDisplayStyleVerticalGap", s)
    } else {
        k("RadicalVerticalGap", s)
    };
    let need = body.h + body.d + gap + t;
    let rad = pick_variant(0x221A, need, s);
    let top = body.h + gap + t;
    let ry = top - sc(rad.y1, s);
    let mut out = Laid::default();
    let mut x0 = 0;
    if let Some(ix) = n.kids.get(1) {
        let index = lay(
            ix,
            Style {
                level: 2,
                display: false,
            },
            subst,
        );
        let raise = (top * 6) / 10;
        let iw = index.w;
        out.h = out.h.max(raise + index.h);
        out.place(index, 0, raise);
        x0 = (iw - sc(260, s)).max(0);
    }
    let radw = sc(rad.adv, s);
    out.items.push(Item {
        it: It::Glyph {
            key: rad.key.clone(),
            x: x0,
            y: ry,
            scale: s,
        },
        node: n.id,
    });
    let bw = body.w;
    let bd = body.d;
    out.place(body, x0 + radw, 0);
    out.items.push(Item {
        it: It::Rule {
            x: x0 + radw - sc(10, s),
            y: top - t,
            w: bw + sc(30, s),
            h: t,
        },
        node: n.id,
    });
    out.w = x0 + radw + bw + sc(60, s);
    out.h = out.h.max(top + k("RadicalExtraAscender", s));
    out.d = bd.max(-(ry + sc(rad.y0, s)));
    out
}

fn big_op(cp: u32, st: Style, s: i32, node: usize) -> Laid {
    let vs = font().variants(cp);
    let g = if st.display {
        vs.get(1).copied().unwrap_or_else(|| glyph_for(cp))
    } else {
        glyph_for(cp)
    };
    let a = k("AxisHeight", s);
    let y = a - sc(g.y1 + g.y0, s) / 2;
    Laid {
        w: sc(g.adv, s),
        h: y + sc(g.y1, s),
        d: -(y + sc(g.y0, s)),
        ital: sc(g.ital, s),
        items: vec![Item {
            it: It::Glyph {
                key: g.key.clone(),
                x: 0,
                y,
                scale: s,
            },
            node,
        }],
        boxes: vec![],
    }
}

fn delim(n: &Node, left: u32, right: u32, st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    let body = lay(&n.kids[0], st, subst);
    let a = k("AxisHeight", s);
    let need = (2 * (body.h - a).max(body.d + a)) * 901 / 1000;
    let mut out = Laid {
        h: body.h,
        d: body.d,
        ..Laid::default()
    };
    let side = |out: &mut Laid, cp: u32| {
        if cp == 0 {
            out.w += sc(120, s);
            return;
        }
        let g = pick_variant(cp, need, s);
        let y = a - sc(g.y1 + g.y0, s) / 2;
        out.items.push(Item {
            it: It::Glyph {
                key: g.key.clone(),
                x: out.w,
                y,
                scale: s,
            },
            node: n.id,
        });
        out.h = out.h.max(y + sc(g.y1, s));
        out.d = out.d.max(-(y + sc(g.y0, s)));
        out.w += sc(g.adv, s);
    };
    side(&mut out, left);
    let x = out.w;
    out.w += body.w;
    out.place(body, x, 0);
    side(&mut out, right);
    out
}

fn accent(n: &Node, a: Acc, st: Style, s: i32, subst: &[(usize, String)]) -> Laid {
    let body = lay(&n.kids[0], st, subst);
    let mut out = Laid {
        w: body.w,
        h: body.h,
        d: body.d,
        ..Laid::default()
    };
    let bh = body.h;
    let skew = body.ital / 2;
    out.place(body, 0, 0);
    if a == Acc::Bar {
        let t = k("OverbarRuleThickness", s);
        let y = bh + sc(90, s);
        out.items.push(Item {
            it: It::Rule {
                x: sc(30, s),
                y,
                w: out.w - sc(30, s),
                h: t,
            },
            node: n.id,
        });
        out.h = y + t + sc(40, s);
        return out;
    }
    let cp = match a {
        Acc::Hat => 0x2C6,
        Acc::Tilde => 0x2DC,
        Acc::Dot => 0x2D9,
        Acc::Ddot => 0xA8,
        _ => 0x20D7,
    };
    let g = glyph_for(cp);
    let y = (bh - k("AccentBaseHeight", s)).max(0);
    let x = (out.w - sc(g.x0 + g.x1, s)) / 2 + skew;
    out.items.push(Item {
        it: It::Glyph {
            key: key_of(cp),
            x,
            y,
            scale: s,
        },
        node: n.id,
    });
    out.h = out.h.max(y + sc(g.y1, s));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tex::parse;

    fn laid(src: &str) -> (Node, Laid) {
        let n = parse(src).unwrap();
        let l = layout(&n, &[]);
        (n, l)
    }

    fn glyph_x(l: &Laid) -> Vec<i32> {
        l.items
            .iter()
            .filter_map(|i| match &i.it {
                It::Glyph { x, .. } => Some(*x),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_row_spaces_atoms_by_class() {
        // y = m x + b: y(adv) + thick + =(adv) + thick + m + x + med + + + med + b
        let (_, l) = laid("y = m x + b");
        let f = font();
        let adv = |c: u32| f.cp(c).unwrap().adv;
        let y = adv(0x1D466);
        let eq_x = y + 278;
        let m_x = eq_x + adv(0x3D) + 278;
        let x_x = m_x + adv(0x1D45A);
        let plus_x = x_x + adv(0x1D465) + 222;
        let b_x = plus_x + adv(0x2B) + 222;
        assert_eq!(glyph_x(&l), vec![0, eq_x, m_x, x_x, plus_x, b_x]);
        assert_eq!(l.w, b_x + adv(0x1D44F));
    }

    #[test]
    fn a_leading_minus_is_unary_and_gets_no_space() {
        let (_, l) = laid("-x");
        let minus = font().cp(0x2212).unwrap().adv;
        assert_eq!(glyph_x(&l), vec![0, minus]);
    }

    #[test]
    fn a_superscript_is_smaller_and_raised_by_the_font_shift() {
        let (_, l) = laid("x^2");
        let two = l
            .items
            .iter()
            .find(|i| matches!(&i.it, It::Glyph { key, .. } if key == "32"))
            .unwrap();
        // 70% size, raised by SuperscriptShiftUp (360 at the base size)
        assert!(
            matches!(
                two.it,
                It::Glyph {
                    scale: 700,
                    y: 360,
                    ..
                }
            ),
            "{:?}",
            two.it
        );
    }

    #[test]
    fn a_fraction_centres_its_parts_on_a_rule_at_the_axis() {
        let (n, l) = laid("\\frac{1}{2}");
        let rule = l.items.iter().find_map(|i| match i.it {
            It::Rule { y, h, .. } => Some((y, h)),
            _ => None,
        });
        // axis 258, rule 68 thick: bottom at 258 - 34 = 224
        assert_eq!(rule, Some((224, 68)));
        let frac = l.find(n.kids[0].id).unwrap();
        assert!(
            frac.h >= 640 && frac.d >= 640,
            "display-style shifts of 640 at least"
        );
    }

    #[test]
    fn every_node_has_a_box_and_a_substitution_replaces_its_glyphs() {
        let (n, l) = laid("m x");
        assert_eq!(l.boxes.len(), 3, "root and two atoms");
        let m = n.kids[0].id;
        let l2 = layout(&n, &[(m, "1.5".into())]);
        let keys: Vec<String> = l2
            .items
            .iter()
            .filter(|i| i.node == m)
            .filter_map(|i| match &i.it {
                It::Glyph { key, .. } => Some(key.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(keys, ["31", "2E", "35"]);
    }

    #[test]
    fn delimiters_grow_around_tall_bodies() {
        let (_, small) = laid("\\left( x \\right)");
        let (_, tall) = laid("\\left( \\frac{1}{2} \\right)");
        let key = |l: &Laid| match &l.items[0].it {
            It::Glyph { key, .. } => key.clone(),
            _ => String::new(),
        };
        assert_eq!(key(&small), "28");
        assert_ne!(key(&tall), "28", "a taller variant");
    }
}
