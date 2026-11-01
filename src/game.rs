use crate::config::{Action, Config, Kind, Lane};

pub const STEP_MS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ready,
    Playing,
    Paused,
    GameOver,
}

pub struct MovingLane {
    pub spec: Lane,
    pub phase: i32,
    elapsed_ms: f64,
}

impl MovingLane {
    pub fn occupied(&self, x: i32, width: i32) -> bool {
        if self.spec.kind == Kind::Safe {
            return false;
        }
        let position = (x - self.phase).rem_euclid(width) as u16;
        position % (self.spec.length.unwrap() + self.spec.gap.unwrap()) < self.spec.length.unwrap()
    }
}

pub struct Game {
    pub config: Config,
    pub state: State,
    pub x: i32,
    pub y: usize,
    pub lanes: Vec<MovingLane>,
    pub homes: [bool; 5],
    pub lives: u32,
    pub score: u64,
    pub round: u32,
    pub remaining_ms: u32,
    pub message: String,
    furthest_row: usize,
}

impl Game {
    pub fn new(config: Config) -> Self {
        let mut game = Self {
            x: i32::from(config.game.width / 2),
            y: 0,
            lives: config.game.lives,
            remaining_ms: config.game.crossing_seconds * 1000,
            config,
            state: State::Ready,
            lanes: Vec::new(),
            homes: [false; 5],
            score: 0,
            round: 1,
            message: "Fill all five homes. Watch traffic and ride logs!".into(),
            furthest_row: 0,
        };
        game.reset_lanes();
        game
    }

    fn reset_lanes(&mut self) {
        self.lanes = self
            .config
            .lanes
            .iter()
            .cloned()
            .map(|spec| MovingLane {
                phase: i32::from(spec.offset.unwrap_or(0)),
                spec,
                elapsed_ms: 0.0,
            })
            .collect();
    }

    pub fn home_y(&self) -> usize {
        self.lanes.len() + 1
    }

    pub fn home_columns(&self) -> [i32; 5] {
        let width = i32::from(self.config.game.width);
        std::array::from_fn(|i| ((i as i32 + 1) * (width - 1)) / 6)
    }

    pub fn speed(&self) -> f64 {
        (1.0 + f64::from(self.round - 1) * self.config.game.speed_increase)
            .min(self.config.game.max_speed)
    }

    pub fn start(&mut self) {
        if self.state == State::Ready {
            self.state = State::Playing;
            self.message = "Reach an empty home (^).".into();
        }
    }

    pub fn action(&mut self, action: Action) {
        match action {
            Action::Restart => *self = Self::new(self.config.clone()),
            Action::Pause => match self.state {
                State::Playing => self.state = State::Paused,
                State::Paused => self.state = State::Playing,
                _ => {}
            },
            Action::Left | Action::Right | Action::Up | Action::Down
                if self.state == State::Playing =>
            {
                match action {
                    Action::Left => self.x = (self.x - 1).max(0),
                    Action::Right => {
                        self.x = (self.x + 1).min(i32::from(self.config.game.width) - 1);
                    }
                    Action::Up => self.y = (self.y + 1).min(self.home_y()),
                    Action::Down => self.y = self.y.saturating_sub(1),
                    _ => unreachable!(),
                }
                if self.y > self.furthest_row {
                    self.score += 10;
                    self.furthest_row = self.y;
                }
                self.check_position();
            }
            _ => {}
        }
    }

    fn reset_frog(&mut self) {
        self.x = i32::from(self.config.game.width / 2);
        self.y = 0;
        self.remaining_ms = self.config.game.crossing_seconds * 1000;
        self.furthest_row = 0;
    }

    fn die(&mut self, reason: &str) {
        self.lives -= 1;
        self.message = reason.into();
        self.reset_frog();
        if self.lives == 0 {
            self.state = State::GameOver;
        }
    }

    // Returns true when a death or successful crossing ended this simulation step.
    fn check_position(&mut self) -> bool {
        let width = i32::from(self.config.game.width);
        if self.x < 0 || self.x >= width {
            self.die("Your log carried you off the edge!");
            return true;
        }
        if self.y == self.home_y() {
            let home = self.home_columns().iter().position(|&x| x == self.x);
            if let Some(i) = home.filter(|&i| !self.homes[i]) {
                self.homes[i] = true;
                self.score += 50 + u64::from(self.remaining_ms / 1000);
                self.message = "Home safe! Aim for another empty slot.".into();
                self.reset_frog();
                if self.homes.iter().all(|&filled| filled) {
                    self.score += 500;
                    self.round = self.round.saturating_add(1);
                    self.homes.fill(false);
                    self.reset_lanes();
                    self.message = format!("Round {}! The lanes are faster.", self.round);
                }
            } else {
                self.die("That home is occupied or blocked!");
            }
            return true;
        }
        if self.y == 0 {
            return false;
        }
        let lane = &self.lanes[self.y - 1];
        let occupied = lane.occupied(self.x, width);
        if lane.spec.kind == Kind::Road && occupied {
            self.die("Hit by traffic!");
            return true;
        }
        if lane.spec.kind == Kind::River && !occupied {
            self.die("Splash! Land on a log to cross the river.");
            return true;
        }
        false
    }

    pub fn tick(&mut self) {
        if self.state != State::Playing {
            return;
        }
        self.remaining_ms = self.remaining_ms.saturating_sub(STEP_MS);
        if self.remaining_ms == 0 {
            self.die("Time ran out!");
            return;
        }
        let width = i32::from(self.config.game.width);
        let speed = self.speed();
        for index in 0..self.lanes.len() {
            if self.lanes[index].spec.kind == Kind::Safe {
                continue;
            }
            self.lanes[index].elapsed_ms += f64::from(STEP_MS);
            let interval = f64::from(self.lanes[index].spec.interval_ms.unwrap()) / speed;
            while self.lanes[index].elapsed_ms >= interval {
                let lane = &mut self.lanes[index];
                lane.elapsed_ms -= interval;
                let direction = lane.spec.direction.unwrap().step();
                lane.phase = (lane.phase + direction).rem_euclid(width);
                if self.y == index + 1 && lane.spec.kind == Kind::River {
                    self.x += direction;
                }
                if self.check_position() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Direction;

    fn single_lane(kind: Kind) -> Game {
        let mut config = Config::default();
        config.lanes.truncate(1);
        config.lanes[0].kind = kind;
        config.lanes[0].direction = Some(Direction::Right);
        config.lanes[0].length = Some(3);
        config.lanes[0].gap = Some(8);
        config.lanes[0].interval_ms = Some(100);
        config.lanes[0].offset = Some(0);
        if kind == Kind::Safe {
            config.lanes[0] = Config::parse("[[lanes]]\nkind = 'safe'")
                .unwrap()
                .lanes
                .remove(0);
        }
        let mut game = Game::new(config);
        game.start();
        game
    }

    fn advance(game: &mut Game, ticks: usize) {
        for _ in 0..ticks {
            game.tick();
        }
    }

    #[test]
    fn boundaries_and_forward_scoring() {
        let mut game = single_lane(Kind::Safe);
        game.x = 0;
        game.action(Action::Left);
        assert_eq!(game.x, 0);
        game.action(Action::Down);
        assert_eq!(game.y, 0);
        game.x = 40;
        game.action(Action::Right);
        assert_eq!(game.x, 40);
        game.action(Action::Up);
        game.action(Action::Down);
        game.action(Action::Up);
        assert_eq!(game.score, 10);
    }

    #[test]
    fn traffic_collides_on_input_and_on_tick() {
        let mut game = single_lane(Kind::Road);
        game.x = 1;
        game.action(Action::Up);
        assert_eq!(game.lives, 2);
        game.x = 3;
        game.action(Action::Up);
        assert_eq!(game.lives, 2);
        advance(&mut game, 10);
        assert_eq!(game.lives, 1);
        assert_eq!(game.y, 0);
    }

    #[test]
    fn objects_wrap_in_both_directions() {
        let mut game = single_lane(Kind::Road);
        game.lanes[0].phase = 40;
        advance(&mut game, 10);
        assert_eq!(game.lanes[0].phase, 0);
        assert!(game.lanes[0].occupied(0, 41));
        game.lanes[0].spec.direction = Some(Direction::Left);
        advance(&mut game, 10);
        assert_eq!(game.lanes[0].phase, 40);
        assert!(game.lanes[0].occupied(40, 41));
    }

    #[test]
    fn water_logs_and_drifting_off_either_edge() {
        let mut game = single_lane(Kind::River);
        game.x = 4;
        game.action(Action::Up);
        assert_eq!(game.lives, 2);
        game.x = 1;
        game.action(Action::Up);
        advance(&mut game, 10);
        assert_eq!((game.x, game.y, game.lives), (2, 1, 2));
        game.x = 40;
        game.lanes[0].phase = 40;
        advance(&mut game, 10);
        assert_eq!((game.y, game.lives), (0, 1));
        game.x = 0;
        game.y = 1;
        game.lanes[0].phase = 0;
        game.lanes[0].spec.direction = Some(Direction::Left);
        advance(&mut game, 10);
        assert_eq!(game.state, State::GameOver);
    }

    #[test]
    fn timeout_pause_and_restart() {
        let mut game = single_lane(Kind::Road);
        game.remaining_ms = 20;
        game.action(Action::Pause);
        advance(&mut game, 100);
        assert_eq!(game.remaining_ms, 20);
        assert_eq!(game.lanes[0].phase, 0);
        game.action(Action::Pause);
        advance(&mut game, 2);
        assert_eq!(game.lives, 2);
        assert_eq!(game.remaining_ms, 60_000);
        game.score = 123;
        game.homes[0] = true;
        game.action(Action::Restart);
        assert_eq!(game.state, State::Ready);
        assert_eq!(game.score, 0);
        assert_eq!(game.lives, 3);
        assert_eq!(game.homes, [false; 5]);
    }

    #[test]
    fn home_scoring_occupancy_and_progression() {
        let mut game = single_lane(Kind::Safe);
        let homes = game.home_columns();
        game.x = homes[0];
        game.action(Action::Up);
        game.action(Action::Up);
        assert_eq!(game.score, 130); // Two rows, home, and 60 seconds.
        assert!(game.homes[0]);
        assert_eq!(game.y, 0);
        game.x = homes[0];
        game.action(Action::Up);
        game.action(Action::Up);
        assert_eq!(game.lives, 2);
        assert!(game.homes[0]);
        for home in &homes[1..] {
            game.x = *home;
            game.action(Action::Up);
            game.action(Action::Up);
        }
        assert_eq!(game.round, 2);
        assert_eq!(game.homes, [false; 5]);
        assert!((game.speed() - 1.1).abs() < 0.001);
        assert_eq!(game.score, 1170);
        game.round = 100;
        assert_eq!(game.speed(), 3.0);
    }

    #[test]
    fn blocked_home_costs_a_life_and_game_over_stops_ticks() {
        let mut game = single_lane(Kind::Safe);
        game.lives = 1;
        game.x = 0;
        game.action(Action::Up);
        game.action(Action::Up);
        assert_eq!(game.state, State::GameOver);
        let remaining = game.remaining_ms;
        advance(&mut game, 100);
        assert_eq!(game.remaining_ms, remaining);
        game.action(Action::Up);
        assert_eq!(game.y, 0);
    }
}
