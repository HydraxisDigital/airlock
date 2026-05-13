use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use console::style;

use airlock::cli;
use airlock::paths::{default_projects_dir, default_secrets_dir, expand_tilde, ProjectPaths};
use airlock::scaffold::ScaffoldOptions;
use airlock::{prerequisites, scaffold, stack};

#[derive(Parser)]
#[command(
    name = "airlock",
    version,
    about = "Secure scaffolding for isolated projects via Dev Containers",
    long_about = "airlock creates an isolated development project in a Dev Container (OrbStack + VS Code).\n\
                  Each project is confined: bridge network, cap-drop=ALL, read-only secrets."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new isolated project
    New {
        /// Project name (optional — interactive prompt if omitted)
        name: Option<String>,

        /// Tech stack (typescript, rust, python, solidity, solidity-ts, minimal)
        #[arg(long, short)]
        stack: Option<String>,

        /// Enable secrets (~/.secrets/<slug>.env)
        #[arg(long)]
        secrets: bool,

        /// Do not initialise a git repo
        #[arg(long)]
        no_git: bool,

        /// Do not open VS Code after setup
        #[arg(long)]
        no_vscode: bool,

        /// Create the project in the global projects directory (~/Projects or $PROJECTS_DIR)
        #[arg(long, short = 'g', conflicts_with = "path")]
        global: bool,

        /// Create the project inside this directory (will contain <slug>/)
        #[arg(long, value_name = "DIR", conflicts_with = "global")]
        path: Option<PathBuf>,
    },

    /// List available stacks
    Stacks,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    print_banner();

    match cli.command {
        Commands::New {
            name,
            stack,
            secrets,
            no_git,
            no_vscode,
            global,
            path,
        } => {
            prerequisites::check()?;

            let is_interactive = name.is_none() || stack.is_none();

            let location: Option<PathBuf> = if global {
                Some(default_projects_dir())
            } else if let Some(p) = path {
                let s = p.to_string_lossy().into_owned();
                Some(expand_tilde(&s))
            } else {
                None
            };

            let args = cli::CliArgs {
                name,
                stack,
                secrets: if secrets { Some(true) } else { None },
                no_git,
                no_vscode,
                location,
            };

            let config = cli::gather_config(args)?;

            let secrets_dir = default_secrets_dir();
            let paths = ProjectPaths::new(&config.name, &config.location, &secrets_dir)?;

            cli::print_summary(&config, &paths.dir);

            if is_interactive {
                let proceed = cli::confirm_proceed()?;
                if !proceed {
                    println!("  Cancelled.");
                    return Ok(());
                }
            }

            let opts = ScaffoldOptions {
                needs_secrets: config.needs_secrets,
                init_git: config.init_git,
                open_vscode: config.open_vscode,
            };

            scaffold::run(&paths, &config.stack, &opts)?;
        }

        Commands::Stacks => {
            println!("\n{}", style("Available stacks:").bold().cyan());
            println!();
            for s in stack::StackRegistry::all() {
                println!("  {} — {}", style(&s.id).bold(), s.label);
                if !s.vscode_extensions.is_empty() {
                    for ext in &s.vscode_extensions {
                        println!("      {}", style(ext).dim());
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    print!("  ");
    print!("{}", style("airlock").bold().cyan());
    print!("{}", style(format!(" v{}", version)).bold().white());
    print!("{}", style("  ·  ").dim());
    println!("{}", style("secure dev sandbox").dim());
}
