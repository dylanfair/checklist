use std::fs::{File, rename};
use std::io::prelude::*;

use anyhow::{Context, Result};
use ratatui::style::{
    Color,
    palette::tailwind::{EMERALD, SLATE},
};
use serde::{Deserialize, Serialize};
use struct_field_names_as_array::FieldNamesAsArray;

use crate::backend::config::ConfigDir;

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
fn cyan_default() -> Color {
    Color::Cyan
}
fn blue_default() -> Color {
    Color::Blue
}
fn yellow_default() -> Color {
    Color::Yellow
}
fn green_default() -> Color {
    Color::Green
}
fn white_default() -> Color {
    Color::White
}
fn magenta_default() -> Color {
    Color::Magenta
}
fn red_default() -> Color {
    Color::Red
}
fn black_default() -> Color {
    Color::Black
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
    #[serde(default = "cyan_default")]
    pub highlight_color_bg: Color,
    #[serde(default = "black_default")]
    pub highlight_color_fg: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            normal_row_bg: slate_950(),
            alt_row_bg: slate_900(),
            selected_style: slate_800(),
            status_bar: emerald_950(),
            tasks_box_bg: slate_950(),
            tasks_box_outline: white_default(),
            tasks_box_scrollbar: white_default(),
            tasks_info_box_bg: slate_950(),
            tasks_info_box_outline: white_default(),
            tasks_info_box_scrollbar: white_default(),
            state_box_bg: slate_950(),
            state_box_outline: white_default(),
            state_box_scrollbar: white_default(),
            help_menu_bg: slate_950(),
            help_menu_outline: white_default(),
            help_menu_scrollbar: white_default(),
            pop_up_bg: slate_800(),
            pop_up_outline: white_default(),
            state_box_outline_during_tags_edit: blue_default(),
            highlight_color_bg: cyan_default(),
            highlight_color_fg: black_default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ThemeText {
    #[serde(default = "cyan_default")]
    pub status_open: Color,
    #[serde(default = "blue_default")]
    pub status_working: Color,
    #[serde(default = "yellow_default")]
    pub status_paused: Color,
    #[serde(default = "green_default")]
    pub status_completed: Color,
    #[serde(default = "green_default")]
    pub urgency_low: Color,
    #[serde(default = "yellow_default")]
    pub urgency_medium: Color,
    #[serde(default = "magenta_default")]
    pub urgency_high: Color,
    #[serde(default = "red_default")]
    pub urgency_critical: Color,
    #[serde(default = "red_default")]
    pub urgency_ascending: Color,
    #[serde(default = "blue_default")]
    pub urgency_descending: Color,
    #[serde(default = "magenta_default")]
    pub title: Color,
    #[serde(default = "cyan_default")]
    pub created_date: Color,
    #[serde(default = "green_default")]
    pub completed_date: Color,
    #[serde(default = "blue_default")]
    pub latest: Color,
    #[serde(default = "magenta_default")]
    pub description: Color,
    #[serde(default = "blue_default")]
    pub tags: Color,
    #[serde(default = "yellow_default")]
    pub layout_smart: Color,
    #[serde(default = "cyan_default")]
    pub layout_horizontal: Color,
    #[serde(default = "blue_default")]
    pub layout_vertical: Color,
    #[serde(default = "cyan_default")]
    pub filter_status_all: Color,
    #[serde(default = "green_default")]
    pub filter_status_completed: Color,
    #[serde(default = "yellow_default")]
    pub filter_status_notcompleted: Color,
    #[serde(default = "blue_default")]
    pub help_actions: Color,
    #[serde(default = "magenta_default")]
    pub help_quick_actions: Color,
    #[serde(default = "yellow_default")]
    pub help_movement: Color,
}

impl Default for ThemeText {
    fn default() -> Self {
        Self {
            status_open: cyan_default(),
            status_working: blue_default(),
            status_paused: yellow_default(),
            status_completed: green_default(),
            urgency_low: green_default(),
            urgency_medium: yellow_default(),
            urgency_high: magenta_default(),
            urgency_critical: red_default(),
            urgency_ascending: red_default(),
            urgency_descending: blue_default(),
            title: magenta_default(),
            created_date: cyan_default(),
            completed_date: green_default(),
            latest: blue_default(),
            description: magenta_default(),
            tags: blue_default(),
            layout_smart: yellow_default(),
            layout_horizontal: cyan_default(),
            layout_vertical: blue_default(),
            filter_status_all: cyan_default(),
            filter_status_completed: green_default(),
            filter_status_notcompleted: yellow_default(),
            help_actions: blue_default(),
            help_quick_actions: magenta_default(),
            help_movement: yellow_default(),
        }
    }
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
fn urgency_low() -> String {
    String::from("   ")
}
fn urgency_medium() -> String {
    String::from("!  ")
}
fn urgency_high() -> String {
    String::from("!! ")
}
fn urgency_critical() -> String {
    String::from("!!!")
}
fn completed() -> String {
    String::from("✓  ")
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
    #[serde(default = "urgency_low")]
    pub urgency_low: String,
    #[serde(default = "urgency_medium")]
    pub urgency_medium: String,
    #[serde(default = "urgency_high")]
    pub urgency_high: String,
    #[serde(default = "urgency_critical")]
    pub urgency_critical: String,
    #[serde(default = "completed")]
    pub completed: String,
}

impl Default for ThemeStyles {
    fn default() -> Self {
        Self {
            scrollbar_begin: scroll_begin(),
            scrollbar_end: scroll_end(),
            scrollbar_thumb: scroll_thumb(),
            scrollbar_track: scroll_track(),
            highlight_symbol: highlight_symbol(),
            urgency_low: urgency_low(),
            urgency_medium: urgency_medium(),
            urgency_high: urgency_high(),
            urgency_critical: urgency_critical(),
            completed: completed(),
        }
    }
}

/// Overall struct that holds `ThemeColors` and `ThemeStyles`
#[derive(Debug, Deserialize, Serialize, FieldNamesAsArray, Default)]
pub struct Theme {
    // Colors
    #[serde(default)]
    pub theme_colors: ThemeColors,
    // Text Colors
    #[serde(default)]
    pub text_colors: ThemeText,
    // Styles
    #[serde(default)]
    pub theme_styles: ThemeStyles,
}

pub fn create_empty_theme_toml(dir: &ConfigDir) -> Result<()> {
    let toml_file_path = dir.theme_path();
    let mut file = File::create(&toml_file_path).with_context(|| {
        format!(
            "Could not create an empty theme.toml file at '{}'",
            toml_file_path.display()
        )
    })?;

    let theme_elements = Theme::FIELD_NAMES_AS_ARRAY;
    for element in theme_elements {
        file.write(format!("[{element}]\n").as_bytes())
            .context("Failed when writing theme elements to newly created theme.toml")?;
    }
    println!("Created a default theme.toml file");

    Ok(())
}

impl Theme {
    /// Saves the `Theme` to the `theme.toml` file inside `dir`.
    ///
    /// Writes to a `.tmp` file first and renames it into place, which
    /// minimizes the chance of data loss if an error happens mid-write.
    pub fn save(&self, dir: &ConfigDir) -> Result<()> {
        let toml_file_path = dir.theme_path();
        let tmp_file_path = dir.path().join("theme.toml.tmp");

        let toml_string =
            toml::to_string(self).context("Had an issue serializing the toml file")?;

        // Create a .tmp file
        let mut file = File::create(&tmp_file_path).context("Failed to make a .tmp file")?;
        file.write_all(toml_string.as_bytes())
            .context("Failed to write theme toml file")?;

        // Rename .tmp file into place
        rename(&tmp_file_path, &toml_file_path).with_context(|| {
            format!(
                "Failed to update theme file with rename:\ntmp_file: {tmp_file_path:?}\ntoml_file: {toml_file_path:?}"
            )
        })?;
        Ok(())
    }
}

/// Returns a `Result<Theme>` from the `theme.toml` file in `dir`.
///
/// This is a pure read — it never writes to disk. Missing tables or fields
/// fall back to their `#[serde(default)]` values, so older `theme.toml` files
/// (or ones missing newly added options) still load. User comments and
/// formatting in the file are preserved because nothing is re-serialized.
///
/// To re-serialize the file with all current keys and defaults (e.g. after an
/// update adds new theme options), use [`migrate_theme`] or `checklist theme --migrate`.
pub fn read_theme(dir: &ConfigDir) -> Result<Theme> {
    let toml_file_path = dir.theme_path();
    let buf = std::fs::read_to_string(&toml_file_path)
        .with_context(|| format!("Failed to read {toml_file_path:?}"))?;

    let theme: Theme =
        toml::from_str(&buf).context("Failed to parse toml string to Theme struct")?;

    Ok(theme)
}

/// Re-serializes `theme.toml` in `dir` with all current keys and defaults.
///
/// Reads the existing theme (filling any gaps with defaults), then writes it
/// back via [`Theme::save`]. This is the only path that rewrites the file, so
/// it is opt-in — note that it will **not** preserve user comments
/// since the file is regenerated from the parsed struct. Intended
/// for picking up newly available theme options after a checklist update.
pub fn migrate_theme(dir: &ConfigDir) -> Result<()> {
    let theme = read_theme(dir)?;
    theme.save(dir)?;
    println!("Re-serialized theme.toml with all current keys and defaults.");
    println!("Note: any comments or custom formatting were not preserved.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, palette::tailwind::SLATE};
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
        [theme_colors]
        normal_row_bg = '#020617'
        alt_row_bg = '#020600'
        selected_style = '#020650'
        [text_colors]
        status_open = 'cyan'
        status_working = 'cyan'
        [theme_styles]
        scrollbar_begin = 'grey'
        scrollbar_end = 'grey'
        "#,
        )
        .unwrap();
        println!("{theme:?}");
    }

    #[test]
    fn default_theme_matches_defaults_without_disk() {
        // Theme::default() should produce the same values a freshly generated
        // theme.toml would, without touching the filesystem.
        let theme = Theme::default();

        // A couple of representative defaults from each section.
        assert_eq!(theme.theme_colors.normal_row_bg, SLATE.c950);
        assert_eq!(theme.text_colors.status_open, Color::Cyan);
        assert_eq!(theme.theme_styles.scrollbar_thumb, Some(String::from("█")));
        assert_eq!(theme.theme_styles.urgency_critical, String::from("!!!"));
    }

    #[test]
    fn read_theme_fills_missing_tables_with_defaults() {
        // A theme.toml missing entire tables should still deserialize, with
        // serde defaults filling the gaps — no patch-up or rewrite needed.
        let tmp = tempfile::tempdir().unwrap();
        let dir = ConfigDir::new(tmp.path().to_path_buf());

        // Only one table, partially specified.
        std::fs::write(
            dir.theme_path(),
            "# a comment that must survive\n[text_colors]\nstatus_open = 'red'\n",
        )
        .unwrap();

        let theme = read_theme(&dir).unwrap();
        // Specified value is honored.
        assert_eq!(theme.text_colors.status_open, Color::Red);
        // Missing field within the present table uses its serde default.
        assert_eq!(theme.text_colors.status_completed, Color::Green);
        // Entirely missing tables fall back to defaults.
        assert_eq!(theme.theme_colors.normal_row_bg, SLATE.c950);
        assert_eq!(theme.theme_styles.scrollbar_thumb, Some(String::from("█")));

        // read_theme must not rewrite the file — the comment survives.
        let on_disk = std::fs::read_to_string(dir.theme_path()).unwrap();
        assert!(
            on_disk.contains("a comment that must survive"),
            "read_theme should not rewrite the file"
        );
    }
}
