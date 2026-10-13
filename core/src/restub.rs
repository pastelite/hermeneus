use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::OnceLock,
};

use crate::fetcher::LangLink;

#[derive(Debug)]
pub struct StubbedPage {
    pub title: String,
    pub stubbed: String,
    pub lang_links: Vec<LangLink>,
    pub template: HashMap<String, String>,
    pub references: HashMap<String, String>,
}

impl StubbedPage {
    /// Restores the stubbed Markdown-like text to MediaWiki wikitext.
    pub fn restub(&self) -> String {
        let wikitext = restub_links(&self.stubbed, &self.lang_links);
        let wikitext = restub_references(&wikitext, &self.references);
        restub_templates(&wikitext, &self.template)
    }
}

fn restub_links(wikitext: &str, lang_links: &[LangLink]) -> String {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| {
        Regex::new(r"\[(?P<label>[^\]]*)\]\((?P<target>[^)]*)\)")
            .expect("Markdown link regex is valid")
    });

    let translated_titles: HashMap<&str, &str> = lang_links
        .iter()
        .map(|link| (link.title.as_str(), link.english_title.as_str()))
        .collect();

    let mut replacements = Vec::new();
    for captures in link.captures_iter(wikitext) {
        let whole_link = captures.get(0).expect("whole link match exists");
        let label = captures
            .name("label")
            .expect("link label match exists")
            .as_str();
        let target = captures
            .name("target")
            .expect("link target match exists")
            .as_str();
        let source_title = translated_titles.get(target).copied().unwrap_or(target);

        let restored = if label == source_title {
            format!("[[{source_title}]]")
        } else {
            format!("[[{source_title}|{label}]]")
        };
        replacements.push((whole_link.start()..whole_link.end(), restored));
    }

    apply_replacements(wikitext.to_string(), replacements)
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
    use super::StubbedPage;
    use crate::fetcher::LangLink;
    use std::collections::HashMap;

    #[test]
    fn restubs_links_references_and_templates() {
        let page = StubbedPage {
            title: "Example".to_string(),
            stubbed: "[ABC](ABCTranslated) [^source] [^2] `[template:Infobox]`".to_string(),
            lang_links: vec![LangLink {
                english_title: "ABC".to_string(),
                title: "ABCTranslated".to_string(),
                lang: "th".to_string(),
            }],
            template: HashMap::from([("Infobox".to_string(), "Infobox|name=Example".to_string())]),
            references: HashMap::from([
                ("source".to_string(), "Citation".to_string()),
                ("2".to_string(), "Unnamed citation".to_string()),
            ]),
        };

        assert_eq!(
            page.restub(),
            "[[ABC]] <ref name=\"source\">Citation</ref> <ref>Unnamed citation</ref> {{Infobox|name=Example}}"
        );
    }

    #[test]
    fn restubs_repeated_named_references_with_one_definition() {
        let page = StubbedPage {
            title: "Example".to_string(),
            stubbed: "[^source] and [^source]".to_string(),
            lang_links: Vec::new(),
            template: HashMap::new(),
            references: HashMap::from([("source".to_string(), "Citation".to_string())]),
        };

        assert_eq!(
            page.restub(),
            "<ref name=\"source\">Citation</ref> and <ref name=\"source\" />"
        );
    }
}
