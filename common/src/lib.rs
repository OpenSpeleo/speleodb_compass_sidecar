use once_cell::sync::Lazy;
use std::path::PathBuf;

/// Name of the hidden application directory inside the user's home directory.
pub const SPELEODB_COMPASS_DIR_NAME: &str = ".speleodb_compass";

/// Lazily-initialized full path to the application directory (home + SPELEODB_COMPASS_DIR_NAME).
///
/// This is a runtime-initialized static because the user's home directory is not known at compile time.
pub static SDB_USER_DIR: Lazy<PathBuf> = Lazy::new(|| {
    match dirs::home_dir() {
        Some(mut p) => {
            p.push(SPELEODB_COMPASS_DIR_NAME);
            p
        }
        None => PathBuf::from(SPELEODB_COMPASS_DIR_NAME),
    }
});

/// Return a clone of the computed application directory path.
pub fn app_dir_path() -> PathBuf {
    SDB_USER_DIR.clone()
}

/// Ensure the application directory exists, creating it if necessary.
pub fn ensure_app_dir_exists() -> std::io::Result<()> {
    let p = SDB_USER_DIR.as_path();
    std::fs::create_dir_all(p)
}

/// Initialize a file logger that writes logs into the SDB user directory.
///
/// The logger writes formatted records that include timestamp and log level. The log file
/// filename will be created inside the `SDB_USER_DIR` directory and flexi_logger will
/// add a timestamp to the filename by default.
///
/// `level` is a string like "info", "debug", etc. If initialization fails, the error is returned.
pub fn init_file_logger(level: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Make sure the directory exists.
    std::fs::create_dir_all(&*SDB_USER_DIR)?;

    use flexi_logger::{FileSpec, Logger};

    // Configure the logger to write to a single file (append mode) inside SDB_USER_DIR.
    // We choose a fixed basename so logs are aggregated into one file across restarts.
    let file_spec = FileSpec::default()
        .directory(SDB_USER_DIR.clone())
        .basename("speleodb_compass")
        .suppress_timestamp();

    Logger::try_with_str(level)?
        .log_to_file(file_spec)
        .append()
        .format(flexi_logger::detailed_format)
        .start()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn konst_and_path() {
        let p = app_dir_path();
        assert!(p.ends_with(SPELEODB_COMPASS_DIR_NAME));
    }
}
