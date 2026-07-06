use std::{
	collections::BTreeMap,
	fs,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MigrationState {
	#[serde(default)]
	pub clients: BTreeMap<String, String>,
	#[serde(default)]
	pub projects: BTreeMap<String, String>,
	#[serde(default)]
	pub tasks: BTreeMap<String, String>,
	#[serde(default)]
	pub tags: BTreeMap<String, String>,
	#[serde(default)]
	pub time_entries: BTreeMap<String, String>,
	pub last_cutoff: Option<String>,
	#[serde(skip)]
	path: Option<PathBuf>,
	#[serde(skip)]
	dirty: bool,
	#[serde(skip)]
	dry_run: bool,
}

impl MigrationState {
	pub fn load(path: &Path, dry_run: bool) -> Result<Self> {
		let mut state = if path.exists() {
			let contents = fs::read_to_string(path).with_context(|| format!("failed to read state file {}", path.display()))?;
			serde_json::from_str::<Self>(&contents).with_context(|| format!("failed to parse state file {}", path.display()))?
		} else {
			Self::default()
		};
		state.path = Some(path.to_path_buf());
		state.dry_run = dry_run;
		Ok(state)
	}

	pub fn persist(&mut self) -> Result<()> {
		if self.dry_run || !self.dirty {
			return Ok(());
		}
		let path = self.path.as_ref().context("state path is not configured")?;
		if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
			fs::create_dir_all(parent).with_context(|| format!("failed to create state directory {}", parent.display()))?;
		}
		let tmp = path.with_extension("json.tmp");
		let data = serde_json::to_vec_pretty(self).context("failed to serialize state")?;
		fs::write(&tmp, data).with_context(|| format!("failed to write temporary state file {}", tmp.display()))?;
		fs::rename(&tmp, path).with_context(|| format!("failed to replace state file {}", path.display()))?;
		self.dirty = false;
		Ok(())
	}

	pub fn put_client(&mut self, clockify_id: &str, solidtime_id: &str) -> Result<()> {
		put(&mut self.clients, &mut self.dirty, clockify_id, solidtime_id);
		self.persist()
	}

	pub fn put_project(&mut self, clockify_id: &str, solidtime_id: &str) -> Result<()> {
		put(&mut self.projects, &mut self.dirty, clockify_id, solidtime_id);
		self.persist()
	}

	pub fn put_task(&mut self, clockify_id: &str, solidtime_id: &str) -> Result<()> {
		put(&mut self.tasks, &mut self.dirty, clockify_id, solidtime_id);
		self.persist()
	}

	pub fn put_tag(&mut self, clockify_id: &str, solidtime_id: &str) -> Result<()> {
		put(&mut self.tags, &mut self.dirty, clockify_id, solidtime_id);
		self.persist()
	}

	pub fn put_time_entry(&mut self, clockify_id: &str, solidtime_id: &str) -> Result<()> {
		put(&mut self.time_entries, &mut self.dirty, clockify_id, solidtime_id);
		self.persist()
	}

	pub fn set_cutoff(&mut self, cutoff: String) -> Result<()> {
		if self.last_cutoff.as_deref() != Some(cutoff.as_str()) {
			self.last_cutoff = Some(cutoff);
			self.dirty = true;
		}
		self.persist()
	}
}

fn put(map: &mut BTreeMap<String, String>, dirty: &mut bool, clockify_id: &str, solidtime_id: &str) {
	if map.get(clockify_id).map(String::as_str) != Some(solidtime_id) {
		map.insert(clockify_id.to_string(), solidtime_id.to_string());
		*dirty = true;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn persists_atomically_readable_state() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("state.json");
		let mut state = MigrationState::load(&path, false).unwrap();
		state.put_client("clockify", "solidtime").unwrap();

		let reloaded = MigrationState::load(&path, false).unwrap();
		assert_eq!(reloaded.clients.get("clockify").map(String::as_str), Some("solidtime"));
		assert!(!path.with_extension("json.tmp").exists());
	}

	#[test]
	fn dry_run_does_not_write_state() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("state.json");
		let mut state = MigrationState::load(&path, true).unwrap();
		state.put_client("clockify", "solidtime").unwrap();

		assert!(!path.exists());
	}
}
