# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-06

### Added

- `validate` - verify Clockify/Solidtime API credentials and configuration
- `compare` - read-only side-by-side comparison of Clockify vs Solidtime projects/tasks
- `migrate` - transfer clients, projects, tasks, time entries, and tags from Clockify to Solidtime, with `--dry-run` preview and `--ignore-archived` filtering
- Idempotent re-runs via fingerprint-based matching and a persisted migration state file
- Config via environment variables, `.env` file, or `--config config.toml`
