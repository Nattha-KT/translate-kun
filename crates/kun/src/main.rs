mod app;
mod args;
mod error;
mod guard;
mod input;
mod providers;
mod settings;

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

use args::Cli;
use error::CliError;

fn main() -> ExitCode {
    // clap handles --help and --version, and exits with code 2 on bad flags.
    let cli = Cli::parse();

    match app::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error);
            ExitCode::from(error.exit_code())
        }
    }
}

/// Prints an error on stderr. With no text at all, print the help instead.
fn report(error: &CliError) {
    if matches!(error, CliError::NoText) {
        let _ = Cli::command().write_help(&mut std::io::stderr());
        eprintln!();
    } else {
        eprintln!("error: {error}");
    }
}
