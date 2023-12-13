#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    match std::env::args().nth(1) {
        Some(path) => dedupy::Report::parse(path)?,
        None => {
            let file_picker = rfd::FileDialog::new()
                .add_filter("csv", &["csv"])
                .set_directory(std::env::current_dir()?)
                .set_title("Select a transaction report")
                .pick_files();

            match file_picker {
                Some(files) => files.into_iter().try_for_each(dedupy::Report::parse),
                _ => {
                    info!("No files selected, exiting.");
                    return Ok(());
                }
            }?
        }
    }
    Ok(())
}
