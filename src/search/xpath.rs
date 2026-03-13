use crate::parser::reader::Node;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum XPathToken {
    Root,
    Child(String),
    Wildcard,
    Attribute(String),
    Predicate(Box<XPathPredicate>),
    Axis(XPathAxis),
}

#[derive(Debug, Clone)]
pub enum XPathAxis {
    Child,
    Descendant,
    Parent,
    Ancestor,
    FollowingSibling,
    PrecedingSibling,
}

#[derive(Debug, Clone)]
pub enum XPathPredicate {
    Position(usize),
    PositionLast,
    AttributeExists(String),
    AttributeEquals(String, String),
    AttributeContains(String, String),
    And(Box<XPathPredicate>, Box<XPathPredicate>),
    Or(Box<XPathPredicate>, Box<XPathPredicate>),
}

#[derive(Debug, Clone)]
pub struct XPathExpr {
    tokens: Vec<XPathToken>,
}

pub struct XPathEngine<'a> {
    nodes: &'a [Node],
}

impl<'a> XPathEngine<'a> {
    pub fn new(nodes: &'a [Node]) -> Self {
        Self { nodes }
    }

    pub fn evaluate(&self, expr: &str) -> Vec<usize> {
        let tokens = match self.parse(expr) {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        self.execute(&tokens)
    }

    fn parse(&self, expr: &str) -> Result<Vec<XPathToken>, String> {
        let mut tokens = Vec::new();
        let expr = expr.trim();

        if expr.is_empty() {
            return Err("Empty expression".to_string());
        }

        let mut chars = expr.chars().peekable();
        let mut first = true;

        while let Some(&ch) = chars.peek() {
            match ch {
                '/' => {
                    chars.next();
                    if first {
                        tokens.push(XPathToken::Root);
                        first = false;
                    }
                    let mut segment = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '/' || c == '[' || c == '@' {
                            break;
                        }
                        segment.push(c);
                        chars.next();
                    }
                    let segment = segment.trim();
                    if !segment.is_empty() {
                        if segment == "*" {
                            tokens.push(XPathToken::Wildcard);
                        } else {
                            tokens.push(XPathToken::Child(segment.to_string()));
                        }
                    }
                }
                '@' => {
                    chars.next();
                    let mut attr_name = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '=' || c == ']' || c == '\'' || c == '"' {
                            break;
                        }
                        attr_name.push(c);
                        chars.next();
                    }
                    tokens.push(XPathToken::Attribute(attr_name.trim().to_string()));
                }
                '[' => {
                    chars.next();
                    let predicate = self.parse_predicate(&mut chars)?;
                    tokens.push(XPathToken::Predicate(Box::new(predicate)));
                }
                '*' => {
                    chars.next();
                    tokens.push(XPathToken::Wildcard);
                }
                _ => {
                    let mut name = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '/' || c == '[' || c == '@' || c.is_whitespace() {
                            break;
                        }
                        name.push(c);
                        chars.next();
                    }
                    if !name.is_empty() {
                        tokens.push(XPathToken::Child(name));
                    }
                }
            }
        }

        Ok(tokens)
    }

    fn parse_predicate(
        &self,
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Result<XPathPredicate, String> {
        let mut content = String::new();
        let mut bracket_count = 1;

        while bracket_count > 0 {
            match chars.next() {
                Some('[') => {
                    bracket_count += 1;
                    content.push('[');
                }
                Some(']') => {
                    bracket_count -= 1;
                    if bracket_count > 0 {
                        content.push(']');
                    }
                }
                Some(c) => content.push(c),
                None => return Err("Unclosed predicate".to_string()),
            }
        }

        let content = content.trim();

        if content == "last()" {
            return Ok(XPathPredicate::PositionLast);
        }

        if let Ok(pos) = content.parse::<usize>() {
            return Ok(XPathPredicate::Position(pos));
        }

        if content.starts_with('@') {
            let parts: Vec<&str> = content.splitn(2, '=').collect();
            let attr_name = parts[0][1..].trim().to_string();

            if parts.len() == 1 {
                return Ok(XPathPredicate::AttributeExists(attr_name));
            }

            let value = parts[1]
                .trim()
                .trim_matches(|c| c == '\'' || c == '"')
                .to_string();

            if value.starts_with("contains(") {
                let inner = value.trim_start_matches("contains(").trim_end_matches(')');
                let inner_parts: Vec<&str> = inner.split(',').collect();
                if inner_parts.len() == 2 {
                    let check_attr = inner_parts[0].trim().trim_start_matches('@').to_string();
                    let check_val = inner_parts[1]
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"')
                        .to_string();
                    return Ok(XPathPredicate::AttributeContains(check_attr, check_val));
                }
            }

            return Ok(XPathPredicate::AttributeEquals(attr_name, value));
        }

        Err(format!("Invalid predicate: {}", content))
    }

    fn execute(&self, tokens: &[XPathToken]) -> Vec<usize> {
        if tokens.is_empty() {
            return vec![];
        }

        let mut current: HashSet<usize> = (0..self.nodes.len()).collect();
        let mut is_root_context = true;

        for token in tokens {
            current = match token {
                XPathToken::Root => {
                    is_root_context = true;
                    self.nodes
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| n.parent.is_none() || n.depth == 0)
                        .map(|(i, _)| i)
                        .collect()
                }
                XPathToken::Child(name) => {
                    is_root_context = false;
                    current
                        .iter()
                        .filter(|&&idx| self.nodes.get(idx).map_or(false, |n| n.tag == *name))
                        .copied()
                        .collect()
                }
                XPathToken::Wildcard => {
                    is_root_context = false;
                    current
                }
                XPathToken::Attribute(name) => current
                    .iter()
                    .filter(|&&idx| {
                        self.nodes
                            .get(idx)
                            .map_or(false, |n| n.attributes.iter().any(|(k, _)| k == name))
                    })
                    .copied()
                    .collect(),
                XPathToken::Predicate(pred) => self.apply_predicate(&current, pred),
                XPathToken::Axis(axis) => self.apply_axis(&current, axis, is_root_context),
            };
        }

        let mut result: Vec<usize> = current.into_iter().collect();
        result.sort_by_key(|&idx| self.nodes.get(idx).map_or(0, |n| n.offset));
        result
    }

    fn apply_predicate(&self, nodes: &HashSet<usize>, pred: &XPathPredicate) -> HashSet<usize> {
        match pred {
            XPathPredicate::Position(pos) => {
                let mut sorted: Vec<usize> = nodes.iter().copied().collect();
                sorted.sort_by_key(|&idx| self.nodes.get(idx).map_or(0, |n| n.offset));
                sorted.into_iter().skip(pos - 1).take(1).collect()
            }
            XPathPredicate::PositionLast => {
                let mut sorted: Vec<usize> = nodes.iter().copied().collect();
                sorted.sort_by_key(|&idx| self.nodes.get(idx).map_or(0, |n| n.offset));
                sorted.into_iter().last().into_iter().collect()
            }
            XPathPredicate::AttributeExists(name) => nodes
                .iter()
                .filter(|&&idx| {
                    self.nodes
                        .get(idx)
                        .map_or(false, |n| n.attributes.iter().any(|(k, _)| k == name))
                })
                .copied()
                .collect(),
            XPathPredicate::AttributeEquals(name, value) => nodes
                .iter()
                .filter(|&&idx| {
                    self.nodes.get(idx).map_or(false, |n| {
                        n.attributes.iter().any(|(k, v)| k == name && v == value)
                    })
                })
                .copied()
                .collect(),
            XPathPredicate::AttributeContains(name, value) => nodes
                .iter()
                .filter(|&&idx| {
                    self.nodes.get(idx).map_or(false, |n| {
                        n.attributes
                            .iter()
                            .any(|(k, v)| k == name && v.contains(value))
                    })
                })
                .copied()
                .collect(),
            XPathPredicate::And(left, right) => {
                let left_result = self.apply_predicate(nodes, left);
                let right_result = self.apply_predicate(nodes, right);
                left_result.intersection(&right_result).copied().collect()
            }
            XPathPredicate::Or(left, right) => {
                let left_result = self.apply_predicate(nodes, left);
                let right_result = self.apply_predicate(nodes, right);
                left_result.union(&right_result).copied().collect()
            }
        }
    }

    fn apply_axis(
        &self,
        nodes: &HashSet<usize>,
        axis: &XPathAxis,
        _is_root: bool,
    ) -> HashSet<usize> {
        let mut result = HashSet::new();

        for &idx in nodes {
            let Some(node) = self.nodes.get(idx) else {
                continue;
            };

            match axis {
                XPathAxis::Child => {
                    for &child_idx in &self.nodes[idx].children {
                        result.insert(child_idx);
                    }
                }
                XPathAxis::Descendant => {
                    self.collect_descendants(idx, &mut result);
                }
                XPathAxis::Parent => {
                    if let Some(parent_idx) = node.parent {
                        result.insert(parent_idx);
                    }
                }
                XPathAxis::Ancestor => {
                    self.collect_ancestors(idx, &mut result);
                }
                XPathAxis::FollowingSibling => {
                    if let Some(parent_idx) = node.parent {
                        if let Some(parent) = self.nodes.get(parent_idx) {
                            for &sib_idx in &parent.children {
                                if self.nodes[sib_idx].offset > node.offset {
                                    result.insert(sib_idx);
                                }
                            }
                        }
                    }
                }
                XPathAxis::PrecedingSibling => {
                    if let Some(parent_idx) = node.parent {
                        if let Some(parent) = self.nodes.get(parent_idx) {
                            for &sib_idx in &parent.children {
                                if self.nodes[sib_idx].offset < node.offset {
                                    result.insert(sib_idx);
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }

    fn collect_descendants(&self, idx: usize, result: &mut HashSet<usize>) {
        let Some(node) = self.nodes.get(idx) else {
            return;
        };
        for &child_idx in &node.children {
            result.insert(child_idx);
            self.collect_descendants(child_idx, result);
        }
    }

    fn collect_ancestors(&self, idx: usize, result: &mut HashSet<usize>) {
        let Some(node) = self.nodes.get(idx) else {
            return;
        };
        if let Some(parent_idx) = node.parent {
            result.insert(parent_idx);
            self.collect_ancestors(parent_idx, result);
        }
    }
}

pub fn xpath_search(nodes: &[Node], expr: &str) -> Vec<usize> {
    let engine = XPathEngine::new(nodes);
    engine.evaluate(expr)
}
