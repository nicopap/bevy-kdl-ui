use kdl::KdlValue;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Template parameters should have an explicit name, instead got {0:?}")]
    NonstringParam(KdlValue),
    #[error("Template node parameters should have a unique child node")]
    BadTemplateNodeParam,
    #[error("Template has no body")]
    NoBody,
    #[error("The input is not properly formatted KDL: {0}")]
    Kdl(#[from] kdl::KdlError),
    #[error("The input to `read_thunk`, is specifically `export`, which is not a thunk")]
    NotThunk,
    #[error("The provided KdlDocument is empty")]
    Empty,
}
impl Error {
    const NONSTR_PARAM: &'static str =
        "Template definition entries represent the tparameters of the template. \
tparameters **must** have a name so that it's possible to refer to them in the \
body of the template. See \
https://github.com/nicopap/bevy-kdl-ui/tree/main/template-kdl#function-templates \
for how to declare a template.";
    const NO_BODY: &'static str =
        "A template definition must have a body. See how to use templates at \
https://github.com/nicopap/bevy-kdl-ui/tree/main/template-kdl#value-templates";
    pub fn help(&self) -> Option<String> {
        match self {
            Error::NonstringParam(_) => Some(Self::NONSTR_PARAM.to_owned()),
            Error::NoBody => Some(Self::NO_BODY.to_owned()),
            _ => None,
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
