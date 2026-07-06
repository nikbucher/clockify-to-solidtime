use std::io::{self, Write};

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};

pub fn run(shell: Shell) -> Result<()> {
	let mut cmd = crate::Cli::command();
	let bin_name = cmd.get_name().to_string();
	let stdout = io::stdout();
	let mut writer = BrokenPipeSafeWriter::new(stdout.lock());
	generate(shell, &mut cmd, bin_name, &mut writer);
	Ok(())
}

struct BrokenPipeSafeWriter<W> {
	inner: W,
	closed: bool,
}

impl<W> BrokenPipeSafeWriter<W> {
	fn new(inner: W) -> Self {
		Self { inner, closed: false }
	}
}

impl<W: Write> Write for BrokenPipeSafeWriter<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		if self.closed {
			return Ok(buf.len());
		}

		match self.inner.write(buf) {
			Ok(written) => Ok(written),
			Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {
				self.closed = true;
				Ok(buf.len())
			}
			Err(err) => Err(err),
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		if self.closed {
			return Ok(());
		}

		match self.inner.flush() {
			Ok(()) => Ok(()),
			Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {
				self.closed = true;
				Ok(())
			}
			Err(err) => Err(err),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn completions_generate_for_all_shells() {
		for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell, Shell::Elvish] {
			let mut cmd = crate::Cli::command();
			let mut buf: Vec<u8> = Vec::new();
			generate(shell, &mut cmd, "clockify-to-solidtime", &mut buf);
			assert!(!buf.is_empty(), "completion output empty for {shell:?}");
		}
	}

	#[test]
	fn broken_pipe_does_not_fail_completion_generation() {
		struct BrokenPipeWriter;

		impl Write for BrokenPipeWriter {
			fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
				Err(io::Error::from(io::ErrorKind::BrokenPipe))
			}

			fn flush(&mut self) -> io::Result<()> {
				Err(io::Error::from(io::ErrorKind::BrokenPipe))
			}
		}

		let mut writer = BrokenPipeSafeWriter::new(BrokenPipeWriter);
		assert!(writer.write_all(b"completion output").is_ok());
		assert!(writer.flush().is_ok());
	}
}
