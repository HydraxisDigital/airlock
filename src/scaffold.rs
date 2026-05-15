use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::Result;
use console::style;

use crate::paths::ProjectPaths;
use crate::stack::{StackMeta, StackRegistry};
use crate::{devcontainer, secrets};

pub struct ScaffoldOptions {
    pub needs_secrets: bool,
    pub init_git: bool,
    pub open_vscode: bool,
}

pub fn run(paths: &ProjectPaths, stack: &StackMeta, opts: &ScaffoldOptions) -> Result<()> {
    create_dirs(paths)?;
    write_devcontainer_files(paths, stack, opts.needs_secrets)?;
    write_gitignore(paths)?;
    write_security_md(paths, stack)?;

    if opts.needs_secrets {
        secrets::setup(paths)?;
    }

    if opts.init_git {
        crate::git::init(paths)?;
    }

    print_summary(paths, stack, opts);

    if opts.open_vscode {
        open_vscode(paths)?;
    }

    Ok(())
}

pub(crate) fn write_devcontainer_files(
    paths: &ProjectPaths,
    stack: &StackMeta,
    needs_secrets: bool,
) -> Result<()> {
    fs::create_dir_all(paths.dir.join(".devcontainer"))?;
    write_dockerfile(paths, stack)?;
    devcontainer::write(paths, stack, needs_secrets)?;
    write_post_create(paths, stack)?;
    Ok(())
}

fn create_dirs(paths: &ProjectPaths) -> Result<()> {
    println!(
        "\n{}",
        style("── Creating directory structure ──").bold().cyan()
    );
    fs::create_dir_all(paths.dir.join(".devcontainer"))?;
    fs::create_dir_all(paths.dir.join("src"))?;
    println!("  {} Directories created", style("✔").green());
    Ok(())
}

fn write_dockerfile(paths: &ProjectPaths, stack: &StackMeta) -> Result<()> {
    println!("\n{}", style("── Generating Dockerfile ──").bold().cyan());
    let content = StackRegistry::get_file_content(&stack.id, "Dockerfile")?;
    fs::write(paths.dir.join(".devcontainer").join("Dockerfile"), content)?;
    println!("  {} Dockerfile ({})", style("✔").green(), stack.id);
    Ok(())
}

fn write_post_create(paths: &ProjectPaths, stack: &StackMeta) -> Result<()> {
    println!("\n{}", style("── Generating post-create ──").bold().cyan());

    let header =
        "#!/usr/bin/env bash\nset -euo pipefail\necho \"Secure container configuration...\"\n";
    let stack_body = StackRegistry::get_file_content(&stack.id, "post-create.sh")?;
    // Strip shebang from stack body if present (we add our own header)
    let stack_body = strip_shebang(stack_body);
    let footer = StackRegistry::get_common_file("post-create-footer.sh").unwrap_or("");

    let content = format!("{}\n{}\n{}", header, stack_body, footer);

    let dest = paths.dir.join(".devcontainer").join("post-create.sh");
    fs::write(&dest, content)?;

    let mut perms = fs::metadata(&dest)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms)?;

    println!("  {} post-create.sh", style("✔").green());
    Ok(())
}

fn strip_shebang(s: &str) -> &str {
    if s.starts_with("#!") {
        s.split_once('\n').map(|x| x.1).unwrap_or(s)
    } else {
        s
    }
}

fn write_gitignore(paths: &ProjectPaths) -> Result<()> {
    let content = StackRegistry::get_common_file("gitignore").unwrap_or("");
    fs::write(paths.dir.join(".gitignore"), content)?;
    println!("  {} .gitignore", style("✔").green());
    Ok(())
}

fn write_security_md(paths: &ProjectPaths, stack: &StackMeta) -> Result<()> {
    let template = StackRegistry::get_common_file("SECURITY.md.tmpl").unwrap_or("");
    let content = template
        .replace("${PROJECT_NAME}", &paths.name)
        .replace("${AUDIT_COMMAND}", &stack.audit_command)
        .replace("${TREE_COMMAND}", &stack.tree_command);
    fs::write(paths.dir.join("SECURITY.md"), content)?;
    println!("  {} SECURITY.md", style("✔").green());
    Ok(())
}

fn print_summary(paths: &ProjectPaths, _stack: &StackMeta, opts: &ScaffoldOptions) {
    println!("\n{}", style("── Setup complete ──").bold().cyan());
    println!();
    println!("  {} is ready.", style(&paths.name).bold().green());
    println!();
    println!("  {} {}", style("📁"), style(paths.dir.display()).dim());
    println!("  🐳 Isolated Dev Container (bridge network, cap-drop=ALL)");

    if opts.needs_secrets {
        println!(
            "  🔑 Secrets → {}",
            style(paths.secret_file.display()).dim()
        );
        println!("     Mounted read-only at /run/secrets/env");
    }

    println!();
    println!("  {}", style("Next steps:").dim());
    println!("  1. Open in VS Code → 'Reopen in Container'");
    if opts.needs_secrets {
        println!("  2. Add your secrets to {}", paths.secret_file.display());
    }
    println!("  3. Verify each package on socket.dev before enabling");
    println!();
}

pub(crate) fn open_vscode(paths: &ProjectPaths) -> Result<()> {
    println!("\n{}", style("── Opening in VS Code ──").bold().cyan());
    println!("  VS Code will open and offer 'Reopen in Container'");
    println!("  Accept to launch the isolated container via OrbStack");

    std::process::Command::new("code")
        .arg(paths.dir.as_os_str())
        .spawn()?;

    println!("  {} Off we go!", style("✔").green());
    Ok(())
}
