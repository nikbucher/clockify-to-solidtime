use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyUser {
	pub id: String,
	pub default_workspace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyWorkspace {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyClient {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyProject {
	pub id: String,
	pub name: String,
	#[serde(default, deserialize_with = "crate::api_envelope::blank_string_as_none")]
	pub client_id: Option<String>,
	pub color: Option<String>,
	#[serde(default)]
	pub billable: bool,
	#[serde(default)]
	pub archived: bool,
	pub estimate: Option<ClockifyEstimate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyTask {
	pub id: String,
	pub name: String,
	#[serde(default, deserialize_with = "crate::api_envelope::blank_string_as_none")]
	pub project_id: Option<String>,
	pub estimate: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyTag {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyEstimate {
	pub estimate: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyTimeEntry {
	pub id: String,
	pub description: Option<String>,
	#[serde(default)]
	pub billable: bool,
	#[serde(default, deserialize_with = "crate::api_envelope::blank_string_as_none")]
	pub project_id: Option<String>,
	#[serde(default, deserialize_with = "crate::api_envelope::blank_string_as_none")]
	pub task_id: Option<String>,
	#[serde(default, deserialize_with = "crate::api_envelope::null_as_default")]
	pub tag_ids: Vec<String>,
	pub time_interval: ClockifyTimeInterval,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyTimeInterval {
	pub start: DateTime<Utc>,
	pub end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeMembership {
	pub organization: SolidtimeOrganization,
	#[serde(rename = "id")]
	pub member_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeOrganization {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeClient {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeProject {
	pub id: String,
	pub name: String,
	pub client_id: Option<String>,
	#[serde(default)]
	pub is_archived: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeTask {
	pub id: String,
	pub name: String,
	pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeTag {
	pub id: String,
	pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidtimeTimeEntry {
	pub id: String,
	pub project_id: Option<String>,
	pub task_id: Option<String>,
	pub start: DateTime<Utc>,
	pub end: Option<DateTime<Utc>>,
	#[serde(default)]
	pub billable: bool,
	pub description: Option<String>,
	#[serde(default, deserialize_with = "crate::api_envelope::null_as_default")]
	pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SolidtimeClientCreate<'a> {
	pub name: &'a str,
}

#[derive(Debug, Serialize)]
pub struct SolidtimeProjectCreate<'a> {
	pub name: &'a str,
	pub color: &'a str,
	pub is_billable: bool,
	pub billable_rate: Option<i64>,
	pub client_id: Option<&'a str>,
	pub estimated_time: Option<i64>,
	pub is_public: bool,
}

#[derive(Debug, Serialize)]
pub struct SolidtimeProjectArchive {
	pub archived_at: String,
	pub is_archived: bool,
}

#[derive(Debug, Serialize)]
pub struct SolidtimeTaskCreate<'a> {
	pub name: &'a str,
	pub project_id: &'a str,
	pub estimated_time: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SolidtimeTagCreate<'a> {
	pub name: &'a str,
}

#[derive(Debug, Serialize)]
pub struct SolidtimeTimeEntryCreate<'a> {
	pub member_id: &'a str,
	pub project_id: Option<&'a str>,
	pub task_id: Option<&'a str>,
	pub start: String,
	pub end: Option<String>,
	pub billable: bool,
	pub description: Option<&'a str>,
	pub tags: Vec<&'a str>,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::api_envelope::ListEnvelope;

	#[test]
	fn deserializes_solidtime_membership_id_as_member_id() {
		let envelope: ListEnvelope<SolidtimeMembership> = serde_json::from_value(serde_json::json!({
				"data": [{
						"id": "member-id",
						"organization": {
								"id": "org-id",
								"name": "Organization",
								"currency": "CHF"
						},
						"role": "owner"
				}]
		}))
		.expect("membership envelope should deserialize");

		let memberships = envelope.into_page().items;

		assert_eq!(memberships[0].member_id, "member-id");
		assert_eq!(memberships[0].organization.id, "org-id");
		assert_eq!(memberships[0].organization.name, "Organization");
	}

	#[test]
	fn deserializes_clockify_time_entry_null_tag_ids_as_empty() {
		let entry: ClockifyTimeEntry = serde_json::from_value(serde_json::json!({
				"id": "entry-id",
				"description": "work",
				"billable": true,
				"projectId": "project-id",
				"taskId": null,
				"tagIds": null,
				"timeInterval": {
						"start": "2026-01-01T10:00:00Z",
						"end": "2026-01-01T11:00:00Z"
				}
		}))
		.expect("Clockify time entry should deserialize");

		assert!(entry.tag_ids.is_empty());
	}

	#[test]
	fn deserializes_blank_clockify_relationship_ids_as_none() {
		let entry: ClockifyTimeEntry = serde_json::from_value(serde_json::json!({
				"id": "entry-id",
				"description": "work",
				"billable": true,
				"projectId": "project-id",
				"taskId": "",
				"tagIds": [],
				"timeInterval": {
						"start": "2026-01-01T10:00:00Z",
						"end": "2026-01-01T11:00:00Z"
				}
		}))
		.expect("Clockify time entry should deserialize");

		assert_eq!(entry.project_id.as_deref(), Some("project-id"));
		assert_eq!(entry.task_id, None);
	}

	#[test]
	fn deserializes_solidtime_time_entry_tag_ids() {
		let envelope: ListEnvelope<SolidtimeTimeEntry> = serde_json::from_value(serde_json::json!({
				"data": [{
						"id": "entry-id",
						"start": "2026-01-01T10:00:00Z",
						"end": "2026-01-01T11:00:00Z",
						"duration": 3600,
						"description": "work",
						"task_id": null,
						"project_id": "project-id",
						"organization_id": "org-id",
						"user_id": "user-id",
						"tags": ["tag-id"],
						"billable": true
				}],
				"meta": {
						"total": 1
				}
		}))
		.expect("time entry envelope should deserialize");

		let entries = envelope.into_page().items;

		assert_eq!(entries[0].tags, ["tag-id"]);
	}
}
