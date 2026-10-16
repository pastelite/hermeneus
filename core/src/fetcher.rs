use mwapi::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::parser::{stub_links, stub_ref, stub_template, stub_title};
use crate::restub::StubbedPage;

/// MediaWiki's maximum number of titles accepted in one `titles` request.
pub const MAX_TITLES_PER_REQUEST: usize = 50;

pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub async fn new(lang: &str) -> Result<Self, String> {
        let url = format!("https://{}.wikipedia.org/w/api.php", lang);
        let client = Client::builder(&url)
            .build()
            .await
            .map_err(|err| err.to_string())?;

        Ok(Self { client })
    }

    pub async fn fetch_links(&self, page: &str) -> Result<Vec<Link>, String> {
        let response = self
            .client
            .get_value(&[
                ("action", "parse"),
                ("page", page),
                ("prop", "links"),
                ("redirects", "1"),
                ("format", "json"),
                ("origin", "*"),
            ])
            .await
            .map_err(|err| err.to_string())?;

        let links = response["parse"]["links"]
            .as_array()
            .ok_or_else(|| "MediaWiki response did not contain a links array".to_string())?;

        links
            .iter()
            .filter(|link| link["ns"].as_i64() == Some(0))
            .map(|link| {
                let title = link["title"]
                    .as_str()
                    .ok_or_else(|| "MediaWiki link did not contain a title".to_string())?;

                Ok(Link {
                    title: title.to_string(),
                    // MediaWiki represents existence by including the `exists` key;
                    // its value is not guaranteed to be a JSON boolean.
                    exists: link.get("exists").is_some(),
                })
            })
            .collect()
    }

    pub async fn fetch_lang_links(&self, page: &[&str]) -> Result<Vec<LangLink>, String> {
        let mut lang_links = Vec::new();

        for titles in page.chunks(MAX_TITLES_PER_REQUEST) {
            let formatted_titles = titles.join("|");

            let response = self
                .client
                .get_value(&[
                    ("action", "query"),
                    ("titles", &formatted_titles),
                    ("prop", "langlinks"),
                    ("lllang", "th"),
                    ("redirects", "1"),
                    ("formatversion", "2"),
                    ("format", "json"),
                    ("origin", "*"),
                ])
                .await
                .map_err(|err| err.to_string())?;

            lang_links.extend(parse_lang_links(&response, titles)?);
        }

        Ok(lang_links)
    }

    pub async fn fetch_wikitext(&self, page: &str) -> Result<String, String> {
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
            .await
            .map_err(|err| err.to_string())?;

        Ok(response["parse"]["wikitext"]
            .as_str()
            .ok_or_else(|| "MediaWiki response did not contain wikitext".to_string())?
            .to_string())
    }

    pub async fn fetch_links_with_langlinks(&self, page: &str) -> Result<Vec<LangLink>, String> {
        let links = self.fetch_links(page).await?;
        let links_list = links
            .iter()
            .map(|link| link.title.as_str())
            .collect::<Vec<_>>();

        let lang_links = self.fetch_lang_links(&links_list).await?;
        Ok(lang_links)
    }

    pub async fn get_stubbed_page(&self, page: &str) -> Result<StubbedPage, String> {
        let wikitext = self.fetch_wikitext(page).await?;
        let links = self.fetch_links_with_langlinks(page).await?;

        let (template_wikitext, template_map) = stub_template(&wikitext);
        let (reference_wikitext, reference_map) = stub_ref(&template_wikitext);
        let link_wikitext = stub_links(&reference_wikitext, &links);
        let titled_wikitext = stub_title(&link_wikitext);

        Ok(StubbedPage {
            stubbed: titled_wikitext,
            template: template_map,
            references: reference_map,
        })
    }
}

fn normalize_title(title: &str) -> String {
    title.replace('_', " ")
}

fn parse_lang_links(response: &Value, requested_titles: &[&str]) -> Result<Vec<LangLink>, String> {
    let pages = response["query"]["pages"]
        .as_array()
        .ok_or_else(|| "MediaWiki response did not contain a pages array".to_string())?;

    let mut links_by_title = HashMap::new();
    for page in pages {
        let Some(link) = page["langlinks"].as_array().and_then(|links| links.first()) else {
            continue;
        };
        let english_title = page["title"]
            .as_str()
            .ok_or_else(|| "MediaWiki page did not contain a title".to_string())?;
        let title = link["title"]
            .as_str()
            .ok_or_else(|| "MediaWiki language link did not contain a title".to_string())?;
        let lang = link["lang"]
            .as_str()
            .ok_or_else(|| "MediaWiki language link did not contain a language".to_string())?;

        links_by_title.insert(
            normalize_title(english_title),
            LangLink {
                english_title: english_title.to_string(),
                title: title.to_string(),
                lang: lang.to_string(),
            },
        );
    }

    let redirects = response["query"]["redirects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|redirect| {
            Some((
                normalize_title(redirect["from"].as_str()?),
                normalize_title(redirect["to"].as_str()?),
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut output = links_by_title.values().cloned().collect::<Vec<_>>();
    let mut included_titles = links_by_title.keys().cloned().collect::<HashSet<_>>();

    for requested_title in requested_titles {
        let requested_title = normalize_title(requested_title);
        let mut resolved_title = requested_title.clone();
        let mut visited_titles = HashSet::new();
        while visited_titles.insert(resolved_title.clone()) {
            let Some(next_title) = redirects.get(&resolved_title) else {
                break;
            };
            resolved_title = next_title.clone();
        }

        if resolved_title == requested_title || !included_titles.insert(requested_title.clone()) {
            continue;
        }

        if let Some(link) = links_by_title.get(&resolved_title) {
            output.push(LangLink {
                english_title: requested_title,
                title: link.title.clone(),
                lang: link.lang.clone(),
            });
        }
    }

    Ok(output)
}

#[derive(Debug)]
pub struct Link {
    pub title: String,
    pub exists: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LangLink {
    pub english_title: String,
    pub title: String,
    pub lang: String,
}

#[derive(Debug)]
pub struct LinkTableRow {
    title: String,
    link_to: String,
    translated: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{parse_lang_links, Fetcher};
    use crate::parser::stub_links;
    use serde_json::json;

    #[test]
    fn fetch_lang_links_maps_redirects_to_the_resolved_page_translation() {
        let response = json!({
            "query": {
                "redirects": [
                    { "from": "Alias", "to": "Intermediate" },
                    { "from": "Intermediate", "to": "Target" }
                ],
                "pages": [{
                    "title": "Target",
                    "langlinks": [{ "title": "เป้าหมาย", "lang": "th" }]
                }]
            }
        });

        let links = parse_lang_links(&response, &["Alias", "Target"]).unwrap();

        assert!(links.iter().any(|link| {
            link.english_title == "Alias" && link.title == "เป้าหมาย" && link.lang == "th"
        }));
        assert!(links.iter().any(|link| {
            link.english_title == "Target" && link.title == "เป้าหมาย" && link.lang == "th"
        }));
        assert_eq!(stub_links("[[Alias]]", &links), "[Alias](เป้าหมาย)");
    }

    #[tokio::test]
    async fn wikipedia_hello_world_redirect_fetches_page_and_thai_lang_link() {
        let fetcher = Fetcher::new("en").await.unwrap();

        let redirected_wikitext = fetcher.fetch_wikitext("Hello world").await.unwrap();
        let canonical_wikitext = fetcher.fetch_wikitext("Hello, world").await.unwrap();
        assert!(!redirected_wikitext.is_empty());
        assert_eq!(redirected_wikitext, canonical_wikitext);

        let stubbed_page = fetcher.get_stubbed_page("Hello world").await.unwrap();
        assert!(!stubbed_page.stubbed.is_empty());

        let redirected_lang_links = fetcher.fetch_lang_links(&["Hello world"]).await.unwrap();
        let canonical_lang_links = fetcher.fetch_lang_links(&["Hello, world"]).await.unwrap();
        let redirected_lang_link = redirected_lang_links
            .iter()
            .find(|link| link.english_title == "Hello world" && link.lang == "th")
            .expect("redirect alias should use the target page's Thai language link");
        let canonical_lang_link = canonical_lang_links
            .iter()
            .find(|link| link.english_title == "Hello, world" && link.lang == "th")
            .expect("canonical page should have a Thai language link");
        assert_eq!(redirected_lang_link.title, canonical_lang_link.title);
    }

    #[tokio::test]
    async fn fetch_links_works() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let links = fetcher.fetch_links("Hello").await.unwrap();
        println!("{:#?}", links);
        assert!(links.len() > 0);
    }

    #[tokio::test]
    async fn fetch_lang_links_works() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let links = fetcher
            .fetch_lang_links(&["English language"])
            .await
            .unwrap();
        println!("{:#?}", links);
    }

    #[tokio::test]
    async fn fetch_wikitext_works() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let wikitext = fetcher.fetch_wikitext("Hello").await.unwrap();
        println!("{:#?}", wikitext);
    }

    #[tokio::test]
    async fn fetch_links_with_langlinks_works() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let links = fetcher.fetch_links_with_langlinks("Hello").await.unwrap();
        println!("{:#?}", links);
    }

    #[tokio::test]
    async fn get_stubbed_page_works() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let page = fetcher.get_stubbed_page("Hello").await.unwrap();
        println!("{:#?}", page);
    }
}
