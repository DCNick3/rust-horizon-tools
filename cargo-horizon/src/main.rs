use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use linkle::format::nxo::NxoFile;
use std::fs::File;
use std::path::{Path, PathBuf};
use tempdir::TempDir;

#[derive(Parser)]
#[clap(name = "cargo-horizon")]
#[clap(bin_name = "cargo")]
enum Args {
    /// Transform the specified ELF file to NRO and run it inside yuzu emulator
    /// Can be used a Cargo runner (see docs for probe-run: https://github.com/knurling-rs/probe-run)
    RunYuzu {
        #[clap(value_parser)]
        elf_path: PathBuf,
        #[clap(long, value_parser)]
        yuzu_log_path: Option<PathBuf>,
    },
    /// Transform the specified ELF file to NRO and run it inside yuzu emulator
    /// Can be used a Cargo runner (see docs for probe-run: https://github.com/knurling-rs/probe-run)
    GdbYuzu {
        #[clap(value_parser)]
        elf_path: PathBuf,
        #[clap(long, value_parser)]
        yuzu_log_path: Option<PathBuf>,
    },
    PrintConfig,
}

mod config;
mod gdb;
mod yuzu_wrapper;

fn convert_to_nro(elf_path: &Path) -> Result<(TempDir, PathBuf)> {
    let tempdir = tempdir::TempDir::new("cargo-horizon-nro")
        .context("Creating a temporary directory to put NRO in")?;

    let nro_path = tempdir.path().join("converted.nro");

    // ewww, this API is not nice
    let mut nxo = NxoFile::from_elf(elf_path.to_str().unwrap())
        .context("Parsing ELF before converting to NRO")?;

    let mut output = File::create(&nro_path).context("Creating NRO file")?;

    // TODO: probably want to support icon & nacp?
    // nah, not in run-yuzu subcommand =)
    // don't care about romfs for now... It's very platform-specific anyway
    // and rust has include_bytes!() which is decent (albeit may use a lot of ram)
    nxo.write_nro(&mut output, None, None, None)
        .context("Writing NRO")?;

    Ok((tempdir, nro_path))
}

fn run_yuzu(
    yuzu_config: &config::Yuzu,
    elf_path: &Path,
    yuzu_log_path: Option<&Path>,
) -> Result<()> {
    let (_temp_dir, nro_path) = convert_to_nro(elf_path).context("Converting ELF to NRO")?;

    yuzu_wrapper::main(nro_path, false, yuzu_log_path, yuzu_config)
        .context("Running yuzu with the converted NRO")?;

    Ok(())
}

fn gdb_yuzu(
    yuzu_config: &config::Yuzu,
    gdb_config: &config::Gdb,
    elf_path: &Path,
    yuzu_log_path: Option<&Path>,
) -> Result<()> {
    let (_temp_dir, nro_path) = convert_to_nro(elf_path).context("Converting ELF to NRO")?;

    let gdbstub_port = yuzu_config.gdbstub_port;

    crossbeam::scope(move |s| {
        let (send, recv) = crossbeam::channel::unbounded();

        let yuzu_send = send;
        let gdb_send = yuzu_send.clone();

        s.spawn(move |_| {
            yuzu_send
                .send(
                    yuzu_wrapper::main(nro_path, true, yuzu_log_path, yuzu_config)
                        .context("Running yuzu with the converted NRO"),
                )
                .unwrap();
        });
        s.spawn(move |_| {
            let res = gdb::run_gdb(gdb_config, gdbstub_port, Some(elf_path))
                .context("Running gdb to connect to yuzu")
                .and_then(|res| {
                    if res.success() {
                        Ok(())
                    } else {
                        Err(anyhow!("Gdb returned exit code {:?}", res.code()))
                    }
                });
            gdb_send.send(res).unwrap();
        });

        while let Ok(r) = recv.recv() {
            if let Err(e) = r {
                return Err(e);
            } else {
                // something exited, we should terminate the other end
                // TODO: how do we do that, exactly?
                // probably async....
            }
        }

        Ok(())
    })
    .unwrap()?;

    Ok(())
}

fn main() -> Result<()> {
    let config = Config::load().context("Loading config")?;

    let args: Args = Args::parse();

    match args {
        Args::RunYuzu {
            elf_path,
            yuzu_log_path,
        } => run_yuzu(&config.yuzu, elf_path.as_path(), yuzu_log_path.as_deref())
            .context("Executing run-yuzu subcommand")?,
        Args::GdbYuzu {
            elf_path,
            yuzu_log_path,
        } => gdb_yuzu(
            &config.yuzu,
            &config.gdb,
            elf_path.as_path(),
            yuzu_log_path.as_deref(),
        )
        .context("Executing gdb-yuzu subcommand")?,
        Args::PrintConfig => {
            println!(
                "{}",
                serde_yaml::to_string(&config).context("Serializing config")?
            );
        }
    }

    Ok(())
}
