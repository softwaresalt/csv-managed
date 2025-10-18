use std::{env, process::Command};

use anyhow::{Context, Result, anyhow};
use log::info;

use crate::cli::InstallArgs;

pub fn execute(args: &InstallArgs) -> Result<()> {
    info!("Installing csv-managed via cargo install");
    let shim = env::var_os("CSV_MANAGED_CARGO_SHIM");
    let mut command = match shim {
        Some(path) if !path.is_empty() => Command::new(path),
        _ => Command::new("cargo"),
    };
    if let Ok(extra) = env::var("CSV_MANAGED_CARGO_SHIM_ARGS") {
        for arg in extra.split('\n').filter(|segment| !segment.is_empty()) {
            command.arg(arg);
        }
    }
    command.arg("install").arg("csv-managed");

    if let Some(version) = &args.version {
        command.arg("--version").arg(version);
    }
    if args.force {
        command.arg("--force");
    }
    if args.locked {
        command.arg("--locked");
    }
    if let Some(root) = &args.root {
        command.arg("--root").arg(root);
    }

    let status = command
        .status()
        .with_context(|| "Failed to spawn `cargo install` command")?;
    if !status.success() {
        return Err(anyhow!(
            "`cargo install csv-managed` exited with status {status}"
        ));
    }
    info!("csv-managed installed successfully");
    Ok(())
}
