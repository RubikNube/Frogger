# Frogger

A terminal Frogger game written in Rust. Dodge traffic, ride logs across the
river, and fill five home slots before moving on to a faster round. Includes
Vim movement keys, configurable TOML settings and layouts, and colored ASCII
graphics.

## Run

Download a prebuilt executable from [GitHub Releases](https://github.com/RubikNube/Frogger/releases)
for Linux x64, Windows x64, or macOS (Intel or Apple Silicon). Extract the archive
and run `frogger` (`frogger.exe` on Windows) in an interactive terminal. Archives
include this README, the license, and the example configuration. Release assets
include a `SHA256SUMS` file for verifying downloads. Binaries are unsigned; Linux
builds target GNU/Linux using Ubuntu 24.04.

Install a current stable Rust toolchain, then run from this directory:

```sh
cargo run --release
```

Or install the executable locally:

```sh
cargo install --path . --locked
frogger
```

The default board needs a terminal of at least **64 columns by 21 rows**.
Smaller terminals display a resize prompt and freeze the simulation until the
board fits again. The game uses stdin and stdout directly; run it in an
interactive terminal rather than piping its output.

## Controls

| Action | Default keys |
| --- | --- |
| Start | Enter |
| Left / down / up / right | `h` / `j` / `k` / `l`, or arrow keys |
| Pause / resume | `p` or Space |
| Restart session | `r` (then Enter) |
| Quit | `q` or Ctrl-C |

The footer displays the configured controls. Key presses and terminal key
repeat both move the frog; Vim command mode and numeric prefixes are not used.
Ctrl-C always exits, including while the terminal is too small.

## Rules

- Your frog is `@`. Vehicles are `X`, logs are `=`, and water is `~`.
  Dots mark safe ground. Empty homes are `^`; filled homes are `O`.
- Move one cell at a time. Touching a vehicle, landing in water without a log,
  riding a log off either edge, or running out of time costs a life.
- Enter an empty home at its exact column. The other cells in the home row,
  including occupied homes, cost a life.
- Start with three lives and 60 seconds per crossing. A death or successful
  crossing resets the frog and timer. Completed homes persist through deaths.
- Earn 10 points for each new highest row reached during a crossing, 50 for a
  home plus remaining whole seconds, and 500 for filling all five homes.
  Moving backward and forward over the same rows does not award extra points.
- Each completed round clears the homes and adds 10% of the initial lane speed,
  capped at three times the initial speed.

## High score

The highest score appears above the board and is saved immediately whenever you
beat it. It survives restarts, game over, quitting (including Ctrl-C), and later
runs. All layouts share the same record by default.

The default save file is `frogger.highscore.toml` in the current working
directory. Set `[game] high_score_file = "/path/to/best.toml"` to share a record
across working directories, or use different paths for different layouts.
Relative save paths are relative to the working directory. Parent directories
are created automatically. Delete the save file while the game is closed to
reset the record.

If loading or saving fails, play continues with an in-memory best and the display
shows “storage unavailable”; the error is printed after exiting. Malformed save
files are preserved for repair rather than overwritten.

## Configuration

The game reads `frogger.toml` in the **current working directory** if it exists.
Otherwise, it uses built-in defaults. Select another file explicitly with:

```sh
cargo run --release -- --config frogger.example.toml
# Or, after installation:
frogger --config /path/to/my-game.toml
```

[frogger.example.toml](frogger.example.toml) contains every setting and the complete
default layout. Copy it to `frogger.toml` to customize it. Configuration is read
at startup; restart the program after editing it. Missing settings inherit their
defaults. Providing any `[[lanes]]` entries replaces the entire default layout.
Invalid files and explicitly requested missing files produce an error before
the terminal enters game mode.

For example, this file keeps the default board but changes lives and movement:

```toml
[game]
lives = 5
crossing_seconds = 90

[controls]
left = ["a", "h", "Left"]
down = ["s", "j", "Down"]
up = ["w", "k", "Up"]
right = ["d", "l", "Right"]
```

### Game settings

| `[game]` field | Default | Accepted values |
| --- | --- | --- |
| `width` | `41` | 11–201 cells |
| `lives` | `3` | 1–99 |
| `crossing_seconds` | `60` | 1–3600 |
| `speed_increase` | `0.1` | 0.0–1.0, added per round relative to initial speed |
| `max_speed` | `3.0` | 1.0–10.0 times initial speed |
| `high_score_file` | `"frogger.highscore.toml"` | Nonempty path to the persistent record |

Board height follows the lane count. The starting bank and home row are
generated automatically, with five evenly spaced single-cell homes.

### Controls and colors

`[controls]` contains `left`, `down`, `up`, `right`, `pause`, `restart`, and `quit`.
Each is a nonempty array of keys. Supported keys are single printable ASCII
characters (case-sensitive) and the names `Left`, `Down`, `Up`, `Right`, `Space`,
`Esc`, and `Tab`. Duplicate bindings are rejected. Enter is reserved for starting;
Ctrl-C is reserved for exiting. Modified key combinations are not configurable.

`[theme]` contains `frog`, `vehicle`, `log`, `water`, `ground`, `home`, and `text`.
Colors are `black`, `dark_grey`, `grey`, `white`, `red`, `green`, `yellow`, `blue`,
`magenta`, `cyan`, and the `dark_` variants of red, green, yellow, blue, magenta,
and cyan. All symbols remain distinguishable without color.

### Custom lanes

Define 1–40 lanes **from bottom to top**. Each lane is `safe`, `road`, or `river`.
Safe lanes only accept `kind`. Road and river lanes require all six moving-lane
fields shown below:

```toml
[[lanes]]
kind = "road"
direction = "left" # "left" or "right"
interval_ms = 450  # One cell per interval; 100–60000 milliseconds
length = 3        # Cells occupied by each vehicle or log
gap = 8           # Empty cells between objects
offset = 0        # Initial horizontal shift, from 0 through width - 1

[[lanes]]
kind = "safe"

[[lanes]]
kind = "river"
direction = "right"
interval_ms = 550
length = 7
gap = 3
offset = 0
```

Both `length` and `gap` must be positive, and their sum must fit within `width`.
The repeated object pattern is clipped to the board width and shifted as a
ring, wrapping at either edge. The seam can shorten the final gap or merge
adjacent objects. Frogs ride logs by one cell per movement interval and never
wrap around the board.

## Development

The simulation is independent of terminal I/O and advances in deterministic
10 ms steps. Configuration, game rules, rendering, and the event loop are
separate modules. Terminal state is restored on normal exit, Rust errors, and
panics. Crossterm provides terminal support for Linux, macOS, and Windows.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Use `frogger --help` for command-line usage and `frogger --version` for the
installed version.

GitHub Actions checks formatting and runs Clippy and tests on pull requests and
pushes to `main`. Validation covers Linux x64, Windows x64, and macOS on Intel
and Apple Silicon. Formatting failures require running `cargo fmt --all` locally
and committing the result; CI does not modify branches.

## Releasing

1. Update `version` in `Cargo.toml`, run `cargo check` to update `Cargo.lock`,
   and commit both files after the development checks pass.
2. Push the commit to `main` and wait for validation and formatting to pass.
3. Create and push a matching version tag, for example:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The tag must equal `v` followed by the package version. The release workflow
repeats formatting and validation against the tagged commit, builds and
smoke-tests all four executables, and publishes a GitHub Release with generated
notes, archives, and SHA-256 checksums only after every check and build succeeds.
Versions with a prerelease suffix (for example, `0.2.0-rc.1`) produce prereleases.
Publishing uses the built-in GitHub Actions token and does not require a custom
secret. Releases do not publish to crates.io.
