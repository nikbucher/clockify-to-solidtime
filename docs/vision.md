# Vision: clockify-to-solidtime

## Goal

Let Clockify users move their time-tracking data to Solidtime completely and safely, without
manually recreating anything.

## Users

- **Migrating individual / freelancer**: Has historical time data in Clockify and wants to
  adopt Solidtime without losing their clients, projects, and entries.
- **Team or workspace owner**: Responsible for a Clockify workspace and needs a trustworthy,
  reviewable way to transfer its data before switching tools.

## Core Features

- **Configuration validation**: Confirm access to both Clockify and Solidtime before any
  migration, surfacing setup problems early.
- **Data migration**: Transfer clients, projects, tasks, tags, and time entries from a Clockify
  workspace into Solidtime, preserving their relationships.
- **Dry-run preview**: Show exactly what would be migrated, changing nothing.
- **Project comparison**: Put Clockify and Solidtime projects and tasks side by side to verify
  they match.
- **Safe re-runs**: Skip or update already-migrated data instead of creating duplicates.

## Key Workflows

1. Validate configuration and access to both services.
2. Preview the migration with a dry-run and review what will change.
3. Run the migration and review the summary of what was transferred.

## Success Criteria

- A user can migrate a workspace's supported data without manual re-entry.
- Dry-run and compare make no remote or state changes.
- Re-running a migration produces no duplicates.
- Failures name the specific step or service that went wrong.
