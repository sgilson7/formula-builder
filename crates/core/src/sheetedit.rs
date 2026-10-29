//! Edits to the sheet and the prose that the student modes make for the
//! student: a card sentence becomes one sheet line, a block becomes one step
//! line, and a sentence typed without link syntax keeps its links. The sheet
//! and prose stay the saved form; these functions only rewrite their text.

use crate::sheet::parse_prose;

/// The lines of the sheet, and the end (exclusive) of the indented block
/// that follows the header on line `header` (1-based).
fn block(sheet: &str, header: usize) -> (Vec<String>, usize) {
    let lines: Vec<String> = sheet.lines().map(String::from).collect();
    let mut end = header.min(lines.len());
    while end < lines.len() && (lines[end].starts_with(' ') || lines[end].starts_with('\t')) {
        end += 1;
    }
    (lines, end)
}

fn join(lines: &[String]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Sets the line starting with `keyword` in the concept whose header is on
/// line `header`: replaces it, adds it at the end of the block, or with
/// `None` removes it. `also` names keywords the new line replaces (a slider
/// replaces a scrub).
pub fn set_line(sheet: &str, header: usize, keyword: &str, line: Option<&str>, also: &[&str]) -> String {
    let (mut lines, end) = block(sheet, header);
    let mut at = None;
    let mut i = header;
    let mut end = end;
    while i < end {
        let first = lines[i].split_whitespace().next().unwrap_or("");
        if first == keyword || also.contains(&first) {
            lines.remove(i);
            end -= 1;
            at.get_or_insert(i);
        } else {
            i += 1;
        }
    }
    if let Some(l) = line {
        lines.insert(at.unwrap_or(end), format!("  {l}"));
    }
    join(&lines)
}

/// The word in a sheet line after its keyword, if the concept has the line.
pub fn get_line(sheet: &str, header: usize, keyword: &str) -> Option<String> {
    let (lines, end) = block(sheet, header);
    lines[header.min(end)..end].iter().find_map(|l| {
        let mut w = l.split_whitespace();
        (w.next() == Some(keyword)).then(|| w.collect::<Vec<_>>().join(" "))
    })
}

/// The prose as plain words, with the link syntax taken out.
pub fn plain(prose: &str) -> String {
    parse_prose(prose).into_iter().map(|s| s.text).collect()
}

/// (words, concept) for every link in the prose.
pub fn links(prose: &str) -> Vec<(String, String)> {
    parse_prose(prose)
        .into_iter()
        .filter_map(|s| s.concept.map(|c| (s.text, c)))
        .collect()
}

/// Plain words with the links put back: the first place each linked phrase
/// still appears is linked again, and a phrase no longer in the text is dropped.
pub fn relink(text: &str, keep: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (words, concept) in keep {
        if words.trim().is_empty() || words.contains(['[', ']', '(', ')']) {
            continue;
        }
        // look only in plain stretches, outside links already made
        let mut search = 0;
        while let Some(found) = out[search..].find(words.as_str()).map(|i| i + search) {
            let inside = out[..found].matches('[').count() > out[..found].matches(']').count();
            if !inside {
                out.replace_range(found..found + words.len(), &format!("[{words}]({concept})"));
                break;
            }
            search = found + words.len();
        }
    }
    out
}

/// The prose with `words` linked to `concept`, and any other link to that
/// concept taken out. Empty words just remove the link.
pub fn link(prose: &str, words: &str, concept: &str) -> String {
    let mut keep: Vec<(String, String)> = links(prose).into_iter().filter(|(_, c)| c != concept).collect();
    let words = words.trim();
    if !words.is_empty() {
        keep.push((words.to_string(), concept.to_string()));
    }
    relink(&plain(prose), &keep)
}

/// One step block of the sheet: its caption and its indented lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepText {
    pub caption: String,
    pub lines: Vec<String>,
}

/// The step blocks, in order.
pub fn steps(sheet: &str) -> Vec<StepText> {
    let mut out: Vec<StepText> = Vec::new();
    let mut inside = false;
    for l in sheet.lines() {
        let indented = l.starts_with(' ') || l.starts_with('\t');
        if !indented {
            inside = l.split_whitespace().next() == Some("step");
            if inside {
                out.push(StepText {
                    caption: l.trim_start()["step".len()..].trim().to_string(),
                    lines: vec![],
                });
            }
        } else if inside && !l.trim().is_empty() {
            if let Some(s) = out.last_mut() {
                s.lines.push(l.trim().to_string());
            }
        }
    }
    out
}

/// The sheet with its step blocks replaced by `new`, written at the end.
pub fn write_steps(sheet: &str, new: &[StepText]) -> String {
    let mut kept: Vec<String> = Vec::new();
    let mut inside = false;
    for l in sheet.lines() {
        let indented = l.starts_with(' ') || l.starts_with('\t');
        if !indented {
            inside = l.split_whitespace().next() == Some("step");
        }
        if !inside {
            kept.push(l.to_string());
        }
    }
    while kept.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        kept.pop();
    }
    for s in new {
        kept.push(format!("step {}", s.caption.trim()));
        kept.extend(s.lines.iter().map(|l| format!("  {l}")));
    }
    if kept.is_empty() {
        return String::new();
    }
    join(&kept)
}

/// A one-word sheet name made from a student's word: `step size` becomes
/// `stepsize`. Letters only, because the LaTeX export makes a macro of it;
/// a name already taken gets a letter added (`heightb`).
pub fn slug(word: &str, taken: &[String]) -> String {
    let base: String = word
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let base = if base.is_empty() { "part".to_string() } else { base };
    // b, c, ... z, then ab, ac, ...: letters, so the macro stays distinct
    let letters = || ('b'..='z').map(String::from);
    let tries = std::iter::once(String::new())
        .chain(letters())
        .chain(('a'..='z').flat_map(move |a| letters().map(move |b| format!("{a}{b}"))));
    tries
        .map(|t| format!("{base}{t}"))
        .find(|n| !taken.contains(n))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "concept height: h\n  name height\n  slider 1 .. 10 step 1 = 4\nconcept base: b\nstep Start\n  formula 1\nstep Next\n  formula 2\n  focus height\n";

    #[test]
    fn a_card_sentence_replaces_adds_or_removes_one_line() {
        let s = set_line(SHEET, 1, "name", Some("name tall side"), &[]);
        assert!(s.starts_with("concept height: h\n  name tall side\n  slider"));
        let s = set_line(SHEET, 1, "scrub", Some("scrub 0 .. 5 step 1 = 1"), &["slider"]);
        assert!(
            s.contains("concept height: h\n  name height\n  scrub 0 .. 5 step 1 = 1\nconcept base"),
            "{s}"
        );
        let s = set_line(SHEET, 4, "unit", Some("unit cm"), &[]);
        assert!(s.contains("concept base: b\n  unit cm\nstep Start"), "{s}");
        let s = set_line(SHEET, 1, "slider", None, &[]);
        assert!(!s.contains("slider"));
        assert_eq!(get_line(SHEET, 1, "name").as_deref(), Some("height"));
        assert_eq!(get_line(SHEET, 4, "name"), None);
    }

    #[test]
    fn a_sentence_typed_plain_keeps_its_links() {
        let prose = "The [height](height) is how tall it is.";
        assert_eq!(plain(prose), "The height is how tall it is.");
        let back = relink("Its height, times the base.", &links(prose));
        assert_eq!(back, "Its [height](height), times the base.");
        assert_eq!(relink("No such word.", &links(prose)), "No such word.");
        assert_eq!(
            link(prose, "base", "base"),
            "The [height](height) is how tall it is."
        );
        assert_eq!(link("Times the base.", "base", "base"), "Times the [base](base).");
        assert_eq!(link(prose, "", "height"), "The height is how tall it is.");
    }

    #[test]
    fn steps_are_read_and_written_back_after_the_rest_of_the_sheet() {
        let st = steps(SHEET);
        assert_eq!(st.len(), 2);
        assert_eq!(st[1].caption, "Next");
        assert_eq!(st[1].lines, ["formula 2", "focus height"]);
        let mut swapped = st.clone();
        swapped.swap(0, 1);
        let s = write_steps(SHEET, &swapped);
        assert!(
            s.ends_with("concept base: b\nstep Next\n  formula 2\n  focus height\nstep Start\n  formula 1\n"),
            "{s}"
        );
        assert_eq!(write_steps("concept a: a\n", &[]), "concept a: a\n");
    }

    #[test]
    fn a_student_word_becomes_a_free_one_word_name() {
        assert_eq!(slug("step size", &[]), "stepsize");
        assert_eq!(slug("height", &["height".into()]), "heightb");
        assert_eq!(slug("height", &["height".into(), "heightb".into()]), "heightc");
        assert_eq!(slug("side 2", &[]), "side");
        assert_eq!(slug("+++", &[]), "part");
    }
}
