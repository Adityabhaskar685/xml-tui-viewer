use crate::parser::reader::Node;
use regex::Regex;

#[allow(dead_code)]
pub fn search(nodes: &[Node], pattern: &str) -> Vec<usize> {
    let re = Regex::new(pattern).unwrap();

    nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| re.is_match(&n.tag))
        .map(|(i, _)| i)
        .collect()
}
