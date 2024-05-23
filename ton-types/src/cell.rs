use std::fmt::{Debug, Formatter};

pub type CellId = u8;

#[derive(PartialEq, Eq)]
pub struct Cell {
    content: Vec<u8>,
    refs: Vec<CellId>
}

impl Cell {
    pub fn new(content: Vec<u8>, refs: Vec<u8>) -> Self {
        Self { content, refs }
    }

    pub fn refs(&self) -> &[u8] {
        &self.refs
    }
}

impl AsRef<[u8]> for Cell {
    fn as_ref(&self) -> &[u8] {
        &self.content
    }
}

impl Debug for Cell {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cell")
            .field("content", &hex::encode(&self.content))
            .field("refs", &self.refs)
        .finish()
    }
}
