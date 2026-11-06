mod error;
mod markdown;
mod translate;
mod wiki;

#[cfg(target_arch = "wasm32")]
mod wasm;

use serde::Serialize;

pub use crate::error::Error;
pub use crate::markdown::StubbedPage;
pub use crate::translate::translate_with_openai;
pub use crate::wiki::{
    glossary_markdown, LinkTableFetcher, LinkTableRow, RawFetcher, StubbedPageFetcher,
};

use crate::wiki::wikipedia_client;

/// Fetches a page's parsed wikitext and links from the MediaWiki API.
async fn fetch_page_response(lang: &str, page: &str) -> std::result::Result<String, String> {
    let client = wikipedia_client(lang)
        .await
        .map_err(|err| err.to_string())?;

    let resp = client
        .get_value(&[
            ("action", "parse"),
            ("page", page),
            ("prop", "wikitext|links"),
            ("format", "json"),
            ("origin", "*"),
        ])
        .await
        .map_err(|err| err.to_string())?;

    Ok(resp.to_string())
}

/// Fetches a stubbed page and its glossary link table as JSON.
async fn fetch_stubbed_page_json(lang: &str, page: &str) -> std::result::Result<String, String> {
    let stubbed_page = StubbedPageFetcher::new(lang)
        .await
        .map_err(|error| error.to_string())?
        .get_stubbed_page(page)
        .await
        .map_err(|error| error.to_string())?;
    let links = LinkTableFetcher::new(lang)
        .await
        .map_err(|error| error.to_string())?
        .get_langlink_table(page)
        .await
        .map_err(|error| error.to_string())?;

    #[derive(Serialize)]
    struct StubbedPageResponse {
        #[serde(flatten)]
        page: StubbedPage,
        links: Vec<LinkTableRow>,
    }

    serde_json::to_string(&StubbedPageResponse {
        page: stubbed_page,
        links,
    })
    .map_err(|error| error.to_string())
}

/// Restores a stubbed page's JSON back to MediaWiki wikitext.
fn restub_page_json(stubbed_page_json: &str) -> std::result::Result<String, String> {
    Ok(markdown::restub_page(&StubbedPage::from_json(
        stubbed_page_json,
    )?))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_page(lang: &str, page: &str) -> std::result::Result<String, String> {
    fetch_page_response(lang, page).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_stubbed_page(lang: &str, page: &str) -> std::result::Result<String, String> {
    fetch_stubbed_page_json(lang, page).await
}

#[cfg(not(target_arch = "wasm32"))]
pub fn restub_page(stubbed_page_json: &str) -> std::result::Result<String, String> {
    restub_page_json(stubbed_page_json)
}

#[cfg(test)]
mod tests {
    use super::{fetch_page, restub_page_json, StubbedPage};
    use std::time::Duration;

    #[tokio::test]
    async fn fetch_english_hello_page() {
        let response = fetch_page("en", "hello")
            .await
            .expect("Wiktionary request should succeed");

        assert!(
            response.contains("\"wikitext\""),
            "expected a parse response containing wikitext, got: {response}"
        );
    }

    #[tokio::test]
    async fn fetch_page_finishes_within_15_seconds() {
        let result = tokio::time::timeout(Duration::from_secs(15), fetch_page("en", "hello")).await;

        assert!(
            result.is_ok(),
            "fetch_page did not finish within 15 seconds"
        );
    }

    #[test]
    fn restub_page_json_restores_stubbed_wikitext() {
        let page = StubbedPage::from("== Title ==");
        let json = page.to_json().unwrap();

        assert_eq!(restub_page_json(&json).unwrap(), "== Title ==");
    }
}
