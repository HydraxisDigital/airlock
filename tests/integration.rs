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
    let stack = airlock::stack::StackRegistry::get(stack_id).unwrap();
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
    assert!(!project_dir.join("src").exists());
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
    let stack = airlock::stack::StackRegistry::get("rust").unwrap();
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
        let stack = airlock::stack::StackRegistry::get(stack_id).unwrap();
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
        let stack = airlock::stack::StackRegistry::get(stack_id).unwrap();
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
