# v0.1.9

> NOTE: In this version AI assistance is used.

## Enhancements

- Tags are now stored relationally (a `tag` table keyed to each task) instead of a single `;`-joined text column. Tags containing `;` or any other character are now stored and round-trip correctly.
- Database schema changes now ship as automatic migrations: on first launch after updating, pending migrations apply in order before the app opens. No manual steps are needed to upgrade an existing database.
- `checklist import` remains lossless when importing from older, pre-migration databases - their tags are read from the legacy format and written into the new one.
- Startup errors retrieving the database or config are now reported to stderr instead of failing silently, making issues like a stale `config.json` path easier to diagnose.
- Before a database upgrade applies, a snapshot of your database is kept in a `checklist-migration-snapshots/` folder next to it (e.g. `checklist-migration-snapshots/checklist.sqlite.pre-migration-v0.bak`). If an upgrade ever misbehaves, that file can be restored and used with the previous version of `checklist`. Snapshots are never overwritten by later attempts, and no backup is created when there is nothing to upgrade.
- Expose a `migrate` command to the user that provides an interface for migrating their database up and down versions. Intended use case for this is if a user needs to take a database with an advanced schema and move it back to be compatible with an older `checklist` binary.
- New `--config-dir <PATH>` flag and `CHECKLIST_CONFIG_DIR` env variable let you point checklist at a specific data directory instead of the default platform location. Precedence is flag > environment > default.
- `checklist wipe --hard` now drops every user table (including the new `tag` table) instead of only `task`, so the next launch rebuilds the schema from scratch.

## Internal Changes

- Schema migrations are powered by the `rusqlite_migration` crate. Migration SQL lives under `migrations/{id}-{name}/up.sql` (+ optional `down.sql`) and is embedded into the binary at compile time via `include_dir`, so installed binaries carry their own migrations. Applied versions are tracked in SQLite's built-in `user_version` pragma.
- Each migration runs inside a transaction together with its version stamp, so a crash mid-migration rolls back cleanly and retries on next launch. After migrating, a `PRAGMA foreign_key_check` gate fails loudly rather than letting dangling references surface later.
- Connections now enable foreign key enforcement (`PRAGMA foreign_keys = ON`, off by default in SQLite), so the tag table's `ON DELETE CASCADE` behaves as declared.
- Task CRUD (`add_to_db`, `update_task_in_db`, `get_all_db_contents`) reads and writes tags through the new table; the reader detects and supports both the old and new formats so import sources can predate the migration.

## Fixes

- `checklist wipe` without `-y` no longer loops forever if stdin closes before an answer arrives (e.g. piped input or Ctrl+D); it now halts with tasks preserved.
- Error conditions now exit with a non-zero status code instead of printing and exiting 0: `where -d/-c/-t`, `theme` without an action, partial import failures, and foreign key enforcement failures.

## Testing

- Added a black-box CLI test suite (`tests/`) covering happy and failure paths for `init`, `where`, `import`, `wipe`, `theme`, and `migrate`

# v0.1.8

> NOTE: In this version AI assistance is used.

## Enhancements

- `checklist import <DB PATH>` now supports `~` expansion for the import path.
- `checklist import --memory <DB PATH>` now imports into an in-memory database instead of the configured on-disk database, and no longer creates config or database files on disk when a config does not exist.
- `checklist import` now accepts a `--display` flag (and optional `-v/--view`) to open the TUI showing the imported tasks once the import finishes. Works with `--memory` — the import is loaded into the in-memory database and then displayed.
- `--memory` no longer creates a `theme.toml` file on disk. A default theme is constructed in memory instead, so `--memory` is now fully ephemeral (no config, database, or theme files written).
- New `checklist theme --migrate` command re-serializes `theme.toml` with all current keys and defaults, so newly added theme options can be surfaced into an existing file. Note that migrate regenerates the file and does not preserve comments.
- `init --set <path>` has been improved upon in the following ways:
  - User provided input to the `init --set <path>` command now supports paths leading with `~` to denote the user's home directory.
  - If user passes in a directory and no 'checklist.sqlite' exists, one is made. Otherwise, the existing 'checklist.sqlite' is used.
  - If user passes in a file, we check if it's a 'sqlite' file. If it is, we use that. Otherwise, an error is returned.

## Removed

- Removed the legacy hand-rolled terminal UI (`display --old`). The ratatui-based TUI is now the only interactive interface.

## Internal Changes

- The config directory is now resolved once at startup and passed through as a value (`ConfigDir`) instead of being a hardcoded global switched by a `testing` flag. As a result, the `--test` CLI flag has been removed (`--memory` covers user testing needs).
- Tests now run against isolated temp directories instead of the real `~/.config/checklist/`. The full test suite passes under default parallel execution; `--test-threads=1` is no longer needed.
- `Runtime` enum (Memory/Test/Real) collapsed to a simpler `memory` flag on the app.
- `import` now returns the `Connection` it imported into, and `run_tui`/`App` accept an existing connection instead of always opening their own. This lets `import --display` (including with `--memory`) show the freshly imported tasks in the same database.
- `read_theme` is now a pure read — it no longer rewrites `theme.toml` on every launch, so user comments and formatting are preserved. Missing tables/fields fall back to `#[serde(default)]` values (the `Theme` sub-structs now implement `Default`). The only path that rewrites the file is the opt-in `checklist theme --migrate`.
- Cleaned up the `toml` dependency version string.

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
