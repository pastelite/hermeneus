/// The shared error type for the library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// MediaWiki API client error
    #[error(transparent)]
    MediaWiki(#[from] mwapi::Error),
    /// A message-only error for cases that do not have a typed source yet.
    #[error("{0}")]
    Message(String),
}

/// The shared result type for library operations.
pub type Result<T> = std::result::Result<T, Error>;

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Message(message.to_owned())
    }
}
