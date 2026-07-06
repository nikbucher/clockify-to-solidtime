use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
	api_envelope::{ItemEnvelope, ListEnvelope, ListPage},
	clockify::format_rfc3339,
	http::HttpClient,
	models::{
		SolidtimeClient, SolidtimeClientCreate, SolidtimeMembership, SolidtimeProject, SolidtimeProjectArchive, SolidtimeProjectCreate, SolidtimeTag, SolidtimeTagCreate, SolidtimeTask,
		SolidtimeTaskCreate, SolidtimeTimeEntry, SolidtimeTimeEntryCreate,
	},
};

const SOLIDTIME_PAGE_SIZE: usize = 100;
const SOLIDTIME_TIME_ENTRY_PAGE_SIZE: usize = 500;

pub struct SolidtimeApi {
	http: HttpClient,
	base_url: String,
	token: String,
}

impl SolidtimeApi {
	pub fn new(base_url: String, token: String) -> Result<Self> {
		Ok(Self {
			http: HttpClient::new()?,
			base_url: base_url.trim_end_matches('/').to_string(),
			token,
		})
	}

	pub fn membership(&self, organization_id: Option<&str>) -> Result<SolidtimeMembership> {
		let memberships = self.list_memberships()?;
		match organization_id {
			Some(id) => memberships
				.into_iter()
				.find(|membership| membership.organization.id == id)
				.with_context(|| format!("Solidtime organization {id} not found in memberships")),
			None if memberships.len() == 1 => Ok(memberships.into_iter().next().expect("membership exists")),
			None => Err(anyhow!("multiple Solidtime memberships found; set SOLIDTIME_ORGANIZATION_ID or solidtime_organization_id in config")),
		}
	}

	pub fn list_memberships(&self) -> Result<Vec<SolidtimeMembership>> {
		self.get_list::<SolidtimeMembership>("/v1/users/me/memberships", &[])
	}

	pub fn list_clients(&self, org_id: &str) -> Result<Vec<SolidtimeClient>> {
		self.get_list(&format!("/v1/organizations/{org_id}/clients"), &[("archived", "all")])
	}

	pub fn create_client(&self, org_id: &str, body: &SolidtimeClientCreate<'_>) -> Result<SolidtimeClient> {
		self.post_item(&format!("/v1/organizations/{org_id}/clients"), body)
	}

	pub fn list_projects(&self, org_id: &str) -> Result<Vec<SolidtimeProject>> {
		self.get_list(&format!("/v1/organizations/{org_id}/projects"), &[("archived", "all")])
	}

	pub fn create_project(&self, org_id: &str, body: &SolidtimeProjectCreate<'_>) -> Result<SolidtimeProject> {
		self.post_item(&format!("/v1/organizations/{org_id}/projects"), body)
	}

	pub fn archive_project(&self, org_id: &str, project_id: &str) -> Result<()> {
		let body = SolidtimeProjectArchive {
			archived_at: format_rfc3339(Utc::now()),
			is_archived: true,
		};
		self.put_item::<_, serde_json::Value>(&format!("/v1/organizations/{org_id}/projects/{project_id}"), &body).map(|_| ())
	}

	pub fn list_tasks(&self, org_id: &str) -> Result<Vec<SolidtimeTask>> {
		self.get_list(&format!("/v1/organizations/{org_id}/tasks"), &[("done", "all")])
	}

	pub fn create_task(&self, org_id: &str, body: &SolidtimeTaskCreate<'_>) -> Result<SolidtimeTask> {
		self.post_item(&format!("/v1/organizations/{org_id}/tasks"), body)
	}

	pub fn list_tags(&self, org_id: &str) -> Result<Vec<SolidtimeTag>> {
		self.get_list(&format!("/v1/organizations/{org_id}/tags"), &[])
	}

	pub fn create_tag(&self, org_id: &str, body: &SolidtimeTagCreate<'_>) -> Result<SolidtimeTag> {
		self.post_item(&format!("/v1/organizations/{org_id}/tags"), body)
	}

	pub fn list_time_entries(&self, org_id: &str, member_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<SolidtimeTimeEntry>> {
		let start = format_rfc3339(start);
		let end = format_rfc3339(end);
		self.get_offset_list(
			&format!("/v1/organizations/{org_id}/time-entries"),
			&[("member_id", member_id), ("start", start.as_str()), ("end", end.as_str())],
		)
	}

	pub fn create_time_entry(&self, org_id: &str, body: &SolidtimeTimeEntryCreate<'_>) -> Result<SolidtimeTimeEntry> {
		self.post_item(&format!("/v1/organizations/{org_id}/time-entries"), body)
	}

	fn get_list<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<Vec<T>> {
		let mut items = Vec::new();
		let mut page = 1;

		loop {
			let page_string = page.to_string();
			let per_page_string = SOLIDTIME_PAGE_SIZE.to_string();
			let mut params = query.to_vec();
			params.push(("page", page_string.as_str()));
			params.push(("per_page", per_page_string.as_str()));

			let request = self.http.client().get(format!("{}{}", self.base_url, path)).bearer_auth(&self.token).query(&params);
			let list_page = self.http.send_json::<ListEnvelope<T>>(request)?.into_page();
			let has_next = should_fetch_next_page(&list_page);
			items.extend(list_page.items);

			if !has_next {
				break;
			}
			page += 1;
		}

		Ok(items)
	}

	fn get_offset_list<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<Vec<T>> {
		let mut items = Vec::new();
		let mut offset = 0;

		loop {
			let limit_string = SOLIDTIME_TIME_ENTRY_PAGE_SIZE.to_string();
			let offset_string = offset.to_string();
			let mut params = query.to_vec();
			params.push(("limit", limit_string.as_str()));
			params.push(("offset", offset_string.as_str()));

			let request = self.http.client().get(format!("{}{}", self.base_url, path)).bearer_auth(&self.token).query(&params);
			let list_page = self.http.send_json::<ListEnvelope<T>>(request)?.into_page();
			let count = list_page.items.len();
			items.extend(list_page.items);

			if count < SOLIDTIME_TIME_ENTRY_PAGE_SIZE {
				break;
			}
			offset += count;
		}

		Ok(items)
	}

	fn post_item<B: Serialize + ?Sized, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
		let request = self.http.client().post(format!("{}{}", self.base_url, path)).bearer_auth(&self.token).json(body);
		let envelope = self.http.send_json::<ItemEnvelope<T>>(request)?;
		Ok(envelope.into_item())
	}

	fn put_item<B: Serialize + ?Sized, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
		let request = self.http.client().put(format!("{}{}", self.base_url, path)).bearer_auth(&self.token).json(body);
		let envelope = self.http.send_json::<ItemEnvelope<T>>(request)?;
		Ok(envelope.into_item())
	}
}

fn should_fetch_next_page<T>(list_page: &ListPage<T>) -> bool {
	if list_page.has_pagination_metadata {
		list_page.has_next
	} else {
		list_page.items.len() == SOLIDTIME_PAGE_SIZE
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn stops_when_explicit_pagination_metadata_has_no_next_page() {
		let page = ListPage {
			items: vec![(); SOLIDTIME_PAGE_SIZE],
			has_next: false,
			has_pagination_metadata: true,
		};

		assert!(!should_fetch_next_page(&page));
	}

	#[test]
	fn follows_explicit_pagination_metadata_when_next_page_exists() {
		let page = ListPage {
			items: Vec::<()>::new(),
			has_next: true,
			has_pagination_metadata: true,
		};

		assert!(should_fetch_next_page(&page));
	}

	#[test]
	fn falls_back_to_full_page_heuristic_without_pagination_metadata() {
		let full_page = ListPage {
			items: vec![(); SOLIDTIME_PAGE_SIZE],
			has_next: false,
			has_pagination_metadata: false,
		};
		let short_page = ListPage {
			items: vec![(); SOLIDTIME_PAGE_SIZE - 1],
			has_next: false,
			has_pagination_metadata: false,
		};

		assert!(should_fetch_next_page(&full_page));
		assert!(!should_fetch_next_page(&short_page));
	}
}
