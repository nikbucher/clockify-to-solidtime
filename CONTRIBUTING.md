# Contributing

Contributions are welcome! Please open an issue or pull request on [GitHub](https://github.com/nikbucher/clockify-to-solidtime).

## Development Methodology

[AIUP](https://aiup.dev) - requirements-driven, iterative. See `docs/` for vision, requirements, and use case specs.

## Code Style

- Follow `.editorconfig` / [rustfmt](https://github.com/rust-lang/rustfmt) (`cargo fmt`)
- Run `cargo fmt --check` before committing
- Run `cargo clippy -- -D warnings` for lint checks

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/).

Format: `<type>[optional scope]: <description>`

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, `build`

## CI Checks

Make sure CI passes before submitting:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
