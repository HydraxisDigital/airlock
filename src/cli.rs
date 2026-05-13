use std::path::PathBuf;

use anyhow::Result;
use console::style;
use inquire::{Confirm, Select, Text};

use crate::paths::{default_projects_dir, expand_tilde};
use crate::stack::{StackMeta, StackRegistry};

pub struct ProjectConfig {
    pub name: String,
    pub stack: StackMeta,
    pub needs_secrets: bool,
    pub init_git: bool,
    pub open_vscode: bool,
    pub location: PathBuf,
}

pub struct CliArgs {
    pub name: Option<String>,
    pub stack: Option<String>,
    pub secrets: Option<bool>,
    pub no_git: bool,
    pub no_vscode: bool,
    pub location: Option<PathBuf>,
}

pub fn gather_config(args: CliArgs) -> Result<ProjectConfig> {
    println!("\n{}", style("── Project configuration ──").bold().cyan());
    println!();

    // Non-interactive mode when both name AND stack are provided as args
    let non_interactive = args.name.is_some() && args.stack.is_some();

    // Project name
    let name = match args.name {
        Some(n) => n,
        None => Text::new(&format!("{}", style("Project name").bold()))
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?,
    };
    if name.trim().is_empty() {
        anyhow::bail!("Project name is required.");
    }
    let name = name.trim().to_string();

    // Stack selection
    let stacks = StackRegistry::all();
    let stack = match args.stack {
        Some(ref id) => stacks
            .iter()
            .find(|s| s.id == *id)
            .cloned()
            .ok_or_else(|| {
                let valid: Vec<&str> = stacks.iter().map(|s| s.id.as_str()).collect();
                anyhow::anyhow!(
                    "Unknown stack '{}'. Available stacks: {}",
                    id,
                    valid.join(", ")
                )
            })?,
        None => {
            let labels: Vec<&str> = stacks.iter().map(|s| s.label.as_str()).collect();
            let choice = Select::new(&format!("{}", style("Tech stack").bold()), labels)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
            stacks.iter().find(|s| s.label == choice).cloned().unwrap()
        }
    };

    // Secrets (default false in non-interactive mode)
    let needs_secrets = match args.secrets {
        Some(v) => v,
        None if non_interactive => false,
        None => Confirm::new(&format!(
            "{}",
            style("Will this project need secrets (API keys, tokens)?").bold()
        ))
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?,
    };

    // Git (default true in non-interactive mode, unless --no-git)
    let init_git = if args.no_git {
        false
    } else if non_interactive {
        true
    } else {
        Confirm::new(&format!("{}", style("Initialise a git repo?").bold()))
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?
    };

    let open_vscode = if args.no_vscode {
        false
    } else if non_interactive {
        true
    } else {
        Confirm::new(&format!("{}", style("Open in VS Code after setup?").bold()))
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?
    };

    // Location — explicit CLI flag wins; otherwise cwd in non-interactive, prompt otherwise.
    let location = match args.location {
        Some(p) => p,
        None if non_interactive => std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("Cannot determine current directory: {}", e))?,
        None => prompt_for_location()?,
    };

    Ok(ProjectConfig {
        name,
        stack,
        needs_secrets,
        init_git,
        open_vscode,
        location,
    })
}

fn prompt_for_location() -> Result<PathBuf> {
    let cwd = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("Cannot determine current directory: {}", e))?;
    let global_dir = default_projects_dir();

    let cwd_label = format!("Current directory  ({})", cwd.display());
    let global_label = format!("Global             ({})", global_dir.display());
    let other_label = String::from("Other path...");

    let labels = vec![cwd_label.clone(), global_label.clone(), other_label.clone()];
    let choice = Select::new(
        &format!("{}", style("Where to create the project?").bold()),
        labels,
    )
    .prompt()
    .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;

    if choice == cwd_label {
        Ok(cwd)
    } else if choice == global_label {
        Ok(global_dir)
    } else {
        let input = Text::new(&format!("{}", style("Path").bold()))
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
        if input.trim().is_empty() {
            anyhow::bail!("Path is required.");
        }
        Ok(expand_tilde(&input))
    }
}

pub fn print_summary(config: &ProjectConfig, dir: &std::path::Path) {
    println!("\n{}", style("── Summary ──").bold().cyan());
    println!();
    println!("  Project : {}", style(&config.name).bold());
    println!("  Dir     : {}", style(dir.display()).dim());
    println!("  Stack   : {}", style(&config.stack.label).bold());
    println!(
        "  Secrets : {}",
        if config.needs_secrets {
            style("yes").green().to_string()
        } else {
            style("no").dim().to_string()
        }
    );
    println!(
        "  Git     : {}",
        if config.init_git {
            style("yes").green().to_string()
        } else {
            style("no").dim().to_string()
        }
    );
    println!();
}

pub fn confirm_proceed() -> Result<bool> {
    Confirm::new(&format!("{}", style("Launch setup?").bold()))
        .with_default(true)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))
}
