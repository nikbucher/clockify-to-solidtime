use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use reqwest::StatusCode;

use crate::{
	clockify::ClockifyApi,
	config::Config,
	http::error_status,
	models::{ClockifyWorkspace, SolidtimeMembership},
	solidtime::SolidtimeApi,
};

pub struct Options {
	pub config_path: Option<PathBuf>,
}

pub struct ValidatedAccess {
	pub clockify: ClockifyApi,
	pub solidtime: SolidtimeApi,
	pub workspace: ClockifyWorkspace,
	pub membership: SolidtimeMembership,
}

pub fn run(options: Options) -> Result<()> {
	let access = validate_access(options.config_path)?;

	println!("Configuration validated");
	println!("Clockify workspace: {} ({})", access.workspace.name, access.workspace.id);
	println!("Solidtime organization: {} ({})", access.membership.organization.name, access.membership.organization.id);
	println!("No Clockify data, Solidtime data, or local migration state was changed.");

	Ok(())
}

pub fn validate_access(config_path: Option<PathBuf>) -> Result<ValidatedAccess> {
	let config = Config::load(config_path.as_deref())?;
	let clockify = ClockifyApi::new(config.clockify_base_url, config.clockify_api_key)?;
	let solidtime = SolidtimeApi::new(config.solidtime_base_url, config.solidtime_api_token)?;

	let user = clockify.get_user().map_err(|err| describe_service_failure("Clockify", "account", err))?;
	let workspace_id = config
		.clockify_workspace_id
		.or(user.default_workspace)
		.context("A Clockify workspace must be selected; set CLOCKIFY_WORKSPACE_ID or provide a default workspace")?;
	let workspace = resolve_clockify_workspace(&clockify, &workspace_id)?;

	let memberships = solidtime.list_memberships().map_err(|err| describe_service_failure("Solidtime", "account", err))?;
	let membership = resolve_solidtime_membership(memberships, config.solidtime_organization_id.as_deref())?;

	Ok(ValidatedAccess {
		clockify,
		solidtime,
		workspace,
		membership,
	})
}

pub fn resolve_clockify_workspace(clockify: &ClockifyApi, workspace_id: &str) -> Result<ClockifyWorkspace> {
	let workspaces = clockify.list_workspaces().map_err(|err| describe_service_failure("Clockify", "workspace", err))?;
	workspaces
		.into_iter()
		.find(|workspace| workspace.id == workspace_id)
		.with_context(|| format!("Clockify workspace `{workspace_id}` is not among your workspaces"))
}

pub fn resolve_solidtime_membership(memberships: Vec<SolidtimeMembership>, organization_id: Option<&str>) -> Result<SolidtimeMembership> {
	match organization_id {
		Some(id) => memberships
			.into_iter()
			.find(|membership| membership.organization.id == id)
			.with_context(|| format!("Solidtime organization `{id}` is not among your memberships")),
		None if memberships.len() == 1 => Ok(memberships.into_iter().next().expect("membership exists")),
		None => bail!("A Solidtime organization must be selected; set SOLIDTIME_ORGANIZATION_ID"),
	}
}

fn service_failure_message(service: &str, subject: &str, status: Option<StatusCode>) -> String {
	match status {
		Some(status) if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN => {
			format!("{service} rejected the configured credential; {subject} access could not be confirmed")
		}
		Some(status) => {
			format!("{service} returned {status}; {subject} access could not be confirmed")
		}
		None => format!("{service} {subject} access could not be confirmed"),
	}
}

pub fn describe_service_failure(service: &str, subject: &str, err: anyhow::Error) -> anyhow::Error {
	let status = error_status(&err);
	err.context(service_failure_message(service, subject, status))
}

#[cfg(test)]
mod tests {
	use super::*;

	use std::fs;

	use anyhow::Result;
	use httpmock::{Method::GET, MockServer};
	use serde_json::json;
	use tempfile::tempdir;

	#[test]
	fn describes_service_failures_from_status() {
		assert_eq!(
			service_failure_message("Clockify", "account", Some(StatusCode::UNAUTHORIZED)),
			"Clockify rejected the configured credential; account access could not be confirmed"
		);
		assert_eq!(
			service_failure_message("Solidtime", "account", Some(StatusCode::INTERNAL_SERVER_ERROR)),
			"Solidtime returned 500 Internal Server Error; account access could not be confirmed"
		);
		assert_eq!(service_failure_message("Clockify", "workspace", None), "Clockify workspace access could not be confirmed");
	}

	#[test]
	fn validates_configuration_with_read_only_service_checks() -> Result<()> {
		let server = MockServer::start();
		let clockify_user = server.mock(|when, then| {
			when.method(GET).path("/clockify/user");
			then.status(200).json_body(json!({
					"id": "clockify-user",
					"defaultWorkspace": "workspace-id"
			}));
		});
		let clockify_workspaces = server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces");
			then.status(200).json_body(json!([{
					"id": "workspace-id",
					"name": "Source Workspace"
			}]));
		});
		let solidtime_memberships = server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/users/me/memberships");
			then.status(200).json_body(json!({
					"data": [{
							"id": "member-id",
							"organization": {
									"id": "organization-id",
									"name": "Target Organization"
							}
					}],
					"links": {
							"next": null
					}
			}));
		});
		let dir = tempdir()?;
		let config_path = write_config(dir.path(), &server)?;

		run(Options { config_path: Some(config_path) })?;

		clockify_user.assert();
		clockify_workspaces.assert();
		solidtime_memberships.assert();
		assert!(!dir.path().join("migration-state.json").exists());
		Ok(())
	}

	#[test]
	fn requires_solidtime_organization_when_multiple_memberships_exist() -> Result<()> {
		let server = MockServer::start();
		mock_valid_clockify(&server);
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/users/me/memberships");
			then.status(200).json_body(json!({
					"data": [
							{
									"id": "member-a",
									"organization": {
											"id": "organization-a",
											"name": "Organization A"
									}
							},
							{
									"id": "member-b",
									"organization": {
											"id": "organization-b",
											"name": "Organization B"
									}
							}
					],
					"links": {
							"next": null
					}
			}));
		});
		let dir = tempdir()?;
		let config_path = write_config(dir.path(), &server)?;

		let err = run(Options { config_path: Some(config_path) }).expect_err("multiple memberships should require explicit organization");

		assert!(err.to_string().contains("A Solidtime organization must be selected"));
		assert!(!dir.path().join("migration-state.json").exists());
		Ok(())
	}

	#[test]
	fn reports_solidtime_credential_rejection() -> Result<()> {
		let server = MockServer::start();
		mock_valid_clockify(&server);
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/users/me/memberships");
			then.status(401).body("invalid token");
		});
		let dir = tempdir()?;
		let config_path = write_config(dir.path(), &server)?;

		let err = run(Options { config_path: Some(config_path) }).expect_err("401 should fail validation");

		assert!(err.to_string().contains("Solidtime rejected the configured credential; account access could not be confirmed"));
		assert!(!dir.path().join("migration-state.json").exists());
		Ok(())
	}

	fn mock_valid_clockify(server: &MockServer) {
		server.mock(|when, then| {
			when.method(GET).path("/clockify/user");
			then.status(200).json_body(json!({
					"id": "clockify-user",
					"defaultWorkspace": "workspace-id"
			}));
		});
		server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces");
			then.status(200).json_body(json!([{
					"id": "workspace-id",
					"name": "Source Workspace"
			}]));
		});
	}

	fn write_config(dir: &std::path::Path, server: &MockServer) -> Result<PathBuf> {
		let path = dir.join("config.toml");
		fs::write(
			&path,
			format!(
				r#"
clockify_api_key = "clockify-key"
solidtime_api_token = "solidtime-token"
clockify_base_url = "{}/clockify"
solidtime_base_url = "{}/solidtime"
"#,
				server.base_url(),
				server.base_url()
			)
			.trim_start(),
		)?;
		Ok(path)
	}
}
