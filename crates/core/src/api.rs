//! The door into the core. Every function takes strings and returns a JSON
//! string: `{"ok": ...}` on success or `{"error": "..."}` with a message the
//! page can show as it is. The shim forwards these calls and nothing else.
//!
//! The core describes the screen as a view model (`Ui`): the formula as SVG,
//! the plot, the prose, the concept list, the controls, the steps, and any
//! export waiting to be saved. Everything a reader does comes back through
//! one function, `act`, with a dotted name such as `view.set`. (After Peter's
//! Mandible.)

use serde::{Deserialize, Serialize};

use crate::doc::{Doc, Model};
use crate::export;
use crate::glossary;
use crate::page::{self, Strings};
use crate::palette::{paint, View};
use crate::render::{morph, pattern_sample, plot_svg, scene, svg, value_text, Look, Scene};
use crate::sheet::{parse_prose, ControlKind, COLOUR_BUDGET};
use crate::tex::{Node, K};
use crate::value::Fx;

fn ok<T: Serialize>(value: &T) -> String {
    serde_json::json!({ "ok": value }).to_string()
}

fn error(message: String) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// Ticks (the page's animation frames) a step holds, and a morph takes.
pub const HOLD_TICKS: u32 = 90;
pub const MORPH_TICKS: u32 = 36;
/// GIF timing, in hundredths of a second.
pub const GIF_HOLD: u16 = 160;
pub const GIF_MORPH_FRAMES: i32 = 10;
pub const GIF_MORPH_DELAY: u16 = 4;
/// Pixels of pointer travel for one step of a scrubbed value.
pub const SCRUB_PX: i64 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Example {
    pub id: String,
    pub label: String,
    pub doc: Doc,
}

/// What the page hands back on the next call. The page stores it and does
/// not read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub mode: String,
    pub strings: Strings,
    pub examples: Vec<Example>,
    pub doc: Doc,
    pub view: String,
    pub step: Option<usize>,
    pub hover: Option<usize>,
    pub select: Option<(usize, usize)>,
    pub overrides: Vec<(String, String)>,
    pub scrub: Option<(usize, i64, String)>,
    pub ghost: Vec<(String, String)>,
    pub playing: bool,
    pub tick: u32,
    /// the page's clock, in milliseconds, when play started
    pub play_t0: Option<i64>,
    pub export_px: i32,
    pub site: String,
    /// concepts switched off with the eye
    #[serde(default)]
    pub muted: Vec<usize>,
    /// PNG and SVG exports without the canvas colour
    #[serde(default)]
    pub export_clear: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub term: String,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProseSeg {
    pub text: String,
    pub num: usize,
    pub colour: String,
    pub line: String,
    pub hovered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptUi {
    pub num: usize,
    pub id: String,
    pub name: String,
    pub say: String,
    pub colour: String,
    pub sample: String,
    pub pattern: String,
    pub hovered: bool,
    pub muted: bool,
    pub features: Vec<Choice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlUi {
    /// slider, edit, limit or hidden
    pub kind: String,
    pub num: usize,
    pub name: String,
    pub label: String,
    pub colour: String,
    pub min: String,
    pub max: String,
    pub step: String,
    pub value: String,
    pub lo: String,
    pub hi: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Frame {
    pub svg: String,
    pub delay: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Out {
    /// download (text file), png, gif, or copy
    pub kind: String,
    pub name: String,
    pub mime: String,
    pub text: String,
    pub frames: Vec<Frame>,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ui {
    pub mode: String,
    pub strings: Strings,
    pub hints: Vec<Hint>,
    pub doc: Doc,
    pub fragment: String,
    pub view: String,
    pub views: Vec<Choice>,
    pub examples: Vec<Choice>,
    pub errors: Vec<String>,
    pub formula_svg: String,
    pub plot_svg: String,
    pub plot_error: String,
    pub prose: Vec<ProseSeg>,
    pub concepts: Vec<ConceptUi>,
    pub budget: String,
    pub budget_used: usize,
    pub budget_max: usize,
    pub controls: Vec<ControlUi>,
    pub steps: Vec<Choice>,
    pub playing: bool,
    pub export_clear: bool,
    pub selection: String,
    pub out: Option<Out>,
    pub state: State,
}

fn s(state: &State, key: &str) -> String {
    state.strings.get(key).cloned().unwrap_or_default()
}

fn fill(template: &str, pairs: &[(&str, String)]) -> String {
    pairs.iter().fold(template.to_string(), |t, (k, v)| {
        t.replace(&format!("{{{k}}}"), v)
    })
}

fn values_of(pairs: &[(String, String)]) -> Vec<(String, Fx)> {
    pairs
        .iter()
        .filter_map(|(n, v)| Fx::parse(v).map(|v| (n.clone(), v)))
        .collect()
}

fn view_of(state: &State) -> View {
    View::from(&state.view).unwrap_or(View::Colour)
}

/// The step on screen, the next step and how far between them (0..1000),
/// while a step sequence plays.
fn timeline(state: &State, steps: usize) -> (usize, Option<(usize, i32)>) {
    let per = HOLD_TICKS + MORPH_TICKS;
    let seg = (state.tick / per) as usize;
    let within = state.tick % per;
    if steps == 0 {
        return (0, None);
    }
    if seg + 1 >= steps || within < HOLD_TICKS {
        return (seg.min(steps - 1), None);
    }
    (
        seg,
        Some((seg + 1, ((within - HOLD_TICKS) * 1000 / MORPH_TICKS) as i32)),
    )
}

fn look(state: &State, m: &Model, interactive: bool) -> Look {
    let step = if m.sheet.steps.is_empty() {
        None
    } else {
        Some(state.step.unwrap_or(0).min(m.sheet.steps.len() - 1))
    };
    Look {
        view: view_of(state),
        hover: if interactive { state.hover } else { None },
        select: if interactive && state.mode == "edit" {
            state.select
        } else {
            None
        },
        step,
        interactive,
        export_px: None,
        ghost: values_of(&state.ghost),
        muted: state.muted.clone(),
        transparent: state.export_clear,
    }
}

fn padded(mut sc: Scene, w: i32, h: i32) -> Scene {
    sc.w = sc.w.max(w);
    sc.h = sc.h.max(h);
    sc
}

fn formula_view(state: &State, m: &Model) -> String {
    let lk = look(state, m, true);
    let values = m.values(&values_of(&state.overrides), lk.step);
    if state.playing && !m.sheet.steps.is_empty() {
        let (from, to) = timeline(state, m.sheet.steps.len());
        let still = Look {
            interactive: false,
            hover: None,
            select: None,
            step: Some(from),
            ..lk.clone()
        };
        let a = scene(m, &values, &still, &m.shown(Some(from)));
        if let Some((next, t)) = to {
            let lb = Look {
                step: Some(next),
                ..still.clone()
            };
            let vb = m.values(&values_of(&state.overrides), Some(next));
            let b = scene(m, &vb, &lb, &m.shown(Some(next)));
            let mid = morph(&a, &b, t, m);
            return svg(m, &mid, &still);
        }
        return svg(m, &a, &still);
    }
    let sc = scene(m, &values, &lk, &m.shown(lk.step));
    svg(m, &sc, &lk)
}

fn features(state: &State, m: &Model, ci: usize) -> Vec<Choice> {
    let c = &m.sheet.concepts[ci];
    let single = m.occs.iter().any(|o| o.concept == ci && o.len == 1);
    let mut out = vec![
        ("label", c.label.is_some()),
        ("brace", c.brace.is_some()),
        ("border", c.border),
        ("tint", c.tint),
        ("pointer", c.pointer),
        ("space", c.space),
    ];
    if single {
        out.push((
            "scrub",
            c.control
                .as_ref()
                .map(|k| k.kind == ControlKind::Scrub)
                .unwrap_or(false),
        ));
        out.push((
            "slider",
            c.control
                .as_ref()
                .map(|k| k.kind == ControlKind::Slider)
                .unwrap_or(false),
        ));
    }
    out.into_iter()
        .map(|(id, on)| Choice {
            id: id.into(),
            label: s(state, &format!("feature_{id}")),
            on,
        })
        .collect()
}

fn controls(state: &State, m: &Model, values: &[(String, Fx)]) -> Vec<ControlUi> {
    let view = view_of(state);
    let mut out = Vec::new();
    let text = |v: Fx, step: Fx| value_text(v, step);
    for c in &m.sheet.concepts {
        let Some(ctl) = &c.control else { continue };
        let name = c.value_name();
        let v = values
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
            .unwrap_or(ctl.value);
        let colour = paint(view, c).text.to_string();
        let base = ControlUi {
            kind: String::new(),
            num: c.num,
            name: name.clone(),
            label: c.name.clone(),
            colour,
            min: text(ctl.min, ctl.step),
            max: text(ctl.max, ctl.step),
            step: text(ctl.step, ctl.step),
            value: text(v, ctl.step),
            lo: String::new(),
            hi: String::new(),
        };
        if ctl.kind == ControlKind::Slider {
            out.push(ControlUi {
                kind: "slider".into(),
                ..base.clone()
            });
        }
        if let Some((lo, hi)) = c.limit {
            let pick = |key: &str, d: Fx| {
                state
                    .overrides
                    .iter()
                    .find(|(n, _)| *n == format!("{key}:{name}"))
                    .and_then(|(_, v)| Fx::parse(v))
                    .unwrap_or(d)
            };
            let (l, h) = (pick("lo", lo), pick("hi", hi));
            out.push(ControlUi {
                kind: "limit".into(),
                label: fill(&s(state, "limit_from"), &[("name", c.name.clone())]),
                lo: text(l, ctl.step),
                hi: text(h, ctl.step),
                ..base
            });
        }
    }
    for h in &m.sheet.hidden {
        let v = values
            .iter()
            .find(|(n, _)| *n == h.name)
            .map(|(_, v)| *v)
            .unwrap_or(h.control.value);
        let st = h.control.step;
        out.push(ControlUi {
            kind: "hidden".into(),
            num: 0,
            name: h.name.clone(),
            label: h.label.clone(),
            colour: crate::palette::INK.into(),
            min: text(h.control.min, st),
            max: text(h.control.max, st),
            step: text(st, st),
            value: text(v, st),
            lo: String::new(),
            hi: String::new(),
        });
    }
    out
}

fn ui(state: State, hints: Vec<Hint>, out: Option<Out>) -> Ui {
    let m = Model::build(&state.doc);
    let view = view_of(&state);
    let lk = look(&state, &m, true);
    let values = m.values(&values_of(&state.overrides), lk.step);
    let formula_svg = formula_view(&state, &m);
    let (plot_svg, plot_error) = match plot_svg(&m, &values, &lk) {
        Some(Ok(p)) => (p, String::new()),
        Some(Err(e)) => (String::new(), e),
        None => (String::new(), String::new()),
    };
    let prose = parse_prose(&state.doc.prose)
        .into_iter()
        .map(|seg| {
            let ci = seg.concept.as_deref().and_then(|c| m.concept_index(c));
            match ci {
                Some(ci) => {
                    let c = &m.sheet.concepts[ci];
                    ProseSeg {
                        text: seg.text,
                        num: c.num,
                        colour: paint(view, c).text.into(),
                        line: if c.pattern.name() == "wave" {
                            "wavy".into()
                        } else {
                            c.pattern.name().into()
                        },
                        hovered: state.hover == Some(ci),
                    }
                }
                None => ProseSeg {
                    text: seg.text,
                    num: 0,
                    colour: String::new(),
                    line: String::new(),
                    hovered: false,
                },
            }
        })
        .collect();
    let concepts = m
        .sheet
        .concepts
        .iter()
        .enumerate()
        .map(|(ci, c)| {
            let p = paint(view, c);
            ConceptUi {
                num: c.num,
                id: c.id.clone(),
                name: c.name.clone(),
                say: c.say.clone(),
                colour: p.text.into(),
                sample: pattern_sample(c.pattern, p.text),
                pattern: c.pattern.name().into(),
                hovered: state.hover == Some(ci),
                muted: state.muted.contains(&ci),
                features: features(&state, &m, ci),
            }
        })
        .collect();
    let used = m
        .sheet
        .concepts
        .iter()
        .filter(|c| c.colour)
        .count()
        .min(COLOUR_BUDGET);
    let budget = fill(
        &s(&state, "budget"),
        &[("used", used.to_string()), ("max", COLOUR_BUDGET.to_string())],
    );
    let steps = m
        .sheet
        .steps
        .iter()
        .enumerate()
        .map(|(i, st)| Choice {
            id: i.to_string(),
            label: st.caption.clone(),
            on: lk.step == Some(i),
        })
        .collect();
    let selection = state
        .select
        .and_then(|(f, id)| {
            let root = m.roots.get(f)?.as_ref()?;
            let n = root.find(id)?;
            m.lines[f].get(n.span.0..n.span.1).map(String::from)
        })
        .unwrap_or_default();
    let views = View::ALL
        .iter()
        .map(|v| Choice {
            id: v.name().into(),
            label: s(&state, &format!("view_{}", v.name())),
            on: *v == view,
        })
        .collect();
    let examples = state
        .examples
        .iter()
        .map(|e| Choice {
            id: e.id.clone(),
            label: e.label.clone(),
            on: e.doc == state.doc,
        })
        .collect();
    Ui {
        mode: state.mode.clone(),
        strings: state.strings.clone(),
        hints,
        doc: state.doc.clone(),
        fragment: export::fragment(&state.doc),
        view: view.name().into(),
        views,
        examples,
        errors: m.errors.clone(),
        formula_svg,
        plot_svg,
        plot_error,
        prose,
        concepts,
        budget,
        budget_used: used,
        budget_max: COLOUR_BUDGET,
        controls: controls(&state, &m, &values),
        steps,
        playing: state.playing,
        export_clear: state.export_clear,
        selection,
        out,
        state,
    }
}

fn hints_from(glossary_md: &str) -> Result<Vec<Hint>, String> {
    let entries = glossary::parse(glossary_md).map_err(|e| e.message())?;
    Ok(entries
        .into_iter()
        .map(|e| Hint {
            term: e.term,
            definition: e.definition,
        })
        .collect())
}

/// Checks the strings file, the examples and the glossary and returns the
/// first screen. `mode` is `edit` for the editor and `embed` for an embed.
pub fn start(strings_json: &str, examples_json: &str, glossary_md: &str, mode: &str) -> String {
    let run = || -> Result<Ui, String> {
        let strings = page::parse_strings(strings_json).map_err(|e| e.message())?;
        let examples: Vec<Example> = serde_json::from_str(examples_json)
            .map_err(|e| format!("The examples file could not be read: {e}"))?;
        let first = examples.first().ok_or("The examples file has no examples.")?;
        let hints = hints_from(glossary_md)?;
        let state = State {
            mode: if mode == "embed" {
                "embed".into()
            } else {
                "edit".into()
            },
            site: strings.get("site").cloned().unwrap_or_default(),
            strings,
            doc: first.doc.clone(),
            examples,
            view: "colour".into(),
            step: None,
            hover: None,
            select: None,
            overrides: vec![],
            scrub: None,
            ghost: vec![],
            playing: false,
            tick: 0,
            play_t0: None,
            export_px: 1200,
            muted: vec![],
            export_clear: false,
        };
        Ok(ui(state, hints, None))
    };
    match run() {
        Ok(u) => ok(&u),
        Err(e) => error(e),
    }
}

fn set_override(state: &mut State, name: &str, v: Fx) {
    state.overrides.retain(|(n, _)| n != name);
    state.overrides.push((name.to_string(), v.format(9)));
}

/// Clamps a reader's value to its control's range and the current limit, and
/// snaps it to the control's step.
fn settle(state: &State, m: &Model, name: &str, v: Fx) -> Option<Fx> {
    let (ctl, limit) = m
        .sheet
        .concepts
        .iter()
        .find(|c| c.control.is_some() && c.value_name() == name)
        .map(|c| (c.control.clone(), c.limit))
        .or_else(|| {
            m.sheet
                .hidden
                .iter()
                .find(|h| h.name == name)
                .map(|h| (Some(h.control.clone()), None))
        })?;
    let ctl = ctl?;
    // a typed value keeps what was typed; sliders and scrubs land on steps
    let k = (v.0 - ctl.min.0 + ctl.step.0 / 2).div_euclid(ctl.step.0);
    let snapped = if ctl.kind == ControlKind::Edit {
        v
    } else {
        Fx(ctl.min.0 + k * ctl.step.0)
    };
    let mut v = snapped.max(ctl.min).min(ctl.max);
    if limit.is_some() {
        let get = |key: &str, d: Fx| {
            state
                .overrides
                .iter()
                .find(|(n, _)| *n == format!("{key}:{name}"))
                .and_then(|(_, x)| Fx::parse(x))
                .unwrap_or(d)
        };
        let (lo, hi) = limit.unwrap_or((ctl.min, ctl.max));
        v = v.max(get("lo", lo)).min(get("hi", hi));
    }
    Some(v)
}

fn concept_block(sheet: &str, line: usize) -> (Vec<String>, usize) {
    let lines: Vec<String> = sheet.lines().map(String::from).collect();
    let mut end = line;
    while end < lines.len()
        && (lines[end].starts_with(' ') || lines[end].starts_with('\t') || lines[end].trim().is_empty())
    {
        if lines[end].trim().is_empty() {
            break;
        }
        end += 1;
    }
    (lines, end)
}

fn toggle(sheet: &str, line: usize, keyword: &str, add: &str, also_remove: &[&str]) -> String {
    let (mut lines, end) = concept_block(sheet, line);
    let found: Vec<usize> = (line..end)
        .filter(|i| lines[*i].split_whitespace().next() == Some(keyword))
        .collect();
    if found.is_empty() {
        let drop: Vec<usize> = (line..end)
            .filter(|i| also_remove.contains(&lines[*i].split_whitespace().next().unwrap_or("")))
            .collect();
        for i in drop.iter().rev() {
            lines.remove(*i);
        }
        lines.insert(end - drop.len(), format!("  {add}"));
    } else {
        for i in found.iter().rev() {
            lines.remove(*i);
        }
    }
    lines.join("\n") + "\n"
}

/// The nodes a click can select, smallest first: each node that sits in a
/// row, from the clicked glyph outward (the paper's R10: snap to meaningful
/// expressions, not glyphs).
fn selectable(root: &Node, id: usize) -> Vec<usize> {
    let path = root.path_to(id);
    let mut out = Vec::new();
    for w in path.windows(2) {
        let parent = root.find(w[0]);
        if parent.map(|p| p.k == K::Row).unwrap_or(false) {
            out.push(w[1]);
        }
    }
    out.reverse();
    out
}

fn gif_frames(state: &State, m: &Model) -> Out {
    let base = Look {
        interactive: false,
        hover: None,
        select: None,
        ..look(state, m, false)
    };
    let overrides = values_of(&state.overrides);
    let n = m.sheet.steps.len();
    let scenes: Vec<Scene> = if n == 0 {
        vec![scene(m, &m.values(&overrides, None), &base, &m.shown(None))]
    } else {
        (0..n)
            .map(|i| {
                scene(
                    m,
                    &m.values(&overrides, Some(i)),
                    &Look {
                        step: Some(i),
                        ..base.clone()
                    },
                    &m.shown(Some(i)),
                )
            })
            .collect()
    };
    let w = scenes.iter().map(|s| s.w).max().unwrap_or(1000);
    let h = scenes.iter().map(|s| s.h).max().unwrap_or(1000);
    let px = state.export_px.clamp(200, 2400);
    let ex = Look {
        export_px: Some(px),
        ..base.clone()
    };
    let mut frames = Vec::new();
    for (i, sc) in scenes.iter().enumerate() {
        let lk = Look {
            step: if n == 0 { None } else { Some(i) },
            ..ex.clone()
        };
        frames.push(Frame {
            svg: svg(m, &padded(scene_copy(sc), w, h), &lk),
            delay: GIF_HOLD,
        });
        if i + 1 < scenes.len() {
            for f in 1..GIF_MORPH_FRAMES {
                let mid = morph(sc, &scenes[i + 1], f * 1000 / GIF_MORPH_FRAMES, m);
                frames.push(Frame {
                    svg: svg(m, &padded(mid, w, h), &lk),
                    delay: GIF_MORPH_DELAY,
                });
            }
        }
    }
    Out {
        kind: "gif".into(),
        name: format!("{}.gif", file_stem(&state.doc.title)),
        mime: "image/gif".into(),
        text: String::new(),
        frames,
        width: px,
        height: (h as i64 * px as i64 / w.max(1) as i64) as i32,
    }
}

fn scene_copy(s: &Scene) -> Scene {
    Scene {
        w: s.w,
        h: s.h,
        drawn: s.drawn.clone(),
        under: s.under.clone(),
        over: s.over.clone(),
        hits: String::new(),
        alt: s.alt.clone(),
    }
}

fn file_stem(title: &str) -> String {
    let t: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let t = t
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if t.is_empty() {
        "formula".into()
    } else {
        t
    }
}

fn picture(state: &State, m: &Model) -> (String, i32, i32) {
    let base = Look {
        interactive: false,
        hover: None,
        select: None,
        ..look(state, m, false)
    };
    let values = m.values(&values_of(&state.overrides), base.step);
    let sc = scene(m, &values, &base, &m.shown(base.step));
    let px = state.export_px.clamp(200, 2400);
    let h = (sc.h as i64 * px as i64 / sc.w.max(1) as i64) as i32;
    (
        svg(
            m,
            &sc,
            &Look {
                export_px: Some(px),
                ..base
            },
        ),
        px,
        h,
    )
}

fn text_out(kind: &str, name: String, mime: &str, text: String) -> Out {
    Out {
        kind: kind.into(),
        name,
        mime: mime.into(),
        text,
        frames: vec![],
        width: 0,
        height: 0,
    }
}

/// Applies one named action to the state the page holds.
pub fn act(state_json: &str, glossary_md: &str, name: &str, value: &str) -> String {
    let run = || -> Result<Ui, String> {
        let mut state: State =
            serde_json::from_str(state_json).map_err(|e| format!("The state could not be read: {e}"))?;
        let hints = hints_from(glossary_md)?;
        let m = Model::build(&state.doc);
        let mut out = None;
        let num_to_index = |v: &str| {
            v.trim()
                .parse::<usize>()
                .ok()
                .filter(|n| *n >= 1 && *n <= m.sheet.concepts.len())
                .map(|n| n - 1)
        };
        match name {
            "doc.title" | "doc.formulas" | "doc.sheet" | "doc.prose" => {
                match name {
                    "doc.title" => state.doc.title = value.to_string(),
                    "doc.formulas" => state.doc.formulas = value.to_string(),
                    "doc.sheet" => state.doc.sheet = value.to_string(),
                    _ => state.doc.prose = value.to_string(),
                }
                state.select = None;
                state.hover = None;
            }
            "doc.example" => {
                let e = state
                    .examples
                    .iter()
                    .find(|e| e.id == value)
                    .ok_or(format!("There is no example \"{value}\"."))?;
                state.doc = e.doc.clone();
                reset(&mut state);
            }
            "doc.new" => {
                state.doc = Doc {
                    title: s(&state, "new_title"),
                    formulas: s(&state, "new_formula"),
                    sheet: String::new(),
                    prose: String::new(),
                };
                reset(&mut state);
            }
            "doc.fragment" => {
                state.doc = export::from_fragment(value)?;
                reset(&mut state);
            }
            "doc.open" => {
                state.doc = export::from_file(value)?;
                reset(&mut state);
            }
            "view.set" => {
                View::from(value).ok_or(format!("There is no view \"{value}\"."))?;
                state.view = value.to_string();
            }
            "hover" => state.hover = num_to_index(value),
            "select" => {
                let (f, id) = value
                    .split_once(':')
                    .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
                    .ok_or("A selection is written formula:node.")?;
                let root = m
                    .roots
                    .get(f)
                    .and_then(|r| r.as_ref())
                    .ok_or("There is no such formula.")?;
                let options = selectable(root, id);
                let next = match state.select {
                    Some((sf, sid)) if sf == f => match options.iter().position(|o| *o == sid) {
                        Some(i) => options.get(i + 1).or(options.first()).copied(),
                        None => options.first().copied(),
                    },
                    _ => options.first().copied(),
                };
                state.select = next.map(|n| (f, n));
            }
            "select.clear" => state.select = None,
            "concept.add" => {
                let (f, id) = state.select.ok_or(s(&state, "no_selection"))?;
                let root = m.roots[f].as_ref().ok_or("There is no such formula.")?;
                let n = root
                    .find(id)
                    .ok_or("The selection is no longer in the formula.")?;
                let src = m.lines[f][n.span.0..n.span.1].trim().to_string();
                let mut k = m.sheet.concepts.len() + 1;
                while m.sheet.concept(&format!("idea{k}")).is_some() {
                    k += 1;
                }
                let mut sheet = state.doc.sheet.trim_end().to_string();
                if !sheet.is_empty() {
                    sheet += "\n";
                }
                sheet += &format!("concept idea{k}: {src}\n  label above\n");
                state.doc.sheet = sheet;
                state.select = None;
            }
            "concept.toggle" => {
                let (num, feature) = value
                    .split_once(' ')
                    .ok_or("A toggle is written number feature.")?;
                let ci = num_to_index(num).ok_or("There is no such concept.")?;
                let c = &m.sheet.concepts[ci];
                let line = c.line;
                let range = "0 .. 10 step 1 = 1";
                state.doc.sheet = match feature {
                    "label" => toggle(&state.doc.sheet, line, "label", "label above", &[]),
                    "brace" => toggle(&state.doc.sheet, line, "brace", "brace below", &[]),
                    "border" | "tint" | "pointer" | "space" => {
                        toggle(&state.doc.sheet, line, feature, feature, &[])
                    }
                    "scrub" => toggle(
                        &state.doc.sheet,
                        line,
                        "scrub",
                        &format!("scrub {range}"),
                        &["slider", "edit"],
                    ),
                    "slider" => toggle(
                        &state.doc.sheet,
                        line,
                        "slider",
                        &format!("slider {range}"),
                        &["scrub", "edit"],
                    ),
                    other => return Err(format!("A concept has no feature \"{other}\".")),
                };
            }
            "concept.mute" => {
                let ci = num_to_index(value).ok_or("There is no such concept.")?;
                if state.muted.contains(&ci) {
                    state.muted.retain(|c| *c != ci);
                } else {
                    state.muted.push(ci);
                }
            }
            "export.clear" => state.export_clear = !state.export_clear,
            "concept.remove" => {
                let ci = num_to_index(value).ok_or("There is no such concept.")?;
                let (mut lines, end) = concept_block(&state.doc.sheet, m.sheet.concepts[ci].line);
                lines.drain(m.sheet.concepts[ci].line - 1..end);
                state.doc.sheet = lines.join("\n") + "\n";
                state.hover = None;
            }
            "value.set" => {
                let (n, v) = value.split_once('|').ok_or("A value is written name|value.")?;
                let v = Fx::parse(v).ok_or(format!("{v} is not a number."))?;
                let v = settle(&state, &m, n, v).ok_or(format!("Nothing in the sheet sets {n}."))?;
                set_override(&mut state, n, v);
            }
            "limit.set" => {
                // lo|name|value or hi|name|value
                let parts: Vec<&str> = value.splitn(3, '|').collect();
                let [side, n, v] = parts[..] else {
                    return Err("A limit is written lo|name|value.".into());
                };
                let v = Fx::parse(v).ok_or(format!("{v} is not a number."))?;
                set_override(&mut state, &format!("{side}:{n}"), v);
                let cur = m
                    .values(&values_of(&state.overrides), None)
                    .into_iter()
                    .find(|(x, _)| x == n)
                    .map(|(_, v)| v);
                if let Some(cur) = cur.and_then(|c| settle(&state, &m, n, c)) {
                    set_override(&mut state, n, cur);
                }
            }
            "scrub.start" => {
                let (num, x) = value.split_once(' ').ok_or("A scrub starts with number x.")?;
                let ci = num_to_index(num).ok_or("There is no such concept.")?;
                let x = x
                    .split('.')
                    .next()
                    .and_then(|x| x.parse::<i64>().ok())
                    .ok_or("The pointer position is not a number.")?;
                let vn = m.sheet.concepts[ci].value_name();
                let v = m
                    .values(&values_of(&state.overrides), state.step)
                    .into_iter()
                    .find(|(n, _)| *n == vn)
                    .map(|(_, v)| v);
                let v = v.ok_or("This concept has no value to scrub.")?;
                state.scrub = Some((ci, x, v.format(9)));
                state.ghost = m
                    .values(&values_of(&state.overrides), state.step)
                    .iter()
                    .map(|(n, v)| (n.clone(), v.format(9)))
                    .collect();
            }
            "scrub.move" => {
                if let Some((ci, x0, v0)) = state.scrub.clone() {
                    let x = value
                        .split('.')
                        .next()
                        .and_then(|x| x.parse::<i64>().ok())
                        .ok_or("The pointer position is not a number.")?;
                    let c = &m.sheet.concepts[ci];
                    let ctl = c.control.as_ref().ok_or("This concept has no value to scrub.")?;
                    let start = Fx::parse(&v0).unwrap_or(ctl.value);
                    let steps = (x - x0) / SCRUB_PX;
                    let v = Fx(start.0 + steps * ctl.step.0);
                    if let Some(v) = settle(&state, &m, &c.value_name(), v) {
                        set_override(&mut state, &c.value_name(), v);
                    }
                }
            }
            "scrub.end" => {
                state.scrub = None;
                state.ghost.clear();
            }
            "scrub.key" => {
                let (num, dir) = value
                    .split_once(' ')
                    .ok_or("A key scrub is written number up|down.")?;
                let ci = num_to_index(num).ok_or("There is no such concept.")?;
                let c = &m.sheet.concepts[ci];
                let ctl = c.control.as_ref().ok_or("This concept has no value.")?;
                let vn = c.value_name();
                let cur = m
                    .values(&values_of(&state.overrides), state.step)
                    .into_iter()
                    .find(|(n, _)| *n == vn)
                    .map(|(_, v)| v)
                    .unwrap_or(ctl.value);
                let v = if dir == "up" {
                    Fx(cur.0 + ctl.step.0)
                } else {
                    Fx(cur.0 - ctl.step.0)
                };
                if let Some(v) = settle(&state, &m, &vn, v) {
                    set_override(&mut state, &vn, v);
                }
            }
            "step.go" => {
                let i: usize = value.parse().map_err(|_| "A step is a number.")?;
                if i >= m.sheet.steps.len() {
                    return Err(format!("There is no step {}.", i + 1));
                }
                state.step = Some(i);
                state.playing = false;
            }
            "step.next" | "step.prev" => {
                let n = m.sheet.steps.len();
                if n > 0 {
                    let cur = state.step.unwrap_or(0);
                    state.step = Some(if name == "step.next" {
                        (cur + 1).min(n - 1)
                    } else {
                        cur.saturating_sub(1)
                    });
                    state.playing = false;
                }
            }
            "play" => {
                let n = m.sheet.steps.len();
                if n > 1 {
                    state.playing = !state.playing;
                    state.play_t0 = None;
                    if state.playing {
                        let from = if state.step.unwrap_or(0) + 1 >= n {
                            0
                        } else {
                            state.step.unwrap_or(0)
                        };
                        state.tick = from as u32 * (HOLD_TICKS + MORPH_TICKS);
                        state.step = Some(from);
                    }
                }
            }
            "tick" => {
                // the page sends its clock; ticks are sixtieths of a second
                // since play started, so a slow frame rate keeps the timing
                if state.playing {
                    let n = m.sheet.steps.len();
                    let now = value.split('.').next().and_then(|v| v.parse::<i64>().ok());
                    match (now, state.play_t0) {
                        (Some(now), Some(t0)) => {
                            state.tick = state.tick.max(((now - t0).max(0) * 60 / 1000) as u32)
                        }
                        (Some(now), None) => state.play_t0 = Some(now - state.tick as i64 * 1000 / 60),
                        (None, _) => state.tick += 1,
                    }
                    let (at, _) = timeline(&state, n);
                    state.step = Some(at);
                    let end = (n.max(1) as u32 - 1) * (HOLD_TICKS + MORPH_TICKS) + HOLD_TICKS;
                    if state.tick >= end {
                        state.playing = false;
                        state.step = Some(n.saturating_sub(1));
                    }
                }
            }
            "export.size" => {
                state.export_px = value
                    .parse::<i32>()
                    .map_err(|_| "A picture width is a number of pixels.")?
                    .clamp(200, 2400);
            }
            "export.svg" => {
                let (text, _, _) = picture(&state, &m);
                out = Some(text_out(
                    "download",
                    format!("{}.svg", file_stem(&state.doc.title)),
                    "image/svg+xml",
                    text,
                ));
            }
            "export.png" => {
                let (text, w, h) = picture(&state, &m);
                out = Some(Out {
                    kind: "png".into(),
                    name: format!("{}.png", file_stem(&state.doc.title)),
                    mime: "image/png".into(),
                    text,
                    frames: vec![],
                    width: w,
                    height: h,
                });
            }
            "export.gif" => out = Some(gif_frames(&state, &m)),
            "export.latex" => {
                let t = export::latex(&m, &s(&state, "export_header"));
                out = Some(text_out("copy", "latex".into(), "text/plain", t));
            }
            "export.embed" => {
                let (_, w, h) = picture(&state, &m);
                let height =
                    (h * 640 / w.max(1)).clamp(160, 900) + if m.sheet.plot.is_some() { 380 } else { 0 } + 140;
                let t = format!(
                    r#"<iframe src="{}embed.html{}" width="680" height="{height}" title="{}" style="border:0;max-width:100%" loading="lazy"></iframe>"#,
                    state.site,
                    export::fragment(&state.doc),
                    crate::render::esc(&fill(
                        &s(&state, "embed_title"),
                        &[("title", state.doc.title.clone())]
                    ))
                );
                out = Some(text_out("copy", "embed".into(), "text/html", t));
            }
            "export.link" => {
                let t = format!("{}{}", state.site, export::fragment(&state.doc));
                out = Some(text_out("copy", "link".into(), "text/plain", t));
            }
            "export.json" => {
                let t = serde_json::to_string_pretty(&state.doc).map_err(|e| e.to_string())?;
                out = Some(text_out(
                    "download",
                    format!("{}.formula.json", file_stem(&state.doc.title)),
                    "application/json",
                    t,
                ));
            }
            other => return Err(format!("Unknown action \"{other}\".")),
        }
        Ok(ui(state, hints, out))
    };
    match run() {
        Ok(u) => ok(&u),
        Err(e) => error(e),
    }
}

fn reset(state: &mut State) {
    state.step = None;
    state.hover = None;
    state.select = None;
    state.overrides.clear();
    state.scrub = None;
    state.ghost.clear();
    state.playing = false;
    state.tick = 0;
    state.play_t0 = None;
    state.muted.clear();
}

/// Turns the GIF frames the page rasterised into a GIF file: `rgba` holds
/// every frame's pixels in order, `delays` the frame times the core sent,
/// comma-separated. The bytes come back for the page to save.
pub fn gif(rgba: &[u8], width: u32, height: u32, delays: &str) -> Vec<u8> {
    let d: Vec<u16> = delays.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    crate::gif::encode(rgba, width.min(65535) as u16, height.min(65535) as u16, &d).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings() -> String {
        let mut pairs: Vec<String> = page::REQUIRED
            .iter()
            .map(|k| format!("\"{k}\": \"{k}\""))
            .collect();
        pairs.push("\"budget\": \"{used} of {max}\"".into());
        pairs.retain(|p| p != "\"budget\": \"budget\"");
        pairs.push("\"site\": \"https://example.org/fb/\"".into());
        format!("{{{}}}", pairs.join(","))
    }

    const EXAMPLES: &str = r#"[{"id":"line","label":"Line","doc":{"title":"A line","formulas":"y = m x + b","sheet":"concept slope: m\n  scrub -3 .. 3 step 0.5 = 2\n  limit -1 .. 2\nconcept icpt: b\n  slider -5 .. 5 step 1 = 1\nstep one\nstep two\n  set b = 3\n","prose":"The [slope](slope) tilts it."}}]"#;

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    fn begin() -> serde_json::Value {
        json(&start(&strings(), EXAMPLES, "", "edit"))
    }

    fn step(v: &serde_json::Value, name: &str, value: &str) -> serde_json::Value {
        let state = v["ok"]["state"].to_string();
        json(&act(&state, "", name, value))
    }

    #[test]
    fn start_draws_the_first_example() {
        let v = begin();
        let ui = &v["ok"];
        assert_eq!(ui["doc"]["title"], "A line");
        assert!(ui["formula_svg"].as_str().unwrap().starts_with("<svg"));
        assert_eq!(ui["concepts"][0]["name"], "slope");
        assert_eq!(ui["budget"], "2 of 4");
        assert_eq!(ui["prose"][1]["num"], 1);
        assert_eq!(
            ui["controls"][0]["kind"], "limit",
            "slope is scrubbed in place, within a limit"
        );
        assert_eq!(ui["controls"][1]["kind"], "slider", "icpt has a slider");
        assert_eq!(ui["steps"][0]["on"], true);
    }

    #[test]
    fn scrubbing_moves_by_steps_and_stays_inside_the_limit() {
        let v = begin();
        let v = step(&v, "scrub.start", "1 100");
        // 6 px per step of 0.5: 12 px is one whole: 2 + 1 = 3, held to the limit of 2
        let v = step(&v, "scrub.move", "112");
        assert_eq!(v["ok"]["state"]["overrides"][0][1], "2.000000000");
        let v = step(&v, "scrub.move", "88");
        assert_eq!(v["ok"]["state"]["overrides"][0][1], "1.000000000");
        let v = step(&v, "scrub.end", "");
        assert_eq!(v["ok"]["state"]["scrub"], serde_json::Value::Null);
        let v = step(&v, "scrub.key", "1 down");
        assert_eq!(v["ok"]["state"]["overrides"][0][1], "0.500000000");
    }

    #[test]
    fn a_slider_value_snaps_to_its_step_and_range() {
        let v = step(&begin(), "value.set", "b|2.6");
        assert_eq!(v["ok"]["controls"][1]["value"], "3");
        let v = step(&v, "value.set", "b|99");
        assert_eq!(v["ok"]["controls"][1]["value"], "5");
        assert_eq!(
            step(&v, "value.set", "q|1")["error"],
            "Nothing in the sheet sets q."
        );
    }

    #[test]
    fn clicks_select_ever_larger_parts_and_a_selection_becomes_a_concept() {
        let v = begin();
        // y = m x + b: root 0, y 1, = 2, m 3, x 4, + 5, b 6
        let v = step(&v, "select", "0:3");
        assert_eq!(v["ok"]["selection"], "m");
        let v = step(&v, "concept.add", "");
        assert!(v["ok"]["doc"]["sheet"]
            .as_str()
            .unwrap()
            .ends_with("concept idea3: m\n  label above\n"));
    }

    #[test]
    fn toggles_edit_the_sheet_text() {
        let v = step(&begin(), "concept.toggle", "2 border");
        assert!(v["ok"]["doc"]["sheet"]
            .as_str()
            .unwrap()
            .contains("concept icpt: b\n  slider -5 .. 5 step 1 = 1\n  border\n"));
        let v = step(&v, "concept.toggle", "2 border");
        assert!(!v["ok"]["doc"]["sheet"].as_str().unwrap().contains("border"));
        let v = step(&v, "concept.toggle", "2 scrub");
        let sheet = v["ok"]["doc"]["sheet"].as_str().unwrap().to_string();
        assert!(
            sheet.contains("  scrub 0 .. 10 step 1 = 1") && !sheet.contains("slider"),
            "scrub replaces the slider: {sheet}"
        );
    }

    #[test]
    fn steps_play_through_and_stop_on_the_last() {
        let mut v = step(&begin(), "play", "");
        assert_eq!(v["ok"]["playing"], true);
        for _ in 0..(HOLD_TICKS + MORPH_TICKS + HOLD_TICKS) {
            v = step(&v, "tick", "");
        }
        assert_eq!(v["ok"]["playing"], false);
        assert_eq!(v["ok"]["steps"][1]["on"], true);
        assert_eq!(v["ok"]["controls"][1]["value"], "3", "step two sets b to 3");
    }

    #[test]
    fn exports_arrive_once_and_carry_the_document() {
        let v = step(&begin(), "export.latex", "");
        assert_eq!(v["ok"]["out"]["kind"], "copy");
        assert!(v["ok"]["out"]["text"].as_str().unwrap().contains("\\fbslope{m}"));
        let embed = step(&v, "export.embed", "");
        let code = embed["ok"]["out"]["text"].as_str().unwrap().to_string();
        assert!(
            code.starts_with("<iframe src=\"https://example.org/fb/embed.html#f="),
            "{code}"
        );
        let gif = step(&v, "export.gif", "");
        assert_eq!(
            gif["ok"]["out"]["frames"].as_array().unwrap().len(),
            1 + GIF_MORPH_FRAMES as usize - 1 + 1
        );
        let again = step(&gif, "hover", "1");
        assert_eq!(
            again["ok"]["out"],
            serde_json::Value::Null,
            "an export is handed over once"
        );
        let frag = v["ok"]["fragment"].as_str().unwrap().to_string();
        let back = step(&step(&v, "doc.new", ""), "doc.fragment", &frag);
        assert_eq!(back["ok"]["doc"]["title"], "A line");
    }

    #[test]
    fn unknown_actions_and_bad_values_are_refused_by_name() {
        let v = begin();
        assert_eq!(
            step(&v, "screen.jump", "")["error"],
            "Unknown action \"screen.jump\"."
        );
        assert_eq!(
            step(&v, "view.set", "sepia")["error"],
            "There is no view \"sepia\"."
        );
        assert_eq!(
            step(&v, "doc.fragment", "#nope")["error"],
            "This link does not hold a formula."
        );
    }
}
