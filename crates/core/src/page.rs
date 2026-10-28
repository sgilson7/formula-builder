//! The page's words. Every string a reader sees lives in `data/strings.json`;
//! the core checks that the file has every key the screen needs before the
//! page draws anything.

use std::collections::BTreeMap;

pub type Strings = BTreeMap<String, String>;

/// Keys the screen cannot be drawn without.
pub const REQUIRED: &[&str] = &[
    "title",
    "intro",
    "formulas",
    "sheet",
    "prose",
    "concepts",
    "budget",
    "add_concept",
    "no_selection",
    "steps",
    "play",
    "pause",
    "view_colour",
    "view_deutan",
    "view_mono",
    "feature_label",
    "feature_brace",
    "feature_border",
    "feature_tint",
    "feature_pointer",
    "feature_scrub",
    "feature_slider",
    "feature_space",
    "view_preview",
    "show",
    "hide",
    "export_clear",
    "remove",
    "limit_from",
    "export_header",
    "embed_title",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringsError {
    NotJson(String),
    Empty(String),
}

impl StringsError {
    /// The message the page shows. Wording that depends on the rule stays
    /// beside the rule.
    pub fn message(&self) -> String {
        match self {
            StringsError::NotJson(detail) => format!("The strings file could not be read: {detail}"),
            StringsError::Empty(field) => format!("The strings file has an empty \"{field}\"."),
        }
    }
}

/// Parses and checks the strings file. A field that is only whitespace counts
/// as empty, because the page would draw nothing.
pub fn parse_strings(json: &str) -> Result<Strings, StringsError> {
    let strings: Strings = serde_json::from_str(json).map_err(|e| StringsError::NotJson(e.to_string()))?;
    for key in REQUIRED {
        if strings.get(*key).map(|v| v.trim().is_empty()).unwrap_or(true) {
            return Err(StringsError::Empty(key.to_string()));
        }
    }
    Ok(strings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> String {
        let pairs: Vec<String> = REQUIRED.iter().map(|k| format!("\"{k}\": \"x\"")).collect();
        format!("{{{}}}", pairs.join(","))
    }

    #[test]
    fn a_complete_file_is_accepted() {
        let s = parse_strings(&full()).unwrap();
        assert_eq!(s["title"], "x");
    }

    #[test]
    fn an_empty_or_missing_key_is_rejected_by_name() {
        let blank = full().replace("\"title\": \"x\"", "\"title\": \"  \"");
        assert_eq!(
            parse_strings(&blank).unwrap_err(),
            StringsError::Empty("title".into())
        );
        assert_eq!(
            parse_strings(&blank).unwrap_err().message(),
            "The strings file has an empty \"title\"."
        );
        let missing = full().replace("\"intro\": \"x\",", "");
        assert_eq!(
            parse_strings(&missing).unwrap_err(),
            StringsError::Empty("intro".into())
        );
    }

    #[test]
    fn text_that_is_not_json_is_rejected() {
        assert!(matches!(parse_strings("title: T"), Err(StringsError::NotJson(_))));
    }
}
