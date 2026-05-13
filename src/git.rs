use anyhow::Result;
use console::style;

use crate::paths::ProjectPaths;

pub fn init(paths: &ProjectPaths) -> Result<()> {
    println!("\n{}", style("── Git initialisation ──").bold().cyan());

    run_git(&["init", "-q", "--initial-branch=main"], &paths.dir)?;
    run_git(&["add", "-A"], &paths.dir)?;

    let message = format!(
        "chore: initial secure project setup via airlock v{}\n\
         \n\
         - Devcontainer with network and filesystem isolation\n\
         - pnpm/uv with scripts disabled by default (if applicable)\n\
         - Secrets mounted read-only from ~/.secrets/\n\
         - All Docker capabilities dropped",
        env!("CARGO_PKG_VERSION")
    );

    run_git(&["commit", "-q", "-m", &message], &paths.dir)?;

    println!("  {} Initial commit created", style("✔").green());
    Ok(())
}

fn run_git(args: &[&str], dir: &std::path::Path) -> Result<()> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()?;

    if !status.success() {
        anyhow::bail!(
            "git {} failed with code {:?}",
            args.join(" "),
            status.code()
        );
    }
    Ok(())
}
