use std::{env, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Config {
	pub clockify_api_key: String,
	pub solidtime_api_token: String,
	pub clockify_base_url: String,
	pub solidtime_base_url: String,
	pub clockify_workspace_id: Option<String>,
	pub solidtime_organization_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
	clockify_api_key: Option<String>,
	solidtime_api_token: Option<String>,
	clockify_base_url: Option<String>,
	solidtime_base_url: Option<String>,
	clockify_workspace_id: Option<String>,
	solidtime_organization_id: Option<String>,
}

impl Config {
	pub fn load(path: Option<&Path>) -> Result<Self> {
		let file = match path {
			Some(path) => {
				let contents = fs::read_to_string(path).with_context(|| format!("failed to read config file {}", path.display()))?;
				toml::from_str::<FileConfig>(&contents).with_context(|| format!("failed to parse config file {}", path.display()))?
			}
			None => FileConfig::default(),
		};

		Ok(Self {
			clockify_api_key: value(file.clockify_api_key, "CLOCKIFY_API_KEY")?,
			solidtime_api_token: value(file.solidtime_api_token, "SOLIDTIME_API_TOKEN")?,
			clockify_base_url: optional_value(file.clockify_base_url, "CLOCKIFY_BASE_URL").unwrap_or_else(|| "https://api.clockify.me/api/v1".to_string()),
			solidtime_base_url: optional_value(file.solidtime_base_url, "SOLIDTIME_BASE_URL").unwrap_or_else(|| "https://app.solidtime.io/api".to_string()),
			clockify_workspace_id: optional_value(file.clockify_workspace_id, "CLOCKIFY_WORKSPACE_ID"),
			solidtime_organization_id: optional_value(file.solidtime_organization_id, "SOLIDTIME_ORGANIZATION_ID"),
		})
	}
}

fn value(file_value: Option<String>, env_key: &str) -> Result<String> {
	optional_value(file_value, env_key).with_context(|| format!("{env_key} is required"))
}

fn optional_value(file_value: Option<String>, env_key: &str) -> Option<String> {
	file_value.or_else(|| env::var(env_key).ok()).filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
	use super::*;

	use std::{fs, path::Path};

	use tempfile::tempdir;

	const ENV_KEYS: &[&str] = &[
		"CLOCKIFY_API_KEY",
		"SOLIDTIME_API_TOKEN",
		"CLOCKIFY_WORKSPACE_ID",
		"SOLIDTIME_ORGANIZATION_ID",
		"CLOCKIFY_BASE_URL",
		"SOLIDTIME_BASE_URL",
	];

	#[test]
	fn loads_dotenv_values_with_expected_precedence() -> Result<()> {
		clear_env();

		let dir = tempdir()?;
		let env_path = dir.path().join(".env");
		fs::write(
			&env_path,
			[
				"CLOCKIFY_API_KEY=dotenv-clockify",
				"SOLIDTIME_API_TOKEN=dotenv-solidtime",
				"CLOCKIFY_WORKSPACE_ID=dotenv-workspace",
				"SOLIDTIME_ORGANIZATION_ID=dotenv-organization",
				"CLOCKIFY_BASE_URL=https://clockify.example.test",
				"SOLIDTIME_BASE_URL=https://solidtime.example.test",
				"",
			]
			.join("\n"),
		)?;

		dotenvy::from_path(&env_path)?;
		let config = Config::load(None)?;
		assert_eq!(config.clockify_api_key, "dotenv-clockify");
		assert_eq!(config.solidtime_api_token, "dotenv-solidtime");
		assert_eq!(config.clockify_workspace_id.as_deref(), Some("dotenv-workspace"));
		assert_eq!(config.solidtime_organization_id.as_deref(), Some("dotenv-organization"));
		assert_eq!(config.clockify_base_url, "https://clockify.example.test");
		assert_eq!(config.solidtime_base_url, "https://solidtime.example.test");

		clear_env();
		set_env("CLOCKIFY_API_KEY", "real-clockify");
		dotenvy::from_path(&env_path)?;
		let config = Config::load(None)?;
		assert_eq!(config.clockify_api_key, "real-clockify");
		assert_eq!(config.solidtime_api_token, "dotenv-solidtime");

		let toml_path = dir.path().join("config.toml");
		write_config(
			&toml_path,
			r#"
clockify_api_key = "toml-clockify"
solidtime_api_token = "toml-solidtime"
clockify_workspace_id = "toml-workspace"
solidtime_organization_id = "toml-organization"
clockify_base_url = "https://clockify.toml.test"
solidtime_base_url = "https://solidtime.toml.test"
"#,
		)?;
		let config = Config::load(Some(&toml_path))?;
		assert_eq!(config.clockify_api_key, "toml-clockify");
		assert_eq!(config.solidtime_api_token, "toml-solidtime");
		assert_eq!(config.clockify_workspace_id.as_deref(), Some("toml-workspace"));
		assert_eq!(config.solidtime_organization_id.as_deref(), Some("toml-organization"));
		assert_eq!(config.clockify_base_url, "https://clockify.toml.test");
		assert_eq!(config.solidtime_base_url, "https://solidtime.toml.test");

		let invalid_env_path = dir.path().join("invalid.env");
		fs::write(&invalid_env_path, "INVALID LINE\n")?;
		let err = dotenvy::from_path(&invalid_env_path).expect_err("invalid .env must fail");
		assert!(!err.not_found());

		clear_env();
		Ok(())
	}

	fn write_config(path: &Path, contents: &str) -> Result<()> {
		fs::write(path, contents.trim_start())?;
		Ok(())
	}

	fn clear_env() {
		for key in ENV_KEYS {
			remove_env(key);
		}
	}

	fn set_env(key: &str, value: &str) {
		// These tests mutate a small, known set of environment variables in one test.
		unsafe { env::set_var(key, value) };
	}

	fn remove_env(key: &str) {
		// These tests mutate a small, known set of environment variables in one test.
		unsafe { env::remove_var(key) };
	}
}
