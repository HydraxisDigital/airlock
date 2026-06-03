use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn scaffold_stack(stack_id: &str) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    let secrets_dir = tmp.path().join("secrets");
    std::fs::create_dir_all(&projects_dir).unwrap();
    std::fs::create_dir_all(&secrets_dir).unwrap();

    let name = format!("test-{}", stack_id);
    let paths = airlock::paths::ProjectPaths::new(&name, &projects_dir, &secrets_dir).unwrap();

    let stack =
        airlock::stack::StackChoice::new(airlock::stack::StackRegistry::get(stack_id).unwrap());
    let opts = airlock::scaffold::ScaffoldOptions {
        needs_secrets: false,
        init_git: false,
        open_vscode: false,
    };

    airlock::scaffold::run(&paths, &stack, &opts).unwrap();

    let project_dir = paths.dir.clone();
    (tmp, project_dir)
}

fn scaffold_stack_with_version(stack_id: &str, version: &str) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    let secrets_dir = tmp.path().join("secrets");
    std::fs::create_dir_all(&projects_dir).unwrap();
    std::fs::create_dir_all(&secrets_dir).unwrap();

    let name = format!("test-{}-{}", stack_id, version);
    let paths = airlock::paths::ProjectPaths::new(&name, &projects_dir, &secrets_dir).unwrap();
    let stack = airlock::stack::StackChoice::with_version(
        airlock::stack::StackRegistry::get(stack_id).unwrap(),
        Some(version.to_string()),
    );
    let opts = airlock::scaffold::ScaffoldOptions {
        needs_secrets: false,
        init_git: false,
        open_vscode: false,
    };
    airlock::scaffold::run(&paths, &stack, &opts).unwrap();
    let project_dir = paths.dir.clone();
    (tmp, project_dir)
}

fn scaffold_monorepo(
    targets: &[&str],
    needs_secrets: bool,
) -> (
    TempDir,
    std::path::PathBuf,
    Vec<airlock::scaffold::MonorepoTarget>,
) {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    let secrets_dir = tmp.path().join("secrets");
    std::fs::create_dir_all(&projects_dir).unwrap();
    std::fs::create_dir_all(&secrets_dir).unwrap();

    let root_paths = airlock::paths::ProjectPaths::new("crm", &projects_dir, &secrets_dir).unwrap();
    let specs = targets.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let parsed = airlock::cli::parse_new_targets(&specs, needs_secrets).unwrap();
    let opts = airlock::scaffold::ScaffoldOptions {
        needs_secrets,
        init_git: false,
        open_vscode: false,
    };

    airlock::scaffold::run_monorepo(&root_paths, &parsed, &opts, &secrets_dir).unwrap();

    (tmp, root_paths.dir.clone(), parsed)
}

fn airlock_bin() -> &'static str {
    env!("CARGO_BIN_EXE_airlock")
}

fn assert_required_files(project_dir: &Path) {
    for file in &[
        ".devcontainer/Dockerfile",
        ".devcontainer/devcontainer.json",
        ".devcontainer/post-create.sh",
        ".gitignore",
        "SECURITY.md",
    ] {
        assert!(project_dir.join(file).exists(), "Missing file: {}", file);
    }

    // The app shell must exist and stay empty: generators like create-next-app
    // refuse to run in a non-empty directory.
    let project = project_dir.join("project");
    assert!(project.is_dir(), "Missing project/ directory");
    assert!(
        std::fs::read_dir(&project).unwrap().next().is_none(),
        "project/ must be empty"
    );
}

fn assert_valid_json(project_dir: &Path, file: &str) {
    let content = std::fs::read_to_string(project_dir.join(file)).unwrap();
    // Strip leading comment lines before JSON
    let json_start = content.find('{').expect("No JSON object found");
    let json_str = &content[json_start..];
    serde_json::from_str::<serde_json::Value>(json_str)
        .unwrap_or_else(|e| panic!("Invalid JSON in {}: {}", file, e));
}

fn assert_executable(project_dir: &Path, file: &str) {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(project_dir.join(file)).unwrap();
    let mode = meta.permissions().mode();
    assert!(mode & 0o111 != 0, "{} is not executable", file);
}

#[test]
fn test_minimal_stack() {
    let (_tmp, dir) = scaffold_stack("minimal");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");
    assert_executable(&dir, ".devcontainer/post-create.sh");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("ubuntu-22.04"));
}

#[test]
fn test_typescript_stack() {
    let (_tmp, dir) = scaffold_stack("typescript");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("pnpm"), "Dockerfile should use pnpm");
    assert!(dockerfile.contains("corepack"), "Should use corepack");
    assert!(
        !dockerfile.contains("RUN npm"),
        "Should not use old npm commands"
    );
    assert!(
        !dockerfile.contains("__NODE_VERSION__"),
        "placeholder must be substituted"
    );
    let stack = airlock::stack::StackRegistry::get("typescript").unwrap();
    let default = stack.language_version.as_ref().unwrap().default.clone();
    assert!(
        dockerfile.contains(&format!("typescript-node:{}", default)),
        "Dockerfile should pin default node version {}: {}",
        default,
        dockerfile
    );

    let json_content =
        std::fs::read_to_string(dir.join(".devcontainer/devcontainer.json")).unwrap();
    let json_start = json_content.find('{').unwrap();
    let v: serde_json::Value = serde_json::from_str(&json_content[json_start..]).unwrap();
    assert_eq!(v["name"], "test-typescript");
    assert!(v["runArgs"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("--cap-drop=ALL")));
}

#[test]
fn test_javascript_stack() {
    let (_tmp, dir) = scaffold_stack("javascript");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("pnpm"), "Dockerfile should use pnpm");
    assert!(dockerfile.contains("corepack"), "Should use corepack");
    assert!(
        !dockerfile.contains("__NODE_VERSION__"),
        "placeholder must be substituted"
    );
    let stack = airlock::stack::StackRegistry::get("javascript").unwrap();
    let default = stack.language_version.as_ref().unwrap().default.clone();
    assert!(
        dockerfile.contains(&format!("javascript-node:{}", default)),
        "Dockerfile should pin default node version {}: {}",
        default,
        dockerfile
    );

    let json_content =
        std::fs::read_to_string(dir.join(".devcontainer/devcontainer.json")).unwrap();
    let json_start = json_content.find('{').unwrap();
    let v: serde_json::Value = serde_json::from_str(&json_content[json_start..]).unwrap();
    assert_eq!(v["name"], "test-javascript");
}

#[test]
fn test_python_stack() {
    let (_tmp, dir) = scaffold_stack("python");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("uv"), "Dockerfile should use uv");
    assert!(
        dockerfile.contains("astral-sh/uv"),
        "Should use official uv image"
    );
    assert!(!dockerfile.contains("poetry"), "Should not use poetry");
    assert!(
        !dockerfile.contains("pip install"),
        "Should not use pip install"
    );
}

#[test]
fn test_rust_stack() {
    let (_tmp, dir) = scaffold_stack("rust");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("devcontainers/rust"));
    assert!(dockerfile.contains("cargo-audit"));
}

#[test]
fn test_solidity_stack() {
    let (_tmp, dir) = scaffold_stack("solidity");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("foundry-rs/foundry"));
    assert!(dockerfile.contains("forge"));
}

#[test]
fn test_solidity_ts_stack() {
    let (_tmp, dir) = scaffold_stack("solidity-ts");
    assert_required_files(&dir);
    assert_valid_json(&dir, ".devcontainer/devcontainer.json");

    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(dockerfile.contains("pnpm"), "Should use pnpm");
    assert!(
        dockerfile.contains("foundry-rs/foundry"),
        "Should include Foundry"
    );
}

#[test]
fn test_typescript_explicit_node_version() {
    let (_tmp, dir) = scaffold_stack_with_version("typescript", "20");
    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(
        dockerfile.contains("typescript-node:20"),
        "explicit node 20 should be pinned: {}",
        dockerfile
    );
    assert!(
        !dockerfile.contains("__NODE_VERSION__"),
        "placeholder must be substituted"
    );
}

#[test]
fn test_python_explicit_version() {
    let (_tmp, dir) = scaffold_stack_with_version("python", "3.13");
    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(
        dockerfile.contains("python:3.13"),
        "explicit python 3.13 should be pinned: {}",
        dockerfile
    );
}

#[test]
fn test_rust_explicit_version() {
    let (_tmp, dir) = scaffold_stack_with_version("rust", "1.83");
    let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(
        dockerfile.contains("devcontainers/rust:1.83"),
        "explicit rust 1.83 should be pinned: {}",
        dockerfile
    );
}

#[test]
fn test_monorepo_new_creates_target_devcontainers() {
    let (_tmp, dir, _targets) =
        scaffold_monorepo(&["frontend=typescript@22", "backend=python@3.12"], false);

    assert!(dir.join(".gitignore").exists());
    assert!(dir.join("SECURITY.md").exists());
    assert!(!dir.join(".devcontainer").exists());
    assert!(!dir.join("project").exists());

    assert!(dir.join("frontend/.devcontainer/Dockerfile").exists());
    assert!(dir
        .join("frontend/.devcontainer/devcontainer.json")
        .exists());
    assert!(dir.join("frontend/.devcontainer/post-create.sh").exists());
    assert!(!dir.join("frontend/project").exists());

    assert!(dir.join("backend/.devcontainer/Dockerfile").exists());
    assert!(dir.join("backend/.devcontainer/devcontainer.json").exists());
    assert!(dir.join("backend/.devcontainer/post-create.sh").exists());
    assert!(!dir.join("backend/project").exists());
}

#[test]
fn test_monorepo_new_pins_target_versions() {
    let (_tmp, dir, _targets) =
        scaffold_monorepo(&["frontend=typescript@22", "backend=python@3.12"], false);

    let frontend_dockerfile =
        std::fs::read_to_string(dir.join("frontend/.devcontainer/Dockerfile")).unwrap();
    assert!(
        frontend_dockerfile.contains("typescript-node:22"),
        "frontend should pin node 22: {}",
        frontend_dockerfile
    );

    let backend_dockerfile =
        std::fs::read_to_string(dir.join("backend/.devcontainer/Dockerfile")).unwrap();
    assert!(
        backend_dockerfile.contains("python:3.12"),
        "backend should pin python 3.12: {}",
        backend_dockerfile
    );
}

#[test]
fn test_monorepo_new_secrets_are_per_target() {
    let (_tmp, dir, targets) =
        scaffold_monorepo(&["frontend=typescript@22", "backend=python@3.12"], true);

    assert_eq!(targets.len(), 2);
    assert!(targets.iter().all(|target| target.needs_secrets));

    let frontend_json =
        std::fs::read_to_string(dir.join("frontend/.devcontainer/devcontainer.json")).unwrap();
    let backend_json =
        std::fs::read_to_string(dir.join("backend/.devcontainer/devcontainer.json")).unwrap();
    assert!(frontend_json.contains("\"mounts\""));
    assert!(backend_json.contains("\"mounts\""));
    assert!(frontend_json.contains("crm-frontend"));
    assert!(backend_json.contains("crm-backend"));
}

#[test]
fn test_parse_new_targets_rejects_duplicates() {
    let specs = vec![
        "frontend=typescript@22".to_string(),
        "frontend=python@3.12".to_string(),
    ];
    let err = airlock::cli::parse_new_targets(&specs, false).unwrap_err();
    assert!(
        err.to_string().contains("Duplicate target"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_parse_new_targets_rejects_versionless_stack_version() {
    let specs = vec!["contracts=solidity@1".to_string()];
    let err = airlock::cli::parse_new_targets(&specs, false).unwrap_err();
    assert!(
        err.to_string()
            .contains("does not support language versions"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_parse_new_targets_supports_per_target_secrets() {
    let specs = vec![
        "frontend=typescript@22:secrets".to_string(),
        "backend=python@3.12:no-secrets".to_string(),
    ];
    let targets = airlock::cli::parse_new_targets(&specs, false).unwrap();
    assert_eq!(targets.len(), 2);
    assert!(targets[0].needs_secrets);
    assert!(!targets[1].needs_secrets);
}

#[test]
fn test_parse_new_targets_rejects_unknown_target_option() {
    let specs = vec!["frontend=typescript@22:maybe".to_string()];
    let err = airlock::cli::parse_new_targets(&specs, false).unwrap_err();
    assert!(
        err.to_string().contains(":secrets or :no-secrets"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn test_cli_new_monorepo_targets_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    std::fs::create_dir_all(&projects_dir).unwrap();

    let output = Command::new(airlock_bin())
        .args([
            "new",
            "crm",
            "--target",
            "frontend=typescript@22",
            "--target",
            "backend=python@3.12",
            "--no-git",
            "--no-vscode",
            "--path",
        ])
        .arg(&projects_dir)
        .env("AIRLOCK_SKIP_PREREQUISITES", "1")
        .env("HOME", tmp.path())
        .env_remove("SECRETS_DIR")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "airlock new failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let root = projects_dir.join("crm");
    assert!(root.join(".gitignore").exists());
    assert!(root.join("SECURITY.md").exists());
    assert!(!root.join(".devcontainer").exists());
    assert!(root
        .join("frontend/.devcontainer/devcontainer.json")
        .exists());
    assert!(root
        .join("backend/.devcontainer/devcontainer.json")
        .exists());

    let frontend_dockerfile =
        std::fs::read_to_string(root.join("frontend/.devcontainer/Dockerfile")).unwrap();
    let backend_dockerfile =
        std::fs::read_to_string(root.join("backend/.devcontainer/Dockerfile")).unwrap();
    assert!(frontend_dockerfile.contains("typescript-node:22"));
    assert!(backend_dockerfile.contains("python:3.12"));
}

#[test]
fn test_cli_new_monorepo_per_target_secrets_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    std::fs::create_dir_all(&projects_dir).unwrap();

    let output = Command::new(airlock_bin())
        .args([
            "new",
            "crm",
            "--target",
            "frontend=typescript@22:secrets",
            "--target",
            "backend=python@3.12:no-secrets",
            "--no-git",
            "--no-vscode",
            "--path",
        ])
        .arg(&projects_dir)
        .env("AIRLOCK_SKIP_PREREQUISITES", "1")
        .env("HOME", tmp.path())
        .env_remove("SECRETS_DIR")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "airlock new failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let root = projects_dir.join("crm");
    let frontend_json =
        std::fs::read_to_string(root.join("frontend/.devcontainer/devcontainer.json")).unwrap();
    let backend_json =
        std::fs::read_to_string(root.join("backend/.devcontainer/devcontainer.json")).unwrap();
    assert!(frontend_json.contains("\"mounts\""));
    assert!(frontend_json.contains(".airlock"));
    assert!(!backend_json.contains("\"mounts\""));
    assert!(tmp.path().join(".airlock").exists());
    assert!(!tmp.path().join(".secrets").exists());
}

#[test]
fn test_cli_new_monorepo_rejects_invalid_target_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    std::fs::create_dir_all(&projects_dir).unwrap();

    let output = Command::new(airlock_bin())
        .args([
            "new",
            "crm",
            "--target",
            "contracts=solidity@1",
            "--no-git",
            "--no-vscode",
            "--path",
        ])
        .arg(&projects_dir)
        .env("AIRLOCK_SKIP_PREREQUISITES", "1")
        .env("HOME", tmp.path())
        .env_remove("SECRETS_DIR")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "invalid target unexpectedly succeeded\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not support language versions"),
        "unexpected stderr: {}",
        stderr
    );
}

#[test]
fn test_minimal_stack_has_no_language_version() {
    let stack = airlock::stack::StackRegistry::get("minimal").unwrap();
    assert!(stack.language_version.is_none());
}

#[test]
fn test_solidity_stack_has_no_language_version() {
    let stack = airlock::stack::StackRegistry::get("solidity").unwrap();
    assert!(stack.language_version.is_none());
}

#[test]
fn test_supported_contains_default_per_stack() {
    for stack_id in &["typescript", "javascript", "python", "rust", "solidity-ts"] {
        let stack = airlock::stack::StackRegistry::get(stack_id).unwrap();
        let lv = stack
            .language_version
            .as_ref()
            .unwrap_or_else(|| panic!("{} should declare language_version", stack_id));
        assert!(
            lv.supports(&lv.default),
            "{}: default {} not in supported {:?}",
            stack_id,
            lv.default,
            lv.versions()
        );
    }
}

#[test]
fn test_detect_node_version_from_nvmrc() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".nvmrc"), "v22.1.0\n").unwrap();
    assert_eq!(
        airlock::detect::detect_language_version(tmp.path(), "node"),
        Some("22".to_string())
    );
}

#[test]
fn test_detect_node_version_from_engines() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        r#"{"engines": {"node": ">=20.0.0"}}"#,
    )
    .unwrap();
    assert_eq!(
        airlock::detect::detect_language_version(tmp.path(), "node"),
        Some("20".to_string())
    );
}

#[test]
fn test_detect_python_version_from_pyproject() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("pyproject.toml"),
        "[project]\nrequires-python = \">=3.11\"\n",
    )
    .unwrap();
    assert_eq!(
        airlock::detect::detect_language_version(tmp.path(), "python"),
        Some("3.11".to_string())
    );
}

#[test]
fn test_detect_python_version_from_python_version_file() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join(".python-version"), "3.12.4\n").unwrap();
    assert_eq!(
        airlock::detect::detect_language_version(tmp.path(), "python"),
        Some("3.12".to_string())
    );
}

#[test]
fn test_detect_rust_version_from_toolchain_toml() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"1.82\"\n",
    )
    .unwrap();
    assert_eq!(
        airlock::detect::detect_language_version(tmp.path(), "rust"),
        Some("1.82".to_string())
    );
}

#[test]
fn test_detect_returns_none_when_no_indicator() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        airlock::detect::detect_language_version(tmp.path(), "node"),
        None
    );
}

#[test]
fn test_devcontainer_security_settings() {
    for stack_id in &[
        "minimal",
        "typescript",
        "javascript",
        "python",
        "rust",
        "solidity",
        "solidity-ts",
    ] {
        let (_tmp, dir) = scaffold_stack(stack_id);
        let content = std::fs::read_to_string(dir.join(".devcontainer/devcontainer.json")).unwrap();
        let json_start = content.find('{').unwrap();
        let v: serde_json::Value = serde_json::from_str(&content[json_start..]).unwrap();

        let run_args = v["runArgs"].as_array().unwrap();
        assert!(
            run_args.contains(&serde_json::json!("--cap-drop=ALL")),
            "Stack {}: cap-drop=ALL missing",
            stack_id
        );
        assert!(
            run_args.contains(&serde_json::json!("--security-opt=no-new-privileges")),
            "Stack {}: no-new-privileges missing",
            stack_id
        );
        assert!(
            run_args.contains(&serde_json::json!("--network=bridge")),
            "Stack {}: network=bridge missing",
            stack_id
        );

        let extensions = v["customizations"]["vscode"]["extensions"]
            .as_array()
            .unwrap();
        for extension in [
            "anthropic.claude-code",
            "openai.chatgpt",
            "Google.geminicodeassist",
        ] {
            assert!(
                extensions.contains(&serde_json::json!(extension)),
                "Stack {}: required VS Code extension {} missing",
                stack_id,
                extension
            );
        }

        assert_eq!(
            v["customizations"]["vscode"]["settings"]["terminal.integrated.defaultProfile.linux"],
            serde_json::json!("zsh"),
            "Stack {}: default terminal profile should be zsh",
            stack_id
        );
        assert_eq!(
            v["customizations"]["vscode"]["settings"]["terminal.integrated.profiles.linux"]["zsh"]
                ["path"],
            serde_json::json!("/usr/bin/zsh"),
            "Stack {}: zsh terminal profile should point to /usr/bin/zsh",
            stack_id
        );
        assert_eq!(
            v["remoteEnv"]["SHELL"],
            serde_json::json!("/usr/bin/zsh"),
            "Stack {}: remote SHELL should be zsh",
            stack_id
        );

        let dockerfile = std::fs::read_to_string(dir.join(".devcontainer/Dockerfile")).unwrap();
        assert!(
            dockerfile.contains("apt-get install") && dockerfile.contains("zsh"),
            "Stack {}: Dockerfile should install zsh",
            stack_id
        );
        assert!(
            dockerfile.contains("chsh -s /usr/bin/zsh"),
            "Stack {}: Dockerfile should configure zsh as login shell",
            stack_id
        );
        assert!(
            dockerfile.contains("github.com/ohmyzsh/ohmyzsh.git"),
            "Stack {}: Dockerfile should install Oh My Zsh",
            stack_id
        );
        assert!(
            dockerfile.contains("! -f ~/.oh-my-zsh/oh-my-zsh.sh"),
            "Stack {}: Oh My Zsh install should be idempotent",
            stack_id
        );
    }
}

#[test]
fn test_post_create_footer_isolation_check() {
    let (_tmp, dir) = scaffold_stack("minimal");
    let content = std::fs::read_to_string(dir.join(".devcontainer/post-create.sh")).unwrap();
    assert!(
        content.contains("found_sensitive"),
        "Footer isolation check must be present"
    );
    assert!(
        content.contains("/Users"),
        "/Users isolation check must be present"
    );
}

#[test]
fn test_post_create_explains_runtime_apt_disabled() {
    let (_tmp, dir) = scaffold_stack("minimal");
    let content = std::fs::read_to_string(dir.join(".devcontainer/post-create.sh")).unwrap();
    assert!(
        content.contains("airlock_apt_help"),
        "post-create should install apt guidance"
    );
    assert!(
        content.contains("~/.zshrc"),
        "post-create should install shell guidance for zsh"
    );
    assert!(
        content.contains("airlock disables runtime apt/sudo"),
        "apt guidance should explain why apt is blocked"
    );
    assert!(
        content.contains("Dev Containers: Rebuild Container"),
        "apt guidance should point to rebuild flow"
    );
}

#[test]
fn test_slug_paths() {
    use airlock::paths::slugify;
    assert_eq!(slugify("My Cool Project"), "my-cool-project");
    assert_eq!(slugify("hello   world"), "hello-world");
    assert_eq!(slugify("--test--"), "test");
}

fn make_existing_project(name: &str) -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join(name);
    std::fs::create_dir_all(&project_dir).unwrap();
    let secrets_dir = tmp.path().join("secrets");
    std::fs::create_dir_all(&secrets_dir).unwrap();
    (tmp, project_dir, secrets_dir)
}

fn single_target(
    project_dir: &Path,
    secrets_dir: &Path,
    stack_id: &str,
    needs_secrets: bool,
) -> airlock::retrofit::RetrofitTarget {
    let uuid = airlock::paths::new_uuid();
    let paths =
        airlock::paths::ProjectPaths::for_existing(project_dir.to_path_buf(), secrets_dir, &uuid)
            .unwrap();
    let stack =
        airlock::stack::StackChoice::new(airlock::stack::StackRegistry::get(stack_id).unwrap());
    airlock::retrofit::RetrofitTarget {
        paths,
        stack,
        needs_secrets,
        display_path: project_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| project_dir.display().to_string()),
    }
}

fn run_single(
    project_dir: &Path,
    secrets_dir: &Path,
    stack_id: &str,
    needs_secrets: bool,
    force: bool,
) -> Vec<airlock::retrofit::RetrofitOutcome> {
    let target = single_target(project_dir, secrets_dir, stack_id, needs_secrets);
    airlock::retrofit::run_many(vec![target], force).unwrap()
}

fn parse_devcontainer_name(devcontainer_json: &Path) -> String {
    let content = std::fs::read_to_string(devcontainer_json).unwrap();
    let json_start = content.find('{').unwrap();
    let v: serde_json::Value = serde_json::from_str(&content[json_start..]).unwrap();
    v["name"].as_str().unwrap().to_string()
}

#[test]
fn test_retrofit_minimal_dir() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");

    let outcomes = run_single(&project_dir, &secrets_dir, "minimal", false, false);
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    assert!(project_dir.join(".devcontainer/Dockerfile").exists());
    assert!(project_dir.join(".devcontainer/devcontainer.json").exists());
    assert!(project_dir.join(".devcontainer/post-create.sh").exists());
    assert!(project_dir.join("SECURITY.md").exists());
    assert!(project_dir.join(".gitignore").exists());
    assert!(!project_dir.join("project").exists());
    assert!(!project_dir.join(".git").exists());
}

#[test]
fn test_retrofit_skips_when_devcontainer_exists() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    std::fs::create_dir_all(project_dir.join(".devcontainer")).unwrap();

    let outcomes = run_single(&project_dir, &secrets_dir, "minimal", false, false);
    assert_eq!(outcomes.len(), 1);
    match &outcomes[0].status {
        airlock::retrofit::TargetStatus::Skipped(reason) => {
            assert!(
                reason.contains("--force"),
                "skip reason should mention --force: {}",
                reason
            );
        }
        airlock::retrofit::TargetStatus::Done => panic!("expected Skipped, got Done"),
    }
}

#[test]
fn test_retrofit_force_overwrites() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    let dc = project_dir.join(".devcontainer");
    std::fs::create_dir_all(&dc).unwrap();
    std::fs::write(dc.join("Dockerfile"), "OLD CONTENT").unwrap();

    let outcomes = run_single(&project_dir, &secrets_dir, "minimal", false, true);
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    let dockerfile = std::fs::read_to_string(project_dir.join(".devcontainer/Dockerfile")).unwrap();
    assert!(
        dockerfile != "OLD CONTENT",
        "Dockerfile should have been overwritten"
    );
    assert!(dockerfile.contains("ubuntu-22.04"));
}

#[test]
fn test_retrofit_env_migration() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    std::fs::write(project_dir.join(".env"), "SECRET=42\nFOO=bar\n").unwrap();

    let target = single_target(&project_dir, &secrets_dir, "minimal", true);
    let secret_file = target.paths.secret_file.clone();
    let outcomes = airlock::retrofit::run_many(vec![target], false).unwrap();
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    assert!(
        !project_dir.join(".env").exists(),
        ".env should have been moved out of the project"
    );
    let content = std::fs::read_to_string(&secret_file).unwrap();
    assert!(
        content.contains("SECRET=42"),
        "secret file should contain migrated content, got: {}",
        content
    );

    let gitignore = std::fs::read_to_string(project_dir.join(".gitignore")).unwrap();
    assert!(
        gitignore.lines().any(|l| l.trim() == ".env"),
        ".env should be listed in .gitignore"
    );
}

#[test]
fn test_retrofit_preserves_existing_security_md() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    let custom = "# Custom SECURITY.md\nDo not overwrite me.\n";
    std::fs::write(project_dir.join("SECURITY.md"), custom).unwrap();

    let outcomes = run_single(&project_dir, &secrets_dir, "minimal", false, false);
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    let content = std::fs::read_to_string(project_dir.join("SECURITY.md")).unwrap();
    assert_eq!(content, custom, "existing SECURITY.md must not be touched");
}

#[test]
fn test_retrofit_merges_existing_gitignore() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    std::fs::write(project_dir.join(".gitignore"), "target/\nmy-custom-line\n").unwrap();

    let outcomes = run_single(&project_dir, &secrets_dir, "minimal", false, false);
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    let content = std::fs::read_to_string(project_dir.join(".gitignore")).unwrap();
    assert!(content.contains("my-custom-line"), "existing entries kept");
    assert!(content.contains(".env"), "common .env entry merged");
    assert_eq!(
        content.matches("target/").count(),
        1,
        "duplicates should not be appended"
    );
}

#[test]
fn test_retrofit_uuid_in_devcontainer_name() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    std::fs::write(project_dir.join("Cargo.toml"), "").unwrap();

    run_single(&project_dir, &secrets_dir, "rust", false, false);

    let name = parse_devcontainer_name(&project_dir.join(".devcontainer/devcontainer.json"));
    let (prefix, rest) = name.split_once('-').expect("expected <uuid>-<slug>");
    assert_eq!(prefix.len(), 4, "uuid prefix must be 4 chars: {}", name);
    assert!(
        prefix
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "uuid prefix must be hex lowercase: {}",
        prefix
    );
    assert_eq!(rest, "demo");
}

#[test]
fn test_retrofit_secrets_path_uses_uuid() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    std::fs::write(project_dir.join("Cargo.toml"), "").unwrap();

    let target = single_target(&project_dir, &secrets_dir, "rust", true);
    let secret_file = target.paths.secret_file.clone();
    let full_name = target.paths.name.clone();
    let outcomes = airlock::retrofit::run_many(vec![target], false).unwrap();
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    assert!(
        secret_file.exists(),
        "secret file missing: {:?}",
        secret_file
    );
    let parent = secret_file.parent().unwrap();
    let dirname = parent.file_name().unwrap().to_string_lossy();
    assert_eq!(dirname, full_name, "secrets dir must equal <uuid>-<slug>");
}

#[test]
fn test_retrofit_force_preserves_uuid() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("demo");
    std::fs::write(project_dir.join("Cargo.toml"), "").unwrap();

    run_single(&project_dir, &secrets_dir, "rust", false, false);
    let dc_json = project_dir.join(".devcontainer/devcontainer.json");
    let first_name = parse_devcontainer_name(&dc_json);

    // Simulate the CLI flow: read existing UUID, reuse, force overwrite
    let existing_uuid = airlock::retrofit::read_existing_uuid(&dc_json)
        .expect("expected to recover UUID from existing devcontainer.json");
    let stack =
        airlock::stack::StackChoice::new(airlock::stack::StackRegistry::get("rust").unwrap());
    let paths = airlock::paths::ProjectPaths::for_existing(
        project_dir.clone(),
        &secrets_dir,
        &existing_uuid,
    )
    .unwrap();
    let target = airlock::retrofit::RetrofitTarget {
        paths,
        stack,
        needs_secrets: false,
        display_path: "demo".to_string(),
    };
    let outcomes = airlock::retrofit::run_many(vec![target], true).unwrap();
    assert!(matches!(
        outcomes[0].status,
        airlock::retrofit::TargetStatus::Done
    ));

    let second_name = parse_devcontainer_name(&dc_json);
    assert_eq!(
        first_name, second_name,
        "UUID must be preserved across --force"
    );
}

#[test]
fn test_cli_retrofit_monorepo_layout_root_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("crm");
    std::fs::create_dir_all(project_dir.join("backend")).unwrap();
    std::fs::write(project_dir.join("backend/pyproject.toml"), "").unwrap();
    std::fs::create_dir_all(project_dir.join("frontend")).unwrap();
    std::fs::write(project_dir.join("frontend/package.json"), "{}").unwrap();

    let output = Command::new(airlock_bin())
        .args([
            "retrofit",
            "--layout",
            "root",
            "--stack",
            "minimal",
            "--no-secrets",
        ])
        .arg(&project_dir)
        .env("AIRLOCK_SKIP_PREREQUISITES", "1")
        .env("HOME", tmp.path())
        .env_remove("SECRETS_DIR")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "airlock retrofit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(project_dir.join(".devcontainer/devcontainer.json").exists());
    assert!(!project_dir.join("backend/.devcontainer").exists());
    assert!(!project_dir.join("frontend/.devcontainer").exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Open the project folder in VS Code"),
        "root layout should mention project folder\nstdout:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("Open the subdir"),
        "root layout must not mention subdir\nstdout:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("Edit secrets"),
        "--no-secrets should not print secrets next step\nstdout:\n{}",
        stdout
    );
}

#[test]
fn test_cli_retrofit_monorepo_layout_subprojects_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("crm");
    std::fs::create_dir_all(project_dir.join("backend")).unwrap();
    std::fs::write(project_dir.join("backend/pyproject.toml"), "").unwrap();
    std::fs::create_dir_all(project_dir.join("frontend")).unwrap();
    std::fs::write(project_dir.join("frontend/package.json"), "{}").unwrap();

    let output = Command::new(airlock_bin())
        .args(["retrofit", "--layout", "subprojects", "--no-secrets"])
        .arg(&project_dir)
        .env("AIRLOCK_SKIP_PREREQUISITES", "1")
        .env("HOME", tmp.path())
        .env_remove("SECRETS_DIR")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "airlock retrofit failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(!project_dir.join(".devcontainer").exists());
    assert!(project_dir
        .join("backend/.devcontainer/devcontainer.json")
        .exists());
    assert!(project_dir
        .join("frontend/.devcontainer/devcontainer.json")
        .exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Open each generated sub-project folder in VS Code"),
        "subprojects layout should mention sub-project folders\nstdout:\n{}",
        stdout
    );
}

#[test]
fn test_cli_retrofit_subprojects_layout_rejects_stack_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("crm");
    std::fs::create_dir_all(project_dir.join("backend")).unwrap();
    std::fs::write(project_dir.join("backend/pyproject.toml"), "").unwrap();
    std::fs::create_dir_all(project_dir.join("frontend")).unwrap();
    std::fs::write(project_dir.join("frontend/package.json"), "{}").unwrap();

    let output = Command::new(airlock_bin())
        .args([
            "retrofit",
            "--layout",
            "subprojects",
            "--stack",
            "rust",
            "--no-secrets",
        ])
        .arg(&project_dir)
        .env("AIRLOCK_SKIP_PREREQUISITES", "1")
        .env("HOME", tmp.path())
        .env_remove("SECRETS_DIR")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "invalid retrofit unexpectedly succeeded\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--stack cannot be used with --layout subprojects"),
        "unexpected stderr: {}",
        stderr
    );
}

#[test]
fn test_retrofit_iterates_monorepo() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("crm");
    std::fs::create_dir_all(project_dir.join("backend")).unwrap();
    std::fs::write(project_dir.join("backend/pyproject.toml"), "").unwrap();
    std::fs::create_dir_all(project_dir.join("frontend")).unwrap();
    std::fs::write(project_dir.join("frontend/package.json"), "{}").unwrap();

    let subs = airlock::detect::detect_substacks(&project_dir);
    assert_eq!(subs.len(), 2);

    let mut targets = Vec::new();
    for (subdir, stack_id) in subs {
        let subdir_name = subdir.file_name().unwrap().to_string_lossy().into_owned();
        let base_slug = airlock::paths::slug_for_subdir(&project_dir, &subdir_name);
        let uuid = airlock::paths::new_uuid();
        let paths = airlock::paths::ProjectPaths::for_existing_with_slug(
            subdir.clone(),
            &secrets_dir,
            &uuid,
            &base_slug,
        )
        .unwrap();
        let stack =
            airlock::stack::StackChoice::new(airlock::stack::StackRegistry::get(stack_id).unwrap());
        targets.push(airlock::retrofit::RetrofitTarget {
            paths,
            stack,
            needs_secrets: true,
            display_path: format!("crm/{}/", subdir_name),
        });
    }

    let outcomes = airlock::retrofit::run_many(targets, false).unwrap();
    assert_eq!(outcomes.len(), 2);

    assert!(project_dir
        .join("backend/.devcontainer/devcontainer.json")
        .exists());
    assert!(project_dir
        .join("frontend/.devcontainer/devcontainer.json")
        .exists());

    // Two distinct secrets paths
    let s1 = outcomes[0]
        .target
        .paths
        .secret_file
        .parent()
        .unwrap()
        .to_path_buf();
    let s2 = outcomes[1]
        .target
        .paths
        .secret_file
        .parent()
        .unwrap()
        .to_path_buf();
    assert_ne!(s1, s2, "secrets dirs must differ");
    assert!(s1.exists() && s2.exists());

    // Both slugs carry the crm- prefix
    let n1 = &outcomes[0].target.paths.name;
    let n2 = &outcomes[1].target.paths.name;
    assert!(n1.contains("-crm-"), "expected crm prefix in: {}", n1);
    assert!(n2.contains("-crm-"), "expected crm prefix in: {}", n2);
}

#[test]
fn test_retrofit_subdir_skipped_continues() {
    let (_tmp, project_dir, secrets_dir) = make_existing_project("crm");
    std::fs::create_dir_all(project_dir.join("backend/.devcontainer")).unwrap();
    std::fs::write(project_dir.join("backend/pyproject.toml"), "").unwrap();
    std::fs::create_dir_all(project_dir.join("frontend")).unwrap();
    std::fs::write(project_dir.join("frontend/package.json"), "{}").unwrap();

    let subs = airlock::detect::detect_substacks(&project_dir);
    let mut targets = Vec::new();
    for (subdir, stack_id) in subs {
        let subdir_name = subdir.file_name().unwrap().to_string_lossy().into_owned();
        let base_slug = airlock::paths::slug_for_subdir(&project_dir, &subdir_name);
        let uuid = airlock::paths::new_uuid();
        let paths = airlock::paths::ProjectPaths::for_existing_with_slug(
            subdir.clone(),
            &secrets_dir,
            &uuid,
            &base_slug,
        )
        .unwrap();
        let stack =
            airlock::stack::StackChoice::new(airlock::stack::StackRegistry::get(stack_id).unwrap());
        targets.push(airlock::retrofit::RetrofitTarget {
            paths,
            stack,
            needs_secrets: false,
            display_path: format!("crm/{}/", subdir_name),
        });
    }

    let outcomes = airlock::retrofit::run_many(targets, false).unwrap();
    assert_eq!(outcomes.len(), 2);

    let by_display: std::collections::HashMap<&str, &airlock::retrofit::RetrofitOutcome> = outcomes
        .iter()
        .map(|o| (o.target.display_path.as_str(), o))
        .collect();

    let backend = by_display.get("crm/backend/").unwrap();
    assert!(
        matches!(backend.status, airlock::retrofit::TargetStatus::Skipped(_)),
        "backend should be Skipped"
    );

    let frontend = by_display.get("crm/frontend/").unwrap();
    assert!(
        matches!(frontend.status, airlock::retrofit::TargetStatus::Done),
        "frontend should be Done"
    );
    assert!(project_dir
        .join("frontend/.devcontainer/devcontainer.json")
        .exists());
}

#[test]
fn test_detect_stack_via_module() {
    // Verifies wiring: airlock::detect::detect_stack is reachable from integration tests
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    assert_eq!(airlock::detect::detect_stack(tmp.path()), Some("rust"));
}

#[test]
fn post_create_scripts_do_not_init_projects() {
    // The empty project/ shell is bootstrapped by the user. post-create.sh must
    // not create any manifest at the workspace root, otherwise generators like
    // create-next-app would refuse to run. The `! -f <manifest>` guards are the
    // distinctive signature of the old init blocks.
    let guards = [
        "! -f package.json",
        "! -f Cargo.toml",
        "! -f pyproject.toml",
        "! -f foundry.toml",
    ];
    for stack_id in [
        "typescript",
        "javascript",
        "rust",
        "python",
        "solidity",
        "solidity-ts",
        "minimal",
    ] {
        let body =
            airlock::stack::StackRegistry::get_file_content(stack_id, "post-create.sh").unwrap();
        for guard in guards {
            assert!(
                !body.contains(guard),
                "{} post-create.sh still guards `{}`",
                stack_id,
                guard
            );
        }
    }
}
