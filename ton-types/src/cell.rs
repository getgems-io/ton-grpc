use std::fmt::{Debug, Formatter};

#[derive(PartialEq, Eq)]
pub struct Cell {
    content: Vec<u8>,
    refs: Vec<u8>
}

impl Cell {
    pub fn new(content: Vec<u8>, refs: Vec<u8>) -> Self {
        Self { content, refs }
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
