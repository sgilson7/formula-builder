//! Colours for each view. Pictures and pages name these; nothing else holds a
//! colour. `tests/colour.rs` measures them under a deuteranopia simulation and
//! in grey (rule FB-6), outside this crate because the measurement needs
//! floats.

use crate::sheet::Concept;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Colour,
    Deutan,
    Mono,
    /// the Colour view as a deuteranope sees it, for authors to check
    Preview,
}

impl View {
    pub const ALL: [View; 4] = [View::Colour, View::Deutan, View::Mono, View::Preview];

    pub fn name(self) -> &'static str {
        match self {
            View::Colour => "colour",
            View::Deutan => "deutan",
            View::Mono => "mono",
            View::Preview => "preview",
        }
    }

    pub fn from(name: &str) -> Option<View> {
        View::ALL.iter().copied().find(|v| v.name() == name)
    }
}

pub const INK: &str = "#3B2340";
pub const CANVAS: &str = "#FFF6E0";
/// Ink at three-tenths over the canvas, for parts outside the focus.
pub const DIM: &str = "#C4B7B0";
pub const MUTED: &str = "#8D7F88";
pub const GREY_TINT: &str = "#E6DDE2";
pub const SELECT: &str = "#4FA3A0";

/// Colour view: text colour and tint for concepts 1 to 4. Each text colour
/// has at least 4.5:1 contrast on the canvas.
pub const COLOUR: [(&str, &str); 4] = [
    ("#1D6FA3", "#BFDDF0"),
    ("#8A5A00", "#F2D28A"),
    ("#7A3E9D", "#DCC6EC"),
    ("#B03A2E", "#F2B8A8"),
];

/// The Colour view's text colours and tints after a deuteranopia simulation
/// (Machado et al. 2009, severity 1). `tests/colour.rs` recomputes them.
pub const PREVIEW: [(&str, &str); 4] = [
    ("#4464A2", "#CED6F0"),
    ("#766905", "#EAD98C"),
    ("#355A9B", "#C5CEEB"),
    ("#796C2A", "#D3CAA8"),
];

/// Deutan view: blue and orange, as text (concepts 1, 2) or as a tint behind
/// ink (concepts 3, 4), the one hue axis a deuteranope keeps.
pub const DEUTAN: [(&str, &str, bool); 4] = [
    ("#1D6FA3", "#BFDDF0", false),
    ("#A85A00", "#F5D3A6", false),
    (INK, "#BFDDF0", true),
    (INK, "#F5D3A6", true),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paint {
    pub text: &'static str,
    pub tint: &'static str,
    /// the tint is drawn whether or not the sheet asks for one
    pub always_tint: bool,
}

pub fn paint(view: View, c: &Concept) -> Paint {
    let i = c.num - 1;
    if !c.colour || i >= COLOUR.len() {
        return Paint {
            text: INK,
            tint: GREY_TINT,
            always_tint: false,
        };
    }
    match view {
        View::Colour => Paint {
            text: COLOUR[i].0,
            tint: COLOUR[i].1,
            always_tint: false,
        },
        View::Deutan => Paint {
            text: DEUTAN[i].0,
            tint: DEUTAN[i].1,
            always_tint: DEUTAN[i].2,
        },
        View::Mono => Paint {
            text: INK,
            tint: GREY_TINT,
            always_tint: false,
        },
        View::Preview => Paint {
            text: PREVIEW[i].0,
            tint: PREVIEW[i].1,
            always_tint: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::parse_sheet;

    #[test]
    fn each_view_paints_the_first_four_concepts_and_no_more() {
        let s = parse_sheet("concept a: a\nconcept b: b\nconcept c: c\nconcept d: d\nconcept e: e\n");
        let c = &s.concepts;
        assert_eq!(paint(View::Colour, &c[0]).text, "#1D6FA3");
        assert_eq!(paint(View::Colour, &c[4]).text, INK, "the fifth concept is ink");
        assert_eq!(paint(View::Mono, &c[1]).text, INK);
        let d3 = paint(View::Deutan, &c[2]);
        assert_eq!(
            (d3.text, d3.always_tint),
            (INK, true),
            "concept 3 carries its hue as a tint"
        );
        assert_eq!(View::from("deutan"), Some(View::Deutan));
        assert_eq!(View::from("sepia"), None);
    }
}
