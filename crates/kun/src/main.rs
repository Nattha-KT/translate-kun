use clap::{CommandFactory, Parser};

/// A command-line translator for developers.
#[derive(Parser)]
#[command(name = "kun", version = kun_core::version())]
struct Cli {}

fn main() {
    // No subcommands yet: parse flags (--help / --version are handled by clap),
    // then print help to stderr and exit with code 2.
    let _cli = Cli::parse();
    let _ = Cli::command().write_help(&mut std::io::stderr());
    eprintln!();
    std::process::exit(2);
}
