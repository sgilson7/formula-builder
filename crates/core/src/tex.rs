//! A parser for the subset of LaTeX math the tool supports. It builds a tree
//! whose nodes carry ids, so a concept can attach to any part of a formula
//! (the paper found half of all augmentations sit on expressions larger than
//! one symbol). Every argument, numerator, denominator and body is a `Row`, so
//! a concept pattern matches runs of siblings inside rows.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acc {
    Hat,
    Tilde,
    Dot,
    Ddot,
    Vec,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum K {
    Row,
    Glyph {
        cp: u32,
        class: Class,
    },
    Num(String),
    /// Upright letters: a function name, `\text` or `\mathrm`.
    Word {
        text: String,
        class: Class,
        limits: bool,
    },
    /// kids: numerator, denominator
    Frac {
        display: Option<bool>,
    },
    /// kids: base, then the subscript if `sub`, then the superscript if `sup`
    Scripts {
        sub: bool,
        sup: bool,
    },
    /// kids: body, then the index if any
    Sqrt,
    BigOp {
        cp: u32,
        limits: bool,
    },
    /// kids: body
    Delim {
        left: u32,
        right: u32,
    },
    /// kids: body
    Accent(Acc),
    /// kids: body
    Brace {
        below: bool,
    },
    /// thousandths of an em
    Space(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: usize,
    /// the largest id in this subtree; ids are given in preorder
    pub end: usize,
    pub k: K,
    pub kids: Vec<Node>,
    pub span: (usize, usize),
}

impl Node {
    fn new(k: K, kids: Vec<Node>, span: (usize, usize)) -> Node {
        Node {
            id: 0,
            end: 0,
            k,
            kids,
            span,
        }
    }

    pub fn class(&self) -> Class {
        match &self.k {
            K::Glyph { class, .. } => *class,
            K::Word { class, .. } => *class,
            K::BigOp { .. } => Class::Op,
            K::Scripts { .. } => self.kids[0].class(),
            K::Frac { .. } | K::Delim { .. } => Class::Inner,
            _ => Class::Ord,
        }
    }

    /// Every node in the subtree, this one first.
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Node>) {
        out.push(self);
        for k in &self.kids {
            k.walk(out);
        }
    }

    pub fn find(&self, id: usize) -> Option<&Node> {
        if self.id == id {
            return Some(self);
        }
        self.kids
            .iter()
            .find(|k| k.id <= id && id <= k.end)
            .and_then(|k| k.find(id))
    }

    /// The chain of ids from the root down to `id`.
    pub fn path_to(&self, id: usize) -> Vec<usize> {
        let mut out = vec![self.id];
        let mut at = self;
        while at.id != id {
            match at.kids.iter().find(|k| k.id <= id && id <= k.end) {
                Some(k) => {
                    out.push(k.id);
                    at = k;
                }
                None => break,
            }
        }
        out
    }
}

fn number(n: &mut Node, next: &mut usize) {
    n.id = *next;
    *next += 1;
    for k in &mut n.kids {
        number(k, next);
    }
    n.end = *next - 1;
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Cmd(String),
    Ch(char),
    Text(String),
    Open,
    Close,
    Sup,
    Sub,
}

#[derive(Debug, Clone)]
struct Token {
    t: Tok,
    at: usize,
    end: usize,
}

const RAW_ARG: &[&str] = &["text", "mathrm", "operatorname", "mathbb", "textrm", "mathit"];

fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let end_of = |i: usize| chars.get(i).map(|c| c.0).unwrap_or(src.len());
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '%' {
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '\\' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1.is_ascii_alphabetic() {
                j += 1;
            }
            if j == i + 1 {
                let ch = chars
                    .get(j)
                    .map(|c| c.1)
                    .ok_or("A formula ends with a lone \\.")?;
                out.push(Token {
                    t: Tok::Cmd(ch.to_string()),
                    at,
                    end: end_of(j + 1),
                });
                i = j + 1;
                continue;
            }
            let name: String = chars[i + 1..j].iter().map(|c| c.1).collect();
            i = j;
            while i < chars.len() && chars[i].1.is_whitespace() {
                i += 1;
            }
            if RAW_ARG.contains(&name.as_str()) {
                out.push(Token {
                    t: Tok::Cmd(name.clone()),
                    at,
                    end: end_of(i),
                });
                if chars.get(i).map(|c| c.1) != Some('{') {
                    return Err(format!(
                        "\\{name} needs its text in braces, like \\{name}{{...}}."
                    ));
                }
                let start = i + 1;
                let mut depth = 1;
                i += 1;
                while i < chars.len() {
                    match chars[i].1 {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if depth != 0 {
                    return Err(format!("The braces after \\{name} are not closed."));
                }
                let text: String = chars[start..i].iter().map(|c| c.1).collect();
                out.push(Token {
                    t: Tok::Text(text),
                    at: end_of(start),
                    end: end_of(i + 1),
                });
                i += 1;
                continue;
            }
            out.push(Token {
                t: Tok::Cmd(name),
                at,
                end: end_of(i),
            });
            continue;
        }
        let t = match c {
            '{' => Tok::Open,
            '}' => Tok::Close,
            '^' => Tok::Sup,
            '_' => Tok::Sub,
            other => Tok::Ch(other),
        };
        out.push(Token {
            t,
            at,
            end: end_of(i + 1),
        });
        i += 1;
    }
    Ok(out)
}

/// Greek letters: name, lowercase italic or uppercase upright code point.
const GREEK: &[(&str, u32)] = &[
    ("alpha", 0x1D6FC),
    ("beta", 0x1D6FD),
    ("gamma", 0x1D6FE),
    ("delta", 0x1D6FF),
    ("varepsilon", 0x1D700),
    ("zeta", 0x1D701),
    ("eta", 0x1D702),
    ("theta", 0x1D703),
    ("iota", 0x1D704),
    ("kappa", 0x1D705),
    ("lambda", 0x1D706),
    ("mu", 0x1D707),
    ("nu", 0x1D708),
    ("xi", 0x1D709),
    ("pi", 0x1D70B),
    ("rho", 0x1D70C),
    ("varsigma", 0x1D70D),
    ("sigma", 0x1D70E),
    ("tau", 0x1D70F),
    ("upsilon", 0x1D710),
    ("varphi", 0x1D711),
    ("chi", 0x1D712),
    ("psi", 0x1D713),
    ("omega", 0x1D714),
    ("epsilon", 0x1D716),
    ("vartheta", 0x1D717),
    ("phi", 0x1D719),
    ("varrho", 0x1D71A),
    ("varpi", 0x1D71B),
    ("Gamma", 0x0393),
    ("Delta", 0x0394),
    ("Theta", 0x0398),
    ("Lambda", 0x039B),
    ("Xi", 0x039E),
    ("Pi", 0x03A0),
    ("Sigma", 0x03A3),
    ("Upsilon", 0x03A5),
    ("Phi", 0x03A6),
    ("Psi", 0x03A8),
    ("Omega", 0x03A9),
];

const SYMBOLS: &[(&str, u32, Class)] = &[
    ("cdot", 0x22C5, Class::Bin),
    ("times", 0xD7, Class::Bin),
    ("div", 0xF7, Class::Bin),
    ("pm", 0xB1, Class::Bin),
    ("mp", 0x2213, Class::Bin),
    ("circ", 0x2218, Class::Bin),
    ("ast", 0x2217, Class::Bin),
    ("cup", 0x222A, Class::Bin),
    ("cap", 0x2229, Class::Bin),
    ("land", 0x2227, Class::Bin),
    ("lor", 0x2228, Class::Bin),
    ("le", 0x2264, Class::Rel),
    ("leq", 0x2264, Class::Rel),
    ("ge", 0x2265, Class::Rel),
    ("geq", 0x2265, Class::Rel),
    ("ne", 0x2260, Class::Rel),
    ("neq", 0x2260, Class::Rel),
    ("approx", 0x2248, Class::Rel),
    ("equiv", 0x2261, Class::Rel),
    ("sim", 0x223C, Class::Rel),
    ("simeq", 0x2243, Class::Rel),
    ("propto", 0x221D, Class::Rel),
    ("to", 0x2192, Class::Rel),
    ("rightarrow", 0x2192, Class::Rel),
    ("leftarrow", 0x2190, Class::Rel),
    ("gets", 0x2190, Class::Rel),
    ("leftrightarrow", 0x2194, Class::Rel),
    ("Rightarrow", 0x21D2, Class::Rel),
    ("Leftarrow", 0x21D0, Class::Rel),
    ("iff", 0x21D4, Class::Rel),
    ("implies", 0x21D2, Class::Rel),
    ("mapsto", 0x21A6, Class::Rel),
    ("in", 0x2208, Class::Rel),
    ("notin", 0x2209, Class::Rel),
    ("subset", 0x2282, Class::Rel),
    ("subseteq", 0x2286, Class::Rel),
    ("nabla", 0x2207, Class::Ord),
    ("partial", 0x1D715, Class::Ord),
    ("infty", 0x221E, Class::Ord),
    ("ell", 0x2113, Class::Ord),
    ("emptyset", 0x2205, Class::Ord),
    ("forall", 0x2200, Class::Ord),
    ("exists", 0x2203, Class::Ord),
    ("neg", 0xAC, Class::Ord),
    ("prime", 0x2032, Class::Ord),
    ("ldots", 0x2026, Class::Inner),
    ("dots", 0x2026, Class::Inner),
    ("cdots", 0x22EF, Class::Inner),
    ("langle", 0x27E8, Class::Open),
    ("rangle", 0x27E9, Class::Close),
    ("lfloor", 0x230A, Class::Open),
    ("rfloor", 0x230B, Class::Close),
    ("lceil", 0x2308, Class::Open),
    ("rceil", 0x2309, Class::Close),
    ("{", 0x7B, Class::Open),
    ("}", 0x7D, Class::Close),
    ("|", 0x2016, Class::Ord),
    ("%", 0x25, Class::Ord),
];

const FUNCTIONS: &[(&str, bool)] = &[
    ("sin", false),
    ("cos", false),
    ("tan", false),
    ("sec", false),
    ("csc", false),
    ("cot", false),
    ("sinh", false),
    ("cosh", false),
    ("tanh", false),
    ("arcsin", false),
    ("arccos", false),
    ("arctan", false),
    ("log", false),
    ("ln", false),
    ("exp", false),
    ("det", true),
    ("lim", true),
    ("max", true),
    ("min", true),
    ("sup", true),
    ("inf", true),
    ("arg", false),
    ("gcd", true),
];

const SPACES: &[(&str, i32)] = &[
    (",", 167),
    (":", 222),
    (">", 222),
    (";", 278),
    ("!", -167),
    ("quad", 1000),
    ("qquad", 2000),
    (" ", 333),
];

const ACCENTS: &[(&str, Acc)] = &[
    ("hat", Acc::Hat),
    ("widehat", Acc::Hat),
    ("tilde", Acc::Tilde),
    ("widetilde", Acc::Tilde),
    ("dot", Acc::Dot),
    ("ddot", Acc::Ddot),
    ("vec", Acc::Vec),
    ("bar", Acc::Bar),
    ("overline", Acc::Bar),
];

pub fn italic(c: char) -> u32 {
    match c {
        'h' => 0x210E,
        'a'..='z' => 0x1D44E + (c as u32 - 'a' as u32),
        'A'..='Z' => 0x1D434 + (c as u32 - 'A' as u32),
        other => other as u32,
    }
}

fn double_struck(c: char) -> u32 {
    match c {
        'C' => 0x2102,
        'H' => 0x210D,
        'N' => 0x2115,
        'P' => 0x2119,
        'Q' => 0x211A,
        'R' => 0x211D,
        'Z' => 0x2124,
        'A'..='Z' => 0x1D538 + (c as u32 - 'A' as u32),
        other => other as u32,
    }
}

fn char_class(c: char) -> Option<(u32, Class)> {
    Some(match c {
        '+' => (0x2B, Class::Bin),
        '-' => (0x2212, Class::Bin),
        '*' => (0x2217, Class::Bin),
        '=' | '<' | '>' => (c as u32, Class::Rel),
        ',' | ';' => (c as u32, Class::Punct),
        ':' => (c as u32, Class::Rel),
        '(' | '[' => (c as u32, Class::Open),
        ')' | ']' | '!' | '?' => (c as u32, Class::Close),
        '|' | '/' => (c as u32, Class::Ord),
        _ => return None,
    })
}

struct Parser<'a> {
    toks: Vec<Token>,
    i: usize,
    src: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|t| &t.t)
    }

    fn at(&self) -> usize {
        self.toks.get(self.i).map(|t| t.at).unwrap_or(self.src.len())
    }

    fn last_end(&self) -> usize {
        if self.i == 0 {
            0
        } else {
            self.toks[self.i - 1].end
        }
    }

    fn row(&mut self) -> Result<Node, String> {
        let start = self.at();
        let mut items: Vec<Node> = Vec::new();
        loop {
            match self.peek() {
                None | Some(Tok::Close) => break,
                Some(Tok::Cmd(c)) if c == "right" => break,
                Some(Tok::Sup) | Some(Tok::Sub) => {
                    let sup = self.peek() == Some(&Tok::Sup);
                    self.i += 1;
                    let arg = self.arg()?;
                    attach(&mut items, arg, sup, start)?;
                }
                Some(Tok::Ch('\'')) => {
                    let at = self.at();
                    self.i += 1;
                    let prime = Node::new(
                        K::Glyph {
                            cp: 0x2032,
                            class: Class::Ord,
                        },
                        vec![],
                        (at, at + 1),
                    );
                    let arg = Node::new(K::Row, vec![prime], (at, at + 1));
                    attach(&mut items, arg, true, start)?;
                }
                _ => {
                    let atom = self.atom(false)?;
                    items.push(atom);
                }
            }
        }
        Ok(Node::new(K::Row, items, (start, self.last_end().max(start))))
    }

    /// A script or command argument: a braced group, or one atom; always a Row.
    fn arg(&mut self) -> Result<Node, String> {
        if self.peek() == Some(&Tok::Open) {
            return self.group();
        }
        if self.peek().is_none() {
            return Err("A ^, _ or command at the end of the formula is missing its argument.".into());
        }
        let a = self.atom(true)?;
        let span = a.span;
        Ok(Node::new(K::Row, vec![a], span))
    }

    fn group(&mut self) -> Result<Node, String> {
        let start = self.at();
        self.i += 1;
        let mut inner = self.row()?;
        if self.peek() != Some(&Tok::Close) {
            return Err("A { has no matching }.".into());
        }
        self.i += 1;
        inner.span = (start, self.last_end());
        Ok(inner)
    }

    fn delim(&mut self) -> Result<u32, String> {
        let t = self.toks.get(self.i).map(|t| t.t.clone());
        self.i += 1;
        Ok(match t {
            Some(Tok::Ch('.')) => 0,
            Some(Tok::Ch(c)) if "()[]|/".contains(c) => c as u32,
            Some(Tok::Cmd(c)) => match c.as_str() {
                "{" => 0x7B,
                "}" => 0x7D,
                "|" => 0x2016,
                "langle" => 0x27E8,
                "rangle" => 0x27E9,
                "lfloor" => 0x230A,
                "rfloor" => 0x230B,
                "lceil" => 0x2308,
                "rceil" => 0x2309,
                other => return Err(format!("\\left and \\right do not take \\{other}.")),
            },
            _ => return Err("\\left and \\right need a delimiter such as ( [ | or .".into()),
        })
    }

    fn atom(&mut self, single: bool) -> Result<Node, String> {
        let start = self.at();
        let tok = self.toks[self.i].t.clone();
        self.i += 1;
        let span = |p: &Parser| (start, p.last_end());
        match tok {
            Tok::Open => {
                self.i -= 1;
                self.group()
            }
            Tok::Close => Err("A } has no matching {.".into()),
            Tok::Sup | Tok::Sub => Err("A ^ or _ needs something before it.".into()),
            Tok::Text(_) => Err("Text appeared where math was expected.".into()),
            Tok::Ch(c) if c.is_ascii_alphabetic() => Ok(Node::new(
                K::Glyph {
                    cp: italic(c),
                    class: Class::Ord,
                },
                vec![],
                span(self),
            )),
            Tok::Ch(c) if c.is_ascii_digit() || c == '.' => {
                let mut text = c.to_string();
                if !single {
                    loop {
                        match self.peek() {
                            Some(Tok::Ch(d)) if d.is_ascii_digit() || (*d == '.' && !text.contains('.')) => {
                                text.push(*d);
                                self.i += 1;
                            }
                            _ => break,
                        }
                    }
                }
                if text == "." {
                    return Ok(Node::new(
                        K::Glyph {
                            cp: 0x2E,
                            class: Class::Punct,
                        },
                        vec![],
                        span(self),
                    ));
                }
                Ok(Node::new(K::Num(text), vec![], span(self)))
            }
            Tok::Ch(c) => match char_class(c) {
                Some((cp, class)) => Ok(Node::new(K::Glyph { cp, class }, vec![], span(self))),
                None => Err(format!("The character {c} is not supported in a formula.")),
            },
            Tok::Cmd(name) => self.command(&name, start),
        }
    }

    fn raw_text(&mut self) -> String {
        match self.toks.get(self.i).map(|t| t.t.clone()) {
            Some(Tok::Text(t)) => {
                self.i += 1;
                t
            }
            _ => String::new(),
        }
    }

    fn command(&mut self, name: &str, start: usize) -> Result<Node, String> {
        let span = |p: &Parser| (start, p.last_end());
        if let Some((_, cp)) = GREEK.iter().find(|(n, _)| *n == name) {
            return Ok(Node::new(
                K::Glyph {
                    cp: *cp,
                    class: Class::Ord,
                },
                vec![],
                span(self),
            ));
        }
        if let Some((_, cp, class)) = SYMBOLS.iter().find(|(n, _, _)| *n == name) {
            return Ok(Node::new(
                K::Glyph {
                    cp: *cp,
                    class: *class,
                },
                vec![],
                span(self),
            ));
        }
        if let Some((n, limits)) = FUNCTIONS.iter().find(|(n, _)| *n == name) {
            let k = K::Word {
                text: n.to_string(),
                class: Class::Op,
                limits: *limits,
            };
            return Ok(Node::new(k, vec![], span(self)));
        }
        if let Some((_, w)) = SPACES.iter().find(|(n, _)| *n == name) {
            return Ok(Node::new(K::Space(*w), vec![], span(self)));
        }
        if let Some((_, acc)) = ACCENTS.iter().find(|(n, _)| *n == name) {
            let body = self.arg()?;
            return Ok(Node::new(K::Accent(*acc), vec![body], span(self)));
        }
        match name {
            "sum" | "prod" => {
                let cp = if name == "sum" { 0x2211 } else { 0x220F };
                Ok(Node::new(K::BigOp { cp, limits: true }, vec![], span(self)))
            }
            "int" | "iint" | "oint" => {
                let cp = match name {
                    "int" => 0x222B,
                    "iint" => 0x222C,
                    _ => 0x222E,
                };
                Ok(Node::new(K::BigOp { cp, limits: false }, vec![], span(self)))
            }
            "frac" | "dfrac" | "tfrac" => {
                let num = self.arg()?;
                let den = self.arg()?;
                let display = match name {
                    "dfrac" => Some(true),
                    "tfrac" => Some(false),
                    _ => None,
                };
                Ok(Node::new(K::Frac { display }, vec![num, den], span(self)))
            }
            "sqrt" => {
                let mut index = None;
                if self.peek() == Some(&Tok::Ch('[')) {
                    let s = self.at();
                    self.i += 1;
                    let mut items = Vec::new();
                    while self.peek().is_some() && self.peek() != Some(&Tok::Ch(']')) {
                        items.push(self.atom(false)?);
                    }
                    if self.peek().is_none() {
                        return Err("The [ after \\sqrt has no matching ].".into());
                    }
                    self.i += 1;
                    index = Some(Node::new(K::Row, items, (s, self.last_end())));
                }
                let body = self.arg()?;
                let mut kids = vec![body];
                kids.extend(index);
                Ok(Node::new(K::Sqrt, kids, span(self)))
            }
            "left" => {
                let left = self.delim()?;
                let body = self.row()?;
                if self.peek() != Some(&Tok::Cmd("right".into())) {
                    return Err("\\left has no matching \\right.".into());
                }
                self.i += 1;
                let right = self.delim()?;
                Ok(Node::new(K::Delim { left, right }, vec![body], span(self)))
            }
            "right" => Err("\\right has no matching \\left.".into()),
            "underbrace" | "overbrace" => {
                let body = self.arg()?;
                Ok(Node::new(
                    K::Brace {
                        below: name == "underbrace",
                    },
                    vec![body],
                    span(self),
                ))
            }
            "text" | "textrm" | "mathrm" => {
                let text = self.raw_text();
                Ok(Node::new(
                    K::Word {
                        text,
                        class: Class::Ord,
                        limits: false,
                    },
                    vec![],
                    span(self),
                ))
            }
            "mathit" => {
                let text = self.raw_text();
                let kids: Vec<Node> = text
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic())
                    .map(|c| {
                        Node::new(
                            K::Glyph {
                                cp: italic(c),
                                class: Class::Ord,
                            },
                            vec![],
                            span(self),
                        )
                    })
                    .collect();
                Ok(Node::new(K::Row, kids, span(self)))
            }
            "operatorname" => {
                let text = self.raw_text();
                Ok(Node::new(
                    K::Word {
                        text,
                        class: Class::Op,
                        limits: false,
                    },
                    vec![],
                    span(self),
                ))
            }
            "mathbb" => {
                let text = self.raw_text();
                let c = text.trim().chars().next().unwrap_or('R');
                Ok(Node::new(
                    K::Glyph {
                        cp: double_struck(c),
                        class: Class::Ord,
                    },
                    vec![],
                    span(self),
                ))
            }
            other => Err(format!("The command \\{other} is not supported yet.")),
        }
    }
}

fn attach(items: &mut Vec<Node>, arg: Node, sup: bool, start: usize) -> Result<(), String> {
    let base = match items.pop() {
        Some(b) => b,
        None => Node::new(K::Row, vec![], (start, start)),
    };
    let (mut kids, has_sub, has_sup, span0) = match base.k {
        K::Scripts { sub, sup } => (base.kids, sub, sup, base.span.0),
        _ => {
            let s = base.span.0;
            (vec![base], false, false, s)
        }
    };
    if (sup && has_sup) || (!sup && has_sub) {
        return Err(if sup {
            "A symbol has two superscripts; group them with braces."
        } else {
            "A symbol has two subscripts; group them with braces."
        }
        .into());
    }
    let end = arg.span.1;
    if sup {
        kids.push(arg);
    } else {
        kids.insert(1, arg);
    }
    let sub = has_sub || !sup;
    let up = has_sup || sup;
    items.push(Node::new(K::Scripts { sub, sup: up }, kids, (span0, end)));
    Ok(())
}

/// Parses one formula. Errors name what is wrong in words a reader can act on.
pub fn parse(src: &str) -> Result<Node, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, i: 0, src };
    let mut root = p.row()?;
    if p.i < p.toks.len() {
        return Err(match p.toks[p.i].t {
            Tok::Close => "A } has no matching {.".into(),
            _ => "\\right has no matching \\left.".into(),
        });
    }
    let mut next = 0;
    number(&mut root, &mut next);
    Ok(root)
}

/// A canonical spelling of a node, independent of spacing and optional
/// braces. Two nodes with the same spelling are the same expression; names of
/// values (`w_{k}`) are spellings.
pub fn canon(n: &Node) -> String {
    spell(n).trim_end().to_string()
}

fn spell(n: &Node) -> String {
    let all = |n: &Node| n.kids.iter().map(spell).collect::<String>();
    match &n.k {
        K::Row => {
            if n.kids.len() == 1 {
                spell(&n.kids[0])
            } else {
                all(n)
            }
        }
        K::Glyph { cp, .. } => {
            if let Some((name, _)) = GREEK.iter().find(|(_, c)| c == cp) {
                return format!("\\{name} ");
            }
            if let Some((name, _, _)) = SYMBOLS.iter().find(|(_, c, _)| c == cp) {
                return format!("\\{name} ");
            }
            match *cp {
                0x210E => "h".into(),
                0x1D44E..=0x1D467 => char::from_u32('a' as u32 + cp - 0x1D44E)
                    .map(String::from)
                    .unwrap_or_default(),
                0x1D434..=0x1D44D => char::from_u32('A' as u32 + cp - 0x1D434)
                    .map(String::from)
                    .unwrap_or_default(),
                0x2212 => "-".into(),
                c => char::from_u32(c).map(String::from).unwrap_or_default(),
            }
        }
        K::Num(t) => t.clone(),
        K::Word { text, .. } => format!("\\{text} "),
        K::Frac { .. } => format!("\\frac{{{}}}{{{}}}", canon(&n.kids[0]), canon(&n.kids[1])),
        K::Scripts { sub, sup } => {
            let mut s = canon(&n.kids[0]);
            let mut i = 1;
            if *sub {
                s += &format!("_{{{}}}", canon(&n.kids[i]));
                i += 1;
            }
            if *sup {
                s += &format!("^{{{}}}", canon(&n.kids[i]));
            }
            s
        }
        K::Sqrt => format!("\\sqrt{{{}}}", canon(&n.kids[0])),
        K::BigOp { cp, .. } => format!("\\op{cp:X} "),
        K::Delim { .. } => format!("({})", canon(&n.kids[0])),
        K::Accent(a) => format!("\\{a:?}{{{}}}", canon(&n.kids[0])),
        K::Brace { .. } => spell(&n.kids[0]),
        K::Space(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(n: &Node) -> Vec<String> {
        n.kids
            .iter()
            .map(|k| {
                format!("{:?}", k.k)
                    .split([' ', '{', '('])
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn a_line_parses_into_a_row_of_atoms_with_classes() {
        let n = parse("y = m x + b").unwrap();
        assert_eq!(kinds(&n), ["Glyph"; 6]);
        let classes: Vec<Class> = n.kids.iter().map(|k| k.class()).collect();
        assert_eq!(
            classes,
            [
                Class::Ord,
                Class::Rel,
                Class::Ord,
                Class::Ord,
                Class::Bin,
                Class::Ord
            ]
        );
        assert_eq!(
            n.kids[2].k,
            K::Glyph {
                cp: 0x1D45A,
                class: Class::Ord
            },
            "m is math italic"
        );
    }

    #[test]
    fn scripts_attach_to_the_atom_before_them_and_ids_are_preorder() {
        let n = parse("w_{k+1}^2").unwrap();
        let s = &n.kids[0];
        assert_eq!(s.k, K::Scripts { sub: true, sup: true });
        assert_eq!(canon(s), "w_{k+1}^{2}");
        // root 0, scripts 1, w 2, sub row 3, k 4, + 5, 1 6, sup row 7, 2 8
        assert_eq!((s.id, s.end), (1, 8));
        assert_eq!(
            n.find(5).unwrap().k,
            K::Glyph {
                cp: 0x2B,
                class: Class::Bin
            }
        );
        assert_eq!(n.path_to(5), vec![0, 1, 3, 5]);
    }

    #[test]
    fn spellings_ignore_spaces_and_optional_braces() {
        assert_eq!(canon(&parse("w_k").unwrap()), canon(&parse("w_{ k }").unwrap()));
        assert_eq!(
            canon(&parse("\\alpha\\nabla f(w_k)").unwrap()),
            "\\alpha \\nabla f(w_{k})"
        );
    }

    #[test]
    fn numbers_frac_sqrt_delims_and_words_parse() {
        let n = parse("\\frac{3.5}{2} + \\sqrt[3]{x} + \\left( a \\right] + \\sin x + \\text{area}").unwrap();
        assert_eq!(n.kids[0].k, K::Frac { display: None });
        assert_eq!(n.kids[0].kids[0].kids[0].k, K::Num("3.5".into()));
        assert_eq!(n.kids[2].k, K::Sqrt);
        assert_eq!(n.kids[2].kids.len(), 2, "body and index");
        assert_eq!(
            n.kids[4].k,
            K::Delim {
                left: 0x28,
                right: 0x5D
            }
        );
        assert!(matches!(&n.kids[6].k, K::Word { text, class: Class::Op, .. } if text == "sin"));
        assert!(matches!(&n.kids[9].k, K::Word { text, .. } if text == "area"));
    }

    #[test]
    fn a_prime_is_a_superscript() {
        let n = parse("f'(x)").unwrap();
        assert_eq!(
            n.kids[0].k,
            K::Scripts {
                sub: false,
                sup: true
            }
        );
        assert_eq!(canon(&n.kids[0]), "f^{\\prime}");
    }

    #[test]
    fn errors_name_the_problem() {
        assert_eq!(
            parse("\\foo x").unwrap_err(),
            "The command \\foo is not supported yet."
        );
        assert_eq!(parse("{x").unwrap_err(), "A { has no matching }.");
        assert_eq!(parse("x}").unwrap_err(), "A } has no matching {.");
        assert_eq!(
            parse("x^2^3").unwrap_err(),
            "A symbol has two superscripts; group them with braces."
        );
        assert_eq!(parse("\\left( x").unwrap_err(), "\\left has no matching \\right.");
        assert_eq!(
            parse("x^").unwrap_err(),
            "A ^, _ or command at the end of the formula is missing its argument."
        );
    }
}
