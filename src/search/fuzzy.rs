use crate::parser::reader::Node;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use rayon::prelude::*;

pub struct FuzzySearchResult {
    pub node_idx: usize,
    pub score: i64,
    pub matched_indices: Vec<usize>,
}

pub struct FuzzySearcher {
    matcher: SkimMatcherV2,
}

impl FuzzySearcher {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default().smart_case().ignore_case(),
        }
    }

    pub fn search(&self, nodes: &[Node], pattern: &str) -> Vec<FuzzySearchResult> {
        if pattern.is_empty() {
            return vec![];
        }

        nodes
            .par_iter()
            .enumerate()
            .filter_map(|(idx, node)| {
                let (score, indices) = self.matcher.fuzzy_indices(&node.tag, pattern)?;
                Some(FuzzySearchResult {
                    node_idx: idx,
                    score,
                    matched_indices: indices,
                })
            })
            .collect()
    }

    pub fn search_with_path(&self, nodes: &[Node], pattern: &str) -> Vec<FuzzySearchResult> {
        if pattern.is_empty() {
            return vec![];
        }

        let paths = build_node_paths(nodes);

        paths
            .par_iter()
            .enumerate()
            .filter_map(|(idx, path)| {
                let (score, indices) = self.matcher.fuzzy_indices(path, pattern)?;
                Some(FuzzySearchResult {
                    node_idx: idx,
                    score,
                    matched_indices: indices,
                })
            })
            .collect()
    }

    pub fn search_attributes(&self, nodes: &[Node], pattern: &str) -> Vec<FuzzySearchResult> {
        if pattern.is_empty() {
            return vec![];
        }

        nodes
            .par_iter()
            .enumerate()
            .filter_map(|(idx, node)| {
                let attr_string: String = node
                    .attributes
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(" ");

                if attr_string.is_empty() {
                    return None;
                }

                let (score, _) = self.matcher.fuzzy_indices(&attr_string, pattern)?;
                Some(FuzzySearchResult {
                    node_idx: idx,
                    score,
                    matched_indices: vec![],
                })
            })
            .collect()
    }
}

fn build_node_paths(nodes: &[Node]) -> Vec<String> {
    let mut paths = vec![String::new(); nodes.len()];

    for (idx, node) in nodes.iter().enumerate() {
        let mut path_parts = vec![node.tag.as_str()];
        let mut current = node.parent;

        while let Some(parent_idx) = current {
            if let Some(parent) = nodes.get(parent_idx) {
                path_parts.push(&parent.tag);
                current = parent.parent;
            } else {
                break;
            }
        }

        path_parts.reverse();
        paths[idx] = path_parts.join("/");
    }

    paths
}

pub fn fuzzy_search(nodes: &[Node], pattern: &str) -> Vec<usize> {
    let searcher = FuzzySearcher::new();
    let mut results = searcher.search(nodes, pattern);
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.iter().map(|r| r.node_idx).collect()
}

pub fn fuzzy_search_with_paths(nodes: &[Node], pattern: &str) -> Vec<usize> {
    let searcher = FuzzySearcher::new();
    let mut results = searcher.search_with_path(nodes, pattern);
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.iter().map(|r| r.node_idx).collect()
}
