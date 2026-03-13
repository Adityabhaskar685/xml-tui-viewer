#[derive(Clone, Debug)]
pub struct Node {
    pub id: usize,
    pub tag: String,
    pub depth: u16,
    pub offset: u64,
    pub end_offset: u64,
    pub parent: Option<usize>,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<usize>,
}

impl Node {
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
    
    /// Byte length of this element in the source file.
    pub fn byte_len(&self) -> u64 {
        self.end_offset.saturating_sub(self.offset)
    }
}
