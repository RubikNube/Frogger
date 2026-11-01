use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU32, Ordering},
};

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "frogger-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_frogger"))
            .current_dir(&self.0)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn help_and_version_work_without_a_terminal_or_valid_config() {
    let workspace = Workspace::new();
    fs::write(workspace.0.join("frogger.toml"), "invalid TOML").unwrap();
    let help = workspace.run(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));
    let version = workspace.run(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "frogger 0.1.0"
    );
}

#[test]
fn defaults_load_without_a_file_and_require_an_interactive_terminal() {
    let workspace = Workspace::new();
    let output = workspace.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    assert!(output.stdout.is_empty());
}

#[test]
fn local_config_errors_are_reported_before_entering_terminal_mode() {
    let workspace = Workspace::new();
    fs::write(workspace.0.join("frogger.toml"), "[game]\nwidth = 1").unwrap();
    let output = workspace.run(&[]);
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(error.contains("frogger.toml") && error.contains("game.width"));
    assert!(!error.contains("interactive terminal"));
    assert!(output.stdout.is_empty());
}

#[test]
fn explicit_config_overrides_local_file_and_missing_path_is_reported() {
    let workspace = Workspace::new();
    fs::write(workspace.0.join("frogger.toml"), "invalid TOML").unwrap();
    fs::write(workspace.0.join("custom.toml"), "[game]\nlives = 5").unwrap();
    let valid = workspace.run(&["--config", "custom.toml"]);
    assert!(String::from_utf8_lossy(&valid.stderr).contains("interactive terminal"));
    let missing = workspace.run(&["--config", "missing.toml"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing.toml"));
    assert!(missing.stdout.is_empty());
}
