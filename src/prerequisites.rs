use anyhow::Result;
use console::style;

pub struct Prerequisites {
    pub age_available: bool,
}

pub fn check() -> Result<Prerequisites> {
    if std::env::var("AIRLOCK_SKIP_PREREQUISITES").as_deref() == Ok("1") {
        return Ok(Prerequisites {
            age_available: false,
        });
    }

    println!("\n{}", style("── Checking prerequisites ──").bold().cyan());
    println!();

    let mut missing = Vec::new();

    let required = [
        ("docker", "docker (OrbStack)"),
        ("code", "code (VS Code CLI)"),
        ("git", "git"),
    ];

    for (cmd, label) in &required {
        if which::which(cmd).is_ok() {
            println!("  {} {}", style("✔").green(), label);
        } else {
            missing.push(*label);
        }
    }

    if !missing.is_empty() {
        eprintln!("\n  {} Missing tools:", style("✖").red());
        for tool in &missing {
            eprintln!("     {} {}", style("•").red(), tool);
        }
        anyhow::bail!("Missing prerequisites. Install the listed tools above.");
    }

    let age_available = which::which("age").is_ok();
    if age_available {
        println!("  {} age (encryption)", style("✔").green());
    } else {
        println!(
            "  {} age not installed — secret encryption disabled",
            style("⚠").yellow()
        );
        let age_hint = if cfg!(target_os = "macos") {
            "brew install age"
        } else if cfg!(target_os = "windows") {
            "winget install FiloSottile.age   (or: scoop install age)"
        } else {
            "apt install age   (or equivalent: dnf/pacman/apk install age)"
        };
        println!("       → {}", age_hint);
        println!("       → https://github.com/FiloSottile/age/releases");
    }

    Ok(Prerequisites { age_available })
}
