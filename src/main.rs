mod api_envelope;
mod clockify;
mod compare;
mod completions;
mod config;
mod fingerprint;
mod http;
mod mapping;
mod migrate;
mod models;
mod project_mapping;
mod solidtime;
mod validate;

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Migrate Clockify data to Solidtime via their APIs")]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
	/// Validate Clockify and Solidtime configuration without changing any data.
	Validate {
		/// Optional TOML config file.
		#[arg(long)]
		config: Option<PathBuf>,
	},

	/// Compare Clockify and Solidtime projects and tasks without changing any data.
	Compare {
		/// Optional TOML config file.
		#[arg(long)]
		config: Option<PathBuf>,

		/// Optional CSV file that maps Clockify project/task names or IDs to existing Solidtime project/task names or IDs.
		#[arg(long)]
		mapping: Option<PathBuf>,

		/// Do not compare archived Clockify projects or their tasks.
		#[arg(long)]
		ignore_archived: bool,
	},

	/// Run the Clockify to Solidtime migration.
	Migrate {
		/// Read and reconcile, but do not write to Solidtime or the state file.
		#[arg(long)]
		dry_run: bool,

		/// Optional TOML config file.
		#[arg(long)]
		config: Option<PathBuf>,

		/// Persistent mapping store.
		#[arg(long, default_value = "migration-state.json")]
		state: PathBuf,

		/// Optional CSV file that maps Clockify project/task names or IDs to existing Solidtime project/task names or IDs.
		#[arg(long)]
		mapping: Option<PathBuf>,

		/// Do not create missing clients, projects, tasks, or tags during a real migration run.
		#[arg(long)]
		no_create_structure: bool,

		/// Do not migrate archived Clockify projects, their tasks, or their time entries.
		#[arg(long)]
		ignore_archived: bool,

		/// Inclusive migration start timestamp, RFC3339/ISO-8601.
		#[arg(long, value_parser = parse_datetime)]
		from: Option<DateTime<Utc>>,

		/// Exclusive migration end timestamp, RFC3339/ISO-8601.
		#[arg(long, value_parser = parse_datetime)]
		to: Option<DateTime<Utc>>,
	},

	/// Generate a shell completion script for the given shell.
	Completions {
		/// Target shell.
		#[arg(value_enum)]
		shell: clap_complete::Shell,
	},
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>> {
	if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
		return Ok(dt.with_timezone(&Utc));
	}
	let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
	Ok(date.and_hms_opt(0, 0, 0).expect("valid midnight").and_utc())
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	if let Commands::Completions { shell } = &cli.command {
		return completions::run(*shell);
	}

	match dotenvy::dotenv() {
		Ok(_) => {}
		Err(err) if err.not_found() => {}
		Err(err) => return Err(err).context("failed to load .env file"),
	}

	match cli.command {
		Commands::Validate { config } => validate::run(validate::Options { config_path: config }),
		Commands::Compare { config, mapping, ignore_archived } => compare::run(compare::Options {
			config_path: config,
			mapping_path: mapping,
			ignore_archived,
		}),
		Commands::Migrate {
			dry_run,
			config,
			state,
			mapping,
			no_create_structure,
			ignore_archived,
			from,
			to,
		} => migrate::run(migrate::Options {
			dry_run,
			config_path: config,
			state_path: state,
			mapping_path: mapping,
			create_structure: !no_create_structure,
			ignore_archived,
			from,
			to,
		}),
		Commands::Completions { .. } => unreachable!("handled before .env load"),
	}
}
