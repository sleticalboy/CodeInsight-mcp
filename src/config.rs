use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

const PROJECT_CONFIG_PATH: &str = ".codeinsight/config.toml";

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
