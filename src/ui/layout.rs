use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::state::{AppState, InputMode};

pub fn draw_ui(frame: &mut Frame, app: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(50),
            Constraint::Percentage(30),
        ])
        .split(chunks[0]);

    render_raw_xml(frame, main_chunks[1], app);
    render_tree(frame, main_chunks[0], app);
    render_attributes(frame, main_chunks[2], app);
    render_status_bar(frame, chunks[1], app);

    if app.mode == InputMode::Help {
        render_help_overlay(frame);
    }
}

fn render_tree(frame: &mut Frame, area: Rect, app: &AppState) {
    // Note: uses get_visible_nodes which doesn't need &mut self.
    let visible = app.get_visible_nodes();
    let height = area.height.saturating_sub(2) as usize;

    let start = app.scroll;
    let end = (start + height).min(visible.len());

    let visible_slice: Vec<_> = visible[start..end].iter().collect();

    let search_results: std::collections::HashSet<usize> =
        app.search.results.iter().copied().collect();

    let items: Vec<ListItem> = visible_slice
        .iter()
        .enumerate()
        .map(|(_idx, n)| {
            let indent = "  ".repeat(n.depth as usize);
            let prefix = if app.expanded.contains(&n.id) {
                "▼ "
            } else if n.has_children() {
                "▶ "
            } else {
                "  "
            };

            let is_search_match = search_results.contains(&n.id);

            let style = if is_search_match {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let tag_span = Span::styled(&n.tag, style);

            let attr_hint = if !n.attributes.is_empty() {
                format!(" [{}]", n.attributes.len())
            } else {
                String::new()
            };

            ListItem::new(Line::from(vec![
                Span::raw(format!("{}{}", indent, prefix)),
                tag_span,
                Span::raw(attr_hint),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected.saturating_sub(app.scroll)));

    let title = format!("XML Tree ({}/{})", visible.len(), app.node_count());
    let tree = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .scroll_padding(2);

    frame.render_stateful_widget(tree, area, &mut list_state);
}

fn render_raw_xml(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let raw_xml = app.get_selected_node_raw_xml();
    let lines: Vec<Line> = raw_xml
        .lines()
        .map(|l| Line::raw(l.to_string()))
        .collect();

    let paragraph = Paragraph::new(lines)
            .block(Block::default().title("Raw XML").borders(Borders::ALL))
            .wrap(Wrap {trim: false });  // enable wraping to text

    frame.render_widget(paragraph, area);
}

fn render_attributes(frame: &mut Frame, area: Rect, app: &AppState) {
    let attrs = app.get_selected_node_attributes();

    let mut lines = vec![
        Line::from(Span::styled(
            "Attributes",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
    ];

    if attrs.is_empty() {
        lines.push(Line::raw("No attributes"));
    } else {
        for (k, v) in attrs {
            lines.push(Line::from(vec![
                Span::styled(k.as_str(), Style::default().fg(Color::Cyan)),
                Span::raw(" = "),
                Span::styled(v.as_str(), Style::default().fg(Color::Green)),
            ]));
        }
    }

    let node = app.get_selected_node_ref();
    if let Some(n) = node {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Node Info",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(format!("ID: {}", n.id)));
        lines.push(Line::raw(format!("Depth: {}", n.depth)));
        lines.push(Line::raw(format!("Offset: {}", n.offset)));
        lines.push(Line::raw(format!("Children: {}", n.children.len())));
    }

    let paragraph =
        Paragraph::new(lines).block(Block::default().title("Details").borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &AppState) {
    // Use node_count and expanded.len() directly to avoid needing &mut self.
    let total = app.node_count();
    let expanded = app.expanded.len();

    let mode_text = match app.mode {
        InputMode::Normal => "NORMAL",
        InputMode::SearchRegex => "REGEX",
        InputMode::SearchFuzzy => "FUZZY",
        InputMode::SearchXPath => "XPATH",
        InputMode::JumpToLine => "JUMP",
        InputMode::Help => "HELP",
    };

    let mode_style = match app.mode {
        InputMode::Normal => Style::default().fg(Color::Green),
        _ => Style::default().fg(Color::Yellow),
    };

    let mut spans = vec![
        Span::styled(format!(" {} ", mode_text), mode_style),
        Span::raw(" | "),
    ];

    if let Some(ref msg) = app.message {
        spans.push(Span::styled(
            msg.clone(),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::raw(" | "));
    }

    if !app.search.results.is_empty() {
        let (current, total) = app.search.result_info();
        spans.push(Span::styled(
            format!("Match {}/{}", current, total),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::raw(" | "));
    }

    spans.push(Span::raw(format!(
        "{} nodes | {} expanded",
        total, expanded
    )));

    let status = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(status, area);
}

fn render_help_overlay(frame: &mut Frame) {
    let area = centered_rect(60, 70, frame.area());

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("j/k, ↑/↓    ", Style::default().fg(Color::Cyan)),
            Span::raw("Navigate up/down"),
        ]),
        Line::from(vec![
            Span::styled("h/l, ←/→    ", Style::default().fg(Color::Cyan)),
            Span::raw("Collapse/Expand"),
        ]),
        Line::from(vec![
            Span::styled("PgUp/PgDn   ", Style::default().fg(Color::Cyan)),
            Span::raw("Page navigation"),
        ]),
        Line::from(vec![
            Span::styled("Home/End    ", Style::default().fg(Color::Cyan)),
            Span::raw("Jump to top/bottom"),
        ]),
        Line::raw(""),
        Line::styled("Search", Style::default().add_modifier(Modifier::BOLD)),
        Line::from(vec![
            Span::styled("/           ", Style::default().fg(Color::Cyan)),
            Span::raw("Regex search"),
        ]),
        Line::from(vec![
            Span::styled("f           ", Style::default().fg(Color::Cyan)),
            Span::raw("Fuzzy search"),
        ]),
        Line::from(vec![
            Span::styled("x           ", Style::default().fg(Color::Cyan)),
            Span::raw("XPath query"),
        ]),
        Line::from(vec![
            Span::styled("n/p         ", Style::default().fg(Color::Cyan)),
            Span::raw("Next/Prev search result"),
        ]),
        Line::raw(""),
        Line::styled("Actions", Style::default().add_modifier(Modifier::BOLD)),
        Line::from(vec![
            Span::styled("g           ", Style::default().fg(Color::Cyan)),
            Span::raw("Jump to node by ID"),
        ]),
        Line::from(vec![
            Span::styled("e           ", Style::default().fg(Color::Cyan)),
            Span::raw("Expand all"),
        ]),
        Line::from(vec![
            Span::styled("c           ", Style::default().fg(Color::Cyan)),
            Span::raw("Collapse all"),
        ]),
        Line::from(vec![
            Span::styled("? / H       ", Style::default().fg(Color::Cyan)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("q           ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ]),
        Line::raw(""),
        Line::styled(
            "Press any key to close",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
