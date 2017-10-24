#[macro_use]
extern crate serde_derive;

extern crate rand;
extern crate tui;
extern crate termion;
extern crate toml;
extern crate taxi;

mod configuration;
mod replay;
mod runner;
mod random_solver;

use std::env;
use std::io;
use std::fmt;
use std::convert::From;

use termion::event;
use termion::input::TermRead;

use configuration::Configuration;
use replay::Replay;

use runner::{run_training_session, Probe, Runner};

use random_solver::RandomSolver;

use taxi::state::State;
use taxi::world::World;
use taxi::distribution::MeasureDistribution;

fn main() {

    if let Err(error) = run() {
        println!("Failed:\n{:?}", error);
    }
}

enum AppError {
    Configuration(configuration::Error),
    World(taxi::world::Error),
    InitialState(taxi::state::Error),
    Runner(runner::Error),
    Replay(io::Error),
}

impl From<configuration::Error> for AppError {
    fn from(error: configuration::Error) -> Self {
        AppError::Configuration(error)
    }
}

impl From<taxi::world::Error> for AppError {
    fn from(error: taxi::world::Error) -> Self {
        AppError::World(error)
    }
}

impl From<taxi::state::Error> for AppError {
    fn from(error: taxi::state::Error) -> Self {
        AppError::InitialState(error)
    }
}

impl From<runner::Error> for AppError {
    fn from(error: runner::Error) -> Self {
        AppError::Runner(error)
    }
}

impl fmt::Debug for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AppError::Configuration(ref config_error) => {
                write!(f, "Failed to read configuration:\n{:?}", config_error)
            }
            AppError::World(ref world_error) => {
                write!(f, "Failed to build world:\n{:?}", world_error)
            }
            AppError::InitialState(ref state_error) => {
                write!(f, "Failed to build initial state:\n{:?}", state_error)
            }
            AppError::Runner(ref runner_error) => {
                write!(f, "Failed to run trial:\n{:?}", runner_error)
            }
            AppError::Replay(ref replay_error) => {
                write!(f, "Failed to replay:\n{:?}", replay_error)
            }
        }
    }
}

fn run() -> Result<(), AppError> {

    let args: Vec<String> = env::args().collect();

    let config = if args.len() < 2 {
        Configuration::default()
    } else {
        Configuration::from_file(&args[1])?
    };

    let world = World::build_from_str(&config.world)?;
    let probes = build_probes(&config, &world)?;

    let mut distribution = MeasureDistribution::new();
    let mut solver = RandomSolver::new();

    for _ in 0..config.sessions {
        let num_steps = run_training_session(
            &world,
            &probes,
            config.max_trials,
            config.max_trial_steps,
            &mut solver,
        )?;

        distribution.add_value(num_steps as f64);
    }

    let (avg_steps, stddev_steps) = distribution.get_distribution();

    println!(
        "Finished {} sessions in {} average steps with stddev of {}.",
        config.sessions,
        avg_steps,
        stddev_steps
    );

    if let Some(replay_config) = config.replay {
        if let Some(_) = wait_for_input() {
            let replay_state = State::build(
                &world,
                replay_config.taxi_pos,
                replay_config.passenger_loc,
                replay_config.destination_loc,
            )?;

            let attempt = solver.attempt(&world, replay_state, replay_config.max_steps);

            let replay = Replay::new(&world, attempt);

            if let Err(error) = replay.run() {
                return Err(AppError::Replay(error));
            }
        }
    }

    Ok(())
}


fn build_probes(config: &Configuration, world: &World) -> Result<Vec<Probe>, AppError> {

    let mut probes = Vec::new();

    for probe_config in &config.probes {
        let state = State::build(
            &world,
            probe_config.taxi_pos,
            probe_config.passenger_loc,
            probe_config.destination_loc,
        )?;

        probes.push(Probe::new(state, probe_config.max_steps));
    }

    Ok(probes)
}

fn wait_for_input() -> Option<()> {
    println!("Press Enter to see replay.  q to exit.");

    loop {
        for c in io::stdin().keys() {

            match c {
                Ok(evt) => {
                    match evt {
                        event::Key::Char('q') |
                        event::Key::Char('Q') => return None,
                        event::Key::Char('\n') => return Some(()),
                        _ => (),
                    }
                }
                Err(_) => return None,
            }
        }
    }
}
