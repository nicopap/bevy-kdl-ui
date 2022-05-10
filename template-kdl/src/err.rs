// TODO: spans here most likely
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("This operation is unsupported: {0}")]
    GenericUnsupported(String),
    #[error("The input is not properly formatted KDL: {0}")]
    Kdl(#[from] kdl::KdlError),
    #[error("The provided KdlDocument is empty")]
    Empty,
}
impl Error {
    pub fn help(&self) -> Option<String> {
        None
    }
}
pub type Result<T> = std::result::Result<T, Error>;
