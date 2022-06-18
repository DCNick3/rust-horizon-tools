use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    pub yuzu: Yuzu,
    pub gdb: Gdb,
    pub build: Build,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let default_config = Config::default();
        let default_config =
            serde_yaml::to_string(&default_config).expect("Serializing default config");

        // actually load config from somewhere
        let config = config::Config::builder()
            .add_source(config::File::from_str(
                &default_config,
                config::FileFormat::Yaml,
            ))
            .add_source(config::Environment::with_prefix("cargo_horizon"))
            .build()
            .context("Building the config file")?;

        config
            .try_deserialize()
            .context("Deserializing config structure")
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Build {
    /// rustup toolchain name to use to build the code
    pub toolchain: String,
    pub target: String,
    pub linker_script: Option<String>,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            toolchain: "horizon-stage1".to_string(),
            target: "aarch64-nintendo-switch-homebrew".to_string(),
            linker_script: Some("aarch64_nintendo_switch_homebrew_linker_script.ld".to_string()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Yuzu {
    /// Alternative yuzu-cmd executable location (inferred from PATH otherwise)
    pub yuzu_cmd_path: Option<PathBuf>,

    /// Default port for gdbstub
    pub gdbstub_port: u16,

    /// Override log_filter used in yuzu config
    /// Be sure to include Debug level for Debug_Emulated channel to get the program output
    pub log_filter: String,
}

impl Default for Yuzu {
    fn default() -> Self {
        Self {
            yuzu_cmd_path: None,
            gdbstub_port: 6543,
            log_filter: "*:Debug".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Gdb {
    /// Alternative gdb executable location (inferred from PATH otherwise)
    pub gdb_location: Option<PathBuf>,

    /// A list of commands to run at gdb start
    pub gdbinit_commands: Vec<String>,

    /// Location of rust gdb pretty printers
    pub rust_pretty_printers_dir: Option<PathBuf>,
}

#[allow(clippy::derivable_impls)]
impl Default for Gdb {
    fn default() -> Self {
        Self {
            gdb_location: None,
            gdbinit_commands: vec![],
            rust_pretty_printers_dir: None,
        }
    }
}
