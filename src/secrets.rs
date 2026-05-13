use std::fs;
use std::os::unix::fs::PermissionsExt;

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
