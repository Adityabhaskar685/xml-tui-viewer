mod app;
mod parser;
mod search;
mod ui;
mod util;
mod viewer;

use anyhow::Result;
use crossterm::{
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use std::{env, path::PathBuf};
use std::io;
use util::input::handle_input;

use crate::ui::screen::{FileBrowser, Screen, resolve_loading};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;
    
    // Start in browser, or jump straight to loading if a path was given.
    let mut screen  = if args.len() >= 2 {
        resolve_loading(PathBuf::from(&args[1]), &mut terminal)?
    } else {
        let start = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Screen::Browser(FileBrowser::new(start))
    };
    
    loop {
        match &mut screen {
            // Draw
            Screen::Browser(fb) => {
                terminal.draw(|f| fb.draw(f))?;
            }
            Screen::Viewer(app) => {
                terminal.draw(|f| ui::layout::draw_ui(f, app))?;
            }
            Screen::Loading(path) => {
                // shouldn't normally be reached in the loop body, but handling 
                // it safely resolving it immediately.
                let path = path.clone();
                screen = resolve_loading(path, &mut terminal)?;
                continue;
            }
        }
        
        // Input
        let event = read()?;
        let Event::Key(KeyEvent { code, modifiers, .. }) = event else {
            continue;
        };
        
        match &mut screen {
            Screen::Browser(fb) => {
                fb.message = None;
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up | KeyCode::Char('k') => fb.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => fb.move_down(),
                    KeyCode::Left | KeyCode::Char('h') => fb.go_up(),
                    KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') =>  {
                        if let Some(path) = fb.enter() {
                            screen = resolve_loading(path, &mut terminal)?
                        }
                    }
                    _ => {}
                }
            }
            Screen::Viewer(app) => {
                let is_normal = app.mode == app::state::InputMode::Normal;
                
                // 'b' in normal mode -> back to the browser, starting from the 
                // directory that contains the currently-open file.
                if is_normal && code == KeyCode::Char('b') {
                    let start = PathBuf::from(&app.file_path)
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
                    
                    screen = Screen::Browser(FileBrowser::new(start));
                    continue;
                }
                
                // Reconstruct the full events so handle_input recevies it unchanges.
                let full_event = Event::Key(KeyEvent::new(code, modifiers));
                if handle_input(app, full_event) {
                    break; // 'q' quits the whole app
                }
            }
            Screen::Loading(_) => unreachable!()
        }
    }
 
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    disable_raw_mode()?;

    Ok(())
}
