use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::OnceLock,
};

/// Converts Markdown headings in `text` back to MediaWiki section headings.
///
/// Two or more leading `#` markers become the same number of `=` markers on
/// both sides, so `## Heading` becomes `== Heading ==`.
pub(super) fn restub_title(text: &str) -> String {
    static TITLE: OnceLock<Regex> = OnceLock::new();
    let title = TITLE.get_or_init(|| {
        Regex::new(r"(?m)^(?P<marks>#{2,})[ \t]*(?P<title>.*?)$").expect("title regex is valid")
    });

    title
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let marks = captures
                .name("marks")
                .expect("heading markers exist")
                .as_str();
            let heading = captures
                .name("title")
                .expect("heading text exists")
                .as_str();
            let equals = "=".repeat(marks.len());
            format!("{equals} {} {equals}", heading.trim())
        })
        .into_owned()
}

/// Restores Markdown links in `text` to `[[target]]` or `[[target|label]]`.
///
/// Angle-bracketed destinations are unescaped first, and the pipe label is
/// omitted when it matches the target.
pub(super) fn restub_links(wikitext: &str) -> String {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| {
        Regex::new(r"\[(?P<label>[^\]]*)\]\((?:<(?P<angled>(?:\\.|[^>])*)>|(?P<plain>[^)]*))\)")
            .expect("Markdown link regex is valid")
    });

    let mut replacements = Vec::new();
    for captures in link.captures_iter(wikitext) {
        let whole_link = captures.get(0).expect("whole link match exists");
        let label = captures
            .name("label")
            .expect("link label match exists")
            .as_str();
        let target = captures
            .name("angled")
            .map(|target| unescape_markdown_destination(target.as_str()))
            .or_else(|| {
                captures
                    .name("plain")
                    .map(|target| target.as_str().to_string())
            })
            .expect("link target match exists");
        let restored = if label == target {
            format!("[[{target}]]")
        } else {
            format!("[[{target}|{label}]]")
        };
        replacements.push((whole_link.start()..whole_link.end(), restored));
    }

    apply_replacements(wikitext.to_string(), replacements)
}

/// Removes Markdown backslash escapes before ASCII punctuation in a target.
fn unescape_markdown_destination(destination: &str) -> String {
    let mut characters = destination.chars().peekable();
    let mut output = String::new();

    while let Some(character) = characters.next() {
        if character == '\\'
            && characters
                .peek()
                .is_some_and(|next| next.is_ascii_punctuation())
        {
            output.push(characters.next().expect("peeked character exists"));
        } else {
            output.push(character);
        }
    }

    output
}

/// Replaces `[^key]` markers in `text` with MediaWiki `<ref>` tags.
///
/// Digit-only keys become unnamed refs. Named refs use one full definition and
/// self-closing tags for later uses, and an empty saved body produces a
/// self-closing ref. Markers without a saved value are left untouched.
pub(super) fn restub_references(wikitext: &str, references: &HashMap<String, String>) -> String {
    static REFERENCE: OnceLock<Regex> = OnceLock::new();
    let reference = REFERENCE
        .get_or_init(|| Regex::new(r"\[\^(?P<key>[^\]]+)\]").expect("reference regex is valid"));

    let mut seen_named_references = HashSet::new();
    let mut replacements = Vec::new();

    for captures in reference.captures_iter(wikitext) {
        let whole_reference = captures.get(0).expect("whole reference match exists");
        let key = captures
            .name("key")
            .expect("reference key match exists")
            .as_str();
        let Some(value) = references.get(key) else {
            continue;
        };

        let restored = if key.chars().all(|character| character.is_ascii_digit()) {
            format!("<ref>{value}</ref>")
        } else if seen_named_references.insert(key) {
            if value.is_empty() {
                format!("<ref name=\"{key}\" />")
            } else {
                format!("<ref name=\"{key}\">{value}</ref>")
            }
        } else {
            format!("<ref name=\"{key}\" />")
        };

        replacements.push((whole_reference.start()..whole_reference.end(), restored));
    }

    apply_replacements(wikitext.to_string(), replacements)
}

/// Applies `replacements` from the end of `text` so earlier byte ranges remain
/// valid as the string is rewritten.
fn apply_replacements(mut text: String, replacements: Vec<(Range<usize>, String)>) -> String {
    for (range, replacement) in replacements.into_iter().rev() {
        text.replace_range(range, &replacement);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn restub_title_converts_markdown_headings_to_wikitext() {
        assert_eq!(
            restub_title("Before\n##test\n### abc\n#### def\n##### ghi\n###### jkl\nAfter"),
            "Before\n== test ==\n=== abc ===\n==== def ====\n===== ghi =====\n====== jkl ======\nAfter"
        );
    }

    #[test]
    fn restub_links_restores_markdown_links() {
        assert_eq!(
            restub_links("[ABC](ABCTranslated) [Plain](Plain) [โดเมน](<โดเมน (ชีววิทยา)>)"),
            "[[ABCTranslated|ABC]] [[Plain]] [[โดเมน (ชีววิทยา)|โดเมน]]"
        );
    }

    #[test]
    fn unescape_markdown_destination_removes_backslash_escapes() {
        assert_eq!(unescape_markdown_destination(r"a\\b\<c\>"), r"a\b<c>");
    }

    #[test]
    fn restub_references_restores_numbered_and_named_refs() {
        let references = HashMap::from([
            ("source".to_string(), "Citation".to_string()),
            ("2".to_string(), "Unnamed citation".to_string()),
            ("empty".to_string(), String::new()),
        ]);

        assert_eq!(
            restub_references("[^source] [^source] [^2] [^empty] [^missing]", &references),
            "<ref name=\"source\">Citation</ref> <ref name=\"source\" /> <ref>Unnamed citation</ref> <ref name=\"empty\" /> [^missing]"
        );
    }

    #[test]
    fn apply_replacements_replaces_all_ranges() {
        let replacements = vec![(0..1, "A".to_string()), (2..3, "C".to_string())];

        assert_eq!(apply_replacements("abc".to_string(), replacements), "AbC");
    }
}
