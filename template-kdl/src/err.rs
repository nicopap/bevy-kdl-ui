#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("This operation is unsupported: {0}")]
    GenericUnsupported(String),
}
impl Error {
    pub fn help(&self) -> Option<String> {
        None
    }
}
pub type Result<T> = std::result::Result<T, Error>;
