use anyhow::{Context, Result};
use ratatui::style::{
    palette::tailwind::{EMERALD, SLATE},
    Color,
};
use serde::{Deserialize, Serialize};
use std::fs::{rename, File};
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

use crate::backend::config::get_config_dir;

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ThemeColors {
    pub normal_row_bg: Color,
    pub alt_row_bg: Color,
    pub selected_style: Color,
    pub status_bar: Color,
    pub tasks_box_bg: Color,
    pub tasks_box_outline: Color,
    pub tasks_box_scrollbar: Color,
    pub tasks_info_box_bg: Color,
    pub tasks_info_box_outline: Color,
    pub tasks_info_box_scrollbar: Color,
    pub state_box_bg: Color,
    pub state_box_outline: Color,
    pub state_box_scrollbar: Color,
    pub help_menu_bg: Color,
    pub help_menu_outline: Color,
    pub help_menu_scrollbar: Color,
    pub pop_up_bg: Color,
    pub pop_up_outline: Color,
}

impl ThemeColors {
    pub fn default() -> Self {
        Self {
            normal_row_bg: SLATE.c950,
            alt_row_bg: SLATE.c900,
            selected_style: SLATE.c800,
            status_bar: EMERALD.c950,
            tasks_box_bg: SLATE.c950,
            tasks_box_outline: Color::White,
            tasks_box_scrollbar: Color::White,
            tasks_info_box_bg: SLATE.c950,
            tasks_info_box_outline: Color::White,
            tasks_info_box_scrollbar: Color::White,
            state_box_bg: SLATE.c950,
            state_box_outline: Color::White,
            state_box_scrollbar: Color::White,
            help_menu_bg: SLATE.c950,
            help_menu_outline: Color::White,
            help_menu_scrollbar: Color::White,
            pop_up_bg: SLATE.c800,
            pop_up_outline: Color::Red,
        }
    }
}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ThemeStyles {
    pub scrollbar_begin: Option<String>,
    pub scrollbar_end: Option<String>,
    pub scrollbar_thumb: Option<String>,
    pub scrollbar_track: Option<String>,
    pub highlight_symbol: String,
}

impl ThemeStyles {
    pub fn default() -> Self {
        Self {
            scrollbar_begin: Some(String::from("↑")),
            scrollbar_end: Some(String::from("↓")),
            scrollbar_thumb: Some(String::from("▐")),
            scrollbar_track: Some(String::from("")),
            highlight_symbol: String::from(">"),
        }
    }
}

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Theme {
    // Colors
    pub theme_colors: ThemeColors,
    // Styles
    pub theme_styles: ThemeStyles,
}

impl Theme {
    pub fn default() -> Self {
        Self {
            theme_colors: ThemeColors::default(),
            theme_styles: ThemeStyles::default(),
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
    let toml_file_path = get_toml_file()?;
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
