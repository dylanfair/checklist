use std::fs::{rename, File};
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::backend::task::Display;

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

    /// Saves the `Config` to a config.json file.
    /// Save location is based on `directories::BaseDirs`.
    /// `testing` bool will save a test.config.json file instead.
    pub fn save(&self, testing: bool) -> Result<()> {
        match get_config_dir() {
            Ok(conf_local_dir) => {
                // We want to update our config
                // We can do this by creating a .tmp file and renaming it
                // This minimizes the chance of data being lost if an error
                // happens mid-write
                let mut config_file = String::from("config.json");
                if testing {
                    config_file = format!("test.{config_file}");
                }
                let tmp_file = format!("{config_file}.tmp");

                let config_file_path = conf_local_dir.join(&config_file);
                let tmp_file_path = conf_local_dir.join(&tmp_file);

                let config_string =
                    serde_json::to_string(self).context("Failed to deserialize Config")?;

                // Create a .tmp file
                let mut file =
                    File::create(&tmp_file_path).context("Failed to make a .tmp file")?;
                file.write_all(config_string.as_bytes())
                    .context("Failed to write to config file")?;

                // Rename .tmp file to old file
                rename(&tmp_file_path, &config_file_path)
                    .with_context(|| { format!("Failed to update config file with rename:\ntmp_file: {tmp_file:?}\nconfig_file:{config_file:?}")})?;
            }
            Err(e) => {
                println!("Failed getting the configuration location: {e:?}");
                panic!()
            }
        }
        Ok(())
    }
}

/// Gets the directory where all checklist files are saved.
/// This is based on `directories::BaseDirs`
pub fn get_config_dir() -> Result<PathBuf> {
    let base_directories =
        BaseDirs::new().expect("Could not find the user's local config directory.");

    let conf_local_dir = base_directories.config_local_dir().join("checklist");
    // Create our checklist folder in local directory if it doesn't exist
    if !conf_local_dir.exists() {
        // Create a brand new config file
        std::fs::create_dir_all(&conf_local_dir)
            .with_context(|| format!("Failed to create the following path: {conf_local_dir:?}"))?;
    }

    Ok(conf_local_dir)
}

/// Looks for where the config.json file should be,
/// and reads it in returning a `Result<Config>`
pub fn read_config(testing: bool) -> Result<Config> {
    match get_config_dir() {
        Ok(local_config_dir) => {
            let mut config_f = String::from("config.json");
            if testing {
                config_f = format!("test.{config_f}");
            }
            let config_file_path = local_config_dir.join(&config_f);

            let config_file = std::fs::File::open(&config_file_path)
                .with_context(|| format!("Failed to open {config_file_path:?}"))?;
            let reader = BufReader::new(config_file);

            let config: Config = serde_json::from_reader(reader)?;

            Ok(config)
        }
        Err(e) => {
            println!("Failed getting the configuration location: {e:?}");
            panic!()
        }
    }
}

/// Will set the SQLite database path in the configuration file to use
/// the `PathBuf` provided. If `testing` is true, will save to the test
/// configuration file instead.
pub fn set_new_path(path: PathBuf, testing: bool) -> Result<()> {
    if !path.exists() {
        panic!("A valid path that exists needs to be supplied")
    }
    let absolute_path = std::fs::canonicalize(&path).with_context(|| {
        format!(
            "Failed to create a canonical path from the following: {:?}",
            &path
        )
    })?;

    match read_config(testing) {
        Ok(mut config) => {
            config.db_path = absolute_path.clone();
            config.save(testing)?;
            println!("Updated db path to {absolute_path:?}");
        }
        Err(_) => {
            let config = Config::new(absolute_path.clone());
            config.save(testing)?;
            println!("Set db path to {absolute_path:?}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_and_read_config(db_path: PathBuf) {
        let config = Config::new(db_path.clone());
        match config.save(true) {
            Ok(()) => {
                let base_directories = BaseDirs::new().expect("Should find");
                let config_file = base_directories
                    .config_local_dir()
                    .join("checklist/test.config.json");
                assert!(config_file.exists());
            }
            Err(_) => {
                println!("Encounted an error saving the test config file");
                panic!()
            }
        }

        match read_config(true) {
            Ok(config) => {
                assert_eq!(config.db_path, db_path);
            }
            Err(_) => {
                println!("Encounted an error reading the test config file");
                panic!()
            }
        }
    }

    #[test]
    fn test_multiple_saves() {
        let db_path = PathBuf::from("db_path.db");
        save_and_read_config(db_path);

        let second_db_path = PathBuf::from("different_path.db");
        save_and_read_config(second_db_path);
    }

    #[test]
    fn test_updating_the_config() -> Result<()> {
        let mut config = Config::new(PathBuf::from("first_db_path.db"));
        config.save(true)?;
        let read_in_config = read_config(true)?;
        assert_eq!(config.db_path, read_in_config.db_path);

        config.db_path = PathBuf::from("second_db_path.db");
        config.save(true)?;
        let second_read_in_config = read_config(true)?;
        assert_eq!(config.db_path, second_read_in_config.db_path);

        Ok(())
    }
}
