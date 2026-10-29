//! The augment sheet: a small language, kept apart from the formulas (the
//! paper's R6), that names concepts and says how each is shown and changed.
//!
//! ```text
//! concept slope: m
//!   say how much y changes for one step in x
//!   label above
//!   scrub -3 .. 3 step 0.5 = 2
//! hidden n 2 .. 40 step 1 = 12 sample points
//! plot y over x from -5 to 5
//!   y from -10 to 10
//!   mark slope rise x = 1
//! step expand z and its conjugate
//!   formula 2
//!   strike imaginary
//! ```

use crate::tex::{canon, parse, Node, K};
use crate::value::Fx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Above,
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Dashed,
    Dotted,
    Wave,
    Double,
}

impl Pattern {
    pub const ALL: [Pattern; 5] = [
        Pattern::Solid,
        Pattern::Dashed,
        Pattern::Dotted,
        Pattern::Wave,
        Pattern::Double,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Pattern::Solid => "solid",
            Pattern::Dashed => "dashed",
            Pattern::Dotted => "dotted",
            Pattern::Wave => "wave",
            Pattern::Double => "double",
        }
    }

    fn from(word: &str) -> Option<Pattern> {
        Pattern::ALL.iter().copied().find(|p| p.name() == word)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Triangle,
    Circle,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Scrub,
    Slider,
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub kind: ControlKind,
    pub min: Fx,
    pub max: Fx,
    pub step: Fx,
    pub value: Fx,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concept {
    pub id: String,
    /// 1-based, in sheet order; the number on its badge
    pub num: usize,
    pub pattern_src: String,
    pub name: String,
    pub say: String,
    /// the unit a reader reads beside its value, such as `cm`
    pub unit: String,
    pub label: Option<(Side, String)>,
    pub brace: Option<(Side, String)>,
    pub border: bool,
    pub tint: bool,
    pub pointer: bool,
    pub space: bool,
    pub shape: Option<Shape>,
    pub pattern: Pattern,
    pub colour: bool,
    pub control: Option<Control>,
    pub limit: Option<(Fx, Fx)>,
    pub line: usize,
}

impl Concept {
    /// The value name a control sets: the spelling of its one-symbol pattern.
    pub fn value_name(&self) -> String {
        parse(&self.pattern_src).map(|n| canon(&n)).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hidden {
    pub name: String,
    pub label: String,
    pub control: Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Point,
    Rise,
    Curve,
    Intercept,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub concept: String,
    pub kind: MarkKind,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plot {
    pub of: String,
    pub var: String,
    pub from: Fx,
    pub to: Fx,
    pub y: Option<(Fx, Fx)>,
    pub samples: String,
    pub marks: Vec<Mark>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Step {
    pub caption: String,
    pub formula: Option<usize>,
    pub strike: Vec<String>,
    pub focus: Vec<String>,
    pub hide: Vec<String>,
    pub set: Vec<(String, Fx)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sheet {
    pub concepts: Vec<Concept>,
    pub hidden: Vec<Hidden>,
    pub plot: Option<Plot>,
    pub steps: Vec<Step>,
    pub errors: Vec<String>,
}

impl Sheet {
    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.iter().find(|c| c.id == id)
    }
}

/// How many concepts get a colour of their own; later ones get a pattern and a
/// number only (the paper's R11, and rule FB-7).
pub const COLOUR_BUDGET: usize = 4;

fn range(words: &[&str]) -> Option<(Fx, Fx, Fx, Fx)> {
    // MIN .. MAX [step S] [= V]
    let min = Fx::parse(words.first()?)?;
    if *words.get(1)? != ".." {
        return None;
    }
    let max = Fx::parse(words.get(2)?)?;
    let mut step = Fx::parse("0.1")?;
    let mut value = min;
    let mut i = 3;
    while i < words.len() {
        match words[i] {
            "step" => {
                step = Fx::parse(words.get(i + 1)?)?;
                i += 2;
            }
            "=" => {
                value = Fx::parse(words.get(i + 1)?)?;
                i += 2;
            }
            _ => break,
        }
    }
    if max <= min || step.0 <= 0 {
        return None;
    }
    Some((min, max, step, value.max(min).min(max)))
}

fn words_after<'a>(line: &'a str, n: usize) -> String {
    let rest: Vec<&'a str> = line.split_whitespace().skip(n).collect();
    rest.join(" ").trim_matches('"').to_string()
}

fn side(word: Option<&&str>) -> Option<Side> {
    match word.copied() {
        Some("above") => Some(Side::Above),
        Some("below") => Some(Side::Below),
        _ => None,
    }
}

enum Block {
    None,
    Concept,
    Plot,
    Step,
}

pub fn parse_sheet(text: &str) -> Sheet {
    let mut sheet = Sheet::default();
    let mut block = Block::None;
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.split('#').next().unwrap_or("");
        if line.trim().is_empty() {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let words: Vec<&str> = line.split_whitespace().collect();
        let err = |sheet: &mut Sheet, msg: &str| sheet.errors.push(format!("Sheet line {line_no}: {msg}"));
        if !indented {
            block = Block::None;
            match words[0] {
                "concept" => {
                    let rest = words_after(line, 1);
                    let Some((id, pattern)) = rest.split_once(':') else {
                        err(
                            &mut sheet,
                            "a concept is written `concept name: pattern`, like `concept slope: m`.",
                        );
                        continue;
                    };
                    let id = id.trim().to_string();
                    if id.is_empty() || id.contains(' ') {
                        err(&mut sheet, "a concept's name is one word, like `slope`.");
                        continue;
                    }
                    if sheet.concept(&id).is_some() {
                        err(&mut sheet, &format!("the concept `{id}` is named twice."));
                        continue;
                    }
                    let pattern = pattern.trim().to_string();
                    if let Err(e) = parse(&pattern) {
                        err(
                            &mut sheet,
                            &format!("the pattern for `{id}` is not valid LaTeX: {e}"),
                        );
                        continue;
                    }
                    let num = sheet.concepts.len() + 1;
                    sheet.concepts.push(Concept {
                        name: id.clone(),
                        id,
                        num,
                        pattern_src: pattern,
                        say: String::new(),
                        unit: String::new(),
                        label: None,
                        brace: None,
                        border: false,
                        tint: false,
                        pointer: false,
                        space: false,
                        shape: None,
                        pattern: Pattern::ALL[(num - 1) % Pattern::ALL.len()],
                        colour: num <= COLOUR_BUDGET,
                        control: None,
                        limit: None,
                        line: line_no,
                    });
                    block = Block::Concept;
                }
                "hidden" => {
                    // hidden NAME MIN .. MAX step S = V LABEL...
                    let Some(name) = words.get(1) else {
                        err(
                            &mut sheet,
                            "a hidden value needs a name, like `hidden n 2 .. 40 step 1 = 12 points`.",
                        );
                        continue;
                    };
                    let Some((min, max, step, value)) = range(&words[2..]) else {
                        err(
                            &mut sheet,
                            "a hidden value needs a range, like `hidden n 2 .. 40 step 1 = 12 points`.",
                        );
                        continue;
                    };
                    let used = words
                        .iter()
                        .skip(2)
                        .take_while(|w| Fx::parse(w).is_some() || ["..", "step", "="].contains(w))
                        .count();
                    let label = words_after(line, 2 + used);
                    let name = parse(name)
                        .map(|n| canon(&n))
                        .unwrap_or_else(|_| name.to_string());
                    sheet.hidden.push(Hidden {
                        label: if label.is_empty() { name.clone() } else { label },
                        name,
                        control: Control {
                            kind: ControlKind::Slider,
                            min,
                            max,
                            step,
                            value,
                        },
                    });
                }
                "plot" => {
                    // plot Y over X from A to B
                    let ok = words.len() >= 8 && words[2] == "over" && words[4] == "from" && words[6] == "to";
                    let (Some(from), Some(to)) = (
                        words.get(5).and_then(|w| Fx::parse(w)),
                        words.get(7).and_then(|w| Fx::parse(w)),
                    ) else {
                        err(&mut sheet, "a plot is written `plot y over x from -5 to 5`.");
                        continue;
                    };
                    if !ok || to <= from {
                        err(
                            &mut sheet,
                            "a plot is written `plot y over x from -5 to 5`, with the end after the start.",
                        );
                        continue;
                    }
                    let spell = |w: &str| parse(w).map(|n| canon(&n)).unwrap_or_else(|_| w.to_string());
                    sheet.plot = Some(Plot {
                        of: spell(words[1]),
                        var: spell(words[3]),
                        from,
                        to,
                        y: None,
                        samples: "120".into(),
                        marks: vec![],
                    });
                    block = Block::Plot;
                }
                "step" => {
                    sheet.steps.push(Step {
                        caption: words_after(line, 1),
                        ..Step::default()
                    });
                    block = Block::Step;
                }
                other => err(
                    &mut sheet,
                    &format!(
                        "`{other}` is not a sheet command; lines start with concept, hidden, plot or step."
                    ),
                ),
            }
            continue;
        }
        match block {
            Block::None => err(
                &mut sheet,
                "an indented line needs a concept, plot or step above it.",
            ),
            Block::Concept => concept_line(&mut sheet, &words, line, line_no),
            Block::Plot => plot_line(&mut sheet, &words, line, line_no),
            Block::Step => step_line(&mut sheet, &words, line_no),
        }
    }
    sheet
}

fn concept_line(sheet: &mut Sheet, words: &[&str], line: &str, line_no: usize) {
    let c = sheet.concepts.last_mut().expect("a concept block has a concept");
    let mut problem = None;
    match words[0] {
        "name" => c.name = words_after(line, 1),
        "say" => c.say = words_after(line, 1),
        "unit" => c.unit = words_after(line, 1),
        "label" | "brace" => match side(words.get(1)) {
            Some(s) => {
                let text = words_after(line, 2);
                let text = if text.is_empty() { c.name.clone() } else { text };
                if words[0] == "label" {
                    c.label = Some((s, text));
                } else {
                    c.brace = Some((s, text));
                }
            }
            None => problem = Some(format!("`{}` takes above or below.", words[0])),
        },
        "border" => c.border = true,
        "tint" => c.tint = true,
        "pointer" => c.pointer = true,
        "space" => c.space = true,
        "shape" => {
            c.shape = match words.get(1).copied() {
                Some("triangle") => Some(Shape::Triangle),
                Some("circle") => Some(Shape::Circle),
                Some("square") => Some(Shape::Square),
                _ => {
                    problem = Some("`shape` takes triangle, circle or square.".into());
                    None
                }
            }
        }
        "pattern" => match words.get(1).and_then(|w| Pattern::from(w)) {
            Some(p) => c.pattern = p,
            None => problem = Some("`pattern` takes solid, dashed, dotted, wave or double.".into()),
        },
        "colour" | "color" => {
            if words.get(1) == Some(&"none") {
                c.colour = false;
            }
        }
        "scrub" | "slider" => match range(&words[1..]) {
            Some((min, max, step, value)) => {
                let kind = if words[0] == "scrub" {
                    ControlKind::Scrub
                } else {
                    ControlKind::Slider
                };
                c.control = Some(Control {
                    kind,
                    min,
                    max,
                    step,
                    value,
                });
            }
            None => {
                problem = Some(format!(
                    "`{}` is written `{} -3 .. 3 step 0.5 = 2`.",
                    words[0], words[0]
                ))
            }
        },
        "edit" => match words.get(1).and_then(|w| Fx::parse(w)) {
            Some(v) => {
                let big = Fx::int(1_000_000);
                // the step is the typed value's own precision: `edit 1000` steps by 1
                let step = Fx(10i64.pow(9 - v.digits_needed(6)));
                c.control = Some(Control {
                    kind: ControlKind::Edit,
                    min: big.neg(),
                    max: big,
                    step,
                    value: v,
                })
            }
            None => problem = Some("`edit` is written `edit 2`, with the starting value.".into()),
        },
        "limit" => match range(&words[1..]) {
            Some((min, max, _, _)) => c.limit = Some((min, max)),
            None => problem = Some("`limit` is written `limit 0 .. 3`.".into()),
        },
        other => problem = Some(format!("`{other}` is not something a concept takes.")),
    }
    let needs_symbol = c.control.is_some() && !single_symbol(&c.pattern_src);
    if needs_symbol {
        c.control = None;
        problem = Some(format!(
            "only a single symbol can take a value; `{}` is more than one.",
            c.pattern_src
        ));
    }
    if let Some(p) = problem {
        sheet.errors.push(format!("Sheet line {line_no}: {p}"));
    }
}

fn single_symbol(src: &str) -> bool {
    parse(src)
        .map(|n| n.kids.len() == 1 && matches!(n.kids[0].k, K::Glyph { .. } | K::Scripts { .. }))
        .unwrap_or(false)
}

fn plot_line(sheet: &mut Sheet, words: &[&str], line: &str, line_no: usize) {
    let plot = sheet.plot.as_mut().expect("a plot block has a plot");
    let mut problem = None;
    match words[0] {
        "y" if words.len() == 5 && words[1] == "from" && words[3] == "to" => {
            match (Fx::parse(words[2]), Fx::parse(words[4])) {
                (Some(a), Some(b)) if b > a => plot.y = Some((a, b)),
                _ => problem = Some("the y range is written `y from -10 to 10`.".to_string()),
            }
        }
        "samples" => plot.samples = words_after(line, 1),
        "mark" => {
            let kind = match words.get(2).copied() {
                Some("point") => Some(MarkKind::Point),
                Some("rise") => Some(MarkKind::Rise),
                Some("curve") => Some(MarkKind::Curve),
                Some("intercept") => Some(MarkKind::Intercept),
                _ => None,
            };
            match (words.get(1), kind) {
                (Some(c), Some(kind)) => {
                    let at = line
                        .split_once('=')
                        .map(|(_, r)| r.trim().to_string())
                        .unwrap_or_default();
                    let needs_at = matches!(kind, MarkKind::Point | MarkKind::Rise);
                    if needs_at && at.is_empty() {
                        problem =
                            Some("a point or rise mark needs a place, like `mark slope rise x = 1`.".into());
                    } else {
                        plot.marks.push(Mark {
                            concept: c.to_string(),
                            kind,
                            at,
                        })
                    }
                }
                _ => {
                    problem = Some(
                        "a mark is written `mark CONCEPT point x = 2` (point, rise, curve or intercept)."
                            .into(),
                    )
                }
            }
        }
        other => problem = Some(format!("`{other}` is not something a plot takes.")),
    }
    if let Some(p) = problem {
        sheet.errors.push(format!("Sheet line {line_no}: {p}"));
    }
}

fn step_line(sheet: &mut Sheet, words: &[&str], line_no: usize) {
    let step = sheet.steps.last_mut().expect("a step block has a step");
    let names = || {
        words[1..]
            .iter()
            .map(|w| w.trim_matches(',').to_string())
            .collect::<Vec<_>>()
    };
    match words[0] {
        "formula" => match words.get(1).and_then(|w| w.parse::<usize>().ok()) {
            Some(n) if n >= 1 => step.formula = Some(n - 1),
            _ => sheet.errors.push(format!(
                "Sheet line {line_no}: `formula` takes the formula's number, from 1."
            )),
        },
        "strike" => step.strike.extend(names()),
        "focus" => step.focus.extend(names()),
        "hide" => step.hide.extend(names()),
        "set" => {
            let v = words.get(3).and_then(|w| Fx::parse(w));
            match (words.get(1), words.get(2), v) {
                (Some(n), Some(&"="), Some(v)) => {
                    let name = parse(n).map(|x| canon(&x)).unwrap_or_else(|_| n.to_string());
                    step.set.push((name, v))
                }
                _ => sheet
                    .errors
                    .push(format!("Sheet line {line_no}: `set` is written `set m = 2`.")),
            }
        }
        other => sheet.errors.push(format!(
            "Sheet line {line_no}: `{other}` is not something a step takes."
        )),
    }
}

/// One place a concept's pattern appears: a run of siblings in a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occ {
    pub formula: usize,
    pub concept: usize,
    pub row: usize,
    pub first: usize,
    pub len: usize,
    /// the ids the run covers, inclusive
    pub lo: usize,
    pub hi: usize,
    /// how many other occurrences contain this one
    pub depth: usize,
}

pub fn occurrences(roots: &[Option<Node>], sheet: &Sheet) -> Vec<Occ> {
    let mut out = Vec::new();
    for (ci, c) in sheet.concepts.iter().enumerate() {
        let Ok(pat) = parse(&c.pattern_src) else { continue };
        let want: Vec<String> = pat.kids.iter().map(canon).collect();
        if want.is_empty() {
            continue;
        }
        for (fi, root) in roots.iter().enumerate() {
            let Some(root) = root else { continue };
            let mut all = Vec::new();
            root.walk(&mut all);
            // a one-symbol pattern also marks the base of a power or prime
            // (the `a` of `a^2`), but not of a subscripted name (`w_k`)
            if want.len() == 1 {
                for sc in all
                    .iter()
                    .filter(|n| matches!(n.k, K::Scripts { sub: false, .. }))
                {
                    if canon(&sc.kids[0]) == want[0] {
                        let b = &sc.kids[0];
                        out.push(Occ {
                            formula: fi,
                            concept: ci,
                            row: sc.id,
                            first: 0,
                            len: 1,
                            lo: b.id,
                            hi: b.end,
                            depth: 0,
                        });
                    }
                }
            }
            for row in all.into_iter().filter(|n| n.k == K::Row) {
                let spelled: Vec<String> = row.kids.iter().map(canon).collect();
                let mut i = 0;
                while i + want.len() <= spelled.len() {
                    if spelled[i..i + want.len()] == want[..] {
                        let lo = row.kids[i].id;
                        let hi = row.kids[i + want.len() - 1].end;
                        out.push(Occ {
                            formula: fi,
                            concept: ci,
                            row: row.id,
                            first: i,
                            len: want.len(),
                            lo,
                            hi,
                            depth: 0,
                        });
                        i += want.len();
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }
    let spans: Vec<(usize, usize, usize)> = out.iter().map(|o| (o.formula, o.lo, o.hi)).collect();
    for o in &mut out {
        o.depth = spans
            .iter()
            .filter(|(f, lo, hi)| *f == o.formula && *lo <= o.lo && o.hi <= *hi && (*lo, *hi) != (o.lo, o.hi))
            .count();
    }
    out
}

/// Words of prose, each optionally linked to a concept: `[slope](slope)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub concept: Option<String>,
}

pub fn parse_prose(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find("](") else { break };
        let tail = &after[close + 2..];
        let Some(end) = tail.find(')') else { break };
        if open > 0 {
            out.push(Segment {
                text: rest[..open].to_string(),
                concept: None,
            });
        }
        out.push(Segment {
            text: after[..close].to_string(),
            concept: Some(tail[..end].trim().to_string()),
        });
        rest = &tail[end + 1..];
    }
    if !rest.is_empty() {
        out.push(Segment {
            text: rest.to_string(),
            concept: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "concept slope: m\n  say how much y changes\n  label above\n  scrub -3 .. 3 step 0.5 = 2\nconcept icpt: b\n  brace below intercept\n  slider -5 .. 5 step 1 = 1\nplot y over x from -5 to 5\n  y from -10 to 10\n  mark slope rise x = 1\nstep start\n  focus slope\n";

    #[test]
    fn a_sheet_reads_concepts_controls_plots_and_steps() {
        let s = parse_sheet(SHEET);
        assert!(s.errors.is_empty(), "{:?}", s.errors);
        let m = &s.concepts[0];
        assert_eq!((m.id.as_str(), m.num, m.pattern), ("slope", 1, Pattern::Solid));
        assert_eq!(
            m.label,
            Some((Side::Above, "slope".into())),
            "a label defaults to the name"
        );
        let c = m.control.as_ref().unwrap();
        assert_eq!(
            (c.kind, c.value, c.step),
            (ControlKind::Scrub, Fx::int(2), Fx::parse("0.5").unwrap())
        );
        assert_eq!(
            s.concepts[1].pattern,
            Pattern::Dashed,
            "the second concept gets the second pattern"
        );
        assert_eq!(s.concepts[1].brace, Some((Side::Below, "intercept".into())));
        let p = s.plot.as_ref().unwrap();
        assert_eq!(
            (p.of.as_str(), p.var.as_str(), p.y),
            ("y", "x", Some((Fx::int(-10), Fx::int(10))))
        );
        assert_eq!(
            p.marks[0],
            Mark {
                concept: "slope".into(),
                kind: MarkKind::Rise,
                at: "1".into()
            }
        );
        assert_eq!(s.steps[0].focus, vec!["slope".to_string()]);
    }

    #[test]
    fn the_fifth_concept_gets_no_colour_but_keeps_pattern_and_number() {
        let text: String = (1..=6).map(|i| format!("concept c{i}: x_{i}\n")).collect();
        let s = parse_sheet(&text);
        let colours: Vec<bool> = s.concepts.iter().map(|c| c.colour).collect();
        assert_eq!(colours, [true, true, true, true, false, false]);
        assert_eq!(s.concepts[4].pattern, Pattern::Double);
        assert_eq!(
            s.concepts[5].pattern,
            Pattern::Solid,
            "patterns cycle; the number tells them apart"
        );
        assert_eq!(s.concepts[5].num, 6);
    }

    #[test]
    fn mistakes_are_reported_with_their_line_and_the_rest_still_reads() {
        let s = parse_sheet("concept a: m\n  scrub 3 .. 1\n  wobble\nfoo\nconcept b: x y\n  slider 0 .. 1\n");
        assert_eq!(s.errors.len(), 4, "{:?}", s.errors);
        assert!(s.errors[0].starts_with("Sheet line 2: `scrub` is written"));
        assert_eq!(
            s.errors[1],
            "Sheet line 3: `wobble` is not something a concept takes."
        );
        assert!(s.errors[2].starts_with("Sheet line 4: `foo` is not a sheet command"));
        assert!(s.errors[3].contains("only a single symbol can take a value"));
        assert_eq!(s.concepts.len(), 2);
    }

    #[test]
    fn a_pattern_matches_runs_of_siblings_in_every_formula() {
        let roots = vec![
            Some(parse("w_{k+1} = w_k - \\alpha \\nabla f(w_k)").unwrap()),
            Some(parse("f(w_k) = 2").unwrap()),
        ];
        let s = parse_sheet("concept grad: \\nabla f(w_k)\nconcept cur: w_k\n");
        let occ = occurrences(&roots, &s);
        let grad: Vec<&Occ> = occ.iter().filter(|o| o.concept == 0).collect();
        assert_eq!(grad.len(), 1);
        assert_eq!((grad[0].first, grad[0].len), (5, 5), "after w_k+1 = w_k - alpha");
        let cur: Vec<(usize, usize)> = occ
            .iter()
            .filter(|o| o.concept == 1)
            .map(|o| (o.formula, o.depth))
            .collect();
        // w_k twice in formula 1 (once inside the gradient), once in formula 2
        assert_eq!(cur, vec![(0, 0), (0, 1), (1, 0)]);
    }

    #[test]
    fn a_symbol_matches_the_base_of_its_powers_but_not_of_subscripted_names() {
        let roots = vec![Some(parse("c^2 = a^2 + a_1 + a").unwrap())];
        let s = parse_sheet("concept a: a\n");
        let occ = occurrences(&roots, &s);
        assert_eq!(occ.len(), 2, "a^2 and the lone a; a_1 is another name: {occ:?}");
    }

    #[test]
    fn prose_links_split_into_segments() {
        let p = parse_prose("The [slope](slope) is rise over run.");
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[1],
            Segment {
                text: "slope".into(),
                concept: Some("slope".into())
            }
        );
        assert_eq!(
            parse_prose("no links"),
            vec![Segment {
                text: "no links".into(),
                concept: None
            }]
        );
    }
}
