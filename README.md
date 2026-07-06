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

## Workflow

1. Configure your Clockify and Solidtime API keys.
2. Run a dry-run to preview what will be migrated.
3. Execute the migration and review the summary.

## Usage

```sh
export CLOCKIFY_API_KEY="..."
export SOLIDTIME_API_TOKEN="..."

cargo run -- validate
cargo run -- compare --ignore-archived
cargo run -- migrate --dry-run --from 2024-01-01T00:00:00Z --to 2024-02-01T00:00:00Z
cargo run -- migrate --from 2024-01-01T00:00:00Z --to 2024-02-01T00:00:00Z
cargo run -- migrate --ignore-archived --from 2024-01-01T00:00:00Z --to 2024-02-01T00:00:00Z
```

### Validate

```sh
cargo run -- validate
cargo run -- validate --config config.toml
```

Checks that required configuration is present and that the selected Clockify workspace and Solidtime organization are reachable.

### Compare

```sh
cargo run -- compare
cargo run -- compare --ignore-archived
cargo run -- compare --config config.toml --mapping project-mapping.csv
```

Shows a read-only, side-by-side comparison of Clockify and Solidtime projects and tasks. It accepts `--config`, `--mapping`, and `--ignore-archived`. See [`docs/use_cases/UC-002-compare-project-setup.md`](docs/use_cases/UC-002-compare-project-setup.md) for the full output format.

### Migrate

```sh
cargo run -- migrate --dry-run --from 2024-01-01T00:00:00Z --to 2024-02-01T00:00:00Z
cargo run -- migrate --from 2024-01-01T00:00:00Z --to 2024-02-01T00:00:00Z
```

Runs the Clockify to Solidtime migration. Use `--dry-run` before a real migration to preview planned changes.

Optional environment variables:

- `CLOCKIFY_WORKSPACE_ID` - overrides the user's default Clockify workspace.
- `SOLIDTIME_ORGANIZATION_ID` - required when the Solidtime token has more than one membership.
- `CLOCKIFY_BASE_URL` and `SOLIDTIME_BASE_URL` - override API base URLs.

Alternatively, put the variables in a `.env` file in the project directory. Use `.env.example` as a template:

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
