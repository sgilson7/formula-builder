//! Computes the value of a formula's expression, for scrubbing, sliders and
//! plots. A formula `y = m x + b` defines `y`; a formula `f(w) = ...` defines a
//! function. Values come from the sheet (scrub, slider, edit, hidden), and a
//! plot binds its own variable. Everything is decimal fixed point (`value`).

use crate::tex::{canon, Class, Node, K};
use crate::value::{Fx, R};

/// What a document's formulas define.
#[derive(Debug, Default)]
pub struct Env<'a> {
    pub values: Vec<(String, Fx)>,
    pub defs: Vec<(String, &'a [Node])>,
    pub funcs: Vec<(String, String, &'a [Node])>,
}

impl<'a> Env<'a> {
    /// Reads `name = ...` and `f(x) = ...` from a parsed formula.
    pub fn define(&mut self, root: &'a Node) {
        let kids = &root.kids;
        let Some(eq) = kids.iter().position(|k| matches!(k.k, K::Glyph { cp: 0x3D, .. })) else {
            return;
        };
        let (lhs, rhs) = (&kids[..eq], &kids[eq + 1..]);
        let end = rhs
            .iter()
            .position(|k| k.class() == Class::Rel)
            .unwrap_or(rhs.len());
        let rhs = &rhs[..end];
        match lhs {
            [one] => self.defs.push((canon(one), rhs)),
            [f, open, x, close] if is_cp(open, 0x28) && is_cp(close, 0x29) => {
                self.funcs.push((canon(f), canon(x), rhs))
            }
            [f, d] if matches!(d.k, K::Delim { .. }) => self.funcs.push((canon(f), canon(&d.kids[0]), rhs)),
            _ => {}
        }
    }

    pub fn set(&mut self, name: &str, v: Fx) {
        self.values.retain(|(n, _)| n != name);
        self.values.insert(0, (name.to_string(), v));
    }

    pub fn has_func(&self, name: &str) -> bool {
        self.funcs.iter().any(|(n, _, _)| n == name)
    }

    /// The value of a name, with `bound` (a plot's variable) taking priority.
    pub fn value_of(&self, name: &str, bound: &[(String, Fx)]) -> R {
        Ctx {
            scope: self,
            bound: bound.to_vec(),
            depth: 0,
        }
        .lookup(name)
    }

    pub fn eval_row(&self, row: &[Node], bound: &[(String, Fx)]) -> R {
        Ctx {
            scope: self,
            bound: bound.to_vec(),
            depth: 0,
        }
        .expr_all(row)
    }

    pub fn call(&self, f: &str, x: Fx, bound: &[(String, Fx)]) -> R {
        Ctx {
            scope: self,
            bound: bound.to_vec(),
            depth: 0,
        }
        .call(f, x)
    }

    pub fn derivative(&self, f: &str, x: Fx, bound: &[(String, Fx)]) -> R {
        Ctx {
            scope: self,
            bound: bound.to_vec(),
            depth: 0,
        }
        .derivative(f, x)
    }
}

fn is_cp(n: &Node, want: u32) -> bool {
    matches!(n.k, K::Glyph { cp, .. } if cp == want)
}

struct Ctx<'e, 'a> {
    scope: &'e Env<'a>,
    bound: Vec<(String, Fx)>,
    depth: usize,
}

const H: i64 = 100_000; // a step of 0.0001 for derivatives

impl Ctx<'_, '_> {
    fn lookup(&self, name: &str) -> R {
        if let Some((_, v)) = self.bound.iter().find(|(n, _)| n == name) {
            return Ok(*v);
        }
        if let Some((_, v)) = self.scope.values.iter().find(|(n, _)| n == name) {
            return Ok(*v);
        }
        if self.depth > 24 {
            return Err(format!("{name} is defined in terms of itself."));
        }
        if let Some((_, rhs)) = self.scope.defs.iter().find(|(n, _)| n == name) {
            let inner = Ctx {
                scope: self.scope,
                bound: self.bound.clone(),
                depth: self.depth + 1,
            };
            return inner.expr_all(rhs);
        }
        match name {
            "\\pi" => Ok(Fx::pi()),
            "e" => Ok(Fx::e()),
            _ => Err(format!(
                "{} has no value yet. Give it one in the sheet, or define it in a formula.",
                show(name)
            )),
        }
    }

    fn call(&self, f: &str, x: Fx) -> R {
        let Some((_, param, rhs)) = self.scope.funcs.iter().find(|(n, _, _)| n == f) else {
            return Err(format!("{} is not a function defined in a formula.", show(f)));
        };
        if self.depth > 24 {
            return Err(format!("{} is defined in terms of itself.", show(f)));
        }
        let mut bound = vec![(param.clone(), x)];
        bound.extend(self.bound.iter().filter(|(n, _)| n != param).cloned());
        Ctx {
            scope: self.scope,
            bound,
            depth: self.depth + 1,
        }
        .expr_all(rhs)
    }

    fn derivative(&self, f: &str, x: Fx) -> R {
        let up = self.call(f, Fx(x.0 + H))?;
        let down = self.call(f, Fx(x.0 - H))?;
        up.sub(down)?.div(Fx(2 * H))
    }

    fn expr_all(&self, items: &[Node]) -> R {
        let mut at = 0;
        let v = self.sum(items, &mut at)?;
        if at < items.len() {
            return Err("Part of this expression could not be computed.".into());
        }
        Ok(v)
    }

    fn sum(&self, items: &[Node], at: &mut usize) -> R {
        let mut v = self.term(items, at)?;
        while let Some(n) = items.get(*at) {
            if is_cp(n, 0x2B) {
                *at += 1;
                v = v.add(self.term(items, at)?)?;
            } else if is_cp(n, 0x2212) {
                *at += 1;
                v = v.sub(self.term(items, at)?)?;
            } else if is_cp(n, 0xB1) {
                *at += 1;
                v = v.add(self.term(items, at)?)?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn term(&self, items: &[Node], at: &mut usize) -> R {
        let mut v = self.factor(items, at)?;
        while let Some(n) = items.get(*at) {
            if is_cp(n, 0x22C5) || is_cp(n, 0xD7) || is_cp(n, 0x2217) {
                *at += 1;
                v = v.mul(self.factor(items, at)?)?;
            } else if is_cp(n, 0xF7) || is_cp(n, 0x2F) {
                *at += 1;
                v = v.div(self.factor(items, at)?)?;
            } else if starts_factor(n) {
                v = v.mul(self.factor(items, at)?)?;
            } else {
                break;
            }
        }
        Ok(v)
    }

    fn factor(&self, items: &[Node], at: &mut usize) -> R {
        let Some(n) = items.get(*at) else {
            return Err("An expression ends where a value was expected.".into());
        };
        if is_cp(n, 0x2212) {
            *at += 1;
            return Ok(self.factor(items, at)?.neg());
        }
        if is_cp(n, 0x2B) {
            *at += 1;
            return self.factor(items, at);
        }
        self.primary(items, at)
    }

    /// Reads `( ... )` starting at an open parenthesis.
    fn paren(&self, items: &[Node], at: &mut usize) -> R {
        *at += 1;
        let v = self.sum(items, at)?;
        match items.get(*at) {
            Some(c) if is_cp(c, 0x29) || is_cp(c, 0x5D) => {
                *at += 1;
                Ok(v)
            }
            // (w - 3)^2: the power attaches to the closing parenthesis
            Some(c)
                if matches!(
                    c.k,
                    K::Scripts {
                        sub: false,
                        sup: true
                    }
                ) && (is_cp(&c.kids[0], 0x29) || is_cp(&c.kids[0], 0x5D)) =>
            {
                *at += 1;
                v.pow(self.expr_all(&c.kids[1].kids)?)
            }
            _ => Err("A ( has no matching ).".into()),
        }
    }

    fn primary(&self, items: &[Node], at: &mut usize) -> R {
        let n = &items[*at];
        *at += 1;
        match &n.k {
            K::Num(t) => Fx::parse(t).ok_or_else(|| format!("{t} is not a number.")),
            K::Glyph { cp: 0x28 | 0x5B, .. } => {
                *at -= 1;
                self.paren(items, at)
            }
            K::Glyph { cp: 0x2207, .. } => {
                // \nabla f(x): the derivative of a one-variable function
                let f = items.get(*at).map(canon).unwrap_or_default();
                *at += 1;
                match items.get(*at) {
                    Some(o) if is_cp(o, 0x28) => {
                        let x = self.paren(items, at)?;
                        self.derivative(&f, x)
                    }
                    _ => Err("\\nabla needs a function and its argument, like \\nabla f(x).".into()),
                }
            }
            K::Row => self.expr_all(&n.kids),
            K::Frac { .. } => self
                .expr_all(&n.kids[0].kids)?
                .div(self.expr_all(&n.kids[1].kids)?),
            K::Sqrt => {
                let body = self.expr_all(&n.kids[0].kids)?;
                match n.kids.get(1) {
                    Some(ix) => body.pow(Fx::ONE.div(self.expr_all(&ix.kids)?)?),
                    None => body.sqrt(),
                }
            }
            K::Delim { left, .. } => {
                let v = self.expr_all(&n.kids[0].kids)?;
                Ok(if *left == 0x7C { Fx(v.0.abs()) } else { v })
            }
            K::Scripts { sub, sup } => self.scripts(n, *sub, *sup, items, at),
            K::Word { text, .. } => {
                let arg = match items.get(*at) {
                    Some(o) if is_cp(o, 0x28) => self.paren(items, at)?,
                    Some(_) => self.factor(items, at)?,
                    None => return Err(format!("\\{text} needs an argument.")),
                };
                apply(text, arg)
            }
            K::Glyph { cp, .. } if !is_name(*cp) => Err(format!(
                "The symbol {} cannot be computed here.",
                char::from_u32(*cp).map(String::from).unwrap_or_default()
            )),
            K::Glyph { .. } => {
                let name = canon(n);
                if self.scope.has_func(&name) {
                    return match items.get(*at) {
                        Some(o) if is_cp(o, 0x28) => {
                            let x = self.paren(items, at)?;
                            self.call(&name, x)
                        }
                        _ => Err(format!("{} is a function; give it an argument.", show(&name))),
                    };
                }
                self.lookup(&name)
            }
            K::Space(_) => {
                if *at < items.len() {
                    self.factor(items, at)
                } else {
                    Ok(Fx::ONE)
                }
            }
            _ => Err("This part of the formula cannot be computed.".into()),
        }
    }

    fn scripts(&self, n: &Node, sub: bool, sup: bool, items: &[Node], at: &mut usize) -> R {
        let base = &n.kids[0];
        let named = if sub {
            format!("{}_{{{}}}", canon(base), canon(&n.kids[1]))
        } else {
            canon(base)
        };
        let sup_node = if sup { n.kids.last() } else { None };
        // f'(x): a derivative
        if let Some(s) = sup_node {
            if s.kids.len() == 1 && is_cp(&s.kids[0], 0x2032) && self.scope.has_func(&named) {
                return match items.get(*at) {
                    Some(o) if is_cp(o, 0x28) => {
                        let x = self.paren(items, at)?;
                        self.derivative(&named, x)
                    }
                    _ => Err(format!("{}' needs an argument.", show(&named))),
                };
            }
        }
        let base_value = if sub {
            self.lookup(&named)?
        } else if let K::Word { text, .. } = &base.k {
            // sin^2 x
            let arg = match items.get(*at) {
                Some(o) if is_cp(o, 0x28) => self.paren(items, at)?,
                _ => self.factor(items, at)?,
            };
            apply(text, arg)?
        } else {
            match items.get(*at) {
                Some(o) if self.scope.has_func(&named) && is_cp(o, 0x28) => {
                    let x = self.paren(items, at)?;
                    self.call(&named, x)?
                }
                _ => self.expr_all(std::slice::from_ref(base))?,
            }
        };
        match sup_node {
            Some(s) => base_value.pow(self.expr_all(&s.kids)?),
            None => Ok(base_value),
        }
    }
}

/// Letters, Greek and double-struck letters name values; other symbols do not.
fn is_name(cp: u32) -> bool {
    matches!(cp, 0x41..=0x5A | 0x61..=0x7A | 0x210E | 0x1D434..=0x1D467 | 0x1D6E2..=0x1D71B | 0x391..=0x3C9 | 0x221E)
}

fn starts_factor(n: &Node) -> bool {
    !matches!(n.class(), Class::Bin | Class::Rel | Class::Close | Class::Punct) && !is_cp(n, 0x2F)
}

fn apply(f: &str, x: Fx) -> R {
    match f {
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "exp" => x.exp(),
        "ln" => x.ln(),
        "log" => x.ln()?.div(Fx::int(10).ln()?),
        "sqrt" => x.sqrt(),
        "sinh" => x.exp()?.sub(x.neg().exp()?)?.div(Fx::int(2)),
        "cosh" => x.exp()?.add(x.neg().exp()?)?.div(Fx::int(2)),
        "tanh" => {
            let (a, b) = (x.exp()?, x.neg().exp()?);
            a.sub(b)?.div(a.add(b)?)
        }
        other => Err(format!("\\{other} cannot be computed yet.")),
    }
}

/// A spelling as a reader would type it, for messages.
pub fn show(name: &str) -> String {
    name.replace("\\", "").replace("{", "").replace("}", "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tex::parse;

    fn value(env_src: &[&str], expr: &str, vals: &[(&str, &str)]) -> R {
        let roots: Vec<Node> = env_src.iter().map(|s| parse(s).unwrap()).collect();
        let mut scope = Env::default();
        for r in &roots {
            scope.define(r);
        }
        for (n, v) in vals {
            scope.set(n, Fx::parse(v).unwrap());
        }
        let e = parse(expr).unwrap();
        scope.eval_row(&e.kids, &[])
    }

    #[test]
    fn a_line_computes_with_implicit_multiplication() {
        // y = 1.5 * 2 + 1 = 4
        let v = value(&["y = m x + b"], "y", &[("m", "1.5"), ("x", "2"), ("b", "1")]).unwrap();
        assert_eq!(v, Fx::int(4));
    }

    #[test]
    fn precedence_fractions_powers_and_parentheses() {
        assert_eq!(value(&[], "2 + 3 \\cdot 4", &[]).unwrap(), Fx::int(14));
        assert_eq!(value(&[], "(2 + 3) 4", &[]).unwrap(), Fx::int(20));
        assert_eq!(
            value(&[], "\\frac{1}{4} - 2^3", &[]).unwrap(),
            Fx::parse("-7.75").unwrap()
        );
        assert_eq!(value(&[], "-x^2", &[("x", "3")]).unwrap(), Fx::int(-9));
        assert_eq!(
            value(&[], "\\sqrt{16} + |-2|", &[]).unwrap_err(),
            "The symbol | cannot be computed here."
        );
        assert_eq!(
            value(&[], "\\sqrt{16} + \\left|-2\\right|", &[]).unwrap(),
            Fx::int(6)
        );
    }

    #[test]
    fn subscripted_names_functions_and_derivatives() {
        // f(w) = (w - 3)^2 / 2, so f'(w) = w - 3; at w_k = 1: 1 - 0.5 * (1 - 3) = 2
        let src = [
            "f(w) = \\frac{(w - 3)^2}{2}",
            "w_{k+1} = w_k - \\alpha \\nabla f(w_k)",
        ];
        let v = value(&src, "w_{k+1}", &[("w_{k}", "1"), ("\\alpha", "0.5")]).unwrap();
        assert!((v.0 - 2 * crate::value::S).abs() < 100, "{v:?}");
        let d = value(&src, "f'(5)", &[]).unwrap();
        assert!((d.0 - 2 * crate::value::S).abs() < 100);
    }

    #[test]
    fn missing_values_and_loops_are_named() {
        assert_eq!(
            value(&[], "a + 1", &[]).unwrap_err(),
            "a has no value yet. Give it one in the sheet, or define it in a formula."
        );
        assert_eq!(
            value(&["y = y + 1"], "y", &[]).unwrap_err(),
            "y is defined in terms of itself."
        );
    }
}
