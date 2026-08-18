# v0.1.7

Focused on robustness and removing unnecessary work in the event loop.

## Build

- Fixed a typo in `Cargo.toml`: the table was `[profiler.release]` instead of
  `[profile.release]`. Cargo silently ignores unknown tables, so `lto` and
  `codegen-units` were never being applied to release builds. They now are.

## Performance

- Config is no longer serialized and rewritten to disk on every keypress. The
  previous behavior ran `serde_json::to_string` + an atomic temp-file rename on
  every event loop iteration, even though only the sort (`s`) and filter (`f`)
  toggles ever mutate persisted config. A `config_dirty` flag is now set at those
  two mutation sites, and the save runs once on exit, guarded by the flag. A
  transient disk error on save now propagates instead of panicking via `unwrap`.

## Resilience

- `get_all_db_contents` no longer panics on a single malformed row. It
  previously chained `.unwrap()` through `prepare`, `query_map`, the row
  closure, and the final loop — so one bad row (a NULL in a NOT NULL column,
  a bad `FromSql` value) would crash the app, and since the load runs on most
  keypresses, one bad row made `checklist` unopenable. Now:
  - `prepare`/`query_map` errors propagate with context.
  - The row closure uses `?`, so a structurally bad row becomes a skipped
    item logged to stderr instead of a panic.
  - Empty tag segments from `;;` or a trailing `;` are filtered out.
- `From<&str>` for `Urgency` and `Status` no longer `panic!()` on unrecognized
  strings. They `eprintln!` a warning and fall back to the default variant
  (`Low` / `Open`), so a typo'd value in the DB degrades gracefully instead of
  crashing. Matching is now case-insensitive as well.
- `update_tasklist` propagates load errors via `?` instead of `.unwrap()`, so a
  fatal DB error surfaces cleanly rather than as a raw `rusqlite::Error` panic.

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
