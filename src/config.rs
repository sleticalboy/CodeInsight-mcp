use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const PROJECT_CONFIG_PATH: &str = ".codeinsight/config.toml";
const SAMPLE_PROJECT_CONFIG_TEMPLATE: &str = r#"# CodeInsight project configuration.
#
# This file is optional. impact_analysis uses built-in suggested check
# inference until you add project-specific commands here.

[javascript]
# Package exports/imports condition priority for package.json resolution.
# package_conditions = ["types", "import", "node", "default"]

[impact_analysis]
test_commands = {test_commands}

# Global commands run for every impact report:
# test_commands = ["pnpm test", "cargo test --locked"]

# Focused commands can match impacted languages and file path prefixes.
# [[impact_analysis.suggested_checks]]
# command = "pnpm exec vitest run src/core.test.ts"
# reason = "Run the focused core test."
# languages = ["typescript", "tsx"]
# files = ["src/core"]
"#;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub impact_analysis: ImpactAnalysisConfig,
    pub javascript: JavascriptConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct JavascriptConfig {
    pub package_conditions: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ImpactAnalysisConfig {
    pub test_commands: Vec<String>,
    pub suggested_checks: Vec<ConfiguredSuggestedCheck>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
    fs::write(&path, sample_project_config(root))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok((path, overwritten))
}

pub fn suggested_test_commands_for_root(root: &Path) -> Vec<String> {
    let mut commands = Vec::new();

    if root.join("Cargo.toml").exists() {
        commands.push("cargo test --locked".to_string());
    }

    if root.join("pnpm-lock.yaml").exists() {
        commands.push("pnpm test".to_string());
    } else if root.join("yarn.lock").exists() {
        commands.push("yarn test".to_string());
    } else if root.join("package-lock.json").exists() || root.join("package.json").exists() {
        commands.push("npm test".to_string());
    }

    if any_root_file_exists(
        root,
        &[
            "pyproject.toml",
            "pytest.ini",
            "setup.cfg",
            "setup.py",
            "tox.ini",
            "requirements.txt",
        ],
    ) {
        commands.push("pytest".to_string());
    }

    if root.join("go.mod").exists() {
        commands.push("go test ./...".to_string());
    }

    if root.join("pom.xml").exists() {
        commands.push("mvn test".to_string());
    } else if root.join("gradlew").exists() {
        commands.push("./gradlew --no-daemon test".to_string());
    } else if root.join("build.gradle").exists() || root.join("build.gradle.kts").exists() {
        commands.push("gradle test".to_string());
    }

    if has_root_child_extension(root, "csproj") {
        commands.push("dotnet test".to_string());
    }

    if root.join("Gemfile").exists() {
        commands.push("bundle exec rspec".to_string());
    }

    if root.join("composer.json").exists() {
        commands.push("composer test".to_string());
    }

    commands
}

fn sample_project_config(root: &Path) -> String {
    SAMPLE_PROJECT_CONFIG_TEMPLATE.replace(
        "{test_commands}",
        &toml_array(&suggested_test_commands_for_root(root)),
    )
}

fn toml_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn any_root_file_exists(root: &Path, files: &[&str]) -> bool {
    files.iter().any(|file| root.join(file).exists())
}

fn has_root_child_extension(root: &Path, extension: &str) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value == extension)
    })
}
