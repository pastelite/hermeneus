pub mod fetcher;
pub mod parser;
pub mod restub;
use mwapi::Client;
use wasm_bindgen::prelude::*;

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

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn fetch_page(lang: &str, page: &str) -> Result<String, JsValue> {
    fetch_page_response(lang, page)
        .await
        .map_err(|err| JsValue::from_str(&err))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_page(lang: &str, page: &str) -> Result<String, String> {
    fetch_page_response(lang, page).await
}

#[cfg(test)]
mod tests {
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
}
