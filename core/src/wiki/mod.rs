mod client;
mod fetcher;
mod link_table;
mod page_fetcher;

pub use client::wikipedia_client;
pub use fetcher::RawFetcher;
pub use link_table::{LinkTableFetcher, LinkTableRow};
pub use page_fetcher::StubbedPageFetcher;
