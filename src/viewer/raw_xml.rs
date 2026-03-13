use std::fs::File;
use std::io::{Read, Seek, SeekFrom};


/// Maximum bytes displayed in the raw XML panel.
/// Elements larger than this are truncated with a note.
const DISPLAY_CAP: u64 = 4096;

pub fn read_chunk(path: &str, offset: u64, end_offset: u64) -> String {
    let byte_len = end_offset.saturating_sub(offset);
    if byte_len == 0 {
        return String::new();
    }
    
    let read_len = byte_len.min(DISPLAY_CAP) as usize;
    let truncated = byte_len > DISPLAY_CAP;
    
    let Ok(mut file) = File::open(path) else {
        return String::from("<!-- count not open file -->");
    };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return String::from("<!-- seek failed -->");
    }
   
    let mut buf = vec![0u8; read_len];
    let n = file.read(&mut buf).unwrap_or(0);
    let mut result = String::from_utf8_lossy(&buf[..n]).into_owned();
    
    if truncated {
        result.push_str(&format!(
           "\n\n<!-- ... truncated ({} bytes total, showing first {}) -->",
           byte_len, DISPLAY_CAP
        ));
    }
    
    result
}
