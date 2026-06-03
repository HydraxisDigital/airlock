use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use console::style;

use airlock::cli;
use airlock::paths::{default_projects_dir, default_secrets_dir, expand_tilde, ProjectPaths};
use airlock::scaffold::ScaffoldOptions;
use airlock::{prerequisites, retrofit, scaffold, stack};

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

        /// Monorepo target, repeatable: <subdir>=<stack>[@<version>]
        #[arg(long, value_name = "SUBDIR=STACK[@VERSION]")]
        target: Vec<String>,

        /// Tech stack (typescript, javascript, rust, python, solidity, solidity-ts, minimal)
        #[arg(long, short)]
        stack: Option<String>,

        /// Enable secrets (~/.airlock/<slug>/env)
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

        /// Node.js major version (typescript / javascript / solidity-ts stacks)
        #[arg(long, value_name = "VERSION")]
        node_version: Option<String>,

        /// Python version, e.g. 3.12 (python stack)
        #[arg(long, value_name = "VERSION")]
        python_version: Option<String>,

        /// Rust version (rust stack)
        #[arg(long, value_name = "VERSION")]
        rust_version: Option<String>,
    },

    /// Apply airlock devcontainer config to an existing project
    Retrofit {
        /// Project directory (default: current directory)
        path: Option<PathBuf>,

        /// Force the stack (skip auto-detection)
        #[arg(long, short)]
        stack: Option<String>,

        /// Enable secrets migration and mount
        #[arg(long, conflicts_with = "no_secrets")]
        secrets: bool,

        /// Disable secrets (useful in non-interactive mode)
        #[arg(long, conflicts_with = "secrets")]
        no_secrets: bool,

        /// Overwrite an existing .devcontainer/
        #[arg(long)]
        force: bool,

        /// Devcontainer layout for detected monorepos (root or subprojects)
        #[arg(long, value_enum)]
        layout: Option<cli::RetrofitLayout>,

        /// Do not open VS Code after setup
        #[arg(long)]
        no_vscode: bool,

        /// Node.js major version (typescript / javascript / solidity-ts stacks)
        #[arg(long, value_name = "VERSION")]
        node_version: Option<String>,

        /// Python version, e.g. 3.12 (python stack)
        #[arg(long, value_name = "VERSION")]
        python_version: Option<String>,

        /// Rust version (rust stack)
        #[arg(long, value_name = "VERSION")]
        rust_version: Option<String>,
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
            target,
            stack,
            secrets,
            no_git,
            no_vscode,
            global,
            path,
            node_version,
            python_version,
            rust_version,
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

            if target.is_empty() && stack.is_none() {
                match cli::prompt_new_layout()? {
                    cli::NewLayout::SingleStack => {}
                    cli::NewLayout::Monorepo => {
                        if node_version.is_some()
                            || python_version.is_some()
                            || rust_version.is_some()
                        {
                            anyhow::bail!(
                                "--node-version/--python-version/--rust-version are single-stack flags; use --target <subdir>=<stack>@<version> for non-interactive monorepos"
                            );
                        }

                        let args = cli::MonorepoArgs {
                            name,
                            secrets: if secrets { Some(true) } else { None },
                            no_git,
                            no_vscode,
                            location,
                        };
                        let config = cli::gather_monorepo_config(args)?;
                        let secrets_dir = default_secrets_dir();
                        let root_paths =
                            ProjectPaths::new(&config.name, &config.location, &secrets_dir)?;

                        cli::print_monorepo_config_summary(&config, &root_paths.dir);
                        let proceed = cli::confirm_proceed()?;
                        if !proceed {
                            println!("  Cancelled.");
                            return Ok(());
                        }

                        let opts = ScaffoldOptions {
                            needs_secrets: config.targets.iter().any(|target| target.needs_secrets),
                            init_git: config.init_git,
                            open_vscode: config.open_vscode,
                        };
                        scaffold::run_monorepo(&root_paths, &config.targets, &opts, &secrets_dir)?;
                        return Ok(());
                    }
                }
            }

            if !target.is_empty() {
                if stack.is_some() {
                    anyhow::bail!("--stack cannot be combined with --target");
                }
                if node_version.is_some() || python_version.is_some() || rust_version.is_some() {
                    anyhow::bail!(
                        "--node-version/--python-version/--rust-version cannot be combined with --target; use <subdir>=<stack>@<version>"
                    );
                }

                let name = name.ok_or_else(|| {
                    anyhow::anyhow!("Project name is required when using --target")
                })?;
                if name.trim().is_empty() {
                    anyhow::bail!("Project name is required.");
                }
                let name = name.trim().to_string();
                let location = location.unwrap_or(std::env::current_dir()?);
                let secrets_dir = default_secrets_dir();
                let root_paths = ProjectPaths::new(&name, &location, &secrets_dir)?;
                let targets = cli::parse_new_targets(&target, secrets)?;
                let opts = ScaffoldOptions {
                    needs_secrets: secrets,
                    init_git: !no_git,
                    open_vscode: !no_vscode,
                };
                scaffold::run_monorepo(&root_paths, &targets, &opts, &secrets_dir)?;
                return Ok(());
            }

            let args = cli::CliArgs {
                name,
                stack,
                secrets: if secrets { Some(true) } else { None },
                no_git,
                no_vscode,
                location,
                node_version,
                python_version,
                rust_version,
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

        Commands::Retrofit {
            path,
            stack,
            secrets,
            no_secrets,
            force,
            layout,
            no_vscode: _,
            node_version,
            python_version,
            rust_version,
        } => {
            prerequisites::check()?;

            let project_dir = match path {
                Some(p) => {
                    let s = p.to_string_lossy().into_owned();
                    expand_tilde(&s)
                }
                None => std::env::current_dir()?,
            }
            .canonicalize()?;

            let secrets_opt = if secrets {
                Some(true)
            } else if no_secrets {
                Some(false)
            } else {
                None
            };

            let secrets_dir = default_secrets_dir();

            let args = cli::RetrofitArgs {
                stack,
                secrets: secrets_opt,
                force,
                layout,
                project_dir: project_dir.clone(),
                secrets_dir: secrets_dir.clone(),
                node_version,
                python_version,
                rust_version,
            };

            let targets = cli::gather_retrofit_targets(args)?;
            let outcomes = retrofit::run_many(targets, force)?;
            cli::print_retrofit_outcomes(&outcomes);
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
