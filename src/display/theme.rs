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

// Default colors
fn slate_950() -> Color {
    SLATE.c950
}
fn slate_900() -> Color {
    SLATE.c900
}
fn slate_800() -> Color {
    SLATE.c800
}
fn emerald_950() -> Color {
    EMERALD.c950
}
fn blue_default() -> Color {
    Color::Blue
}
fn white_default() -> Color {
    Color::White
}

/// Struct holds all the color configurations for `checklist`
/// that the user can change
#[derive(Debug, Deserialize, Serialize)]
pub struct ThemeColors {
    #[serde(default = "slate_950")]
    pub normal_row_bg: Color,
    #[serde(default = "slate_900")]
    pub alt_row_bg: Color,
    #[serde(default = "slate_800")]
    pub selected_style: Color,
    #[serde(default = "emerald_950")]
    pub status_bar: Color,
    #[serde(default = "slate_950")]
    pub tasks_box_bg: Color,
    #[serde(default = "white_default")]
    pub tasks_box_outline: Color,
    #[serde(default = "white_default")]
    pub tasks_box_scrollbar: Color,
    #[serde(default = "slate_950")]
    pub tasks_info_box_bg: Color,
    #[serde(default = "white_default")]
    pub tasks_info_box_outline: Color,
    #[serde(default = "white_default")]
    pub tasks_info_box_scrollbar: Color,
    #[serde(default = "slate_950")]
    pub state_box_bg: Color,
    #[serde(default = "white_default")]
    pub state_box_outline: Color,
    #[serde(default = "white_default")]
    pub state_box_scrollbar: Color,
    #[serde(default = "slate_950")]
    pub help_menu_bg: Color,
    #[serde(default = "white_default")]
    pub help_menu_outline: Color,
    #[serde(default = "white_default")]
    pub help_menu_scrollbar: Color,
    #[serde(default = "slate_800")]
    pub pop_up_bg: Color,
    #[serde(default = "white_default")]
    pub pop_up_outline: Color,
    #[serde(default = "blue_default")]
    pub state_box_outline_during_tags_edit: Color,
}

// Default Theme styles
fn scroll_begin() -> Option<String> {
    Some(String::from("↑"))
}
fn scroll_end() -> Option<String> {
    Some(String::from("↓"))
}
fn scroll_thumb() -> Option<String> {
    Some(String::from("█"))
}
fn scroll_track() -> Option<String> {
    Some(String::from(""))
}
fn highlight_symbol() -> String {
    String::from(">")
}

/// Struct that holds different elements the user can style
#[derive(Debug, Deserialize, Serialize)]
pub struct ThemeStyles {
    #[serde(default = "scroll_begin")]
    pub scrollbar_begin: Option<String>,
    #[serde(default = "scroll_end")]
    pub scrollbar_end: Option<String>,
    #[serde(default = "scroll_thumb")]
    pub scrollbar_thumb: Option<String>,
    #[serde(default = "scroll_track")]
    pub scrollbar_track: Option<String>,
    #[serde(default = "highlight_symbol")]
    pub highlight_symbol: String,
}

/// Overall struct that holds `ThemeColors` and `ThemeStyles`
#[derive(Debug, Deserialize, Serialize)]
pub struct Theme {
    // Colors
    pub theme_colors: ThemeColors,
    // Styles
    pub theme_styles: ThemeStyles,
}

pub fn create_empty_theme_toml() -> Result<()> {
    let toml_file_path = get_toml_file()?;
    let mut file = File::create(&toml_file_path).with_context(|| {
        format!(
            "Could not create an empty theme.toml file at '{}'",
            toml_file_path.display()
        )
    })?;
    file.write_all(b"[theme_colors]\n[theme_styles]")
        .context("Could not write to newly created theme.toml")?;
    println!("Created a default theme.toml file");

    Ok(())
}

impl Theme {
    /// Saves the `Theme` to a theme.toml file.
    /// Save location is based on `directories::BaseDirs`.
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

/// Returns a `Result<PathBuf>` of the theme.toml file
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

/// Returns a `Result<Theme>` from the theme.toml file
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

    // Save in case elements are missing
    // i.e. if user updates checklist version with new
    // config options
    theme.save()?;

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
