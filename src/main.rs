use clap::Parser;
use lazy_static::lazy_static;
use regex::bytes::Regex;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// A yuzu wrapper to run rust homebrew
///
/// Extracts program output from yuzu logs and allows you to enable the gdb stub
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[clap(value_parser)]
    program_path: PathBuf,

    /// Use this yuzu-cmd instead of one from PATH
    #[clap(long, value_parser)]
    yuzu_cmd_path: Option<PathBuf>,

    /// Enable gdbstub and use this port
    #[clap(long, value_parser)]
    gdbstub_port: Option<u32>,

    /// Write all yuzu logs to this location (discarded by default)
    #[clap(long, value_parser)]
    log_path: Option<PathBuf>,

    /// Override log_filter used in yuzu config
    #[clap(long, value_parser, default_value = "*:Debug")]
    log_filter: String,
}

lazy_static! {
    static ref LOG_LINE_REGEX: Regex =
        Regex::new(r"^(\x1B\x5B[0-9;]{1,}[A-Za-z])*\[ *(?P<time>[0-9.]+)\] (?P<channel>[a-zA-Z.]+) <(?P<level>[a-zA-Z]+)> (?P<file>[^:]+):(?P<function>[^:]+):(?P<line>[0-9]+): (?P<contents>.*)\n$")
            .unwrap();
}

fn main() {
    let args: Args = Args::parse();

    let yuzu_path = args
        .yuzu_cmd_path
        .or_else(|| which::which("yuzu-cmd").ok())
        .expect("Could not locate yuzu-cmd executable");

    let config_dir =
        tempdir::TempDir::new("yuzu-cmd-config").expect("Creating temp directory for config");

    let config_path = config_dir.path().join("yuzu.ini");

    let config = {
        let mut config = String::new();
        let log_filter = args.log_filter;

        write!(
            config,
            r"
[Miscellaneous]
log_filter={log_filter}
"
        )
        .unwrap();

        if let Some(gdbstub_port) = args.gdbstub_port {
            write!(
                config,
                r"            
[Debugging]
gdbstub_port={gdbstub_port}
use_gdbstub=true
"
            )
            .unwrap();
        }

        config
    };

    std::fs::write(&config_path, config).expect("Writing config");

    let child = Command::new(yuzu_path)
        .arg("-c")
        .arg(config_path)
        .arg(args.program_path)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut log_file = args
        .log_path
        .map(|path| File::create(path).expect("Could not open log file"));

    let stdout = child.stderr.unwrap();
    let mut stdout = BufReader::new(stdout);

    let mut is_in_debug = false;
    loop {
        let mut buffer = Vec::new();
        if stdout.read_until(b'\n', &mut buffer).unwrap() == 0 {
            break;
        }

        if let Some(log_file) = log_file.as_mut() {
            log_file.write_all(&buffer).expect("Writing the log file");
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
}
