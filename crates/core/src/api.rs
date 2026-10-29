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
use crate::parts::{self, Run};
use crate::render::{morph, pattern_sample, plot_svg, scene, svg, value_text, Look, Scene};
use crate::sheet::{parse_prose, ControlKind, COLOUR_BUDGET};
use crate::sheetedit::{self, StepText};
use crate::tiles::{self, Cursor, Key};
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
    /// `shelf` for the sixth-grade formulas a student starts from; any other
    /// group is a field of mathematics, headed by its `field_` string
    #[serde(default)]
    pub group: String,
}

/// The three ways into the editor (journal K4).
pub const ROLES: [&str; 3] = ["explore", "build", "teach"];

fn build_role() -> String {
    "build".into()
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
    /// the selected part: formula, row, first sibling, how many (see parts.rs)
    pub select: Option<(usize, usize, usize, usize)>,
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
    /// explore, build or teach
    #[serde(default = "build_role")]
    pub role: String,
    /// the Teacher mode's switch that shows the LaTeX and the sheet
    #[serde(default)]
    pub code: bool,
    /// the line being written with tiles, and the cursor in it
    #[serde(default)]
    pub tiles: Option<(usize, String)>,
    /// the letter keys are open
    #[serde(default)]
    pub letters: bool,
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
    /// the value with its unit, for the readout
    pub shown: String,
    pub min: String,
    pub max: String,
    pub step: String,
    pub value: String,
    pub lo: String,
    pub hi: String,
}

/// The card for the selected part: sentences a student finishes with menus
/// and numbers, each writing one sheet line (journal K2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    /// the selected part, drawn on its own
    pub picture: String,
    /// its number as a part, or 0 before it is made one
    pub num: usize,
    pub name: String,
    pub colour: String,
    pub sample: String,
    pub words: Vec<Choice>,
    pub units: Vec<Choice>,
    /// one symbol, so readers can change its value
    pub number: bool,
    pub controls: Vec<Choice>,
    pub min: String,
    pub max: String,
    pub step: String,
    pub say: String,
    /// the words in the sentence that mean this part
    pub sentence_words: String,
    pub features: Vec<Choice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartChip {
    pub num: usize,
    pub name: String,
    pub colour: String,
    pub on: bool,
    /// the part's symbols are no longer in any formula
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TileLine {
    pub html: String,
    /// tiles can show this line; a root or subscript leaves it to the teacher
    pub editable: bool,
    pub on: bool,
}

/// An entry in the examples menu. `field` is the heading it sits under in
/// the Teacher mode, and empty for a student, whose menu is the shelf alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExampleUi {
    pub id: String,
    pub label: String,
    pub on: bool,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyUi {
    pub id: String,
    pub face: String,
    pub label: String,
    pub on: bool,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockAct {
    pub verb: String,
    pub label: String,
    pub arg: String,
    pub args: Vec<Choice>,
    pub value: String,
    pub value_too: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockStep {
    pub caption: String,
    pub on: bool,
    pub acts: Vec<BlockAct>,
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
    pub examples: Vec<ExampleUi>,
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
    pub role: String,
    pub roles: Vec<Choice>,
    /// the panels this mode shows, by their data-panel names
    pub shows: Vec<String>,
    pub code: bool,
    pub card: Option<Card>,
    pub parts: Vec<PartChip>,
    pub tiles: Vec<TileLine>,
    pub keys: Vec<KeyUi>,
    pub letters: bool,
    pub blocks: Vec<BlockStep>,
    pub verbs: Vec<Choice>,
    /// the prose without its link syntax
    pub sentence: String,
    pub state: State,
}

/// A string in the words of the current mode: outside the Teacher mode a
/// `kid_` key replaces the author's word (journal K4, the word table), and
/// in Explore an `explore_` key replaces both.
fn s(state: &State, key: &str) -> String {
    if state.role == "explore" {
        if let Some(v) = state.strings.get(&format!("explore_{key}")) {
            return v.clone();
        }
    }
    if state.role != "teach" {
        if let Some(v) = state.strings.get(&format!("kid_{key}")) {
            return v.clone();
        }
    }
    state.strings.get(key).cloned().unwrap_or_default()
}

fn strings_for(state: &State) -> Strings {
    let mut out = state.strings.clone();
    for k in state.strings.keys() {
        if let Some(base) = k.strip_prefix("kid_").or(k.strip_prefix("explore_")) {
            out.insert(base.to_string(), s(state, base));
        }
    }
    out
}

fn list(state: &State, key: &str) -> Vec<String> {
    s(state, key)
        .split(',')
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

/// A number as the sheet writes it: no trailing zeros, an ASCII minus.
fn num_text(v: Fx) -> String {
    let t = v.format(9);
    if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        t
    }
}

fn run_of(sel: (usize, usize, usize, usize)) -> Run {
    Run {
        f: sel.0,
        row: sel.1,
        first: sel.2,
        len: sel.3,
    }
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
            state.select.and_then(|sel| {
                let run = run_of(sel);
                let root = m.roots.get(run.f)?.as_ref()?;
                Some((run.f, run.nodes(root)))
            })
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
            shown: if c.unit.is_empty() {
                text(v, ctl.step)
            } else {
                format!("{} {}", text(v, ctl.step), c.unit)
            },
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
            shown: text(v, st),
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

/// The panels a mode shows, by the `data-panel` names in the page.
fn shows(state: &State) -> Vec<String> {
    let mut out: Vec<&str> = vec!["reader", "roles"];
    if state.role != "explore" {
        out.extend(["file", "tiles", "card", "blocks", "sentence", "exports"]);
    }
    if state.role == "teach" {
        out.extend(["teach", "exports-all", "concepts"]);
        if state.code {
            out.push("code");
        }
    }
    out.into_iter().map(String::from).collect()
}

/// A part drawn on its own, for the card.
fn picture_of(src: &str, view: View) -> String {
    let doc = Doc {
        formulas: src.into(),
        ..Doc::default()
    };
    let m = Model::build(&doc);
    let lk = Look {
        view,
        hover: None,
        select: None,
        step: None,
        interactive: false,
        export_px: None,
        ghost: vec![],
        muted: vec![],
        transparent: true,
    };
    let sc = scene(&m, &m.values(&[], None), &lk, &m.shown(None));
    svg(&m, &sc, &lk)
}

/// The concept the selection is an occurrence of, if any.
fn selected_concept(state: &State, m: &Model) -> Option<usize> {
    let (f, row, first, len) = state.select?;
    m.occs
        .iter()
        .find(|o| o.formula == f && o.row == row && o.first == first && o.len == len)
        .map(|o| o.concept)
}

fn control_kind(c: &crate::sheet::Concept) -> &'static str {
    match c.control.as_ref().map(|k| k.kind) {
        Some(ControlKind::Scrub) => "drag",
        Some(ControlKind::Slider) => "slider",
        Some(ControlKind::Edit) => "type",
        None => "none",
    }
}

fn choices(state: &State, items: &[String], key_prefix: Option<&str>, on: &str) -> Vec<Choice> {
    items
        .iter()
        .map(|id| Choice {
            id: id.clone(),
            label: match key_prefix {
                Some(p) => s(state, &format!("{p}{id}")),
                None => id.clone(),
            },
            on: id == on,
        })
        .collect()
}

fn card(state: &State, m: &Model) -> Option<Card> {
    if state.role == "explore" {
        return None;
    }
    let run = run_of(state.select?);
    let root = m.roots.get(run.f)?.as_ref()?;
    let src = run.source(root, &m.lines[run.f])?.to_string();
    let view = view_of(state);
    let picture = picture_of(&src, view);
    let mut words = list(state, "part_words");
    let units = {
        let mut u = vec![String::new()];
        u.extend(list(state, "part_units"));
        u
    };
    let unit_choices = |on: &str| -> Vec<Choice> {
        units
            .iter()
            .map(|u| Choice {
                id: u.clone(),
                label: if u.is_empty() {
                    s(state, "unit_none")
                } else {
                    u.clone()
                },
                on: u == on,
            })
            .collect()
    };
    let kinds: Vec<String> = ["none", "drag", "slider", "type"]
        .iter()
        .map(|k| k.to_string())
        .collect();
    let Some(ci) = selected_concept(state, m) else {
        return Some(Card {
            picture,
            num: 0,
            name: String::new(),
            colour: String::new(),
            sample: String::new(),
            words: {
                // a new part starts on no word, so a wrong one is not suggested
                let mut w = vec![Choice {
                    id: String::new(),
                    label: s(state, "card_pick"),
                    on: true,
                }];
                w.extend(choices(state, &words, None, ""));
                w
            },
            units: unit_choices(""),
            number: false,
            controls: vec![],
            min: String::new(),
            max: String::new(),
            step: String::new(),
            say: String::new(),
            sentence_words: String::new(),
            features: vec![],
        });
    };
    let c = &m.sheet.concepts[ci];
    if !words.contains(&c.name) {
        words.push(c.name.clone());
    }
    let p = paint(view, c);
    let (min, max, step) = c
        .control
        .as_ref()
        .map(|k| (num_text(k.min), num_text(k.max), num_text(k.step)))
        .unwrap_or_default();
    let number = m.occs.iter().any(|o| o.concept == ci && o.len == 1);
    let sentence_words = sheetedit::links(&state.doc.prose)
        .into_iter()
        .find(|(_, id)| *id == c.id)
        .map(|(w, _)| w)
        .unwrap_or_default();
    Some(Card {
        picture,
        num: c.num,
        name: c.name.clone(),
        colour: p.text.into(),
        sample: pattern_sample(c.pattern, p.text),
        words: choices(state, &words, None, &c.name),
        units: unit_choices(&c.unit),
        number,
        controls: if number {
            choices(state, &kinds, Some("control_"), control_kind(c))
        } else {
            vec![]
        },
        min,
        max,
        step,
        say: c.say.clone(),
        sentence_words,
        features: features(state, m, ci)
            .into_iter()
            .filter(|f| f.id != "scrub" && f.id != "slider" && f.id != "space")
            .collect(),
    })
}

fn part_chips(state: &State, m: &Model) -> Vec<PartChip> {
    let on = selected_concept(state, m);
    m.sheet
        .concepts
        .iter()
        .enumerate()
        .map(|(ci, c)| PartChip {
            num: c.num,
            name: c.name.clone(),
            colour: paint(view_of(state), c).text.into(),
            on: on == Some(ci),
            missing: !m.occs.iter().any(|o| o.concept == ci),
        })
        .collect()
}

/// The tile cursor: where the student put it, or else the end of the first
/// line, so the keys work before anything is tapped.
fn cursor_of(state: &State, m: &Model) -> Option<(usize, Cursor)> {
    if state.role == "explore" {
        return None;
    }
    match state.tiles.as_ref() {
        Some((line, text)) => Some((*line, Cursor::read(text)?)),
        None => {
            let t = tiles::from_line(m.lines.first()?)?;
            Some((
                0,
                Cursor {
                    path: vec![],
                    pos: t.len(),
                },
            ))
        }
    }
}

/// The keys on the keypad: (id, face, label key, group). The id is what
/// `tiles.key` takes.
fn key_table(state: &State) -> Vec<(String, String, String, &'static str)> {
    let mut out: Vec<(String, String, String, &'static str)> = Vec::new();
    for d in ["7", "8", "9", "4", "5", "6", "1", "2", "3", "0", "."] {
        out.push((d.into(), d.into(), d.into(), "digit"));
    }
    for (id, sym) in [
        ("+", "+"),
        ("-", "-"),
        ("*", "\\times"),
        ("div", "\\div"),
        ("=", "="),
    ] {
        out.push((id.into(), tiles::face(sym), tiles::face(sym), "op"));
    }
    for id in ["frac", "pow", "paren", "letters", "left", "right", "back"] {
        out.push((
            id.into(),
            s(state, &format!("face_{id}")),
            s(state, &format!("key_{id}")),
            "do",
        ));
    }
    if state.letters {
        for c in 'a'..='z' {
            out.push((c.to_string(), c.to_string(), c.to_string(), "letter"));
        }
        out.push(("ell".into(), tiles::face("\\ell"), tiles::face("\\ell"), "letter"));
    }
    out
}

fn key_of(id: &str) -> Option<Key<'_>> {
    match id {
        "div" => Some(Key::Op("\\div")),
        "ell" => Some(Key::Put("\\ell")),
        "frac" => Some(Key::Frac),
        "pow" => Some(Key::Pow),
        "paren" => Some(Key::Paren),
        "left" => Some(Key::Left),
        "right" => Some(Key::Right),
        "back" => Some(Key::Back),
        other => Key::from_typed(other),
    }
}

fn tiles_ui(state: &State, m: &Model) -> (Vec<TileLine>, Vec<KeyUi>) {
    let cur = cursor_of(state, m);
    let label = s(state, "tiles_box");
    let lines: Vec<TileLine> = m
        .lines
        .iter()
        .enumerate()
        .map(|(i, src)| match tiles::from_line(src) {
            Some(t) => TileLine {
                html: tiles::html(
                    &t,
                    i,
                    cur.as_ref().filter(|(l, _)| *l == i).map(|(_, c)| c),
                    &label,
                ),
                editable: true,
                on: cur.as_ref().map(|(l, _)| *l == i).unwrap_or(false),
            },
            None => TileLine {
                html: String::new(),
                editable: false,
                on: false,
            },
        })
        .collect();
    let line_tiles = cur
        .as_ref()
        .and_then(|(l, c)| Some((tiles::from_line(m.lines.get(*l)?)?, c.clone())));
    let keys = key_table(state)
        .into_iter()
        .map(|(id, face, label, group)| {
            let on = match (&line_tiles, id.as_str()) {
                (_, "letters") => true,
                (Some((t, c)), _) => key_of(&id).and_then(|k| press_line(t, c, k)).is_some(),
                (None, _) => false,
            };
            KeyUi {
                id,
                face,
                label,
                on,
                group: group.into(),
            }
        })
        .collect();
    (lines, keys)
}

/// A key on a line, refusing a key that would leave the line empty: a line
/// is removed with its own button, not by deleting its last tile.
fn press_line(t: &[tiles::Tile], c: &Cursor, k: Key) -> Option<(Vec<tiles::Tile>, Cursor)> {
    tiles::press(t, c, k).filter(|(after, _)| !after.is_empty())
}

const VERBS: [&str; 5] = ["show", "spotlight", "cross", "hide", "set"];

fn verb_of(line: &str) -> &'static str {
    match line.split_whitespace().next() {
        Some("formula") => "show",
        Some("focus") => "spotlight",
        Some("strike") => "cross",
        Some("hide") => "hide",
        Some("set") => "set",
        _ => "other",
    }
}

fn keyword_of(verb: &str) -> &'static str {
    match verb {
        "show" => "formula",
        "spotlight" => "focus",
        "cross" => "strike",
        "hide" => "hide",
        _ => "set",
    }
}

/// The step blocks, one line per block: a line naming several parts
/// becomes one block for each.
fn step_blocks(sheet: &str) -> Vec<StepText> {
    sheetedit::steps(sheet)
        .into_iter()
        .map(|st| StepText {
            caption: st.caption,
            lines: st
                .lines
                .iter()
                .flat_map(|l| {
                    let mut w = l.split_whitespace();
                    let first = w.next().unwrap_or("");
                    if matches!(first, "focus" | "strike" | "hide") {
                        w.map(|n| format!("{first} {}", n.trim_matches(',')))
                            .collect::<Vec<_>>()
                    } else {
                        vec![l.clone()]
                    }
                })
                .collect(),
        })
        .collect()
}

fn block_args(state: &State, m: &Model, verb: &str) -> Vec<Choice> {
    match verb {
        "show" => (1..=m.lines.len())
            .map(|n| Choice {
                id: n.to_string(),
                label: fill(&s(state, "block_line"), &[("n", n.to_string())]),
                on: false,
            })
            .collect(),
        "set" => m
            .sheet
            .concepts
            .iter()
            .filter(|c| c.control.is_some())
            .map(|c| Choice {
                id: c.value_name(),
                label: c.name.clone(),
                on: false,
            })
            .collect(),
        "other" => vec![],
        _ => m
            .sheet
            .concepts
            .iter()
            .map(|c| Choice {
                id: c.id.clone(),
                label: c.name.clone(),
                on: false,
            })
            .collect(),
    }
}

fn blocks_ui(state: &State, m: &Model, current: Option<usize>) -> Vec<BlockStep> {
    step_blocks(&state.doc.sheet)
        .into_iter()
        .enumerate()
        .map(|(i, st)| BlockStep {
            caption: st.caption,
            on: current == Some(i),
            acts: st
                .lines
                .iter()
                .map(|l| {
                    let verb = verb_of(l);
                    let words: Vec<&str> = l.split_whitespace().collect();
                    let arg = words.get(1).map(|w| w.to_string()).unwrap_or_default();
                    let arg = if verb == "set" {
                        crate::tex::parse(&arg)
                            .map(|n| crate::tex::canon(&n))
                            .unwrap_or(arg)
                    } else {
                        arg
                    };
                    let args = block_args(state, m, verb)
                        .into_iter()
                        .map(|c| Choice { on: c.id == arg, ..c })
                        .collect();
                    BlockAct {
                        verb: verb.into(),
                        label: s(state, &format!("verb_{verb}")),
                        arg: if verb == "other" { String::new() } else { arg },
                        args,
                        value: if verb == "set" {
                            words.get(3).map(|w| w.to_string()).unwrap_or_default()
                        } else {
                            String::new()
                        },
                        value_too: verb == "set",
                    }
                })
                .collect(),
        })
        .collect()
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
        .and_then(|sel| {
            let run = run_of(sel);
            let root = m.roots.get(run.f)?.as_ref()?;
            run.source(root, &m.lines[run.f]).map(String::from)
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
        .filter(|e| state.role == "teach" || e.group == "shelf")
        .map(|e| ExampleUi {
            id: e.id.clone(),
            label: e.label.clone(),
            on: e.doc == state.doc,
            field: if state.role == "teach" {
                s(&state, &format!("field_{}", e.group))
            } else {
                String::new()
            },
        })
        .collect();
    let (tiles, keys) = tiles_ui(&state, &m);
    let roles = ROLES
        .iter()
        .map(|r| Choice {
            id: r.to_string(),
            label: s(&state, &format!("role_{r}")),
            on: state.role == *r,
        })
        .collect();
    let verbs = choices(
        &state,
        &VERBS.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
        Some("verb_"),
        "",
    );
    // a student cannot read or fix a sheet line, so outside the Teacher mode
    // the errors are left to the teacher, apart from a missing part (shown
    // on its chip)
    let errors = if state.role == "teach" {
        m.errors.clone()
    } else {
        vec![]
    };
    Ui {
        mode: state.mode.clone(),
        strings: strings_for(&state),
        hints,
        doc: state.doc.clone(),
        fragment: export::fragment(&state.doc),
        view: view.name().into(),
        views,
        examples,
        errors,
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
        role: state.role.clone(),
        roles,
        shows: shows(&state),
        code: state.code,
        card: card(&state, &m),
        parts: part_chips(&state, &m),
        tiles,
        keys,
        letters: state.letters,
        blocks: blocks_ui(&state, &m, lk.step),
        verbs,
        sentence: sheetedit::plain(&state.doc.prose),
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
        let role = if mode == "embed" { "explore" } else { "build" };
        // a student starts on the shelf; the teacher's examples come after
        let first = examples
            .iter()
            .find(|e| e.group == "shelf")
            .or(examples.first())
            .ok_or("The examples file has no examples.")?;
        let hints = hints_from(glossary_md)?;
        let state = State {
            mode: if mode == "embed" {
                "embed".into()
            } else {
                "edit".into()
            },
            role: role.into(),
            code: false,
            tiles: None,
            letters: false,
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
                    open: String::new(),
                };
                reset(&mut state);
            }
            "doc.fragment" => {
                state.doc = export::from_fragment(value)?;
                // a teacher's link says which mode it opens in
                if state.mode == "edit" && (state.doc.open == "explore" || state.doc.open == "build") {
                    state.role = state.doc.open.clone();
                }
                state.doc.open.clear();
                reset(&mut state);
            }
            "role.set" => {
                if !ROLES.contains(&value) {
                    return Err(format!("There is no mode \"{value}\"."));
                }
                state.role = value.to_string();
                state.select = None;
                state.tiles = None;
            }
            "code.toggle" => state.code = !state.code,
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
                // formula:node, or formula:node|row:first:len for a tap on a part
                let (value, part) = value.split_once('|').unwrap_or((value, ""));
                let (f, id) = value
                    .split_once(':')
                    .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
                    .ok_or("A selection is written formula:node.")?;
                let root = m
                    .roots
                    .get(f)
                    .and_then(|r| r.as_ref())
                    .ok_or("There is no such formula.")?;
                // each tap on the same place selects the next larger part
                let options = parts::choices(root, f, id);
                let cur = state.select.map(run_of);
                let tapped: Option<Run> = {
                    let n: Vec<usize> = part.split(':').filter_map(|x| x.parse().ok()).collect();
                    (n.len() == 3).then(|| Run {
                        f,
                        row: n[0],
                        first: n[1],
                        len: n[2],
                    })
                };
                let next = match (tapped, cur) {
                    (Some(t), c) if c != Some(t) => Some(t),
                    _ => match cur.and_then(|c| options.iter().position(|o| *o == c)) {
                        Some(i) => options.get(i + 1).or(options.first()).copied(),
                        None => options.first().copied(),
                    },
                };
                state.select = if state.role == "explore" {
                    None
                } else {
                    next.map(|r| (r.f, r.row, r.first, r.len))
                };
            }
            "select.clear" => state.select = None,
            "concept.add" => {
                let run = run_of(state.select.ok_or(s(&state, "no_selection"))?);
                let root = m.roots[run.f].as_ref().ok_or("There is no such formula.")?;
                let src = run
                    .source(root, &m.lines[run.f])
                    .ok_or("The selection is no longer in the formula.")?
                    .to_string();
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
            "part.make" => {
                let run = run_of(state.select.ok_or(s(&state, "no_selection"))?);
                let root = m.roots[run.f].as_ref().ok_or("There is no such formula.")?;
                let src = run
                    .source(root, &m.lines[run.f])
                    .ok_or("The selection is no longer in the formula.")?
                    .to_string();
                let word = value.trim();
                let word = if word.is_empty() {
                    s(&state, "part_default")
                } else {
                    word.to_string()
                };
                let taken: Vec<String> = m.sheet.concepts.iter().map(|c| c.id.clone()).collect();
                let id = sheetedit::slug(&word, &taken);
                let mut sheet = state.doc.sheet.trim_end().to_string();
                if !sheet.is_empty() {
                    sheet += "\n";
                }
                sheet += &format!("concept {id}: {src}\n");
                if word != id {
                    sheet += &format!("  name {word}\n");
                }
                sheet += "  label above\n";
                state.doc.sheet = sheet;
            }
            "part.pick" => {
                let ci = num_to_index(value).ok_or("There is no such part.")?;
                let o = m
                    .occs
                    .iter()
                    .find(|o| o.concept == ci)
                    .ok_or(s(&state, "part_missing"))?;
                state.select = Some((o.formula, o.row, o.first, o.len));
            }
            "part.name" | "part.unit" | "part.say" | "part.words" | "part.control" | "part.range" => {
                let (num, rest) = value
                    .split_once('|')
                    .ok_or("A part change is written number|value.")?;
                let ci = num_to_index(num).ok_or("There is no such part.")?;
                let c = &m.sheet.concepts[ci];
                let rest = rest.trim();
                let sheet = &state.doc.sheet;
                state.doc.sheet = match name {
                    "part.name" => {
                        let line = (!rest.is_empty() && rest != c.id).then(|| format!("name {rest}"));
                        sheetedit::set_line(sheet, c.line, "name", line.as_deref(), &[])
                    }
                    "part.unit" => {
                        let line = (!rest.is_empty()).then(|| format!("unit {rest}"));
                        sheetedit::set_line(sheet, c.line, "unit", line.as_deref(), &[])
                    }
                    "part.say" => {
                        let line = (!rest.is_empty()).then(|| format!("say {rest}"));
                        sheetedit::set_line(sheet, c.line, "say", line.as_deref(), &[])
                    }
                    "part.words" => {
                        state.doc.prose = sheetedit::link(&state.doc.prose, rest, &c.id);
                        sheet.clone()
                    }
                    "part.control" => {
                        let (lo, hi, st, v) = c
                            .control
                            .as_ref()
                            .map(|k| (k.min, k.max, k.step, k.value))
                            .unwrap_or((
                                Fx(0),
                                Fx::parse("10").unwrap_or(Fx(0)),
                                Fx::parse("1").unwrap_or(Fx(0)),
                                Fx::parse("1").unwrap_or(Fx(0)),
                            ));
                        let range = format!(
                            "{} .. {} step {} = {}",
                            num_text(lo),
                            num_text(hi),
                            num_text(st),
                            num_text(v)
                        );
                        let line = match rest {
                            "none" => None,
                            "drag" => Some(format!("scrub {range}")),
                            "slider" => Some(format!("slider {range}")),
                            "type" => Some(format!("edit {}", num_text(v))),
                            other => return Err(format!("A part has no control \"{other}\".")),
                        };
                        let all = ["scrub", "slider", "edit", "limit"];
                        let s1 = sheetedit::set_line(sheet, c.line, "scrub", None, &all);
                        match line {
                            Some(l) => sheetedit::set_line(
                                &s1,
                                c.line,
                                l.split(' ').next().unwrap_or(""),
                                Some(&l),
                                &[],
                            ),
                            None => s1,
                        }
                    }
                    _ => {
                        // part.range: min|max|step, keeping the control's kind and value
                        let nums: Vec<Option<Fx>> = rest.split('|').map(|x| Fx::parse(x.trim())).collect();
                        let (Some(Some(lo)), Some(Some(hi)), Some(Some(st))) =
                            (nums.first(), nums.get(1), nums.get(2))
                        else {
                            return Err(s(&state, "range_bad"));
                        };
                        if hi <= lo || st.0 <= 0 {
                            return Err(s(&state, "range_bad"));
                        }
                        let k = c.control.as_ref().ok_or(s(&state, "range_bad"))?;
                        let v = k.value.max(*lo).min(*hi);
                        let word = match k.kind {
                            ControlKind::Scrub => "scrub",
                            ControlKind::Slider => "slider",
                            ControlKind::Edit => return Err(s(&state, "range_bad")),
                        };
                        let l = format!(
                            "{word} {} .. {} step {} = {}",
                            num_text(*lo),
                            num_text(*hi),
                            num_text(*st),
                            num_text(v)
                        );
                        state.overrides.retain(|(n, _)| *n != c.value_name());
                        sheetedit::set_line(sheet, c.line, word, Some(&l), &[])
                    }
                };
            }
            "prose.plain" => {
                let keep = sheetedit::links(&state.doc.prose);
                state.doc.prose = sheetedit::relink(value, &keep);
            }
            "tiles.at" => {
                let (line, cur) = value
                    .split_once('|')
                    .ok_or("A tile place is written line|cursor.")?;
                let line: usize = line.parse().map_err(|_| "A tile line is a number.")?;
                let cur = Cursor::read(cur).ok_or("That is not a place in the tiles.")?;
                if line >= m.lines.len() {
                    return Err("There is no such line.".into());
                }
                state.tiles = Some((line, cur.write()));
            }
            "tiles.key" => {
                if value == "letters" {
                    state.letters = !state.letters;
                } else if let Some((line, cur)) = cursor_of(&state, &m) {
                    let t = m
                        .lines
                        .get(line)
                        .and_then(|l| tiles::from_line(l))
                        .ok_or(s(&state, "tiles_locked"))?;
                    if let Some((after, c)) = key_of(value).and_then(|k| press_line(&t, &cur, k)) {
                        let mut lines = m.lines.clone();
                        lines[line] = tiles::to_latex(&after);
                        state.doc.formulas = lines.join("\n");
                        state.tiles = Some((line, c.write()));
                        state.overrides.clear();
                        state.select = None;
                    }
                }
            }
            "tiles.add_line" | "tiles.remove_line" => {
                let at = cursor_of(&state, &m)
                    .map(|(l, _)| l)
                    .unwrap_or(m.lines.len().saturating_sub(1));
                let mut lines = m.lines.clone();
                if name == "tiles.add_line" {
                    // the next line starts as a copy, to change one move at a time
                    let copy = lines.get(at).cloned().unwrap_or_else(|| s(&state, "new_formula"));
                    lines.insert((at + 1).min(lines.len()), copy.clone());
                    let end = tiles::from_line(&copy).map(|t| t.len()).unwrap_or(0);
                    state.tiles = Some((
                        at + 1,
                        Cursor {
                            path: vec![],
                            pos: end,
                        }
                        .write(),
                    ));
                } else if lines.len() > 1 {
                    lines.remove(at.min(lines.len() - 1));
                    state.tiles = None;
                } else {
                    return Err(s(&state, "line_last"));
                }
                state.doc.formulas = lines.join("\n");
                state.select = None;
            }
            "tiles.done" => state.tiles = None,
            "blocks.add_step" | "blocks.remove_step" | "blocks.caption" | "blocks.move_step"
            | "blocks.add" | "blocks.remove" | "blocks.arg" | "blocks.value" | "blocks.move" => {
                let mut st = step_blocks(&state.doc.sheet);
                let parts: Vec<&str> = value.split('|').collect();
                let n = |k: usize| -> Result<usize, String> {
                    parts
                        .get(k)
                        .and_then(|x| x.trim().parse::<usize>().ok())
                        .ok_or("A block place is a number.".into())
                };
                let at_step = |i: usize, st: &Vec<StepText>| -> Result<(), String> {
                    if i < st.len() {
                        Ok(())
                    } else {
                        Err("There is no such step.".into())
                    }
                };
                match name {
                    "blocks.add_step" => {
                        let caption = fill(&s(&state, "new_step"), &[("n", (st.len() + 1).to_string())]);
                        st.push(StepText {
                            caption,
                            lines: vec![format!("formula {}", m.lines.len().max(1))],
                        });
                    }
                    "blocks.remove_step" => {
                        let i = n(0)?;
                        at_step(i, &st)?;
                        st.remove(i);
                        state.step = None;
                    }
                    "blocks.caption" => {
                        let i = n(0)?;
                        at_step(i, &st)?;
                        let text = parts[1..].join("|");
                        let text = text.trim();
                        st[i].caption = if text.is_empty() {
                            fill(&s(&state, "new_step"), &[("n", (i + 1).to_string())])
                        } else {
                            text.to_string()
                        };
                    }
                    "blocks.move_step" => {
                        // i|j, or i|up and i|down from the arrows
                        let i = n(0)?;
                        let j = match parts.get(1).copied() {
                            Some("up") => i.saturating_sub(1),
                            Some("down") => (i + 1).min(st.len().saturating_sub(1)),
                            _ => n(1)?,
                        };
                        at_step(i, &st)?;
                        at_step(j, &st)?;
                        let item = st.remove(i);
                        st.insert(j, item);
                    }
                    "blocks.add" => {
                        let i = n(0)?;
                        at_step(i, &st)?;
                        let verb = parts.get(1).copied().unwrap_or("");
                        let args = block_args(&state, &m, verb);
                        let first = args.first().ok_or(s(&state, "block_no_parts"))?;
                        let line = match verb {
                            "show" => format!(
                                "formula {}",
                                args.last().map(|a| a.id.clone()).unwrap_or_default()
                            ),
                            "set" => {
                                let v = m
                                    .values(&[], None)
                                    .into_iter()
                                    .find(|(x, _)| *x == first.id)
                                    .map(|(_, v)| num_text(v))
                                    .unwrap_or_else(|| "1".into());
                                format!("set {} = {v}", first.id)
                            }
                            "spotlight" | "cross" | "hide" => format!("{} {}", keyword_of(verb), first.id),
                            other => return Err(format!("There is no block \"{other}\".")),
                        };
                        st[i].lines.push(line);
                    }
                    "blocks.remove" | "blocks.arg" | "blocks.value" => {
                        let (i, j) = (n(0)?, n(1)?);
                        at_step(i, &st)?;
                        let line = st[i].lines.get(j).cloned().ok_or("There is no such block.")?;
                        let verb = verb_of(&line);
                        let words: Vec<&str> = line.split_whitespace().collect();
                        let given = parts.get(2).copied().unwrap_or("").trim();
                        match name {
                            "blocks.remove" => {
                                st[i].lines.remove(j);
                            }
                            "blocks.arg" => {
                                if !block_args(&state, &m, verb).iter().any(|a| a.id == given) {
                                    return Err("That block cannot take that.".into());
                                }
                                st[i].lines[j] = if verb == "set" {
                                    format!("set {given} = {}", words.get(3).copied().unwrap_or("1"))
                                } else {
                                    format!("{} {given}", keyword_of(verb))
                                };
                            }
                            _ => {
                                let v = Fx::parse(given).ok_or(format!("{given} is not a number."))?;
                                if verb != "set" {
                                    return Err("Only a set block takes a number.".into());
                                }
                                st[i].lines[j] =
                                    format!("set {} = {}", words.get(1).copied().unwrap_or(""), num_text(v));
                            }
                        }
                    }
                    _ => {
                        // blocks.move: from step i, block j, to step k, before
                        // block l; or i|j|up and i|j|down from the arrows,
                        // which cross into the step before or after at an end
                        let (i, j) = (n(0)?, n(1)?);
                        at_step(i, &st)?;
                        let (k, l) = match parts.get(2).copied() {
                            Some("up") if j > 0 => (i, j - 1),
                            Some("up") if i > 0 => (i - 1, st[i - 1].lines.len()),
                            Some("up") => (i, j),
                            Some("down") if j + 1 < st[i].lines.len() => (i, j + 2),
                            Some("down") if i + 1 < st.len() => (i + 1, 0),
                            Some("down") => (i, j + 1),
                            _ => (n(2)?, n(3)?),
                        };
                        at_step(i, &st)?;
                        at_step(k, &st)?;
                        if j >= st[i].lines.len() {
                            return Err("There is no such block.".into());
                        }
                        let line = st[i].lines.remove(j);
                        let l = if i == k && l > j { l - 1 } else { l };
                        let l = l.min(st[k].lines.len());
                        st[k].lines.insert(l, line);
                    }
                }
                state.doc.sheet = sheetedit::write_steps(&state.doc.sheet, &st);
                state.playing = false;
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
                // a link a student shares opens for reading; a teacher picks
                let open = match (state.role.as_str(), value) {
                    ("teach", "build") => "build",
                    ("teach", "") => "",
                    _ => "explore",
                };
                let doc = Doc {
                    open: open.into(),
                    ..state.doc.clone()
                };
                let t = format!("{}{}", state.site, export::fragment(&doc));
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
    state.tiles = None;
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

    fn sheet(v: &serde_json::Value) -> String {
        v["ok"]["doc"]["sheet"].as_str().unwrap().to_string()
    }

    fn shows(v: &serde_json::Value) -> Vec<String> {
        serde_json::from_value(v["ok"]["shows"].clone()).unwrap()
    }

    #[test]
    fn the_code_shows_only_in_the_teacher_mode_with_its_switch_on() {
        let v = begin();
        assert_eq!(v["ok"]["role"], "build", "the editor opens for a student");
        assert!(!shows(&v).contains(&"code".to_string()));
        assert!(shows(&v).contains(&"tiles".to_string()));
        let t = step(&v, "role.set", "teach");
        assert!(!shows(&t).contains(&"code".to_string()), "the switch starts off");
        let t = step(&t, "code.toggle", "");
        assert!(shows(&t).contains(&"code".to_string()));
        let e = step(&t, "role.set", "explore");
        assert_eq!(shows(&e), ["reader", "roles"], "Explore is the reader alone");
        assert_eq!(
            step(&v, "role.set", "admin")["error"],
            "There is no mode \"admin\"."
        );
    }

    #[test]
    fn a_student_mode_reads_the_kid_word_where_one_exists() {
        let strings = strings().replacen('{', "{\"kid_concepts\": \"Parts\",", 1);
        let v = json(&start(&strings, EXAMPLES, "", "edit"));
        assert_eq!(v["ok"]["strings"]["concepts"], "Parts");
        let t = step(&v, "role.set", "teach");
        assert_eq!(t["ok"]["strings"]["concepts"], "concepts");
        let more = super::tests::strings().replacen(
            '{',
            "{\"kid_intro\": \"Build\", \"explore_intro\": \"Read\",",
            1,
        );
        let v = json(&start(&more, EXAMPLES, "", "edit"));
        assert_eq!(v["ok"]["strings"]["intro"], "Build");
        assert_eq!(step(&v, "role.set", "explore")["ok"]["strings"]["intro"], "Read");
        assert_eq!(step(&v, "role.set", "teach")["ok"]["strings"]["intro"], "intro");
    }

    #[test]
    fn explore_refuses_a_selection_and_shows_no_card() {
        let v = step(&step(&begin(), "role.set", "explore"), "select", "0:3");
        assert_eq!(v["ok"]["selection"], "");
        assert_eq!(v["ok"]["card"], serde_json::Value::Null);
    }

    #[test]
    fn a_card_makes_a_part_and_each_sentence_writes_one_line() {
        // y = m x + b: x is node 4
        let v = step(&begin(), "select", "0:4");
        assert_eq!(v["ok"]["card"]["num"], 0, "x is not a part yet");
        assert_eq!(
            v["ok"]["card"]["words"][0]["id"], "",
            "no word is suggested for a new part"
        );
        let v = step(&v, "part.make", "time");
        assert!(
            sheet(&v).ends_with("concept time: x\n  label above\n"),
            "{}",
            sheet(&v)
        );
        assert_eq!(v["ok"]["card"]["num"], 3, "x is the third part");
        let v = step(&v, "part.unit", "3|s");
        let v = step(&v, "part.control", "3|slider");
        assert!(
            sheet(&v).ends_with("concept time: x\n  label above\n  unit s\n  slider 0 .. 10 step 1 = 1\n"),
            "{}",
            sheet(&v)
        );
        let slider = v["ok"]["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "x")
            .unwrap()
            .clone();
        assert_eq!(slider["shown"], "1 s", "the readout carries the unit");
        let v = step(&v, "part.range", "3|2|8|2");
        assert!(
            sheet(&v).contains("  slider 2 .. 8 step 2 = 2\n"),
            "the value 1 is raised to the new least, 2"
        );
        assert_eq!(step(&v, "part.range", "3|8|2|1")["error"], "range_bad");
        let v = step(&v, "part.control", "3|none");
        assert!(
            sheet(&v).ends_with("concept time: x\n  label above\n  unit s\n"),
            "the slider goes; the intercept keeps its own"
        );
        let v = step(&v, "part.name", "3|time taken");
        assert!(
            sheet(&v).ends_with("  unit s\n  name time taken\n"),
            "{}",
            sheet(&v)
        );
        let v = step(&v, "part.words", "3|tilts");
        assert_eq!(v["ok"]["doc"]["prose"], "The [slope](slope) [tilts](time) it.");
        assert_eq!(v["ok"]["sentence"], "The slope tilts it.");
        let v = step(&v, "prose.plain", "The slope tilts it a lot.");
        assert_eq!(
            v["ok"]["doc"]["prose"],
            "The [slope](slope) [tilts](time) it a lot."
        );
    }

    #[test]
    fn a_tap_on_a_part_of_several_symbols_selects_it_whole() {
        // y = m x + b with a part "m x" (row 0, siblings 2 and 3); the tap
        // lands on m (node 3), and the part comes first, then larger ones
        let v = step(&begin(), "doc.sheet", "concept term: m x\n");
        let v = step(&v, "select", "0:3|0:2:2");
        assert_eq!(v["ok"]["selection"], "m x");
        assert_eq!(v["ok"]["card"]["num"], 1);
        let v = step(&v, "select", "0:3|0:2:2");
        assert_eq!(
            v["ok"]["selection"], "m x + b",
            "a second tap grows past the part"
        );
    }

    #[test]
    fn a_tap_on_a_part_opens_its_card_and_a_chip_picks_it() {
        let v = step(&begin(), "select", "0:3");
        assert_eq!(v["ok"]["card"]["num"], 1, "m is the slope, part 1");
        assert_eq!(v["ok"]["card"]["number"], true);
        assert_eq!(v["ok"]["card"]["controls"][1]["on"], true, "the slope is dragged");
        let v = step(&begin(), "part.pick", "2");
        assert_eq!(v["ok"]["selection"], "b");
    }

    #[test]
    fn keys_write_the_formula_and_a_key_that_does_not_fit_is_off() {
        // y = m x + b is six tiles; the cursor starts after the last
        let v = begin();
        assert!(v["ok"]["tiles"][0]["html"]
            .as_str()
            .unwrap()
            .ends_with("<span class=\"caret\" aria-hidden=\"true\"></span>"));
        // a selection made before a key is dropped, so it cannot point past the line
        let v = step(&step(&v, "select", "0:6"), "tiles.key", "back");
        assert_eq!(v["ok"]["doc"]["formulas"], "y = m x +");
        assert_eq!(v["ok"]["selection"], "");
        let v = step(&begin(), "tiles.at", "0|;6");
        let keys = v["ok"]["keys"].as_array().unwrap().clone();
        let on = |id: &str| keys.iter().find(|k| k["id"] == id).unwrap()["on"].clone();
        assert_eq!(on("="), false, "one = to a line");
        assert_eq!(on("+"), true);
        let v = step(&step(&v, "tiles.key", "+"), "tiles.key", "2");
        assert_eq!(v["ok"]["doc"]["formulas"], "y = m x + b + 2");
        let v = step(&v, "tiles.key", "=");
        assert_eq!(
            v["ok"]["doc"]["formulas"], "y = m x + b + 2",
            "a refused key changes nothing"
        );
        let v = step(&v, "tiles.add_line", "");
        assert_eq!(v["ok"]["doc"]["formulas"], "y = m x + b + 2\ny = m x + b + 2");
        let v = step(&v, "tiles.remove_line", "");
        assert_eq!(v["ok"]["doc"]["formulas"], "y = m x + b + 2");
        assert_eq!(step(&v, "tiles.remove_line", "")["error"], "line_last");
        // a one-tile line keeps its tile: delete is off there
        let v = step(&step(&begin(), "tiles.at", "0|;0"), "doc.formulas", "y");
        let v = step(&step(&v, "tiles.at", "0|;1"), "tiles.key", "back");
        assert_eq!(
            v["ok"]["doc"]["formulas"], "y",
            "a line is removed with its button, not emptied"
        );
    }

    #[test]
    fn blocks_rewrite_the_steps_and_keep_the_rest_of_the_sheet() {
        // the example's steps: "step one" (empty), "step two" with set b = 3
        let v = step(&begin(), "blocks.value", "1|0|4");
        assert!(
            sheet(&v).ends_with("step one\nstep two\n  set b = 4\n"),
            "{}",
            sheet(&v)
        );
        let v = step(&v, "blocks.move", "1|0|0|0");
        assert!(
            sheet(&v).ends_with("step one\n  set b = 4\nstep two\n"),
            "{}",
            sheet(&v)
        );
        let v = step(&v, "blocks.add", "1|spotlight");
        assert!(sheet(&v).ends_with("step two\n  focus slope\n"), "{}", sheet(&v));
        let v = step(&v, "blocks.arg", "1|0|icpt");
        assert!(sheet(&v).ends_with("step two\n  focus icpt\n"));
        let v = step(&v, "blocks.add_step", "");
        assert!(sheet(&v).ends_with("step new_step\n  formula 1\n"));
        let v = step(&v, "blocks.move_step", "2|0");
        assert!(sheet(&v).contains("  limit -1 .. 2\nconcept icpt: b\n  slider -5 .. 5 step 1 = 1\nstep new_step\n  formula 1\nstep one\n"), "{}", sheet(&v));
        assert_eq!(v["ok"]["blocks"][1]["acts"][0]["label"], "verb_set");
        assert_eq!(
            step(&v, "blocks.arg", "1|0|q")["error"],
            "That block cannot take that."
        );
        // steps now: new_step (formula 1), one (set b = 4), two (focus icpt)
        let v = step(&v, "blocks.move", "1|0|up");
        assert!(
            sheet(&v)
                .ends_with("step new_step\n  formula 1\n  set b = 4\nstep one\nstep two\n  focus icpt\n"),
            "{}",
            sheet(&v)
        );
        let v = step(&v, "blocks.move", "0|0|down");
        assert!(
            sheet(&v).contains("step new_step\n  set b = 4\n  formula 1\nstep one\n"),
            "{}",
            sheet(&v)
        );
        let v = step(&v, "blocks.move", "0|1|down");
        assert!(
            sheet(&v)
                .ends_with("step new_step\n  set b = 4\nstep one\n  formula 1\nstep two\n  focus icpt\n"),
            "{}",
            sheet(&v)
        );
        let v = step(&v, "blocks.move_step", "2|up");
        assert!(
            sheet(&v).ends_with("step two\n  focus icpt\nstep one\n  formula 1\n"),
            "{}",
            sheet(&v)
        );
    }

    #[test]
    fn a_shared_link_opens_in_the_mode_it_names() {
        let v = step(&begin(), "export.link", "");
        let link = v["ok"]["out"]["text"].as_str().unwrap().to_string();
        let frag = &link[link.find('#').unwrap()..];
        assert_eq!(
            export::from_fragment(frag).unwrap().open,
            "explore",
            "a student's link opens for reading"
        );
        let back = step(&begin(), "doc.fragment", frag);
        assert_eq!(back["ok"]["role"], "explore");
        assert_eq!(
            back["ok"]["fragment"].as_str().unwrap(),
            v["ok"]["fragment"].as_str().unwrap(),
            "the address keeps no mode"
        );
        let t = step(&step(&begin(), "role.set", "teach"), "export.link", "build");
        let link = t["ok"]["out"]["text"].as_str().unwrap().to_string();
        assert_eq!(
            export::from_fragment(&link[link.find('#').unwrap()..])
                .unwrap()
                .open,
            "build"
        );
    }

    #[test]
    fn the_teacher_menu_heads_each_field_once_and_a_student_menu_has_no_headings() {
        let strings = include_str!("../../../data/strings.json");
        let examples = include_str!("../../../data/examples.json");
        let all: Vec<Example> = serde_json::from_str(examples).unwrap();
        let words: serde_json::Value = serde_json::from_str(strings).unwrap();
        let v = json(&start(strings, examples, "", "edit"));
        let kid = v["ok"]["examples"].as_array().unwrap();
        assert_eq!(kid.len(), all.iter().filter(|e| e.group == "shelf").count());
        assert!(
            kid.iter().all(|e| e["field"] == ""),
            "a student's menu is the shelf, unheaded"
        );
        let t = step(&v, "role.set", "teach");
        let menu = t["ok"]["examples"].as_array().unwrap();
        assert_eq!(menu.len(), all.len());
        let mut heads: Vec<&str> = Vec::new();
        for (e, item) in all.iter().zip(menu) {
            let head = item["field"].as_str().unwrap();
            assert!(!head.is_empty(), "{} has no heading", e.id);
            assert_eq!(head, words[format!("field_{}", e.group)].as_str().unwrap());
            if heads.last() != Some(&head) {
                assert!(!heads.contains(&head), "{head} is split in two");
                heads.push(head);
            }
        }
        // the shelf, the paper's sources, and seven fields
        assert_eq!(heads.len(), 9);
    }

    #[test]
    fn the_new_examples_compute_values_worked_out_by_hand() {
        let all: Vec<Example> = serde_json::from_str(include_str!("../../../data/examples.json")).unwrap();
        let value = |id: &str, name: &str| {
            let e = all.iter().find(|e| e.id == id).unwrap();
            let m = Model::build(&e.doc);
            let values = m.values(&[], None);
            let name = crate::tex::canon(&crate::tex::parse(name).unwrap());
            m.scope(&values).value_of(&name, &[]).unwrap().format(3)
        };
        // worked by hand from each sheet's starting values
        assert_eq!(value("handshakes", "H"), "10.000"); // 5 x 4 / 2
        assert_eq!(value("gausssum", "S"), "55.000"); // 10 x 11 / 2
        assert_eq!(value("determinant", "D"), "5.000"); // 3 x 2 - 1 x 1
        assert_eq!(value("metrics", "d"), "5.000"); // sqrt(3^2 + 4^2)
        assert_eq!(value("metrics", "m"), "7.000"); // 3 + 4
        assert_eq!(value("perimeter", "P"), "14.000"); // 2 x 5 + 2 x 2
        assert_eq!(value("rectangle", "A"), "12.000"); // 4 x 3, with the script l
        assert_eq!(value("box", "V"), "30.000"); // 5 x 3 x 2
        assert_eq!(value("mean", "m"), "7.000"); // (4 + 7 + 10) / 3
        assert_eq!(value("percent", "p"), "20.000"); // 25 / 100 x 80
        assert_eq!(value("circle", "C"), "6.283"); // 2 pi x 1
        assert_eq!(value("testresult", "P"), "0.092"); // 0.009 / (0.009 + 0.0891)
        assert_eq!(value("atleastone", "P"), "0.518"); // 1 - 625/1296
        assert_eq!(value("pendulum", "T"), "2.007"); // 2 pi sqrt(1 / 9.8)
        assert_eq!(value("momentum", "E"), "0.058"); // ((sqrt 50 - 1)/(sqrt 50 + 1))^10
    }

    #[test]
    fn every_example_builds_cleanly_and_every_shelf_line_opens_as_tiles() {
        let all: Vec<Example> = serde_json::from_str(include_str!("../../../data/examples.json")).unwrap();
        assert!(all.iter().filter(|e| e.group == "shelf").count() >= 6);
        for e in &all {
            let m = Model::build(&e.doc);
            assert!(m.errors.is_empty(), "{}: {:?}", e.id, m.errors);
            for c in &m.sheet.concepts {
                assert!(
                    m.occs.iter().any(|o| m.sheet.concepts[o.concept].id == c.id),
                    "{}: {} marks nothing",
                    e.id,
                    c.id
                );
            }
            if e.group == "shelf" {
                for l in &m.lines {
                    assert!(tiles::from_line(l).is_some(), "{}: {l} has no tiles", e.id);
                }
            }
        }
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
