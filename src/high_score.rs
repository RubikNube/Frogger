use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub player: String,
    pub score: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

impl Entry {
    pub fn new(player: String) -> Self {
        let now = SystemTime::now();
        let nanos = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            id: format!(
                "{nanos:039}-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ),
            player,
            score: 0,
            date: utc_date(now),
        }
    }
}

// Calendar dates are captured once at game start, using UTC on every platform.
fn utc_date(time: SystemTime) -> Option<String> {
    let mut days = time.duration_since(UNIX_EPOCH).ok()?.as_secs() / 86_400;
    for year in 1970..=9999 {
        let months = month_lengths(year);
        let year_days: u64 = months.iter().sum();
        if days >= year_days {
            days -= year_days;
            continue;
        }
        for (index, length) in months.into_iter().enumerate() {
            if days < length {
                return Some(format!("{year:04}-{:02}-{:02}", index + 1, days + 1));
            }
            days -= length;
        }
    }
    None
}

fn month_lengths(year: u32) -> [u64; 12] {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ]
}

fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap();
    let month: usize = date[5..7].parse().unwrap();
    let day: u64 = date[8..].parse().unwrap();
    year > 0 && (1..=12).contains(&month) && (1..=month_lengths(year)[month - 1]).contains(&day)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Records {
    version: u32,
    high_score: u64,
    players: Vec<String>,
    entries: Vec<Entry>,
}

impl Default for Records {
    fn default() -> Self {
        Self {
            version: 1,
            high_score: 0,
            players: Vec::new(),
            entries: Vec::new(),
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 20
        && name == name.trim()
        && name.bytes().all(|c| c.is_ascii_graphic() || c == b' ')
}

impl Records {
    fn merge(&mut self, other: Self) {
        self.high_score = self.high_score.max(other.high_score);
        for player in other.players {
            if !self.players.iter().any(|p| p.eq_ignore_ascii_case(&player)) {
                self.players.push(player);
            }
        }
        for entry in other.entries {
            if let Some(existing) = self.entries.iter_mut().find(|e| e.id == entry.id) {
                existing.score = existing.score.max(entry.score);
                if existing.date.is_none() {
                    existing.date = entry.date;
                }
            } else {
                self.entries.push(entry);
            }
        }
        self.entries
            .sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
        self.entries.truncate(10);
        self.high_score = self
            .high_score
            .max(self.entries.first().map_or(0, |e| e.score));
    }
}

fn read(path: &Path) -> io::Result<Records> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Records::default()),
        Err(error) => return Err(error),
    };
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Legacy {
        high_score: u64,
    }
    if let Ok(legacy) = toml::from_str::<Legacy>(&text) {
        let mut records = Records {
            high_score: legacy.high_score,
            ..Records::default()
        };
        if legacy.high_score > 0 {
            records.entries.push(Entry {
                id: "0".into(),
                player: "Legacy".into(),
                score: legacy.high_score,
                date: None,
            });
        }
        return Ok(records);
    }
    let records: Records =
        toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    if records.version != 1
        || records
            .players
            .iter()
            .any(|p| !valid_name(p) || !names.insert(p.to_ascii_lowercase()))
        || records.entries.iter().any(|e| {
            e.id.is_empty()
                || !ids.insert(&e.id)
                || !valid_name(&e.player)
                || e.date.as_deref().is_some_and(|date| !valid_date(date))
                || !(e.id == "0" && e.player == "Legacy"
                    || names.contains(&e.player.to_ascii_lowercase()))
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid player or score records",
        ));
    }
    let mut normalized = Records::default();
    normalized.merge(records);
    Ok(normalized)
}

pub struct HighScore {
    path: PathBuf,
    records: Records,
    pub warning: Option<String>,
    writable: bool,
}

impl HighScore {
    pub fn load(path: PathBuf) -> Self {
        let (records, warning, writable) = match read(&path) {
            Ok(records) => (records, None, true),
            Err(error) => (
                Records::default(),
                Some(format!(
                    "cannot load high score {}: {error}",
                    path.display()
                )),
                false,
            ),
        };
        Self {
            path,
            records,
            warning,
            writable,
        }
    }

    pub fn value(&self) -> u64 {
        self.records.high_score
    }
    pub fn players(&self) -> &[String] {
        &self.records.players
    }
    pub fn entries(&self) -> &[Entry] {
        &self.records.entries
    }

    pub fn add_player(&mut self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if !valid_name(name) {
            return Err("Use 1-20 printable ASCII characters.".into());
        }
        if self
            .records
            .players
            .iter()
            .any(|p| p.eq_ignore_ascii_case(name))
        {
            return Err("That player already exists.".into());
        }
        self.records.players.push(name.into());
        self.persist();
        Ok(name.into())
    }

    pub fn record(&mut self, entry: &Entry) {
        let other = Records {
            entries: vec![entry.clone()],
            ..Records::default()
        };
        self.records.merge(other);
        self.persist();
    }

    pub fn persist(&mut self) {
        if !self.writable {
            return;
        }
        match self.save() {
            Ok(()) => self.warning = None,
            Err(error) => {
                self.warning = Some(format!(
                    "cannot save high score {}: {error}",
                    self.path.display()
                ))
            }
        }
    }

    fn save(&mut self) -> io::Result<()> {
        self.records.merge(read(&self.path)?);
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mut name = self.path.as_os_str().to_os_string();
        name.push(format!(
            ".{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = PathBuf::from(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            let text = toml::to_string(&self.records).map_err(io::Error::other)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "frogger-score-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("scores/best.toml")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn utc_dates_cover_epoch_leap_days_and_midnight() {
        use std::time::Duration;
        for (seconds, expected) in [
            (0, "1970-01-01"),
            (86_399, "1970-01-01"),
            (86_400, "1970-01-02"),
            (951_782_400, "2000-02-29"),
            (1_709_164_800, "2024-02-29"),
            (1_709_251_200, "2024-03-01"),
            (4_107_542_400, "2100-03-01"),
        ] {
            assert_eq!(
                utc_date(UNIX_EPOCH + Duration::from_secs(seconds)).as_deref(),
                Some(expected)
            );
        }
        assert!(utc_date(UNIX_EPOCH - Duration::from_secs(1)).is_none());
        for invalid in [
            "2025-02-29",
            "2100-02-29",
            "2026-13-01",
            "2026-00-01",
            "2026-01-00",
            "2026-04-31",
            "0000-01-01",
            "2026-1-01",
            "2026-11-01\x1b",
            "éé-11-01",
        ] {
            assert!(!valid_date(invalid), "accepted {invalid}");
        }
    }

    #[test]
    fn undated_score_tables_remain_readable_and_dates_merge() {
        let dir = Directory::new();
        let path = dir.0.join("undated.toml");
        fs::write(&path, "version = 1\nhigh_score = 20\nplayers = ['Alice']\n[[entries]]\nid = 'old'\nplayer = 'Alice'\nscore = 20\n").unwrap();
        let mut scores = HighScore::load(path.clone());
        assert!(scores.warning.is_none());
        assert!(scores.entries()[0].date.is_none());
        let mut entry = scores.entries()[0].clone();
        entry.date = Some("2026-11-01".into());
        scores.record(&entry);
        // An older copy of the same session must not erase its saved date.
        entry.date = None;
        scores.record(&entry);
        let loaded = HighScore::load(path);
        assert_eq!(loaded.entries()[0].date.as_deref(), Some("2026-11-01"));
    }

    #[test]
    fn profiles_validate_and_survive_reload() {
        let dir = Directory::new();
        let mut scores = HighScore::load(dir.file());
        assert!(!dir.file().exists());
        assert_eq!(scores.add_player(" Alice ").unwrap(), "Alice");
        for bad in ["alice", "", "  ", "a\nb", "é", "123456789012345678901"] {
            assert!(scores.add_player(bad).is_err());
        }
        assert_eq!(HighScore::load(dir.file()).players(), &["Alice"]);
    }

    #[test]
    fn ranking_updates_sessions_and_retains_only_ten() {
        let dir = Directory::new();
        let mut scores = HighScore::load(dir.file());
        scores.add_player("Alice").unwrap();
        let mut first = Entry::new("Alice".into());
        assert!(first.date.as_deref().is_some_and(valid_date));
        first.date = Some("2026-11-01".into());
        scores.record(&first);
        assert_eq!(scores.entries()[0].score, 0);
        for _ in 0..12 {
            let mut e = Entry::new("Alice".into());
            e.score = 50;
            scores.record(&e);
        }
        first.score = 50;
        scores.record(&first);
        assert_eq!(scores.entries()[0].id, first.id);
        first.score = 100;
        scores.record(&first);
        scores.record(&first);
        let loaded = HighScore::load(dir.file());
        assert_eq!(loaded.entries().len(), 10);
        assert_eq!(loaded.entries()[0].score, 100);
        assert_eq!(loaded.entries()[0].date.as_deref(), Some("2026-11-01"));
        assert_eq!(
            loaded.entries().iter().filter(|e| e.id == first.id).count(),
            1
        );
        assert_eq!(loaded.value(), 100);
        assert_eq!(
            fs::read_dir(dir.file().parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn migrates_legacy_without_selectable_legacy_player() {
        let dir = Directory::new();
        let path = dir.0.join("old.toml");
        fs::write(&path, "high_score = 500").unwrap();
        let mut scores = HighScore::load(path.clone());
        assert!(scores.players().is_empty());
        assert_eq!(scores.entries()[0].player, "Legacy");
        scores.add_player("New").unwrap();
        let loaded = HighScore::load(path);
        assert_eq!(loaded.value(), 500);
        assert_eq!(loaded.entries().len(), 1);
        assert!(loaded.entries()[0].date.is_none());
    }

    #[test]
    fn corrupt_and_unsupported_files_are_preserved() {
        let dir = Directory::new();
        let path = dir.0.join("bad.toml");
        for text in [
            "not a score",
            "version = 2\nhigh_score = 0\nplayers = []\nentries = []",
        ] {
            fs::write(&path, text).unwrap();
            let mut scores = HighScore::load(path.clone());
            assert!(scores.warning.is_some());
            scores.add_player("Alice").unwrap();
            let mut e = Entry::new("Alice".into());
            e.score = 42;
            scores.record(&e);
            assert_eq!(scores.value(), 42);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
    }

    #[test]
    fn write_failure_recovers_and_sequential_sessions_merge() {
        let dir = Directory::new();
        let mut first = HighScore::load(dir.file());
        fs::write(dir.0.join("scores"), "blocked").unwrap();
        first.add_player("Alice").unwrap();
        assert!(first.warning.is_some());
        fs::remove_file(dir.0.join("scores")).unwrap();
        first.persist();
        assert!(first.warning.is_none());
        let mut second = HighScore::load(dir.file());
        let mut a = Entry::new("Alice".into());
        a.score = 500;
        first.record(&a);
        second.add_player("Bob").unwrap();
        let mut b = Entry::new("Bob".into());
        b.score = 100;
        second.record(&b);
        let loaded = HighScore::load(dir.file());
        assert_eq!(loaded.value(), 500);
        assert_eq!(loaded.players().len(), 2);
        assert_eq!(loaded.entries().len(), 2);
    }
}
