use std::fs;
use std::path::{Path, PathBuf};

const SUBDIR_BLACKLIST: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "vendor",
    "__pycache__",
    "venv",
];

pub fn detect_substacks(project_dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let Ok(entries) = fs::read_dir(project_dir) else {
        return Vec::new();
    };

    let mut results: Vec<(PathBuf, &'static str)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if SUBDIR_BLACKLIST.contains(&name) {
            continue;
        }

        let detected = detect_stack(&path).unwrap_or("minimal");
        if detected == "minimal" {
            continue;
        }
        results.push((path, detected));
    }

    results.sort_by(|a, b| {
        a.0.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .cmp(b.0.file_name().and_then(|s| s.to_str()).unwrap_or(""))
    });
    results
}

pub fn detect_stack(project_dir: &Path) -> Option<&'static str> {
    let foundry = project_dir.join("foundry.toml").exists();
    let package_json = project_dir.join("package.json");
    let has_package = package_json.exists();

    if foundry && has_package {
        return Some("solidity-ts");
    }
    if foundry {
        return Some("solidity");
    }
    if has_package {
        if package_uses_hardhat(&package_json) {
            return Some("solidity-ts");
        }
        return Some("typescript");
    }
    if project_dir.join("Cargo.toml").exists() {
        return Some("rust");
    }
    if project_dir.join("pyproject.toml").exists()
        || project_dir.join("requirements.txt").exists()
        || project_dir.join("setup.py").exists()
    {
        return Some("python");
    }

    Some("minimal")
}

fn package_uses_hardhat(package_json: &Path) -> bool {
    let Ok(content) = fs::read_to_string(package_json) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };

    for key in ["dependencies", "devDependencies"] {
        if let Some(deps) = value.get(key).and_then(|v| v.as_object()) {
            if deps.contains_key("hardhat") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").unwrap();
    }

    #[test]
    fn detect_stack_rust() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "Cargo.toml");
        assert_eq!(detect_stack(tmp.path()), Some("rust"));
    }

    #[test]
    fn detect_stack_typescript() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_stack(tmp.path()), Some("typescript"));
    }

    #[test]
    fn detect_stack_python_pyproject() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "pyproject.toml");
        assert_eq!(detect_stack(tmp.path()), Some("python"));
    }

    #[test]
    fn detect_stack_python_requirements() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "requirements.txt");
        assert_eq!(detect_stack(tmp.path()), Some("python"));
    }

    #[test]
    fn detect_stack_python_setup_py() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "setup.py");
        assert_eq!(detect_stack(tmp.path()), Some("python"));
    }

    #[test]
    fn detect_stack_solidity() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "foundry.toml");
        assert_eq!(detect_stack(tmp.path()), Some("solidity"));
    }

    #[test]
    fn detect_stack_solidity_ts_foundry_and_package() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "foundry.toml");
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_stack(tmp.path()), Some("solidity-ts"));
    }

    #[test]
    fn detect_stack_solidity_ts_hardhat() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{"devDependencies": {"hardhat": "^2.0"}}"#,
        )
        .unwrap();
        assert_eq!(detect_stack(tmp.path()), Some("solidity-ts"));
    }

    #[test]
    fn detect_stack_minimal_empty() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_stack(tmp.path()), Some("minimal"));
    }

    fn mkdir_with(dir: &Path, sub: &str, manifest: &str) {
        let p = dir.join(sub);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join(manifest), "").unwrap();
    }

    #[test]
    fn detect_substacks_monorepo() {
        let tmp = TempDir::new().unwrap();
        mkdir_with(tmp.path(), "backend", "pyproject.toml");
        fs::create_dir_all(tmp.path().join("frontend")).unwrap();
        fs::write(tmp.path().join("frontend/package.json"), "{}").unwrap();
        // node_modules nested package.json must be ignored
        let nm = tmp.path().join("node_modules/foo");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("package.json"), "{}").unwrap();

        let out = detect_substacks(tmp.path());
        let names: Vec<(String, &str)> = out
            .iter()
            .map(|(p, s)| (p.file_name().unwrap().to_string_lossy().into_owned(), *s))
            .collect();
        assert_eq!(
            names,
            vec![
                ("backend".to_string(), "python"),
                ("frontend".to_string(), "typescript"),
            ]
        );
    }

    #[test]
    fn detect_substacks_skips_blacklist() {
        let tmp = TempDir::new().unwrap();
        mkdir_with(tmp.path(), "target", "Cargo.toml");
        mkdir_with(tmp.path(), "dist", "package.json");
        mkdir_with(tmp.path(), "node_modules", "package.json");
        let out = detect_substacks(tmp.path());
        assert!(out.is_empty(), "expected empty, got: {:?}", out);
    }

    #[test]
    fn detect_substacks_skips_dotdirs() {
        let tmp = TempDir::new().unwrap();
        mkdir_with(tmp.path(), ".foo", "pyproject.toml");
        mkdir_with(tmp.path(), ".venv", "setup.py");
        let out = detect_substacks(tmp.path());
        assert!(out.is_empty(), "expected empty, got: {:?}", out);
    }
}
