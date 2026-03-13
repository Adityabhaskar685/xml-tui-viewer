use crate::parser::reader::Node;
use crate::viewer::raw_xml;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    SearchRegex,
    SearchFuzzy,
    SearchXPath,
    JumpToLine,
    Help,
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<usize>,
    pub current_result_idx: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            current_result_idx: 0,
        }
    }

    pub fn current_result(&self) -> Option<usize> {
        self.results.get(self.current_result_idx).copied()
    }

    pub fn next_result(&mut self) {
        if !self.results.is_empty() {
            self.current_result_idx = (self.current_result_idx + 1) % self.results.len();
        }
    }

    pub fn prev_result(&mut self) {
        if !self.results.is_empty() {
            self.current_result_idx = if self.current_result_idx == 0 {
                self.results.len() - 1
            } else {
                self.current_result_idx - 1
            };
        }
    }

    pub fn result_info(&self) -> (usize, usize) {
        (self.current_result_idx + 1, self.results.len())
    }
}

pub struct AppState {
    pub nodes: Vec<Node>,
    pub scroll: usize,
    pub selected: usize,
    pub expanded: HashSet<usize>,
    pub file_path: String,
    pub mode: InputMode,
    pub search: SearchState,
    pub message: Option<String>,
    pub jump_input: String,
    /// Cached visible node IDs; invalidated on expand/collapse.
    visible_cache: Option<Vec<usize>>,
    /// Cached raw XML for the selected node offset.
    cached_raw_xml: Option<(u64, String)>,
}

impl AppState {
    pub fn new(nodes: Vec<Node>, file: String) -> Self {
        let mut expanded = HashSet::new();
        if !nodes.is_empty() {
            for node in &nodes {
                if node.depth < 2 {
                    expanded.insert(node.id);
                }
            }
        }
        Self {
            nodes,
            scroll: 0,
            selected: 0,
            expanded,
            file_path: file,
            mode: InputMode::Normal,
            search: SearchState::new(),
            message: None,
            jump_input: String::new(),
            visible_cache: None,
            cached_raw_xml: None,
        }
    }

    /// Invalidate the visible cache. Must be called after any expand/collapse change.
    fn invalidate_visible(&mut self) {
        self.visible_cache = None;
    }

    fn collect_visible_ids(&self, id: usize, result: &mut Vec<usize>) {
        if let Some(node) = self.nodes.get(id) {
            result.push(id);
            if self.expanded.contains(&id) {
                for &child_id in &node.children {
                    self.collect_visible_ids(child_id, result);
                }
            }
        }
    }

    pub fn get_visible_ids(&mut self) -> &[usize] {
        if self.visible_cache.is_none() {
            let mut visible = Vec::new();
            if !self.nodes.is_empty() {
                self.collect_visible_ids(0, &mut visible);
            }
            self.visible_cache = Some(visible);
        }
        self.visible_cache.as_ref().unwrap()
    }

    pub fn get_visible_nodes(&self) -> Vec<&Node> {
        // Use cached IDs if available (warmed by earlier &mut self calls).
        if let Some(ref cache) = self.visible_cache {
            return cache.iter().filter_map(|&id| self.nodes.get(id)).collect();
        }
        let mut visible = Vec::new();
        if !self.nodes.is_empty() {
            self.collect_visible(0, &mut visible);
        }
        visible
    }

    fn collect_visible<'a>(&'a self, id: usize, result: &mut Vec<&'a Node>) {
        if let Some(node) = self.nodes.get(id) {
            result.push(node);
            if self.expanded.contains(&id) {
                for &child_id in &node.children {
                    self.collect_visible(child_id, result);
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        let len = self.get_visible_ids().len();
        if self.selected + 1 < len {
            self.selected += 1;
            self.ensure_scroll();
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_scroll();
        }
    }

    pub fn page_down(&mut self) {
        let len = self.get_visible_ids().len();
        if len == 0 {
            return;
        }
        let jump = 15.min(len.saturating_sub(1));
        self.selected = (self.selected + jump).min(len - 1);
        self.ensure_scroll();
    }

    pub fn page_up(&mut self) {
        let jump = 15.min(self.selected);
        self.selected -= jump;
        self.ensure_scroll();
    }

    pub fn goto_top(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn goto_bottom(&mut self) {
        let len = self.get_visible_ids().len();
        self.selected = len.saturating_sub(1);
        self.ensure_scroll();
    }

    pub fn toggle_expand(&mut self) {
        let selected = self.selected;
        let node_id = {
            let visible = self.get_visible_ids();
            if selected < visible.len() {
                Some(visible[selected])
            } else {
                None
            }
        };
        if let Some(node_id) = node_id {
            if self.expanded.contains(&node_id) {
                self.expanded.remove(&node_id);
            } else {
                self.expanded.insert(node_id);
            }
            self.invalidate_visible();
        }
    }

    pub fn expand_all(&mut self) {
        for i in 0..self.nodes.len() {
            self.expanded.insert(self.nodes[i].id);
        }
        self.invalidate_visible();
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        if !self.nodes.is_empty() {
            self.expanded.insert(0);
        }
        self.selected = 0;
        self.scroll = 0;
        self.invalidate_visible();
    }

    pub fn get_children(&self, parent_id: usize) -> &[usize] {
        self.nodes
            .get(parent_id)
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
    }

    /// Adjust scroll to keep selection in view. Does NOT recompute visible list.
    fn ensure_scroll(&mut self) {
        if self.scroll > self.selected {
            self.scroll = self.selected;
        } else if self.selected - self.scroll > 20 {
            self.scroll = self.selected.saturating_sub(20);
        }
    }

    pub fn get_selected_node(&mut self) -> Option<usize> {
        let selected = self.selected;
        let visible = self.get_visible_ids();
        visible.get(selected).copied()
    }

    pub fn get_selected_node_ref(&self) -> Option<&Node> {
        // Use cached IDs if available to avoid re-traversal.
        if let Some(ref cache) = self.visible_cache {
            return cache.get(self.selected).and_then(|&id| self.nodes.get(id));
        }
        // Fallback: walk the tree.
        let mut visible_ids = Vec::new();
        if !self.nodes.is_empty() {
            self.collect_visible_ids(0, &mut visible_ids);
        }
        visible_ids
            .get(self.selected)
            .and_then(|&id| self.nodes.get(id))
    }

    pub fn get_selected_node_raw_xml(&mut self) -> &str {
        let offsets = self.get_selected_node().
            and_then(|id| self.nodes.get(id))
            .map(|n| (n.offset, n.end_offset));
        
        let Some((offset, end_offset)) = offsets else {
            return "";
        };
        
        // cache hit - same start offset means same node. 
        if matches!(&self.cached_raw_xml, Some((cached, _)) if *cached == offset) {
            return &self.cached_raw_xml.as_ref().unwrap().1;
        }
        
        let xml = raw_xml::read_chunk(&self.file_path, offset, end_offset);
        self.cached_raw_xml = Some((offset, xml));
        &self.cached_raw_xml.as_ref().unwrap().1
    }

    pub fn get_selected_node_attributes(&self) -> &[(String, String)] {
        self.get_selected_node_ref()
            .map(|n| n.attributes.as_slice())
            .unwrap_or(&[])
    }

    pub fn jump_to_node(&mut self, node_id: usize) -> bool {
        // First try without expanding.
        {
            let visible = self.get_visible_ids();
            if let Some(pos) = visible.iter().position(|&id| id == node_id) {
                self.selected = pos;
                self.ensure_scroll();
                return true;
            }
        }

        // Expand ancestors to make the node visible.
        let mut path_to_root = vec![node_id];
        let mut current = self.nodes.get(node_id).and_then(|n| n.parent);
        while let Some(parent_id) = current {
            path_to_root.push(parent_id);
            current = self.nodes.get(parent_id).and_then(|n| n.parent);
        }

        for &id in path_to_root.iter().rev() {
            self.expanded.insert(id);
        }
        self.invalidate_visible();

        let visible = self.get_visible_ids();
        if let Some(pos) = visible.iter().position(|&id| id == node_id) {
            self.selected = pos;
            self.ensure_scroll();
            return true;
        }
        false
    }

    pub fn jump_to_search_result(&mut self) {
        if let Some(node_id) = self.search.current_result() {
            self.jump_to_node(node_id);
        }
    }

    pub fn next_search_result(&mut self) {
        self.search.next_result();
        self.jump_to_search_result();
    }

    pub fn prev_search_result(&mut self) {
        self.search.prev_result();
        self.jump_to_search_result();
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn visible_count(&mut self) -> usize {
        self.get_visible_ids().len()
    }

    pub fn stats(&mut self) -> (usize, usize, usize) {
        let visible_len = self.get_visible_ids().len();
        (self.nodes.len(), visible_len, self.expanded.len())
    }
}
