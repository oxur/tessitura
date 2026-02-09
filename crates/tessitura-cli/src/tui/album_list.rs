use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use super::App;

/// Render the album list view.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title bar
            Constraint::Min(5),    // Album table
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    render_title(frame, app, chunks[0]);
    render_table(frame, app, chunks[1]);
    render_help(frame, chunks[2]);
}

fn render_title(frame: &mut Frame, app: &App, area: Rect) {
    let pending_count = app.albums.len();
    let title = Paragraph::new(format!(
        "Albums Awaiting Review    {} pending",
        pending_count
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, area);
}

fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("#").style(Style::default().fg(Color::DarkGray)),
        Cell::from("Album").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Artist"),
        Cell::from("Tracks"),
        Cell::from("Conflicts"),
    ])
    .height(1);

    // Calculate visible range based on viewport
    // area.height - 2 for borders - 1 for header
    let viewport_height = (area.height.saturating_sub(3)) as usize;
    let visible_start = app.album_list_offset;
    let visible_end = (visible_start + viewport_height).min(app.albums.len());

    // Only render visible albums
    let rows: Vec<Row> = app
        .albums
        .iter()
        .enumerate()
        .skip(visible_start)
        .take(viewport_height)
        .map(|(i, album)| {
            let style = if i == app.selected_album {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)),
                Cell::from(album.title.clone()),
                Cell::from(album.artist.clone()),
                Cell::from(format!("{}", album.tracks.len())),
                Cell::from(if album.conflict_count > 0 {
                    format!("{}", album.conflict_count)
                } else {
                    "-".to_string()
                }),
            ])
            .style(style)
        })
        .collect();

    let title = if app.albums.len() > viewport_height {
        format!(
            "Albums [{}-{} of {}]",
            visible_start + 1,
            visible_end,
            app.albums.len()
        )
    } else {
        "Albums".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Percentage(35),
            Constraint::Percentage(25),
            Constraint::Length(8),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(table, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  \u{2191}/k Up  \u{2193}/j Down  Enter Select  q Quit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, area);
}
