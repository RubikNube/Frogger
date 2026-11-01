mod app;
mod config;
mod game;
mod high_score;
mod terminal;

use std::{
    env,
    io::{self, IsTerminal},
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use app::App;
use config::Config;
use game::STEP_MS;
use high_score::HighScore;
use terminal::Terminal;

const HELP: &str = "Frogger - terminal arcade game

Usage: frogger [--config PATH]
       frogger --help
       frogger --version

Options:
  --config PATH  Load a TOML configuration file
  -h, --help     Show this help
  -V, --version  Show version

Uses ./frogger.toml when present, otherwise built-in defaults.
Move: h/j/k/l or arrows. Pause: p/Space. Restart: r. Quit: q/Ctrl-C.
Select a player, then Enter to start. Scores: F2. Players: F3.
See frogger.example.toml for configuration.";

fn arguments(args: impl Iterator<Item = std::ffi::OsString>) -> Result<Option<PathBuf>, String> {
    let mut args = args;
    let mut path = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--config") if path.is_none() => {
                path = Some(PathBuf::from(
                    args.next().ok_or("--config requires a path")?,
                ));
            }
            _ => return Err(format!("unexpected argument {:?}; use --help", arg)),
        }
    }
    Ok(path)
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 {
        match args[0].to_str() {
            Some("--help" | "-h") => {
                println!("{HELP}");
                return Ok(());
            }
            Some("--version" | "-V") => {
                println!("frogger {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }
    let path = arguments(args.into_iter())?;
    let config = Config::load(path.as_deref())?;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "an interactive terminal is required (stdin and stdout must be terminals)".into(),
        );
    }
    let mut high_score = HighScore::load(config.game.high_score_file.clone());
    let result = play(config, &mut high_score).map_err(|e| e.to_string());
    if let Some(warning) = &high_score.warning {
        eprintln!("frogger: {warning}");
    }
    result
}

fn play(config: Config, high_score: &mut HighScore) -> io::Result<()> {
    let mut app = App::new(config);
    let mut terminal = Terminal::enter()?;
    let result = play_loop(&mut app, high_score, &mut terminal);
    app.finish(high_score);
    result
}

fn play_loop(app: &mut App, scores: &mut HighScore, terminal: &mut Terminal) -> io::Result<()> {
    let step = Duration::from_millis(u64::from(STEP_MS));
    let mut last = Instant::now();
    let mut accumulator = Duration::ZERO;
    let mut fits = Terminal::fits(app)?;
    terminal.render(app, scores)?;
    loop {
        let was_simulating = app.simulating();
        let mut interrupted = false;
        if event::poll(Duration::from_millis(16))? {
            // Limit a burst so pasted text or key-repeat cannot starve the clock.
            for _ in 0..32 {
                match event::read()? {
                    Event::Key(key) if key.kind != KeyEventKind::Release => {
                        if key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)
                        {
                            return Ok(());
                        }
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        {
                            let before = app.simulating();
                            if app.key(key.code, scores, fits) {
                                return Ok(());
                            }
                            interrupted |= before != app.simulating();
                            fits = Terminal::fits(app)?;
                        }
                    }
                    Event::Resize(_, _) => {
                        interrupted = true;
                        fits = Terminal::fits(app)?;
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        let now = Instant::now();
        let new_fits = Terminal::fits(app)?;
        if fits && new_fits && was_simulating && app.simulating() && !interrupted {
            accumulator += (now - last).min(Duration::from_millis(250));
            while accumulator >= step {
                app.tick(scores);
                accumulator -= step;
            }
        } else {
            accumulator = Duration::ZERO;
        }
        fits = new_fits;
        last = now;
        terminal.render(app, scores)?;
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("frogger: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_config_and_errors() {
        let parse = |args: &[&str]| arguments(args.iter().map(std::ffi::OsString::from));
        assert_eq!(parse(&[]).unwrap(), None);
        assert_eq!(
            parse(&["--config", "my game.toml"]).unwrap(),
            Some("my game.toml".into())
        );
        assert!(parse(&["--config"]).is_err());
        assert!(parse(&["--bad"]).is_err());
        assert!(parse(&["--config", "a", "--config", "b"]).is_err());
    }
}
