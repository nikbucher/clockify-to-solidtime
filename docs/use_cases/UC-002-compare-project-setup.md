# Use Case: Compare Project Setup

## Overview

**Use Case ID:** UC-002  
**Use Case Name:** Compare Project Setup  
**Primary Actor:** User  
**Goal:** See how the Clockify and Solidtime project structures match by viewing their projects and tasks side by side with each item marked as present in both systems, missing from one system, or requiring manual review.  
**Status:** Approved

## Preconditions

- User can run the command-line application.
- Clockify and Solidtime configuration is available through the supported configuration sources.
- Clockify and Solidtime are expected to be available for read access.

## Configuration Reference

The command form is:

```sh
clockify-to-solidtime compare [--config <path>] [--mapping <path>] [--ignore-archived]
```

Configuration values and their sources are the same as UC-001 (Validate Configuration), which this use case includes. The comparison resolves and validates configuration through UC-001 before reading any project data; it does not define its own configuration values.

`--mapping <path>` optionally supplies a CSV file pairing Clockify project/task names or IDs with existing Solidtime project/task names or IDs, using the same file format as the migrate command's mapping file. Only entries that resolve to projects and tasks retrieved from both systems are used; the comparison never creates a project or task to satisfy a mapping entry. `--ignore-archived` excludes archived Clockify projects and their tasks from the comparison; archived projects are included by default.

### Mapping File Format

The mapping file is a CSV. The parser requires `Clockify_Project` and `Solidtime_Project`; task and ID columns are optional. For ordinary name-based project and task mappings, use this header:

```csv
Clockify_Project,Clockify_Task,Solidtime_Project,Solidtime_Task
```

Each row maps one Clockify project to one Solidtime project, and may also map one Clockify task to one Solidtime task within that project. Blank task values are allowed for project-only rows. The optional columns `Clockify_Project_ID`, `Clockify_Task_ID`, `Solidtime_Project_ID`, and `Solidtime_Task_ID` may be added to the same CSV; when present, IDs take precedence over their matching name columns. A row that has a Solidtime task but no Clockify task is treated as a project default task row by migration; compare resolves only the project mapping and ignores that row for task pairing. Missing, ambiguous, out-of-project, or conflicting rows fail the comparison instead of being guessed. See the README for examples.

## Output Format

The comparison output is plain text with these sections:

1. Title: `Project comparison`
2. Legend: `= both, -> Clockify only, <- Solidtime only, ! manual review, A archived`
3. One project table per client, headed as `Client: <client name>`. Projects without a client are grouped under `Client: (No client)`.
4. Optional no-differences message when all projects and tasks match with no manual-review items.
5. Summary table with counts for projects and tasks.

Each project table has columns `Type`, `Clockify`, relation, and `Solidtime`.

| Relation | Meaning                                                                      |
|----------|------------------------------------------------------------------------------|
| `=`      | Item exists in both Clockify and Solidtime.                                  |
| `->`     | Item exists only in Clockify and is missing from Solidtime.                  |
| `<-`     | Item exists only in Solidtime and is missing from Clockify.                  |
| `!`      | Item requires manual review because the system cannot choose a single match. |

Archived projects are marked by appending `[A]` to the project name. Manual-review items are followed by a note row that explains why review is required.

## Main Success Scenario

1. User runs the `compare` command, optionally providing a configuration file with `--config`, a project/task mapping file with `--mapping`, and/or `--ignore-archived` to exclude archived projects.
2. System validates the Clockify and Solidtime configuration and confirms access to the source workspace and target organization, as specified by UC-001 (Validate Configuration).
3. System reads and parses the mapping file, if provided, without yet matching its entries to any project or task.
4. System retrieves all projects in the Clockify source workspace, including each project's client, tasks, and archived status, excluding archived projects and their tasks when `--ignore-archived` was given.
5. System retrieves all projects in the Solidtime target organization, including each project's client, tasks, and archived status.
6. System resolves each parsed mapping entry against the retrieved Clockify and Solidtime projects and tasks, identifying the specific project or task each entry names, except that an entry with no Clockify task is resolved only at the project level.
7. System pairs each Clockify project with the Solidtime project that has the same name under the same client, or with the Solidtime project resolved for it by the mapping file, identifying projects present in both systems and projects present in only one.
8. For each matched project pair, System pairs their tasks by name or by the mapping file, identifying tasks present in both, only in Clockify, only in Solidtime, or requiring manual review.
9. System displays the projects grouped by client using the specified output format, marking each project as present in both systems, only in Clockify, only in Solidtime, or requiring manual review, and under each matched project lists its tasks with the same markings.
10. System displays a summary counting how many projects and tasks match, how many exist in only one system, and how many require manual review.
11. User reviews the annotated comparison to verify how the Clockify and Solidtime project structures match, achieving the goal, and the system confirms that no data was changed.

## Alternative Flows

### A1: Configuration Validation Fails

**Trigger:** Configuration validation reports a problem with configuration or account access (step 2)
**Flow:**

1. System reports the specific setup step or service that failed, as specified by UC-001.
2. System does not retrieve any project data.
3. Use case ends.

### A2: A Remote Service Cannot Be Reached

**Trigger:** Clockify or Solidtime cannot be reached, or responds with an error, while retrieving projects or tasks (step 4 or step 5)
**Flow:**

1. System reports which service failed and that the comparison could not be completed.
2. System does not display a comparison.
3. Use case ends.

### A3: Clockify Workspace Has No Projects

**Trigger:** The resolved Clockify source workspace contains no projects (step 4)
**Flow:**

1. System notes that there are no Clockify projects.
2. System treats any Solidtime projects as present only in Solidtime.
3. Use case continues at step 5.

### A4: Solidtime Organization Has No Projects

**Trigger:** The resolved Solidtime target organization contains no projects (step 5)
**Flow:**

1. System notes that there are no Solidtime projects.
2. System treats any Clockify projects as present only in Clockify.
3. Use case continues at step 6.

### A5: A Project Name Is Ambiguous

**Trigger:** More than one project shares the same name under the same client in one of the systems, and the mapping file does not resolve the pairing (step 7)
**Flow:**

1. System does not guess a match for the ambiguous project name.
2. System marks the ambiguous project name for manual review in the displayed comparison.
3. Use case continues at step 9.

### A6: Project Structures Are Identical

**Trigger:** Every project and task is present in both systems (step 9)
**Flow:**

1. System reports that the Clockify and Solidtime project structures match with no differences.
2. Use case continues at step 10.

### A7: A Task Name Is Ambiguous

**Trigger:** More than one task shares the same name within a matched project in one of the systems, and the mapping file does not resolve the pairing (step 8)
**Flow:**

1. System does not guess a match for the ambiguous task name.
2. System marks the ambiguous task name for manual review in the displayed comparison.
3. Use case continues at step 9.

### A8: Mapping File Cannot Be Read

**Trigger:** The file given with `--mapping` does not exist, cannot be read, or is not a valid mapping file (step 3)
**Flow:**

1. System reports the specific problem with the mapping file.
2. System does not retrieve any project data.
3. Use case ends.

### A9: A Mapping Entry Is Missing, Ambiguous, or Conflicting

**Trigger:** A mapping entry names or identifies a Clockify or Solidtime project or task that is not among the retrieved data, a name-based reference in the mapping file matches more than one such project or task, or the mapping file pairs the same Clockify project or task with two different Solidtime targets (step 6)
**Flow:**

1. System reports which mapping entry is missing, ambiguous, or conflicting, and why.
2. System does not display a comparison.
3. Use case ends.

## Postconditions

### Success Postconditions

- User has seen the current Clockify and Solidtime projects and tasks, with each item marked as present in both systems, missing from one system, or requiring manual review.
- No Clockify data, Solidtime data, or local migration state has been changed.

### Failure Postconditions

- User has not seen a completed comparison.
- User has received at least one specific error message naming the failed setup step or service.
- No Clockify data, Solidtime data, or local migration state has been changed.

## Business Rules

### BR-008: Read-Only Comparison

Comparison must not create, update, or delete Clockify data, Solidtime data, or local migration state.

### BR-009: Configuration Validated First

Comparison proceeds only after configuration validation (UC-001) confirms usable Clockify and Solidtime access.

### BR-010: Name-Based Project Matching

A Clockify project and a Solidtime project are treated as the same project when they share a name under the same client, consistent with how the migrate command matches projects.

### BR-011: Name-Based Task Matching

A Clockify task and a Solidtime task are treated as the same task when they share a name within a matched project.

### BR-012: Active And Archived Projects Included

The comparison includes both active and archived projects, and their tasks, so it reflects the full project set in each system.

See BR-016: Archived Inclusion Is Configurable; this default can be overridden with `--ignore-archived`.

### BR-013: Ambiguous Matches Are Flagged, Not Guessed

When more than one project shares the same name under the same client, or more than one task shares the same name within a matched project, the system flags it for manual review instead of choosing a match.

### BR-014: Mapping Overrides Name Matching

When the mapping file pairs a Clockify project or task with a Solidtime project or task, that pairing is used for the comparison instead of name-based matching, consistent with how the migrate command applies mapping overrides.

### BR-015: Mapping File Is Read-Only Input

The mapping file is read as input only; the comparison does not modify the mapping file, local migration state, Clockify data, or Solidtime data.

### BR-016: Archived Inclusion Is Configurable

By default the comparison includes archived projects and their tasks (BR-012); when `--ignore-archived` is specified, archived Clockify projects and their tasks are excluded from the comparison, consistent with the migrate command's `--ignore-archived` behavior.

### BR-017: Mapping Rows Without a Clockify Task Are Exempt From Task Resolution

A mapping row that specifies a Solidtime task but no Clockify task (a "project default task" row, used by the migrate command to assign untracked time entries) has its project reference resolved normally, but is exempt from task-level resolution and has no effect on task pairing in the comparison, since the comparison does not process time entries. BR-018's task-level requirements do not apply to such a row.

### BR-018: Missing, Ambiguous, or Conflicting Mapping Entries Fail Closed

The comparison reports the problem and displays no results, rather than guessing or creating data, when: a mapping entry names or identifies a Clockify or Solidtime project or task that is not among the retrieved data; a name-based reference in the mapping file matches more than one such project or task; or the mapping file pairs the same Clockify project or task with two different Solidtime targets. This applies to every mapping row's project reference, and to a row's task reference except where BR-017 exempts it.
