# clockify-to-solidtime

A command-line tool to migrate your time tracking data from [Clockify](https://clockify.me) to [Solidtime](https://solidtime.io) via their APIs.

## What it does

Transfers supported Clockify workspace data to Solidtime as completely and reliably as possible:

- **Clients & Projects** — full hierarchy including projects and tasks
- **Time entries** — dates, durations, descriptions, and project associations
- **Tags** — carried over with their assignments

## Who it's for

Clockify users with the required API access who want to switch to Solidtime without manually recreating their data.

## Workflow

1. Configure your Clockify and Solidtime API keys.
2. Run a dry-run to preview what will be migrated.
3. Execute the migration and review the summary.

## Safe repeatable runs

The tool is designed to be safe to re-run where possible — skipping or updating already-migrated data rather than creating duplicates.
