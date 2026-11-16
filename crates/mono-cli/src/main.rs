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
        /// Run only in projects affected by changes: the changed projects and the
        /// projects that depend on them.
        #[arg(long)]
        affected: bool,
        /// The revision that changes are measured against, with `--affected`.
        #[arg(long, default_value = "HEAD", requires = "affected")]
        base: String,
    },
    /// Print the project graph in dependency order.
    Graph,
    /// Show metadata about the projects of the workspace.
    Show {
        #[command(subcommand)]
        what: Show,
    },
}

#[derive(Subcommand)]
enum Show {
    /// List the names of all projects.
    Projects {
        /// Print a JSON array instead of one name per line.
        #[arg(long)]
        json: bool,
    },
    /// Show the metadata of one project: location, version, dependencies, and targets.
    Project {
        /// Project name, as printed by `mono show projects`.
        name: String,
        /// Print a JSON object instead of text.
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run { target, affected, base } => {
            commands::run::run(&target, affected.then_some(base.as_str()))
        }
        Command::Graph => commands::graph::graph(),
        Command::Show { what: Show::Projects { json } } => commands::show::projects(json),
        Command::Show { what: Show::Project { name, json } } => commands::show::project(&name, json),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("mono: {err}");
            std::process::exit(2);
        }
    }
}
