mod commands;
mod context;
mod output;

use clap::{Parser, Subcommand};

use crate::commands::{adopt, disable, doctor, enable, fix, init, list};

#[derive(Parser)]
#[command(
    name = "skm",
    version,
    about = "Skills Manager CLI - manage AI assistant skills from the terminal",
    after_help = "Run 'skm <command> --help' for command-specific options."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize the Skills Manager config (first-run setup without the GUI)
    Init(init::Args),
    /// Move skills already installed in tool directories into the hub
    /// and replace them with links back to it
    Adopt(adopt::Args),
    /// List skills and their per-tool link status
    List(list::Args),
    /// Enable a skill for a tool (creates the symlink)
    Enable(enable::Args),
    /// Disable a skill for a tool (removes the symlink)
    Disable(disable::Args),
    /// Detect installed tools and report broken links
    Doctor(doctor::Args),
    /// Fix sync issues reported by doctor (requires --yes to apply)
    Fix(fix::Args),
}

fn main() {
    let cli = Cli::parse();

    let (result, json_mode) = match &cli.command {
        Command::Init(args) => (init::run(args), args.json),
        Command::Adopt(args) => (adopt::run(args), args.json),
        Command::List(args) => (list::run(args), args.json),
        Command::Enable(args) => (enable::run(args), false),
        Command::Disable(args) => (disable::run(args), false),
        Command::Doctor(args) => (doctor::run(args), args.json),
        Command::Fix(args) => (fix::run(args), args.json),
    };

    if let Err(error) = result {
        output::print_error(&error, json_mode);
        std::process::exit(1);
    }
}
