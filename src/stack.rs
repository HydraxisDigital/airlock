use std::collections::HashMap;

use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use thiserror::Error;

static TEMPLATES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackMeta {
    pub id: String,
    pub label: String,
    pub vscode_extensions: Vec<String>,
    pub audit_command: String,
    pub tree_command: String,
    pub remote_env: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum StackError {
    #[error("Unknown stack: '{0}'")]
    Unknown(String),
    #[error("Missing template for stack '{0}': {1}")]
    MissingTemplate(String, String),
    #[error("Parse error in stack.json for '{0}': {1}")]
    ParseError(String, String),
}

pub struct StackRegistry;

impl StackRegistry {
    pub fn all() -> Vec<StackMeta> {
        let mut stacks = Vec::new();

        for entry in TEMPLATES_DIR.dirs() {
            let name = entry.path().to_string_lossy();
            if name.starts_with('_') {
                continue;
            }
            if let Ok(meta) = Self::load_meta(entry.path().to_str().unwrap_or("")) {
                stacks.push(meta);
            }
        }

        // deterministic order
        let order = [
            "typescript",
            "rust",
            "python",
            "solidity",
            "solidity-ts",
            "minimal",
        ];
        stacks.sort_by_key(|s| order.iter().position(|&o| o == s.id).unwrap_or(usize::MAX));

        stacks
    }

    pub fn get(id: &str) -> Result<StackMeta, StackError> {
        let dir = TEMPLATES_DIR
            .get_dir(id)
            .ok_or_else(|| StackError::Unknown(id.to_string()))?;

        let json_file = dir
            .get_file(format!("{}/stack.json", id))
            .ok_or_else(|| StackError::MissingTemplate(id.to_string(), "stack.json".to_string()))?;

        let content = json_file
            .contents_utf8()
            .ok_or_else(|| StackError::ParseError(id.to_string(), "invalid UTF-8".to_string()))?;

        serde_json::from_str(content)
            .map_err(|e| StackError::ParseError(id.to_string(), e.to_string()))
    }

    pub fn get_file_content(stack_id: &str, filename: &str) -> Result<&'static str, StackError> {
        let path = format!("{}/{}", stack_id, filename);
        let file = TEMPLATES_DIR.get_file(&path).ok_or_else(|| {
            StackError::MissingTemplate(stack_id.to_string(), filename.to_string())
        })?;

        file.contents_utf8()
            .ok_or_else(|| StackError::MissingTemplate(stack_id.to_string(), filename.to_string()))
    }

    pub fn get_common_file(filename: &str) -> Option<&'static str> {
        let path = format!("_common/{}", filename);
        TEMPLATES_DIR
            .get_file(&path)
            .and_then(|f| f.contents_utf8())
    }

    fn load_meta(id: &str) -> Result<StackMeta, StackError> {
        Self::get(id)
    }
}
