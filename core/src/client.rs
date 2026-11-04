use mwapi::Client;

use crate::error::Result;

/// Creates a MediaWiki API client for a Wikipedia language edition.
pub async fn wikipedia_client(language: &str) -> Result<Client> {
    let url = format!("https://{}.wikipedia.org/w/api.php", language);
    Ok(Client::builder(&url).build().await?)
}

#[cfg(test)]
mod tests {
    use super::wikipedia_client;

    #[tokio::test]
    async fn creates_a_client_for_a_wikipedia_language() {
        wikipedia_client("en").await.unwrap();
    }
}
