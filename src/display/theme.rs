use anyhow::{Context, Result};
use ratatui::style::{palette::tailwind::SLATE, Color};
use serde::{Deserialize, Serialize};
use std::fs::{rename, File};
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

use crate::backend::config::get_config_dir;

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Theme {
    normal_row_bg: Color,
    alt_row_bg: Color,
    selected_style: Color,
}

impl Theme {
    pub fn default() -> Self {
        Self {
            normal_row_bg: SLATE.c950,
            alt_row_bg: SLATE.c900,
            selected_style: SLATE.c800,
        }
    }

    pub fn save(&self) -> Result<()> {
        match get_config_dir() {
            Ok(conf_local_dir) => {
                // For when we want to save the toml file
                // We can do this by creating a .tmp file and renaming it
                // This minimizes the chance of data being lost if an error
                // happens mid-write
                let toml_file = String::from("theme.toml");
                let tmp_file = format!("{}.tmp", toml_file);

                let toml_file_path = conf_local_dir.join(&toml_file);
                let tmp_file_path = conf_local_dir.join(&tmp_file);

                let toml_string =
                    toml::to_string(self).context("Had an issue serializing the toml file")?;

                // Create a .tmp file
                let mut file =
                    File::create(&tmp_file_path).context("Failed to make a .tmp file")?;
                file.write_all(toml_string.as_bytes())
                    .context("Failed to write theme toml file")?;

                // Rename .tmp file to old file
                rename(&tmp_file_path, &toml_file_path)
                    .with_context(|| { format!("Failed to update config file with rename:\ntmp_file: {:?}\nconfig_file:{:?}", tmp_file, toml_file)})?;
            }
            Err(e) => {
                println!("Failed getting the configuration location: {:?}", e);
                panic!()
            }
        }
        Ok(())
    }
}

pub fn get_toml_file() -> Result<PathBuf> {
    match get_config_dir() {
        Ok(local_config_dir) => {
            let toml_f = String::from("theme.toml");
            let toml_file_path = local_config_dir.join(&toml_f);

            Ok(toml_file_path)
        }
        Err(e) => {
            println!("Failed getting the theme toml at: {:?}", e);
            panic!()
        }
    }
}

pub fn read_theme() -> Result<Theme> {
    let toml_file_path = get_config_dir()?;
    let toml_file = std::fs::File::open(&toml_file_path)
        .with_context(|| format!("Failed to open {:?}", toml_file_path))?;
    let mut reader = BufReader::new(toml_file);

    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .context("Failed to read file contents to string")?;
    let theme: Theme =
        toml::from_str(&buf).context("Failed to parse toml string to Theme struct")?;

    Ok(theme)
}

#[cfg(test)]
mod tests {
    use ratatui::style::{palette::tailwind::SLATE, Color};
    use std::str::FromStr;
    use toml;

    use super::*;

    #[test]
    fn make_color_from_string() {
        let blue_color = Color::from_str("blue").unwrap();
        assert_eq!(blue_color, Color::Blue);
    }

    #[test]
    fn color_to_string() {
        let blue_color = Color::Blue;
        let blue_string = blue_color.to_string();
        assert_eq!(blue_string, String::from("Blue"));

        let dark_slate = SLATE.c950;
        let dark_slate_string = dark_slate.to_string();
        assert_eq!(dark_slate_string, "#020617");
    }

    #[test]
    fn read_from_toml() {
        let theme: Theme = toml::from_str(
            r#"
        normal_row_bg = '#020617'
        alt_row_bg = '#020600'
        selected_style = '#020650'
        "#,
        )
        .unwrap();
        println!("{:?}", theme);
    }
}
