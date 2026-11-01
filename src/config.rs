use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crossterm::{event::KeyCode, style::Color};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub game: Settings,
    pub theme: Theme,
    pub controls: Controls,
    pub lanes: Vec<Lane>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game: Settings::default(),
            theme: Theme::default(),
            controls: Controls::default(),
            lanes: default_lanes(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub width: u16,
    pub lives: u32,
    pub crossing_seconds: u32,
    pub speed_increase: f64,
    pub max_speed: f64,
    pub high_score_file: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 41,
            lives: 3,
            crossing_seconds: 60,
            speed_increase: 0.1,
            max_speed: 3.0,
            high_score_file: "frogger.highscore.toml".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub frog: String,
    pub vehicle: String,
    pub log: String,
    pub water: String,
    pub ground: String,
    pub home: String,
    pub text: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            frog: "green".into(),
            vehicle: "red".into(),
            log: "yellow".into(),
            water: "blue".into(),
            ground: "dark_green".into(),
            home: "cyan".into(),
            text: "white".into(),
        }
    }
}

pub fn color(name: &str) -> Option<Color> {
    Some(match name {
        "black" => Color::Black,
        "dark_grey" => Color::DarkGrey,
        "grey" => Color::Grey,
        "white" => Color::White,
        "red" => Color::Red,
        "dark_red" => Color::DarkRed,
        "green" => Color::Green,
        "dark_green" => Color::DarkGreen,
        "yellow" => Color::Yellow,
        "dark_yellow" => Color::DarkYellow,
        "blue" => Color::Blue,
        "dark_blue" => Color::DarkBlue,
        "magenta" => Color::Magenta,
        "dark_magenta" => Color::DarkMagenta,
        "cyan" => Color::Cyan,
        "dark_cyan" => Color::DarkCyan,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Safe,
    Road,
    River,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Left,
    Right,
}

impl Direction {
    pub fn step(self) -> i32 {
        match self {
            Self::Left => -1,
            Self::Right => 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub kind: Kind,
    pub direction: Option<Direction>,
    pub interval_ms: Option<u32>,
    pub length: Option<u16>,
    pub gap: Option<u16>,
    pub offset: Option<u16>,
}

fn default_lanes() -> Vec<Lane> {
    let moving = |kind, direction, interval_ms, length, gap, offset| Lane {
        kind,
        direction: Some(direction),
        interval_ms: Some(interval_ms),
        length: Some(length),
        gap: Some(gap),
        offset: Some(offset),
    };
    vec![
        moving(Kind::Road, Direction::Left, 450, 3, 8, 0),
        moving(Kind::Road, Direction::Right, 350, 2, 7, 4),
        moving(Kind::Road, Direction::Left, 550, 5, 9, 7),
        moving(Kind::Road, Direction::Right, 300, 2, 8, 2),
        moving(Kind::Road, Direction::Left, 400, 3, 9, 5),
        Lane {
            kind: Kind::Safe,
            direction: None,
            interval_ms: None,
            length: None,
            gap: None,
            offset: None,
        },
        moving(Kind::River, Direction::Right, 550, 7, 3, 0),
        moving(Kind::River, Direction::Left, 450, 6, 3, 3),
        moving(Kind::River, Direction::Right, 650, 8, 3, 6),
        moving(Kind::River, Direction::Left, 500, 7, 3, 1),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Left,
    Down,
    Up,
    Right,
    Pause,
    Restart,
    Quit,
    Leaderboard,
    Players,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Controls {
    pub left: Vec<String>,
    pub down: Vec<String>,
    pub up: Vec<String>,
    pub right: Vec<String>,
    pub pause: Vec<String>,
    pub restart: Vec<String>,
    pub quit: Vec<String>,
    pub leaderboard: Vec<String>,
    pub players: Vec<String>,
}

impl Default for Controls {
    fn default() -> Self {
        let keys = |names: &[&str]| names.iter().map(|s| (*s).into()).collect();
        Self {
            left: keys(&["h", "Left"]),
            down: keys(&["j", "Down"]),
            up: keys(&["k", "Up"]),
            right: keys(&["l", "Right"]),
            pause: keys(&["p", "Space"]),
            restart: keys(&["r"]),
            quit: keys(&["q"]),
            leaderboard: keys(&["F2"]),
            players: keys(&["F3"]),
        }
    }
}

pub fn key_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "Left" => KeyCode::Left,
        "Down" => KeyCode::Down,
        "Up" => KeyCode::Up,
        "Right" => KeyCode::Right,
        "Space" => KeyCode::Char(' '),
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "F2" => KeyCode::F(2),
        "F3" => KeyCode::F(3),
        _ if name.len() == 1 && name.as_bytes()[0].is_ascii_graphic() => {
            KeyCode::Char(name.chars().next()?)
        }
        _ => return None,
    })
}

impl Controls {
    pub fn bindings(&self) -> [(Action, &Vec<String>); 9] {
        [
            (Action::Left, &self.left),
            (Action::Down, &self.down),
            (Action::Up, &self.up),
            (Action::Right, &self.right),
            (Action::Pause, &self.pause),
            (Action::Restart, &self.restart),
            (Action::Quit, &self.quit),
            (Action::Leaderboard, &self.leaderboard),
            (Action::Players, &self.players),
        ]
    }

    pub fn action(&self, code: KeyCode) -> Option<Action> {
        self.bindings().into_iter().find_map(|(action, keys)| {
            keys.iter()
                .any(|key| key_code(key) == Some(code))
                .then_some(action)
        })
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn load(explicit: Option<&Path>) -> Result<Self, String> {
        let path = explicit.unwrap_or_else(|| Path::new("frogger.toml"));
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if explicit.is_none() && e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let game = &self.game;
        if game.high_score_file.as_os_str().is_empty() {
            return Err("game.high_score_file must not be empty".into());
        }
        if !(11..=201).contains(&game.width) {
            return Err("game.width must be between 11 and 201".into());
        }
        if !(1..=99).contains(&game.lives) || !(1..=3600).contains(&game.crossing_seconds) {
            return Err("game.lives must be 1..99 and crossing_seconds must be 1..3600".into());
        }
        if !game.speed_increase.is_finite()
            || !(0.0..=1.0).contains(&game.speed_increase)
            || !game.max_speed.is_finite()
            || !(1.0..=10.0).contains(&game.max_speed)
        {
            return Err("game.speed_increase must be 0..1 and max_speed must be 1..10".into());
        }
        if self.lanes.is_empty() || self.lanes.len() > 40 {
            return Err("lanes must contain between 1 and 40 entries".into());
        }
        for (i, lane) in self.lanes.iter().enumerate() {
            let bad = |detail| format!("lanes[{i}]: {detail}");
            if lane.kind == Kind::Safe {
                if lane.direction.is_some()
                    || lane.interval_ms.is_some()
                    || lane.length.is_some()
                    || lane.gap.is_some()
                    || lane.offset.is_some()
                {
                    return Err(bad("safe lanes only accept kind"));
                }
                continue;
            }
            let (Some(_), Some(interval), Some(length), Some(gap), Some(offset)) = (
                lane.direction,
                lane.interval_ms,
                lane.length,
                lane.gap,
                lane.offset,
            ) else {
                return Err(bad(
                    "moving lanes require direction, interval_ms, length, gap, offset",
                ));
            };
            if !(100..=60_000).contains(&interval) {
                return Err(bad("interval_ms must be 100..60000"));
            }
            if length == 0 || gap == 0 || u32::from(length) + u32::from(gap) > u32::from(game.width)
            {
                return Err(bad(
                    "length and gap must be positive and their sum must fit the board",
                ));
            }
            if offset >= game.width {
                return Err(bad("offset must be less than game.width"));
            }
        }
        for (name, value) in [
            ("frog", &self.theme.frog),
            ("vehicle", &self.theme.vehicle),
            ("log", &self.theme.log),
            ("water", &self.theme.water),
            ("ground", &self.theme.ground),
            ("home", &self.theme.home),
            ("text", &self.theme.text),
        ] {
            if color(value).is_none() {
                return Err(format!("theme.{name}: unsupported color {value:?}"));
            }
        }
        let mut seen = HashSet::new();
        for (action, keys) in self.controls.bindings() {
            if keys.is_empty() {
                return Err(format!("controls.{action:?} needs at least one key"));
            }
            for key in keys {
                let code = key_code(key).ok_or_else(|| format!("unsupported key {key:?}"))?;
                if !seen.insert(code) {
                    return Err(format!("conflicting key binding {key:?}"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_example_match() {
        let config = Config::parse(include_str!("../frogger.example.toml")).unwrap();
        assert_eq!(format!("{config:?}"), format!("{:?}", Config::default()));
        Config::parse("").unwrap();
    }

    #[test]
    fn partial_settings_and_custom_controls() {
        let config = Config::parse("[game]\nlives = 5\n[controls]\nup = ['w']").unwrap();
        assert_eq!(config.game.lives, 5);
        assert_eq!(config.game.width, 41);
        assert_eq!(config.controls.action(KeyCode::Char('w')), Some(Action::Up));
        assert_eq!(config.controls.action(KeyCode::Char('k')), None);
    }

    #[test]
    fn custom_layout_replaces_defaults() {
        let config = Config::parse("[[lanes]]\nkind = 'safe'").unwrap();
        assert_eq!(config.lanes.len(), 1);
    }

    #[test]
    fn rejects_invalid_configuration() {
        for text in [
            "oops = 1",
            "[game]\nwidth = 5",
            "[game]\nlives = 0",
            "[game]\nhigh_score_file = ''",
            "[game]\nspeed_increase = nan",
            "[theme]\nfrog = 'invisible'",
            "[controls]\nup = ['h']",
            "[controls]\nup = ['Ctrl-K']",
            "[controls]\nup = []",
            "[controls]\nplayers = ['F2']",
            "lanes = []",
            "[[lanes]]\nkind = 'road'",
            "[[lanes]]\nkind = 'safe'\ngap = 4",
            "[broken",
        ] {
            assert!(Config::parse(text).is_err(), "accepted {text}");
        }
        let mut config = Config::default();
        config.lanes[0].gap = Some(100);
        assert!(config.validate().is_err());
        config.lanes[0].gap = Some(8);
        config.lanes[0].interval_ms = Some(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn explicit_missing_file_is_an_error() {
        assert!(Config::load(Some(Path::new("/missing/frogger/config.toml"))).is_err());
    }
}
