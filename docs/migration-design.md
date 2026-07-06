# Migration Design

## Purpose

`clockify-to-solidtime` migrates time tracking data from a Clockify workspace to a
Solidtime organization through the two services' APIs. It is designed for repeatable
single-user migrations where dry-runs and reruns should be safe.

## Scope

The migration supports:

- Clients
- Projects
- Tasks
- Tags
- Time entries

The migration does not migrate billable rates. It does migrate the billable flag on
projects and time entries.

## Key Decisions

- The tool is a Rust command-line application, not a script or CSV importer wrapper.
- The migration reads from Clockify and writes directly to Solidtime through their APIs.
- `migrate --dry-run` performs reads and reconciliation without remote writes or state-file writes.
- The local state file records Clockify IDs mapped to Solidtime IDs so interrupted runs can resume.
- Existing Solidtime records are adopted when they match the expected scope instead of being recreated.
- Time entries use remote fingerprint matching in addition to local state so reruns do not depend only on `migration-state.json`.
- Archived Clockify projects are included by default. `--ignore-archived` skips archived projects, their tasks, and their time entries.

## Data Mapping

| Clockify                 | Solidtime                | Notes                                                   |
|--------------------------|--------------------------|---------------------------------------------------------|
| Client name              | Client name              | Matched by name within the organization.                |
| Project name             | Project name             | Matched by name and Solidtime client.                   |
| Project color            | Project color            | Invalid or missing colors use a default.                |
| Project billable flag    | Project billable flag    | Billable rates are intentionally not migrated.          |
| Project estimate         | Project estimated time   | ISO-8601 duration is converted to seconds when present. |
| Task name                | Task name                | Matched by name within the mapped project.              |
| Task estimate            | Task estimated time      | ISO-8601 duration is converted to seconds when present. |
| Tag name                 | Tag name                 | Matched by name within the organization.                |
| Time entry start/end     | Time entry start/end     | Migrated in monthly windows.                            |
| Time entry description   | Time entry description   | Trimmed for fingerprint comparison.                     |
| Time entry billable flag | Time entry billable flag | Included in duplicate detection.                        |
| Time entry tags          | Time entry tags          | Tags are mapped to Solidtime tag IDs.                   |

## Idempotency

The migration uses three layers to avoid duplicates:

1. Local state in `migration-state.json` maps source IDs to target IDs.
2. Clients, projects, tasks, and tags are looked up in Solidtime before creation and can be adopted after create conflicts.
3. Time entries are compared against existing Solidtime entries in the same time window using a deterministic fingerprint made from member, project, task, start, end, billable flag, description, and sorted tags.

State writes are atomic and happen after each successful mapping update. Dry-runs do not write
state.

## Operational Notes

Run `validate` before migration to catch missing credentials or ambiguous Solidtime
organizations. Run `migrate --dry-run` before a real migration to review planned changes.

Configuration can come from `--config`, real environment variables, `.env`, and built-in
defaults for base URLs. See the README for the supported keys.

Use `--state` to choose a state file. Reusing the same state file is the normal way to resume
or rerun a migration. Deleting the state file should still avoid time-entry duplicates when the
matching Solidtime entries are visible to the API.

## Verification

Recommended checks:

- `cargo test`
- `cargo run -- validate`
- `cargo run -- migrate --dry-run ...`
- A real run against a test Solidtime organization
- A second run with the same state file to verify no duplicate creates
- A rerun without the state file to spot-check time-entry fingerprint reconciliation
