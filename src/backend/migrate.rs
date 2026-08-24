//! Schema migrations for the checklist SQLite database, powered by
//! [`rusqlite_migration`].
//!
//! Migrations live as SQL files under the `migrations/` directory at the
//! crate root and are embedded into the binary at compile time via
//! [`include_dir`] — so installed binaries (e.g. from `cargo install`) carry
//! their migrations with them and don't depend on files on disk.
//!
//! # Layout
//!
//! Each migration is a subdirectory named `{id}-{name}`, where `id` is a
//! 1-based consecutive number and `name` is a short slug:
//!
//! ```text
//! migrations/
//! ├── 1-placeholder_noop/
//! │   └── up.sql
//! └── 2-next_migration/
//!     └── up.sql        (down.sql is optional)
//! ```
//!
//! To add a migration: create the next numbered subdirectory with an
//! `up.sql`, then rebuild. `rusqlite_migration` validates that ids are
//! consecutive starting at 1.
//!
//! # Version tracking
//!
//! `rusqlite_migration` tracks the applied schema version in SQLite's built-in
//! `user_version` pragma rather than a bookkeeping table — no extra schema
//! needed. Databases created before this system existed are at version 0, so
//! all pending migrations apply to them in order on first launch of a version
//! of checklist that runs migrations.

use include_dir::{Dir, include_dir};
use rusqlite::Connection;
use rusqlite_migration::Migrations;

/// The embedded migrations directory.
static MIGRATION_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// Load the embedded migrations. Panics if the migration files are malformed
/// (non-consecutive ids, missing `up.sql`) — that's a build-time authoring bug,
/// not a runtime condition, so failing fast is appropriate.
pub fn migrations() -> Migrations<'static> {
    Migrations::from_directory(&MIGRATION_DIR).expect("embedded migration files should be valid")
}

/// Bring a database up to the latest schema version. Applies any pending
/// migrations in order; a no-op when already current.
///
/// Per rusqlite_migration's guidance, foreign key enforcement is switched off
/// for the duration of the migration and restored afterwards — SQLite advises
/// running schema changes with FK checks off, and PRAGMA foreign_keys is a
/// no-op inside the migration's own transaction.
pub fn run_migrations(conn: &mut Connection) -> anyhow::Result<()> {
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let result = migrations()
        .to_latest(conn)
        .map_err(|e| anyhow::anyhow!("Database migration failed: {e}"));
    conn.pragma_update(None, "foreign_keys", "ON")?;
    result?;

    // Integrity gate: fail loudly if the migrated database contains dangling
    // references, instead of letting them surface as confusing errors later.
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let violations = stmt
        .query_map([], |row| {
            row.get::<_, String>(0)?; // child table
            row.get::<_, Option<String>>(1)?; // rowid
            Ok(())
        })?
        .count();
    if violations > 0 {
        anyhow::bail!(
            "Database migration left {violations} foreign key violation(s); \
             please report this — your data may need attention"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::backend::database::make_memory_connection;
    use rusqlite_migration::SchemaVersion;

    #[test]
    fn migrations_load_from_embedded_directory() {
        // A malformed migrations directory would have made `migrations()`
        // panic; reaching here means ids were consecutive and every migration
        // had an up.sql.
        let conn = make_memory_connection().unwrap();
        assert_eq!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::NoneSet,
            "a freshly initialized checklist database should start at version 0"
        );
    }

    #[test]
    fn run_migrations_applies_to_a_managed_database() {
        // Checklist-managed databases are never truly empty: they're created
        // by init_schema (old-style, version 0) and then brought forward.
        // Simulate exactly that.
        let mut conn = make_memory_connection().unwrap();
        run_migrations(&mut conn).unwrap();
        assert!(matches!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::Inside(_) | SchemaVersion::Outside(_),
        ));
    }

    #[test]
    fn run_migrations_is_idempotent() {
        let mut conn = make_memory_connection().unwrap();
        run_migrations(&mut conn).unwrap();
        let after_first = migrations().current_version(&conn).unwrap();

        // Second call should be a no-op, not an error.
        run_migrations(&mut conn).unwrap();

        assert_eq!(migrations().current_version(&conn).unwrap(), after_first);
    }

    /// The real litmus test: simulates a pre-v0.1.9 database exactly as
    /// `make_memory_connection` creates it (old-style `task` schema with a
    /// ;-joined `tags TEXT` column, version 0), seeds it the way the old app
    /// wrote tasks, then migrates and verifies everything a user would care
    /// about survived.
    #[test]
    fn v1_migrates_an_old_style_database_end_to_end() {
        use std::collections::HashSet;

        use crate::backend::database::make_memory_connection;

        let mut conn = make_memory_connection().unwrap();
        assert_eq!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::NoneSet,
            "make_memory_connection should produce a version-0 (pre-migration) database"
        );

        // Seed exactly the way the OLD writer did: raw inserts into task with
        // a ;-joined tags string. We must not use today's add_to_db here —
        // it writes relationally and would defeat the simulation. Ids are
        // kept on the Rust side since rusqlite stores Uuids as BLOBs.
        let tagged_id = uuid::Uuid::new_v4();
        conn.execute(
            "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                tagged_id,
                "Migrate me",
                "A description",
                None::<String>,
                "High",
                "Open",
                "work;urgent",
                chrono::Local::now(),
                None::<chrono::DateTime<chrono::Local>>,
            ],
        )
        .unwrap();

        // A task with no tags at all — exercises the NULL path.
        let untagged_id = uuid::Uuid::new_v4();
        conn.execute(
            "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                untagged_id,
                "No tags",
                None::<String>,
                None::<String>,
                "Low",
                "Open",
                None::<String>,
                chrono::Local::now(),
                None::<chrono::DateTime<chrono::Local>>,
            ],
        )
        .unwrap();

        run_migrations(&mut conn).unwrap();

        // 1. Version stamped at exactly V1.
        assert_eq!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::Inside(std::num::NonZeroUsize::new(1).unwrap()),
        );

        // 2. The old tags column is gone.
        let tags_column_gone = conn.prepare("SELECT tags FROM task").is_err();
        assert!(
            tags_column_gone,
            "task.tags column should have been dropped"
        );

        // 3. Tags were copied into the tag table for the tagged task...
        let tag_rows: HashSet<String> = conn
            .prepare("SELECT tag FROM tag WHERE task_id = ?1")
            .unwrap()
            .query_map([&tagged_id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(
            tag_rows,
            HashSet::from_iter(["work".to_string(), "urgent".to_string(),])
        );

        // ...and the untagged task produced no rows.
        let untagged_rows: i64 = conn
            .prepare("SELECT COUNT(*) FROM tag WHERE task_id = ?1")
            .unwrap()
            .query_row([&untagged_id], |row| row.get(0))
            .unwrap();
        assert_eq!(untagged_rows, 0);

        // 4. The task rows themselves survived intact.
        let (name, description): (String, Option<String>) = conn
            .query_row(
                "SELECT name, description FROM task WHERE id = ?1",
                [tagged_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Migrate me");
        assert_eq!(description.as_deref(), Some("A description"));
    }
}
