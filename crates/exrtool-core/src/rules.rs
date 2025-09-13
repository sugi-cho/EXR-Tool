use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub before: String,
    pub after: String,
    pub changed: bool,
}

#[derive(Debug, Clone)]
pub struct ReplacementRule {
    from: String,
    to: String,
}

impl ReplacementRule {
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
        }
    }

    pub fn apply(&self, input: &str) -> String {
        input.replace(&self.from, &self.to)
    }
}

pub fn apply_rules(paths: &[PathBuf], rules: &[ReplacementRule]) -> Result<Vec<FileChange>> {
    let mut logs = Vec::new();
    for path in paths {
        let before = fs::read_to_string(path)?;
        let mut after = before.clone();
        for rule in rules {
            after = rule.apply(&after);
        }
        let changed = before != after;
        if changed {
            fs::write(path, &after)?;
        }
        logs.push(FileChange {
            path: path.clone(),
            before,
            after,
            changed,
        });
    }
    Ok(logs)
}
