# clockify-to-solidtime

[![CI](https://github.com/nikbucher/clockify-to-solidtime/actions/workflows/ci.yml/badge.svg)](https://github.com/nikbucher/clockify-to-solidtime/actions/workflows/ci.yml)

A command-line tool that migrates time-tracking data from [Clockify](https://clockify.me) to [Solidtime](https://solidtime.io) through their APIs.

It transfers clients, projects, tasks, tags, and time entries while preserving relationships. Dates, durations, descriptions, billable flags, associations, and tags are retained; billable rates are not migrated.

The migration is one-way. Archived Clockify projects are included by default, and real migrations are not transactional: writes completed before an error remain in Solidtime. Preview a limited date range first and preserve the state file for retries.

## Quick Start

Homebrew is recommended on macOS and Linux:

```sh
brew install nikbucher/tap/clockify-to-solidtime
curl -fsSLO https://raw.githubusercontent.com/nikbucher/clockify-to-solidtime/main/.env.example
cp .env.example .env
```

Edit `.env`, set `CLOCKIFY_API_KEY` and `SOLIDTIME_API_TOKEN`, and never commit it. Then validate the selected accounts and compare their projects:

```sh
clockify-to-solidtime validate
clockify-to-solidtime compare
```

If either command fails, use [Troubleshooting](#troubleshooting) to identify the configuration or connectivity problem before continuing.

Preview a small range (start inclusive, end exclusive):

```sh
clockify-to-solidtime migrate --dry-run \
  --from 2024-01-01 --to 2024-02-01 \
  --state ./clockify-2024-01-state.json
```

After reviewing the preview, run the real migration with the same range and state path:

```sh
clockify-to-solidtime migrate \
  --from 2024-01-01 --to 2024-02-01 \
  --state ./clockify-2024-01-state.json
```

Keep the state file and reuse it for every retry or resumed run. Add `--ignore-archived` to both migration commands if archived projects, their tasks, and their time entries should be excluded.

## Installation

### Homebrew

```sh
brew install nikbucher/tap/clockify-to-solidtime
```

Homebrew also installs bash, zsh, and fish completions.

### Release Archives

Download the archive for your platform from [GitHub Releases](https://github.com/nikbucher/clockify-to-solidtime/releases).

| Platform            | Archive                                                  |
|---------------------|----------------------------------------------------------|
| Linux x86_64        | `clockify-to-solidtime-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux arm64         | `clockify-to-solidtime-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel         | `clockify-to-solidtime-x86_64-apple-darwin.tar.gz`       |
| macOS Apple silicon | `clockify-to-solidtime-aarch64-apple-darwin.tar.gz`      |
| Windows x86_64      | `clockify-to-solidtime-x86_64-pc-windows-msvc.zip`       |
| Windows arm64       | `clockify-to-solidtime-aarch64-pc-windows-msvc.zip`      |

On macOS or Linux, replace `<target>` with the archive target:

```sh
mkdir -p "$HOME/.local/bin"
tar -xzf clockify-to-solidtime-<target>.tar.gz
install -m 755 clockify-to-solidtime "$HOME/.local/bin/clockify-to-solidtime"
export PATH="$HOME/.local/bin:$PATH"
```

Add the final line to your shell profile. On Windows, run PowerShell and replace `<target>` with a Windows target:

```powershell
$InstallDir = Join-Path $HOME ".local\bin"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Expand-Archive ".\clockify-to-solidtime-<target>.zip" -DestinationPath $InstallDir -Force
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($UserPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
}
$env:Path = "$env:Path;$InstallDir"
```

### Build from Source

Install [Rust with rustup](https://www.rust-lang.org/tools/install). The project uses Rust edition 2024 and requires a current stable toolchain.

```sh
cargo install --git https://github.com/nikbucher/clockify-to-solidtime
```

For development:

```sh
git clone https://github.com/nikbucher/clockify-to-solidtime.git
cd clockify-to-solidtime
cargo build --release
cargo run -- validate
```

## Configuration

Create credentials using the service instructions:

- [Clockify API authentication](https://docs.clockify.me/#section/Authentication)
- [Solidtime API access and token creation](https://docs.solidtime.io/user-guide/access-api)

The portable setup is a `.env` file in the command's working directory. Copy [`.env.example`](.env.example), then set:

```dotenv
CLOCKIFY_API_KEY=your-clockify-api-key
SOLIDTIME_API_TOKEN=your-solidtime-api-token
```

Never commit credentials. This repository ignores `.env` and `config.toml`; verify ignore rules elsewhere.

Clockify uses the account's default workspace unless `CLOCKIFY_WORKSPACE_ID` is set. Solidtime selects automatically only when the token has exactly one organization membership; set `SOLIDTIME_ORGANIZATION_ID` for multiple memberships or to make the intended target explicit.

POSIX environment variables:

```sh
export CLOCKIFY_API_KEY="..."
export SOLIDTIME_API_TOKEN="..."
export CLOCKIFY_WORKSPACE_ID="..."       # optional
export SOLIDTIME_ORGANIZATION_ID="..."   # optional
```

PowerShell environment variables:

```powershell
$env:CLOCKIFY_API_KEY = "..."
$env:SOLIDTIME_API_TOKEN = "..."
$env:CLOCKIFY_WORKSPACE_ID = "..."       # optional
$env:SOLIDTIME_ORGANIZATION_ID = "..."   # optional
```

Or pass `--config config.toml`:

```toml
clockify_api_key = "..."
solidtime_api_token = "..."
clockify_workspace_id = "..."
solidtime_organization_id = "..."
```

Precedence is: `--config` file, exported variables, `.env`, then built-in defaults. Advanced deployments may override `CLOCKIFY_BASE_URL` and `SOLIDTIME_BASE_URL`; ordinary users should retain the production defaults in `.env.example`.

## Troubleshooting

`validate` reports the configuration or connectivity problem it found. Dynamic values in the messages below are shown as `<id>`, `<path>`, `<Service>`, `<subject>`, and `<status>`.

| Symptom                                                                                           | Cause                                                                                             | Resolution                                                                                            |
|---------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|
| `failed to load .env file`                                                                        | A `.env` file exists in the working directory but cannot be read or parsed.                       | Fix the file's permissions or syntax, or remove it if configuration comes from another source.        |
| `CLOCKIFY_API_KEY is required` or `SOLIDTIME_API_TOKEN is required`                               | The credential is missing or empty in every configured source.                                    | Set the named value in the `--config` file, environment, or `.env`.                                   |
| `failed to read config file <path>`                                                               | The path passed to `--config` does not exist or is not readable.                                  | Correct the path and file permissions, or omit `--config` when using environment variables or `.env`. |
| `failed to parse config file <path>`                                                              | The configuration file is not valid TOML.                                                         | Correct its TOML syntax and use the keys shown in [Configuration](#configuration).                    |
| `A Clockify workspace must be selected; set CLOCKIFY_WORKSPACE_ID or provide a default workspace` | No workspace ID is configured and the Clockify account has no default workspace.                  | Set `CLOCKIFY_WORKSPACE_ID` to an accessible workspace.                                               |
| ``Clockify workspace `<id>` is not among your workspaces``                                        | The configured or default workspace ID is not visible to the API key.                             | Check the workspace ID and the API key's account access.                                              |
| `A Solidtime organization must be selected; set SOLIDTIME_ORGANIZATION_ID`                        | No organization ID is configured and the token does not have exactly one organization membership. | Set `SOLIDTIME_ORGANIZATION_ID` to the intended target organization.                                  |
| ``Solidtime organization `<id>` is not among your memberships``                                   | The configured organization ID is not visible to the token.                                       | Check the organization ID and the token's memberships.                                                |
| `<Service> rejected the configured credential; <subject> access could not be confirmed`           | Clockify or Solidtime returned `401 Unauthorized` or `403 Forbidden`.                             | Replace an expired or revoked credential and confirm it grants access to the selected target.         |
| `<Service> returned <status>; <subject> access could not be confirmed`                            | The service returned an unexpected HTTP status.                                                   | Check the service status and retry; persistent failures may require service-side investigation.       |
| `<Service> <subject> access could not be confirmed`                                               | The service could not be reached and no HTTP status was received.                                 | Check network access and the configured `CLOCKIFY_BASE_URL` or `SOLIDTIME_BASE_URL`.                  |

For canonical failure scenarios and postconditions, see [UC-001 Validate Configuration](docs/use_cases/UC-001-validate-configuration.md).

## Safe Migration Workflow

1. Run `validate` and confirm the displayed workspace and organization are the intended source and target.
2. Run `compare`; add `--mapping` for renamed or ambiguous projects or tasks.
3. Run `migrate --dry-run` with limited `--from`/`--to` dates and an explicit `--state`. It writes neither Solidtime nor the state file.
4. Review the summary, then remove only `--dry-run`. Keep the range, mapping, archive choice, and state path unchanged.
5. Preserve the state file. A real migration writes incrementally and is not transactional. On failure, completed remote writes remain; rerun with the same state file so recorded items are reused.

Test against the organization you intend to migrate into. Archived Clockify projects are migrated unless `--ignore-archived` is used.

Sanitized successful output:

```text
Configuration validated
Clockify workspace: Example Workspace (workspace-example)
Solidtime organization: Example Organization (organization-example)
No Clockify data, Solidtime data, or local migration state was changed.
```

```text
Project comparison

Legend: = both, -> Clockify only, <- Solidtime only, ! manual review, A archived

Client: Example Client
+---------+----------+---+-----------+
| Type    | Clockify |   | Solidtime |
+---------+----------+---+-----------+
| Project | Website  | = | Website   |
+---------+----------+---+-----------+

Clockify and Solidtime project structures match with no differences.

Summary
+----------+---------+---------------+----------------+---------------+
| Type     | Matched | Clockify only | Solidtime only | Manual review |
+----------+---------+---------------+----------------+---------------+
| Projects | 1       | 0             | 0              | 0             |
| Tasks    | 0       | 0             | 0              | 0             |
+----------+---------+---------------+----------------+---------------+
```

```text
Reading Clockify workspace workspace-example and Solidtime organization organization-example
Dry-run: Solidtime writes and state persistence are disabled
Migration summary
  clients: 1 created, 0 reused
  projects: 1 created, 0 reused, 0 archived, 0 archive failures
  tasks: 2 created, 0 reused
  tags: 3 created, 0 reused
  time entries: 12 created, 0 reused
```

## Command Reference

### `validate`

```sh
clockify-to-solidtime validate [--config <path>]
```

Checks credentials and target selection without changes. See [UC-001](docs/use_cases/UC-001-validate-configuration.md).

### `compare`

```sh
clockify-to-solidtime compare [--config <path>] [--mapping <path>] [--ignore-archived]
```

Compares projects and tasks without changes. Archived projects are included by default. See [UC-002](docs/use_cases/UC-002-compare-project-setup.md).

### `migrate`

```sh
clockify-to-solidtime migrate [--dry-run] [--config <path>] [--state <path>] \
  [--mapping <path>] [--no-create-structure] [--ignore-archived] \
  [--from <timestamp>] [--to <timestamp>]
```

| Option                  | Meaning                                             |
|-------------------------|-----------------------------------------------------|
| `--dry-run`             | Reconcile without Solidtime or state-file writes.   |
| `--config <path>`       | Read a TOML configuration file.                     |
| `--state <path>`        | Select state; default: `migration-state.json`.      |
| `--mapping <path>`      | Read explicit project/task mappings from CSV.       |
| `--no-create-structure` | Abort a real run if required structure is missing.  |
| `--ignore-archived`     | Exclude archived projects, tasks, and time entries. |
| `--from <timestamp>`    | Inclusive start; default: `2000-01-01T00:00:00Z`.   |
| `--to <timestamp>`      | Exclusive end; default: current time.               |

Short dates and RFC3339 timestamps are accepted. See [UC-003](docs/use_cases/UC-003-migrate-time-tracking-data.md).

### `completions`

```sh
clockify-to-solidtime completions <bash|zsh|fish|powershell|elvish>
```

Writes a completion script to standard output without configuration or network access. See [UC-004](docs/use_cases/UC-004-generate-shell-completions.md).

## Mapping

Use `--mapping project-task-mapping.csv` when name matching is insufficient:

```csv
Clockify_Project,Clockify_Task,Solidtime_Project,Solidtime_Task
Legacy Website,,Website Relaunch,
Website Relaunch,QA,Website Relaunch,Testing
Website Relaunch,,Website Relaunch,General
```

These rows map a renamed project, a renamed task, and a default task for untasked entries. Optional `Clockify_Project_ID`, `Clockify_Task_ID`, `Solidtime_Project_ID`, and `Solidtime_Task_ID` columns disambiguate names; IDs take precedence.

Missing, ambiguous, out-of-project, and conflicting rows stop the command. Canonical rules are in [UC-002](docs/use_cases/UC-002-compare-project-setup.md) and [UC-003](docs/use_cases/UC-003-migrate-time-tracking-data.md); see also the [sample CSV](docs/examples/project-task-mapping.csv).

## Advanced and Reference Documentation

- [Vision](docs/vision.md)
- [Requirements](docs/requirements.md)
- [Use-case diagram](docs/use_cases.puml)
- [Use-case specifications](docs/use_cases/)
- [Migration design and idempotency](docs/migration-design.md)
- [Contributing](CONTRIBUTING.md)

## License

MIT — see [`LICENSE`](LICENSE).
