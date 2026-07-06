use std::{
	collections::{BTreeMap, BTreeSet},
	path::PathBuf,
};

use anyhow::{Context, Result};

use crate::{
	clockify::ignored_archived_project_ids,
	models::{ClockifyClient, ClockifyProject, ClockifyTask, SolidtimeClient, SolidtimeProject, SolidtimeTask},
	project_mapping::{ProjectMappingRow, insert_mapping, read_project_mapping_rows, resolve_clockify_project, resolve_clockify_task, resolve_solidtime_project, resolve_solidtime_task},
	validate::{describe_service_failure, validate_access},
};

pub struct Options {
	pub config_path: Option<PathBuf>,
	pub mapping_path: Option<PathBuf>,
	pub ignore_archived: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Summary {
	projects_matched: usize,
	projects_clockify_only: usize,
	projects_solidtime_only: usize,
	projects_manual_review: usize,
	tasks_matched: usize,
	tasks_clockify_only: usize,
	tasks_solidtime_only: usize,
	tasks_manual_review: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProjectKey {
	client_name: Option<String>,
	project_name: String,
}

#[derive(Debug)]
struct Comparison {
	groups: BTreeMap<Option<String>, Vec<ProjectComparison>>,
	summary: Summary,
}

#[derive(Debug)]
struct ProjectComparison {
	clockify_name: Option<String>,
	solidtime_name: Option<String>,
	status: ProjectStatus,
	clockify_archived: Option<bool>,
	solidtime_archived: Option<bool>,
	tasks: Vec<TaskComparison>,
}

#[derive(Debug, PartialEq, Eq)]
enum ProjectStatus {
	Both,
	ClockifyOnly,
	SolidtimeOnly,
	ManualReview(String),
}

#[derive(Debug)]
struct TaskComparison {
	clockify_name: Option<String>,
	solidtime_name: Option<String>,
	status: TaskStatus,
}

#[derive(Debug, PartialEq, Eq)]
enum TaskStatus {
	Both,
	ClockifyOnly,
	SolidtimeOnly,
	ManualReview(String),
}

#[derive(Debug, Default)]
struct CompareMappings {
	projects: BTreeMap<String, String>,
	tasks: BTreeMap<String, String>,
}

pub fn run(options: Options) -> Result<()> {
	let access = validate_access(options.config_path)?;
	let workspace_id = access.workspace.id;
	let org_id = access.membership.organization.id;

	let mapping_rows = options
		.mapping_path
		.as_deref()
		.map(read_project_mapping_rows)
		.transpose()
		.with_context(|| {
			options
				.mapping_path
				.as_ref()
				.map(|path| format!("failed to read mapping file {}", path.display()))
				.unwrap_or_else(|| "failed to read mapping file".to_string())
		})?
		.unwrap_or_default();

	let clockify_clients = access
		.clockify
		.list_clients(&workspace_id)
		.map_err(|err| describe_service_failure("Clockify", "clients", err))
		.context("Clockify project comparison could not be completed")?;
	let mut clockify_projects = access
		.clockify
		.list_projects(&workspace_id)
		.map_err(|err| describe_service_failure("Clockify", "projects", err))
		.context("Clockify project comparison could not be completed")?;
	let ignored_archived_project_ids = ignored_archived_project_ids(&clockify_projects, options.ignore_archived);
	if options.ignore_archived {
		clockify_projects.retain(|project| !ignored_archived_project_ids.contains(&project.id));
	}
	if clockify_projects.is_empty() {
		println!("Clockify workspace has no projects.");
	}
	let mut clockify_tasks = Vec::new();
	for project in &clockify_projects {
		clockify_tasks.extend(
			access
				.clockify
				.list_tasks(&workspace_id, &project.id)
				.map_err(|err| describe_service_failure("Clockify", "tasks", err))
				.with_context(|| format!("Clockify project comparison could not be completed while reading tasks for project {}", project.id))?,
		);
	}

	let solidtime_clients = access
		.solidtime
		.list_clients(&org_id)
		.map_err(|err| describe_service_failure("Solidtime", "clients", err))
		.context("Solidtime project comparison could not be completed")?;
	let solidtime_projects = access
		.solidtime
		.list_projects(&org_id)
		.map_err(|err| describe_service_failure("Solidtime", "projects", err))
		.context("Solidtime project comparison could not be completed")?;
	if solidtime_projects.is_empty() {
		println!("Solidtime organization has no projects.");
	}
	let solidtime_tasks = access
		.solidtime
		.list_tasks(&org_id)
		.map_err(|err| describe_service_failure("Solidtime", "tasks", err))
		.context("Solidtime project comparison could not be completed")?;

	let mappings = resolve_compare_mappings(mapping_rows, &clockify_projects, &clockify_tasks, &solidtime_projects, &solidtime_tasks)?;
	let comparison = build_comparison(
		&clockify_clients,
		&clockify_projects,
		&clockify_tasks,
		&solidtime_clients,
		&solidtime_projects,
		&solidtime_tasks,
		&mappings,
	);
	print_comparison(&comparison);
	println!("No Clockify data, Solidtime data, or local migration state was changed.");

	Ok(())
}

fn resolve_compare_mappings(
	rows: Vec<ProjectMappingRow>,
	clockify_projects: &[ClockifyProject],
	clockify_tasks: &[ClockifyTask],
	solidtime_projects: &[SolidtimeProject],
	solidtime_tasks: &[SolidtimeTask],
) -> Result<CompareMappings> {
	let mut mappings = CompareMappings::default();

	for row in rows {
		let clockify_project = resolve_clockify_project(&row, clockify_projects)?;
		let solidtime_project = resolve_solidtime_project(&row, solidtime_projects)?;
		insert_mapping(&mut mappings.projects, None, &clockify_project.id, &solidtime_project.id, "project")?;

		if !row.has_solidtime_task() || !row.has_clockify_task() {
			continue;
		}

		let clockify_task = resolve_clockify_task(&row, &clockify_project.id, clockify_tasks)?;
		let solidtime_task = resolve_solidtime_task(&row, solidtime_project, solidtime_tasks)?;
		insert_mapping(&mut mappings.tasks, None, &clockify_task.id, &solidtime_task.id, "task")?;
	}

	Ok(mappings)
}

fn build_comparison(
	clockify_clients: &[ClockifyClient],
	clockify_projects: &[ClockifyProject],
	clockify_tasks: &[ClockifyTask],
	solidtime_clients: &[SolidtimeClient],
	solidtime_projects: &[SolidtimeProject],
	solidtime_tasks: &[SolidtimeTask],
	mappings: &CompareMappings,
) -> Comparison {
	let clockify_client_names = clockify_clients.iter().map(|client| (client.id.as_str(), client.name.as_str())).collect::<BTreeMap<_, _>>();
	let solidtime_client_names = solidtime_clients.iter().map(|client| (client.id.as_str(), client.name.as_str())).collect::<BTreeMap<_, _>>();

	let clockify_project_by_id = clockify_projects.iter().map(|project| (project.id.as_str(), project)).collect::<BTreeMap<_, _>>();
	let solidtime_project_by_id = solidtime_projects.iter().map(|project| (project.id.as_str(), project)).collect::<BTreeMap<_, _>>();
	let mapped_clockify_project_ids = mappings.projects.keys().map(String::as_str).collect::<BTreeSet<_>>();
	let mapped_solidtime_project_ids = mappings.projects.values().map(String::as_str).collect::<BTreeSet<_>>();
	let unmapped_clockify_projects = clockify_projects
		.iter()
		.filter(|project| !mapped_clockify_project_ids.contains(project.id.as_str()))
		.collect::<Vec<_>>();
	let unmapped_solidtime_projects = solidtime_projects
		.iter()
		.filter(|project| !mapped_solidtime_project_ids.contains(project.id.as_str()))
		.collect::<Vec<_>>();

	let clockify_by_key = clockify_projects_by_key(unmapped_clockify_projects.iter().copied(), &clockify_client_names);
	let solidtime_by_key = solidtime_projects_by_key(unmapped_solidtime_projects.iter().copied(), &solidtime_client_names);
	let clockify_tasks_by_project = clockify_tasks_by_project(clockify_tasks);
	let solidtime_tasks_by_project = solidtime_tasks_by_project(solidtime_tasks);

	let mut keys = BTreeSet::new();
	keys.extend(clockify_by_key.keys().cloned());
	keys.extend(solidtime_by_key.keys().cloned());

	let mut groups = BTreeMap::<Option<String>, Vec<ProjectComparison>>::new();
	let mut summary = Summary::default();

	for key in keys {
		let clockify_matches = clockify_by_key.get(&key).map(Vec::as_slice).unwrap_or(&[]);
		let solidtime_matches = solidtime_by_key.get(&key).map(Vec::as_slice).unwrap_or(&[]);
		let comparison = match (clockify_matches, solidtime_matches) {
			([clockify], [solidtime]) => {
				summary.projects_matched += 1;
				let tasks = compare_tasks(
					clockify_tasks_by_project.get(clockify.id.as_str()).map(Vec::as_slice).unwrap_or(&[]),
					solidtime_tasks_by_project.get(solidtime.id.as_str()).map(Vec::as_slice).unwrap_or(&[]),
					mappings,
					&mut summary,
				);
				ProjectComparison {
					clockify_name: Some(key.project_name.clone()),
					solidtime_name: Some(key.project_name.clone()),
					status: ProjectStatus::Both,
					clockify_archived: Some(clockify.archived),
					solidtime_archived: Some(solidtime.is_archived),
					tasks,
				}
			}
			([clockify], []) => {
				summary.projects_clockify_only += 1;
				let tasks = clockify_tasks_by_project
					.get(clockify.id.as_str())
					.into_iter()
					.flatten()
					.map(|task| {
						summary.tasks_clockify_only += 1;
						TaskComparison {
							clockify_name: Some(task.name.clone()),
							solidtime_name: None,
							status: TaskStatus::ClockifyOnly,
						}
					})
					.collect();
				ProjectComparison {
					clockify_name: Some(key.project_name.clone()),
					solidtime_name: None,
					status: ProjectStatus::ClockifyOnly,
					clockify_archived: Some(clockify.archived),
					solidtime_archived: None,
					tasks,
				}
			}
			([], [solidtime]) => {
				summary.projects_solidtime_only += 1;
				let tasks = solidtime_tasks_by_project
					.get(solidtime.id.as_str())
					.into_iter()
					.flatten()
					.map(|task| {
						summary.tasks_solidtime_only += 1;
						TaskComparison {
							clockify_name: None,
							solidtime_name: Some(task.name.clone()),
							status: TaskStatus::SolidtimeOnly,
						}
					})
					.collect();
				ProjectComparison {
					clockify_name: None,
					solidtime_name: Some(key.project_name.clone()),
					status: ProjectStatus::SolidtimeOnly,
					clockify_archived: None,
					solidtime_archived: Some(solidtime.is_archived),
					tasks,
				}
			}
			(clockify, solidtime) => {
				summary.projects_manual_review += 1;
				ProjectComparison {
					clockify_name: Some(key.project_name.clone()),
					solidtime_name: Some(key.project_name.clone()),
					status: ProjectStatus::ManualReview(format!("ambiguous project name: {} Clockify match(es), {} Solidtime match(es)", clockify.len(), solidtime.len())),
					clockify_archived: None,
					solidtime_archived: None,
					tasks: Vec::new(),
				}
			}
		};
		groups.entry(key.client_name).or_default().push(comparison);
	}

	for (clockify_id, solidtime_id) in &mappings.projects {
		let clockify = clockify_project_by_id.get(clockify_id.as_str()).expect("resolved Clockify project mapping should still exist");
		let solidtime = solidtime_project_by_id.get(solidtime_id.as_str()).expect("resolved Solidtime project mapping should still exist");
		summary.projects_matched += 1;
		let tasks = compare_tasks(
			clockify_tasks_by_project.get(clockify.id.as_str()).map(Vec::as_slice).unwrap_or(&[]),
			solidtime_tasks_by_project.get(solidtime.id.as_str()).map(Vec::as_slice).unwrap_or(&[]),
			mappings,
			&mut summary,
		);
		groups
			.entry(clockify_client_key(clockify.client_id.as_deref(), &clockify_client_names))
			.or_default()
			.push(ProjectComparison {
				clockify_name: Some(clockify.name.clone()),
				solidtime_name: Some(solidtime.name.clone()),
				status: ProjectStatus::Both,
				clockify_archived: Some(clockify.archived),
				solidtime_archived: Some(solidtime.is_archived),
				tasks,
			});
	}

	for projects in groups.values_mut() {
		projects.sort_by(|left, right| {
			comparison_sort_name(left.clockify_name.as_deref(), left.solidtime_name.as_deref()).cmp(comparison_sort_name(right.clockify_name.as_deref(), right.solidtime_name.as_deref()))
		});
	}

	Comparison { groups, summary }
}

fn compare_tasks(clockify_tasks: &[&ClockifyTask], solidtime_tasks: &[&SolidtimeTask], mappings: &CompareMappings, summary: &mut Summary) -> Vec<TaskComparison> {
	let clockify_task_by_id = clockify_tasks.iter().map(|task| (task.id.as_str(), *task)).collect::<BTreeMap<_, _>>();
	let solidtime_task_by_id = solidtime_tasks.iter().map(|task| (task.id.as_str(), *task)).collect::<BTreeMap<_, _>>();
	let mapped_clockify_task_ids = mappings.tasks.keys().map(String::as_str).collect::<BTreeSet<_>>();
	let mapped_solidtime_task_ids = mappings.tasks.values().map(String::as_str).collect::<BTreeSet<_>>();
	let clockify_by_name = group_by_name(clockify_tasks.iter().copied().filter(|task| !mapped_clockify_task_ids.contains(task.id.as_str())), |task| {
		task.name.as_str()
	});
	let solidtime_by_name = group_by_name(solidtime_tasks.iter().copied().filter(|task| !mapped_solidtime_task_ids.contains(task.id.as_str())), |task| {
		task.name.as_str()
	});
	let mut names = BTreeSet::new();
	names.extend(clockify_by_name.keys().cloned());
	names.extend(solidtime_by_name.keys().cloned());

	let mut comparisons = names
		.into_iter()
		.map(|name| {
			let clockify = clockify_by_name.get(name.as_str()).map(Vec::as_slice).unwrap_or(&[]);
			let solidtime = solidtime_by_name.get(name.as_str()).map(Vec::as_slice).unwrap_or(&[]);
			match (clockify, solidtime) {
				([_], [_]) => {
					summary.tasks_matched += 1;
					TaskComparison {
						clockify_name: Some(name.clone()),
						solidtime_name: Some(name),
						status: TaskStatus::Both,
					}
				}
				([_], []) => {
					summary.tasks_clockify_only += 1;
					TaskComparison {
						clockify_name: Some(name),
						solidtime_name: None,
						status: TaskStatus::ClockifyOnly,
					}
				}
				([], [_]) => {
					summary.tasks_solidtime_only += 1;
					TaskComparison {
						clockify_name: None,
						solidtime_name: Some(name),
						status: TaskStatus::SolidtimeOnly,
					}
				}
				(clockify, solidtime) => {
					summary.tasks_manual_review += 1;
					TaskComparison {
						clockify_name: Some(name.clone()),
						solidtime_name: Some(name),
						status: TaskStatus::ManualReview(format!("ambiguous task name: {} Clockify match(es), {} Solidtime match(es)", clockify.len(), solidtime.len())),
					}
				}
			}
		})
		.collect::<Vec<_>>();

	for (clockify_id, solidtime_id) in &mappings.tasks {
		let Some(clockify) = clockify_task_by_id.get(clockify_id.as_str()) else {
			continue;
		};
		let Some(solidtime) = solidtime_task_by_id.get(solidtime_id.as_str()) else {
			continue;
		};
		summary.tasks_matched += 1;
		comparisons.push(TaskComparison {
			clockify_name: Some(clockify.name.clone()),
			solidtime_name: Some(solidtime.name.clone()),
			status: TaskStatus::Both,
		});
	}

	comparisons.sort_by(|left, right| {
		comparison_sort_name(left.clockify_name.as_deref(), left.solidtime_name.as_deref()).cmp(comparison_sort_name(right.clockify_name.as_deref(), right.solidtime_name.as_deref()))
	});
	comparisons
}

fn comparison_sort_name<'a>(clockify_name: Option<&'a str>, solidtime_name: Option<&'a str>) -> &'a str {
	clockify_name.or(solidtime_name).unwrap_or("")
}

fn clockify_projects_by_key<'a>(projects: impl Iterator<Item = &'a ClockifyProject>, client_names: &BTreeMap<&str, &str>) -> BTreeMap<ProjectKey, Vec<&'a ClockifyProject>> {
	let mut by_key = BTreeMap::<ProjectKey, Vec<&ClockifyProject>>::new();
	for project in projects {
		by_key
			.entry(ProjectKey {
				client_name: clockify_client_key(project.client_id.as_deref(), client_names),
				project_name: project.name.clone(),
			})
			.or_default()
			.push(project);
	}
	by_key
}

fn solidtime_projects_by_key<'a>(projects: impl Iterator<Item = &'a SolidtimeProject>, client_names: &BTreeMap<&str, &str>) -> BTreeMap<ProjectKey, Vec<&'a SolidtimeProject>> {
	let mut by_key = BTreeMap::<ProjectKey, Vec<&SolidtimeProject>>::new();
	for project in projects {
		by_key
			.entry(ProjectKey {
				client_name: solidtime_client_key(project.client_id.as_deref(), client_names),
				project_name: project.name.clone(),
			})
			.or_default()
			.push(project);
	}
	by_key
}

fn clockify_client_key(client_id: Option<&str>, client_names: &BTreeMap<&str, &str>) -> Option<String> {
	client_id.map(|id| client_names.get(id).map(|name| (*name).to_string()).unwrap_or_else(|| format!("(Unknown Clockify client {id})")))
}

fn solidtime_client_key(client_id: Option<&str>, client_names: &BTreeMap<&str, &str>) -> Option<String> {
	client_id.map(|id| client_names.get(id).map(|name| (*name).to_string()).unwrap_or_else(|| format!("(Unknown Solidtime client {id})")))
}

fn clockify_tasks_by_project(tasks: &[ClockifyTask]) -> BTreeMap<&str, Vec<&ClockifyTask>> {
	let mut by_project = BTreeMap::<&str, Vec<&ClockifyTask>>::new();
	for task in tasks {
		if let Some(project_id) = task.project_id.as_deref() {
			by_project.entry(project_id).or_default().push(task);
		}
	}
	by_project
}

fn solidtime_tasks_by_project(tasks: &[SolidtimeTask]) -> BTreeMap<&str, Vec<&SolidtimeTask>> {
	let mut by_project = BTreeMap::<&str, Vec<&SolidtimeTask>>::new();
	for task in tasks {
		by_project.entry(&task.project_id).or_default().push(task);
	}
	by_project
}

fn group_by_name<'a, T>(items: impl Iterator<Item = &'a T>, name: impl Fn(&T) -> &str) -> BTreeMap<String, Vec<&'a T>>
where
	T: 'a,
{
	let mut by_name = BTreeMap::<String, Vec<&T>>::new();
	for item in items {
		by_name.entry(name(item).to_string()).or_default().push(item);
	}
	by_name
}

fn print_comparison(comparison: &Comparison) {
	print!("{}", render_comparison(comparison));
}

fn render_comparison(comparison: &Comparison) -> String {
	let mut output = String::new();
	output.push_str("Project comparison\n\n");
	output.push_str("Legend: = both, -> Clockify only, <- Solidtime only, ! manual review, A archived\n\n");

	for (client_name, projects) in &comparison.groups {
		output.push_str(&format!("Client: {}\n", client_name.as_deref().unwrap_or("(No client)")));
		output.push_str(&render_project_table(projects));
		output.push('\n');
	}

	let summary = &comparison.summary;
	if summary.projects_clockify_only == 0
		&& summary.projects_solidtime_only == 0
		&& summary.projects_manual_review == 0
		&& summary.tasks_clockify_only == 0
		&& summary.tasks_solidtime_only == 0
		&& summary.tasks_manual_review == 0
	{
		output.push_str("Clockify and Solidtime project structures match with no differences.\n\n");
	}

	output.push_str("Summary\n");
	output.push_str(&render_summary_table(summary));
	output.push('\n');
	output
}

#[derive(Debug, PartialEq, Eq)]
struct TableRow {
	item_type: String,
	clockify: String,
	relation: &'static str,
	solidtime: String,
}

fn render_project_table(projects: &[ProjectComparison]) -> String {
	let mut rows = Vec::new();
	for project in projects {
		rows.push(project_table_row(project));
		if let ProjectStatus::ManualReview(reason) = &project.status {
			rows.push(note_table_row(reason));
		}
		for task in &project.tasks {
			rows.push(task_table_row(task));
			if let TaskStatus::ManualReview(reason) = &task.status {
				rows.push(note_table_row(reason));
			}
		}
	}
	render_table(&["Type", "Clockify", "", "Solidtime"], rows)
}

fn project_table_row(project: &ProjectComparison) -> TableRow {
	let clockify_name = project.clockify_name.as_deref().unwrap_or("");
	let solidtime_name = project.solidtime_name.as_deref().unwrap_or("");
	match &project.status {
		ProjectStatus::Both => TableRow {
			item_type: "Project".to_string(),
			clockify: archived_name(clockify_name, project.clockify_archived),
			relation: "=",
			solidtime: archived_name(solidtime_name, project.solidtime_archived),
		},
		ProjectStatus::ClockifyOnly => TableRow {
			item_type: "Project".to_string(),
			clockify: archived_name(clockify_name, project.clockify_archived),
			relation: "->",
			solidtime: String::new(),
		},
		ProjectStatus::SolidtimeOnly => TableRow {
			item_type: "Project".to_string(),
			clockify: String::new(),
			relation: "<-",
			solidtime: archived_name(solidtime_name, project.solidtime_archived),
		},
		ProjectStatus::ManualReview(_) => TableRow {
			item_type: "Project".to_string(),
			clockify: clockify_name.to_string(),
			relation: "!",
			solidtime: solidtime_name.to_string(),
		},
	}
}

fn task_table_row(task: &TaskComparison) -> TableRow {
	let clockify_name = task.clockify_name.as_deref().unwrap_or("");
	let solidtime_name = task.solidtime_name.as_deref().unwrap_or("");
	match &task.status {
		TaskStatus::Both => TableRow {
			item_type: "Task".to_string(),
			clockify: clockify_name.to_string(),
			relation: "=",
			solidtime: solidtime_name.to_string(),
		},
		TaskStatus::ClockifyOnly => TableRow {
			item_type: "Task".to_string(),
			clockify: clockify_name.to_string(),
			relation: "->",
			solidtime: String::new(),
		},
		TaskStatus::SolidtimeOnly => TableRow {
			item_type: "Task".to_string(),
			clockify: String::new(),
			relation: "<-",
			solidtime: solidtime_name.to_string(),
		},
		TaskStatus::ManualReview(_) => TableRow {
			item_type: "Task".to_string(),
			clockify: clockify_name.to_string(),
			relation: "!",
			solidtime: solidtime_name.to_string(),
		},
	}
}

fn note_table_row(reason: &str) -> TableRow {
	TableRow {
		item_type: "Note".to_string(),
		clockify: reason.to_string(),
		relation: "!",
		solidtime: String::new(),
	}
}

fn archived_name(name: &str, archived: Option<bool>) -> String {
	if archived == Some(true) { format!("{name} [A]") } else { name.to_string() }
}

fn render_summary_table(summary: &Summary) -> String {
	render_table_with_extra(
		&["Type", "Matched", "Clockify only", "Solidtime only", "Manual review"],
		&[
			vec![
				"Projects".to_string(),
				summary.projects_matched.to_string(),
				summary.projects_clockify_only.to_string(),
				summary.projects_solidtime_only.to_string(),
				summary.projects_manual_review.to_string(),
			],
			vec![
				"Tasks".to_string(),
				summary.tasks_matched.to_string(),
				summary.tasks_clockify_only.to_string(),
				summary.tasks_solidtime_only.to_string(),
				summary.tasks_manual_review.to_string(),
			],
		],
	)
}

fn render_table(headers: &[&str; 4], rows: Vec<TableRow>) -> String {
	let cells = rows
		.into_iter()
		.map(|row| vec![row.item_type, row.clockify, row.relation.to_string(), row.solidtime])
		.collect::<Vec<_>>();
	render_table_with_extra(headers, &cells)
}

fn render_table_with_extra(headers: &[&str], rows: &[Vec<String>]) -> String {
	let mut widths = headers.iter().map(|header| header.len()).collect::<Vec<_>>();
	for row in rows {
		for (index, cell) in row.iter().enumerate() {
			widths[index] = widths[index].max(cell.len());
		}
	}

	let mut output = String::new();
	output.push_str(&border(&widths));
	output.push_str(&table_line(headers.iter().copied(), &widths));
	output.push_str(&border(&widths));
	for row in rows {
		output.push_str(&table_line(row.iter().map(String::as_str), &widths));
	}
	output.push_str(&border(&widths));
	output
}

fn border(widths: &[usize]) -> String {
	let mut line = String::new();
	line.push('+');
	for width in widths {
		line.push_str(&"-".repeat(width + 2));
		line.push('+');
	}
	line.push('\n');
	line
}

fn table_line<'a>(cells: impl Iterator<Item = &'a str>, widths: &[usize]) -> String {
	let mut line = String::new();
	line.push('|');
	for (cell, width) in cells.zip(widths.iter()) {
		line.push(' ');
		line.push_str(cell);
		line.push_str(&" ".repeat(width - cell.len() + 1));
		line.push('|');
	}
	line.push('\n');
	line
}

#[cfg(test)]
mod tests {
	use super::*;

	use std::fs;

	use anyhow::Result;
	use httpmock::{Method::GET, Method::POST, Method::PUT, MockServer};
	use serde_json::json;
	use tempfile::tempdir;

	fn clockify_client(id: &str, name: &str) -> ClockifyClient {
		ClockifyClient {
			id: id.to_string(),
			name: name.to_string(),
		}
	}

	fn solidtime_client(id: &str, name: &str) -> SolidtimeClient {
		SolidtimeClient {
			id: id.to_string(),
			name: name.to_string(),
		}
	}

	fn clockify_project(id: &str, name: &str, client_id: Option<&str>) -> ClockifyProject {
		ClockifyProject {
			id: id.to_string(),
			name: name.to_string(),
			client_id: client_id.map(str::to_string),
			color: None,
			billable: false,
			archived: false,
			estimate: None,
		}
	}

	fn solidtime_project(id: &str, name: &str, client_id: Option<&str>) -> SolidtimeProject {
		SolidtimeProject {
			id: id.to_string(),
			name: name.to_string(),
			client_id: client_id.map(str::to_string),
			is_archived: false,
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

	fn solidtime_task(id: &str, name: &str, project_id: &str) -> SolidtimeTask {
		SolidtimeTask {
			id: id.to_string(),
			name: name.to_string(),
			project_id: project_id.to_string(),
		}
	}

	fn archived_clockify_project(id: &str, name: &str, client_id: Option<&str>) -> ClockifyProject {
		let mut project = clockify_project(id, name, client_id);
		project.archived = true;
		project
	}

	fn archived_solidtime_project(id: &str, name: &str, client_id: Option<&str>) -> SolidtimeProject {
		let mut project = solidtime_project(id, name, client_id);
		project.is_archived = true;
		project
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

	#[test]
	fn matches_projects_by_name_under_same_client_and_compares_tasks() {
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[clockify_project("cp-1", "Shared", Some("cc-1")), clockify_project("cp-2", "Clockify Only", Some("cc-1"))],
			&[clockify_task("ct-1", "Design", "cp-1"), clockify_task("ct-2", "Build", "cp-1")],
			&[solidtime_client("sc-1", "Client A")],
			&[solidtime_project("sp-1", "Shared", Some("sc-1")), solidtime_project("sp-2", "Solidtime Only", Some("sc-1"))],
			&[solidtime_task("st-1", "Design", "sp-1"), solidtime_task("st-2", "Review", "sp-1")],
			&CompareMappings::default(),
		);

		assert_eq!(comparison.summary.projects_matched, 1);
		assert_eq!(comparison.summary.projects_clockify_only, 1);
		assert_eq!(comparison.summary.projects_solidtime_only, 1);
		assert_eq!(comparison.summary.tasks_matched, 1);
		assert_eq!(comparison.summary.tasks_clockify_only, 1);
		assert_eq!(comparison.summary.tasks_solidtime_only, 1);
	}

	#[test]
	fn renders_matched_project_with_mixed_task_relations() {
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[clockify_project("cp-1", "Shared", Some("cc-1"))],
			&[clockify_task("ct-1", "Design", "cp-1"), clockify_task("ct-2", "Build", "cp-1")],
			&[solidtime_client("sc-1", "Client A")],
			&[solidtime_project("sp-1", "Shared", Some("sc-1"))],
			&[solidtime_task("st-1", "Design", "sp-1"), solidtime_task("st-2", "Review", "sp-1")],
			&CompareMappings::default(),
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("Legend: = both, -> Clockify only, <- Solidtime only, ! manual review, A archived"));
		assert!(output.contains("Client: Client A"));
		assert!(output.contains("| Project | Shared   | =  | Shared    |"));
		assert!(output.contains("| Task    | Build    | -> |           |"));
		assert!(output.contains("| Task    | Design   | =  | Design    |"));
		assert!(output.contains("| Task    |          | <- | Review    |"));
		assert!(output.contains("| Projects | 1       | 0             | 0              | 0             |"));
		assert!(output.contains("| Tasks    | 1       | 1             | 1              | 0             |"));
	}

	#[test]
	fn renders_clockify_only_archived_project_with_tasks() {
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[archived_clockify_project("cp-1", "Legacy", Some("cc-1"))],
			&[clockify_task("ct-1", "Fallback Host", "cp-1")],
			&[solidtime_client("sc-1", "Client A")],
			&[],
			&[],
			&CompareMappings::default(),
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("| Project | Legacy [A]    | -> |           |"));
		assert!(output.contains("| Task    | Fallback Host | -> |           |"));
	}

	#[test]
	fn renders_solidtime_only_project() {
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[],
			&[],
			&[solidtime_client("sc-1", "Client A")],
			&[archived_solidtime_project("sp-1", "Target", Some("sc-1"))],
			&[solidtime_task("st-1", "Services", "sp-1")],
			&CompareMappings::default(),
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("| Project |          | <- | Target [A] |"));
		assert!(output.contains("| Task    |          | <- | Services   |"));
	}

	#[test]
	fn flags_ambiguous_project_names_without_guessing() {
		let comparison = build_comparison(
			&[],
			&[clockify_project("cp-1", "Ambiguous", None), clockify_project("cp-2", "Ambiguous", None)],
			&[clockify_task("ct-1", "Task", "cp-1")],
			&[],
			&[solidtime_project("sp-1", "Ambiguous", None)],
			&[solidtime_task("st-1", "Task", "sp-1")],
			&CompareMappings::default(),
		);

		let projects = comparison.groups.get(&None).expect("no-client group");

		assert_eq!(comparison.summary.projects_manual_review, 1);
		assert_eq!(comparison.summary.tasks_matched, 0);
		assert!(matches!(projects[0].status, ProjectStatus::ManualReview(_)));
		assert!(projects[0].tasks.is_empty());
	}

	#[test]
	fn renders_manual_review_project_with_reason_row() {
		let comparison = build_comparison(
			&[],
			&[clockify_project("cp-1", "Ambiguous", None), clockify_project("cp-2", "Ambiguous", None)],
			&[],
			&[],
			&[solidtime_project("sp-1", "Ambiguous", None)],
			&[],
			&CompareMappings::default(),
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("| Project | Ambiguous"));
		assert!(output.contains("| ! | Ambiguous |"));
		assert!(output.contains("| Note    | ambiguous project name: 2 Clockify match(es), 1 Solidtime match(es) | ! |"));
	}

	#[test]
	fn renders_manual_review_task_with_reason_row() {
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[clockify_project("cp-1", "Shared", Some("cc-1"))],
			&[clockify_task("ct-1", "Ambiguous", "cp-1"), clockify_task("ct-2", "Ambiguous", "cp-1")],
			&[solidtime_client("sc-1", "Client A")],
			&[solidtime_project("sp-1", "Shared", Some("sc-1"))],
			&[solidtime_task("st-1", "Ambiguous", "sp-1")],
			&CompareMappings::default(),
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("| Task    | Ambiguous"));
		assert!(output.contains("| ! | Ambiguous |"));
		assert!(output.contains("| Note    | ambiguous task name: 2 Clockify match(es), 1 Solidtime match(es) | ! |"));
		assert!(output.contains("| Tasks    | 0       | 0             | 0              | 1             |"));
	}

	#[test]
	fn renders_no_differences_message_when_structures_match() {
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[clockify_project("cp-1", "Shared", Some("cc-1"))],
			&[clockify_task("ct-1", "Design", "cp-1")],
			&[solidtime_client("sc-1", "Client A")],
			&[solidtime_project("sp-1", "Shared", Some("sc-1"))],
			&[solidtime_task("st-1", "Design", "sp-1")],
			&CompareMappings::default(),
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("Clockify and Solidtime project structures match with no differences."));
		assert!(output.contains("| Projects | 1       | 0             | 0              | 0             |"));
		assert!(output.contains("| Tasks    | 1       | 0             | 0              | 0             |"));
	}

	#[test]
	fn mapping_pairs_differently_named_projects_and_tasks() {
		let mut mappings = CompareMappings::default();
		mappings.projects.insert("cp-1".to_string(), "sp-1".to_string());
		mappings.tasks.insert("ct-1".to_string(), "st-1".to_string());
		let comparison = build_comparison(
			&[clockify_client("cc-1", "Client A")],
			&[clockify_project("cp-1", "Clockify Name", Some("cc-1"))],
			&[clockify_task("ct-1", "Clockify Task", "cp-1")],
			&[solidtime_client("sc-1", "Client A")],
			&[solidtime_project("sp-1", "Solidtime Name", Some("sc-1"))],
			&[solidtime_task("st-1", "Solidtime Task", "sp-1")],
			&mappings,
		);

		let output = render_comparison(&comparison);

		assert!(output.contains("Clockify Name"));
		assert!(output.contains("Solidtime Name"));
		assert!(output.contains("Clockify Task"));
		assert!(output.contains("Solidtime Task"));
		assert_eq!(comparison.summary.projects_matched, 1);
		assert_eq!(comparison.summary.tasks_matched, 1);
	}

	#[test]
	fn mapping_override_bypasses_name_ambiguity() {
		let mut mappings = CompareMappings::default();
		mappings.projects.insert("cp-1".to_string(), "sp-1".to_string());
		let comparison = build_comparison(
			&[],
			&[clockify_project("cp-1", "Ambiguous", None), clockify_project("cp-2", "Ambiguous", None)],
			&[],
			&[],
			&[solidtime_project("sp-1", "Renamed", None)],
			&[],
			&mappings,
		);

		assert_eq!(comparison.summary.projects_matched, 1);
		assert_eq!(comparison.summary.projects_clockify_only, 1);
		assert_eq!(comparison.summary.projects_manual_review, 0);
	}

	#[test]
	fn default_task_mapping_row_does_not_affect_task_pairing() {
		let rows = vec![mapping_row("Clockify Project", "", "Solidtime Project", "Missing Default Task")];
		let mappings = resolve_compare_mappings(
			rows,
			&[clockify_project("cp-1", "Clockify Project", None)],
			&[clockify_task("ct-1", "Build", "cp-1")],
			&[solidtime_project("sp-1", "Solidtime Project", None)],
			&[],
		)
		.unwrap();

		assert_eq!(mappings.projects.get("cp-1").map(String::as_str), Some("sp-1"));
		assert!(mappings.tasks.is_empty());
	}

	#[test]
	fn resolves_compare_mappings_by_name_and_id_and_rejects_conflicts() {
		let rows = vec![mapping_row("Clockify Project", "Clockify Task", "Solidtime Project", "Solidtime Task")];
		let mappings = resolve_compare_mappings(
			rows,
			&[clockify_project("cp-1", "Clockify Project", None)],
			&[clockify_task("ct-1", "Clockify Task", "cp-1")],
			&[solidtime_project("sp-1", "Solidtime Project", None)],
			&[solidtime_task("st-1", "Solidtime Task", "sp-1")],
		)
		.unwrap();
		assert_eq!(mappings.projects.get("cp-1").map(String::as_str), Some("sp-1"));
		assert_eq!(mappings.tasks.get("ct-1").map(String::as_str), Some("st-1"));

		let mut id_row = mapping_row("", "", "", "");
		id_row.clockify_project_id = "cp-1".to_string();
		id_row.clockify_task_id = "ct-1".to_string();
		id_row.solidtime_project_id = "sp-1".to_string();
		id_row.solidtime_task_id = "st-1".to_string();
		assert!(
			resolve_compare_mappings(
				vec![id_row],
				&[clockify_project("cp-1", "Clockify Project", None)],
				&[clockify_task("ct-1", "Clockify Task", "cp-1")],
				&[solidtime_project("sp-1", "Solidtime Project", None)],
				&[solidtime_task("st-1", "Solidtime Task", "sp-1")],
			)
			.is_ok()
		);

		let err = resolve_compare_mappings(
			vec![
				mapping_row("Clockify Project", "", "Solidtime Project", ""),
				mapping_row("Clockify Project", "", "Other Solidtime Project", ""),
			],
			&[clockify_project("cp-1", "Clockify Project", None)],
			&[],
			&[solidtime_project("sp-1", "Solidtime Project", None), solidtime_project("sp-2", "Other Solidtime Project", None)],
			&[],
		)
		.unwrap_err();

		assert!(err.to_string().contains("mapping conflict"));
	}

	#[test]
	fn run_compares_project_setup_with_read_only_requests() -> Result<()> {
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
		let clockify_clients = server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/clients");
			then.status(200).header("X-Last-Page", "true").json_body(json!([{
					"id": "clockify-client-id",
					"name": "Client A"
			}]));
		});
		let clockify_projects = server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/projects");
			then.status(200).header("X-Last-Page", "true").json_body(json!([{
					"id": "clockify-project-id",
					"name": "Shared Project",
					"clientId": "clockify-client-id",
					"archived": false
			}]));
		});
		let clockify_tasks = server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/projects/clockify-project-id/tasks");
			then.status(200).header("X-Last-Page", "true").json_body(json!([{
					"id": "clockify-task-id",
					"name": "Shared Task",
					"projectId": "clockify-project-id"
			}]));
		});
		let solidtime_clients = server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/organization-id/clients");
			then.status(200).json_body(json!({
					"data": [{
							"id": "solidtime-client-id",
							"name": "Client A",
							"is_archived": false
					}]
			}));
		});
		let solidtime_projects = server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/organization-id/projects");
			then.status(200).json_body(json!({
					"data": [{
							"id": "solidtime-project-id",
							"name": "Shared Project",
							"client_id": "solidtime-client-id",
							"is_archived": false
					}]
			}));
		});
		let solidtime_tasks = server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/organization-id/tasks");
			then.status(200).json_body(json!({
					"data": [{
							"id": "solidtime-task-id",
							"name": "Shared Task",
							"project_id": "solidtime-project-id"
					}]
			}));
		});
		let solidtime_project_write = server.mock(|when, then| {
			when.method(POST).path("/solidtime/v1/organizations/organization-id/projects");
			then.status(500);
		});
		let solidtime_task_write = server.mock(|when, then| {
			when.method(POST).path("/solidtime/v1/organizations/organization-id/tasks");
			then.status(500);
		});
		let solidtime_archive_write = server.mock(|when, then| {
			when.method(PUT).path("/solidtime/v1/organizations/organization-id/projects/solidtime-project-id");
			then.status(500);
		});
		let dir = tempdir()?;
		let config_path = write_config(dir.path(), &server)?;

		run(Options {
			config_path: Some(config_path),
			mapping_path: None,
			ignore_archived: false,
		})?;

		clockify_user.assert();
		clockify_workspaces.assert();
		solidtime_memberships.assert();
		assert_eq!(clockify_clients.hits(), 2);
		assert_eq!(clockify_projects.hits(), 2);
		clockify_tasks.assert();
		solidtime_clients.assert();
		solidtime_projects.assert();
		solidtime_tasks.assert();
		assert_eq!(solidtime_project_write.hits(), 0);
		assert_eq!(solidtime_task_write.hits(), 0);
		assert_eq!(solidtime_archive_write.hits(), 0);
		assert!(!dir.path().join("migration-state.json").exists());
		Ok(())
	}

	#[test]
	fn run_ignore_archived_skips_archived_project_task_fetch() -> Result<()> {
		let server = MockServer::start();
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
		server.mock(|when, then| {
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
		server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/clients");
			then.status(200).header("X-Last-Page", "true").json_body(json!([]));
		});
		server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/projects");
			then.status(200).header("X-Last-Page", "true").json_body(json!([
					{
							"id": "active-project-id",
							"name": "Active Project",
							"archived": false
					},
					{
							"id": "archived-project-id",
							"name": "Archived Project",
							"archived": true
					}
			]));
		});
		let active_tasks = server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/projects/active-project-id/tasks");
			then.status(200).header("X-Last-Page", "true").json_body(json!([]));
		});
		let archived_tasks = server.mock(|when, then| {
			when.method(GET).path("/clockify/workspaces/workspace-id/projects/archived-project-id/tasks");
			then.status(500);
		});
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/organization-id/clients");
			then.status(200).json_body(json!({
					"data": []
			}));
		});
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/organization-id/projects");
			then.status(200).json_body(json!({
					"data": []
			}));
		});
		server.mock(|when, then| {
			when.method(GET).path("/solidtime/v1/organizations/organization-id/tasks");
			then.status(200).json_body(json!({
					"data": []
			}));
		});
		let dir = tempdir()?;
		let config_path = write_config(dir.path(), &server)?;

		run(Options {
			config_path: Some(config_path),
			mapping_path: None,
			ignore_archived: true,
		})?;

		active_tasks.assert();
		assert_eq!(archived_tasks.hits(), 0);
		Ok(())
	}
}
