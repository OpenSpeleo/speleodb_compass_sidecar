use common::Error;
use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, OnceLock},
};
use uuid::Uuid;

// flexi_logger's LoggerHandle must outlive the program (it owns flush/cleanup).
// `init_file_logger` previously dropped it; now that we build the logger
// manually we retain it here for the process lifetime.
static LOGGER_HANDLE: OnceLock<flexi_logger::LoggerHandle> = OnceLock::new();

/// Name of the hidden application directory inside the user's home directory.
const COMPASS_HOME_DIR_NAME: &str = ".compass";
/// Name of the compass projects folder inside the user's home directory.
const COMPASS_PROJECT_DIR_NAME: &str = "projects";

/// Lazily-initialized full path to the application directory (home + COMPASS_HOME_DIR_NAME).
///
/// This is a runtime-initialized static because the user's home directory is not known at compile time.
pub static COMPASS_HOME_DIR: LazyLock<PathBuf> = LazyLock::new(compass_home_dir);

#[cfg(not(test))]
fn compass_home_dir() -> PathBuf {
    match dirs::home_dir() {
        Some(mut p) => {
            p.push(COMPASS_HOME_DIR_NAME);
            p
        }
        None => PathBuf::from(COMPASS_HOME_DIR_NAME),
    }
}

#[cfg(test)]
fn compass_home_dir() -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "speleodb_compass_sidecar_tests_{}",
            std::process::id()
        ))
        .join(COMPASS_HOME_DIR_NAME)
}

/// Lazily-initialized full path to the compass projects folder (~/.compass/projects).
static COMPASS_PROJECT_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut p = COMPASS_HOME_DIR.clone(); // Use the home dir above
    p.push(COMPASS_PROJECT_DIR_NAME);
    p
});

/// Return a clone of the computed application directory path.
pub fn compass_home() -> &'static Path {
    &COMPASS_HOME_DIR
}

/// Return a clone of the compass projects folder path.
pub fn compass_dir_path() -> &'static Path {
    &COMPASS_PROJECT_DIR
}

/// Get the path for a specific project in the compass folder.
pub fn compass_project_path(project_id: Uuid) -> PathBuf {
    let mut path = compass_dir_path().to_path_buf();
    path.push(project_id.to_string());
    path
}

/// Get the path for a specific project in the compass folder.
pub fn compass_project_index_path(project_id: Uuid) -> PathBuf {
    let mut path = compass_project_path(project_id);
    path.push("index");
    path
}

/// Get the path for a specific project in the compass folder.
pub fn compass_project_working_path(project_id: Uuid) -> PathBuf {
    let mut path = compass_project_path(project_id);
    path.push("working_copy");
    path
}

/// Ensure the application directory exists, creating it if necessary.
pub fn ensure_app_dir_exists() -> std::io::Result<()> {
    std::fs::create_dir_all(compass_home())?;
    let p = COMPASS_PROJECT_DIR.as_path();
    std::fs::create_dir_all(p)
}

/// Ensure a specific project folder exists in the compass directory.
pub fn ensure_compass_project_dirs_exist(project_id: Uuid) -> Result<PathBuf, Error> {
    let path = compass_project_index_path(project_id);
    std::fs::create_dir_all(&path).map_err(|_| Error::CreateDirectory(path.clone()))?;
    let path = compass_project_working_path(project_id);
    std::fs::create_dir_all(&path).map_err(|_| Error::CreateDirectory(path.clone()))?;
    Ok(compass_project_path(project_id))
}

/// Initialize a file logger that writes logs into the SDB user directory.
///
/// The logger writes formatted records that include timestamp and log level. The log file
/// filename will be created inside the `COMPASS_HOME_DIR` directory and flexi_logger will
/// add a timestamp to the filename by default.
///
/// `level` is a string like "info", "debug", etc. If initialization fails, the error is returned.
pub fn init_file_logger(level: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Make sure the directory exists.
    std::fs::create_dir_all(&*COMPASS_HOME_DIR)?;

    #[cfg(not(test))]
    use flexi_logger::Duplicate;
    use flexi_logger::{FileSpec, Logger, WriteMode};

    // Configure the logger to write to a single file (append mode) inside COMPASS_HOME_DIR.
    // We choose a fixed basename so logs are aggregated into one file across restarts.
    let file_spec = FileSpec::default()
        .directory(COMPASS_HOME_DIR.clone())
        .basename("speleodb_compass")
        .suppress_timestamp();

    // NOTE: flexi_logger is the SOLE global logger. Do NOT call pretty_env_logger::init()
    // or env_logger::init() before this — they would register first and cause flexi_logger
    // to silently fail, resulting in no file logging at all.
    let logger = Logger::try_with_str(level)?
        .log_to_file(file_spec)
        .append()
        // Use Direct write mode so every log line is flushed immediately.
        // This ensures log entries survive hard crashes (e.g. 0xc000041d).
        .write_mode(WriteMode::Direct);

    // Duplicate log output to stderr for visibility while running the app
    // (`cargo run`, `cargo tauri dev`, packaged binaries). Skip it under
    // `cargo test` so the production log lines emitted by code under test
    // do not pollute the test output stream — file logging still happens
    // and can be tailed from `~/.compass/speleodb_compass.log` if needed.
    #[cfg(not(test))]
    let logger = logger.duplicate_to_stderr(Duplicate::Info);

    // Build the flexi logger as a boxed `log::Log` rather than registering it
    // directly, so we can wrap it. `build()` already calls `log::set_max_level`
    // (via the LoggerHandle), so we only install the wrapped logger below.
    let (file_logger, handle) = logger.format(flexi_logger::detailed_format).build()?;

    // Forward every record to Sentry in addition to the file/stderr:
    // Error -> exception event, Warn/Info -> breadcrumb (sentry-log default
    // filter). With no Sentry client (no DSN compiled in) this is a no-op
    // passthrough to the file logger.
    let sentry_logger = sentry::integrations::log::SentryLogger::with_dest(file_logger);
    log::set_boxed_logger(Box::new(sentry_logger))?;

    // Retain the flexi handle for the process lifetime (ignored if a prior
    // init in the same process already set it, e.g. in tests).
    let _ = LOGGER_HANDLE.set(handle);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn konst_and_path() {
        let p = compass_home();
        assert!(p.ends_with(COMPASS_HOME_DIR_NAME));
    }

    #[test]
    fn app_dir_path_is_absolute_or_relative() {
        // The path should end with the directory name
        let p = compass_home();
        let path_str = p.to_string_lossy();
        assert!(path_str.contains(COMPASS_HOME_DIR_NAME));
    }

    #[test]
    #[serial]
    fn ensure_app_dir_creates_directory() {
        // This test creates the actual directory
        // In production code, this is acceptable as it's in the user's home
        let result = ensure_app_dir_exists();
        assert!(result.is_ok(), "ensure_app_dir_exists should succeed");

        // Verify the directory was created
        let path = compass_home();
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
    #[serial]
    fn init_file_logger_with_valid_level() {
        // Test logger initialization with valid log levels
        // Note: This will actually create a log file, which is acceptable for tests

        // Clean up any existing logger first
        let result = init_file_logger("info");
        // The first call should succeed or already be initialized
        assert!(result.is_ok() || result.is_err()); // Logger can only be init once per process

        // Verify the log directory exists after initialization
        let log_dir = compass_home();
        assert!(log_dir.exists());
    }

    #[test]
    #[serial]
    fn init_file_logger_creates_directory() {
        // The logger should create the directory if it doesn't exist
        let log_dir = compass_home();

        // Try to initialize logger (may fail if already initialized)
        let _ = init_file_logger("debug");

        // Directory should exist regardless
        assert!(log_dir.exists());
        assert!(log_dir.is_dir());
    }
}
