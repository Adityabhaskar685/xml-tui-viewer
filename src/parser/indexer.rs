use anyhow::Result;
use memmap2::Mmap;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::BufReader;

use crate::parser::reader::Node;


pub fn build_index(path: &str) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let file_size = file.metadata()?.len();
    if file_size > 50 * 1024 * 1024 {
        build_index_mmap(path)
    } else {
        build_index_sequential(path)
    }
}

// Mmap-backed parse (large files)
//
// Identical logic to sequential but feeds the mmap slice directly to
// quick_xml, avoiding BufReader overhead and letting the OS page-cache
// stream the file at memory bandwidth.

pub fn build_index_mmap(path: &str) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    // Advise the OS to read-ahead sequentially.
    #[cfg(unix)]
    {
        use memmap2::Advice;
        let _ = mmap.advise(Advice::Sequential);
    }

    let mut reader = Reader::from_reader(mmap.as_ref());
    reader.trim_text(true);

    let mut buf    = Vec::with_capacity(256);
    let mut nodes: Vec<Node> = Vec::new();
    let mut depth  = 0u16;
    let mut parent_stack: Vec<(usize, u64)> = Vec::with_capacity(64);
    let mut id     = 0usize;

    loop {
        let offset = reader.buffer_position() as u64;
        
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let parent = parent_stack.last().map(|&(id, _)| id);
                let attributes = collect_attrs(&e);

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
                let end_offset = reader.buffer_position() as u64;
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let parent = parent_stack.last().map(|&(id, _)| id);
                let attributes = collect_attrs(&e);

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
                let end_offset = reader.buffer_position() as u64;
                if let Some((node_id, _)) = parent_stack.pop() {
                    nodes[node_id].end_offset = end_offset;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(nodes)
}


pub fn build_index_sequential(path: &str) -> Result<Vec<Node>> {
    let file = File::open(path)?;
    let mut reader = Reader::from_reader(BufReader::with_capacity(256 * 1024, file));
    reader.trim_text(true);

    let mut buf    = Vec::with_capacity(256);
    let mut nodes: Vec<Node> = Vec::new();
    let mut depth  = 0u16;
    let mut parent_stack: Vec<(usize, u64)> = Vec::with_capacity(64);
    let mut id     = 0usize;

    loop {
        let offset = reader.buffer_position() as u64;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag  = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let parent = parent_stack.last().map(|&(id, _)| id);
                let attributes = collect_attrs(&e);

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
                let end_offset = reader.buffer_position() as u64;
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let parent = parent_stack.last().map(|&(id, _)| id);
                let attributes = collect_attrs(&e);

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
                let end_offset = reader.buffer_position() as u64;
                if let Some((node_id, _)) = parent_stack.pop() {
                    nodes[node_id].end_offset = end_offset;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(nodes)
}


#[inline]
fn collect_attrs(e: &quick_xml::events::BytesStart<'_>) -> Vec<(String, String)> {
    e.attributes()
        .flatten()
        .map(|a| (
            String::from_utf8_lossy(a.key.as_ref()).into_owned(),
            String::from_utf8_lossy(&a.value).into_owned(),
        ))
        .collect()
}