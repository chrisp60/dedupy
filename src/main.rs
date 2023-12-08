use tracing_subscriber::EnvFilter;

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let mut args = std::env::args();
    match args.nth(1) {
        Some(path) => {
            dedupy::Report::parse(path)?;
        }
        None => {
            let picks = rfd::FileDialog::new().pick_files();
            let Some(files) = picks else {
                println!("No files selected, exiting.");
                return Ok(());
            };
            for file in files {
                println!("Parsing {:?}", file.display());
                dedupy::Report::parse(file)?;
            }
        }
    }
    Ok(())
}
