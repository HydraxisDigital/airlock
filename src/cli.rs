use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::ValueEnum;
use console::style;
use inquire::{Confirm, Select, Text};

use crate::detect;
use crate::paths::{default_projects_dir, expand_tilde, new_uuid, slug_for_subdir, ProjectPaths};
use crate::retrofit::{self, RetrofitOutcome, RetrofitTarget, TargetStatus};
use crate::scaffold::MonorepoTarget;
use crate::stack::{LanguageVersion, StackChoice, StackMeta, StackRegistry};

pub struct ProjectConfig {
    pub name: String,
    pub stack: StackChoice,
    pub needs_secrets: bool,
    pub init_git: bool,
    pub open_vscode: bool,
    pub location: PathBuf,
}

pub struct MonorepoConfig {
    pub name: String,
    pub targets: Vec<MonorepoTarget>,
    pub init_git: bool,
    pub open_vscode: bool,
    pub location: PathBuf,
}

pub enum NewLayout {
    SingleStack,
    Monorepo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum RetrofitLayout {
    /// Create one .devcontainer/ at the project root.
    Root,
    /// Create one .devcontainer/ per detected sub-project.
    Subprojects,
}

pub struct CliArgs {
    pub name: Option<String>,
    pub stack: Option<String>,
    pub secrets: Option<bool>,
    pub no_git: bool,
    pub no_vscode: bool,
    pub location: Option<PathBuf>,
    pub node_version: Option<String>,
    pub python_version: Option<String>,
    pub rust_version: Option<String>,
}

pub struct MonorepoArgs {
    pub name: Option<String>,
    pub secrets: Option<bool>,
    pub no_git: bool,
    pub no_vscode: bool,
    pub location: Option<PathBuf>,
}

pub fn prompt_new_layout() -> Result<NewLayout> {
    let single = "Single stack";
    let monorepo = "Monorepo";
    let choice = Select::new(
        &format!("{}", style("Project layout").bold()),
        vec![single, monorepo],
    )
    .prompt()
    .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;

    if choice == monorepo {
        Ok(NewLayout::Monorepo)
    } else {
        Ok(NewLayout::SingleStack)
    }
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
    let stack_meta = match args.stack {
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

    let version_flags = LanguageVersionFlags {
        node: args.node_version.clone(),
        python: args.python_version.clone(),
        rust: args.rust_version.clone(),
    };
    let chosen_version =
        resolve_language_version(&stack_meta, &version_flags, non_interactive, None)?;
    let stack = StackChoice::with_version(stack_meta, chosen_version);

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

pub fn gather_monorepo_config(args: MonorepoArgs) -> Result<MonorepoConfig> {
    println!("\n{}", style("── Monorepo configuration ──").bold().cyan());
    println!();

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

    let targets = prompt_monorepo_targets(args.secrets)?;

    let init_git = if args.no_git {
        false
    } else {
        Confirm::new(&format!("{}", style("Initialise a git repo?").bold()))
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?
    };

    let open_vscode = if args.no_vscode {
        false
    } else {
        Confirm::new(&format!("{}", style("Open in VS Code after setup?").bold()))
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?
    };

    let location = match args.location {
        Some(p) => p,
        None => prompt_for_location()?,
    };

    Ok(MonorepoConfig {
        name,
        targets,
        init_git,
        open_vscode,
        location,
    })
}

fn prompt_monorepo_targets(secrets: Option<bool>) -> Result<Vec<MonorepoTarget>> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();

    loop {
        let target = prompt_monorepo_target(secrets, &mut seen)?;
        targets.push(target);

        let add_another = Confirm::new(&format!("{}", style("Add another sub-project?").bold()))
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
        if !add_another {
            break;
        }
    }

    Ok(targets)
}

fn prompt_monorepo_target(
    secrets: Option<bool>,
    seen: &mut HashSet<String>,
) -> Result<MonorepoTarget> {
    let name = loop {
        let input = Text::new(&format!("{}", style("Sub-project name").bold()))
            .prompt()
            .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
        let trimmed = input.trim();
        if let Err(e) = validate_target_name(trimmed, trimmed) {
            println!("  {} {}", style("⚠").yellow(), e);
            continue;
        }
        let slug = crate::paths::slugify(trimmed);
        if seen.contains(&slug) {
            println!(
                "  {} Duplicate target subdir '{}'",
                style("⚠").yellow(),
                trimmed
            );
            continue;
        }
        seen.insert(slug);
        break trimmed.to_string();
    };

    let stacks = StackRegistry::all();
    let labels: Vec<&str> = stacks.iter().map(|s| s.label.as_str()).collect();
    let choice = Select::new(&format!("{}", style("Tech stack").bold()), labels)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
    let stack_meta = stacks.iter().find(|s| s.label == choice).cloned().unwrap();

    let chosen_version =
        resolve_language_version(&stack_meta, &LanguageVersionFlags::default(), false, None)?;
    let stack = StackChoice::with_version(stack_meta, chosen_version);

    let needs_secrets = match secrets {
        Some(v) => v,
        None => Confirm::new(&format!(
            "{}",
            style(format!("Configure secrets for {}/?", name)).bold()
        ))
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?,
    };

    Ok(MonorepoTarget {
        name,
        stack,
        needs_secrets,
    })
}

pub fn parse_new_targets(specs: &[String], needs_secrets: bool) -> Result<Vec<MonorepoTarget>> {
    let mut targets = Vec::with_capacity(specs.len());
    let mut seen = HashSet::new();

    for spec in specs {
        let (name, stack_spec) = spec.split_once('=').ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid target '{}': expected <subdir>=<stack>[@<version>]",
                spec
            )
        })?;
        let name = name.trim();
        validate_target_name(name, spec)?;

        let slug = crate::paths::slugify(name);
        if !seen.insert(slug.clone()) {
            anyhow::bail!("Duplicate target subdir '{}'", name);
        }

        let stack_spec = stack_spec.trim();
        if stack_spec.is_empty() {
            anyhow::bail!("Invalid target '{}': stack is required", spec);
        }

        let (stack_spec, target_secrets) = parse_target_secret_override(stack_spec, spec)?;

        let (stack_id, version) = match stack_spec.split_once('@') {
            Some((id, v)) => {
                let id = id.trim();
                let v = v.trim();
                if id.is_empty() || v.is_empty() {
                    anyhow::bail!(
                        "Invalid target '{}': expected <subdir>=<stack>[@<version>]",
                        spec
                    );
                }
                (id, Some(v))
            }
            None => (stack_spec, None),
        };

        let stack_meta = stack_meta_for(stack_id)?;
        let chosen_version = match stack_meta.language_version.as_ref() {
            Some(lv) => match version {
                Some(v) => {
                    validate_supported(lv, v)?;
                    Some(v.to_string())
                }
                None => Some(lv.default.clone()),
            },
            None => {
                if version.is_some() {
                    anyhow::bail!(
                        "Target '{}' uses stack '{}' which does not support language versions",
                        name,
                        stack_id
                    );
                }
                None
            }
        };

        targets.push(MonorepoTarget {
            name: name.to_string(),
            stack: StackChoice::with_version(stack_meta, chosen_version),
            needs_secrets: target_secrets.unwrap_or(needs_secrets),
        });
    }

    Ok(targets)
}

fn parse_target_secret_override<'a>(
    stack_spec: &'a str,
    full_spec: &str,
) -> Result<(&'a str, Option<bool>)> {
    let Some((stack_part, option)) = stack_spec.split_once(':') else {
        return Ok((stack_spec, None));
    };
    let stack_part = stack_part.trim();
    let option = option.trim();
    if stack_part.is_empty() {
        anyhow::bail!("Invalid target '{}': stack is required", full_spec);
    }
    let secrets = match option {
        "secrets" => true,
        "no-secrets" => false,
        _ => {
            anyhow::bail!(
                "Invalid target '{}': expected :secrets or :no-secrets",
                full_spec
            );
        }
    };
    Ok((stack_part, Some(secrets)))
}

fn validate_target_name(name: &str, spec: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Invalid target '{}': subdir name is required", spec);
    }
    if name == "." || name == ".." || name.starts_with('.') {
        anyhow::bail!(
            "Invalid target '{}': subdir must not be hidden or relative",
            spec
        );
    }
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!(
            "Invalid target '{}': subdir must be a direct child name",
            spec
        );
    }
    if crate::paths::slugify(name).is_empty() {
        anyhow::bail!(
            "Invalid target '{}': subdir name does not produce a valid slug",
            spec
        );
    }
    Ok(())
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
    println!("  Stack   : {}", style(&config.stack.meta.label).bold());
    if let Some(v) = config.stack.effective_version() {
        let lang = config
            .stack
            .meta
            .language_version
            .as_ref()
            .map(|lv| lv.name.as_str())
            .unwrap_or("version");
        println!("  {:<7} : {}", lang, style(v).bold());
    }
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

pub fn print_monorepo_config_summary(config: &MonorepoConfig, dir: &std::path::Path) {
    println!("\n{}", style("── Summary ──").bold().cyan());
    println!();
    println!("  {:<7} : {}", "Project", style(&config.name).bold());
    println!("  {:<7} : {}", "Dir", style(dir.display()).dim());
    println!();
    for target in &config.targets {
        let stack_label = match target.stack.effective_version() {
            Some(version) => format!("{} {}", target.stack.meta.id, version),
            None => target.stack.meta.id.clone(),
        };
        println!(
            "  {:<7} : {:<16} {}",
            "Target",
            format!("{}/", target.name),
            style(stack_label).bold()
        );
        println!(
            "  {:<7} : {}",
            "Secrets",
            if target.needs_secrets {
                style("yes").green().to_string()
            } else {
                style("no").dim().to_string()
            }
        );
    }
    println!(
        "  {:<7} : {}",
        "Git",
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
    pub layout: Option<RetrofitLayout>,
    pub project_dir: PathBuf,
    pub secrets_dir: PathBuf,
    pub node_version: Option<String>,
    pub python_version: Option<String>,
    pub rust_version: Option<String>,
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
    let substacks = detect::detect_substacks(&args.project_dir);
    let explicit_layout = args.layout.is_some();
    let layout = resolve_retrofit_layout(args.layout, &substacks, args.stack.is_some())?;

    let mono_stack_mode = !matches!(layout, RetrofitLayout::Subprojects)
        && (matches!(layout, RetrofitLayout::Root)
            || root_stack != "minimal"
            || args.stack.is_some());

    let version_flags = LanguageVersionFlags {
        node: args.node_version.clone(),
        python: args.python_version.clone(),
        rust: args.rust_version.clone(),
    };

    if mono_stack_mode {
        let target = build_single_target(
            args.project_dir.clone(),
            display_path_for_root(&args.project_dir),
            args.stack.as_deref(),
            args.secrets,
            &args.secrets_dir,
            None,
            &version_flags,
        )?;
        return Ok(vec![target]);
    }

    if args.stack.is_some() {
        bail!("--stack cannot be used with --layout subprojects; pass a specific PATH or use --layout root");
    }

    if version_flags.any() {
        bail!(
            "version flags require a single-stack target (no --node-version/--python-version/--rust-version in multi-subdir mode)"
        );
    }

    // Multi sub-dir mode
    if substacks.is_empty() {
        bail!(
            "No stack detected at {} and no sub-directories with a recognised stack — use `airlock new` instead or pass `--stack`.",
            args.project_dir.display()
        );
    }

    println!(
        "  {} detected {} candidate sub-{}",
        style("ℹ").cyan(),
        substacks.len(),
        if substacks.len() == 1 {
            "directory"
        } else {
            "directories"
        }
    );
    println!();

    let secrets_default = args.secrets.unwrap_or(false);
    let mut targets: Vec<RetrofitTarget> = Vec::new();

    for (subdir, stack_id) in substacks {
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
        let apply = if explicit_layout {
            true
        } else {
            Confirm::new(&format!("{}", style(prompt).bold()))
                .with_default(true)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?
        };
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

        let detected_version =
            detect_version_for_stack(&subdir, &stack_meta).map(|v| v.to_string());
        let chosen_version = resolve_language_version(
            &stack_meta,
            &LanguageVersionFlags::default(),
            explicit_layout,
            detected_version.as_deref(),
        )?;
        let stack = StackChoice::with_version(stack_meta, chosen_version);

        targets.push(RetrofitTarget {
            paths,
            stack,
            needs_secrets,
            display_path,
        });
    }

    if targets.is_empty() {
        bail!("Nothing to do — all sub-directories were declined.");
    }

    Ok(targets)
}

fn resolve_retrofit_layout(
    layout: Option<RetrofitLayout>,
    substacks: &[(PathBuf, &'static str)],
    forced_stack: bool,
) -> Result<RetrofitLayout> {
    if let Some(layout) = layout {
        return Ok(layout);
    }

    if substacks.is_empty() || forced_stack {
        return Ok(RetrofitLayout::Root);
    }

    println!("  {} Monorepo detected:", style("ℹ").cyan());
    for (subdir, stack_id) in substacks {
        let name = subdir.file_name().and_then(|s| s.to_str()).unwrap_or("?");
        let label = StackRegistry::get(stack_id)
            .map(|s| s.label)
            .unwrap_or_else(|_| stack_id.to_string());
        println!("      - {}/ {}", name, style(label).dim());
    }
    println!();

    let root = "One devcontainer at project root";
    let subprojects = "One devcontainer per sub-project";
    let choice = Select::new(
        &format!("{}", style("Devcontainer layout").bold()),
        vec![root, subprojects],
    )
    .prompt()
    .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;

    if choice == subprojects {
        Ok(RetrofitLayout::Subprojects)
    } else {
        Ok(RetrofitLayout::Root)
    }
}

fn build_single_target(
    project_dir: PathBuf,
    display_path: String,
    forced_stack: Option<&str>,
    secrets_arg: Option<bool>,
    secrets_dir: &Path,
    explicit_slug: Option<&str>,
    version_flags: &LanguageVersionFlags,
) -> Result<RetrofitTarget> {
    let stacks = StackRegistry::all();
    let non_interactive = forced_stack.is_some() || secrets_arg.is_some();

    let stack_meta = match forced_stack {
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

    let detected_version =
        detect_version_for_stack(&project_dir, &stack_meta).map(|v| v.to_string());
    let chosen_version = resolve_language_version(
        &stack_meta,
        version_flags,
        non_interactive,
        detected_version.as_deref(),
    )?;
    let stack = StackChoice::with_version(stack_meta, chosen_version);

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

fn detect_version_for_stack(project_dir: &Path, stack: &StackMeta) -> Option<String> {
    let lv = stack.language_version.as_ref()?;
    detect::detect_language_version(project_dir, &lv.name)
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
        let stack_id = &o.target.stack.meta.id;
        let name = &o.target.paths.name;
        let stack_label = match o.target.stack.effective_version() {
            Some(v) => format!("{} {}", stack_id, v),
            None => stack_id.clone(),
        };

        match &o.status {
            TargetStatus::Done => {
                done += 1;
                println!(
                    "  {} {}   {}   {}",
                    style("✔").green(),
                    style(path).bold(),
                    style(&stack_label).dim(),
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
    if done == 0 {
        println!("  1. Re-run with --force for skipped targets you want to overwrite");
        println!();
        return;
    }

    let done_targets: Vec<&RetrofitOutcome> = outcomes
        .iter()
        .filter(|o| matches!(o.status, TargetStatus::Done))
        .collect();
    let multi_target = done_targets.len() > 1
        || done_targets
            .iter()
            .any(|o| o.target.display_path.ends_with('/'));
    if multi_target {
        println!("  1. Open each generated sub-project folder in VS Code");
        println!("  2. Run 'Reopen in Container' for each one");
    } else {
        println!("  1. Open the project folder in VS Code");
        println!("  2. Run 'Reopen in Container'");
    }

    if done_targets.iter().any(|o| o.target.needs_secrets) {
        println!("  3. Edit secrets at the paths shown above");
    }
    println!();
}

#[derive(Default, Clone)]
pub struct LanguageVersionFlags {
    pub node: Option<String>,
    pub python: Option<String>,
    pub rust: Option<String>,
}

impl LanguageVersionFlags {
    fn any(&self) -> bool {
        self.node.is_some() || self.python.is_some() || self.rust.is_some()
    }

    fn for_lang(&self, name: &str) -> Option<&str> {
        match name {
            "node" => self.node.as_deref(),
            "python" => self.python.as_deref(),
            "rust" => self.rust.as_deref(),
            _ => None,
        }
    }

    fn mismatched_flag(&self, stack_lang: Option<&str>) -> Option<&'static str> {
        let conflicts = |name: &str, value: &Option<String>| -> Option<&'static str> {
            if value.is_some() && Some(name) != stack_lang {
                match name {
                    "node" => Some("--node-version"),
                    "python" => Some("--python-version"),
                    "rust" => Some("--rust-version"),
                    _ => None,
                }
            } else {
                None
            }
        };
        conflicts("node", &self.node)
            .or_else(|| conflicts("python", &self.python))
            .or_else(|| conflicts("rust", &self.rust))
    }
}

fn validate_supported(lv: &LanguageVersion, version: &str) -> Result<()> {
    if !lv.supports(version) {
        anyhow::bail!(
            "Version '{}' is not supported for {}. Supported: {}",
            version,
            lv.name,
            lv.versions().join(", ")
        );
    }
    Ok(())
}

fn resolve_language_version(
    stack: &StackMeta,
    flags: &LanguageVersionFlags,
    non_interactive: bool,
    detected: Option<&str>,
) -> Result<Option<String>> {
    let lv = match stack.language_version.as_ref() {
        Some(v) => v,
        None => {
            if let Some(flag) = flags.mismatched_flag(None) {
                anyhow::bail!(
                    "{} is not applicable to stack '{}' (no versionable language)",
                    flag,
                    stack.id
                );
            }
            return Ok(None);
        }
    };

    if let Some(flag) = flags.mismatched_flag(Some(lv.name.as_str())) {
        anyhow::bail!(
            "{} is not applicable to stack '{}' (expected --{}-version)",
            flag,
            stack.id,
            lv.name
        );
    }

    if let Some(explicit) = flags.for_lang(&lv.name) {
        validate_supported(lv, explicit)?;
        return Ok(Some(explicit.to_string()));
    }

    if let Some(d) = detected {
        if lv.supports(d) {
            if non_interactive {
                println!(
                    "  {} detected {} {} (from project files)",
                    style("✔").green(),
                    lv.name,
                    d
                );
                return Ok(Some(d.to_string()));
            }
            let prompt = format!("Detected {} {}. Use this version?", lv.name, d);
            let confirmed = Confirm::new(&format!("{}", style(prompt).bold()))
                .with_default(true)
                .prompt()
                .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
            if confirmed {
                return Ok(Some(d.to_string()));
            }
        } else {
            println!(
                "  {} detected {} {}, not in supported list — falling back to default",
                style("⚠").yellow(),
                lv.name,
                d
            );
            if non_interactive {
                return Ok(Some(lv.default.clone()));
            }
        }
    } else if non_interactive {
        return Ok(Some(lv.default.clone()));
    }

    let label = format!("{} version", lv.name);
    let options: Vec<String> = lv.versions().iter().map(|s| s.to_string()).collect();
    let default_idx = options.iter().position(|v| v == &lv.default).unwrap_or(0);
    let choice = Select::new(&format!("{}", style(label).bold()), options.clone())
        .with_starting_cursor(default_idx)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Cancelled: {}", e))?;
    Ok(Some(choice))
}

#[allow(dead_code)]
fn _ensure_flags_consumed(flags: &LanguageVersionFlags) {
    let _ = flags.any();
}
