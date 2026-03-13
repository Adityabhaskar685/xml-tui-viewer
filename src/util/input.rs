use crate::app::state::{AppState, InputMode};
use crate::search::{fuzzy_search, regex_search, xpath_search};
use crossterm::event::{Event, KeyCode, KeyEventKind};

pub fn handle_input(app: &mut AppState, event: Event) -> bool {
    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        match app.mode {
            InputMode::Normal => handle_normal_mode(app, key),
            InputMode::SearchRegex => handle_search_input(app, key, 'r'),
            InputMode::SearchFuzzy => handle_search_input(app, key, 'f'),
            InputMode::SearchXPath => handle_search_input(app, key, 'x'),
            InputMode::JumpToLine => handle_jump_input(app, key),
            InputMode::Help => handle_help_mode(app, key),
        }
    } else {
        false
    }
}

fn handle_normal_mode(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') => return true,
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::PageDown => app.page_down(),
        KeyCode::PageUp => app.page_up(),
        KeyCode::Home => app.goto_top(),
        KeyCode::End => app.goto_bottom(),
        KeyCode::Right | KeyCode::Char('l') => app.toggle_expand(),
        KeyCode::Left | KeyCode::Char('h') => app.toggle_expand(),
        KeyCode::Char('/') => {
            app.mode = InputMode::SearchRegex;
            app.search.query.clear();
            app.message = Some("Regex search:".to_string());
        }
        KeyCode::Char('f') => {
            app.mode = InputMode::SearchFuzzy;
            app.search.query.clear();
            app.message = Some("Fuzzy search:".to_string());
        }
        KeyCode::Char('x') => {
            app.mode = InputMode::SearchXPath;
            app.search.query.clear();
            app.message = Some("XPath:".to_string());
        }
        KeyCode::Char('g') => {
            app.mode = InputMode::JumpToLine;
            app.jump_input.clear();
            app.message = Some("Jump to node #:".to_string());
        }
        KeyCode::Char('?') | KeyCode::Char('H') => {
            app.mode = InputMode::Help;
        }
        KeyCode::Char('e') => app.expand_all(),
        KeyCode::Char('c') => app.collapse_all(),
        KeyCode::Char('n') => {
            if !app.search.results.is_empty() {
                app.next_search_result();
            }
        }
        KeyCode::Char('p') => {
            if !app.search.results.is_empty() {
                app.prev_search_result();
            }
        }
        KeyCode::Enter => {
            if !app.search.results.is_empty() {
                app.jump_to_search_result();
            }
        }
        _ => {}
    }
    false
}

fn handle_search_input(app: &mut AppState, key: crossterm::event::KeyEvent, mode: char) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.search.query.clear();
            app.search.results.clear();
            app.message = None;
        }
        KeyCode::Enter => {
            if !app.search.query.is_empty() {
                execute_search(app, mode);
                if !app.search.results.is_empty() {
                    app.jump_to_search_result();
                    let (current, total) = app.search.result_info();
                    app.message = Some(format!("Found {} matches (showing {})", total, current));
                } else {
                    app.message = Some("No matches found".to_string());
                }
            }
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.search.query.pop();
            update_search_hint(app, mode);
        }
        KeyCode::Char(c) => {
            app.search.query.push(c);
            update_search_hint(app, mode);
        }
        _ => {}
    }
    false
}

fn update_search_hint(app: &mut AppState, mode: char) {
    let prefix = match mode {
        'r' => "Regex:",
        'f' => "Fuzzy:",
        'x' => "XPath:",
        _ => "Search:",
    };
    app.message = Some(format!("{} {}", prefix, app.search.query));
}

fn execute_search(app: &mut AppState, mode: char) {
    app.search.results = match mode {
        'r' => regex_search(&app.nodes, &app.search.query),
        'f' => fuzzy_search(&app.nodes, &app.search.query),
        'x' => xpath_search(&app.nodes, &app.search.query),
        _ => vec![],
    };
    app.search.current_result_idx = 0;
}

fn handle_jump_input(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.mode = InputMode::Normal;
            app.jump_input.clear();
            app.message = None;
        }
        KeyCode::Enter => {
            if let Ok(node_id) = app.jump_input.parse::<usize>() {
                if node_id < app.nodes.len() {
                    app.jump_to_node(node_id);
                    app.message = Some(format!("Jumped to node {}", node_id));
                } else {
                    app.message = Some(format!(
                        "Node {} out of range (0-{})",
                        node_id,
                        app.nodes.len() - 1
                    ));
                }
            }
            app.mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.jump_input.pop();
            app.message = Some(format!("Jump to node #: {}", app.jump_input));
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            app.jump_input.push(c);
            app.message = Some(format!("Jump to node #: {}", app.jump_input));
        }
        _ => {}
    }
    false
}

fn handle_help_mode(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
            app.mode = InputMode::Normal;
        }
        _ => {}
    }
    false
}
