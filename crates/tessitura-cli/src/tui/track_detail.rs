use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::App;

/// Render the track detail view for a given album.
pub fn render(frame: &mut Frame, app: &App, album_idx: usize) {
    let area = frame.area();

    let Some(album) = app.albums.get(album_idx) else {
        let msg = Paragraph::new("Album not found").style(Style::default().fg(Color::Red));
        frame.render_widget(msg, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Album header
            Constraint::Length(3), // Track header
            Constraint::Min(5),    // Proposed tags
            Constraint::Length(3), // Help bar
        ])
        .split(area);

    render_album_header(frame, album, chunks[0]);
    render_track_header(frame, app, album, chunks[1]);
    render_proposed_tags(frame, app, album, chunks[2]);
    render_help(frame, chunks[3]);
}

fn render_album_header(frame: &mut Frame, album: &super::ReviewAlbum, area: Rect) {
    let header = Paragraph::new(format!("{} \u{2014} {}", album.title, album.artist))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn render_track_header(frame: &mut Frame, app: &App, album: &super::ReviewAlbum, area: Rect) {
    let track = album.tracks.get(app.selected_track);
    let track_title = track
        .map(|t| format!("Track {}: {}", app.selected_track + 1, t.title))
        .unwrap_or_else(|| "No tracks".to_string());
    let header = Paragraph::new(track_title)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn render_proposed_tags(frame: &mut Frame, app: &App, album: &super::ReviewAlbum, area: Rect) {
    let tag_lines: Vec<Line<'_>> = if let Some(track) = album.tracks.get(app.selected_track) {
        if track.proposed_tags.is_empty() {
            vec![Line::from(Span::styled(
                "  No proposed tags yet. Run 'tessitura harmonize' first.",
                Style::default().fg(Color::Yellow),
            ))]
        } else {
            track
                .proposed_tags
                .iter()
                .map(|tag| {
                    let field = tag
                        .get("field")
                        .and_then(|f| f.as_str())
                        .unwrap_or("unknown");
                    let value = tag.get("value").and_then(|v| v.as_str()).unwrap_or("?");
                    let rule = tag.get("rule_name").and_then(|r| r.as_str()).unwrap_or("");
                    let confidence = tag
                        .get("confidence")
                        .and_then(|c| c.as_f64())
                        .unwrap_or(0.0);

                    Line::from(vec![
                        Span::styled(format!("  {:<20}", field), Style::default().fg(Color::Cyan)),
                        Span::raw(format!("{:<30}", value)),
                        Span::styled(
                            format!("[{} {:.0}%]", rule, confidence * 100.0),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])
                })
                .collect()
        }
    } else {
        vec![Line::from("  No track selected")]
    };

    let tags = Paragraph::new(tag_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Proposed Tags"),
    );
    frame.render_widget(tags, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Paragraph::new("  \u{2191}/k Prev  \u{2193}/j Next  b Back  q Quit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, area);
}
