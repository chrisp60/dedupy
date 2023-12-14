#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::env::current_dir;

#[allow(unused)]
use tracing::{debug, error, info, instrument, trace, warn, Level};
use tracing_subscriber::EnvFilter;

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let mut args = std::env::args();
    trace!(?args);
    match args.nth(1) {
        Some(path) => {
            trace!(path);
            dedupy::Report::parse(path)?;
        }
        None => {
            let file_picker = rfd::FileDialog::new()
                .add_filter("csv", &["csv"])
                .set_directory(current_dir()?)
                .set_title("Select a transaction report")
                .pick_files();

            let Some(files) = file_picker else {
                tracing::info!("No files selected, exiting.");
                return Ok(());
            };
            for file in files {
                tracing::info!("Parsing {:?}", file.display());
                dedupy::Report::parse(file)?;
            }
        }
    }
    Ok(())
}
