use mwapi::Client as MWClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;

/// MediaWiki's maximum number of titles accepted in one `titles` request.
pub const MAX_TITLES_PER_REQUEST: usize = 50;

pub struct RawFetcher {
    client: MWClient,
}

impl RawFetcher {
    /// Creates a raw fetcher from an existing MediaWiki client.
    pub fn new(client: MWClient) -> Self {
        Self { client }
    }

    /// Fetches all links reported for each requested page.
    ///
    /// MediaWiki redirects are followed, so links are collected from the resolved page.
    /// The returned [`Link`]s retain their source page title and namespace. One API
    /// request is made for each requested page.
    pub async fn fetch_links(&self, page_titles: &[&str]) -> Result<Vec<Link>> {
        let mut links = Vec::new();

        for page_title in page_titles {
            let response = self
                .client
                .get_value(&[
                    ("action", "parse"),
                    ("page", page_title),
                    ("prop", "links"),
                    ("redirects", "1"),
                    ("formatversion", "2"),
                    ("format", "json"),
                    ("origin", "*"),
                ])
                .await?;

            links.extend(parse_links(&response)?);
        }

        Ok(links)
    }

    /// Fetches the Thai language link for each requested page, if one exists.
    ///
    /// This queries language links on the requested pages themselves; it does not
    /// inspect links contained in those pages. MediaWiki redirects are followed, and
    /// each result's [`LangLink::page_title`] is the resolved source page title.
    pub async fn fetch_langlinks(&self, page_titles: &[&str]) -> Result<Vec<LangLink>> {
        self.fetch_langlinks_for_language(page_titles, "th").await
    }

    /// Fetches language links in `lang` for each requested page, if one exists.
    ///
    /// This queries language links on the requested pages themselves; it does not
    /// inspect links contained in those pages. MediaWiki redirects are followed, and
    /// each result's [`LangLink::page_title`] is the resolved source page title.
    pub async fn fetch_langlinks_for_language(
        &self,
        page_titles: &[&str],
        lang: &str,
    ) -> Result<Vec<LangLink>> {
        let mut langlinks = Vec::new();

        for titles in page_titles.chunks(MAX_TITLES_PER_REQUEST) {
            let formatted_titles = titles.join("|");

            let response = self
                .client
                .get_value(&[
                    ("action", "query"),
                    ("titles", &formatted_titles),
                    ("prop", "langlinks"),
                    ("lllang", lang),
                    ("lllimit", "max"),
                    ("redirects", "1"),
                    ("formatversion", "2"),
                    ("format", "json"),
                    ("origin", "*"),
                ])
                .await?;

            langlinks.extend(parse_langlinks(&response)?);
        }

        Ok(langlinks)
    }

    /// Fetches the wikitext of a page, following MediaWiki redirects.
    pub async fn fetch_wikitext(&self, page: &str) -> Result<String> {
        let response = self
            .client
            .get_value(&[
                ("action", "parse"),
                ("page", page),
                ("prop", "wikitext"),
                ("redirects", "1"),
                ("format", "json"),
                ("origin", "*"),
            ])
            .await?;

        Ok(response["parse"]["wikitext"]
            .as_str()
            .ok_or_else(|| "MediaWiki response did not contain wikitext".to_string())?
            .to_string())
    }
}

fn parse_links(response: &Value) -> Result<Vec<Link>> {
    let page = &response["parse"];
    let page_title = page["title"]
        .as_str()
        .ok_or_else(|| "MediaWiki page did not contain a title".to_string())?;
    let page_links = page["links"]
        .as_array()
        .ok_or_else(|| "MediaWiki response did not contain a links array".to_string())?;

    page_links
        .iter()
        .map(|link| {
            let namespace = link["ns"]
                .as_i64()
                .ok_or_else(|| "MediaWiki link did not contain a namespace".to_string())?;
            let title = link["title"]
                .as_str()
                .ok_or_else(|| "MediaWiki link did not contain a title".to_string())?;
            Ok(Link {
                page_title: page_title.to_string(),
                namespace,
                title: title.to_string(),
                // MediaWiki represents existence by including the `exists` key;
                // its value is not guaranteed to be a JSON boolean.
                exists: link.get("exists").is_some(),
            })
        })
        .collect()
}

fn parse_langlinks(response: &Value) -> Result<Vec<LangLink>> {
    let pages = response["query"]["pages"]
        .as_array()
        .ok_or_else(|| "MediaWiki response did not contain a pages array".to_string())?;

    let mut langlinks = Vec::new();
    for page in pages {
        let Some(page_langlinks) = page["langlinks"].as_array() else {
            continue;
        };
        let page_title = page["title"]
            .as_str()
            .ok_or_else(|| "MediaWiki page did not contain a title".to_string())?;

        for link in page_langlinks {
            let title = link["title"]
                .as_str()
                .ok_or_else(|| "MediaWiki language link did not contain a title".to_string())?;
            let lang = link["lang"]
                .as_str()
                .ok_or_else(|| "MediaWiki language link did not contain a language".to_string())?;
            langlinks.push(LangLink {
                page_title: page_title.to_string(),
                title: title.to_string(),
                lang: lang.to_string(),
            });
        }
    }

    Ok(langlinks)
}

/// A link reported by MediaWiki for a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// The resolved source page whose links were fetched.
    pub page_title: String,
    /// The link target's MediaWiki title.
    pub title: String,
    /// The target namespace ID (`0` is the main/article namespace).
    pub namespace: i64,
    /// Whether MediaWiki marked the target page as existing.
    pub exists: bool,
}

/// A page's link to its corresponding page in another language.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LangLink {
    /// The resolved title of the source page on this wiki.
    pub page_title: String,
    /// The target page title in `lang`.
    pub title: String,
    /// The target page's language code.
    pub lang: String,
}

#[cfg(test)]
mod tests {
    use super::{parse_langlinks, parse_links, RawFetcher};
    use crate::client::wikipedia_client;
    use serde_json::json;

    #[test]
    fn parses_only_langlinks_attached_to_the_requested_page_results() {
        let response = json!({
            "query": {
                "pages": [
                    {
                        "title": "Flower",
                        "langlinks": [{ "title": "ดอกไม้", "lang": "th" }]
                    },
                    {
                        "title": "Petal",
                        "langlinks": [{ "title": "กลีบดอก", "lang": "th" }]
                    }
                ]
            }
        });

        let langlinks = parse_langlinks(&response).unwrap();

        assert_eq!(langlinks.len(), 2);
        assert!(langlinks.iter().any(|link| {
            link.page_title == "Flower" && link.title == "ดอกไม้" && link.lang == "th"
        }));
        assert!(langlinks.iter().any(|link| {
            link.page_title == "Petal" && link.title == "กลีบดอก" && link.lang == "th"
        }));
    }

    #[test]
    fn parses_all_links_and_preserves_namespace_and_existence() {
        let response = json!({
            "parse": {
                "title": "Flower",
                "links": [
                    { "ns": 0, "title": "Petal", "exists": "" },
                    { "ns": 0, "title": "Rose" },
                    { "ns": 6, "title": "File:Flower.jpg" }
                ]
            }
        });

        let links = parse_links(&response).unwrap();

        assert_eq!(
            links,
            vec![
                super::Link {
                    page_title: "Flower".to_string(),
                    title: "Petal".to_string(),
                    namespace: 0,
                    exists: true,
                },
                super::Link {
                    page_title: "Flower".to_string(),
                    title: "Rose".to_string(),
                    namespace: 0,
                    exists: false,
                },
                super::Link {
                    page_title: "Flower".to_string(),
                    title: "File:Flower.jpg".to_string(),
                    namespace: 6,
                    exists: false,
                },
            ]
        );
    }

    #[tokio::test]
    async fn new_builds_client_for_language() {
        let client = wikipedia_client("en").await.unwrap();
        let _fetcher = RawFetcher::new(client);
    }

    #[tokio::test]
    async fn fetch_methods_follow_redirects() {
        let client = wikipedia_client("en").await.unwrap();
        let fetcher = RawFetcher::new(client);

        let multiple_page_links = fetcher.fetch_links(&["Hello", "World"]).await.unwrap();
        assert!(multiple_page_links
            .iter()
            .any(|link| link.page_title == "Hello"));
        assert!(multiple_page_links
            .iter()
            .any(|link| link.page_title == "World"));

        let redirected_links = fetcher.fetch_links(&["Hello world"]).await.unwrap();
        let canonical_links = fetcher.fetch_links(&["Hello, world"]).await.unwrap();
        assert!(!redirected_links.is_empty());
        assert!(redirected_links
            .iter()
            .all(|link| link.page_title == "Hello, world"));
        assert_eq!(
            redirected_links
                .iter()
                .map(|link| &link.title)
                .collect::<Vec<_>>(),
            canonical_links
                .iter()
                .map(|link| &link.title)
                .collect::<Vec<_>>()
        );

        let redirected_langlinks = fetcher.fetch_langlinks(&["Hello world"]).await.unwrap();
        let canonical_langlinks = fetcher.fetch_langlinks(&["Hello, world"]).await.unwrap();
        assert_eq!(redirected_langlinks.len(), 1);
        let redirected_langlink = redirected_langlinks
            .iter()
            .find(|link| link.page_title == "Hello, world" && link.lang == "th")
            .expect("redirect alias should use the target page's Thai language link");
        let canonical_langlink = canonical_langlinks
            .iter()
            .find(|link| link.page_title == "Hello, world" && link.lang == "th")
            .expect("canonical page should have a Thai language link");
        assert_eq!(redirected_langlink.title, canonical_langlink.title);

        let redirected_wikitext = fetcher.fetch_wikitext("Hello world").await.unwrap();
        let canonical_wikitext = fetcher.fetch_wikitext("Hello, world").await.unwrap();
        assert!(!redirected_wikitext.is_empty());
        assert_eq!(redirected_wikitext, canonical_wikitext);
    }

    #[tokio::test]
    async fn fetch_langlinks_for_language_uses_the_requested_language() {
        let client = wikipedia_client("en").await.unwrap();
        let fetcher = RawFetcher::new(client);

        let langlinks = fetcher
            .fetch_langlinks_for_language(&["English language"], "fr")
            .await
            .unwrap();

        assert!(langlinks
            .iter()
            .any(|link| { link.page_title == "English language" && link.lang == "fr" }));
    }
}
