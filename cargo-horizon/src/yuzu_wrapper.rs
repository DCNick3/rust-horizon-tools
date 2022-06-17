use crate::config;
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use regex::bytes::Regex;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

lazy_static! {
    static ref LOG_LINE_REGEX: Regex =
        Regex::new(r"^(\x1B\x5B[0-9;]{1,}[A-Za-z])*\[ *(?P<time>[0-9.]+)\] (?P<channel>[a-zA-Z.]+) <(?P<level>[a-zA-Z]+)> (?P<file>[^:]+):(?P<function>[^:]+):(?P<line>[0-9]+): (?P<contents>.*)\n$")
            .unwrap();
}

pub fn main(
    program_path: PathBuf,
    enable_gdbstub: bool,
    log_path: Option<&Path>,
    config: &config::Yuzu,
) -> Result<()> {
    let yuzu_path = config
        .yuzu_cmd_path
        .as_ref()
        .cloned()
        .or_else(|| which::which("yuzu-cmd").ok())
        .context("Could not locate yuzu-cmd executable")?;

    let config_dir = tempdir::TempDir::new("yuzu-cmd-config")
        .context("Creating temp directory for yuzu config")?;

    let config_path = config_dir.path().join("yuzu.ini");

    let yuzu_config = {
        let log_filter = &config.log_filter;
        let mut yuzu_config = String::new();

        write!(
            yuzu_config,
            r"
[Miscellaneous]
log_filter={log_filter}
"
        )
        .unwrap();

        if enable_gdbstub {
            let gdbstub_port = config.gdbstub_port;
            write!(
                yuzu_config,
                r"            
[Debugging]
gdbstub_port={gdbstub_port}
use_gdbstub=true
"
            )
            .unwrap();
        }

        yuzu_config
    };

    std::fs::write(&config_path, yuzu_config).context("Writing yuzu config")?;

    let child = Command::new(yuzu_path)
        .arg("-c")
        .arg(config_path)
        .arg(program_path)
        .stderr(Stdio::piped())
        .spawn()
        .context("Spawning yuzu-cmd")?;

    let mut log_file = log_path
        .map(|path| File::create(path).context("Could not open log file"))
        .map_or(Ok(None), |v| v.map(Some))? /*Option<Result> -> Result<Option> magic*/;

    let stdout = child.stderr.unwrap();
    let mut stdout = BufReader::new(stdout);

    let mut is_in_debug = false;
    loop {
        let mut buffer = Vec::new();
        if stdout.read_until(b'\n', &mut buffer).unwrap() == 0 {
            break;
        }

        if let Some(log_file) = log_file.as_mut() {
            log_file
                .write_all(&buffer)
                .context("Writing the log file")?;
        }

        let m = LOG_LINE_REGEX.captures(&buffer);
        if let Some(m) = m {
            let content = m.name("contents").unwrap().as_bytes();
            let func = std::str::from_utf8(m.name("function").unwrap().as_bytes()).unwrap();

            if func == "OutputDebugString" {
                is_in_debug = true;

                let mut stdout = std::io::stdout().lock();

                stdout.write_all(content).unwrap();
            } else {
                is_in_debug = false;
            }
        } else if is_in_debug {
            let mut stdout = std::io::stdout().lock();

            let buffer = if !buffer.is_empty() {
                &buffer[..buffer.len() - 1]
            } else {
                &buffer
            };

            writeln!(stdout).unwrap();
            stdout.write_all(&buffer).unwrap();
        }
    }

    Ok(())
}
