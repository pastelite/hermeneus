pub mod fetcher;
pub mod parser;
pub mod restub;
use mwapi::Client;
use wasm_bindgen::prelude::*;

use crate::restub::StubbedPage;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen(start)]
pub fn test_bindgen() {
    log("Wiktionary WASM loaded");
}

async fn fetch_page_response(lang: &str, page: &str) -> Result<String, String> {
    let url = format!("https://{}.wikipedia.org/w/api.php", lang);
    let client = Client::builder(&url)
        .build()
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

async fn fetch_stubbed_page_json(lang: &str, page: &str) -> Result<String, String> {
    let fetcher = fetcher::Fetcher::new(lang).await?;
    fetcher.get_stubbed_page(page).await?.to_json()
}

fn restub_page_json(stubbed_page_json: &str) -> Result<String, String> {
    Ok(StubbedPage::from_json(stubbed_page_json)?.restub())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn fetch_page(lang: &str, page: &str) -> Result<String, JsValue> {
    fetch_page_response(lang, page)
        .await
        .map_err(|err| JsValue::from_str(&err))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn fetch_stubbed_page(lang: &str, page: &str) -> Result<String, JsValue> {
    fetch_stubbed_page_json(lang, page)
        .await
        .map_err(|error| JsValue::from_str(&error))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn restub_page(stubbed_page_json: &str) -> Result<String, JsValue> {
    restub_page_json(stubbed_page_json).map_err(|error| JsValue::from_str(&error))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_page(lang: &str, page: &str) -> Result<String, String> {
    fetch_page_response(lang, page).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_stubbed_page(lang: &str, page: &str) -> Result<String, String> {
    fetch_stubbed_page_json(lang, page).await
}

#[cfg(not(target_arch = "wasm32"))]
pub fn restub_page(stubbed_page_json: &str) -> Result<String, String> {
    restub_page_json(stubbed_page_json)
}

#[cfg(test)]
mod tests {
    use crate::fetcher::Fetcher;

    use super::fetch_page;
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

    #[tokio::test]
    async fn get_translation_ready_page() {
        let fetcher = Fetcher::new("en").await.unwrap();
        let page = fetcher
            .get_stubbed_page("Textured_vegetable_protein")
            .await
            .unwrap();
        println!("{}", page.stubbed);
    }
}
