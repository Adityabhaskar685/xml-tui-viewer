use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use rayon::prelude::*;
use std::fs::File;
use std::io::BufReader;

use crate::parser::reader::Node;


pub fn build_index(path: &str) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let file_size = file.metadata()?.len();

    if file_size > 100 * 1024 * 1024 {
        build_index_parallel(path)
    } else {
        build_index_sequential(path)
    }
}

pub fn build_index_sequential(path: &str) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut nodes: Vec<Node> = Vec::new();

    let mut depth = 0u16;
    let mut parent_stack: Vec<(usize, u64)> = Vec::new();
    let mut id = 0usize;

    loop {
        let offset = reader.buffer_position() as u64;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let parent = parent_stack.last().map(|&(id, _)| id);

                let mut attributes = Vec::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let value = String::from_utf8_lossy(&attr.value).to_string();
                    attributes.push((key, value));
                }

                nodes.push(Node {
                    id,
                    tag,
                    depth,
                    offset,
                    end_offset: 0, // will be filled on matching End
                    parent,
                    attributes,
                    children: Vec::new(),
                });

                if let Some(&(pid, _)) = parent_stack.last() {
                    if pid < nodes.len() {
                        nodes[pid].children.push(id);
                    }
                }

                parent_stack.push((id, offset));
                depth += 1;
                id += 1;
            }
            Ok(Event::Empty(e)) => {
                // Self-closing tag: start == end is already known right after reading.
                let end = reader.buffer_position() as u64;
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let parent = parent_stack.last().map(|&(id,_)| id);
                
                let mut attributes = Vec::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let value = String::from_utf8_lossy(attr.value.as_ref()).to_string();
                    attributes.push((key, value));
                }
                
                nodes.push(Node {
                    id,
                    tag,
                    depth,
                    offset,
                    end_offset: end,
                    parent,
                    attributes,
                    children: Vec::new()
                });
                
                if let Some(&(pid, _)) = parent_stack.last() {
                    nodes[pid].children.push(id);
                }
                
                id += 1;
            }
            Ok(Event::End(_)) => {
                let end = reader.buffer_position() as u64;
                if let Some((node_id, _)) = parent_stack.pop() {
                    nodes[node_id].end_offset = end;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parsing error: {}", e)),
            _ => {}
        }

        buf.clear();
    }

    Ok(nodes)
}


pub fn build_index_parallel(path: &str) -> Result<Vec<Node>> {
    let subtree_ranges = find_top_level_subtrees(path)?;
 
    let (root_offset, root_end, root_tag, root_attrs, children_ranges) = subtree_ranges;
     
    if children_ranges.is_empty() {
       return build_index_sequential(path);
    }
 
    let file_bytes = std::fs::read(path)?;
 
    let child_forests: Vec<Vec<Node>> = children_ranges
        .par_iter()
        .map(|&(start, end)| {
            let slice = &file_bytes[start..end];
            parse_subtree(slice, start as u64).unwrap_or_default()
        })
        .collect();
 
    let total: usize = child_forests.iter().map(|f| f.len()).sum();
    let mut nodes: Vec<Node> = Vec::with_capacity(1 + total);
 
    nodes.push(Node {
        id: 0,
        tag: root_tag,
        depth: 0,
        offset: root_offset,
        end_offset: root_end,
        parent: None,
        attributes: root_attrs,
        children: Vec::new(),
    });
 
    let mut id_base = 1usize; 
 
    for mut forest in child_forests {
        if forest.is_empty() {
            continue;
        }
 
        let subtree_root_global = id_base;
        nodes[0].children.push(subtree_root_global);
 
        for node in &mut forest {
            node.id += id_base;
            node.parent = match node.parent {
                None => Some(0),                      // subtree root → document root
                Some(p) => Some(p + id_base),         // inner node → shifted
            };
            for child_id in &mut node.children {
                *child_id += id_base;
            }
        }
 
        id_base += forest.len();
        nodes.extend(forest);
    }
 
    Ok(nodes)
}
 
/// Returns (root_offset, root_end_offset, root_tag, root_attrs,
///          Vec<(child_start_byte, child_end_byte)>)
/// by doing one fast sequential scan.
fn find_top_level_subtrees(
    path: &str,
) -> Result<(u64, u64, String, Vec<(String, String)>, Vec<(usize, usize)>)> {
    let file_bytes = std::fs::read(path)?;
    let mut reader = Reader::from_reader(file_bytes.as_slice());
    reader.trim_text(true);
 
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut root_offset = 0u64;
    let mut root_tag = String::new();
    let mut root_attrs: Vec<(String, String)> = Vec::new();
    let mut root_end = 0u64;
    let mut children: Vec<(usize, usize)> = Vec::new();
    let mut child_start: Option<usize> = None;
 
    loop {
        let pos = reader.buffer_position();
 
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                if depth == 1 {
                    // This is the document root.
                    root_offset = pos as u64;
                    root_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    for attr in e.attributes().flatten() {
                        root_attrs.push((
                            String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                            String::from_utf8_lossy(&attr.value).to_string(),
                        ));
                    }
                } else if depth == 2 {
                    // Start of a direct child subtree.
                    child_start = Some(pos);
                }
            }
 
            Ok(Event::Empty(e)) => {
                if depth == 1 {
                    // Self-closing direct child of root.
                    let end = reader.buffer_position();
                    children.push((pos, end));
                }
                // depth == 0 means self-closing root (degenerate doc) — ignore.
                let _ = e;
            }
 
            Ok(Event::End(_)) => {
                if depth == 2 {
                    // Closing tag of a direct child subtree.
                    if let Some(start) = child_start.take() {
                        children.push((start, reader.buffer_position()));
                    }
                } else if depth == 1 {
                    root_end = reader.buffer_position() as u64;
                }
                depth = depth.saturating_sub(1);
            }
 
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML scan error: {}", e)),
            _ => {}
        }
 
        buf.clear();
    }
 
    Ok((root_offset, root_end, root_tag, root_attrs, children))
}
 
/// Parse a self-contained XML subtree from `data` (a byte slice starting at
/// `base_offset` in the original file). Returns nodes with local IDs starting
/// at 0; the caller re-numbers them.
fn parse_subtree(data: &[u8], base_offset: u64) -> Result<Vec<Node>> {
    let mut reader = Reader::from_reader(data);
    reader.trim_text(true);
 
    let mut buf = Vec::new();
    let mut nodes: Vec<Node> = Vec::new();
    let mut depth = 0u16;
    let mut parent_stack: Vec<(usize, u64)> = Vec::new();
    let mut id = 0usize;
 
    loop {
        let local_pos = reader.buffer_position() as u64;
        let offset = base_offset + local_pos;
 
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let parent = parent_stack.last().map(|&(id, _)| id);
 
                let mut attributes = Vec::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let value = String::from_utf8_lossy(&attr.value).to_string();
                    attributes.push((key, value));
                }
 
                nodes.push(Node {
                    id,
                    tag,
                    depth,
                    offset,
                    end_offset: 0,
                    parent,
                    attributes,
                    children: Vec::new(),
                });
 
                if let Some(&(pid, _)) = parent_stack.last() {
                    nodes[pid].children.push(id);
                }
 
                parent_stack.push((id, offset));
                depth += 1;
                id += 1;
            }
 
            Ok(Event::Empty(e)) => {
                let end_offset = base_offset + reader.buffer_position() as u64;
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let parent = parent_stack.last().map(|&(id, _)| id);
 
                let mut attributes = Vec::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let value = String::from_utf8_lossy(&attr.value).to_string();
                    attributes.push((key, value));
                }
 
                nodes.push(Node {
                    id,
                    tag,
                    depth,
                    offset,
                    end_offset,
                    parent,
                    attributes,
                    children: Vec::new(),
                });
 
                if let Some(&(pid, _)) = parent_stack.last() {
                    nodes[pid].children.push(id);
                }
 
                id += 1;
            }
 
            Ok(Event::End(_)) => {
                let end_offset = base_offset + reader.buffer_position() as u64;
                if let Some((node_id, _)) = parent_stack.pop() {
                    nodes[node_id].end_offset = end_offset;
                }
                depth = depth.saturating_sub(1);
            }
 
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
 
        buf.clear();
    }
 
    Ok(nodes)
}


