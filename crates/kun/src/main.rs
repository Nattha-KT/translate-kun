mod app;
mod args;
mod error;
mod guard;
mod input;
mod providers;
mod settings;

use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};

use args::{Cli, describe_parse_error};
use error::CliError;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // --help and --version keep clap's own behaviour.
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp
                    | ErrorKind::DisplayVersion
                    | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) {
                error.exit();
            }
            // Any other error: clap's message could repeat a value the user
            // typed (maybe a secret), so print a fixed one instead.
            eprintln!("error: {}", describe_parse_error(&error));
            return ExitCode::from(2);
        }
    };

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
