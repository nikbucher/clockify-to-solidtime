# clockify-to-solidtime

[![CI](https://github.com/nikbucher/clockify-to-solidtime/actions/workflows/ci.yml/badge.svg)](https://github.com/nikbucher/clockify-to-solidtime/actions/workflows/ci.yml)

A command-line tool to migrate your time tracking data from [Clockify](https://clockify.me) to [Solidtime](https://solidtime.io) via their APIs.

## What it does

Transfers supported Clockify workspace data to Solidtime as completely and reliably as possible:

- **Clients & Projects** - full hierarchy including projects and tasks
- **Time entries** - dates, durations, descriptions, and project associations
- **Tags** - carried over with their assignments

## Who it's for

Clockify users with the required API access who want to switch to Solidtime without manually recreating their data.

## Installation

### Prerequisites

- A Clockify account with API access.
- A Solidtime account with API access.

Create a Clockify API key and a Solidtime API token from the account or developer settings in each product, then provide them through environment variables, a `.env` file, or a config file.

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

From a source checkout, you can run the same commands with `cargo run --`, for example:

```sh
cargo run -- validate
```

## Configuration

Provide required credentials through exported environment variables, a `.env` file, or a TOML config file:

```sh
export CLOCKIFY_API_KEY="..."
export SOLIDTIME_API_TOKEN="..."
```

Optional environment variables:

- `CLOCKIFY_WORKSPACE_ID` - overrides the user's default Clockify workspace.
- `SOLIDTIME_ORGANIZATION_ID` - required when the Solidtime token has more than one membership.
- `CLOCKIFY_BASE_URL` and `SOLIDTIME_BASE_URL` - override API base URLs.

Alternatively, put the variables in a `.env` file in the working directory. Use `.env.example` as a template:

```dotenv
CLOCKIFY_API_KEY=...
SOLIDTIME_API_TOKEN=...
```

Real environment variables and `--config` values take precedence over `.env` values.

The same keys can be provided via `--config config.toml`:

```toml
clockify_api_key = "..."
solidtime_api_token = "..."
clockify_workspace_id = "..."
solidtime_organization_id = "..."
```

Migration state is stored in `migration-state.json` by default. Override it with `--state`.

Use `--ignore-archived` to skip archived Clockify projects, their tasks, and their time entries.

## Workflow

1. Configure your Clockify and Solidtime API keys.
2. Run `validate` to check credentials and service access.
3. Run `compare` to preview project and task alignment.
4. Run `migrate --dry-run` to preview writes.
5. Run `migrate` and review the summary.

## Usage

Check credentials and reachability:

```sh
clockify-to-solidtime validate
```

Preview project and task alignment:

```sh
clockify-to-solidtime compare --ignore-archived
```

Preview migration writes:

```sh
clockify-to-solidtime migrate --dry-run --from 2024-01-01 --to 2024-02-01
```

Run the migration:

```sh
clockify-to-solidtime migrate --from 2024-01-01 --to 2024-02-01
```

### Shell completions

Homebrew installs bash, zsh, and fish completions automatically.

For manual installation, generate a completion script for the target shell:

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

### Validate

```sh
clockify-to-solidtime validate
clockify-to-solidtime validate --config config.toml
```

Checks that required configuration is present and that the selected Clockify workspace and Solidtime organization are reachable.

### Compare

```sh
clockify-to-solidtime compare
clockify-to-solidtime compare --ignore-archived
clockify-to-solidtime compare --config config.toml --mapping project-task-mapping.csv
```

Shows a read-only, side-by-side comparison of Clockify and Solidtime projects and tasks. It accepts `--config`, `--mapping`, and `--ignore-archived`. See [`docs/use_cases/UC-002-compare-project-setup.md`](docs/use_cases/UC-002-compare-project-setup.md) for the full output format.

### Migrate

```sh
clockify-to-solidtime migrate --dry-run --from 2024-01-01 --to 2024-02-01
clockify-to-solidtime migrate --from 2024-01-01 --to 2024-02-01
```

Runs the Clockify to Solidtime migration. Use `--dry-run` before a real migration to preview planned changes.

`--from` and `--to` are optional. `--from` defaults to `2000-01-01T00:00:00Z`, and `--to` defaults to the current time, so omitting both migrates all entries up to now. `--from` is inclusive and `--to` is exclusive. Each accepts a short date such as `2024-01-01` or a full RFC3339 timestamp such as `2024-01-01T00:00:00Z`.

On a real run, `--no-create-structure` skips creating missing clients, projects, tasks, and tags. The migration stops if required Solidtime structure is missing.

### Mapping CSV for projects and tasks

Use `--mapping project-task-mapping.csv` with `compare` or `migrate` when Clockify and Solidtime projects or tasks should be paired differently than the default name-based matching. The filename is only an example; any CSV path can be used.

This file is useful for:

- Renamed projects or tasks.
- Duplicate or ambiguous project or task names.
- Assigning a default Solidtime task to Clockify time entries that have no task.

For most mappings, use this header:

```csv
Clockify_Project,Clockify_Task,Solidtime_Project,Solidtime_Task
```

Columns:

| Column              | Required in header | Value may be blank | Description                                                                                                                                                                              |
|---------------------|--------------------|--------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Clockify_Project`  | Yes                | No                 | Clockify project name.                                                                                                                                                                   |
| `Clockify_Task`     | No                 | Yes                | Clockify task name. Leave blank for project-only rows or default-task rows.                                                                                                              |
| `Solidtime_Project` | Yes                | No                 | Solidtime project name.                                                                                                                                                                  |
| `Solidtime_Task`    | No                 | Yes                | Solidtime task name. Leave blank for project-only rows. Set this with a blank `Clockify_Task` to define the migration default task for un-tasked Clockify entries in the mapped project. |

When names are duplicated or ambiguous, add any of these optional ID columns to the same CSV:

| Optional column        | Takes precedence over |
|------------------------|-----------------------|
| `Clockify_Project_ID`  | `Clockify_Project`    |
| `Clockify_Task_ID`     | `Clockify_Task`       |
| `Solidtime_Project_ID` | `Solidtime_Project`   |
| `Solidtime_Task_ID`    | `Solidtime_Task`      |

Only `Clockify_Project` and `Solidtime_Project` are required by the parser. Include the task columns anyway unless you are intentionally creating a project-only mapping file; the four-column header is easier to read and matches the examples below.

Mapping entries fail closed: missing records, ambiguous name matches, tasks outside the mapped project, conflicting CSV rows, or conflicts with `migration-state.json` stop the command instead of guessing.

Common examples:

| Use case                                        | `Clockify_Project` | `Clockify_Task` | `Solidtime_Project` | `Solidtime_Task` |
|-------------------------------------------------|--------------------|-----------------|---------------------|------------------|
| Project rename                                  | `Legacy Website`   |                 | `Website Relaunch`  |                  |
| Task rename                                     | `Website Relaunch` | `QA`            | `Website Relaunch`  | `Testing`        |
| Default task for Clockify entries without tasks | `Website Relaunch` |                 | `Website Relaunch`  | `General`        |

ID-based mapping for ambiguous names:

| `Clockify_Project` | `Clockify_Task` | `Solidtime_Project` | `Solidtime_Task` | `Clockify_Project_ID` | `Clockify_Task_ID` | `Solidtime_Project_ID` | `Solidtime_Task_ID` |
|--------------------|-----------------|---------------------|------------------|-----------------------|--------------------|------------------------|---------------------|
| `Website Relaunch` | `QA`            | `Website Relaunch`  | `Testing`        | `clk_project_123`     | `clk_task_456`     | `sol_project_789`      | `sol_task_012`      |

Recommended workflow:

1. Run `clockify-to-solidtime compare`.
2. Create `project-task-mapping.csv` for renamed or ambiguous projects and tasks.
3. Run `clockify-to-solidtime compare --mapping project-task-mapping.csv`.
4. Run `clockify-to-solidtime migrate --dry-run --mapping project-task-mapping.csv`.
5. Run `clockify-to-solidtime migrate --mapping project-task-mapping.csv` only after the dry run looks right.

## Safe repeatable runs

The tool is designed to be safe to re-run where possible, skipping or updating already-migrated data rather than creating duplicates.

For implementation decisions, data mapping, and idempotency details, see [`docs/migration-design.md`](docs/migration-design.md).

## Documentation

- [`docs/vision.md`](docs/vision.md)
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
