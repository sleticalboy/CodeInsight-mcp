use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const PROJECT_CONFIG_PATH: &str = ".codeinsight/config.toml";
const SAMPLE_PROJECT_CONFIG: &str = r#"# CodeInsight project configuration.
#
# This file is optional. impact_analysis uses built-in suggested check
# inference until you add project-specific commands here.

[impact_analysis]
test_commands = []

# Global commands run for every impact report:
# test_commands = ["pnpm test", "cargo test --locked"]

# Focused commands can match impacted languages and file path prefixes.
# [[impact_analysis.suggested_checks]]
# command = "pnpm exec vitest run src/core.test.ts"
# reason = "Run the focused core test."
# languages = ["typescript", "tsx"]
# files = ["src/core"]
"#;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub impact_analysis: ImpactAnalysisConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ImpactAnalysisConfig {
    pub test_commands: Vec<String>,
    pub suggested_checks: Vec<ConfiguredSuggestedCheck>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ConfiguredSuggestedCheck {
    pub command: String,
    pub reason: Option<String>,
    pub languages: Vec<String>,
    pub files: Vec<String>,
}

pub fn load_project_config(root: &Path) -> Result<Option<ProjectConfig>> {
    let path = root.join(PROJECT_CONFIG_PATH);
    if !path.exists() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let config = toml::from_str::<ProjectConfig>(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(config))
}

pub fn project_config_path() -> &'static str {
    PROJECT_CONFIG_PATH
}

pub fn init_project_config(root: &Path, force: bool) -> Result<(std::path::PathBuf, bool)> {
    let path = root.join(PROJECT_CONFIG_PATH);
    if path.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite it",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let overwritten = path.exists();
    fs::write(&path, SAMPLE_PROJECT_CONFIG)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok((path, overwritten))
}
