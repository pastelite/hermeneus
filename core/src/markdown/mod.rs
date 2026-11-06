mod restub;
mod stub;
mod stubbed_page;

pub use stubbed_page::StubbedPage;

/// Restores a [`StubbedPage`] to MediaWiki wikitext.
pub fn restub_page(page: &StubbedPage) -> String {
    page.restub()
}
