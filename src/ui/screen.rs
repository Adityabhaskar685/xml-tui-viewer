use std::{env, io, path::{Path, PathBuf}, time::Instant};

use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};


use crate::{app::state::AppState, parser::indexer::build_index};

pub enum Screen {
    Browser(FileBrowser),
    Viewer(AppState),
    Loading(PathBuf)
}

pub struct FileBrowser {
    pub current_dir: PathBuf,
    pub entries: Vec<PathBuf>,
    pub selected: usize,
    pub scroll: usize,
    pub message: Option<String>
}

impl FileBrowser {
    pub fn new(start: PathBuf) -> Self {
        let mut fb = Self {
            current_dir: start, 
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            message: None
        };
        fb.refresh();
        fb
    }
    
    pub fn refresh(&mut self) {
        self.entries.clear();
        self.selected = 0;
        self.scroll = 0;
        
        if self.current_dir.parent().is_some() {
            self.entries.push(self.current_dir.join(".."));
        }
        
        let mut dirs : Vec<PathBuf> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();
        
        if let Ok(rd) = std::fs::read_dir(&self.current_dir) {
            for entry in rd.flatten() {
                let path = entry.path(); 
                if path.is_dir() {
                    dirs.push(path);
                } else if is_xml(&path) {
                    files.push(path);
                }
            }
        }
        
        dirs.sort();
        files.sort();
        self.entries.extend(dirs);
        self.entries.extend(files);
    }
    
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.clamp_scroll();
        }
    }
    
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.clamp_scroll();
        }
    }
    
    fn clamp_scroll(&mut self) {
        if self.scroll > self.selected {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + 30 {
            self.scroll = self.selected.saturating_sub(29);
        }
    }
    
    pub fn go_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh();
        }
    }
    
    /// Returns `Some(path)` when user selects an XML file.
    pub fn enter(&mut self) -> Option<PathBuf> {
        let path = self.entries.get(self.selected)?.clone();
        
        if path.ends_with("..") {
            self.go_up();
            return None;
        }
        
        if path.is_dir() {
            self.current_dir = path;
            self.refresh();
            return None;
        } else if is_xml(&path) {
            Some(path)
        } else {
            self.message = Some("Not an XML file".to_string());
            None
        }
    }
    
    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(1)
            ])
            .split(area);
        
        let height = chunks[0].height.saturating_sub(2) as usize;
        let end = (self.scroll + height).min(self.entries.len());
        
        let items: Vec<ListItem> = self.entries[self.scroll..end]
            .iter()
            .map(|p| {
                let is_dotdot = p.ends_with("..");
                let name =if is_dotdot {
                    "../".to_string()
                }  else if p.is_dir() {
                    format!("{}/", p.file_name().unwrap_or_default().to_string_lossy())
                } else {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                };
                
                let style = if p.is_dir() || is_dotdot {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                
                ListItem::new(Line::from(Span::styled(name, style)))
            })
            .collect();
        
        let mut list_state = ListState::default();
        list_state.select(Some(self.selected.saturating_sub(self.scroll)));
        
        let title = format!(" Browse: {}", self.current_dir.display());
        let list = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            );
        
        frame.render_stateful_widget(list, chunks[0], &mut list_state);
        
        let hint = self
            .message
            .clone()
            .unwrap_or_else(|| "↑/↓  j/k  navigate    Enter/l  open    h  parent dir    q  quit".to_string());
        
        let status = Paragraph::new(Line::from(Span::raw(format!(" {}", hint))))
            .style(Style::default().bg(Color::DarkGray));
        frame.render_widget(status, chunks[1]);
            
        
    }
}

fn is_xml(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("xml" | "XML" | "xsl" | "XSL" | "svg" | "xhmtl" | "res" | "atom")
    )
}

/// Draw loading screen while indexing a xml file.
fn draw_loading(frame: &mut Frame, path: &Path, size_mb: f64) {
    let msg = format!(
        " Indexin {} ({:.2} MB) ..",
        path.file_name().unwrap_or_default().to_string_lossy(),
        size_mb
    );
    
    let para = Paragraph::new(Line::from(Span::styled (
        msg,
        Style::default().fg(Color::Yellow)
    )))
    .block(Block::default().borders(Borders::ALL).title(" Loading "));
    let popup = centered_rect(60, 20, frame.area());
    frame.render_widget(para, popup);
}

/// Consume a `Loading` screen, index the file, and return a `Viewer`.
/// Falls back to `Browser` on error. 
pub fn resolve_loading(
    path: PathBuf,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>
) -> Result<Screen> {
    
    if !path.exists() {
        let start = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut fb = FileBrowser::new(start);
        fb.message = Some(format!("File not found: {}", path.display()));
        return Ok(Screen::Browser(fb));
    }
    
    let size_mb = std::fs::metadata(&path)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0);
    
    terminal.draw(|f| draw_loading(f, &path, size_mb))?;
    
    let file_str = path.to_string_lossy().to_string();
    let t0 = Instant::now();
    
    match build_index(&file_str) {
        Ok(nodex) => {
            let elapsed = t0.elapsed();
            let mut app = AppState::new(nodex, file_str.clone());
            app.message = Some(format!(
                "Loaded {} ({:.2}s) -b: browser ?: help",
                path.file_name().unwrap_or_default().to_string_lossy(),
                elapsed.as_secs_f64(),
            ));
            Ok(Screen::Viewer(app))
        } 
        Err(e) => {
            let start = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let mut fb = FileBrowser::new(start);
            fb.message = Some(format!("Error laoding file: {}", e));
            Ok(Screen::Browser(fb))
        }
    }
}

fn centered_rect(percent_x: u16, percent_y : u16, r: Rect) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2) 
        ])
        .split(r);
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2) 
        ])
        .split(popup[1])[1]
}