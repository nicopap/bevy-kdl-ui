use std::fmt;

#[derive(Clone, Copy, Debug)]
pub(super) struct Span {
    pub(super) offset: u32,
    pub(super) size: u32,
}
impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.offset, self.offset + self.size)
    }
}
