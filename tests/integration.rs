use std::path::Path;

use tempfile::TempDir;

fn scaffold_stack(stack_id: &str) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("Projects");
    let secrets_dir = tmp.path().join("secrets");
    std::fs::create_dir_all(&projects_dir).unwrap();
    std::fs::create_dir_all(&secrets_dir).unwrap();

    let name = format!("test-{}", stack_id);
    let paths = airlock::paths::ProjectPaths::new(&name, &projects_dir, &secrets_dir).unwrap();

    let stack = airlock::stack::StackRegistry::get(stack_id).unwrap();
    let opts = airlock::scaffold::ScaffoldOptions {
        needs_secrets: false,
        init_git: false,
        open_vscode: false,
    };

    airlock::scaffold::run(&paths, &stack, &opts).unwrap();

    let project_dir = paths.dir.clone();
    (tmp, project_dir)
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
fn test_devcontainer_security_settings() {
    for stack_id in &["minimal", "typescript", "python", "rust"] {
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
fn test_slug_paths() {
    use airlock::paths::slugify;
    assert_eq!(slugify("My Cool Project"), "my-cool-project");
    assert_eq!(slugify("hello   world"), "hello-world");
    assert_eq!(slugify("--test--"), "test");
}
