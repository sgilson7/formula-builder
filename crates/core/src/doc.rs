//! A document: formulas, the augment sheet and prose, as the author typed
//! them, and what the core reads from them.

use serde::{Deserialize, Serialize};

use crate::eval::Env;
use crate::sheet::{occurrences, parse_sheet, ControlKind, Occ, Sheet};
use crate::tex::{parse, Node};
use crate::value::Fx;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub title: String,
    /// one formula per line
    pub formulas: String,
    pub sheet: String,
    pub prose: String,
    /// the mode a shared link opens in: `explore`, `build`, or empty
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub open: String,
}

#[derive(Debug)]
pub struct Model {
    pub lines: Vec<String>,
    pub roots: Vec<Option<Node>>,
    pub errors: Vec<String>,
    pub sheet: Sheet,
    pub occs: Vec<Occ>,
}

impl Model {
    pub fn build(doc: &Doc) -> Model {
        let lines: Vec<String> = doc
            .formulas
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        let mut errors = Vec::new();
        let roots: Vec<Option<Node>> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| match parse(l) {
                Ok(n) => Some(n),
                Err(e) => {
                    errors.push(format!("Formula {}: {e}", i + 1));
                    None
                }
            })
            .collect();
        let sheet = parse_sheet(&doc.sheet);
        errors.extend(sheet.errors.iter().cloned());
        let occs = occurrences(&roots, &sheet);
        for c in &sheet.concepts {
            if !occs.iter().any(|o| sheet.concepts[o.concept].id == c.id) && !roots.is_empty() {
                errors.push(format!(
                    "Sheet line {}: the concept `{}` ({}) is not in any formula.",
                    c.line, c.id, c.pattern_src
                ));
            }
        }
        Model {
            lines,
            roots,
            errors,
            sheet,
            occs,
        }
    }

    pub fn concept_index(&self, id: &str) -> Option<usize> {
        self.sheet.concepts.iter().position(|c| c.id == id)
    }

    /// Every value a control or a hidden line sets, after the reader's changes
    /// (`overrides`) and the current step's `set` lines.
    pub fn values(&self, overrides: &[(String, Fx)], step: Option<usize>) -> Vec<(String, Fx)> {
        let mut out: Vec<(String, Fx)> = Vec::new();
        let pick = |name: &str, default: Fx| {
            overrides
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .unwrap_or(default)
        };
        for c in &self.sheet.concepts {
            if let Some(ctl) = &c.control {
                let name = c.value_name();
                let mut v = pick(&name, ctl.value);
                if let Some((lo, hi)) = c.limit {
                    v = v.max(lo).min(hi);
                }
                out.push((name, v));
            }
        }
        for h in &self.sheet.hidden {
            out.push((h.name.clone(), pick(&h.name, h.control.value)));
        }
        if let Some(i) = step {
            for s in self.sheet.steps.iter().take(i + 1) {
                for (n, v) in &s.set {
                    out.retain(|(m, _)| m != n);
                    out.push((n.clone(), *v));
                }
            }
        }
        out
    }

    pub fn scope(&self, values: &[(String, Fx)]) -> Env<'_> {
        let mut scope = Env::default();
        for r in self.roots.iter().flatten() {
            scope.define(r);
        }
        for (n, v) in values {
            scope.set(n, *v);
        }
        scope
    }

    /// The formula a step shows: its own `formula` line, or the last one
    /// named before it, or the first.
    pub fn step_formula(&self, step: usize) -> usize {
        self.sheet
            .steps
            .iter()
            .take(step + 1)
            .filter_map(|s| s.formula)
            .next_back()
            .unwrap_or(0)
    }

    /// The formulas on the canvas: one per step when there are steps.
    pub fn shown(&self, step: Option<usize>) -> Vec<usize> {
        match step {
            Some(s) if !self.sheet.steps.is_empty() => {
                let f = self.step_formula(s);
                if self.roots.get(f).map(|r| r.is_some()).unwrap_or(false) {
                    vec![f]
                } else {
                    vec![]
                }
            }
            _ => (0..self.roots.len())
                .filter(|i| self.roots[*i].is_some())
                .collect(),
        }
    }

    /// Whether a concept's value shows as a number in place of its symbol.
    pub fn shows_number(&self, ci: usize) -> bool {
        matches!(
            self.sheet.concepts[ci].control.as_ref().map(|c| c.kind),
            Some(ControlKind::Scrub | ControlKind::Edit)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(formulas: &str, sheet: &str) -> Doc {
        Doc {
            title: "t".into(),
            formulas: formulas.into(),
            sheet: sheet.into(),
            prose: String::new(),
            ..Doc::default()
        }
    }

    #[test]
    fn values_take_overrides_limits_and_step_settings() {
        let m = Model::build(&doc("y = m x + b", "concept s: m\n  slider -5 .. 5 step 1 = 2\n  limit 0 .. 3\nconcept i: b\n  scrub -5 .. 5 = 1\nstep one\nstep two\n  set b = 4\n"));
        assert!(m.errors.is_empty(), "{:?}", m.errors);
        let v = m.values(&[("m".into(), Fx::int(5))], None);
        assert_eq!(v[0], ("m".into(), Fx::int(3)), "the limit holds the slider to 3");
        assert_eq!(v[1], ("b".into(), Fx::int(1)));
        let v = m.values(&[], Some(1));
        assert!(v.contains(&("b".into(), Fx::int(4))), "step two sets b");
        let scope = m.scope(&v);
        assert_eq!(
            scope.value_of("y", &[("x".into(), Fx::int(1))]).unwrap(),
            Fx::int(6)
        );
    }

    #[test]
    fn formula_errors_and_unmatched_concepts_are_reported() {
        let m = Model::build(&doc("y = \\foo\nx + 1", "concept z: z\n"));
        assert_eq!(m.errors[0], "Formula 1: The command \\foo is not supported yet.");
        assert_eq!(
            m.errors[1],
            "Sheet line 1: the concept `z` (z) is not in any formula."
        );
        assert_eq!(m.shown(None), vec![1]);
    }

    #[test]
    fn a_step_shows_the_formula_named_last() {
        let m = Model::build(&doc("a\nb\nc", "step one\nstep two\n  formula 3\nstep three\n"));
        assert_eq!(
            (m.step_formula(0), m.step_formula(1), m.step_formula(2)),
            (0, 2, 2)
        );
        assert_eq!(m.shown(Some(2)), vec![2]);
        assert_eq!(m.shown(None), vec![0, 1, 2]);
    }
}
