use mwapi::Client;

pub async fn build_client(lang: &str) -> Result<Client, String> {
    let url = format!("https://{}.wikipedia.org/w/api.php", lang);
    Client::builder(&url)
        .build()
        .await
        .map_err(|err| err.to_string())
}

#[derive(Debug)]
pub struct Link {
    pub title: String,
    pub exists: bool,
}

pub async fn fetch_links(client: &Client, page: &str) -> Result<Vec<Link>, String> {
    let response = client
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

#[derive(Debug)]
pub struct LangLink {
    pub english_title: String,
    pub title: String,
    pub lang: String,
}

pub async fn fetch_lang_links(client: &Client, page: &[&str]) -> Result<Vec<LangLink>, String> {
    let formatted_titles = page.join("|");

    let response = client
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
            let lang = link["lang"]
                .as_str()
                .ok_or_else(|| "MediaWiki language link did not contain a language".to_string())?;

            Ok(LangLink {
                english_title: english_title.to_string(),
                title: title.to_string(),
                lang: lang.to_string(),
            })
        })
        .collect::<Result<Vec<_>, String>>()
}

pub async fn fetch_wikitext(client: &Client, page: &str) -> Result<String, String> {
    let response = client
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

#[derive(Debug)]
pub struct LinkTableRow {
    title: String,
    link_to: String,
    translated: Option<String>,
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn fetch_links_works() {
        let client = super::build_client("en").await.unwrap();
        let links = super::fetch_links(&client, "Hello").await.unwrap();
        println!("{:#?}", links);
        assert!(links.len() > 0);
    }

    #[tokio::test]
    async fn fetch_lang_links_works() {
        let client = super::build_client("en").await.unwrap();
        let links = super::fetch_lang_links(&client, &["English language"])
            .await
            .unwrap();
        println!("{:#?}", links);
    }

    #[tokio::test]
    async fn fetch_wikitext_works() {
        let client = super::build_client("en").await.unwrap();
        let wikitext = super::fetch_wikitext(&client, "Hello").await.unwrap();
        println!("{:#?}", wikitext);
    }
}
