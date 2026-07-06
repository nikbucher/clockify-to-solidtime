use std::{
	collections::{BTreeMap, BTreeSet, VecDeque},
	path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, TimeZone, Utc};
use reqwest::StatusCode;

use crate::{
	clockify::{ClockifyApi, format_rfc3339, ignored_archived_project_ids, month_windows},
	config::Config,
	fingerprint::time_entry_fingerprint,
	http::error_status,
	mapping::MigrationState,
	models::{
		ClockifyProject, ClockifyTask, ClockifyTimeEntry, SolidtimeClient, SolidtimeClientCreate, SolidtimeProject, SolidtimeProjectCreate, SolidtimeTag, SolidtimeTagCreate, SolidtimeTask,
		SolidtimeTaskCreate, SolidtimeTimeEntry, SolidtimeTimeEntryCreate,
	},
	project_mapping::{
		ProjectMappingRow, insert_mapping, read_project_mapping_rows, resolve_clockify_project, resolve_clockify_task, resolve_solidtime_project, single_match, task_project_id, try_resolve_solidtime_task,
	},
	solidtime::SolidtimeApi,
};

pub struct Options {
	pub dry_run: bool,
	pub config_path: Option<PathBuf>,
	pub state_path: PathBuf,
	pub mapping_path: Option<PathBuf>,
	pub create_structure: bool,
	pub ignore_archived: bool,
	pub from: Option<DateTime<Utc>>,
	pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
struct ProjectMappings {
	projects: BTreeMap<String, String>,
	tasks: BTreeMap<String, String>,
	default_tasks: BTreeMap<String, String>,
}

struct ProjectMappingContext<'a> {
	solidtime: &'a SolidtimeApi,
	org_id: &'a str,
	clockify_projects: &'a [ClockifyProject],
	clockify_tasks: &'a [ClockifyTask],
	solid_projects: &'a [SolidtimeProject],
	solid_tasks: &'a mut Vec<SolidtimeTask>,
	state: &'a MigrationState,
	dry_run: bool,
	create_structure: bool,
}

#[derive(Debug, Default)]
struct Summary {
	clients_created: usize,
	clients_reused: usize,
	projects_created: usize,
	projects_reused: usize,
	tasks_created: usize,
	tasks_reused: usize,
	tags_created: usize,
	tags_reused: usize,
	time_entries_created: usize,
	time_entries_reused: usize,
	projects_archived: usize,
	archive_failures: usize,
}

pub fn run(options: Options) -> Result<()> {
	let config = Config::load(options.config_path.as_deref())?;
	let clockify = ClockifyApi::new(config.clockify_base_url, config.clockify_api_key)?;
	let solidtime = SolidtimeApi::new(config.solidtime_base_url, config.solidtime_api_token)?;
	let mut state = MigrationState::load(&options.state_path, options.dry_run)?;
	let mut summary = Summary::default();

	let membership = solidtime.membership(config.solidtime_organization_id.as_deref())?;
	let org_id = membership.organization.id;
	let member_id = membership.member_id;
	let user = clockify.get_user()?;
	let workspace_id = config
		.clockify_workspace_id
		.or(user.default_workspace)
		.context("Clockify workspace id is required; user has no default workspace")?;
	let from = options.from.unwrap_or_else(|| Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).single().expect("valid default timestamp"));
	let to = options.to.unwrap_or_else(Utc::now);
	if from >= to {
		bail!("--from must be before --to");
	}
	let cutoff = format_rfc3339(to);

	println!("Reading Clockify workspace {workspace_id} and Solidtime organization {org_id}");
	if options.dry_run {
		println!("Dry-run: Solidtime writes and state persistence are disabled");
	}
	if !options.create_structure {
		println!("No-create-structure: missing Solidtime clients, projects, tasks, and tags will abort real migrations");
	}

	let clients = clockify.list_clients(&workspace_id)?;
	let mut projects = clockify.list_projects(&workspace_id)?;
	let ignored_archived_project_ids = ignored_archived_project_ids(&projects, options.ignore_archived);
	if options.ignore_archived {
		projects.retain(|project| !ignored_archived_project_ids.contains(&project.id));
		println!(
			"Ignore-archived: skipped {} archived Clockify projects and their tasks/time entries",
			ignored_archived_project_ids.len()
		);
	}
	let mut tasks = Vec::new();
	for project in &projects {
		tasks.extend(
			clockify
				.list_tasks(&workspace_id, &project.id)
				.with_context(|| format!("failed to list Clockify tasks for project {}", project.id))?,
		);
	}
	let tags = clockify.list_tags(&workspace_id)?;

	let solid_clients = solidtime.list_clients(&org_id)?;
	let solid_projects = solidtime.list_projects(&org_id)?;
	let mut solid_tasks = solidtime.list_tasks(&org_id)?;
	let solid_tags = solidtime.list_tags(&org_id)?;
	let project_mappings = load_project_mappings(
		options.mapping_path.as_deref(),
		ProjectMappingContext {
			solidtime: &solidtime,
			org_id: &org_id,
			clockify_projects: &projects,
			clockify_tasks: &tasks,
			solid_projects: &solid_projects,
			solid_tasks: &mut solid_tasks,
			state: &state,
			dry_run: options.dry_run,
			create_structure: options.create_structure,
		},
	)?;

	for (clockify_id, solidtime_id) in &project_mappings.projects {
		state.put_project(clockify_id, solidtime_id)?;
	}
	for (clockify_id, solidtime_id) in &project_mappings.tasks {
		state.put_task(clockify_id, solidtime_id)?;
	}

	for client in &clients {
		if state.clients.contains_key(&client.id) {
			summary.clients_reused += 1;
			continue;
		}
		let existing = single_match(solid_clients.iter().filter(|candidate| candidate.name == client.name), "client", &client.name)?;
		let solid_id = if let Some(existing) = existing {
			summary.clients_reused += 1;
			existing.id.clone()
		} else if options.dry_run {
			summary.clients_created += 1;
			format!("dry-run-client-{}", client.id)
		} else if !options.create_structure {
			bail!(
				"missing Solidtime client {:?} for Clockify client {}; rerun without --no-create-structure to create it",
				client.name,
				client.id
			);
		} else {
			match create_client_or_adopt(&solidtime, &org_id, &client.name)? {
				CreateOutcome::Created(client) => {
					summary.clients_created += 1;
					client.id
				}
				CreateOutcome::Adopted(client) => {
					summary.clients_reused += 1;
					client.id
				}
			}
		};
		state.put_client(&client.id, &solid_id)?;
	}

	for project in &projects {
		if state.projects.contains_key(&project.id) {
			summary.projects_reused += 1;
			continue;
		}
		let client_id = project.client_id.as_ref().and_then(|id| state.clients.get(id).map(String::as_str));
		let existing = single_match(
			solid_projects.iter().filter(|candidate| candidate.name == project.name && candidate.client_id.as_deref() == client_id),
			"project",
			&project.name,
		)?;
		let solid_id = if let Some(existing) = existing {
			summary.projects_reused += 1;
			existing.id.clone()
		} else if options.dry_run {
			summary.projects_created += 1;
			format!("dry-run-project-{}", project.id)
		} else if !options.create_structure {
			bail!(
				"missing Solidtime project {:?} for Clockify project {}; add it to --mapping, create it in Solidtime, or rerun without --no-create-structure",
				project.name,
				project.id
			);
		} else {
			let body = SolidtimeProjectCreate {
				name: &project.name,
				color: normalized_color(project.color.as_deref()),
				is_billable: project.billable,
				billable_rate: None,
				client_id,
				estimated_time: project.estimate.as_ref().and_then(|estimate| estimate.estimate.as_deref()).and_then(parse_iso8601_duration_seconds),
				is_public: false,
			};
			match create_project_or_adopt(&solidtime, &org_id, &body)? {
				CreateOutcome::Created(project) => {
					summary.projects_created += 1;
					project.id
				}
				CreateOutcome::Adopted(project) => {
					summary.projects_reused += 1;
					project.id
				}
			}
		};
		state.put_project(&project.id, &solid_id)?;
	}

	for task in &tasks {
		let project_id = task_project_id(task).and_then(|id| state.projects.get(id).map(String::as_str));
		let Some(project_id) = project_id else {
			println!("Skipping task '{}' because its project is missing", task.name);
			continue;
		};
		if state.tasks.contains_key(&task.id) {
			summary.tasks_reused += 1;
			continue;
		}
		let existing = single_match(
			solid_tasks.iter().filter(|candidate| candidate.name == task.name && candidate.project_id == project_id),
			"task",
			&task.name,
		)?;
		let solid_id = if let Some(existing) = existing {
			summary.tasks_reused += 1;
			existing.id.clone()
		} else if options.dry_run {
			summary.tasks_created += 1;
			format!("dry-run-task-{}", task.id)
		} else if !options.create_structure {
			bail!(
				"missing Solidtime task {:?} for Clockify task {}; add it to --mapping, create it in Solidtime, or rerun without --no-create-structure",
				task.name,
				task.id
			);
		} else {
			let body = SolidtimeTaskCreate {
				name: &task.name,
				project_id,
				estimated_time: task.estimate.as_deref().and_then(parse_iso8601_duration_seconds),
			};
			match create_task_or_adopt(&solidtime, &org_id, &body)? {
				CreateOutcome::Created(task) => {
					summary.tasks_created += 1;
					task.id
				}
				CreateOutcome::Adopted(task) => {
					summary.tasks_reused += 1;
					task.id
				}
			}
		};
		state.put_task(&task.id, &solid_id)?;
	}

	for tag in &tags {
		if state.tags.contains_key(&tag.id) {
			summary.tags_reused += 1;
			continue;
		}
		let existing = single_match(solid_tags.iter().filter(|candidate| candidate.name == tag.name), "tag", &tag.name)?;
		let solid_id = if let Some(existing) = existing {
			summary.tags_reused += 1;
			existing.id.clone()
		} else if options.dry_run {
			summary.tags_created += 1;
			format!("dry-run-tag-{}", tag.id)
		} else if !options.create_structure {
			bail!("missing Solidtime tag {:?} for Clockify tag {}; rerun without --no-create-structure to create it", tag.name, tag.id);
		} else {
			match create_tag_or_adopt(&solidtime, &org_id, &tag.name)? {
				CreateOutcome::Created(tag) => {
					summary.tags_created += 1;
					tag.id
				}
				CreateOutcome::Adopted(tag) => {
					summary.tags_reused += 1;
					tag.id
				}
			}
		};
		state.put_tag(&tag.id, &solid_id)?;
	}

	for window in month_windows(from, to) {
		let clockify_entries = clockify.list_time_entries(&workspace_id, &user.id, &window)?;
		let existing_entries = solidtime.list_time_entries(&org_id, &member_id, window.start, window.end)?;
		let mut existing_fingerprints = fingerprint_existing_entries(&existing_entries, &member_id);

		for entry in &clockify_entries {
			if time_entry_uses_ignored_project(entry, &ignored_archived_project_ids) {
				continue;
			}
			if state.time_entries.contains_key(&entry.id) {
				summary.time_entries_reused += 1;
				continue;
			}
			let Some(body) = time_entry_body(entry, &member_id, &state, &project_mappings)? else {
				println!("Skipping time entry {} because it has a task but no project", entry.id);
				continue;
			};
			let fingerprint = time_entry_fingerprint(&body);
			if let Some(existing_id) = existing_fingerprints.get_mut(&fingerprint).and_then(VecDeque::pop_front) {
				summary.time_entries_reused += 1;
				state.put_time_entry(&entry.id, &existing_id)?;
				continue;
			}
			if options.dry_run {
				summary.time_entries_created += 1;
				state.put_time_entry(&entry.id, &format!("dry-run-time-entry-{}", entry.id))?;
			} else {
				let created = solidtime.create_time_entry(&org_id, &body).with_context(|| format!("failed to create time entry {}", entry.id))?;
				summary.time_entries_created += 1;
				state.put_time_entry(&entry.id, &created.id)?;
			}
		}
	}

	for project in projects.iter().filter(|project| project.archived) {
		let Some(project_id) = state.projects.get(&project.id) else {
			continue;
		};
		if options.dry_run {
			summary.projects_archived += 1;
			continue;
		}
		match solidtime.archive_project(&org_id, project_id) {
			Ok(()) => summary.projects_archived += 1,
			Err(err) => {
				summary.archive_failures += 1;
				println!("Could not archive project '{}': {err:#}", project.name);
			}
		}
	}

	state.set_cutoff(cutoff)?;
	print_summary(&summary);
	Ok(())
}

fn load_project_mappings(path: Option<&Path>, ctx: ProjectMappingContext<'_>) -> Result<ProjectMappings> {
	let Some(path) = path else {
		return Ok(ProjectMappings::default());
	};

	let rows = read_project_mapping_rows(path)?;
	resolve_project_mapping_rows(rows, ctx).with_context(|| format!("failed to resolve mapping file {}", path.display()))
}

fn resolve_project_mapping_rows(rows: Vec<ProjectMappingRow>, ctx: ProjectMappingContext<'_>) -> Result<ProjectMappings> {
	let mut mappings = ProjectMappings::default();

	for row in rows {
		let clockify_project = resolve_clockify_project(&row, ctx.clockify_projects)?;
		let solid_project = resolve_solidtime_project(&row, ctx.solid_projects)?;
		insert_mapping(
			&mut mappings.projects,
			ctx.state.projects.get(&clockify_project.id).map(String::as_str),
			&clockify_project.id,
			&solid_project.id,
			"project",
		)?;

		if !row.has_solidtime_task() {
			continue;
		}

		let solid_task_id = resolve_or_create_solidtime_task(&row, ctx.solidtime, ctx.org_id, solid_project, ctx.solid_tasks, ctx.dry_run, ctx.create_structure)?;
		if !row.has_clockify_task() {
			insert_mapping(&mut mappings.default_tasks, None, &clockify_project.id, &solid_task_id, "project default task")?;
			continue;
		}

		let clockify_task = resolve_clockify_task(&row, &clockify_project.id, ctx.clockify_tasks)?;
		insert_mapping(
			&mut mappings.tasks,
			ctx.state.tasks.get(&clockify_task.id).map(String::as_str),
			&clockify_task.id,
			&solid_task_id,
			"task",
		)?;
	}

	Ok(mappings)
}

fn resolve_or_create_solidtime_task(
	row: &ProjectMappingRow,
	solidtime: &SolidtimeApi,
	org_id: &str,
	project: &SolidtimeProject,
	tasks: &mut Vec<SolidtimeTask>,
	dry_run: bool,
	create_structure: bool,
) -> Result<String> {
	if let Some(task) = try_resolve_solidtime_task(row, project, tasks)? {
		return Ok(task.id.clone());
	}

	if dry_run {
		return Ok(format!("dry-run-mapped-task-{}-{}", project.id, row.solidtime_task));
	}
	if !create_structure {
		bail!(
			"mapped Solidtime task {:?} under project {:?} does not exist; create it in Solidtime or rerun without --no-create-structure",
			row.solidtime_task,
			project.name
		);
	}

	let body = SolidtimeTaskCreate {
		name: &row.solidtime_task,
		project_id: &project.id,
		estimated_time: None,
	};
	match create_task_or_adopt(solidtime, org_id, &body)? {
		CreateOutcome::Created(task) | CreateOutcome::Adopted(task) => {
			let id = task.id.clone();
			tasks.push(task);
			Ok(id)
		}
	}
}

enum CreateOutcome<T> {
	Created(T),
	Adopted(T),
}

fn create_client_or_adopt(solidtime: &SolidtimeApi, org_id: &str, name: &str) -> Result<CreateOutcome<SolidtimeClient>> {
	match solidtime.create_client(org_id, &SolidtimeClientCreate { name }) {
		Ok(created) => Ok(CreateOutcome::Created(created)),
		Err(error) if is_create_conflict(&error) => {
			let clients = solidtime.list_clients(org_id)?;
			let existing = single_match(clients.iter().filter(|candidate| candidate.name == name), "client", name)?
				.with_context(|| format!("client {name:?} create conflicted, but no matching Solidtime client was found"))?;
			Ok(CreateOutcome::Adopted(existing.clone()))
		}
		Err(error) => Err(error).with_context(|| format!("failed to create client {name}")),
	}
}

fn create_project_or_adopt(solidtime: &SolidtimeApi, org_id: &str, body: &SolidtimeProjectCreate<'_>) -> Result<CreateOutcome<SolidtimeProject>> {
	match solidtime.create_project(org_id, body) {
		Ok(created) => Ok(CreateOutcome::Created(created)),
		Err(error) if is_create_conflict(&error) => {
			let projects = solidtime.list_projects(org_id)?;
			let existing = single_match(
				projects.iter().filter(|candidate| candidate.name == body.name && candidate.client_id.as_deref() == body.client_id),
				"project",
				body.name,
			)?
			.with_context(|| format!("project {:?} create conflicted, but no matching Solidtime project was found", body.name))?;
			Ok(CreateOutcome::Adopted(existing.clone()))
		}
		Err(error) => Err(error).with_context(|| format!("failed to create project {}", body.name)),
	}
}

fn create_task_or_adopt(solidtime: &SolidtimeApi, org_id: &str, body: &SolidtimeTaskCreate<'_>) -> Result<CreateOutcome<SolidtimeTask>> {
	match solidtime.create_task(org_id, body) {
		Ok(created) => Ok(CreateOutcome::Created(created)),
		Err(error) if is_create_conflict(&error) => {
			let tasks = solidtime.list_tasks(org_id)?;
			let existing = single_match(
				tasks.iter().filter(|candidate| candidate.name == body.name && candidate.project_id == body.project_id),
				"task",
				body.name,
			)?
			.with_context(|| format!("task {:?} create conflicted, but no matching Solidtime task was found", body.name))?;
			Ok(CreateOutcome::Adopted(existing.clone()))
		}
		Err(error) => Err(error).with_context(|| format!("failed to create task {}", body.name)),
	}
}

fn create_tag_or_adopt(solidtime: &SolidtimeApi, org_id: &str, name: &str) -> Result<CreateOutcome<SolidtimeTag>> {
	match solidtime.create_tag(org_id, &SolidtimeTagCreate { name }) {
		Ok(created) => Ok(CreateOutcome::Created(created)),
		Err(error) if is_create_conflict(&error) => {
			let tags = solidtime.list_tags(org_id)?;
			let existing =
				single_match(tags.iter().filter(|candidate| candidate.name == name), "tag", name)?.with_context(|| format!("tag {name:?} create conflicted, but no matching Solidtime tag was found"))?;
			Ok(CreateOutcome::Adopted(existing.clone()))
		}
		Err(error) => Err(error).with_context(|| format!("failed to create tag {name}")),
	}
}

fn is_create_conflict(error: &anyhow::Error) -> bool {
	error_status(error).is_some_and(|status| status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::CONFLICT)
}

fn normalized_color(color: Option<&str>) -> &str {
	color.filter(|value| value.len() == 7 && value.starts_with('#')).unwrap_or("#4F46E5")
}

fn time_entry_uses_ignored_project(entry: &ClockifyTimeEntry, ignored_project_ids: &BTreeSet<String>) -> bool {
	entry.project_id.as_deref().is_some_and(|project_id| ignored_project_ids.contains(project_id))
}

fn time_entry_body<'a>(entry: &'a ClockifyTimeEntry, member_id: &'a str, state: &'a MigrationState, project_mappings: &'a ProjectMappings) -> Result<Option<SolidtimeTimeEntryCreate<'a>>> {
	let project_id = match entry.project_id.as_ref() {
		Some(id) => Some(state.projects.get(id).with_context(|| format!("missing project mapping for Clockify project {id}"))?.as_str()),
		None => None,
	};
	let task_id = match (entry.project_id.as_ref(), entry.task_id.as_ref()) {
		(Some(_), Some(task_id)) if project_mappings.tasks.contains_key(task_id) => Some(
			project_mappings
				.tasks
				.get(task_id)
				.with_context(|| format!("missing explicit task mapping for Clockify task {task_id}"))?
				.as_str(),
		),
		(Some(project_id), _) if project_mappings.default_tasks.contains_key(project_id) => Some(
			project_mappings
				.default_tasks
				.get(project_id)
				.with_context(|| format!("missing default task mapping for Clockify project {project_id}"))?
				.as_str(),
		),
		(_, Some(task_id)) => Some(state.tasks.get(task_id).with_context(|| format!("missing task mapping for Clockify task {task_id}"))?.as_str()),
		_ => None,
	};
	let mut tags = Vec::with_capacity(entry.tag_ids.len());
	for tag_id in &entry.tag_ids {
		tags.push(state.tags.get(tag_id).with_context(|| format!("missing tag mapping for Clockify tag {tag_id}"))?.as_str());
	}
	if task_id.is_some() && project_id.is_none() {
		return Ok(None);
	}
	Ok(Some(SolidtimeTimeEntryCreate {
		member_id,
		project_id,
		task_id,
		start: format_rfc3339(entry.time_interval.start),
		end: entry.time_interval.end.map(format_rfc3339),
		billable: entry.billable,
		description: entry.description.as_deref(),
		tags,
	}))
}

fn fingerprint_existing_entries(entries: &[SolidtimeTimeEntry], member_id: &str) -> BTreeMap<String, VecDeque<String>> {
	let mut fingerprints = BTreeMap::<String, VecDeque<String>>::new();
	for entry in entries {
		let tag_ids = entry.tags.iter().map(String::as_str).collect::<Vec<_>>();
		let body = SolidtimeTimeEntryCreate {
			member_id,
			project_id: entry.project_id.as_deref(),
			task_id: entry.task_id.as_deref(),
			start: format_rfc3339(entry.start),
			end: entry.end.map(format_rfc3339),
			billable: entry.billable,
			description: entry.description.as_deref(),
			tags: tag_ids,
		};
		fingerprints.entry(time_entry_fingerprint(&body)).or_default().push_back(entry.id.clone());
	}
	fingerprints
}

pub fn parse_iso8601_duration_seconds(value: &str) -> Option<i64> {
	if !value.starts_with('P') {
		return None;
	}
	let mut seconds = 0_i64;
	let mut number = String::new();
	let mut in_time = false;
	for ch in value.chars().skip(1) {
		match ch {
			'T' => in_time = true,
			'0'..='9' => number.push(ch),
			'D' => {
				seconds += number.parse::<i64>().ok()? * 86_400;
				number.clear();
			}
			'H' if in_time => {
				seconds += number.parse::<i64>().ok()? * 3_600;
				number.clear();
			}
			'M' if in_time => {
				seconds += number.parse::<i64>().ok()? * 60;
				number.clear();
			}
			'S' if in_time => {
				seconds += number.parse::<i64>().ok()?;
				number.clear();
			}
			_ => return None,
		}
	}
	if number.is_empty() { Some(seconds) } else { None }
}

fn print_summary(summary: &Summary) {
	println!("Migration summary");
	println!("  clients: {} created, {} reused", summary.clients_created, summary.clients_reused);
	println!(
		"  projects: {} created, {} reused, {} archived, {} archive failures",
		summary.projects_created, summary.projects_reused, summary.projects_archived, summary.archive_failures
	);
	println!("  tasks: {} created, {} reused", summary.tasks_created, summary.tasks_reused);
	println!("  tags: {} created, {} reused", summary.tags_created, summary.tags_reused);
	println!("  time entries: {} created, {} reused", summary.time_entries_created, summary.time_entries_reused);
}

#[cfg(test)]
mod tests {
	use super::*;
	use httpmock::prelude::HttpMockRequest;
	use httpmock::{Method::GET, Method::POST, Mock, MockServer};
	use serde_json::{Value, json};
	use std::{
		fs,
		sync::atomic::{AtomicUsize, Ordering},
	};

	static CLIENT_LIST_GETS: AtomicUsize = AtomicUsize::new(0);
	static PROJECT_LIST_GETS: AtomicUsize = AtomicUsize::new(0);

	fn clockify_project(id: &str, name: &str) -> ClockifyProject {
		ClockifyProject {
			id: id.to_string(),
			name: name.to_string(),
			client_id: None,
			color: None,
			billable: false,
			archived: false,
			estimate: None,
		}
	}

	fn clockify_archived_project(id: &str, name: &str) -> ClockifyProject {
		ClockifyProject {
			archived: true,
			..clockify_project(id, name)
		}
	}

	fn clockify_task(id: &str, name: &str, project_id: &str) -> ClockifyTask {
		ClockifyTask {
			id: id.to_string(),
			name: name.to_string(),
			project_id: Some(project_id.to_string()),
			estimate: None,
		}
	}

	fn clockify_time_entry(id: &str, project_id: Option<&str>) -> ClockifyTimeEntry {
		ClockifyTimeEntry {
			id: id.to_string(),
			description: None,
			billable: false,
			project_id: project_id.map(str::to_string),
			task_id: None,
			tag_ids: vec![],
			time_interval: crate::models::ClockifyTimeInterval {
				start: Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
				end: None,
			},
		}
	}

	fn solid_project(id: &str, name: &str) -> SolidtimeProject {
		SolidtimeProject {
			id: id.to_string(),
			name: name.to_string(),
			client_id: None,
			is_archived: false,
		}
	}

	fn solid_task(id: &str, name: &str, project_id: &str) -> SolidtimeTask {
		SolidtimeTask {
			id: id.to_string(),
			name: name.to_string(),
			project_id: project_id.to_string(),
		}
	}

	fn mapping_row(clockify_project: &str, clockify_task: &str, solidtime_project: &str, solidtime_task: &str) -> ProjectMappingRow {
		ProjectMappingRow {
			clockify_project: clockify_project.to_string(),
			clockify_task: clockify_task.to_string(),
			solidtime_project: solidtime_project.to_string(),
			solidtime_task: solidtime_task.to_string(),
			clockify_project_id: String::new(),
			clockify_task_id: String::new(),
			solidtime_project_id: String::new(),
			solidtime_task_id: String::new(),
		}
	}

	fn mapping_context<'a>(
		solidtime: &'a SolidtimeApi,
		clockify_projects: &'a [ClockifyProject],
		clockify_tasks: &'a [ClockifyTask],
		solid_projects: &'a [SolidtimeProject],
		solid_tasks: &'a mut Vec<SolidtimeTask>,
		state: &'a MigrationState,
		dry_run: bool,
	) -> ProjectMappingContext<'a> {
		ProjectMappingContext {
			solidtime,
			org_id: "org",
			clockify_projects,
			clockify_tasks,
			solid_projects,
			solid_tasks,
			state,
			dry_run,
			create_structure: false,
		}
	}

	fn write_config(dir: &std::path::Path, server: &MockServer) -> Result<PathBuf> {
		let path = dir.join("config.toml");
		fs::write(
			&path,
			format!(
				r#"
clockify_api_key = "clockify-key"
solidtime_api_token = "solidtime-token"
clockify_workspace_id = "workspace-1"
solidtime_organization_id = "org-1"
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

	fn run_migration(config_path: PathBuf, state_path: PathBuf, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<()> {
		run(Options {
			dry_run: false,
			config_path: Some(config_path),
			state_path,
			mapping_path: None,
			create_structure: true,
			ignore_archived: false,
			from: Some(from),
			to: Some(to),
		})
	}

	fn test_window() -> (DateTime<Utc>, DateTime<Utc>) {
		(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(), Utc.with_ymd_and_hms(2024, 1, 2, 0, 0, 0).unwrap())
	}

	fn state_at(path: &Path) -> MigrationState {
		MigrationState::load(path, false).unwrap()
	}

	fn list_envelope(data: Vec<Value>) -> Value {
		json!({
				"data": data,
				"meta": {
						"current_page": 1,
						"last_page": 1
				}
		})
	}

	fn item_envelope(data: Value) -> Value {
		json!({ "data": data })
	}

	fn clockify_client_json(id: &str, name: &str) -> Value {
		json!({ "id": id, "name": name })
	}

	fn clockify_project_json(id: &str, name: &str, client_id: Option<&str>) -> Value {
		json!({
				"id": id,
				"name": name,
				"clientId": client_id,
				"color": "#123456",
				"billable": false,
				"archived": false,
				"estimate": null
		})
	}

	struct TimeEntryJson<'a> {
		id: &'a str,
		project_id: Option<&'a str>,
		task_id: Option<&'a str>,
		tags: Vec<&'a str>,
		start: DateTime<Utc>,
		end: DateTime<Utc>,
		description: Option<&'a str>,
		billable: bool,
	}

	fn clockify_time_entry_json(entry: TimeEntryJson<'_>) -> Value {
		json!({
				"id": entry.id,
				"description": entry.description,
				"billable": entry.billable,
				"projectId": entry.project_id,
				"taskId": entry.task_id,
				"tagIds": entry.tags,
				"timeInterval": {
						"start": format_rfc3339(entry.start),
						"end": format_rfc3339(entry.end)
				}
		})
	}

	fn solid_client_json(id: &str, name: &str) -> Value {
		json!({ "id": id, "name": name })
	}

	fn solid_project_json(id: &str, name: &str, client_id: Option<&str>) -> Value {
		json!({
				"id": id,
				"name": name,
				"client_id": client_id,
				"is_archived": false
		})
	}

	fn solid_time_entry_json(entry: TimeEntryJson<'_>) -> Value {
		json!({
				"id": entry.id,
				"project_id": entry.project_id,
				"task_id": entry.task_id,
				"tags": entry.tags,
				"start": format_rfc3339(entry.start),
				"end": format_rfc3339(entry.end),
				"description": entry.description,
				"billable": entry.billable
		})
	}

	fn mock_bootstrap(server: &MockServer) {
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/users/me/memberships");
			then.status(200).json_body(list_envelope(vec![json!({
					"id": "member-1",
					"organization": {
							"id": "org-1",
							"name": "Organization"
					}
			})]));
		});
		server.mock(|when, then| {
			when.method(GET).path("/clockify/user");
			then.status(200).json_body(json!({
					"id": "user-1",
					"defaultWorkspace": "workspace-1"
			}));
		});
	}

	fn mock_clockify_clients(server: &MockServer, clients: Vec<Value>) {
		for (archived, body) in [("false", clients), ("true", vec![])] {
			server.mock(|when, then| {
				when
					.method(GET)
					.path("/clockify/workspaces/workspace-1/clients")
					.query_param("archived", archived)
					.query_param("page", "1")
					.query_param("page-size", "5000");
				then.status(200).header("X-Last-Page", "true").json_body(json!(body));
			});
		}
	}

	fn mock_clockify_projects(server: &MockServer, projects: Vec<Value>) {
		for (archived, body) in [("false", projects), ("true", vec![])] {
			server.mock(|when, then| {
				when
					.method(GET)
					.path("/clockify/workspaces/workspace-1/projects")
					.query_param("archived", archived)
					.query_param("page", "1")
					.query_param("page-size", "5000");
				then.status(200).header("X-Last-Page", "true").json_body(json!(body));
			});
		}
	}

	fn mock_clockify_tasks(server: &MockServer, project_id: &str, tasks: Vec<Value>) {
		server.mock(|when, then| {
			when
				.method(GET)
				.path(format!("/clockify/workspaces/workspace-1/projects/{project_id}/tasks"))
				.query_param("page", "1")
				.query_param("page-size", "5000");
			then.status(200).header("X-Last-Page", "true").json_body(json!(tasks));
		});
	}

	fn mock_clockify_tags(server: &MockServer, tags: Vec<Value>) {
		server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-1/tags").query_param("page", "1").query_param("page-size", "5000");
			then.status(200).header("X-Last-Page", "true").json_body(json!(tags));
		});
	}

	fn mock_clockify_time_entries(server: &MockServer, entries: Vec<Value>) {
		server.mock(|when, then| {
			when
				.method(GET)
				.path("/clockify/workspaces/workspace-1/user/user-1/time-entries")
				.query_param("page", "1")
				.query_param("page-size", "5000");
			then.status(200).header("X-Last-Page", "true").json_body(json!(entries));
		});
	}

	fn mock_solid_clients(server: &MockServer, clients: Vec<Value>) {
		server.mock(|when, then| {
			when
				.method(GET)
				.path("/solidtime/v1/organizations/org-1/clients")
				.query_param("archived", "all")
				.query_param("page", "1")
				.query_param("per_page", "100");
			then.status(200).json_body(list_envelope(clients));
		});
	}

	fn mock_solid_projects(server: &MockServer, projects: Vec<Value>) {
		server.mock(|when, then| {
			when
				.method(GET)
				.path("/solidtime/v1/organizations/org-1/projects")
				.query_param("archived", "all")
				.query_param("page", "1")
				.query_param("per_page", "100");
			then.status(200).json_body(list_envelope(projects));
		});
	}

	fn mock_solid_tasks(server: &MockServer, tasks: Vec<Value>) {
		server.mock(|when, then| {
			when
				.method(GET)
				.path("/solidtime/v1/organizations/org-1/tasks")
				.query_param("done", "all")
				.query_param("page", "1")
				.query_param("per_page", "100");
			then.status(200).json_body(list_envelope(tasks));
		});
	}

	fn mock_solid_tags(server: &MockServer, tags: Vec<Value>) {
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/org-1/tags").query_param("page", "1").query_param("per_page", "100");
			then.status(200).json_body(list_envelope(tags));
		});
	}

	fn mock_solid_time_entries(server: &MockServer, entries: Vec<Value>) -> Mock<'_> {
		server.mock(|when, then| {
			when
				.method(GET)
				.path("/solidtime/v1/organizations/org-1/time-entries")
				.query_param("member_id", "member-1")
				.query_param("limit", "500")
				.query_param("offset", "0");
			then.status(200).json_body(list_envelope(entries));
		})
	}

	fn mock_create_project<'a>(server: &'a MockServer, id: &str, name: &str, client_id: Option<&str>) -> Mock<'a> {
		server.mock(|when, then| {
			when.method(POST).path("/solidtime/v1/organizations/org-1/projects");
			then.status(201).json_body(item_envelope(solid_project_json(id, name, client_id)));
		})
	}

	fn mock_create_time_entry<'a>(server: &'a MockServer, id: &str, project_id: Option<&str>) -> Mock<'a> {
		server.mock(|when, then| {
			when.method(POST).path("/solidtime/v1/organizations/org-1/time-entries");
			then.status(201).json_body(item_envelope(solid_time_entry_json(TimeEntryJson {
				id,
				project_id,
				task_id: None,
				tags: vec![],
				start: Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap(),
				end: Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(),
				description: Some("work"),
				billable: true,
			})));
		})
	}

	fn request_has_query(req: &HttpMockRequest, name: &str, value: &str) -> bool {
		req
			.query_params
			.as_ref()
			.is_some_and(|params| params.iter().any(|(param_name, param_value)| param_name == name && param_value == value))
	}

	fn matches_first_solid_client_list(req: &HttpMockRequest) -> bool {
		req.path == "/solidtime/v1/organizations/org-1/clients"
			&& request_has_query(req, "archived", "all")
			&& request_has_query(req, "page", "1")
			&& request_has_query(req, "per_page", "100")
			&& CLIENT_LIST_GETS.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_ok()
	}

	fn matches_recovery_solid_client_list(req: &HttpMockRequest) -> bool {
		req.path == "/solidtime/v1/organizations/org-1/clients"
			&& request_has_query(req, "archived", "all")
			&& request_has_query(req, "page", "1")
			&& request_has_query(req, "per_page", "100")
			&& CLIENT_LIST_GETS.load(Ordering::SeqCst) >= 1
	}

	fn matches_first_solid_project_list(req: &HttpMockRequest) -> bool {
		req.path == "/solidtime/v1/organizations/org-1/projects"
			&& request_has_query(req, "archived", "all")
			&& request_has_query(req, "page", "1")
			&& request_has_query(req, "per_page", "100")
			&& PROJECT_LIST_GETS.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_ok()
	}

	fn matches_recovery_solid_project_list(req: &HttpMockRequest) -> bool {
		req.path == "/solidtime/v1/organizations/org-1/projects"
			&& request_has_query(req, "archived", "all")
			&& request_has_query(req, "page", "1")
			&& request_has_query(req, "per_page", "100")
			&& PROJECT_LIST_GETS.load(Ordering::SeqCst) >= 1
	}

	#[test]
	fn adopts_existing_client_after_create_conflict() {
		let dir = tempfile::tempdir().unwrap();
		let server = MockServer::start();
		let config_path = write_config(dir.path(), &server).unwrap();
		let state_path = dir.path().join("state.json");
		let (from, to) = test_window();
		mock_bootstrap(&server);
		mock_clockify_clients(&server, vec![clockify_client_json("clockify-client-1", "Client A")]);
		mock_clockify_projects(&server, vec![]);
		mock_clockify_tags(&server, vec![]);
		mock_solid_projects(&server, vec![]);
		mock_solid_tasks(&server, vec![]);
		mock_solid_tags(&server, vec![]);
		mock_clockify_time_entries(&server, vec![]);
		mock_solid_time_entries(&server, vec![]);
		CLIENT_LIST_GETS.store(0, Ordering::SeqCst);
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/org-1/clients").matches(matches_first_solid_client_list);
			then.status(200).json_body(list_envelope(vec![]));
		});
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/org-1/clients").matches(matches_recovery_solid_client_list);
			then.status(200).json_body(list_envelope(vec![solid_client_json("solid-client-1", "Client A")]));
		});
		let create_client = server.mock(|when, then| {
			when.method(POST).path("/solidtime/v1/organizations/org-1/clients");
			then.status(422).json_body(json!({ "message": "already exists" }));
		});

		run_migration(config_path, state_path.clone(), from, to).unwrap();

		create_client.assert();
		assert_eq!(state_at(&state_path).clients.get("clockify-client-1").map(String::as_str), Some("solid-client-1"));
	}

	#[test]
	fn adopts_existing_project_after_422_conflict() {
		let dir = tempfile::tempdir().unwrap();
		let server = MockServer::start();
		let config_path = write_config(dir.path(), &server).unwrap();
		let state_path = dir.path().join("state.json");
		let (from, to) = test_window();
		mock_bootstrap(&server);
		mock_clockify_clients(&server, vec![clockify_client_json("clockify-client-1", "Client A")]);
		mock_clockify_projects(&server, vec![clockify_project_json("clockify-project-1", "Project A", Some("clockify-client-1"))]);
		mock_clockify_tasks(&server, "clockify-project-1", vec![]);
		mock_clockify_tags(&server, vec![]);
		mock_solid_clients(&server, vec![solid_client_json("solid-client-1", "Client A")]);
		mock_solid_tasks(&server, vec![]);
		mock_solid_tags(&server, vec![]);
		mock_clockify_time_entries(&server, vec![]);
		mock_solid_time_entries(&server, vec![]);
		PROJECT_LIST_GETS.store(0, Ordering::SeqCst);
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/org-1/projects").matches(matches_first_solid_project_list);
			then.status(200).json_body(list_envelope(vec![]));
		});
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/org-1/projects").matches(matches_recovery_solid_project_list);
			then
				.status(200)
				.json_body(list_envelope(vec![solid_project_json("solid-project-1", "Project A", Some("solid-client-1"))]));
		});
		let create_project = server.mock(|when, then| {
			when.method(POST).path("/solidtime/v1/organizations/org-1/projects");
			then.status(422).json_body(json!({ "message": "already exists" }));
		});

		run_migration(config_path, state_path.clone(), from, to).unwrap();

		create_project.assert();
		assert_eq!(state_at(&state_path).projects.get("clockify-project-1").map(String::as_str), Some("solid-project-1"));
	}

	#[test]
	fn aborts_when_conflict_recovery_finds_multiple_matches() {
		let dir = tempfile::tempdir().unwrap();
		let server = MockServer::start();
		let config_path = write_config(dir.path(), &server).unwrap();
		let state_path = dir.path().join("state.json");
		let (from, to) = test_window();
		mock_bootstrap(&server);
		mock_clockify_clients(&server, vec![clockify_client_json("clockify-client-1", "Client A")]);
		mock_clockify_projects(&server, vec![]);
		mock_clockify_tags(&server, vec![]);
		mock_solid_clients(&server, vec![solid_client_json("solid-client-1", "Client A"), solid_client_json("solid-client-2", "Client A")]);
		mock_solid_projects(&server, vec![]);
		mock_solid_tasks(&server, vec![]);
		mock_solid_tags(&server, vec![]);

		let err = run_migration(config_path, state_path, from, to).unwrap_err();

		assert!(format!("{err:#}").contains("multiple Solidtime client records match"));
	}

	#[test]
	fn paginates_solidtime_time_entries_before_fingerprint_matching() {
		let dir = tempfile::tempdir().unwrap();
		let server = MockServer::start();
		let config_path = write_config(dir.path(), &server).unwrap();
		let state_path = dir.path().join("state.json");
		let (from, to) = test_window();
		let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
		let end = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
		mock_bootstrap(&server);
		mock_clockify_clients(&server, vec![]);
		mock_clockify_projects(&server, vec![clockify_project_json("clockify-project-1", "Project A", None)]);
		mock_clockify_tasks(&server, "clockify-project-1", vec![]);
		mock_clockify_tags(&server, vec![]);
		mock_clockify_time_entries(
			&server,
			vec![clockify_time_entry_json(TimeEntryJson {
				id: "clockify-entry-1",
				project_id: Some("clockify-project-1"),
				task_id: None,
				tags: vec![],
				start,
				end,
				description: Some("work"),
				billable: true,
			})],
		);
		mock_solid_clients(&server, vec![]);
		mock_solid_projects(&server, vec![solid_project_json("solid-project-1", "Project A", None)]);
		mock_solid_tasks(&server, vec![]);
		mock_solid_tags(&server, vec![]);
		let first_page = (0..500)
			.map(|idx| {
				let id = format!("existing-entry-{idx}");
				solid_time_entry_json(TimeEntryJson {
					id: &id,
					project_id: Some("other-project"),
					task_id: None,
					tags: vec![],
					start,
					end,
					description: Some("other work"),
					billable: false,
				})
			})
			.collect::<Vec<_>>();
		let first_time_entries_page = mock_solid_time_entries(&server, first_page);
		let second_time_entries_page = server.mock(|when, then| {
			when
				.method(GET)
				.path("/solidtime/v1/organizations/org-1/time-entries")
				.query_param("member_id", "member-1")
				.query_param("limit", "500")
				.query_param("offset", "500");
			then.status(200).json_body(list_envelope(vec![solid_time_entry_json(TimeEntryJson {
				id: "solid-entry-1",
				project_id: Some("solid-project-1"),
				task_id: None,
				tags: vec![],
				start,
				end,
				description: Some("work"),
				billable: true,
			})]));
		});
		let create_time_entry = mock_create_time_entry(&server, "created-entry", Some("solid-project-1"));

		run_migration(config_path, state_path.clone(), from, to).unwrap();

		first_time_entries_page.assert();
		second_time_entries_page.assert();
		assert_eq!(create_time_entry.hits(), 0);
		assert_eq!(state_at(&state_path).time_entries.get("clockify-entry-1").map(String::as_str), Some("solid-entry-1"));
	}

	#[test]
	fn second_run_reuses_state() {
		let dir = tempfile::tempdir().unwrap();
		let server = MockServer::start();
		let config_path = write_config(dir.path(), &server).unwrap();
		let state_path = dir.path().join("state.json");
		let (from, to) = test_window();
		let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
		let end = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
		mock_bootstrap(&server);
		mock_clockify_clients(&server, vec![]);
		mock_clockify_projects(&server, vec![clockify_project_json("clockify-project-1", "Project A", None)]);
		mock_clockify_tasks(&server, "clockify-project-1", vec![]);
		mock_clockify_tags(&server, vec![]);
		mock_clockify_time_entries(
			&server,
			vec![clockify_time_entry_json(TimeEntryJson {
				id: "clockify-entry-1",
				project_id: Some("clockify-project-1"),
				task_id: None,
				tags: vec![],
				start,
				end,
				description: Some("work"),
				billable: true,
			})],
		);
		mock_solid_clients(&server, vec![]);
		mock_solid_projects(&server, vec![]);
		mock_solid_tasks(&server, vec![]);
		mock_solid_tags(&server, vec![]);
		mock_solid_time_entries(&server, vec![]);
		let create_project = mock_create_project(&server, "solid-project-1", "Project A", None);
		let create_time_entry = mock_create_time_entry(&server, "solid-entry-1", Some("solid-project-1"));

		run_migration(config_path.clone(), state_path.clone(), from, to).unwrap();
		run_migration(config_path, state_path.clone(), from, to).unwrap();

		assert_eq!(create_project.hits(), 1);
		assert_eq!(create_time_entry.hits(), 1);
		let state = state_at(&state_path);
		assert_eq!(state.projects.get("clockify-project-1").map(String::as_str), Some("solid-project-1"));
		assert_eq!(state.time_entries.get("clockify-entry-1").map(String::as_str), Some("solid-entry-1"));
	}

	#[test]
	fn state_deleted_rerun_reuses_existing_time_entries_by_fingerprint() {
		let dir = tempfile::tempdir().unwrap();
		let server = MockServer::start();
		let config_path = write_config(dir.path(), &server).unwrap();
		let state_path = dir.path().join("state.json");
		let (from, to) = test_window();
		let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
		let end = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
		mock_bootstrap(&server);
		mock_clockify_clients(&server, vec![]);
		mock_clockify_projects(&server, vec![clockify_project_json("clockify-project-1", "Project A", None)]);
		mock_clockify_tasks(&server, "clockify-project-1", vec![]);
		mock_clockify_tags(&server, vec![]);
		mock_clockify_time_entries(
			&server,
			vec![clockify_time_entry_json(TimeEntryJson {
				id: "clockify-entry-1",
				project_id: Some("clockify-project-1"),
				task_id: None,
				tags: vec![],
				start,
				end,
				description: Some("work"),
				billable: true,
			})],
		);
		mock_solid_clients(&server, vec![]);
		mock_solid_projects(&server, vec![solid_project_json("solid-project-1", "Project A", None)]);
		mock_solid_tasks(&server, vec![]);
		mock_solid_tags(&server, vec![]);
		mock_solid_time_entries(
			&server,
			vec![solid_time_entry_json(TimeEntryJson {
				id: "solid-entry-1",
				project_id: Some("solid-project-1"),
				task_id: None,
				tags: vec![],
				start,
				end,
				description: Some("work"),
				billable: true,
			})],
		);
		let create_time_entry = mock_create_time_entry(&server, "created-entry", Some("solid-project-1"));

		run_migration(config_path, state_path.clone(), from, to).unwrap();

		assert_eq!(create_time_entry.hits(), 0);
		assert_eq!(state_at(&state_path).time_entries.get("clockify-entry-1").map(String::as_str), Some("solid-entry-1"));
	}

	#[test]
	fn parses_clockify_duration_subset() {
		assert_eq!(parse_iso8601_duration_seconds("PT1H30M"), Some(5_400));
		assert_eq!(parse_iso8601_duration_seconds("P1DT2H3M4S"), Some(93_784));
		assert_eq!(parse_iso8601_duration_seconds("P1M"), None);
	}

	#[test]
	fn keeps_all_existing_time_entry_fingerprint_matches() {
		let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
		let end = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
		let entries = vec![
			SolidtimeTimeEntry {
				id: "first".to_string(),
				project_id: Some("project".to_string()),
				task_id: Some("task".to_string()),
				start,
				end: Some(end),
				billable: true,
				description: Some("work".to_string()),
				tags: vec!["tag".to_string()],
			},
			SolidtimeTimeEntry {
				id: "second".to_string(),
				project_id: Some("project".to_string()),
				task_id: Some("task".to_string()),
				start,
				end: Some(end),
				billable: true,
				description: Some("work".to_string()),
				tags: vec!["tag".to_string()],
			},
		];

		let mut fingerprints = fingerprint_existing_entries(&entries, "member");
		let matches = fingerprints.values_mut().next().expect("one fingerprint");

		assert_eq!(matches.pop_front().as_deref(), Some("first"));
		assert_eq!(matches.pop_front().as_deref(), Some("second"));
		assert_eq!(matches.pop_front(), None);
	}

	#[test]
	fn collects_archived_project_ids_only_when_ignore_archived_is_enabled() {
		let projects = vec![clockify_project("active", "Active Project"), clockify_archived_project("archived", "Archived Project")];

		let ignored = ignored_archived_project_ids(&projects, true);

		assert!(ignored.contains("archived"));
		assert!(!ignored.contains("active"));
		assert!(ignored_archived_project_ids(&projects, false).is_empty());
	}

	#[test]
	fn detects_time_entries_for_ignored_archived_projects() {
		let ignored = BTreeSet::from(["archived".to_string()]);

		assert!(time_entry_uses_ignored_project(&clockify_time_entry("entry-1", Some("archived")), &ignored));
		assert!(!time_entry_uses_ignored_project(&clockify_time_entry("entry-2", Some("active")), &ignored));
	}

	#[test]
	fn keeps_time_entries_without_project_when_archived_projects_are_ignored() {
		let ignored = BTreeSet::from(["archived".to_string()]);

		assert!(!time_entry_uses_ignored_project(&clockify_time_entry("entry", None), &ignored));
	}

	#[test]
	fn parses_project_mapping_csv_shape() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("../.project_mapping.csv");
		fs::write(
			&path,
			"Clockify_Project,Clockify_Task,Solidtime_Project,Solidtime_Task\nEG ALEX: Betrieb,,EG ALEX,OPS | Operations / Betrieb\nEG ALEX: Dienstleistungen,,EG ALEX,Dienstleistungen\n",
		)
		.unwrap();

		let rows = read_project_mapping_rows(&path).unwrap();

		assert_eq!(rows.len(), 2);
		assert_eq!(rows[0].clockify_project, "EG ALEX: Betrieb");
		assert_eq!(rows[0].clockify_task, "");
		assert_eq!(rows[0].solidtime_project, "EG ALEX");
		assert_eq!(rows[0].solidtime_task, "OPS | Operations / Betrieb");
	}

	#[test]
	fn applies_blank_clockify_task_as_project_default_mapping() {
		let clockify_projects = vec![clockify_project("cp-1", "Clockify Project")];
		let clockify_tasks = vec![clockify_task("ct-1", "Clockify Task", "cp-1")];
		let solid_projects = vec![solid_project("sp-1", "Solidtime Project")];
		let mut solid_tasks = vec![solid_task("st-default", "Default Task", "sp-1"), solid_task("st-specific", "Specific Task", "sp-1")];
		let solidtime = SolidtimeApi::new("http://localhost".to_string(), "token".to_string()).unwrap();
		let state = MigrationState::default();

		let mappings = resolve_project_mapping_rows(
			vec![
				mapping_row("Clockify Project", "", "Solidtime Project", "Default Task"),
				mapping_row("Clockify Project", "Clockify Task", "Solidtime Project", "Specific Task"),
			],
			mapping_context(&solidtime, &clockify_projects, &clockify_tasks, &solid_projects, &mut solid_tasks, &state, false),
		)
		.unwrap();

		assert_eq!(mappings.projects.get("cp-1").map(String::as_str), Some("sp-1"));
		assert_eq!(mappings.default_tasks.get("cp-1").map(String::as_str), Some("st-default"));
		assert_eq!(mappings.tasks.get("ct-1").map(String::as_str), Some("st-specific"));
	}

	#[test]
	fn uses_project_default_task_for_time_entries_without_specific_mapping() {
		let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
		let entry = ClockifyTimeEntry {
			id: "entry".to_string(),
			description: None,
			billable: false,
			project_id: Some("cp-1".to_string()),
			task_id: Some("ct-1".to_string()),
			tag_ids: vec![],
			time_interval: crate::models::ClockifyTimeInterval { start, end: None },
		};
		let mut state = MigrationState::default();
		state.projects.insert("cp-1".to_string(), "sp-1".to_string());
		state.tasks.insert("ct-1".to_string(), "st-original".to_string());
		let mut mappings = ProjectMappings::default();
		mappings.default_tasks.insert("cp-1".to_string(), "st-default".to_string());

		let body = time_entry_body(&entry, "member", &state, &mappings).unwrap().unwrap();

		assert_eq!(body.project_id, Some("sp-1"));
		assert_eq!(body.task_id, Some("st-default"));
	}

	#[test]
	fn specific_task_mapping_overrides_project_default_task() {
		let start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
		let entry = ClockifyTimeEntry {
			id: "entry".to_string(),
			description: None,
			billable: false,
			project_id: Some("cp-1".to_string()),
			task_id: Some("ct-1".to_string()),
			tag_ids: vec![],
			time_interval: crate::models::ClockifyTimeInterval { start, end: None },
		};
		let mut state = MigrationState::default();
		state.projects.insert("cp-1".to_string(), "sp-1".to_string());
		let mut mappings = ProjectMappings::default();
		mappings.default_tasks.insert("cp-1".to_string(), "st-default".to_string());
		mappings.tasks.insert("ct-1".to_string(), "st-specific".to_string());

		let body = time_entry_body(&entry, "member", &state, &mappings).unwrap().unwrap();

		assert_eq!(body.task_id, Some("st-specific"));
	}

	#[test]
	fn rejects_missing_mapped_solidtime_task_with_no_create_structure() {
		let clockify_projects = vec![clockify_project("cp-1", "Clockify Project")];
		let clockify_tasks = Vec::new();
		let solid_projects = vec![solid_project("sp-1", "Solidtime Project")];
		let mut solid_tasks = Vec::new();
		let solidtime = SolidtimeApi::new("http://localhost".to_string(), "token".to_string()).unwrap();
		let state = MigrationState::default();

		let err = resolve_project_mapping_rows(
			vec![mapping_row("Clockify Project", "", "Solidtime Project", "Missing Task")],
			mapping_context(&solidtime, &clockify_projects, &clockify_tasks, &solid_projects, &mut solid_tasks, &state, false),
		)
		.unwrap_err();

		assert!(format!("{err:#}").contains("mapped Solidtime task"));
	}

	#[test]
	fn reports_missing_mapped_solidtime_task_in_dry_run() {
		let clockify_projects = vec![clockify_project("cp-1", "Clockify Project")];
		let clockify_tasks = Vec::new();
		let solid_projects = vec![solid_project("sp-1", "Solidtime Project")];
		let mut solid_tasks = Vec::new();
		let solidtime = SolidtimeApi::new("http://localhost".to_string(), "token".to_string()).unwrap();
		let state = MigrationState::default();

		let mappings = resolve_project_mapping_rows(
			vec![mapping_row("Clockify Project", "", "Solidtime Project", "Missing Task")],
			mapping_context(&solidtime, &clockify_projects, &clockify_tasks, &solid_projects, &mut solid_tasks, &state, true),
		)
		.unwrap();

		assert_eq!(mappings.default_tasks.get("cp-1").map(String::as_str), Some("dry-run-mapped-task-sp-1-Missing Task"));
	}

	#[test]
	fn rejects_state_conflict_with_csv_mapping() {
		let clockify_projects = vec![clockify_project("cp-1", "Clockify Project")];
		let clockify_tasks = Vec::new();
		let solid_projects = vec![solid_project("sp-1", "Solidtime Project")];
		let mut solid_tasks = Vec::new();
		let solidtime = SolidtimeApi::new("http://localhost".to_string(), "token".to_string()).unwrap();
		let mut state = MigrationState::default();
		state.projects.insert("cp-1".to_string(), "different-project".to_string());

		let err = resolve_project_mapping_rows(
			vec![mapping_row("Clockify Project", "", "Solidtime Project", "")],
			mapping_context(&solidtime, &clockify_projects, &clockify_tasks, &solid_projects, &mut solid_tasks, &state, false),
		)
		.unwrap_err();

		assert!(format!("{err:#}").contains("mapping conflict"));
	}
}
