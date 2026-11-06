use std::collections::HashMap;

use mwapi::Client;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::wiki::client::wikipedia_client;
use crate::wiki::fetcher::{LangLink, Link, RawFetcher};

/// A row in the glossary link table used when translating a page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkTableRow {
    /// The link's display text in the source page.
    pub title: String,
    /// The source page title the link points to.
    pub link_to: String,
    /// The corresponding Thai page title, when one exists.
    pub translated: Option<String>,
}

/// Fetches a page and builds a table of its links and Thai-language targets.
pub struct LinkTableFetcher {
    raw_fetcher: RawFetcher,
}

impl LinkTableFetcher {
    /// Creates a helper for pages on the specified source Wikipedia edition.
    pub async fn new(source_language: &str) -> Result<Self> {
        let client = wikipedia_client(source_language).await?;
        Ok(Self::with_client(client))
    }

    /// Creates a helper using an already-built MediaWiki API client.
    pub fn with_client(client: Client) -> Self {
        Self {
            raw_fetcher: RawFetcher::new(client),
        }
    }

    /// Fetches `page`'s article links and returns their Thai langlinks, if present.
    pub async fn get_langlink_table(&self, page: &str) -> Result<Vec<LinkTableRow>> {
        let links = self.raw_fetcher.fetch_links(&[page]).await?;
        let article_titles = links
            .iter()
            .filter(|link| link.namespace == 0)
            .map(|link| link.title.as_str())
            .collect::<Vec<_>>();
        let langlinks = self.raw_fetcher.fetch_langlinks(&article_titles).await?;

        Ok(build_langlink_table(&links, &langlinks))
    }
}

/// Formats `links` as the Markdown glossary table inserted into a translation
/// prompt.
pub fn glossary_markdown(links: &[LinkTableRow]) -> String {
    let mut table =
        String::from("| คำภาษาอังกฤษเดิม | ลิงก์หน้าต้นฉบับ | ลิงก์ภาษาไทย |\n| --- | --- | --- |");

    for link in links {
        let translated = link.translated.as_deref().unwrap_or_default();
        table.push_str(&format!(
            "\n| {} | {} | {} |",
            escape_table_cell(&link.title),
            escape_table_cell(&link.link_to),
            escape_table_cell(translated),
        ));
    }

    table
}

fn escape_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn build_langlink_table(links: &[Link], langlinks: &[LangLink]) -> Vec<LinkTableRow> {
    let translated_by_title = langlinks
        .iter()
        .map(|link| (normalize_title(&link.page_title), link.title.clone()))
        .collect::<HashMap<_, _>>();

    links
        .iter()
        .filter(|link| link.namespace == 0)
        .map(|link| LinkTableRow {
            // The raw links API provides target titles, not wikitext display labels.
            // Parsing labels belongs to a later stage.
            title: link.title.clone(),
            link_to: link.title.clone(),
            translated: translated_by_title
                .get(&normalize_title(&link.title))
                .cloned(),
        })
        .collect()
}

fn normalize_title(title: &str) -> String {
    title.replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::{build_langlink_table, glossary_markdown, LinkTableFetcher};
    use crate::wiki::fetcher::{LangLink, Link};
    use crate::wiki::LinkTableRow;

    #[test]
    fn builds_a_table_for_page_links_and_leaves_untranslated_targets_empty() {
        let langlinks = [LangLink {
            page_title: "Flower".to_string(),
            title: "ดอกไม้".to_string(),
            lang: "th".to_string(),
        }];

        let links = [
            Link {
                page_title: "Source page".to_string(),
                title: "Flower".to_string(),
                namespace: 0,
                exists: true,
            },
            Link {
                page_title: "Source page".to_string(),
                title: "Petal".to_string(),
                namespace: 0,
                exists: true,
            },
            Link {
                page_title: "Source page".to_string(),
                title: "File:Petal.jpg".to_string(),
                namespace: 6,
                exists: true,
            },
        ];

        let table = build_langlink_table(&links, &langlinks);

        assert_eq!(table.len(), 2);
        assert_eq!(table[0].title, "Flower");
        assert_eq!(table[0].link_to, "Flower");
        assert_eq!(table[0].translated.as_deref(), Some("ดอกไม้"));
        assert_eq!(table[1].title, "Petal");
        assert_eq!(table[1].link_to, "Petal");
        assert_eq!(table[1].translated, None);
    }

    #[test]
    fn glossary_markdown_includes_link_table_rows() {
        let glossary = glossary_markdown(&[LinkTableRow {
            title: "Mercury".to_string(),
            link_to: "Mercury (planet)".to_string(),
            translated: Some("ดาวพุธ".to_string()),
        }]);

        assert!(glossary.contains("| คำภาษาอังกฤษเดิม | ลิงก์หน้าต้นฉบับ | ลิงก์ภาษาไทย |"));
        assert!(glossary.contains("| Mercury | Mercury (planet) | ดาวพุธ |"));
    }

    #[test]
    fn glossary_markdown_escapes_table_delimiters() {
        let glossary = glossary_markdown(&[LinkTableRow {
            title: "A | B".to_string(),
            link_to: "Source".to_string(),
            translated: None,
        }]);

        assert!(glossary.contains("| A \\| B | Source |  |"));
    }

    #[tokio::test]
    async fn fetches_a_page_and_builds_its_langlink_table() {
        let fetcher = LinkTableFetcher::new("en").await.unwrap();
        let table = fetcher.get_langlink_table("Hello").await.unwrap();

        println!("{:#?}", table);

        assert!(!table.is_empty());
    }
}
