# v0.1.7

> NOTE: In this version AI assistance is used, notably with inspecting the application and resolving more pressing bugs.

Focused on robustness and removing unnecessary work in the event loop.

## Build

- Fixed a typo in `Cargo.toml`: the table was `[profiler.release]` instead of
  `[profile.release]`.

## Performance

- Config is no longer serialized and rewritten to disk on every keypress. Now the config is only saved on exit after actions that would have changed the config (i.e. sort (s) and filter (f)) are done.

## Resilience

- Broadly reduced usage of `.unwrap()`, in particular around the code involved with reading tasks from the database. Tasks that are not able to be read now are instead logged out to stderr instead of crashing the app.

# v0.1.6

Bumped ratatui and rusqlite to v0.30.0 and v0.40.0 respectively.

# v0.1.5

Bumped edition to 2024.

Added a variety of improvements to how text is handled for some of the inputs.

## Highlighting text

Can now highlight text via:

- Ctrl + a to highlight all text
- Shift + <= or Shift + => to highlight a single character at a time

Once highlighted, the text can either be deleted or a new character entered which replaces the highlighted text.

## Movement

Added the ability to quickly move to the beginning or end of the text with Ctrl + <= or Ctrl + =>

## Theme

Highlight background and foreground color can be changed in the theme via the highlight_color_bg and highlight_color_fg values respectively

# v0.1.4

- added the `checklist import <checklist_db>` command that will import those tasks into your current database.

# v0.1.3

- Added the following to the `Cargo.toml` under `[profile.release]`:
  - `lto = true`
  - `codegen-units = 1`

These are intended to improve performance of the app in release mode.

# v0.1.2

- `text_colors` has been added to the `theme.toml` to give users more customization options
- Some more styles can be changed under `theme_styles` in the `theme.toml`
- Updated README with details on what can be customized in `theme.toml`

# v0.1.1

- `checklist` is now much more robust in reading in a `theme.toml` file, in preparation for any additional theme elements (color vs style) or theme attributes that could be added in future releases
- Fixed the Help Menu text that was overlapping the bottom border
- Pop-ups should now be more consistent across different terminal sizes/layouts now
- Downgraded `rusqlite` to v0.31.0 (from v0.32.1) so that `cargo install checklist-tui` work for rust versions earlier than 1.77
