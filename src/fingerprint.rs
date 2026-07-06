use crate::models::SolidtimeTimeEntryCreate;

pub fn time_entry_fingerprint(entry: &SolidtimeTimeEntryCreate<'_>) -> String {
	let mut tags = entry.tags.clone();
	tags.sort_unstable();
	[
		entry.member_id.to_string(),
		entry.project_id.unwrap_or("").to_string(),
		entry.task_id.unwrap_or("").to_string(),
		entry.start.clone(),
		entry.end.clone().unwrap_or_default(),
		entry.billable.to_string(),
		entry.description.unwrap_or("").trim().to_string(),
		tags.join(","),
	]
	.join("|")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fingerprint_is_stable_for_tag_order() {
		let left = SolidtimeTimeEntryCreate {
			member_id: "member",
			project_id: Some("project"),
			task_id: Some("task"),
			start: "2024-01-01T10:00:00Z".to_string(),
			end: Some("2024-01-01T11:00:00Z".to_string()),
			billable: true,
			description: Some(" work "),
			tags: vec!["b", "a"],
		};
		let right = SolidtimeTimeEntryCreate { tags: vec!["a", "b"], ..left };

		assert_eq!(time_entry_fingerprint(&right), "member|project|task|2024-01-01T10:00:00Z|2024-01-01T11:00:00Z|true|work|a,b");
	}
}
