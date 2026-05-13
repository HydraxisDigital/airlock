use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub name: String,
    pub slug: String,
    pub dir: PathBuf,
    pub secret_file: PathBuf,
}

impl ProjectPaths {
    pub fn new(name: &str, projects_dir: &Path, secrets_dir: &Path) -> Result<Self> {
        let slug = slugify(name);
        if slug.is_empty() {
            bail!("Project name does not produce a valid slug: '{}'", name);
        }

        let dir = projects_dir.join(&slug);
        if dir.exists() {
            bail!("Directory {} already exists.", dir.display());
        }

        let secret_file = secrets_dir.join(&slug).join("env");

        Ok(Self {
            name: name.to_string(),
            slug,
            dir,
            secret_file,
        })
    }
}

pub fn slugify(input: &str) -> String {
    let s = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();

    // collapse consecutive dashes, strip leading/trailing
    let mut result = String::with_capacity(s.len());
    let mut prev_dash = true; // treat start as if preceded by dash
    for c in s.chars() {
        if c == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    // strip trailing dash
    result.trim_end_matches('-').to_string()
}

pub fn default_projects_dir() -> PathBuf {
    std::env::var("PROJECTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_home().join("Projects"))
}

pub fn default_secrets_dir() -> PathBuf {
    std::env::var("SECRETS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_home().join(".secrets"))
}

pub fn default_age_recipients() -> PathBuf {
    std::env::var("AGE_RECIPIENTS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_home().join(".age").join("recipients.txt"))
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn expand_tilde(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed == "~" {
        dirs_home()
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        dirs_home().join(rest)
    } else {
        PathBuf::from(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_basic() {
        assert_eq!(slugify("My Cool Project"), "my-cool-project");
    }

    #[test]
    fn slug_unicode() {
        // Non-ASCII chars become dashes, consecutive dashes collapse
        assert_eq!(slugify("Projet Été 2024"), "projet-t-2024");
        let s = slugify("Été");
        assert!(!s.is_empty());
        assert!(!s.starts_with('-'));
        assert!(!s.ends_with('-'));
    }

    #[test]
    fn slug_multiple_spaces() {
        assert_eq!(slugify("hello   world"), "hello-world");
    }

    #[test]
    fn slug_leading_trailing_special() {
        assert_eq!(slugify("--my-project--"), "my-project");
    }

    #[test]
    fn slug_numbers() {
        assert_eq!(slugify("Project 42 Alpha"), "project-42-alpha");
    }

    #[test]
    fn slug_already_valid() {
        assert_eq!(slugify("my-project"), "my-project");
    }

    #[test]
    fn expand_tilde_alone() {
        std::env::set_var("HOME", "/home/test");
        assert_eq!(expand_tilde("~"), PathBuf::from("/home/test"));
    }

    #[test]
    fn expand_tilde_with_path() {
        std::env::set_var("HOME", "/home/test");
        assert_eq!(
            expand_tilde("~/projects"),
            PathBuf::from("/home/test/projects")
        );
    }

    #[test]
    fn expand_tilde_absolute_unchanged() {
        assert_eq!(expand_tilde("/tmp/foo"), PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn expand_tilde_relative_unchanged() {
        assert_eq!(expand_tilde("./foo"), PathBuf::from("./foo"));
    }
}
