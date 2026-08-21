# Contributing

Contributions are welcome through [GitHub issues and pull requests](https://github.com/nikbucher/clockify-to-solidtime).

## Development Setup

Install [Rust with rustup](https://www.rust-lang.org/tools/install). The project uses Rust edition 2024 and expects a current stable toolchain with `rustfmt` and `clippy`:

```sh
rustup toolchain install stable --component rustfmt --component clippy
git clone https://github.com/nikbucher/clockify-to-solidtime.git
cd clockify-to-solidtime
cargo build
cargo test
```

Offline smoke tests do not require credentials or API access:

```sh
cargo run -- --help
cargo run -- completions bash > /dev/null
```

Commands other than `completions` contact Clockify or Solidtime after loading configuration. Use test credentials and non-production organizations when manually exercising them.

## Project Orientation

- `src/main.rs` defines the CLI and dispatches subcommands.
- `src/config.rs` loads configuration.
- `src/validate.rs`, `src/compare.rs`, `src/migrate.rs`, and `src/completions.rs` implement the use cases.
- `src/clockify.rs` and `src/solidtime.rs` are the service clients.
- `src/mapping.rs` and `src/project_mapping.rs` handle persistent state and explicit mappings.
- `docs/requirements.md` is the requirements catalog.
- `docs/use_cases.puml` and `docs/use_cases/` define canonical behavior.
- `docs/migration-design.md` records implementation and idempotency decisions.

This project follows the [AI Unified Process](https://aiup.dev). Before changing behavior, review `docs/requirements.md` and the relevant use-case specification. Update those artifacts when the intended behavior changes. Read `docs/entity_model.md` before data-access changes if that artifact exists.

## Checks

Run the same commands as CI before submitting:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Follow `.editorconfig` and [rustfmt](https://github.com/rust-lang/rustfmt). Keep documentation and examples consistent with the CLI help and use-case specifications.

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>[optional scope]: <description>
```

Common types are `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, and `build`.
