# Unreleased

# v0.1.2

* `text_colors` has been added to the `theme.toml` to give users more customization options
* Some more styles can be changed under `theme_styles` in the `theme.toml`
* Updated README with details on what can be customized in `theme.toml`

# v0.1.1

* `checklist` is now much more robust in reading in a `theme.toml` file, in preparation for any additional theme elements (color vs style) or theme attributes that could be added in future releases
* Fixed the Help Menu text that was overlapping the bottom border
* Pop-ups should now be more consistent across different terminal sizes/layouts now
* Downgraded `rusqlite` to v0.31.0 (from v0.32.1) so that `cargo install checklist-tui` work for rust versions earlier than 1.77

