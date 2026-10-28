//! Rule FB-6: the palettes stay readable and distinguishable. The measurement
//! needs floats, so it lives here, outside the core's source, and reads the
//! core's own palette constants.
//!
//! Deuteranopia is simulated with Machado, Oliveira and Fernandes (2009),
//! severity 1.0, in linear RGB; distance is CIE76 delta E in CIELAB; contrast
//! is the WCAG ratio.

use tool_core::palette::{CANVAS, COLOUR, DEUTAN, INK, PREVIEW};

fn rgb(hex: &str) -> [f64; 3] {
    let h = hex.trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64 / 255.0;
    [v(0), v(2), v(4)]
}

fn lin(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn unlin(c: f64) -> f64 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn deutan(c: [f64; 3]) -> [f64; 3] {
    const M: [[f64; 3]; 3] = [
        [0.367322, 0.860646, -0.227968],
        [0.280085, 0.672501, 0.047413],
        [-0.011820, 0.042940, 0.968881],
    ];
    let l = c.map(lin);
    let mut out = [0.0; 3];
    for (i, row) in M.iter().enumerate() {
        out[i] = unlin(row[0] * l[0] + row[1] * l[1] + row[2] * l[2]);
    }
    out
}

fn lab(c: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = c.map(lin);
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    [116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z))]
}

fn delta_e(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (p, q) = (lab(a), lab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let lum = |c: [f64; 3]| {
        let l = c.map(lin);
        0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn every_concept_text_colour_reads_on_the_canvas() {
    let canvas = rgb(CANVAS);
    for (text, _) in COLOUR {
        let c = contrast(rgb(text), canvas);
        assert!(c >= 4.5, "{text} on the canvas is {c:.2}:1, under 4.5:1");
    }
    for (text, _, _) in DEUTAN {
        let c = contrast(rgb(text), canvas);
        assert!(c >= 4.5, "{text} on the canvas is {c:.2}:1, under 4.5:1");
    }
}

#[test]
fn colour_view_concepts_differ_in_normal_vision() {
    for (i, a) in COLOUR.iter().enumerate() {
        for (j, b) in COLOUR.iter().enumerate().skip(i + 1) {
            let d = delta_e(rgb(a.0), rgb(b.0));
            assert!(
                d >= 30.0,
                "concepts {} and {} are only {d:.1} apart",
                i + 1,
                j + 1
            );
        }
    }
}

#[test]
fn deutan_view_hues_stay_apart_for_a_deuteranope() {
    // concepts 1 and 2 carry their hue as text, 3 and 4 as a tint behind ink
    let d_text = delta_e(deutan(rgb(DEUTAN[0].0)), deutan(rgb(DEUTAN[1].0)));
    assert!(d_text >= 20.0, "deutan text colours are {d_text:.1} apart");
    let d_tint = delta_e(deutan(rgb(DEUTAN[2].1)), deutan(rgb(DEUTAN[3].1)));
    assert!(d_tint >= 20.0, "deutan tints are {d_tint:.1} apart");
    for t in [DEUTAN[2].1, DEUTAN[3].1] {
        let d = delta_e(deutan(rgb(t)), deutan(rgb(CANVAS)));
        assert!(
            d >= 10.0,
            "tint {t} is only {d:.1} from the canvas for a deuteranope"
        );
        assert!(contrast(rgb(INK), rgb(t)) >= 7.0, "ink on tint {t} must read");
    }
}

#[test]
fn the_deutan_preview_is_the_simulated_colour_view() {
    for ((text, tint), (ptext, ptint)) in COLOUR.iter().zip(PREVIEW) {
        for (from, shown) in [(text, ptext), (tint, ptint)] {
            let want = deutan(rgb(from)).map(|c| (c * 255.0).round());
            let have = rgb(shown).map(|c| (c * 255.0).round());
            for k in 0..3 {
                assert!(
                    (want[k] - have[k]).abs() <= 1.0,
                    "{from} simulates to {want:?}, the preview holds {shown}"
                );
            }
        }
    }
}

#[test]
fn the_colour_view_alone_would_fail_a_deuteranope_which_is_why_patterns_exist() {
    // the measurement behind journal R1.3: harbor and plum collapse
    let d = delta_e(deutan(rgb(COLOUR[0].0)), deutan(rgb(COLOUR[2].0)));
    assert!(d < 10.0, "harbor and plum are {d:.1} apart under deuteranopia");
}
