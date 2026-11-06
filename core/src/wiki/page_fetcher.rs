use mwapi::Client;

use crate::error::Result;
use crate::markdown::StubbedPage;
use crate::wiki::client::wikipedia_client;
use crate::wiki::fetcher::RawFetcher;

/// Fetches pages and converts their wikitext to stubbed Markdown.
pub struct StubbedPageFetcher {
    raw_fetcher: RawFetcher,
}

impl StubbedPageFetcher {
    /// Creates a helper for pages on the specified wiki edition.
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

    /// Fetches `page` and converts its wikitext to a stubbed Markdown page.
    pub async fn get_stubbed_page(&self, page: &str) -> Result<StubbedPage> {
        let wikitext = self.raw_fetcher.fetch_wikitext(page).await?;
        Ok(StubbedPage::from(wikitext.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_builds_client_for_language() {
        let _fetcher = StubbedPageFetcher::new("en").await.unwrap();
    }

    #[tokio::test]
    async fn with_client_builds_a_fetcher() {
        let client = wikipedia_client("en").await.unwrap();
        let _fetcher = StubbedPageFetcher::with_client(client);
    }

    #[tokio::test]
    async fn get_stubbed_page_converts_a_live_page_to_markdown() {
        let fetcher = StubbedPageFetcher::new("en").await.unwrap();
        let page = fetcher.get_stubbed_page("Hello").await.unwrap();

        assert!(!page.stubbed.is_empty());
        assert!(page.stubbed.contains('#'));
    }

    #[tokio::test]
    async fn get_stubbed_page_returns_a_translation_ready_page() {
        let fetcher = StubbedPageFetcher::new("en").await.unwrap();
        let page = fetcher
            .get_stubbed_page("Textured_vegetable_protein")
            .await
            .unwrap();

        assert!(!page.stubbed.is_empty());
        assert!(!page.template.is_empty() || !page.references.is_empty());
    }
}
