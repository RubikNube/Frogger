use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    high_score: u64,
}

pub struct HighScore {
    path: PathBuf,
    pub value: u64,
    pub warning: Option<String>,
    writable: bool,
}

fn read(path: &Path) -> io::Result<u64> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str::<Record>(&text)
            .map(|record| record.high_score)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

impl HighScore {
    pub fn load(path: PathBuf) -> Self {
        let result = read(&path);
        let writable = result.is_ok();
        let (value, warning) = match result {
            Ok(value) => (value, None),
            Err(error) => (
                0,
                Some(format!(
                    "cannot load high score {}: {error}",
                    path.display()
                )),
            ),
        };
        Self {
            path,
            value,
            warning,
            writable,
        }
    }

    pub fn record(&mut self, score: u64) {
        if score <= self.value {
            return;
        }
        self.value = score;
        // Preserve unreadable or malformed files, and keep playing with a session best.
        if !self.writable {
            return;
        }
        match self.save() {
            Ok(()) => self.warning = None,
            Err(error) => {
                self.warning = Some(format!(
                    "cannot save high score {}: {error}",
                    self.path.display()
                ));
            }
        }
    }

    fn save(&mut self) -> io::Result<()> {
        self.value = self.value.max(read(&self.path)?);
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
            let text = toml::to_string(&Record {
                high_score: self.value,
            })
            .map_err(io::Error::other)?;
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
        fn score_file(&self) -> PathBuf {
            self.0.join("scores/best.toml")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_starts_at_zero_and_best_survives_reloading() {
        let directory = Directory::new();
        let mut best = HighScore::load(directory.score_file());
        assert_eq!(best.value, 0);
        assert!(best.warning.is_none());
        assert!(!directory.score_file().exists());
        best.record(130);
        assert!(best.warning.is_none());
        best.record(10);
        best.record(130);
        assert_eq!(HighScore::load(directory.score_file()).value, 130);
        best.record(500);
        assert_eq!(HighScore::load(directory.score_file()).value, 500);
        assert_eq!(
            fs::read_dir(directory.score_file().parent().unwrap())
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn corrupt_file_is_preserved_and_session_best_still_updates() {
        let directory = Directory::new();
        let path = directory.0.join("broken.toml");
        fs::write(&path, "not a score").unwrap();
        let mut best = HighScore::load(path.clone());
        assert!(best.warning.is_some());
        best.record(42);
        assert_eq!(best.value, 42);
        assert_eq!(fs::read_to_string(path).unwrap(), "not a score");
    }

    #[test]
    fn write_failure_is_nonfatal_and_does_not_lose_in_memory_best() {
        let directory = Directory::new();
        let mut best = HighScore::load(directory.score_file());
        fs::write(directory.0.join("scores"), "blocks parent directory").unwrap();
        best.record(20);
        assert_eq!(best.value, 20);
        assert!(best.warning.is_some());
        fs::remove_file(directory.0.join("scores")).unwrap();
        best.record(30);
        assert!(best.warning.is_none());
        assert_eq!(HighScore::load(directory.score_file()).value, 30);
    }

    #[test]
    fn another_sessions_saved_best_is_not_lowered() {
        let directory = Directory::new();
        let mut first = HighScore::load(directory.score_file());
        let mut second = HighScore::load(directory.score_file());
        first.record(500);
        second.record(100);
        assert_eq!(second.value, 500);
        assert_eq!(HighScore::load(directory.score_file()).value, 500);
    }
}
