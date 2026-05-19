use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::Result;
use console::style;

use crate::paths::{default_age_recipients, ProjectPaths};

pub fn setup(paths: &ProjectPaths) -> Result<()> {
    println!("\n{}", style("── Configuring secrets ──").bold().cyan());

    let secrets_dir = paths.secret_file.parent().unwrap();
    fs::create_dir_all(secrets_dir)?;

    let mut perms = fs::metadata(secrets_dir)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(secrets_dir, perms)?;

    if !paths.secret_file.exists() {
        let content = format!(
            "# Secrets for {}\n\
             # Mounted read-only in the container at /run/secrets/env → /workspace/.env\n\
             # NEVER commit this file.\n\
             \n\
             # API_KEY=\n\
             # DATABASE_URL=\n\
             # RPC_URL=\n",
            paths.name
        );
        fs::write(&paths.secret_file, &content)?;

        let mut perms = fs::metadata(&paths.secret_file)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&paths.secret_file, perms)?;

        println!(
            "  {} Secrets file created: {}",
            style("✔").green(),
            style(paths.secret_file.display()).dim()
        );
    } else {
        println!(
            "  {} Existing secrets file kept: {}",
            style("⚠").yellow(),
            style(paths.secret_file.display()).dim()
        );
    }

    try_encrypt(paths)?;

    Ok(())
}

pub fn migrate_existing_env(project_dir: &Path, paths: &ProjectPaths) -> Result<()> {
    let src = project_dir.join(".env");
    if !src.exists() {
        return Ok(());
    }

    println!("\n{}", style("── Migrating existing .env ──").bold().cyan());

    if secret_file_has_content(&paths.secret_file) {
        println!(
            "  {} {} exists and {} is non-empty — manual merge needed.",
            style("⚠").yellow(),
            src.display(),
            paths.secret_file.display()
        );
        println!("     No files were modified.");
        return Ok(());
    }

    if let Some(parent) = paths.secret_file.parent() {
        fs::create_dir_all(parent)?;
    }

    if fs::rename(&src, &paths.secret_file).is_err() {
        // Fallback (e.g. cross-device EXDEV)
        fs::copy(&src, &paths.secret_file)?;
        fs::remove_file(&src)?;
    }

    let mut perms = fs::metadata(&paths.secret_file)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&paths.secret_file, perms)?;

    println!(
        "  {} Moved .env → {}",
        style("✔").green(),
        style(paths.secret_file.display()).dim()
    );

    if env_in_git_history(project_dir) {
        println!(
            "  {} {} was previously committed — it remains in git history.",
            style("⚠").yellow().bold(),
            src.display()
        );
        println!("     Consider rotating the secrets and rewriting history.");
    }

    ensure_env_in_gitignore(project_dir)?;

    Ok(())
}

fn secret_file_has_content(secret_file: &Path) -> bool {
    let Ok(content) = fs::read_to_string(secret_file) else {
        return false;
    };
    content
        .lines()
        .map(str::trim)
        .any(|l| !l.is_empty() && !l.starts_with('#'))
}

fn env_in_git_history(project_dir: &Path) -> bool {
    if !project_dir.join(".git").exists() {
        return false;
    }
    let output = std::process::Command::new("git")
        .args(["log", "--all", "--", ".env"])
        .current_dir(project_dir)
        .output();
    match output {
        Ok(out) => out.status.success() && !out.stdout.is_empty(),
        Err(_) => false,
    }
}

fn ensure_env_in_gitignore(project_dir: &Path) -> Result<()> {
    let gitignore = project_dir.join(".gitignore");
    let existing = fs::read_to_string(&gitignore).unwrap_or_default();
    let already = existing.lines().map(str::trim).any(|l| l == ".env");
    if already {
        return Ok(());
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(b".env\n")?;
    println!(
        "  {} Added .env to {}",
        style("✔").green(),
        style(gitignore.display()).dim()
    );
    Ok(())
}

fn try_encrypt(paths: &ProjectPaths) -> Result<()> {
    if which::which("age").is_err() {
        return Ok(());
    }

    let recipients_file = default_age_recipients();
    if !recipients_file.exists() {
        return Ok(());
    }

    let encrypted = paths.secret_file.with_extension("env.age");

    let status = std::process::Command::new("age")
        .args([
            "-R",
            recipients_file.to_str().unwrap(),
            "-o",
            encrypted.to_str().unwrap(),
            paths.secret_file.to_str().unwrap(),
        ])
        .status()?;

    if status.success() {
        println!(
            "  {} Secrets encrypted → {}",
            style("✔").green(),
            style(encrypted.display()).dim()
        );
        println!(
            "     To decrypt: age -d -i ~/.age/key.txt {}",
            encrypted.display()
        );
    }

    Ok(())
}
