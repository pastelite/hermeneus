use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::restub::{restub_links, restub_references, restub_templates, restub_title};
use super::stub::{stub_links, stub_ref, stub_template, stub_title};

/// A page whose wikitext has been converted to Markdown-like text with
/// placeholder tokens for templates and references.
#[derive(Debug, Serialize, Deserialize)]
pub struct StubbedPage {
    /// The Markdown-like page text containing template and reference tokens.
    pub stubbed: String,
    /// Saved templates keyed by their `name#n` token.
    pub template: HashMap<String, String>,
    /// Saved references keyed by their footnote label.
    pub references: HashMap<String, String>,
}

impl StubbedPage {
    /// Converts `wikitext` to Markdown and collects the templates and
    /// references that were replaced with tokens.
    pub fn from_wikitext(wikitext: &str) -> Self {
        let (template_wikitext, template) = stub_template(wikitext);
        let (reference_wikitext, references) = stub_ref(&template_wikitext);
        let link_wikitext = stub_links(&reference_wikitext);
        let stubbed = stub_title(&link_wikitext);

        Self {
            stubbed,
            template,
            references,
        }
    }

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

impl From<&str> for StubbedPage {
    fn from(wikitext: &str) -> Self {
        Self::from_wikitext(wikitext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn from_wikitext_returns_markdown_and_tokens() {
        let wikitext = "{{Infobox|name=Example}}\n== Title ==\n[[Target|Label]] <ref name=\"source\">Citation</ref>";
        let page = StubbedPage::from(wikitext);

        assert!(page.stubbed.contains("`[template:Infobox#1]`"));
        assert!(page.stubbed.contains("## Title"));
        assert!(page.stubbed.contains("[Label](Target)"));
        assert!(page.stubbed.contains("[^source]"));
        assert_eq!(
            page.template.get("Infobox#1").map(String::as_str),
            Some("Infobox|name=Example")
        );
        assert_eq!(
            page.references.get("source").map(String::as_str),
            Some("Citation")
        );
    }

    #[test]
    fn to_json_serializes_the_page_and_metadata() {
        let page = StubbedPage {
            stubbed: "[ABC](ABCTranslated) [^source] `[template:Infobox#1]`".to_string(),
            template: HashMap::from([(
                "Infobox#1".to_string(),
                "Infobox|name=Example".to_string(),
            )]),
            references: HashMap::from([("source".to_string(), "Citation".to_string())]),
        };

        let serialized = page.to_json().unwrap();
        let value: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(value.as_object().unwrap().len(), 3);
        assert_eq!(value["template"]["Infobox#1"], "Infobox|name=Example");
        assert_eq!(value["references"]["source"], "Citation");
    }

    #[test]
    fn from_json_restores_the_page_and_metadata() {
        let page = StubbedPage {
            stubbed: "[ABC](ABCTranslated) [^source] `[template:Infobox#1]`".to_string(),
            template: HashMap::from([(
                "Infobox#1".to_string(),
                "Infobox|name=Example".to_string(),
            )]),
            references: HashMap::from([("source".to_string(), "Citation".to_string())]),
        };

        let restored = StubbedPage::from_json(&page.to_json().unwrap()).unwrap();

        assert_eq!(restored.stubbed, page.stubbed);
        assert_eq!(restored.template, page.template);
        assert_eq!(restored.references, page.references);
        assert_eq!(restored.restub(), page.restub());
    }

    #[test]
    fn restub_restores_links_references_and_templates() {
        let page = StubbedPage {
            stubbed: "[ABC](ABCTranslated) [^source] [^2] `[template:Infobox#1]`".to_string(),
            template: HashMap::from([(
                "Infobox#1".to_string(),
                "Infobox|name=Example".to_string(),
            )]),
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
    fn restub_restores_a_parenthesized_thai_markdown_target() {
        let page = StubbedPage {
            stubbed: "[โดเมน](<โดเมน (ชีววิทยา)>)".to_string(),
            template: HashMap::new(),
            references: HashMap::new(),
        };

        assert_eq!(page.restub(), "[[โดเมน (ชีววิทยา)|โดเมน]]");
    }

    #[test]
    fn restub_restores_repeated_named_references_with_one_definition() {
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
}
