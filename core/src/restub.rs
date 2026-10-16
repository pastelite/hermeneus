use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::OnceLock,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct StubbedPage {
    pub stubbed: String,
    pub template: HashMap<String, String>,
    pub references: HashMap<String, String>,
}

impl StubbedPage {
    /// Serializes this page and all stubbing metadata as JSON.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|error| error.to_string())
    }

    /// Restores a page and its stubbing metadata from JSON.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|error| error.to_string())
    }

    /// Restores the stubbed Markdown-like text to MediaWiki wikitext.
    pub fn restub(&self) -> String {
        let wikitext = restub_links(&self.stubbed);
        let wikitext = restub_references(&wikitext, &self.references);
        let wikitext = restub_templates(&wikitext, &self.template);
        restub_title(&wikitext)
    }
}

pub fn restub_title(text: &str) -> String {
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

fn restub_links(wikitext: &str) -> String {
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

fn restub_references(wikitext: &str, references: &HashMap<String, String>) -> String {
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

fn restub_templates(wikitext: &str, templates: &HashMap<String, String>) -> String {
    static TEMPLATE: OnceLock<Regex> = OnceLock::new();
    let template = TEMPLATE.get_or_init(|| {
        Regex::new(r"`\[template:(?P<key>[^\]]+)\]`").expect("template placeholder regex is valid")
    });

    let replacements = template
        .captures_iter(wikitext)
        .filter_map(|captures| {
            let whole_template = captures.get(0)?;
            let key = captures.name("key")?.as_str();
            let value = templates.get(key)?;

            Some((
                whole_template.start()..whole_template.end(),
                format!("{{{{{value}}}}}"),
            ))
        })
        .collect();

    apply_replacements(wikitext.to_string(), replacements)
}

fn apply_replacements(mut text: String, replacements: Vec<(Range<usize>, String)>) -> String {
    for (range, replacement) in replacements.into_iter().rev() {
        text.replace_range(range, &replacement);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{restub_title, StubbedPage};
    use std::collections::HashMap;

    #[test]
    fn restub_title_converts_markdown_headings_to_wikitext() {
        assert_eq!(
            restub_title("Before\n##test\n### abc\n#### def\n##### ghi\n###### jkl\nAfter"),
            "Before\n== test ==\n=== abc ===\n==== def ====\n===== ghi =====\n====== jkl ======\nAfter"
        );
    }

    #[test]
    fn restubs_links_references_and_templates() {
        let page = StubbedPage {
            stubbed: "[ABC](ABCTranslated) [^source] [^2] `[template:Infobox]`".to_string(),
            template: HashMap::from([("Infobox".to_string(), "Infobox|name=Example".to_string())]),
            references: HashMap::from([
                ("source".to_string(), "Citation".to_string()),
                ("2".to_string(), "Unnamed citation".to_string()),
            ]),
        };

        assert_eq!(
            page.restub(),
            "[[ABCTranslated|ABC]] <ref name=\"source\">Citation</ref> <ref>Unnamed citation</ref> {{Infobox|name=Example}}"
        );
    }

    #[test]
    fn restubs_parenthesized_thai_markdown_target() {
        let page = StubbedPage {
            stubbed: "[โดเมน](<โดเมน (ชีววิทยา)>)".to_string(),
            template: HashMap::new(),
            references: HashMap::new(),
        };

        assert_eq!(page.restub(), "[[โดเมน (ชีววิทยา)|โดเมน]]");
    }

    #[test]
    fn restubs_repeated_named_references_with_one_definition() {
        let page = StubbedPage {
            stubbed: "[^source] and [^source]".to_string(),
            template: HashMap::new(),
            references: HashMap::from([("source".to_string(), "Citation".to_string())]),
        };

        assert_eq!(
            page.restub(),
            "<ref name=\"source\">Citation</ref> and <ref name=\"source\" />"
        );
    }

    #[test]
    fn stubbed_page_json_round_trip_preserves_restub_data() {
        let page = StubbedPage {
            stubbed: "[ABC](ABCTranslated) [^source] `[template:Infobox]`".to_string(),
            template: HashMap::from([("Infobox".to_string(), "Infobox|name=Example".to_string())]),
            references: HashMap::from([("source".to_string(), "Citation".to_string())]),
        };

        let serialized = page.to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 3);
        assert!(value.get("lang_links").is_none());
        assert!(value.get("title").is_none());

        let restored = StubbedPage::from_json(&serialized).unwrap();

        assert_eq!(restored.restub(), page.restub());
    }
}
