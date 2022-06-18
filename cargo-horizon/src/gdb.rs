use crate::config;
use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus};

pub fn run_gdb(
    gdb_config: &config::Gdb,
    connect_port: u16,
    symbols_file: Option<&Path>,
) -> Result<ExitStatus> {
    let gdb_location = gdb_config
        .gdb_location
        .as_ref()
        .cloned()
        .or_else(|| which::which("gdb-multiarch").ok())
        .or_else(|| which::which("gdb").ok())
        .context("Could not locate gdb executable")?;

    let mut command = Command::new(gdb_location);

    // rust pretty printers
    if let Some(pretty_printers_directory) = &gdb_config.rust_pretty_printers_dir {
        let python_path = if let Some(mut python_path) = std::env::var_os("PYTHONPATH") {
            python_path.push(OsStr::new(":"));
            python_path.push(pretty_printers_directory);
            python_path
        } else {
            pretty_printers_directory.as_os_str().to_owned()
        };

        // add to pythonpath for imports to work
        command.env("PYTHONPATH", python_path);

        // > Add directory to the path to search for source files.
        // I think it's for gdb to automagically find the .py file from autoload section
        command.arg("-d");
        command.arg(pretty_printers_directory);

        // Pretty printers directory is safe, I gurantee it
        command.arg("-iex");
        command.arg(format!(
            "add-auto-load-safe-path {}",
            pretty_printers_directory.to_str().unwrap()
        ));
    } else {
        eprintln!("NOTICE: Pretty printers path not configured, running without them");
    }

    if let Some(symbols_file) = symbols_file {
        command.arg("-ex");
        command.arg(format!("file {}", symbols_file.to_str().unwrap()));
    } else {
        eprintln!("NOTICE: Symbols file not specified, loading w/o debug symbols");
    }

    command.arg("-ex");
    command.arg(format!("target remote localhost:{}", connect_port));

    for gdbinit_command in gdb_config.gdbinit_commands.iter() {
        command.arg("-ex");
        command.arg(gdbinit_command);
    }

    command.status().context("Executing gdb")
}
