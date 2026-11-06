use wasm_bindgen::prelude::*;

use crate::translate::get_full_prompt;
use crate::wiki::LinkTableRow;
use crate::{fetch_page_response, fetch_stubbed_page_json, restub_page_json};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen(start)]
pub fn test_bindgen() {
    log("Wiktionary WASM loaded");
}

/// Fetches a page's parsed wikitext and links.
#[wasm_bindgen]
pub async fn fetch_page(lang: &str, page: &str) -> Result<String, JsValue> {
    fetch_page_response(lang, page)
        .await
        .map_err(|err| JsValue::from_str(&err))
}

/// Fetches a stubbed page and its glossary link table as JSON.
#[wasm_bindgen]
pub async fn fetch_stubbed_page(lang: &str, page: &str) -> Result<String, JsValue> {
    fetch_stubbed_page_json(lang, page)
        .await
        .map_err(|error| JsValue::from_str(&error))
}

/// Restores a stubbed page's JSON back to MediaWiki wikitext.
#[wasm_bindgen]
pub fn restub_page(stubbed_page_json: &str) -> Result<String, JsValue> {
    restub_page_json(stubbed_page_json).map_err(|error| JsValue::from_str(&error))
}

/// Builds the full translation prompt from Markdown and a glossary JSON array.
#[wasm_bindgen(js_name = get_full_prompt)]
pub fn get_full_prompt_for_web(original_text: &str, links_json: &str) -> Result<String, JsValue> {
    let links: Vec<LinkTableRow> =
        serde_json::from_str(links_json).map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(get_full_prompt(original_text, &links))
}
