use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use console::style;
use inquire::{Confirm, Select, Text};

use crate::detect;
use crate::paths::{default_projects_dir, expand_tilde, new_uuid, slug_for_subdir, ProjectPaths};
use crate::retrofit::{self, RetrofitOutcome, RetrofitTarget, TargetStatus};
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

pub struct RetrofitArgs {
    pub stack: Option<String>,
    pub secrets: Option<bool>,
    pub force: bool,
    pub project_dir: PathBuf,
    pub secrets_dir: PathBuf,
}

pub fn gather_retrofit_targets(args: RetrofitArgs) -> Result<Vec<RetrofitTarget>> {
    println!("\n{}", style("── Retrofit configuration ──").bold().cyan());
    println!();
    println!(
        "  {} {}",
        style("Target").bold(),
        args.project_dir.display()
    );
    println!();

    let root_stack = detect::detect_stack(&args.project_dir).unwrap_or("minimal");

    let mono_stack_mode = root_stack != "minimal" || args.stack.is_some();

    if mono_stack_mode {
        // Reject ambiguous --stack on a manifest-less monorepo root
        if args.stack.is_some() && root_stack == "minimal" {
            let subs = detect::detect_substacks(&args.project_dir);
            if !subs.is_empty() {
                let names: Vec<String> = subs
                    .iter()
                    .map(|(p, _)| {
                        p.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("?")
                            .to_string()
                    })
                    .collect();
                bail!(
                    "--stack requires a specific PATH, not a monorepo with multiple subdirs (found: {})",
                    names.join(", ")
                );
            }
        }

        let target = build_single_target(
            args.project_dir.clone(),
            display_path_for_root(&args.project_dir),
            args.stack.as_deref(),
            args.secrets,
            &args.secrets_dir,
            None,
        )?;
        return Ok(vec![target]);
    }

    // Multi sub-dir mode
    let subs = detect::detect_substacks(&args.project_dir);
    if subs.is_empty() {
        bail!(
            "No stack detected at {} and no sub-directories with a recognised stack — use `airlock new` instead or pass `--stack`.",
            args.project_dir.display()
        );
    }

    println!(
        "  {} detected {} candidate sub-{}",
        style("ℹ").cyan(),
        subs.len(),
        if subs.len() == 1 {
            "directory"
        } else {
            "directories"
        }
    );
    println!();

    let secrets_default = args.secrets.unwrap_or(false);
    let mut targets: Vec<RetrofitTarget> = Vec::new();

    for (subdir, stack_id) in subs {
        let subdir_name = subdir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();

        let stack_meta = stack_meta_for(stack_id)?;

        let prompt = format!(
            "Detected {} in {}/. Apply retrofit?",
            stack_meta.label, subdir_name
        );
        let apply = Confirm::new(&format!("{}", style(prompt).bold()))
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
        if !apply {
            continue;
        }

        let needs_secrets = match args.secrets {
            Some(v) => v,
            None => Confirm::new(&format!(
                "{}",
                style(format!("  Configure secrets for {}/?", subdir_name)).bold()
            ))
            .with_default(secrets_default)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?,
        };

        let base_slug = slug_for_subdir(&args.project_dir, &subdir_name);
        let uuid = determine_uuid(&subdir);
        let paths = ProjectPaths::for_existing_with_slug(
            subdir.clone(),
            &args.secrets_dir,
            &uuid,
            &base_slug,
        )?;

        let display_path = format!(
            "{}/{}/",
            display_path_for_root(&args.project_dir),
            subdir_name
        );

        targets.push(RetrofitTarget {
            paths,
            stack: stack_meta,
            needs_secrets,
            display_path,
        });
    }

    if targets.is_empty() {
        bail!("Nothing to do — all sub-directories were declined.");
    }

    Ok(targets)
}

fn build_single_target(
    project_dir: PathBuf,
    display_path: String,
    forced_stack: Option<&str>,
    secrets_arg: Option<bool>,
    secrets_dir: &Path,
    explicit_slug: Option<&str>,
) -> Result<RetrofitTarget> {
    let stacks = StackRegistry::all();
    let non_interactive = forced_stack.is_some() || secrets_arg.is_some();

    let stack = match forced_stack {
        Some(id) => stacks.iter().find(|s| s.id == id).cloned().ok_or_else(|| {
            let valid: Vec<&str> = stacks.iter().map(|s| s.id.as_str()).collect();
            anyhow::anyhow!(
                "Unknown stack '{}'. Available stacks: {}",
                id,
                valid.join(", ")
            )
        })?,
        None => {
            let detected = detect::detect_stack(&project_dir).unwrap_or("minimal");
            let detected_meta = stacks
                .iter()
                .find(|s| s.id == detected)
                .cloned()
                .unwrap_or_else(|| stacks.iter().find(|s| s.id == "minimal").cloned().unwrap());

            println!(
                "  Detected stack: {}",
                style(&detected_meta.label).bold().green()
            );

            let confirmed = Confirm::new(&format!("{}", style("Use this stack?").bold()))
                .with_default(true)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;

            if confirmed {
                detected_meta
            } else {
                let labels: Vec<&str> = stacks.iter().map(|s| s.label.as_str()).collect();
                let choice = Select::new(&format!("{}", style("Tech stack").bold()), labels)
                    .prompt()
                    .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
                stacks.iter().find(|s| s.label == choice).cloned().unwrap()
            }
        }
    };

    let needs_secrets = match secrets_arg {
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

    let uuid = determine_uuid(&project_dir);
    let paths = match explicit_slug {
        Some(slug) => {
            ProjectPaths::for_existing_with_slug(project_dir.clone(), secrets_dir, &uuid, slug)?
        }
        None => ProjectPaths::for_existing(project_dir.clone(), secrets_dir, &uuid)?,
    };

    Ok(RetrofitTarget {
        paths,
        stack,
        needs_secrets,
        display_path,
    })
}

fn stack_meta_for(id: &str) -> Result<StackMeta> {
    StackRegistry::all()
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| anyhow::anyhow!("Unknown stack '{}'", id))
}

fn determine_uuid(target_dir: &Path) -> String {
    let dc = target_dir.join(".devcontainer").join("devcontainer.json");
    if dc.exists() {
        match retrofit::read_existing_uuid(&dc) {
            Some(u) => return u,
            None => {
                println!(
                    "  {} existing .devcontainer/ has no UUID prefix — generating a new one (previous secrets path may be orphaned)",
                    style("⚠").yellow()
                );
            }
        }
    }
    new_uuid()
}

fn display_path_for_root(root: &Path) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| root.display().to_string())
}

pub fn print_retrofit_outcomes(outcomes: &[RetrofitOutcome]) {
    println!("\n{}", style("── Retrofit summary ──").bold().cyan());
    println!();

    let mut done = 0usize;
    let mut skipped = 0usize;

    for o in outcomes {
        let path = &o.target.display_path;
        let stack_id = &o.target.stack.id;
        let name = &o.target.paths.name;

        match &o.status {
            TargetStatus::Done => {
                done += 1;
                println!(
                    "  {} {}   {}   {}",
                    style("✔").green(),
                    style(path).bold(),
                    style(stack_id).dim(),
                    style(name).cyan(),
                );
                if o.target.needs_secrets {
                    println!(
                        "      🔑 secrets → {}",
                        style(o.target.paths.secret_file.display()).dim()
                    );
                }
            }
            TargetStatus::Skipped(reason) => {
                skipped += 1;
                println!(
                    "  {} {}   skipped ({})",
                    style("⚠").yellow(),
                    style(path).bold(),
                    reason
                );
            }
        }
    }

    println!();
    println!("  {} retrofitted, {} skipped.", done, skipped);
    println!();
    println!("  {}", style("Next steps:").dim());
    println!("  1. Open the subdir in VS Code → 'Reopen in Container'");
    println!("  2. Edit secrets at the paths shown above");
    println!();
}
