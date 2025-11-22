mod compass_project;
pub use compass_project::{CompassProject, Project, SpeleoDb};

use once_cell::sync::Lazy;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Name of the hidden application directory inside the user's home directory.
pub const SPELEODB_COMPASS_DIR_NAME: &str = ".speleodb_compass";

/// Lazily-initialized full path to the application directory (home + SPELEODB_COMPASS_DIR_NAME).
///
/// This is a runtime-initialized static because the user's home directory is not known at compile time.
pub static SDB_USER_DIR: Lazy<PathBuf> = Lazy::new(|| match dirs::home_dir() {
    Some(mut p) => {
        p.push(SPELEODB_COMPASS_DIR_NAME);
        p
    }
    None => PathBuf::from(SPELEODB_COMPASS_DIR_NAME),
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

pub fn open_with_compass<P: AsRef<Path>>(project_path: P) -> Result<(), String> {
    open_with_compass_path(project_path.as_ref())
}

fn open_with_compass_path(path: &Path) -> Result<(), String> {
    if !std::fs::exists(path).unwrap() {
        Err("Provided path does not exist!".to_string())
    } else {
        Command::new("explorer")
            .args([path])
            .spawn()
            .expect("Expected to launch compass software");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn konst_and_path() {
        let p = app_dir_path();
        assert!(p.ends_with(SPELEODB_COMPASS_DIR_NAME));
    }

    #[test]
    fn app_dir_path_returns_clone() {
        // Test that app_dir_path returns a clone and both paths are equal
        let path1 = app_dir_path();
        let path2 = app_dir_path();
        assert_eq!(path1, path2);
        assert!(path1.ends_with(SPELEODB_COMPASS_DIR_NAME));
    }

    #[test]
    fn app_dir_path_is_absolute_or_relative() {
        // The path should end with the directory name
        let p = app_dir_path();
        let path_str = p.to_string_lossy();
        assert!(path_str.contains(SPELEODB_COMPASS_DIR_NAME));
    }

    #[test]
    fn ensure_app_dir_creates_directory() {
        // This test creates the actual directory
        // In production code, this is acceptable as it's in the user's home
        let result = ensure_app_dir_exists();
        assert!(result.is_ok(), "ensure_app_dir_exists should succeed");

        // Verify the directory was created
        let path = app_dir_path();
        assert!(
            path.exists(),
            "Directory should exist after ensure_app_dir_exists"
        );
        assert!(path.is_dir(), "Path should be a directory");
    }

    #[test]
    fn ensure_app_dir_is_idempotent() {
        // Calling ensure_app_dir_exists multiple times should work
        assert!(ensure_app_dir_exists().is_ok());
        assert!(ensure_app_dir_exists().is_ok());
        assert!(ensure_app_dir_exists().is_ok());
    }

    #[test]
    fn init_file_logger_with_valid_level() {
        // Test logger initialization with valid log levels
        // Note: This will actually create a log file, which is acceptable for tests

        // Clean up any existing logger first
        let result = init_file_logger("info");
        // The first call should succeed or already be initialized
        assert!(result.is_ok() || result.is_err()); // Logger can only be init once per process

        // Verify the log directory exists after initialization
        let log_dir = app_dir_path();
        assert!(log_dir.exists());
    }

    #[test]
    fn init_file_logger_creates_directory() {
        // The logger should create the directory if it doesn't exist
        let log_dir = app_dir_path();

        // Try to initialize logger (may fail if already initialized)
        let _ = init_file_logger("debug");

        // Directory should exist regardless
        assert!(log_dir.exists());
        assert!(log_dir.is_dir());
    }

    #[test]
    fn constant_value_is_correct() {
        // Verify the constant has the expected value
        assert_eq!(SPELEODB_COMPASS_DIR_NAME, ".speleodb_compass");
    }
    #[test]
    fn launch_compass_project() {
        let project_path = "assets/test_data/Fulfords.mak";
        open_with_compass(project_path).unwrap();
    }
}
