# Use Case: Migrate Time Tracking Data

## Overview

**Use Case ID:** UC-003  
**Use Case Name:** Migrate Time Tracking Data  
**Primary Actor:** User  
**Goal:** Transfer supported Clockify clients, projects, tasks, tags, and time entries to Solidtime without recreating data that already exists or has already been migrated.  
**Status:** Approved

## Preconditions

- User can run the command-line application.
- Clockify and Solidtime configuration is available through the supported configuration sources.
- Clockify and Solidtime are expected to be available for the required reads and Solidtime changes.
- User has decided the migration time range to process, or accepts the system defaults.

## Configuration Reference

The command form is:

```sh
clockify-to-solidtime migrate [--dry-run] [--config <path>] [--state <path>] [--mapping <path>] [--no-create-structure] [--ignore-archived] [--from <timestamp>] [--to <timestamp>]
```

Configuration values and their sources are the same as UC-001 (Validate Configuration), which this use case includes. The migration resolves and validates configuration through UC-001 before reading or changing migration data.

`--dry-run` previews the migration by reading and reconciling data without changing Solidtime or the local migration state. `--state <path>` selects the local migration state file; when omitted, the default is `migration-state.json`. `--mapping <path>` optionally supplies a CSV file pairing Clockify project/task names or IDs with existing Solidtime project/task names or IDs, using the same mapping file format as UC-002 (Compare Project Setup). `--no-create-structure` requires missing Solidtime clients, projects, tasks, and tags to already exist or be supplied by mapping instead of being created during a real migration. `--ignore-archived` excludes archived Clockify projects, their tasks, and their time entries from migration. `--from <timestamp>` is the inclusive migration start time. `--to <timestamp>` is the exclusive migration end time.

## Migrated Data

| Clockify data             | Solidtime data            | Migration behavior                                         |
|---------------------------|---------------------------|------------------------------------------------------------|
| Client name               | Client name               | Matched by name before a new client is created.            |
| Project name              | Project name              | Matched by name under the mapped client before creation.   |
| Project color             | Project color             | Migrated when valid; otherwise a default color is used.    |
| Project billable flag     | Project billable flag     | Migrated.                                                  |
| Project estimate          | Project estimated time    | Migrated when present and supported.                       |
| Task name                 | Task name                 | Matched by name within the mapped project before creation. |
| Task estimate             | Task estimated time       | Migrated when present and supported.                       |
| Tag name                  | Tag name                  | Matched by name before a new tag is created.               |
| Time entry start and end  | Time entry start and end  | Migrated for entries in the selected time range.           |
| Time entry description    | Time entry description    | Migrated.                                                  |
| Time entry billable flag  | Time entry billable flag  | Migrated.                                                  |
| Time entry tag assignment | Time entry tag assignment | Migrated after each tag is matched or created.             |

Billable rates are not migrated.

## Main Success Scenario

1. User runs the `migrate` command, optionally providing dry-run mode, a configuration file, a state file, a mapping file, structure creation behavior, archived-project behavior, and/or a migration time range.
2. System validates the Clockify and Solidtime configuration and confirms access to the source workspace and target organization, as specified by UC-001 (Validate Configuration).
3. System determines the migration time range from the provided values or defaults.
4. System opens the selected local migration state, or starts with an empty migration state when no state file exists.
5. System reads and parses the mapping file, if provided, without yet applying its entries.
6. System retrieves the Clockify clients, projects, tasks, tags, and time entries needed for the selected workspace and time range, excluding archived Clockify projects, their tasks, and their time entries when `--ignore-archived` was given.
7. System retrieves the Solidtime clients, projects, tasks, tags, and time entries needed to reconcile the selected target organization and time range.
8. System resolves each parsed mapping entry against the retrieved Clockify and Solidtime projects and tasks.
9. System matches each Clockify client to an existing Solidtime client by name, reuses a state-file mapping when present, or creates a Solidtime client when needed and allowed.
10. System matches each Clockify project to an existing Solidtime project by name under the mapped client, reuses a state-file or mapping-file pairing when present, or creates a Solidtime project when needed and allowed.
11. System matches each Clockify task to an existing Solidtime task by name under the mapped project, reuses a state-file or mapping-file pairing when present, or creates a Solidtime task when needed and allowed.
12. System matches each Clockify tag to an existing Solidtime tag by name, reuses a state-file mapping when present, or creates a Solidtime tag when needed and allowed.
13. For each Clockify time entry in the selected time range, System determines the corresponding Solidtime project, task, and tags.
14. System skips each time entry that is already represented in the local migration state or already exists in Solidtime with the same migration-relevant attributes.
15. System creates each remaining Solidtime time entry when this is a real migration run, or counts it as planned when this is a dry-run.
16. System marks migrated Solidtime projects as archived when their source Clockify projects are archived and archived projects are included.
17. System records successful mappings and the processed cutoff in the local migration state when this is a real migration run.
18. System displays a migration summary showing created and reused clients, projects, tasks, tags, and time entries, including archived project results.
19. User reviews the summary confirming the supported Clockify data was migrated or previewed without duplicate migrated records, achieving the goal.

## Alternative Flows

### A1: Configuration Validation Fails

**Trigger:** Configuration validation reports a problem with configuration or account access (step 2)
**Flow:**

1. System reports the specific setup step or service that failed, as specified by UC-001.
2. System does not read migration data, change Solidtime, or change local migration state.
3. Use case ends.

### A2: Migration Time Range Is Invalid

**Trigger:** The selected migration start time is not before the selected migration end time (step 3)
**Flow:**

1. System reports that the migration start time must be before the migration end time.
2. System does not read migration data, change Solidtime, or change local migration state.
3. Use case ends.

### A3: Local Migration State Cannot Be Loaded

**Trigger:** The selected state file cannot be read or is not valid migration state (step 4)
**Flow:**

1. System reports the specific problem with the selected state file.
2. System does not retrieve migration data or change Solidtime.
3. Use case ends.

### A4: Mapping File Cannot Be Read

**Trigger:** The file given with `--mapping` does not exist, cannot be read, or is not a valid mapping file (step 5)
**Flow:**

1. System reports the specific problem with the mapping file.
2. System does not retrieve migration data or change Solidtime.
3. Use case ends.

### A5: Remote Data Cannot Be Retrieved

**Trigger:** Clockify or Solidtime cannot be reached, or responds with an error, while data is being retrieved (step 6 or step 7)
**Flow:**

1. System reports which service failed and that the migration could not be completed.
2. System does not create additional Solidtime records.
3. Use case ends.

### A6: Mapping Entry Is Missing, Ambiguous, or Conflicting

**Trigger:** A mapping entry cannot be resolved to exactly one usable project or task, or conflicts with another mapping or the local migration state (step 8)
**Flow:**

1. System reports which mapping entry is missing, ambiguous, or conflicting, and why.
2. System does not apply the unresolved or conflicting mapping.
3. Use case ends.

### A7: Structure Creation Is Disabled and Required Structure Is Missing

**Trigger:** A required Solidtime client, project, task, or tag is missing and `--no-create-structure` was given during a real migration run (step 9, step 10, step 11, or step 12)
**Flow:**

1. System reports which required structure item is missing.
2. System explains that the user must create it in Solidtime, supply an applicable mapping, or rerun with structure creation enabled.
3. Use case ends.

### A8: Time Entry Cannot Be Associated With Required Structure

**Trigger:** A Clockify time entry refers to a project, task, or tag that has no resolved Solidtime counterpart (step 13)
**Flow:**

1. System reports the missing association needed for the time entry.
2. System does not create that time entry in Solidtime.
3. Use case ends.

### A9: Time Entry Has a Task but No Project

**Trigger:** A Clockify time entry has a task assignment but no project assignment (step 13)
**Flow:**

1. System reports that the time entry is skipped because the task cannot be associated without a project.
2. System does not create that time entry in Solidtime.
3. Use case continues at step 14.

### A10: Solidtime Creation Fails

**Trigger:** Solidtime rejects or cannot complete a required client, project, task, tag, or time-entry creation (step 9, step 10, step 11, step 12, or step 15)
**Flow:**

1. System reports which Solidtime creation failed.
2. System stops the migration without reporting success.
3. Use case ends.

### A11: Solidtime Project Archive Fails

**Trigger:** Solidtime rejects or cannot complete a project archive action after the project has otherwise been migrated (step 16)
**Flow:**

1. System reports which project archive action failed.
2. System records the archive failure in the summary.
3. Use case continues at step 17.

### A12: Dry-Run Requested

**Trigger:** User provided `--dry-run` (step 1)
**Flow:**

1. System reads and reconciles Clockify and Solidtime data as in the main success scenario.
2. System reports which clients, projects, tasks, tags, and time entries would be created, reused, or archived.
3. System does not change Solidtime or local migration state.
4. Use case continues at step 18.

## Postconditions

### Success Postconditions

- User has received a migration summary for the selected Clockify workspace, Solidtime organization, and migration time range.
- In a real migration run, supported Clockify clients, projects, tasks, tags, and time entries in scope have been created in or matched to Solidtime.
- In a real migration run, the local migration state records successful mappings and the processed cutoff.
- In a dry-run, no Solidtime data or local migration state has been changed.
- Re-running the migration with the same state and visible Solidtime data will not create duplicate records for already migrated data.

### Failure Postconditions

- User has received at least one specific error message naming the failed setup step, data item, file, or service.
- The migration has not been reported as successful.
- Local migration state contains only mappings that were completed before the failure, unless the failure occurred during dry-run.
- Solidtime contains only changes that were completed before the failure, unless the failure occurred before any real migration changes or during dry-run.

## Business Rules

### BR-019: Configuration Validated First

Migration proceeds only after configuration validation (UC-001) confirms usable Clockify and Solidtime access.

### BR-020: Supported Data Scope

Migration supports Clockify clients, projects, tasks, tags, and time entries. Billable rates are excluded from migration.

### BR-021: Dry-Run Is Read-Only

Dry-run migration must perform no Solidtime changes and no local migration state changes.

### BR-022: Local State Supports Repeatable Migration

The local migration state records source-to-target mappings so interrupted or repeated migrations can reuse records that were already matched or created.

### BR-023: State File Is User Selectable

The user may select the local migration state file. When the user does not select one, the system uses `migration-state.json`.

### BR-024: Existing Structure Is Adopted Before Creation

Before creating a Solidtime client, project, task, or tag, the system must first reuse an existing matching Solidtime record when exactly one matching record exists in the expected scope.

### BR-025: Mapping Overrides Name Matching

When the mapping file pairs a Clockify project or task with a Solidtime project or task, that pairing is used for migration instead of name-based matching.

### BR-026: Missing, Ambiguous, or Conflicting Mapping Entries Fail Closed

The migration reports the problem and stops rather than guessing or creating data from a mapping entry that is missing, ambiguous, or conflicting.

### BR-027: Structure Creation Can Be Disabled

When structure creation is disabled for a real migration run, missing Solidtime clients, projects, tasks, and tags must cause the migration to stop with a specific error instead of being created.

### BR-028: Archived Projects Included By Default

Archived Clockify projects, their tasks, and their time entries are included in migration by default.

### BR-029: Archived Inclusion Is Configurable

When `--ignore-archived` is specified, archived Clockify projects, their tasks, and their time entries are excluded from migration.

### BR-030: Time Entries Are Migrated Within The Selected Time Range

Only Clockify time entries that fall within the selected inclusive start time and exclusive end time are in scope for a migration run.

### BR-031: Time Entry Duplicate Prevention

A Clockify time entry must not be recreated in Solidtime when it is already represented in the local migration state or when an existing Solidtime time entry has the same migration-relevant member, project, task, start time, end time, billable flag, description, and tags.

### BR-032: Project Default Task Mapping

A mapping row that specifies a Solidtime task but no Clockify task may be used as the default task for Clockify time entries under that project that do not have an explicit Clockify task mapping.

### BR-033: Successful Mapping Updates Are Durable

In a real migration run, each successful client, project, task, tag, and time-entry mapping is recorded in the local migration state so a later failure can be resumed without repeating completed work.
