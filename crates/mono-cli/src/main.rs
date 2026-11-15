mod commands;
mod context;

use clap::{Parser, Subcommand};

/// Monorepo management tool.
#[derive(Parser)]
#[command(name = "mono", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a target in every project that defines it.
    Run {
        /// Target name, for example `build`.
        target: String,
    },
    /// Print the project graph in dependency order.
    Graph,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run { target } => commands::run::run(&target),
        Command::Graph => commands::graph::graph(),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("mono: {err}");
            std::process::exit(2);
        }
    }
}
