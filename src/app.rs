use crossterm::event::KeyCode;

use crate::{
    config::{Action, Config},
    game::{Game, State},
    high_score::{Entry, HighScore},
};

#[derive(Debug)]
pub enum Screen {
    Game,
    Players { selected: usize },
    Create { name: String, error: String },
    Leaderboard { back: Box<Screen> },
}

pub struct App {
    pub game: Game,
    pub screen: Screen,
    pub player: Option<String>,
    pub notice: String,
    session: Option<Entry>,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            game: Game::new(config),
            screen: Screen::Players { selected: 0 },
            player: None,
            notice: String::new(),
            session: None,
        }
    }

    pub fn simulating(&self) -> bool {
        matches!(self.screen, Screen::Game) && self.game.state == State::Playing
    }

    pub fn sync_score(&mut self, scores: &mut HighScore) {
        if let Some(entry) = &mut self.session
            && entry.score != self.game.score
        {
            entry.score = self.game.score;
            scores.record(entry);
        }
    }

    pub fn finish(&mut self, scores: &mut HighScore) {
        self.sync_score(scores);
        if self.session.take().is_some() {
            scores.persist();
        }
    }

    fn select(&mut self, player: String, scores: &mut HighScore) {
        self.finish(scores);
        self.player = Some(player);
        self.game.action(Action::Restart);
        self.notice.clear();
        self.screen = Screen::Game;
    }

    // Returns true when the caller should exit. Ctrl-C is handled globally.
    pub fn key(&mut self, code: KeyCode, scores: &mut HighScore, fits: bool) -> bool {
        let action = self.game.config.controls.action(code);
        // Text input owns printable keys, including configured gameplay bindings.
        if let Screen::Create { name, error } = &mut self.screen {
            if !fits {
                return false;
            }
            match code {
                KeyCode::Esc => {
                    self.screen = Screen::Players {
                        selected: scores.players().len(),
                    }
                }
                KeyCode::Backspace => {
                    name.pop();
                    error.clear();
                }
                KeyCode::Char(c) if c.is_ascii_graphic() || c == ' ' => {
                    if name.len() < 20 {
                        name.push(c);
                        error.clear();
                    } else {
                        *error = "Names can contain at most 20 characters.".into();
                    }
                }
                KeyCode::Enter => match scores.add_player(name) {
                    Ok(player) => self.select(player, scores),
                    Err(message) => *error = message,
                },
                _ => {}
            }
            return false;
        }
        if action == Some(Action::Quit) {
            self.finish(scores);
            return true;
        }
        if !fits {
            return false;
        }
        if matches!(self.screen, Screen::Leaderboard { .. }) {
            if code == KeyCode::Esc || action == Some(Action::Leaderboard) {
                let Screen::Leaderboard { back } =
                    std::mem::replace(&mut self.screen, Screen::Game)
                else {
                    unreachable!()
                };
                self.screen = *back;
            }
            return false;
        }
        if action == Some(Action::Leaderboard) {
            let back = Box::new(std::mem::replace(&mut self.screen, Screen::Game));
            self.screen = Screen::Leaderboard { back };
            return false;
        }
        if let Screen::Players { selected } = &mut self.screen {
            match code {
                KeyCode::Up => *selected = selected.saturating_sub(1),
                KeyCode::Down => *selected = (*selected + 1).min(scores.players().len()),
                KeyCode::Esc if self.player.is_some() => self.screen = Screen::Game,
                KeyCode::Enter => {
                    if let Some(player) = scores.players().get(*selected).cloned() {
                        self.select(player, scores);
                    } else {
                        self.screen = Screen::Create {
                            name: String::new(),
                            error: String::new(),
                        };
                    }
                }
                _ => {}
            }
            return false;
        }
        self.notice.clear();
        match action {
            Some(Action::Players) if matches!(self.game.state, State::Ready | State::GameOver) => {
                self.screen = Screen::Players { selected: 0 };
            }
            Some(Action::Players) => self.notice = "Restart before switching players.".into(),
            Some(Action::Restart) => {
                self.finish(scores);
                self.game.action(Action::Restart);
            }
            Some(action) => {
                self.game.action(action);
                self.sync_score(scores);
            }
            None if code == KeyCode::Enter && self.game.state == State::Ready => {
                if let Some(player) = &self.player {
                    self.game.start();
                    let entry = Entry::new(player.clone());
                    scores.record(&entry);
                    self.session = Some(entry);
                }
            }
            _ => {}
        }
        if self.game.state == State::GameOver {
            self.finish(scores);
        }
        false
    }

    pub fn tick(&mut self, scores: &mut HighScore) {
        if self.simulating() {
            self.game.tick();
            self.sync_score(scores);
            if self.game.state == State::GameOver {
                self.finish(scores);
            }
        }
    }

    pub fn menu_lines(&self, scores: &HighScore) -> Vec<String> {
        let mut lines = match &self.screen {
            Screen::Game => return Vec::new(),
            Screen::Players { selected } => {
                let count = scores.players().len() + 1;
                let start = selected / 8 * 8;
                let mut lines = vec!["CHOOSE PLAYER".into(), String::new()];
                for i in start..(start + 8).min(count) {
                    let label = scores
                        .players()
                        .get(i)
                        .map_or("Create player", String::as_str);
                    lines.push(format!(
                        "{} {label}",
                        if i == *selected { ">" } else { " " }
                    ));
                }
                lines.push(String::new());
                lines.push(format!(
                    "Players {}-{} of {count}",
                    start + 1,
                    (start + 8).min(count)
                ));
                lines.push("Up/Down: select  Enter: confirm  Esc: back".into());
                lines
            }
            Screen::Create { name, error } => vec![
                "CREATE PLAYER".into(),
                String::new(),
                format!("Name: {name}_"),
                "1-20 printable ASCII characters.".into(),
                error.clone(),
                "Enter: create  Backspace: edit  Esc: cancel".into(),
            ],
            Screen::Leaderboard { .. } => {
                let mut lines = vec![
                    "TOP 10 GAMES".into(),
                    String::new(),
                    format!(
                        "{:<5} {:<20} {:>20}  {:<10}",
                        "Rank", "Player", "Score", "Date (UTC)"
                    ),
                ];
                if scores.entries().is_empty() {
                    lines.push("No games yet. Select a player and press Enter to play!".into());
                }
                for (i, entry) in scores.entries().iter().enumerate() {
                    lines.push(format!(
                        "{:<5} {:<20} {:>20}  {:<10}",
                        i + 1,
                        entry.player,
                        entry.score,
                        entry.date.as_deref().unwrap_or("Unknown")
                    ));
                }
                lines.push(String::new());
                lines.push("Esc: return".into());
                lines
            }
        };
        let controls = &self.game.config.controls;
        lines.push(format!(
            "Scores: {}  Quit: {}  Ctrl-C: exit",
            controls.leaderboard.join(","),
            controls.quit.join(",")
        ));
        if scores.warning.is_some() {
            lines.push("Storage unavailable - changes kept in memory.".into());
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        app: App,
        scores: HighScore,
        path: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "frogger-app-{}-{}.toml",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            Self {
                app: App::new(Config::default()),
                scores: HighScore::load(path.clone()),
                path,
            }
        }
        fn key(&mut self, key: KeyCode) {
            assert!(!self.app.key(key, &mut self.scores, true));
        }
        fn player(&mut self) {
            self.key(KeyCode::Enter);
            for c in "quirk".chars() {
                self.key(KeyCode::Char(c));
            }
            self.key(KeyCode::Enter);
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn leaderboard_shows_dates_and_unknown_without_exceeding_terminal_width() {
        let mut f = Fixture::new();
        f.scores.add_player("12345678901234567890").unwrap();
        let mut entry = Entry::new("12345678901234567890".into());
        entry.date = Some("2026-11-01".into());
        entry.score = 100;
        f.scores.record(&entry);
        let mut old = Entry::new(entry.player.clone());
        old.date = None;
        f.scores.record(&old);
        f.key(KeyCode::F(2));
        let lines = f.app.menu_lines(&f.scores);
        assert!(lines.iter().any(|line| line.contains("Date (UTC)")));
        assert!(lines.iter().any(|line| line.contains("2026-11-01")));
        assert!(lines.iter().any(|line| line.contains("Unknown")));
        assert!(lines.iter().all(|line| line.len() <= 64));
    }

    #[test]
    fn startup_and_text_input_own_gameplay_keys() {
        let mut f = Fixture::new();
        f.player();
        assert_eq!(f.app.player.as_deref(), Some("quirk"));
        assert_eq!(f.app.game.state, State::Ready);
        assert!(f.scores.entries().is_empty());
        f.key(KeyCode::Enter);
        f.key(KeyCode::Enter);
        assert_eq!(f.scores.entries().len(), 1);
        assert_eq!(f.scores.entries()[0].score, 0);
    }

    #[test]
    fn overlays_freeze_and_restore_playing_or_paused_state() {
        let mut f = Fixture::new();
        f.player();
        f.key(KeyCode::Enter);
        let time = f.app.game.remaining_ms;
        f.key(KeyCode::F(2));
        f.app.tick(&mut f.scores);
        assert_eq!(f.app.game.remaining_ms, time);
        f.key(KeyCode::Esc);
        assert!(f.app.simulating());
        f.key(KeyCode::Char('p'));
        f.key(KeyCode::F(2));
        f.key(KeyCode::Esc);
        assert_eq!(f.app.game.state, State::Paused);
        f.key(KeyCode::F(3));
        assert!(matches!(f.app.screen, Screen::Game));
        assert!(f.app.notice.contains("Restart"));
    }

    #[test]
    fn restart_quit_and_game_over_record_each_game_once() {
        let mut f = Fixture::new();
        f.player();
        f.key(KeyCode::Enter);
        f.app.game.score = 80;
        f.key(KeyCode::Char('r'));
        assert_eq!(f.scores.entries()[0].score, 80);
        f.key(KeyCode::F(3));
        assert!(matches!(f.app.screen, Screen::Players { .. }));
        f.key(KeyCode::Esc);
        f.key(KeyCode::Enter);
        f.app.game.lives = 1;
        f.app.game.remaining_ms = 1;
        f.app.tick(&mut f.scores);
        assert_eq!(f.app.game.state, State::GameOver);
        f.key(KeyCode::Char('r'));
        f.key(KeyCode::Enter);
        f.app.game.score = 20;
        assert!(f.app.key(KeyCode::Char('q'), &mut f.scores, true));
        f.app.finish(&mut f.scores); // Same path used by Ctrl-C and terminal errors.
        let loaded = HighScore::load(f.path.clone());
        assert_eq!(loaded.entries().len(), 3);
        assert_eq!(
            loaded.entries().iter().map(|e| e.score).collect::<Vec<_>>(),
            vec![80, 20, 0]
        );
    }

    #[test]
    fn menus_page_profiles_and_resize_blocks_input() {
        let mut f = Fixture::new();
        for n in 0..17 {
            f.scores.add_player(&format!("Player {n}")).unwrap();
        }
        for _ in 0..17 {
            f.key(KeyCode::Down);
        }
        let lines = f.app.menu_lines(&f.scores);
        assert!(lines.iter().any(|l| l == "> Create player"));
        assert!(!f.app.key(KeyCode::Enter, &mut f.scores, false));
        assert!(matches!(f.app.screen, Screen::Players { .. }));
        f.key(KeyCode::Enter);
        f.key(KeyCode::Enter);
        assert!(matches!(&f.app.screen, Screen::Create { error, .. } if !error.is_empty()));
    }
}
