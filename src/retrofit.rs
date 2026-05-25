use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use console::style;

use crate::paths::ProjectPaths;
use crate::scaffold;
use crate::secrets;
use crate::stack::{StackChoice, StackMeta, StackRegistry};

pub struct RetrofitTarget {
    pub paths: ProjectPaths,
    pub stack: StackChoice,
    pub needs_secrets: bool,
    pub display_path: String,
}

pub enum TargetStatus {
    Done,
    Skipped(String),
}

pub struct RetrofitOutcome {
    pub target: RetrofitTarget,
    pub status: TargetStatus,
}

pub fn run_many(targets: Vec<RetrofitTarget>, force: bool) -> Result<Vec<RetrofitOutcome>> {
    let mut outcomes = Vec::with_capacity(targets.len());
    for target in targets {
        let outcome = run_single(target, force)?;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

fn run_single(target: RetrofitTarget, force: bool) -> Result<RetrofitOutcome> {
    let devcontainer_dir = target.paths.dir.join(".devcontainer");
    if devcontainer_dir.exists() && !force {
        return Ok(RetrofitOutcome {
            target,
            status: TargetStatus::Skipped(".devcontainer/ already exists, use --force".to_string()),
        });
    }

    println!(
        "\n{} {}",
        style("──").bold().cyan(),
        style(format!("Retrofitting {}", target.display_path))
            .bold()
            .cyan()
    );

    scaffold::write_devcontainer_files(&target.paths, &target.stack, target.needs_secrets)?;
    println!("  {} Dockerfile", style("✔").green());
    println!("  {} devcontainer.json", style("✔").green());
    println!("  {} post-create.sh", style("✔").green());

    write_security_md_if_missing(&target.paths, &target.stack.meta)?;
    merge_gitignore(&target.paths)?;

    if target.needs_secrets {
        secrets::setup(&target.paths)?;
        secrets::migrate_existing_env(&target.paths.dir, &target.paths)?;
    }

    Ok(RetrofitOutcome {
        target,
        status: TargetStatus::Done,
    })
}

pub fn read_existing_uuid(devcontainer_json: &Path) -> Option<String> {
    let content = fs::read_to_string(devcontainer_json).ok()?;
    let json_start = content.find('{')?;
    let value: serde_json::Value = serde_json::from_str(&content[json_start..]).ok()?;
    let name = value.get("name")?.as_str()?;
    let (prefix, _rest) = name.split_once('-')?;
    if prefix.len() == 4
        && prefix
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        Some(prefix.to_string())
    } else {
        None
    }
}

fn write_security_md_if_missing(paths: &ProjectPaths, stack: &StackMeta) -> Result<()> {
    let dest = paths.dir.join("SECURITY.md");
    if dest.exists() {
        println!(
            "  {} SECURITY.md skipped — file exists",
            style("⚠").yellow()
        );
        return Ok(());
    }

    let template = StackRegistry::get_common_file("SECURITY.md.tmpl").unwrap_or("");
    let content = template
        .replace("${PROJECT_NAME}", &paths.name)
        .replace("${AUDIT_COMMAND}", &stack.audit_command)
        .replace("${TREE_COMMAND}", &stack.tree_command);
    fs::write(&dest, content)?;
    println!("  {} SECURITY.md", style("✔").green());
    Ok(())
}

fn merge_gitignore(paths: &ProjectPaths) -> Result<()> {
    let common = StackRegistry::get_common_file("gitignore").unwrap_or("");
    let gitignore = paths.dir.join(".gitignore");

    if !gitignore.exists() {
        fs::write(&gitignore, common)?;
        println!("  {} .gitignore", style("✔").green());
        return Ok(());
    }

    let existing = fs::read_to_string(&gitignore).unwrap_or_default();
    let existing_lines: std::collections::HashSet<&str> = existing
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    let to_append: Vec<&str> = common
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !existing_lines.contains(trimmed)
        })
        .collect();

    if to_append.is_empty() {
        println!(
            "  {} .gitignore already has all required entries",
            style("✔").green()
        );
        return Ok(());
    }

    let mut file = fs::OpenOptions::new().append(true).open(&gitignore)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(b"\n# Added by airlock retrofit\n")?;
    for line in &to_append {
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    println!(
        "  {} .gitignore — appended {} entries",
        style("✔").green(),
        to_append.len()
    );
    Ok(())
}
