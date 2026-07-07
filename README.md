# clockify-to-solidtime

[![CI](https://github.com/nikbucher/clockify-to-solidtime/actions/workflows/ci.yml/badge.svg)](https://github.com/nikbucher/clockify-to-solidtime/actions/workflows/ci.yml)

A command-line tool to migrate time tracking data from [Clockify](https://clockify.me) to [Solidtime](https://solidtime.io) through their APIs.

## What it does

Transfers supported Clockify workspace data to a Solidtime organization:

- **Clients and projects** - including project metadata and task structure.
- **Time entries** - including dates, durations, descriptions, billable flags, project associations, task associations, and tag assignments.
- **Tags** - matched or created by name.

Billable rates are not migrated.

## Who it's for

Clockify users with API access who want to move to Solidtime without manually recreating clients, projects, tasks, tags, and time entries.

## Installation

### Prerequisites

- A Clockify account with API access.
- A Solidtime account with API access.

Create a Clockify API key and a Solidtime API token from the account or developer settings in each product, then provide them through environment variables, a `.env` file, or a TOML config file.

### Pre-built binaries

Download the archive for your platform from [GitHub Releases](https://github.com/nikbucher/clockify-to-solidtime/releases).

| Platform            | Target                      | Archive                                                  |
|---------------------|-----------------------------|----------------------------------------------------------|
| Linux x86_64        | `x86_64-unknown-linux-gnu`  | `clockify-to-solidtime-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux arm64         | `aarch64-unknown-linux-gnu` | `clockify-to-solidtime-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel         | `x86_64-apple-darwin`       | `clockify-to-solidtime-x86_64-apple-darwin.tar.gz`       |
| macOS Apple silicon | `aarch64-apple-darwin`      | `clockify-to-solidtime-aarch64-apple-darwin.tar.gz`      |
| Windows x86_64      | `x86_64-pc-windows-msvc`    | `clockify-to-solidtime-x86_64-pc-windows-msvc.zip`       |
| Windows arm64       | `aarch64-pc-windows-msvc`   | `clockify-to-solidtime-aarch64-pc-windows-msvc.zip`      |

### Build from source

Requires Rust with edition 2024 support.

```sh
cargo install --git https://github.com/nikbucher/clockify-to-solidtime
```

For local development:

```sh
git clone https://github.com/nikbucher/clockify-to-solidtime.git
cd clockify-to-solidtime
cargo build --release
```

From a source checkout, run commands with `cargo run --`, for example:

```sh
cargo run -- validate
```

## Configuration

At minimum, provide:

```sh
export CLOCKIFY_API_KEY="..."
export SOLIDTIME_API_TOKEN="..."
```

Optional configuration values:

| Purpose                    | Environment variable        | Config file key             | Default                          |
|----------------------------|-----------------------------|-----------------------------|----------------------------------|
| Clockify workspace         | `CLOCKIFY_WORKSPACE_ID`     | `clockify_workspace_id`     | User's default workspace         |
| Solidtime organization     | `SOLIDTIME_ORGANIZATION_ID` | `solidtime_organization_id` | User's only available membership |
| Clockify API base URL      | `CLOCKIFY_BASE_URL`         | `clockify_base_url`         | Clockify production API          |
| Solidtime API base URL     | `SOLIDTIME_BASE_URL`        | `solidtime_base_url`        | Solidtime production API         |

Configuration sources are applied in this precedence order:

1. Values in the file passed with `--config <path>`.
2. Exported environment variables.
3. Values from a `.env` file in the working directory.
4. Built-in defaults, when available.

Use [`.env.example`](.env.example) as a template for local `.env` files.

The same keys can be provided via `--config config.toml`:

```toml
clockify_api_key = "..."
solidtime_api_token = "..."
clockify_workspace_id = "..."
solidtime_organization_id = "..."
```

`validate`, `compare`, and `migrate` validate configuration before reading or changing migration data. `completions` does not require configuration or network access.

## Recommended Workflow

1. Configure your Clockify and Solidtime credentials.
2. Run `validate` to check credentials, workspace selection, and organization selection.
3. Run `compare` to review project and task alignment.
4. Add a mapping CSV if names are renamed, duplicated, or ambiguous.
5. Run `migrate --dry-run` to preview writes.
6. Run `migrate` and review the summary.

Use the same `--state` file for repeat or resumed migrations. The default state file is `migration-state.json`.

## Use Cases

The use-case specs are the canonical behavior reference for full scenarios, edge cases, business rules, and output expectations:

- [UC-001 Validate Configuration](docs/use_cases/UC-001-validate-configuration.md)
- [UC-002 Compare Project Setup](docs/use_cases/UC-002-compare-project-setup.md)
- [UC-003 Migrate Time Tracking Data](docs/use_cases/UC-003-migrate-time-tracking-data.md)
- [UC-004 Generate Shell Completions](docs/use_cases/UC-004-generate-shell-completions.md)

## Commands

### Validate

```sh
clockify-to-solidtime validate
clockify-to-solidtime validate --config config.toml
```

Checks that required configuration is present and that the selected Clockify workspace and Solidtime organization are reachable. It does not change Clockify data, Solidtime data, or local migration state.

Full behavior: [UC-001 Validate Configuration](docs/use_cases/UC-001-validate-configuration.md).

### Compare

```sh
clockify-to-solidtime compare
clockify-to-solidtime compare --ignore-archived
clockify-to-solidtime compare --config config.toml --mapping project-task-mapping.csv
```

Shows a read-only comparison of Clockify and Solidtime projects and tasks. By default, archived Clockify projects and their tasks are included. Use `--ignore-archived` to exclude them. Use `--mapping` when existing Solidtime projects or tasks should be paired with differently named, duplicated, or ambiguous Clockify projects or tasks.

Full behavior: [UC-002 Compare Project Setup](docs/use_cases/UC-002-compare-project-setup.md).

### Migrate

```sh
clockify-to-solidtime migrate --dry-run --from 2024-01-01 --to 2024-02-01
clockify-to-solidtime migrate --from 2024-01-01 --to 2024-02-01
```

Migrates supported Clockify clients, projects, tasks, tags, and time entries to Solidtime. Use `--dry-run` before a real migration to preview planned creates, reuses, archive actions, and skipped duplicates without writing to Solidtime or the state file.

Useful options:

| Option                  | Use it when                                                                                         |
|-------------------------|------------------------------------------------------------------------------------------------------|
| `--state <path>`        | You want to choose the local migration state file instead of `migration-state.json`.                 |
| `--mapping <path>`      | You need explicit project or task pairings, or a default Solidtime task for untasked Clockify entries. |
| `--ignore-archived`     | You want to skip archived Clockify projects, their tasks, and their time entries.                    |
| `--no-create-structure` | Existing Solidtime clients, projects, tasks, and tags must be reused instead of created.             |

`--from` is inclusive and defaults to `2000-01-01T00:00:00Z`. `--to` is exclusive and defaults to the current time. Both options accept a short date such as `2024-01-01` or a full RFC3339 timestamp such as `2024-01-01T00:00:00Z`.

Full behavior: [UC-003 Migrate Time Tracking Data](docs/use_cases/UC-003-migrate-time-tracking-data.md).

### Shell Completions

Homebrew installs bash, zsh, and fish completions automatically. For manual installation, generate a completion script for the target shell:

```sh
clockify-to-solidtime completions bash | sudo tee /etc/bash_completion.d/clockify-to-solidtime
```

```zsh
clockify-to-solidtime completions zsh > "${fpath[1]}/_clockify-to-solidtime"
```

```fish
clockify-to-solidtime completions fish > ~/.config/fish/completions/clockify-to-solidtime.fish
```

```elvish
clockify-to-solidtime completions elvish > ~/.config/elvish/lib/clockify-to-solidtime.elv
```

```powershell
clockify-to-solidtime completions powershell | Out-String | Invoke-Expression
```

Supported shell values are `bash`, `zsh`, `fish`, `powershell`, and `elvish`.

Full behavior: [UC-004 Generate Shell Completions](docs/use_cases/UC-004-generate-shell-completions.md).

## Mapping CSV Basics

Use `--mapping project-task-mapping.csv` with `compare` or `migrate` when default name-based matching is not enough. The filename is only an example; any CSV path can be used.

The recommended header is:

```csv
Clockify_Project,Clockify_Task,Solidtime_Project,Solidtime_Task
```

Examples:

```csv
Clockify_Project,Clockify_Task,Solidtime_Project,Solidtime_Task
Legacy Website,,Website Relaunch,
Website Relaunch,QA,Website Relaunch,Testing
Website Relaunch,,Website Relaunch,General
```

The first row maps a renamed project. The second row maps a renamed task within that project. The third row defines a default Solidtime task for Clockify time entries in `Website Relaunch` that do not have a Clockify task.

Optional ID columns can be added when names are duplicated or ambiguous:

- `Clockify_Project_ID`
- `Clockify_Task_ID`
- `Solidtime_Project_ID`
- `Solidtime_Task_ID`

IDs take precedence over their matching name columns. Missing, ambiguous, out-of-project, or conflicting mapping rows stop the command instead of being guessed.

For the detailed mapping rules, see [UC-002 Compare Project Setup](docs/use_cases/UC-002-compare-project-setup.md) and [UC-003 Migrate Time Tracking Data](docs/use_cases/UC-003-migrate-time-tracking-data.md).

## Safe Repeatable Runs

The migration is designed to be safe to re-run where possible. It uses local state, existing Solidtime records, and time-entry matching to avoid creating duplicate migrated data.

For implementation decisions, data mapping, and idempotency details, see [`docs/migration-design.md`](docs/migration-design.md).

## Documentation

- [`docs/requirements.md`](docs/requirements.md)
- [`docs/use_cases.puml`](docs/use_cases.puml)
- [`docs/use_cases/`](docs/use_cases/)
- [`docs/migration-design.md`](docs/migration-design.md)

## Releasing

Create and push a version tag to build and publish release binaries:

```sh
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

MIT - see [`LICENSE`](LICENSE).
