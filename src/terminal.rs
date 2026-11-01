use std::io::{self, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};

use crate::{
    config::{Kind, color},
    game::{Game, State},
};

pub struct Terminal {
    out: Vec<u8>,
}

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), ResetColor, Show, LeaveAlternateScreen);
}

impl Terminal {
    pub fn enter() -> io::Result<Self> {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
        enable_raw_mode()?;
        let terminal = Self { out: Vec::new() };
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(terminal)
    }

    pub fn dimensions(game: &Game) -> (u16, u16) {
        (
            (game.config.game.width + 2).max(64),
            game.home_y() as u16 + 10,
        )
    }

    pub fn fits(game: &Game) -> io::Result<bool> {
        let (width, height) = terminal::size()?;
        let (needed_width, needed_height) = Self::dimensions(game);
        Ok(width >= needed_width && height >= needed_height)
    }

    pub fn render(&mut self, game: &Game, high_score: u64, score_warning: bool) -> io::Result<()> {
        let (width, height) = terminal::size()?;
        self.out.clear();
        queue!(self.out, ResetColor, Clear(ClearType::All))?;
        let (needed_width, needed_height) = Self::dimensions(game);
        if width < needed_width || height < needed_height {
            let lines = [
                "FROGGER - resize to continue".to_owned(),
                format!("Need {needed_width}x{needed_height}; current {width}x{height}"),
                "Simulation paused. Ctrl-C quits.".to_owned(),
            ];
            for (y, line) in lines.iter().take(height as usize).enumerate() {
                self.line(0, y as u16, line, width)?;
            }
            return self.flush_frame();
        }
        let left = (width - needed_width) / 2;
        let top = (height - needed_height) / 2;
        let theme = &game.config.theme;
        queue!(self.out, SetForegroundColor(color(&theme.text).unwrap()))?;
        self.line(left, top, "F R O G G E R", needed_width)?;
        self.line(
            left,
            top + 1,
            &format!(
                "Score: {}  Lives: {}  Round: {}  Time: {}s",
                game.score,
                game.lives,
                game.round,
                game.remaining_ms.div_ceil(1000)
            ),
            needed_width,
        )?;

        self.line(
            left,
            top + 2,
            &format!(
                "High score: {high_score}{}",
                if score_warning {
                    " (storage unavailable)"
                } else {
                    ""
                }
            ),
            needed_width,
        )?;
        let board_left = (width - game.config.game.width - 2) / 2;
        let border = format!("+{}+", "-".repeat(game.config.game.width as usize));
        self.line(board_left, top + 3, &border, needed_width)?;
        for y in (0..=game.home_y()).rev() {
            let row = top + 4 + (game.home_y() - y) as u16;
            queue!(
                self.out,
                SetForegroundColor(color(&theme.text).unwrap()),
                MoveTo(board_left, row),
                Print('|')
            )?;
            for x in 0..i32::from(game.config.game.width) {
                let (glyph, paint) = cell(game, x, y);
                queue!(self.out, SetForegroundColor(paint), Print(glyph))?;
            }
            queue!(
                self.out,
                SetForegroundColor(color(&theme.text).unwrap()),
                Print('|')
            )?;
        }
        let bottom = top + 5 + game.home_y() as u16;
        self.line(board_left, bottom, &border, needed_width)?;
        let status = match game.state {
            State::Ready => "READY - Enter to start",
            State::Playing => &game.message,
            State::Paused => "PAUSED - use your pause key to resume",
            State::GameOver => "GAME OVER - restart for a fresh game",
        };
        self.line(left, bottom + 1, status, needed_width)?;
        let controls = &game.config.controls;
        self.line(
            left,
            bottom + 2,
            &format!(
                "Move: {}/{}/{}/{}  Pause: {}",
                controls.left.join(","),
                controls.down.join(","),
                controls.up.join(","),
                controls.right.join(","),
                controls.pause.join(",")
            ),
            needed_width,
        )?;
        self.line(
            left,
            bottom + 3,
            &format!(
                "Restart: {}  Quit: {}  Ctrl-C: exit",
                controls.restart.join(","),
                controls.quit.join(",")
            ),
            needed_width,
        )?;
        queue!(self.out, ResetColor)?;
        self.flush_frame()
    }

    fn flush_frame(&self) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&self.out)?;
        stdout.flush()
    }

    fn line(&mut self, x: u16, y: u16, text: &str, max_width: u16) -> io::Result<()> {
        let clipped: String = text.chars().take(max_width as usize).collect();
        queue!(self.out, MoveTo(x, y), Print(clipped))
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        restore();
    }
}

fn cell(game: &Game, x: i32, y: usize) -> (char, Color) {
    let theme = &game.config.theme;
    let paint = |glyph, name: &str| (glyph, color(name).unwrap());
    if game.x == x && game.y == y {
        return paint('@', &theme.frog);
    }
    if y == game.home_y() {
        if let Some(i) = game.home_columns().iter().position(|&column| column == x) {
            return paint(if game.homes[i] { 'O' } else { '^' }, &theme.home);
        }
        return paint('#', &theme.ground);
    }
    if y == 0 {
        return paint('.', &theme.ground);
    }
    let lane = &game.lanes[y - 1];
    let occupied = lane.occupied(x, i32::from(game.config.game.width));
    match lane.spec.kind {
        Kind::Safe => paint('.', &theme.ground),
        Kind::Road if occupied => paint('X', &theme.vehicle),
        Kind::Road => paint(' ', &theme.text),
        Kind::River if occupied => paint('=', &theme.log),
        Kind::River => paint('~', &theme.water),
    }
}
