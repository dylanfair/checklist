use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use std::fs::{rename, File};
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub db_path: PathBuf,
}

impl Config {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn save(&self) -> Result<()> {
        match get_config_dir() {
            Ok(conf_local_dir) => {
                // We want to update our config
                // We can do this by creating a .tmp file and renaming it
                // This minimizes the chance of data being lost if an error
                // happens mid-write

                let config_file = conf_local_dir.join("config.json");
                let tmp_file = conf_local_dir.join("config.json.tmp");

                let config_string =
                    serde_json::to_string(self).context("Failed to deserialize Config")?;

                // Create a .tmp file
                let mut file = File::create(&tmp_file).context("Failed to make a .tmp file")?;
                file.write_all(config_string.as_bytes())
                    .context("Failed to write to config file")?;

                // Rename .tmp file to old file
                rename(&tmp_file, &config_file)
                    .with_context(|| { format!("Failed to update config file with rename:\ntmp_file: {:?}\nconfig_file:{:?}", tmp_file, config_file)})?;
            }
            Err(e) => {
                println!("Failed getting the configuration location: {:?}", e);
                panic!()
            }
        }
        Ok(())
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    let base_directories =
        BaseDirs::new().expect("Could not find the user's local config directory.");

    let conf_local_dir = base_directories.config_local_dir().join("checklist");
    // Create our checklist folder in local directory if it doesn't exist
    if conf_local_dir.exists() == false {
        // Create a brand new config file
        std::fs::create_dir_all(&conf_local_dir).with_context(|| {
            format!("Failed to create the following path: {:?}", conf_local_dir)
        })?;
    }

    Ok(conf_local_dir)
}

pub fn read_config() -> Result<Config> {
    match get_config_dir() {
        Ok(local_config_dir) => {
            let config_file_path = local_config_dir.join("config.json");
            let config_file = std::fs::File::open(&config_file_path)
                .with_context(|| format!("Failed to open {:?}", config_file_path))?;
            let reader = BufReader::new(config_file);

            let config: Config = serde_json::from_reader(reader)?;

            Ok(config)
        }
        Err(e) => {
            println!("Failed getting the configuration location: {:?}", e);
            panic!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save_and_read_config(db_path: PathBuf) {
        let config = Config::new(db_path.clone());
        match config.save() {
            Ok(()) => {
                let base_directories = BaseDirs::new().expect("Should find");
                let config_file = base_directories
                    .config_local_dir()
                    .join("checklist/config.json");
                assert_eq!(config_file.exists(), true);
            }
            Err(_) => {
                panic!()
            }
        }

        match read_config() {
            Ok(config) => {
                assert_eq!(config.db_path, db_path);
            }
            Err(_) => {
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
        config.save()?;
        let read_in_config = read_config()?;
        assert_eq!(config.db_path, read_in_config.db_path);

        config.db_path = PathBuf::from("second_db_path.db");
        config.save()?;
        let second_read_in_config = read_config()?;
        assert_eq!(config.db_path, second_read_in_config.db_path);

        Ok(())
    }
}
