use mwapi::Client;

use crate::parser::{stub_links, stub_ref, stub_template};
use crate::restub::StubbedPage;

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
        let formatted_titles = page.join("|");

        let response = self
            .client
            .get_value(&[
                ("action", "query"),
                ("titles", &formatted_titles),
                ("prop", "langlinks"),
                ("lllang", "th"),
                ("formatversion", "2"),
                ("format", "json"),
                ("origin", "*"),
            ])
            .await
            .map_err(|err| err.to_string())?;

        let pages = response["query"]["pages"]
            .as_array()
            .ok_or_else(|| "MediaWiki response did not contain a pages array".to_string())?;

        pages
            .iter()
            .filter_map(|page| {
                page["langlinks"]
                    .as_array()?
                    .first()
                    .map(|link| (page, link))
            })
            .map(|(page, link)| {
                let english_title = page["title"]
                    .as_str()
                    .ok_or_else(|| "MediaWiki page did not contain a title".to_string())?;
                let title = link["title"]
                    .as_str()
                    .ok_or_else(|| "MediaWiki language link did not contain a title".to_string())?;
                let lang = link["lang"].as_str().ok_or_else(|| {
                    "MediaWiki language link did not contain a language".to_string()
                })?;

                Ok(LangLink {
                    english_title: english_title.to_string(),
                    title: title.to_string(),
                    lang: lang.to_string(),
                })
            })
            .collect::<Result<Vec<_>, String>>()
    }

    pub async fn fetch_wikitext(&self, page: &str) -> Result<String, String> {
        let response = self
            .client
            .get_value(&[
                ("action", "parse"),
                ("page", page),
                ("prop", "wikitext"),
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

        Ok(StubbedPage {
            title: page.to_string(),
            stubbed: link_wikitext,
            lang_links: links,
            template: template_map,
            references: reference_map,
        })
    }
}

#[derive(Debug)]
pub struct Link {
    pub title: String,
    pub exists: bool,
}

#[derive(Debug)]
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
    use super::Fetcher;

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
