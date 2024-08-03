use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use std::fs::{rename, File};
use std::io::prelude::*;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    db_path: PathBuf,
}

impl Config {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

pub fn save_db_path(db_path: PathBuf) -> Result<()> {
    let config = Config::new(db_path);
    let config_string = serde_json::to_string(&config).context("Failed to deserialize Config")?;

    let base_directories =
        BaseDirs::new().expect("Could not find the user's local config directory.");
    let conf_local_dir = base_directories.config_local_dir();
    let full_config_file_path = conf_local_dir.join("checklist/config.json");
    println!("{:?}", full_config_file_path);

    if full_config_file_path.exists() == false {
        // Create a brand new config file
        let prefix = full_config_file_path.parent().unwrap();
        std::fs::create_dir_all(prefix)
            .with_context(|| format!("Failed to create the following path: {:?}", prefix))?;

        let mut file =
            File::create(full_config_file_path).context("Failed to make the 'config.json' file")?;

        file.write_all(config_string.as_bytes())
            .context("Failed to write to config file")?;
    } else {
        // We want to update our config
        // We can do this by creating a .tmp file and renaming it
        // This minimizes the chance of data being lost if an error
        // happens mid-write

        // Create a .tmp file
        let tmp_file = full_config_file_path.join(".tmp");
        let mut file = File::create(&tmp_file).context("Failed to make a .tmp file")?;
        file.write_all(config_string.as_bytes())
            .context("Failed to write to config file")?;

        // Rename .tmp file to old file
        rename(tmp_file, full_config_file_path).context("Failed to update config file")?;
    }
    Ok(())
}
