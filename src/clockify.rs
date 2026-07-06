use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde::de::DeserializeOwned;

use crate::{
	http::HttpClient,
	models::{ClockifyClient, ClockifyProject, ClockifyTag, ClockifyTask, ClockifyTimeEntry, ClockifyUser, ClockifyWorkspace},
};

pub struct ClockifyApi {
	http: HttpClient,
	base_url: String,
	api_key: String,
}

#[derive(Debug, Clone)]
pub struct TimeWindow {
	pub start: DateTime<Utc>,
	pub end: DateTime<Utc>,
}

impl ClockifyApi {
	pub fn new(base_url: String, api_key: String) -> Result<Self> {
		Ok(Self {
			http: HttpClient::new()?,
			base_url: base_url.trim_end_matches('/').to_string(),
			api_key,
		})
	}

	pub fn get_user(&self) -> Result<ClockifyUser> {
		self.get("/user", &[])
	}

	pub fn list_workspaces(&self) -> Result<Vec<ClockifyWorkspace>> {
		self.get("/workspaces", &[])
	}

	pub fn list_clients(&self, workspace_id: &str) -> Result<Vec<ClockifyClient>> {
		let mut by_id = BTreeMap::new();
		for archived in ["false", "true"] {
			for client in self.get_paged::<ClockifyClient>(&format!("/workspaces/{workspace_id}/clients"), &[("archived", archived)])? {
				by_id.insert(client.id.clone(), client);
			}
		}
		Ok(by_id.into_values().collect())
	}

	pub fn list_projects(&self, workspace_id: &str) -> Result<Vec<ClockifyProject>> {
		let mut by_id = BTreeMap::new();
		for archived in ["false", "true"] {
			for project in self.get_paged::<ClockifyProject>(&format!("/workspaces/{workspace_id}/projects"), &[("archived", archived)])? {
				by_id.insert(project.id.clone(), project);
			}
		}
		Ok(by_id.into_values().collect())
	}

	pub fn list_tasks(&self, workspace_id: &str, project_id: &str) -> Result<Vec<ClockifyTask>> {
		self.get_paged(&format!("/workspaces/{workspace_id}/projects/{project_id}/tasks"), &[])
	}

	pub fn list_tags(&self, workspace_id: &str) -> Result<Vec<ClockifyTag>> {
		self.get_paged(&format!("/workspaces/{workspace_id}/tags"), &[])
	}

	pub fn list_time_entries(&self, workspace_id: &str, user_id: &str, window: &TimeWindow) -> Result<Vec<ClockifyTimeEntry>> {
		let start = format_rfc3339(window.start);
		let end = format_rfc3339(window.end);
		self.get_paged(&format!("/workspaces/{workspace_id}/user/{user_id}/time-entries"), &[("start", start.as_str()), ("end", end.as_str())])
	}

	fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
		let request = self.http.client().get(format!("{}{}", self.base_url, path)).header("X-Api-Key", &self.api_key).query(query);
		self.http.send_json(request)
	}

	fn get_paged<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<Vec<T>> {
		let mut items = Vec::new();
		let mut page = 1;

		loop {
			let page_string = page.to_string();
			let mut params = query.to_vec();
			params.push(("page", page_string.as_str()));
			params.push(("page-size", "5000"));

			let request = self.http.client().get(format!("{}{}", self.base_url, path)).header("X-Api-Key", &self.api_key).query(&params);
			let response = self.http.send(request)?;
			let is_last_page = response
				.headers()
				.get("X-Last-Page")
				.and_then(|value| value.to_str().ok())
				.is_some_and(|value| value.eq_ignore_ascii_case("true"));
			let page_items = response.json::<Vec<T>>().context("failed to deserialize Clockify page")?;
			let is_empty = page_items.is_empty();
			items.extend(page_items);

			if is_empty || is_last_page {
				break;
			}
			page += 1;
		}

		Ok(items)
	}
}

pub fn month_windows(from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<TimeWindow> {
	let mut windows = Vec::new();
	let mut cursor = from;

	while cursor < to {
		let (year, month) = if cursor.month() == 12 { (cursor.year() + 1, 1) } else { (cursor.year(), cursor.month() + 1) };
		let next_month = Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0).single().expect("valid first day of month");
		let end = next_month.min(to);
		windows.push(TimeWindow { start: cursor, end });
		cursor = end;
	}

	windows
}

pub fn format_rfc3339(value: DateTime<Utc>) -> String {
	value.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub(crate) fn ignored_archived_project_ids(projects: &[ClockifyProject], ignore_archived: bool) -> BTreeSet<String> {
	if !ignore_archived {
		return BTreeSet::new();
	}
	projects.iter().filter(|project| project.archived).map(|project| project.id.clone()).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn creates_month_windows() {
		let from = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
		let to = Utc.with_ymd_and_hms(2024, 3, 2, 0, 0, 0).unwrap();
		let windows = month_windows(from, to);

		assert_eq!(windows.len(), 3);
		assert_eq!(windows[0].start, from);
		assert_eq!(windows[0].end, Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap());
		assert_eq!(windows[2].end, to);
	}

	#[test]
	fn collects_archived_project_ids_only_when_ignore_archived_is_enabled() {
		let active = ClockifyProject {
			id: "active".to_string(),
			name: "Active Project".to_string(),
			client_id: None,
			color: None,
			billable: false,
			archived: false,
			estimate: None,
		};
		let archived = ClockifyProject {
			id: "archived".to_string(),
			name: "Archived Project".to_string(),
			client_id: None,
			color: None,
			billable: false,
			archived: true,
			estimate: None,
		};
		let projects = vec![active, archived];

		let ignored = ignored_archived_project_ids(&projects, true);

		assert!(ignored.contains("archived"));
		assert!(!ignored.contains("active"));
		assert!(ignored_archived_project_ids(&projects, false).is_empty());
	}
}
