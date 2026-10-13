use wiki_translate_client::fetch_page;
use wiki_translate_client::fetcher::fetch_lang_links;

#[tokio::main]
async fn main() {
    match fetch_page("en", "hello").await {
        Ok(response) => println!("{response}"),
        Err(error) => eprintln!("fetch_page failed: {error:?}"),
    }
    // match fetch_lang_links("en", "Flower").await {
    //     Ok(response) => println!("{response}"),
    //     Err(error) => eprintln!("fetch_lang_links failed: {error:?}"),
    // }
}
