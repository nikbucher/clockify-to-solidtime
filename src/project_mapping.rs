use std::{collections::BTreeMap, fs::File, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::models::{ClockifyProject, ClockifyTask, SolidtimeProject, SolidtimeTask};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ProjectMappingRow {
	#[serde(rename = "Clockify_Project")]
	pub(crate) clockify_project: String,
	#[serde(rename = "Clockify_Task", default)]
	pub(crate) clockify_task: String,
	#[serde(rename = "Solidtime_Project")]
	pub(crate) solidtime_project: String,
	#[serde(rename = "Solidtime_Task", default)]
	pub(crate) solidtime_task: String,
	#[serde(rename = "Clockify_Project_ID", default)]
	pub(crate) clockify_project_id: String,
	#[serde(rename = "Clockify_Task_ID", default)]
	pub(crate) clockify_task_id: String,
	#[serde(rename = "Solidtime_Project_ID", default)]
	pub(crate) solidtime_project_id: String,
	#[serde(rename = "Solidtime_Task_ID", default)]
	pub(crate) solidtime_task_id: String,
}

impl ProjectMappingRow {
	pub(crate) fn has_solidtime_task(&self) -> bool {
		!self.solidtime_task.trim().is_empty() || !self.solidtime_task_id.trim().is_empty()
	}

	pub(crate) fn has_clockify_task(&self) -> bool {
		!self.clockify_task.trim().is_empty() || !self.clockify_task_id.trim().is_empty()
	}
}

pub(crate) fn read_project_mapping_rows(path: &Path) -> Result<Vec<ProjectMappingRow>> {
	let file = File::open(path).with_context(|| format!("failed to open mapping file {}", path.display()))?;
	csv::Reader::from_reader(file)
		.deserialize()
		.collect::<std::result::Result<Vec<_>, _>>()
		.with_context(|| format!("failed to parse mapping file {}", path.display()))
}

pub(crate) fn resolve_clockify_project<'a>(row: &ProjectMappingRow, projects: &'a [ClockifyProject]) -> Result<&'a ClockifyProject> {
	if !row.clockify_project_id.trim().is_empty() {
		return projects
			.iter()
			.find(|project| project.id == row.clockify_project_id)
			.with_context(|| format!("Clockify project ID {:?} from mapping was not found", row.clockify_project_id));
	}
	single_match(projects.iter().filter(|project| project.name == row.clockify_project), "Clockify project", &row.clockify_project)?
		.with_context(|| format!("Clockify project {:?} from mapping was not found", row.clockify_project))
}

pub(crate) fn resolve_clockify_task<'a>(row: &ProjectMappingRow, project_id: &str, tasks: &'a [ClockifyTask]) -> Result<&'a ClockifyTask> {
	if !row.clockify_task_id.trim().is_empty() {
		let task = tasks
			.iter()
			.find(|task| task.id == row.clockify_task_id)
			.with_context(|| format!("Clockify task ID {:?} from mapping was not found", row.clockify_task_id))?;
		if task_project_id(task) != Some(project_id) {
			bail!("Clockify task ID {:?} from mapping does not belong to Clockify project {project_id}", row.clockify_task_id);
		}
		return Ok(task);
	}
	single_match(
		tasks.iter().filter(|task| task.name == row.clockify_task && task_project_id(task) == Some(project_id)),
		"Clockify task",
		&row.clockify_task,
	)?
	.with_context(|| format!("Clockify task {:?} from mapping was not found under Clockify project {project_id}", row.clockify_task))
}

pub(crate) fn resolve_solidtime_project<'a>(row: &ProjectMappingRow, projects: &'a [SolidtimeProject]) -> Result<&'a SolidtimeProject> {
	if !row.solidtime_project_id.trim().is_empty() {
		return projects
			.iter()
			.find(|project| project.id == row.solidtime_project_id)
			.with_context(|| format!("Solidtime project ID {:?} from mapping was not found", row.solidtime_project_id));
	}
	single_match(projects.iter().filter(|project| project.name == row.solidtime_project), "Solidtime project", &row.solidtime_project)?
		.with_context(|| format!("Solidtime project {:?} from mapping was not found", row.solidtime_project))
}

pub(crate) fn try_resolve_solidtime_task<'a>(row: &ProjectMappingRow, project: &SolidtimeProject, tasks: &'a [SolidtimeTask]) -> Result<Option<&'a SolidtimeTask>> {
	if !row.solidtime_task_id.trim().is_empty() {
		let task = tasks
			.iter()
			.find(|task| task.id == row.solidtime_task_id)
			.with_context(|| format!("Solidtime task ID {:?} from mapping was not found", row.solidtime_task_id))?;
		if task.project_id != project.id {
			bail!("Solidtime task ID {:?} from mapping does not belong to Solidtime project {:?}", row.solidtime_task_id, project.name);
		}
		return Ok(Some(task));
	}

	single_match(
		tasks.iter().filter(|task| task.name == row.solidtime_task && task.project_id == project.id),
		"Solidtime task",
		&row.solidtime_task,
	)
}

pub(crate) fn resolve_solidtime_task<'a>(row: &ProjectMappingRow, project: &SolidtimeProject, tasks: &'a [SolidtimeTask]) -> Result<&'a SolidtimeTask> {
	try_resolve_solidtime_task(row, project, tasks)?.with_context(|| format!("Solidtime task {:?} from mapping was not found under Solidtime project {:?}", row.solidtime_task, project.name))
}

pub(crate) fn insert_mapping(map: &mut BTreeMap<String, String>, state_value: Option<&str>, clockify_id: &str, solidtime_id: &str, kind: &str) -> Result<()> {
	if let Some(existing) = state_value
		&& existing != solidtime_id
	{
		bail!("mapping conflict for Clockify {kind} {clockify_id}: migration-state.json points to {existing}, but CSV points to {solidtime_id}");
	}
	if let Some(existing) = map.get(clockify_id)
		&& existing != solidtime_id
	{
		bail!("mapping conflict for Clockify {kind} {clockify_id}: CSV points to both {existing} and {solidtime_id}");
	}
	map.insert(clockify_id.to_string(), solidtime_id.to_string());
	Ok(())
}

pub(crate) fn single_match<'a, T>(matches: impl Iterator<Item = &'a T>, kind: &str, name: &str) -> Result<Option<&'a T>> {
	let mut matches = matches.take(2).collect::<Vec<_>>();
	if matches.len() > 1 {
		bail!("multiple Solidtime {kind} records match {name:?}; refusing to guess");
	}
	Ok(matches.pop())
}

pub(crate) fn task_project_id(task: &ClockifyTask) -> Option<&str> {
	task.project_id.as_deref()
}

#[cfg(test)]
mod tests {
	use super::*;

	use std::fs;

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

	fn clockify_task(id: &str, name: &str, project_id: &str) -> ClockifyTask {
		ClockifyTask {
			id: id.to_string(),
			name: name.to_string(),
			project_id: Some(project_id.to_string()),
			estimate: None,
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
	fn rejects_unresolved_and_ambiguous_mapping_references() {
		let row = mapping_row("Project", "Task", "Target", "Target Task");

		assert!(resolve_clockify_project(&row, &[]).unwrap_err().to_string().contains("Clockify project"));
		assert!(resolve_clockify_task(&row, "cp-1", &[]).unwrap_err().to_string().contains("Clockify task"));
		assert!(resolve_solidtime_project(&row, &[]).unwrap_err().to_string().contains("Solidtime project"));
		assert!(resolve_solidtime_task(&row, &solid_project("sp-1", "Target"), &[]).unwrap_err().to_string().contains("Solidtime task"));

		assert!(resolve_clockify_project(&row, &[clockify_project("cp-1", "Project"), clockify_project("cp-2", "Project")]).is_err());
		assert!(resolve_clockify_task(&row, "cp-1", &[clockify_task("ct-1", "Task", "cp-1"), clockify_task("ct-2", "Task", "cp-1")]).is_err());
		assert!(resolve_solidtime_project(&row, &[solid_project("sp-1", "Target"), solid_project("sp-2", "Target")]).is_err());
		assert!(
			resolve_solidtime_task(
				&row,
				&solid_project("sp-1", "Target"),
				&[solid_task("st-1", "Target Task", "sp-1"), solid_task("st-2", "Target Task", "sp-1")]
			)
			.is_err()
		);
	}

	#[test]
	fn detects_csv_internal_mapping_conflicts() {
		let mut map = BTreeMap::new();

		insert_mapping(&mut map, None, "clockify", "solidtime-1", "project").unwrap();
		let err = insert_mapping(&mut map, None, "clockify", "solidtime-2", "project").unwrap_err();

		assert!(err.to_string().contains("CSV points to both solidtime-1 and solidtime-2"));
	}
}
