//! Draws the reader's view as SVG text: formulas laid out by `layout`, every
//! concept's colour, pattern, number, label, brace, border, tint, shape and
//! pointer, the hover card, and the plot. The page puts this text on screen
//! unchanged, and the exports reuse it, so the screen and a PNG, SVG or GIF
//! match exactly.

use crate::doc::Model;
use crate::font::font;
use crate::layout::{layout, It, Laid};
use crate::palette::{paint, Paint, View, CANVAS, DIM, INK, MUTED, SELECT};
use crate::sheet::{ControlKind, MarkKind, Pattern, Shape, Side};
use crate::tex::{parse, Node};
use crate::value::Fx;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    pub view: View,
    pub hover: Option<usize>,
    /// the selected part: its formula and the nodes it covers
    pub select: Option<(usize, Vec<usize>)>,
    pub step: Option<usize>,
    /// hit areas, data attributes and the editable field
    pub interactive: bool,
    /// a background rectangle and a width in pixels, for exports
    pub export_px: Option<i32>,
    pub ghost: Vec<(String, Fx)>,
    /// concepts the reader has switched off with the eye in the concept list
    pub muted: Vec<usize>,
    /// an export without the canvas colour behind it
    pub transparent: bool,
}

const MARGIN: i32 = 420;
/// The formula's size on screen: pixels to the em.
pub const SCREEN_PX_PER_EM: i32 = 44;
const LABEL_SIZE: i32 = 330;
const BADGE_R: i32 = 175;
const ROW_GAP: i32 = 560;
const SANS: &str = "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif";

pub fn esc(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Thousandths written as a decimal for an SVG attribute.
fn milli(v: i32) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.abs();
    if a % 1000 == 0 {
        format!("{sign}{}", a / 1000)
    } else {
        format!("{sign}{}.{:03}", a / 1000, a % 1000)
            .trim_end_matches('0')
            .to_string()
    }
}

fn text_width(t: &str, size: i32) -> i32 {
    t.chars()
        .map(|c| {
            if c.is_ascii_uppercase() || c == 'm' || c == 'w' {
                650
            } else if " il.,'".contains(c) {
                300
            } else {
                540
            }
        })
        .sum::<i32>()
        * size
        / 1000
}

/// A glyph or rule placed on the canvas, in SVG coordinates (y down).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    pub key: String,
    pub x: i32,
    pub y: i32,
    pub scale: i32,
    pub w: i32,
    pub h: i32,
    pub fill: &'static str,
    pub concept: Option<usize>,
    pub formula: usize,
    pub node: usize,
    pub opacity: i32,
}

#[derive(Debug, Default)]
pub struct Scene {
    pub w: i32,
    pub h: i32,
    pub drawn: Vec<Drawn>,
    pub under: String,
    pub over: String,
    pub hits: String,
    pub alt: Vec<String>,
}

struct Span {
    x0: i32,
    x1: i32,
    top: i32,
    bottom: i32,
}

fn occ_span(
    root: &Node,
    laid: &Laid,
    row: usize,
    first: usize,
    len: usize,
    bx: i32,
    by: i32,
) -> Option<Span> {
    let row = root.find(row)?;
    let mut sp: Option<Span> = None;
    for kid in row.kids.iter().skip(first).take(len) {
        let b = laid.find(kid.id)?;
        let (x0, x1, top, bottom) = (bx + b.x, bx + b.x + b.w, by - b.y - b.h, by - b.y + b.d);
        sp = Some(match sp {
            None => Span { x0, x1, top, bottom },
            Some(s) => Span {
                x0: s.x0.min(x0),
                x1: s.x1.max(x1),
                top: s.top.min(top),
                bottom: s.bottom.max(bottom),
            },
        });
    }
    sp
}

fn underline(p: Pattern, x0: i32, x1: i32, y: i32, colour: &str) -> String {
    match p {
        Pattern::Solid => format!(
            r#"<line x1="{x0}" y1="{y}" x2="{x1}" y2="{y}" stroke="{colour}" stroke-width="55" stroke-linecap="round"/>"#
        ),
        Pattern::Dashed => format!(
            r#"<line x1="{x0}" y1="{y}" x2="{x1}" y2="{y}" stroke="{colour}" stroke-width="55" stroke-dasharray="130 80"/>"#
        ),
        Pattern::Dotted => format!(
            r#"<line x1="{x0}" y1="{y}" x2="{x1}" y2="{y}" stroke="{colour}" stroke-width="72" stroke-dasharray="0 120" stroke-linecap="round"/>"#
        ),
        Pattern::Double => format!(
            r#"<line x1="{x0}" y1="{a}" x2="{x1}" y2="{a}" stroke="{colour}" stroke-width="34"/><line x1="{x0}" y1="{b}" x2="{x1}" y2="{b}" stroke="{colour}" stroke-width="34"/>"#,
            a = y - 34,
            b = y + 34
        ),
        Pattern::Wave => {
            let mut d = format!("M{x0},{y}");
            let mut x = x0;
            let mut up = true;
            while x < x1 {
                let nx = (x + 80).min(x1);
                let cy = if up { y - 60 } else { y + 60 };
                d += &format!(" Q{},{cy} {nx},{y}", (x + nx) / 2);
                x = nx;
                up = !up;
            }
            format!(
                r#"<path d="{d}" fill="none" stroke="{colour}" stroke-width="42" stroke-linecap="round"/>"#
            )
        }
    }
}

/// A small sample of a pattern, for the concept list and legends, in its own
/// viewBox 0 0 600 120.
pub fn pattern_sample(p: Pattern, colour: &str) -> String {
    format!(
        r#"<svg viewBox="0 0 600 120" aria-hidden="true">{}</svg>"#,
        underline(p, 40, 560, 60, colour)
    )
}

fn badge(cx: i32, cy: i32, num: usize, p: &Paint) -> String {
    format!(
        r#"<circle cx="{cx}" cy="{cy}" r="{BADGE_R}" fill="{f}" stroke="{INK}" stroke-width="22"/><text x="{cx}" y="{ty}" font-size="230" font-weight="700" text-anchor="middle" fill="white" font-family="{SANS}">{num}</text>"#,
        f = p.text,
        ty = cy + 82
    )
}

fn brace_path(x0: i32, x1: i32, y: i32, below: bool, colour: &str) -> String {
    let h = if below { 200 } else { -200 };
    let mid = (x0 + x1) / 2;
    let q = 100.min((x1 - x0) / 4);
    let d = format!(
        "M{x0},{y} Q{x0},{y1} {xa},{y1} L{xb},{y1} Q{mid},{y1} {mid},{y2} Q{mid},{y1} {xc},{y1} L{xd},{y1} Q{x1},{y1} {x1},{y}",
        y1 = y + h / 2,
        y2 = y + h,
        xa = x0 + q,
        xb = mid - q,
        xc = mid + q,
        xd = x1 - q
    );
    format!(
        r#"<path d="{d}" fill="none" stroke="{colour}" stroke-width="40" stroke-linejoin="round" stroke-linecap="round"/>"#
    )
}

/// Placements along a row that do not overlap: each item wants to be centred
/// at `want` with width `w`; returns left edges.
fn sweep(items: &[(i32, i32)], gap: i32) -> Vec<i32> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|i| items[*i].0);
    let mut out = vec![0; items.len()];
    let mut right = i32::MIN;
    for i in order {
        let (want, w) = items[i];
        let left = (want - w / 2).max(if right == i32::MIN {
            MARGIN / 3
        } else {
            right + gap
        });
        out[i] = left;
        right = left + w;
    }
    out
}

/// A computed value to three decimals, without trailing zeros.
pub fn short(v: Fx) -> String {
    let t = v.format(3);
    let t = if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        t
    };
    t.replace('-', "\u{2212}")
}

pub fn value_text(v: Fx, step: Fx) -> String {
    v.format(step.digits_needed(6).max(v.digits_needed(6)))
}

/// Lays out the shown formulas with every decoration.
pub fn scene(m: &Model, values: &[(String, Fx)], look: &Look, shown: &[usize]) -> Scene {
    let mut sc = Scene::default();
    let step = look.step.and_then(|s| m.sheet.steps.get(s));
    let focus: Vec<usize> = step
        .map(|s| s.focus.iter().filter_map(|c| m.concept_index(c)).collect())
        .unwrap_or_default();
    let strike: Vec<usize> = step
        .map(|s| s.strike.iter().filter_map(|c| m.concept_index(c)).collect())
        .unwrap_or_default();
    let hide: Vec<usize> = step
        .map(|s| s.hide.iter().filter_map(|c| m.concept_index(c)).collect())
        .unwrap_or_default();
    let mut y = MARGIN;
    let mut width = 0;
    let mut cards: Vec<(usize, Span)> = Vec::new();
    let mut concepts_shown: Vec<usize> = Vec::new();
    for &fi in shown {
        let Some(root) = m.roots[fi].as_ref() else {
            continue;
        };
        let occs: Vec<_> = m
            .occs
            .iter()
            .filter(|o| o.formula == fi && !look.muted.contains(&o.concept))
            .collect();
        // numbers in place of scrubbed and edited symbols
        let mut subst: Vec<(usize, String)> = occs
            .iter()
            .filter(|o| m.shows_number(o.concept) && o.len == 1)
            .filter_map(|o| {
                let c = &m.sheet.concepts[o.concept];
                let ctl = c.control.as_ref()?;
                let v = values.iter().find(|(n, _)| *n == c.value_name())?.1;
                let t = value_text(v, ctl.step);
                Some((o.lo, t.replace('-', "\u{2212}")))
            })
            .collect();
        // room around concepts that ask for space
        for o in occs.iter().filter(|o| m.sheet.concepts[o.concept].space) {
            if let Some(row) = root.find(o.row) {
                subst.push((row.kids[o.first].id, crate::layout::PAD_BEFORE.into()));
                subst.push((row.kids[o.first + o.len - 1].id, crate::layout::PAD_AFTER.into()));
            }
        }
        let laid = layout(root, &subst);
        concepts_shown.extend(occs.iter().map(|o| o.concept));
        let has_above = occs.iter().any(|o| !hide.contains(&o.concept));
        let braces_above = occs
            .iter()
            .any(|o| matches!(m.sheet.concepts[o.concept].brace, Some((Side::Above, _))));
        let row_y = y + if has_above { 2 * BADGE_R } else { 0 };
        let brace_band = if braces_above { 650 } else { 0 };
        let base = row_y + BADGE_R + 260 + brace_band + laid.h;
        let bx = MARGIN;
        // glyphs and rules
        for it in &laid.items {
            // the deepest occurrence covering this node colours it
            let covering = occs
                .iter()
                .filter(|o| o.lo <= it.node && it.node <= o.hi)
                .max_by_key(|o| o.depth);
            let ci = covering.map(|o| o.concept);
            let p = ci.map(|c| paint(look.view, &m.sheet.concepts[c]));
            let mut fill = p.map(|p| p.text).unwrap_or(INK);
            if !focus.is_empty() && !ci.map(|c| focus.contains(&c)).unwrap_or(false) {
                fill = DIM;
            }
            // an editable value shows in its input field on screen, as glyphs in exports
            let editing = |c: usize| {
                look.interactive
                    && m.sheet.concepts[c]
                        .control
                        .as_ref()
                        .map(|k| k.kind == ControlKind::Edit)
                        .unwrap_or(false)
            };
            let hidden = ci
                .map(|c| hide.contains(&c) || m.sheet.concepts[c].shape.is_some() || editing(c))
                .unwrap_or(false);
            let opacity = if hidden { 0 } else { 1000 };
            match &it.it {
                It::Glyph { key, x, y: gy, scale } => sc.drawn.push(Drawn {
                    key: key.clone(),
                    x: bx + x,
                    y: base - gy,
                    scale: *scale,
                    w: 0,
                    h: 0,
                    fill,
                    concept: ci,
                    formula: fi,
                    node: it.node,
                    opacity,
                }),
                It::Rule { x, y: ry, w, h } => sc.drawn.push(Drawn {
                    key: String::new(),
                    x: bx + x,
                    y: base - ry - h,
                    scale: 0,
                    w: *w,
                    h: *h,
                    fill,
                    concept: ci,
                    formula: fi,
                    node: it.node,
                    opacity,
                }),
                It::Brace {
                    x, y: by, w, below, ..
                } => {
                    sc.over += &brace_path(bx + x, bx + x + w, base - by, *below, fill);
                }
            }
        }
        width = width.max(laid.w);
        // decorations, outermost first
        let mut top_items: Vec<(i32, i32, usize, String)> = Vec::new(); // (centre, width, concept, label)
        let mut top_occ_centres: Vec<(i32, i32)> = Vec::new();
        let mut bottom = base + laid.d;
        let mut below_labels: Vec<(i32, i32, usize, String, i32)> = Vec::new();
        for o in &occs {
            let c = &m.sheet.concepts[o.concept];
            if hide.contains(&o.concept) {
                continue;
            }
            let Some(sp) = occ_span(root, &laid, o.row, o.first, o.len, bx, base) else {
                continue;
            };
            let p = paint(look.view, c);
            let hovered = look.hover == Some(o.concept);
            if c.tint || p.always_tint || hovered {
                let tint = p.tint;
                sc.under += &format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="60" fill="{tint}"{}/>"#,
                    sp.x0 - 50,
                    sp.top - 50,
                    sp.x1 - sp.x0 + 100,
                    sp.bottom - sp.top + 100,
                    if hovered {
                        " stroke=\"#3B2340\" stroke-width=\"24\""
                    } else {
                        ""
                    }
                );
            }
            if c.border {
                sc.over += &format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="70" fill="none" stroke="{}" stroke-width="40"/>"#,
                    sp.x0 - 70,
                    sp.top - 70,
                    sp.x1 - sp.x0 + 140,
                    sp.bottom - sp.top + 140,
                    p.text
                );
            }
            if let Some(shape) = c.shape {
                let (x0, x1, t, b) = (sp.x0 + 30, sp.x1 - 30, sp.top + 30, sp.bottom - 30);
                let body = match shape {
                    Shape::Triangle => format!(r#"<path d="M{x0},{b} L{x1},{b} L{x1},{t} Z""#),
                    Shape::Circle => format!(
                        r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}""#,
                        (x0 + x1) / 2,
                        (t + b) / 2,
                        (x1 - x0) / 2,
                        (b - t) / 2
                    ),
                    Shape::Square => format!(
                        r#"<rect x="{x0}" y="{t}" width="{}" height="{}" rx="30""#,
                        x1 - x0,
                        b - t
                    ),
                };
                sc.over += &format!(
                    r#"{body} fill="{}" stroke="{INK}" stroke-width="40" stroke-linejoin="round"/>"#,
                    p.tint
                );
            }
            let uy = sp.bottom + 95 + o.depth as i32 * 120;
            sc.over += &underline(c.pattern, sp.x0 + 15, sp.x1 - 15, uy, p.text);
            bottom = bottom.max(uy + 60);
            if strike.contains(&o.concept) {
                sc.over += &format!(
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{INK}" stroke-width="60" stroke-linecap="round"/>"#,
                    sp.x0 - 20,
                    sp.bottom - 30,
                    sp.x1 + 20,
                    sp.top + 30
                );
            }
            if c.pointer {
                let tip_x = (sp.x0 + sp.x1) / 2 - 40;
                let tip_y = sp.top - 60;
                sc.over += &format!(
                    r#"<line x1="{}" y1="{}" x2="{tip_x}" y2="{tip_y}" stroke="{INK}" stroke-width="40" stroke-linecap="round"/><path d="M{tip_x},{tip_y} l-40,-150 l140,60 z" fill="{INK}"/>"#,
                    tip_x - 700,
                    tip_y - 260
                );
            }
            let cx = (sp.x0 + sp.x1) / 2;
            // Mono names every concept beside its number, since nothing else tells them apart
            let label = match &c.label {
                Some((Side::Above, t)) => t.clone(),
                Some(_) => String::new(),
                None if look.view == View::Mono && o.depth == 0 => c.name.clone(),
                None => String::new(),
            };
            if hovered
                && c.control
                    .as_ref()
                    .map(|k| k.kind == ControlKind::Scrub)
                    .unwrap_or(false)
                && o.len == 1
            {
                // the drag affordance: a double arrow over a scrubbable value
                let (ax, bx, ay) = (sp.x0 - 60, sp.x1 + 60, sp.top - 90);
                sc.over += &format!(
                    r#"<line x1="{}" y1="{ay}" x2="{}" y2="{ay}" stroke="{}" stroke-width="34"/><path d="M{ax},{ay} l110,-70 l0,140 z M{bx},{ay} l-110,-70 l0,140 z" fill="{}"/>"#,
                    ax + 80,
                    bx - 80,
                    p.text,
                    p.text
                );
            }
            let w = 2 * BADGE_R
                + if label.is_empty() {
                    0
                } else {
                    70 + text_width(&label, LABEL_SIZE)
                };
            top_items.push((cx, w, o.concept, label));
            top_occ_centres.push((cx, sp.top));
            if let Some((side, text)) = &c.brace {
                let below = *side == Side::Below;
                let by = if below { bottom + 40 } else { sp.top - 80 };
                sc.over += &brace_path(sp.x0, sp.x1, by, below, p.text);
                if below {
                    below_labels.push((
                        cx,
                        text_width(text, LABEL_SIZE),
                        o.concept,
                        text.clone(),
                        by + 200,
                    ));
                    bottom = bottom.max(by + 220);
                } else {
                    let ty = by - 260;
                    sc.over += &format!(
                        r#"<text x="{cx}" y="{ty}" font-size="{LABEL_SIZE}" font-weight="600" text-anchor="middle" fill="{}" font-family="{SANS}">{}</text>"#,
                        p.text,
                        esc(text)
                    );
                }
            }
            if let Some((Side::Below, text)) = &c.label {
                below_labels.push((cx, text_width(text, LABEL_SIZE), o.concept, text.clone(), bottom));
            }
            if look.interactive {
                // a tap on a part selects that part, so its card opens
                let mut attrs = format!(
                    r#" data-c="{}" data-n="{}:{}" data-part="{}:{}:{}""#,
                    c.num, o.formula, o.lo, o.row, o.first, o.len
                );
                if let Some(ctl) = &c.control {
                    if ctl.kind == ControlKind::Scrub && o.len == 1 {
                        let v = values
                            .iter()
                            .find(|(n, _)| *n == c.value_name())
                            .map(|(_, v)| *v)
                            .unwrap_or(ctl.value);
                        attrs += &format!(
                            r#" data-scrub="{}" class="fb-scrub" tabindex="0" role="slider" aria-label="{}" aria-valuetext="{}""#,
                            c.num,
                            esc(&c.name),
                            esc(&value_text(v, ctl.step))
                        );
                    }
                }
                sc.hits += &format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="white" fill-opacity="0"{attrs}/>"##,
                    sp.x0 - 40,
                    sp.top - 60,
                    sp.x1 - sp.x0 + 80,
                    sp.bottom - sp.top + 120
                );
                if let Some(ctl) = &c.control {
                    if ctl.kind == ControlKind::Edit && o.len == 1 {
                        let v = values
                            .iter()
                            .find(|(n, _)| *n == c.value_name())
                            .map(|(_, v)| *v)
                            .unwrap_or(ctl.value);
                        sc.hits += &format!(
                            r#"<foreignObject x="{}" y="{}" width="{}" height="{}"><input xmlns="http://www.w3.org/1999/xhtml" class="fb-edit" data-edit="{}" data-name="{}" value="{}" inputmode="decimal" aria-label="{}"/></foreignObject>"#,
                            sp.x0 - 40,
                            sp.top - 40,
                            sp.x1 - sp.x0 + 80,
                            sp.bottom - sp.top + 80,
                            c.num,
                            esc(&c.value_name()),
                            esc(&value_text(v, ctl.step)),
                            esc(&c.name)
                        );
                    }
                }
            }
            if hovered && cards.iter().all(|(ci, _)| *ci != o.concept) {
                cards.push((
                    o.concept,
                    Span {
                        x0: sp.x0,
                        x1: sp.x1,
                        top: sp.top,
                        bottom: sp.bottom,
                    },
                ));
            }
            if o.depth == 0 && !sc.alt.contains(&c.name) {
                sc.alt.push(c.name.clone());
            }
        }
        // the top row: numbers, and labels beside them, at one height
        let lefts = sweep(&top_items.iter().map(|t| (t.0, t.1)).collect::<Vec<_>>(), 60);
        for (i, (_, w, ci, label)) in top_items.iter().enumerate() {
            let c = &m.sheet.concepts[*ci];
            let p = paint(look.view, c);
            let left = lefts[i];
            let bcx = left + BADGE_R;
            let (ocx, otop) = top_occ_centres[i];
            if !label.is_empty() || (bcx - ocx).abs() > 120 {
                sc.over += &format!(
                    r#"<line x1="{bcx}" y1="{}" x2="{ocx}" y2="{}" stroke="{}" stroke-width="22"/>"#,
                    row_y + BADGE_R,
                    otop - 40,
                    if look.view == View::Mono { INK } else { p.text }
                );
            }
            sc.over += &badge(bcx, row_y, c.num, &p);
            if !label.is_empty() {
                sc.over += &format!(
                    r#"<text x="{}" y="{}" font-size="{LABEL_SIZE}" font-weight="600" fill="{}" font-family="{SANS}">{}</text>"#,
                    left + 2 * BADGE_R + 60,
                    row_y + 95,
                    p.text,
                    esc(label)
                );
            }
            width = width.max(left - MARGIN + w);
        }
        // labels below, at one height
        if !below_labels.is_empty() {
            let row = below_labels.iter().map(|l| l.4).max().unwrap_or(bottom) + 340;
            let lefts = sweep(&below_labels.iter().map(|l| (l.0, l.1)).collect::<Vec<_>>(), 80);
            for (i, (cx, w, ci, text, from)) in below_labels.iter().enumerate() {
                let p = paint(look.view, &m.sheet.concepts[*ci]);
                let x = lefts[i];
                sc.over += &format!(
                    r#"<line x1="{cx}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="22"/><text x="{x}" y="{row}" font-size="{LABEL_SIZE}" font-weight="600" fill="{}" font-family="{SANS}">{}</text>"#,
                    from + 30,
                    x + w / 2,
                    row - 250,
                    p.text,
                    p.text,
                    esc(text)
                );
                width = width.max(x - MARGIN + w);
            }
            bottom = row + 80;
        }
        if let Some((nf, nodes)) = &look.select {
            let boxes: Vec<_> = nodes.iter().filter_map(|n| laid.find(*n)).collect();
            if *nf == fi && !boxes.is_empty() {
                // one box around the whole part, however many nodes it covers
                let x0 = boxes.iter().map(|b| b.x).min().unwrap_or(0);
                let x1 = boxes.iter().map(|b| b.x + b.w).max().unwrap_or(0);
                let top = boxes.iter().map(|b| b.y + b.h).max().unwrap_or(0);
                let bot = boxes.iter().map(|b| b.d - b.y).max().unwrap_or(0);
                sc.over += &format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="40" fill="none" stroke="{SELECT}" stroke-width="40" stroke-dasharray="90 60"/>"#,
                    bx + x0 - 40,
                    base - top - 40,
                    x1 - x0 + 80,
                    top + bot + 80
                );
            }
        }
        y = bottom + ROW_GAP;
    }
    // the legend: each concept's number, pattern and name, so a printout or a
    // Mono copy explains itself
    concepts_shown.sort();
    concepts_shown.dedup();
    if !concepts_shown.is_empty() {
        let mut x = MARGIN;
        let mut ly = y - ROW_GAP + 520;
        let limit = width.max(7000) + MARGIN;
        for ci in &concepts_shown {
            let c = &m.sheet.concepts[*ci];
            let p = paint(look.view, c);
            let w = 2 * BADGE_R + 120 + 620 + 100 + text_width(&c.name, LABEL_SIZE) + 360;
            if x + w > limit && x > MARGIN {
                x = MARGIN;
                ly += 520;
            }
            sc.over += &badge(x + BADGE_R, ly, c.num, &p);
            sc.over += &underline(
                c.pattern,
                x + 2 * BADGE_R + 120,
                x + 2 * BADGE_R + 740,
                ly,
                p.text,
            );
            sc.over += &format!(
                r#"<text x="{}" y="{}" font-size="{LABEL_SIZE}" fill="{}" font-family="{SANS}">{}</text>"#,
                x + 2 * BADGE_R + 840,
                ly + 110,
                if look.view == View::Mono { INK } else { p.text },
                esc(&c.name)
            );
            x += w;
            width = width.max(x - MARGIN);
        }
        y = ly + 300 + ROW_GAP;
    }
    sc.w = width + 2 * MARGIN;
    sc.h = (y - ROW_GAP + MARGIN).max(1400);
    for (ci, sp) in cards {
        sc.over += &card(m, values, look, ci, &sp, sc.w);
    }
    sc
}

/// The definition card a hovered concept opens (details on demand).
fn card(m: &Model, values: &[(String, Fx)], look: &Look, ci: usize, sp: &Span, canvas_w: i32) -> String {
    let c = &m.sheet.concepts[ci];
    let p = paint(look.view, c);
    let mut title = c.name.clone();
    if let Some(ctl) = &c.control {
        if let Some((_, v)) = values.iter().find(|(n, _)| *n == c.value_name()) {
            title += &format!(" = {}", value_text(*v, ctl.step));
        }
    } else if let Ok(scope_v) = m.scope(values).value_of(&c.value_name(), &[]) {
        if !c.value_name().is_empty() && c.value_name().chars().count() < 12 {
            title += &format!(" = {}", short(scope_v));
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in c.say.split_whitespace() {
        if cur.len() + w.len() > 34 && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur += w;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    let w = lines
        .iter()
        .map(|l| text_width(l, 320))
        .chain([text_width(&title, 350) + 480])
        .max()
        .unwrap_or(1000)
        + 300;
    let h = 560 + lines.len() as i32 * 410;
    let x = ((sp.x0 + sp.x1) / 2 - 300).max(60).min(canvas_w - w - 60);
    let y = if sp.top - h - 140 > 40 {
        sp.top - h - 140
    } else {
        sp.bottom + 200
    };
    let mut out = format!(
        r#"<g class="fb-card"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="70" fill="white" stroke="{INK}" stroke-width="30"/>"#
    );
    out += &badge(x + 250, y + 270, c.num, &p);
    out += &format!(
        r#"<text x="{}" y="{}" font-size="350" font-weight="700" fill="{}" font-family="{SANS}">{}</text>"#,
        x + 500,
        y + 390,
        p.text,
        esc(&title)
    );
    for (i, l) in lines.iter().enumerate() {
        out += &format!(
            r#"<text x="{}" y="{}" font-size="320" fill="{INK}" font-family="{SANS}">{}</text>"#,
            x + 150,
            y + 860 + i as i32 * 410,
            esc(l)
        );
    }
    out + "</g>"
}

fn glyph_defs(drawn: &[Drawn]) -> String {
    let mut keys: Vec<&str> = drawn
        .iter()
        .filter(|d| !d.key.is_empty())
        .map(|d| d.key.as_str())
        .collect();
    keys.sort();
    keys.dedup();
    let f = font();
    keys.iter()
        .filter_map(|k| {
            f.glyph(k)
                .map(|g| format!(r#"<path id="fbg{k}" d="{}"/>"#, g.path))
        })
        .collect()
}

fn draw_glyphs(drawn: &[Drawn], interactive: bool, m: &Model) -> String {
    if interactive {
        return draw_list(drawn.iter(), true, m);
    }
    // an export groups each concept's glyphs, named, for vector editors (R9)
    let mut out = String::new();
    for (ci, c) in m.sheet.concepts.iter().enumerate() {
        let mine: Vec<&Drawn> = drawn.iter().filter(|d| d.concept == Some(ci)).collect();
        if !mine.is_empty() {
            out += &format!(
                r#"<g id="c{}-{}" data-pattern="{}">"#,
                c.num,
                esc(&c.id),
                c.pattern.name()
            );
            out += &draw_list(mine.into_iter(), false, m);
            out += "</g>";
        }
    }
    out + &draw_list(drawn.iter().filter(|d| d.concept.is_none()), false, m)
}

fn draw_list<'a>(drawn: impl Iterator<Item = &'a Drawn>, interactive: bool, m: &Model) -> String {
    let mut out = String::new();
    for d in drawn {
        if d.opacity <= 0 {
            continue;
        }
        let op = if d.opacity < 1000 {
            format!(r#" opacity="{}""#, milli(d.opacity))
        } else {
            String::new()
        };
        let mut attrs = String::new();
        if interactive {
            attrs += &format!(r#" data-n="{}:{}""#, d.formula, d.node);
            if let Some(c) = d.concept {
                attrs += &format!(r#" data-c="{}""#, m.sheet.concepts[c].num);
            }
        }
        if d.key.is_empty() {
            out += &format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"{op}{attrs}/>"#,
                d.x, d.y, d.w, d.h, d.fill
            );
        } else {
            let s = milli(d.scale);
            out += &format!(
                r##"<use href="#fbg{}" transform="translate({},{}) scale({s},-{s})" fill="{}"{op}{attrs}/>"##,
                d.key, d.x, d.y, d.fill
            );
        }
    }
    out
}

/// The finished SVG for a scene. `export_px` gives the picture a pixel width
/// and a background.
pub fn svg(m: &Model, sc: &Scene, look: &Look) -> String {
    let size = match look.export_px {
        Some(px) => format!(
            r#" width="{px}" height="{}""#,
            (sc.h as i64 * px as i64 / sc.w.max(1) as i64)
        ),
        None => format!(
            r#" width="{}" height="{}""#,
            sc.w * SCREEN_PX_PER_EM / 1000,
            sc.h * SCREEN_PX_PER_EM / 1000
        ),
    };
    let bg = if look.export_px.is_some() && !look.transparent {
        format!(r#"<rect width="{}" height="{}" fill="{CANVAS}"/>"#, sc.w, sc.h)
    } else {
        String::new()
    };
    let title = esc(&sc.alt.join(", "));
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}"{size} role="img" class="fb-formula"><title>{title}</title><defs>{}</defs>{bg}{}{}{}{}</svg>"#,
        sc.w,
        sc.h,
        glyph_defs(&sc.drawn),
        sc.under,
        if look.interactive { &sc.hits } else { "" },
        draw_glyphs(&sc.drawn, look.interactive, m),
        sc.over
    )
}

/// One frame between two steps: glyphs that persist slide, the rest fade.
/// `t` runs from 0 to 1000.
pub fn morph(a: &Scene, b: &Scene, t: i32, m: &Model) -> Scene {
    let ident = |s: &Scene| {
        let mut seen: Vec<(Option<usize>, String, usize)> = Vec::new();
        s.drawn
            .iter()
            .map(|d| {
                let n = seen
                    .iter()
                    .filter(|(c, k, _)| *c == d.concept && *k == d.key)
                    .count();
                seen.push((d.concept, d.key.clone(), n));
                (d.concept, d.key.clone(), n)
            })
            .collect::<Vec<_>>()
    };
    let (ia, ib) = (ident(a), ident(b));
    let lerp = |p: i32, q: i32| p + (q - p) * t / 1000;
    let mut out = Scene {
        w: lerp(a.w, b.w),
        h: lerp(a.h, b.h),
        ..Scene::default()
    };
    for (i, d) in a.drawn.iter().enumerate() {
        match ib.iter().position(|k| *k == ia[i]) {
            Some(j) => {
                let e = &b.drawn[j];
                out.drawn.push(Drawn {
                    x: lerp(d.x, e.x),
                    y: lerp(d.y, e.y),
                    scale: lerp(d.scale, e.scale),
                    w: lerp(d.w, e.w),
                    h: lerp(d.h, e.h),
                    opacity: lerp(d.opacity, e.opacity),
                    fill: if t < 500 { d.fill } else { e.fill },
                    ..d.clone()
                })
            }
            None => out.drawn.push(Drawn {
                opacity: d.opacity * (1000 - t) / 1000,
                ..d.clone()
            }),
        }
    }
    for (j, e) in b.drawn.iter().enumerate() {
        if !ia.contains(&ib[j]) {
            out.drawn.push(Drawn {
                opacity: e.opacity * t / 1000,
                ..e.clone()
            });
        }
    }
    let _ = m;
    out
}

// ---------------------------------------------------------------- plot

pub const PLOT_W: i32 = 640;
pub const PLOT_H: i32 = 340;
const PAD_L: i32 = 52;
const PAD_R: i32 = 18;
const PAD_T: i32 = 16;
const PAD_B: i32 = 40;

fn nice_step(span: Fx) -> Fx {
    let target = span.0 / 5;
    let mut unit: i64 = 1;
    while unit * 10 <= target {
        unit *= 10;
    }
    while unit > target && unit > 1 {
        unit /= 10;
    }
    let step = [1, 2, 5, 10]
        .iter()
        .map(|m| unit * m)
        .find(|s| *s >= target)
        .unwrap_or(unit * 10);
    Fx(step.max(1))
}

/// The plot, with every mark the sheet asks for, or None without a plot.
pub fn plot_svg(m: &Model, values: &[(String, Fx)], look: &Look) -> Option<Result<String, String>> {
    let plot = m.sheet.plot.as_ref()?;
    let scope = m.scope(values);
    let n: i64 = match Fx::parse(&plot.samples) {
        Some(v) => v.0 / Fx::ONE.0,
        None => scope
            .value_of(&plot.samples, &[])
            .map(|v| v.0 / Fx::ONE.0)
            .unwrap_or(120),
    }
    .clamp(2, 400);
    let f = |x: Fx| -> Result<Fx, String> {
        let bound = vec![(plot.var.clone(), x)];
        if scope.has_func(&plot.of) {
            scope.call(&plot.of, x, &[])
        } else {
            scope.value_of(&plot.of, &bound)
        }
    };
    let span = plot.to.sub(plot.from).ok()?;
    let xs: Vec<Fx> = (0..=n)
        .map(|i| Fx(plot.from.0 + (span.0 as i128 * i as i128 / n as i128) as i64))
        .collect();
    let mut ys = Vec::new();
    for x in &xs {
        match f(*x) {
            Ok(y) => ys.push(Some(y)),
            Err(e) if ys.is_empty() && !e.contains("divides by zero") && !e.contains("no real value") => {
                return Some(Err(format!("The plot cannot be drawn: {e}")))
            }
            Err(_) => ys.push(None),
        }
    }
    let (y0, y1) = match plot.y {
        Some(r) => r,
        None => {
            let lo = ys.iter().flatten().min().copied().unwrap_or(Fx::int(-1));
            let hi = ys.iter().flatten().max().copied().unwrap_or(Fx::ONE);
            let pad = Fx(((hi.0 - lo.0) / 10).max(Fx::ONE.0 / 2));
            (Fx(lo.0 - pad.0), Fx(hi.0 + pad.0))
        }
    };
    let iw = (PLOT_W - PAD_L - PAD_R) as i128;
    let ih = (PLOT_H - PAD_T - PAD_B) as i128;
    let px = |x: Fx| PAD_L + ((x.0 - plot.from.0) as i128 * iw / span.0 as i128) as i32;
    let py = |y: Fx| PAD_T + ((y1.0 - y.0) as i128 * ih / (y1.0 - y0.0).max(1) as i128) as i32;
    let inside = |y: Fx| y >= y0 && y <= y1;
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {PLOT_W} {PLOT_H}" class="fb-plot" role="img"><title>plot of {} over {}</title><rect x="{PAD_L}" y="{PAD_T}" width="{}" height="{}" fill="white" stroke="{INK}" stroke-width="1.5" rx="4"/>"#,
        esc(&crate::eval::show(&plot.of)),
        esc(&crate::eval::show(&plot.var)),
        iw,
        ih
    );
    // grid and ticks
    for (lo, hi, horizontal) in [(plot.from, plot.to, true), (y0, y1, false)] {
        let step = nice_step(Fx(hi.0 - lo.0));
        let mut v = Fx((lo.0.div_euclid(step.0) + 1) * step.0);
        let digits = step.digits_needed(4);
        while v < hi {
            let label = v.format(digits).replace('-', "\u{2212}");
            if horizontal {
                let x = px(v);
                out += &format!(
                    r#"<line x1="{x}" y1="{PAD_T}" x2="{x}" y2="{}" stroke="{}" stroke-width="{}"/><text x="{x}" y="{}" font-size="12" text-anchor="middle" fill="{MUTED}" font-family="{SANS}">{label}</text>"#,
                    PLOT_H - PAD_B,
                    if v.0 == 0 { MUTED } else { "#EFE6DA" },
                    if v.0 == 0 { "1.5" } else { "1" },
                    PLOT_H - PAD_B + 16
                );
            } else {
                let y = py(v);
                out += &format!(
                    r#"<line x1="{PAD_L}" y1="{y}" x2="{}" y2="{y}" stroke="{}" stroke-width="{}"/><text x="{}" y="{}" font-size="12" text-anchor="end" fill="{MUTED}" font-family="{SANS}">{label}</text>"#,
                    PLOT_W - PAD_R,
                    if v.0 == 0 { MUTED } else { "#EFE6DA" },
                    if v.0 == 0 { "1.5" } else { "1" },
                    PAD_L - 6,
                    y + 4
                );
            }
            v = Fx(v.0 + step.0);
        }
    }
    out += &format!(
        r#"<text x="{}" y="{}" font-size="13" text-anchor="end" fill="{INK}" font-family="{SANS}" font-style="italic">{}</text>"#,
        PLOT_W - PAD_R,
        PLOT_H - 4,
        esc(&crate::eval::show(&plot.var))
    );
    let polyline = |ys: &[Option<Fx>], colour: &str, extra: &str| -> String {
        let mut s = String::new();
        let mut pts: Vec<String> = Vec::new();
        let flush = |pts: &mut Vec<String>, s: &mut String| {
            if pts.len() > 1 {
                *s += &format!(
                    r#"<polyline points="{}" fill="none" stroke="{colour}" stroke-width="3" stroke-linejoin="round" stroke-linecap="round"{extra}/>"#,
                    pts.join(" ")
                );
            }
            pts.clear();
        };
        for (x, y) in xs.iter().zip(ys) {
            match y {
                Some(y) if inside(*y) => pts.push(format!("{},{}", px(*x), py(*y))),
                _ => flush(&mut pts, &mut s),
            }
        }
        flush(&mut pts, &mut s);
        s
    };
    if !look.ghost.is_empty() {
        let gscope = m.scope(&look.ghost);
        let gys: Vec<Option<Fx>> = xs
            .iter()
            .map(|x| {
                if gscope.has_func(&plot.of) {
                    gscope.call(&plot.of, *x, &[]).ok()
                } else {
                    gscope.value_of(&plot.of, &[(plot.var.clone(), *x)]).ok()
                }
            })
            .collect();
        out += &polyline(&gys, "#B9AEB5", r#" stroke-dasharray="6 6""#);
    }
    let curve_mark = plot
        .marks
        .iter()
        .find(|mk| mk.kind == MarkKind::Curve)
        .and_then(|mk| m.concept_index(&mk.concept));
    match curve_mark {
        Some(ci) => {
            let p = paint(look.view, &m.sheet.concepts[ci]);
            let dash = match m.sheet.concepts[ci].pattern {
                Pattern::Dashed => r#" stroke-dasharray="12 7""#,
                Pattern::Dotted => r#" stroke-dasharray="0.1 8""#,
                _ => "",
            };
            out += &format!(
                r#"<g data-c="{}">{}</g>"#,
                m.sheet.concepts[ci].num,
                polyline(&ys, p.text, dash)
            );
        }
        None => out += &polyline(&ys, INK, ""),
    }
    // marks
    let axis_y = py(Fx::ZERO.max(y0).min(y1));
    for mk in &plot.marks {
        let Some(ci) = m.concept_index(&mk.concept) else {
            continue;
        };
        let c = &m.sheet.concepts[ci];
        let p = paint(look.view, c);
        let thick = if look.hover == Some(ci) { 5 } else { 3 };
        let at =
            |src: &str| -> Option<Fx> { parse(src).ok().and_then(|n| scope.eval_row(&n.kids, &[]).ok()) };
        let mut g = String::new();
        match mk.kind {
            MarkKind::Point | MarkKind::Intercept => {
                let x = if mk.kind == MarkKind::Intercept {
                    Some(Fx::ZERO)
                } else {
                    at(&mk.at)
                };
                let Some(x) = x else { continue };
                let Ok(y) = f(x) else { continue };
                if x < plot.from || x > plot.to || !inside(y) {
                    continue;
                }
                let (cx, cy) = (px(x), py(y));
                g += &plot_line(c.pattern, cx, axis_y, cx, cy, p.text, thick);
                g += &format!(
                    r#"<circle cx="{cx}" cy="{cy}" r="7" fill="{}" stroke="{INK}" stroke-width="2"/>"#,
                    p.text
                );
                g += &plot_badge(cx, axis_y + 2, c.num, &p);
            }
            MarkKind::Rise => {
                let Some(x) = at(&mk.at) else { continue };
                let run = Fx(nice_step(span).0);
                let x2 = Fx(x.0 + run.0);
                let (Ok(ya), Ok(yb)) = (f(x), f(x2)) else { continue };
                if !inside(ya) || !inside(yb) {
                    continue;
                }
                let (ax, ay, bx, by) = (px(x), py(ya), px(x2), py(yb));
                g += &format!(
                    r#"<line x1="{ax}" y1="{ay}" x2="{bx}" y2="{ay}" stroke="{INK}" stroke-width="2"/>"#
                );
                g += &plot_line(c.pattern, bx, ay, bx, by, p.text, thick + 1);
                g += &plot_badge(bx + 16, (ay + by) / 2, c.num, &p);
            }
            MarkKind::Curve => {}
        }
        out += &format!(r#"<g data-c="{}">{g}</g>"#, c.num);
    }
    out += "</svg>";
    Some(Ok(out))
}

fn plot_line(p: Pattern, x1: i32, y1: i32, x2: i32, y2: i32, colour: &str, w: i32) -> String {
    let dash = match p {
        Pattern::Dashed => r#" stroke-dasharray="10 6""#,
        Pattern::Dotted => r#" stroke-dasharray="0.1 7" stroke-linecap="round""#,
        Pattern::Wave => r#" stroke-dasharray="2 3 8 3""#,
        Pattern::Double => r#" stroke-dasharray="14 3 2 3""#,
        Pattern::Solid => "",
    };
    format!(r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{colour}" stroke-width="{w}"{dash}/>"#)
}

fn plot_badge(cx: i32, cy: i32, num: usize, p: &Paint) -> String {
    format!(
        r#"<circle cx="{cx}" cy="{}" r="10" fill="{}" stroke="{INK}" stroke-width="1.5"/><text x="{cx}" y="{}" font-size="12" font-weight="700" text-anchor="middle" fill="white" font-family="{SANS}">{num}</text>"#,
        cy + 12,
        p.text,
        cy + 16
    )
}
