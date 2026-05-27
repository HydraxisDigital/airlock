use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::Result;
use console::style;

use crate::paths::{new_uuid, slug_for_subdir, ProjectPaths};
use crate::stack::{StackChoice, StackMeta, StackRegistry};
use crate::{devcontainer, secrets};

pub struct ScaffoldOptions {
    pub needs_secrets: bool,
    pub init_git: bool,
    pub open_vscode: bool,
}

#[derive(Debug, Clone)]
pub struct MonorepoTarget {
    pub name: String,
    pub stack: StackChoice,
    pub needs_secrets: bool,
}

pub fn run(paths: &ProjectPaths, stack: &StackChoice, opts: &ScaffoldOptions) -> Result<()> {
    create_dirs(paths)?;
    write_devcontainer_files(paths, stack, opts.needs_secrets)?;
    write_gitignore(paths)?;
    write_security_md(paths, &stack.meta)?;

    if opts.needs_secrets {
        secrets::setup(paths)?;
    }

    if opts.init_git {
        crate::git::init(paths)?;
    }

    print_summary(paths, &stack.meta, opts);

    if opts.open_vscode {
        open_vscode(paths)?;
    }

    Ok(())
}

pub fn run_monorepo(
    root_paths: &ProjectPaths,
    targets: &[MonorepoTarget],
    opts: &ScaffoldOptions,
    secrets_dir: &Path,
) -> Result<()> {
    create_monorepo_root(root_paths)?;

    let mut target_paths = Vec::with_capacity(targets.len());
    for target in targets {
        let subdir = root_paths.dir.join(&target.name);
        fs::create_dir_all(&subdir)?;

        let base_slug = slug_for_subdir(&root_paths.dir, &target.name);
        let uuid = new_uuid();
        let paths = ProjectPaths::for_existing_with_slug(subdir, secrets_dir, &uuid, &base_slug)?;

        println!(
            "\n{} {}",
            style("──").bold().cyan(),
            style(format!("Generating {}", target.name)).bold().cyan()
        );
        write_devcontainer_files(&paths, &target.stack, target.needs_secrets)?;

        if target.needs_secrets {
            secrets::setup(&paths)?;
        }

        target_paths.push((target, paths));
    }

    write_gitignore(root_paths)?;
    write_monorepo_security_md(root_paths, targets)?;

    if opts.init_git {
        crate::git::init(root_paths)?;
    }

    print_monorepo_summary(root_paths, &target_paths);

    if opts.open_vscode {
        open_vscode(root_paths)?;
    }

    Ok(())
}

pub(crate) fn write_devcontainer_files(
    paths: &ProjectPaths,
    stack: &StackChoice,
    needs_secrets: bool,
) -> Result<()> {
    fs::create_dir_all(paths.dir.join(".devcontainer"))?;
    write_dockerfile(paths, stack)?;
    devcontainer::write(paths, &stack.meta, needs_secrets)?;
    write_post_create(paths, &stack.meta)?;
    Ok(())
}

fn create_monorepo_root(paths: &ProjectPaths) -> Result<()> {
    println!(
        "\n{}",
        style("── Creating monorepo structure ──").bold().cyan()
    );
    fs::create_dir_all(&paths.dir)?;
    println!("  {} Root directory created", style("✔").green());
    Ok(())
}

fn create_dirs(paths: &ProjectPaths) -> Result<()> {
    println!(
        "\n{}",
        style("── Creating directory structure ──").bold().cyan()
    );
    fs::create_dir_all(paths.dir.join(".devcontainer"))?;
    // Empty shell for the app: the user bootstraps their framework here
    // (`cd project && pnpm create next-app .`, `cargo init .`, `uv init`, …).
    // Kept empty on purpose — any pre-seeded file would trip the
    // "directory not empty" guard of generators like create-next-app.
    fs::create_dir_all(paths.dir.join("project"))?;
    println!("  {} Directories created", style("✔").green());
    Ok(())
}

fn write_dockerfile(paths: &ProjectPaths, stack: &StackChoice) -> Result<()> {
    println!("\n{}", style("── Generating Dockerfile ──").bold().cyan());
    let raw = StackRegistry::get_file_content(&stack.meta.id, "Dockerfile")?;
    let rendered = match &stack.meta.language_version {
        Some(lv) => {
            let version = stack.effective_version().unwrap_or(lv.default.as_str());
            let image = lv.image_for(version).unwrap_or(version);
            let placeholder = format!("__{}__", lv.token);
            raw.replace(&placeholder, image)
        }
        None => raw.to_string(),
    };
    fs::write(
        paths.dir.join(".devcontainer").join("Dockerfile"),
        &rendered,
    )?;
    let suffix = stack
        .effective_version()
        .map(|v| format!(" {}", v))
        .unwrap_or_default();
    println!(
        "  {} Dockerfile ({}{})",
        style("✔").green(),
        stack.meta.id,
        suffix
    );
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

fn write_monorepo_security_md(paths: &ProjectPaths, targets: &[MonorepoTarget]) -> Result<()> {
    let template = StackRegistry::get_common_file("SECURITY.md.tmpl").unwrap_or("");
    let audit = targets
        .iter()
        .map(|target| format!("cd {} && {}", target.name, target.stack.meta.audit_command))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = targets
        .iter()
        .map(|target| format!("cd {} && {}", target.name, target.stack.meta.tree_command))
        .collect::<Vec<_>>()
        .join("\n");
    let content = template
        .replace("${PROJECT_NAME}", &paths.name)
        .replace("${AUDIT_COMMAND}", &audit)
        .replace("${TREE_COMMAND}", &tree);
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
    let mut step = 1;
    println!("  {step}. Open in VS Code → 'Reopen in Container'");
    step += 1;
    if opts.needs_secrets {
        println!(
            "  {step}. Add your secrets to {}",
            paths.secret_file.display()
        );
        step += 1;
    }
    println!(
        "  {step}. Bootstrap your app in {}/ (e.g. cd project && pnpm create next-app@latest . --yes)",
        style("project").bold()
    );
    step += 1;
    println!("  {step}. Verify each package on socket.dev before enabling");
    println!();
}

fn print_monorepo_summary(paths: &ProjectPaths, targets: &[(&MonorepoTarget, ProjectPaths)]) {
    println!("\n{}", style("── Setup complete ──").bold().cyan());
    println!();
    println!("  {} is ready.", style(&paths.name).bold().green());
    println!();
    println!("  {} {}", style("📁"), style(paths.dir.display()).dim());
    println!("  🐳 Isolated Dev Containers per sub-directory");
    println!();

    for (target, target_paths) in targets {
        let stack_label = match target.stack.effective_version() {
            Some(version) => format!("{} {}", target.stack.meta.id, version),
            None => target.stack.meta.id.clone(),
        };
        println!(
            "  {} {:<16} {:<18} {}",
            style("✔").green(),
            format!("{}/", target.name),
            style(stack_label).dim(),
            style(&target_paths.name).cyan()
        );
        if target.needs_secrets {
            println!(
                "      🔑 secrets → {}",
                style(target_paths.secret_file.display()).dim()
            );
        }
    }

    println!();
    println!("  {}", style("Next steps:").dim());
    println!("  1. Open the root in VS Code to work across the monorepo");
    println!("  2. Bootstrap each app in its subdir");
    println!("  3. Reopen a subdir in Container when you want its isolated environment");
    println!("  4. Verify each package on socket.dev before enabling");
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
