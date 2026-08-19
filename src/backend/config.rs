use std::fs::{File, rename};
use std::io::{BufReader, prelude::*};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::backend::database::create_sqlite_db_at;
use crate::backend::task::Display;

/// The directory where checklist stores its data files: the SQLite database
/// (by default), the `config.json`, and the `theme.toml`.
///
/// Resolved once at startup and threaded through the functions that read or
/// write those files. This replaces the previous approach of a hardcoded
/// global directory (via `directories::BaseDirs`) plus a `testing: bool`
/// switch over filenames, and lets tests point at an isolated tempdir instead
/// of sharing the real `~/.config/checklist/`.
///
/// Note: [`Config::db_path`] is the *configured* database path, which may have
/// been overridden by the user via `checklist init --set <PATH>`. The path
/// returned by [`ConfigDir::db_path`] is only the default location.
#[derive(Debug, Clone)]
pub struct ConfigDir(PathBuf);

impl ConfigDir {
    /// Resolve the default config directory from the OS and ensure it exists.
    pub fn resolve_default() -> Result<Self> {
        let base = BaseDirs::new().context("Could not find the user's local config directory.")?;
        let dir = base.config_local_dir().join("checklist");
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create the following path: {dir:?}"))?;
        }
        Ok(Self(dir))
    }

    /// Construct a `ConfigDir` pointing at an explicit path. Does not create
    /// the directory; intended for tests pointing at a `tempfile::tempdir()`,
    /// and for a future `--config-dir` override.
    #[allow(dead_code)]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// The underlying directory path.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Path to the `config.json` file within this directory.
    pub fn config_path(&self) -> PathBuf {
        self.0.join("config.json")
    }

    /// Path to the default `checklist.sqlite` database within this directory.
    pub fn db_path(&self) -> PathBuf {
        self.0.join("checklist.sqlite")
    }

    /// Path to the `theme.toml` file within this directory.
    pub fn theme_path(&self) -> PathBuf {
        self.0.join("theme.toml")
    }
}

/// Struct to hold information for the program between sessions
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub db_path: PathBuf,
    pub display_filter: Display,
    pub urgency_sort_desc: bool,
}

impl Config {
    /// Creates a new config, taking in the path of a SQLite database
    pub fn new(db_path: PathBuf) -> Self {
        let urgency_sort_desc = true;
        let display_filter = Display::All;

        Self {
            db_path,
            display_filter,
            urgency_sort_desc,
        }
    }

    /// Saves the `Config` to the `config.json` file inside `dir`.
    ///
    /// Writes to a `.tmp` file first and renames it into place, which
    /// minimizes the chance of data loss if an error happens mid-write.
    pub fn save(&self, dir: &ConfigDir) -> Result<()> {
        let config_file_path = dir.config_path();
        let tmp_file_path = dir.path().join("config.json.tmp");

        let config_string = serde_json::to_string(self).context("Failed to serialize Config")?;

        let mut file = File::create(&tmp_file_path).context("Failed to make a .tmp file")?;
        file.write_all(config_string.as_bytes())
            .context("Failed to write to config file")?;

        rename(&tmp_file_path, &config_file_path).with_context(|| {
            format!(
                "Failed to update config file with rename:\ntmp_file: {tmp_file_path:?}\nconfig_file: {config_file_path:?}"
            )
        })?;
        Ok(())
    }
}

/// Looks for the `config.json` file in `dir` and reads it in, returning a
/// `Result<Config>`.
pub fn read_config(dir: &ConfigDir) -> Result<Config> {
    let config_file_path = dir.config_path();
    let config_file = std::fs::File::open(&config_file_path)
        .with_context(|| format!("Failed to open {config_file_path:?}"))?;
    let reader = BufReader::new(config_file);

    let config: Config = serde_json::from_reader(reader)?;

    Ok(config)
}

/// Sets the SQLite database path in the configuration file in `dir` to use the
/// `PathBuf` provided. If no config exists yet, a fresh one is created.
pub fn set_new_path(path: PathBuf, dir: &ConfigDir) -> Result<()> {
    if !path.exists() {
        bail!("A valid path to either an existing directory or sqlite file needs to be supplied");
    } else if path.is_file() {
        // if existing path, check if a sqlite file
        if let Some(extension) = path.extension()
            && extension.eq_ignore_ascii_case("sqlite")
        {
            let absolute_path = std::fs::canonicalize(&path).with_context(|| {
                format!(
                    "Failed to create a canonical path from the following: {:?}",
                    path
                )
            })?;
            write_db_path(absolute_path, dir)?;
        } else {
            bail!("File provided needs to end with a .sqlite extension");
        }
    } else if path.is_dir() {
        // If existing directory, check if 'checklist.sqlite' is there
        // if there, use it
        // if not, make a new one
        let absolute_path = std::fs::canonicalize(&path).with_context(|| {
            format!(
                "Failed to create a canonical path from the following: {:?}",
                path
            )
        })?;

        let checklist_path = absolute_path.join("checklist.sqlite");
        if checklist_path.exists() {
            write_db_path(checklist_path, dir)?;
        } else {
            // Create the database at the user's path first, then point the
            // config at it — so config never references a not-yet-existing
            // file. `create_sqlite_db_at` only creates the DB + schema; it
            // does not touch config.json (unlike `create_sqlite_db`).
            create_sqlite_db_at(checklist_path.clone())?;
            write_db_path(checklist_path, dir)?;
        }
    } else {
        bail!("Path is neither a file nor a directory");
    }

    Ok(())
}

fn write_db_path(db_path: PathBuf, dir: &ConfigDir) -> Result<()> {
    match read_config(dir) {
        Ok(mut config) => {
            config.db_path = db_path.clone();
            config.save(dir)?;
            println!("Updated db path to {db_path:?}");
        }
        Err(_) => {
            let config = Config::new(db_path.clone());
            config.save(dir)?;
            println!("Set db path to {db_path:?}");
        }
    }
    Ok(())
}

/// Expand a leading '~' to the user's home directory.
/// Intended to be used as a value_parser within clap
/// to simplify backend path handling logic
pub fn expand_tilde(path: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let home = BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .ok_or("could not determine the user's home directory")?;
    let expanded = match path {
        "~" => home,
        p if p.starts_with("~/") => home.join(&p[2..]),
        p => PathBuf::from(p),
    };
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn save_and_read_config(db_path: PathBuf, dir: &ConfigDir) {
        let config = Config::new(db_path.clone());
        config
            .save(dir)
            .expect("saving the test config file should succeed");
        assert!(dir.config_path().exists());

        let read_in = read_config(dir).expect("reading the test config file should succeed");
        assert_eq!(read_in.db_path, db_path);
    }

    #[test]
    fn test_multiple_saves() {
        // Each test owns an isolated tempdir, so parallel test runs no longer
        // race on a shared real config file.
        let tmp = tempdir().expect("tempdir should be created");
        let dir = ConfigDir::new(tmp.path().to_path_buf());

        save_and_read_config(PathBuf::from("db_path.db"), &dir);
        save_and_read_config(PathBuf::from("different_path.db"), &dir);
    }

    #[test]
    fn test_updating_the_config() -> Result<()> {
        let tmp = tempdir()?;
        let dir = ConfigDir::new(tmp.path().to_path_buf());

        let mut config = Config::new(PathBuf::from("first_db_path.db"));
        config.save(&dir)?;
        let read_in_config = read_config(&dir)?;
        assert_eq!(config.db_path, read_in_config.db_path);

        config.db_path = PathBuf::from("second_db_path.db");
        config.save(&dir)?;
        let second_read_in_config = read_config(&dir)?;
        assert_eq!(config.db_path, second_read_in_config.db_path);

        Ok(())
    }

    #[test]
    fn expand_tilde_passthrough() {
        assert_eq!(
            expand_tilde("/etc/passwd").unwrap(),
            PathBuf::from("/etc/passwd")
        );
        assert_eq!(
            expand_tilde("relative/path").unwrap(),
            PathBuf::from("relative/path")
        );
    }

    #[test]
    fn expand_tilde_home() {
        let expanded = expand_tilde("~").unwrap();
        assert!(
            expanded.is_absolute(),
            "bare ~ should resolve to an absolute home path"
        );
        let expanded_sub = expand_tilde("~/foo").unwrap();
        assert!(expanded_sub.starts_with(expanded.as_path()));
        assert_eq!(expanded_sub.file_name(), Some(std::ffi::OsStr::new("foo")));
    }

    // --- set_new_path branch tests ---
    //
    // Each test uses its own tempdir as the checklist config dir, and a
    // separate tempdir/file for the path passed to `set_new_path`, so none
    // of them touch the real config dir or each other.

    fn config_db_path(dir: &ConfigDir) -> PathBuf {
        read_config(dir).expect("config should exist after set_new_path").db_path
    }

    #[test]
    fn set_new_path_existing_sqlite_file_points_config_at_it() -> Result<()> {
        let cfg_tmp = tempdir()?;
        let cfg_dir = ConfigDir::new(cfg_tmp.path().to_path_buf());

        // Set up an existing .sqlite file (with a valid task schema) to point at.
        let db_tmp = tempdir()?;
        let db_path = db_tmp.path().join("existing.sqlite");
        create_sqlite_db_at(db_path.clone())?;
        assert!(db_path.exists());

        set_new_path(db_path.clone(), &cfg_dir)?;

        let configured = config_db_path(&cfg_dir);
        assert_eq!(configured, std::fs::canonicalize(&db_path)?);
        Ok(())
    }

    #[test]
    fn set_new_path_non_sqlite_file_bails() -> Result<()> {
        let cfg_tmp = tempdir()?;
        let cfg_dir = ConfigDir::new(cfg_tmp.path().to_path_buf());

        // An existing file that doesn't end in .sqlite should be rejected.
        let db_tmp = tempdir()?;
        let not_db = db_tmp.path().join("not_a_db.txt");
        std::fs::write(&not_db, "hello")?;

        let result = set_new_path(not_db, &cfg_dir);
        assert!(result.is_err(), "non-.sqlite file should be rejected");
        assert!(!cfg_dir.config_path().exists(), "config should not have been written");
        Ok(())
    }

    #[test]
    fn set_new_path_existing_dir_without_sqlite_creates_db_and_points_config() -> Result<()> {
        let cfg_tmp = tempdir()?;
        let cfg_dir = ConfigDir::new(cfg_tmp.path().to_path_buf());

        // An existing directory with no checklist.sqlite yet.
        let target_tmp = tempdir()?;
        let expected_db = target_tmp.path().join("checklist.sqlite");
        assert!(!expected_db.exists());

        set_new_path(target_tmp.path().to_path_buf(), &cfg_dir)?;

        // The DB should have been created at <dir>/checklist.sqlite ...
        assert!(expected_db.exists(), "checklist.sqlite should have been created");
        // ... and config should point at it.
        let configured = config_db_path(&cfg_dir);
        assert_eq!(configured, std::fs::canonicalize(&expected_db)?);
        Ok(())
    }

    #[test]
    fn set_new_path_existing_dir_with_sqlite_points_config_at_it() -> Result<()> {
        let cfg_tmp = tempdir()?;
        let cfg_dir = ConfigDir::new(cfg_tmp.path().to_path_buf());

        // An existing directory that already contains a checklist.sqlite.
        let target_tmp = tempdir()?;
        let existing_db = target_tmp.path().join("checklist.sqlite");
        create_sqlite_db_at(existing_db.clone())?;

        set_new_path(target_tmp.path().to_path_buf(), &cfg_dir)?;

        let configured = config_db_path(&cfg_dir);
        assert_eq!(configured, std::fs::canonicalize(&existing_db)?);
        Ok(())
    }

    #[test]
    fn set_new_path_nonexistent_path_bails() -> Result<()> {
        let cfg_tmp = tempdir()?;
        let cfg_dir = ConfigDir::new(cfg_tmp.path().to_path_buf());

        let bogus = cfg_tmp.path().join("does-not-exist");
        let result = set_new_path(bogus, &cfg_dir);
        assert!(result.is_err(), "non-existent path should be rejected");
        assert!(!cfg_dir.config_path().exists(), "config should not have been written");
        Ok(())
    }
}
