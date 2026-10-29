//! Writes sample reader views as SVG for a person (or agent) to look at:
//! `cargo run -p tool_core --example preview -- OUT_DIR`. Not part of the site.
use tool_core::doc::{Doc, Model};
use tool_core::palette::View;
use tool_core::render::{plot_svg, scene, svg, Look};

fn main() {
    let out = std::env::args().nth(1).expect("an output directory");
    let docs = [
        ("line", "y = m x + b", "concept slope: m\n  say how much y changes for one step in x\n  label above\n  scrub -3 .. 3 step 0.5 = 1.5\nconcept intercept: b\n  brace below\n  slider -5 .. 5 step 1 = 1\nplot y over x from -5 to 5\n  y from -6 to 6\n  mark slope rise x = 1\n  mark intercept intercept\n"),
        ("gd", "w_{k+1} = w_k - \\alpha \\nabla f(w_k)\nf(w) = \\frac{(w - 3)^2}{5} + \\frac{1}{2}", "concept next: w_{k+1}\n  label below next weights\nconcept current: w_k\n  slider 0 .. 6 step 0.1 = 1.2\nconcept step: \\alpha\n  say how far one update moves\n  scrub 0.05 .. 2 step 0.05 = 0.8\n  tint\nconcept gradient: \\nabla f(w_k)\n  brace below gradient\nplot f over w from 0 to 6\n  mark current point w = w_k\n  mark next point w = w_{k+1}\n"),
        ("mix", "\\mathrm{Re}(z) = \\frac{z + \\bar z}{2} = \\sum_{i=1}^{n} x_i^2 + \\sqrt{a^2+b^2} + \\int_0^1 \\sin x \\, dx + \\left( \\frac{a}{b} \\right)", "concept sum: \\sum_{i=1}^{n} x_i^2\n  border\nconcept root: \\sqrt{a^2+b^2}\n  label above hypotenuse\n  pointer\n"),
    ];
    for (name, f, s) in docs {
        let d = Doc {
            title: name.into(),
            formulas: f.into(),
            sheet: s.into(),
            prose: String::new(),
            ..Doc::default()
        };
        let m = Model::build(&d);
        eprintln!("{name}: errors {:?}", m.errors);
        let values = m.values(&[], None);
        for view in View::ALL {
            let look = Look {
                view,
                hover: if name == "gd" { Some(2) } else { None },
                select: None,
                step: None,
                interactive: false,
                export_px: Some(900),
                ghost: vec![],
                muted: vec![],
                transparent: false,
            };
            let sc = scene(&m, &values, &look, &m.shown(None));
            std::fs::write(format!("{out}/{name}-{}.svg", view.name()), svg(&m, &sc, &look)).unwrap();
            if let Some(Ok(p)) = plot_svg(&m, &values, &look) {
                std::fs::write(format!("{out}/{name}-plot-{}.svg", view.name()), p).unwrap();
            } else if let Some(Err(e)) = plot_svg(&m, &values, &look) {
                eprintln!("plot error {e}");
            }
        }
    }
}
