use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use tessitura_core::schema::Database;

pub mod album_list;
pub mod track_detail;

/// Which view the TUI is currently displaying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    AlbumList,
    /// Track detail view for the album at the given index.
    TrackDetail(usize),
}

/// An album grouped from identified items, ready for review.
#[derive(Debug)]
pub struct ReviewAlbum {
    pub title: String,
    pub artist: String,
    pub tracks: Vec<ReviewTrack>,
    pub conflict_count: usize,
}

/// A single track within a review album.
#[derive(Debug)]
pub struct ReviewTrack {
    /// Item ID for treadle review approval (used when accept is wired up).
    #[allow(dead_code)]
    pub item_id: String,
    pub title: String,
    pub proposed_tags: Vec<serde_json::Value>,
    /// Whether this track has conflicting proposals (used in future conflict display).
    #[allow(dead_code)]
    pub has_conflicts: bool,
}

/// Application state for the review TUI.
#[derive(Debug)]
pub struct App {
    pub view: View,
    pub albums: Vec<ReviewAlbum>,
    pub selected_album: usize,
    pub selected_track: usize,
    pub album_list_offset: usize, // First visible album in the list
    pub should_quit: bool,
}

impl App {
    /// Create a new `App` by loading review data from the database.
    pub fn new(db_path: &Path) -> Result<Self> {
        let albums = load_review_albums(db_path)?;
        Ok(Self {
            view: View::AlbumList,
            albums,
            selected_album: 0,
            selected_track: 0,
            album_list_offset: 0,
            should_quit: false,
        })
    }

    fn handle_key(&mut self, key: KeyCode) {
        match &self.view {
            View::AlbumList => self.handle_album_list_key(key),
            View::TrackDetail(_) => self.handle_track_detail_key(key),
        }
    }

    fn handle_album_list_key(&mut self, key: KeyCode) {
        // Assume reasonable viewport height (will be refined in render)
        const VIEWPORT_HEIGHT: usize = 20;

        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_album + 1 < self.albums.len() {
                    self.selected_album += 1;
                    // Scroll down if selection goes below visible area
                    if self.selected_album >= self.album_list_offset + VIEWPORT_HEIGHT {
                        self.album_list_offset = self.selected_album - VIEWPORT_HEIGHT + 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_album > 0 {
                    self.selected_album -= 1;
                    // Scroll up if selection goes above visible area
                    if self.selected_album < self.album_list_offset {
                        self.album_list_offset = self.selected_album;
                    }
                }
            }
            KeyCode::Enter => {
                if !self.albums.is_empty() {
                    self.selected_track = 0;
                    self.view = View::TrackDetail(self.selected_album);
                }
            }
            _ => {}
        }
    }

    fn handle_track_detail_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc | KeyCode::Char('b') => {
                self.view = View::AlbumList;
            }
            KeyCode::Char('n' | 'j') | KeyCode::Down => {
                if let View::TrackDetail(album_idx) = self.view {
                    if album_idx < self.albums.len() {
                        let track_count = self.albums[album_idx].tracks.len();
                        if self.selected_track + 1 < track_count {
                            self.selected_track += 1;
                        }
                    }
                }
            }
            KeyCode::Char('p' | 'k') | KeyCode::Up => {
                if self.selected_track > 0 {
                    self.selected_track -= 1;
                }
            }
            _ => {}
        }
    }
}

/// Load identified items from the database and group them into albums for review.
fn load_review_albums(db_path: &Path) -> Result<Vec<ReviewAlbum>> {
    let db = Database::open(db_path)?;
    let items = db.list_identified_items()?;

    if items.is_empty() {
        return Ok(Vec::new());
    }

    // Group items by album name
    let mut albums_map: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for item in &items {
        let album = item
            .tag_album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());
        albums_map.entry(album).or_default().push(item);
    }

    let albums = albums_map
        .into_iter()
        .map(|(album_name, items)| {
            let artist = items
                .first()
                .and_then(|i| i.tag_artist.clone())
                .unwrap_or_else(|| "Unknown Artist".to_string());
            let tracks = items
                .iter()
                .map(|item| ReviewTrack {
                    item_id: item.id.to_string(),
                    title: item
                        .tag_title
                        .clone()
                        .unwrap_or_else(|| "Unknown Track".to_string()),
                    proposed_tags: Vec::new(), // Populated from harmonization results
                    has_conflicts: false,
                })
                .collect();
            ReviewAlbum {
                title: album_name,
                artist,
                conflict_count: 0,
                tracks,
            }
        })
        .collect();

    Ok(albums)
}

/// Run the review TUI.
///
/// Sets up the terminal, runs the main event loop, and restores the terminal
/// on exit (including on error).
pub fn run_tui(db_path: PathBuf) -> Result<()> {
    let app = App::new(&db_path)?;

    if app.albums.is_empty() {
        println!("No items awaiting review.");
        println!("Run 'tessitura harmonize' first to generate proposed tags.");
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the event loop, capturing any error so we can restore the terminal
    let result = run_event_loop(&mut terminal, app);

    // Restore terminal regardless of success or failure
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<()> {
    loop {
        terminal.draw(|frame| match &app.view {
            View::AlbumList => album_list::render(frame, &app),
            View::TrackDetail(album_idx) => track_detail::render(frame, &app, *album_idx),
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_key(key.code);
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
