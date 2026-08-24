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

use anyhow::Context;
use include_dir::{Dir, include_dir};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, SchemaVersion};
use std::path::Path;

/// The embedded migrations directory.
static MIGRATION_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// The highest schema version the running binary knows about. Migration ids
/// are consecutive starting at 1 (validated by `from_directory`), so the
/// directory count is exactly the latest id.
fn latest_schema_version() -> usize {
    MIGRATION_DIR.dirs().count()
}

/// Load the embedded migrations. Panics if the migration files are malformed
/// (non-consecutive ids, missing `up.sql`) — that's a build-time authoring bug,
/// not a runtime condition, so failing fast is appropriate.
pub fn migrations() -> Migrations<'static> {
    Migrations::from_directory(&MIGRATION_DIR).expect("embedded migration files should be valid")
}

/// Snapshot the database file before a pending migration, so a bad migration
/// can be recovered from by restoring the backup and reinstalling an older
/// binary (schema and binary are a matched pair — see README).
///
/// Behavior:
/// - No-op when the database is already at the latest version (nothing is
///   about to change).
/// - Snapshots collect in a `migration-snapshots/` folder next to the
///   database (wherever it lives — the config dir by default, or an
///   `init --set` location), keeping each database's safety net with it.
/// - Backup files carry the source version in their name
///   (`checklist.sqlite.pre-migration-v0.bak`), so each upgrade era gets its
///   own snapshot.
/// - An existing backup for that era is never overwritten: if a migration
///   fails and corrupts the database, retrying cannot clobber the pristine
///   pre-migration snapshot with the damaged file.
pub fn backup_before_migration(conn: &Connection, db_path: &Path) -> anyhow::Result<()> {
    let current = match migrations().current_version(conn)? {
        SchemaVersion::Inside(n) | SchemaVersion::Outside(n) => n.get(),
        SchemaVersion::NoneSet => 0,
    };
    let latest = latest_schema_version();
    if current >= latest {
        // Nothing pending — the database is already current (or ahead of us).
        return Ok(());
    }

    // Collect snapshots in a dedicated folder next to the database rather
    // than cluttering its directory.
    let snapshots_dir = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("migration-snapshots");
    std::fs::create_dir_all(&snapshots_dir).with_context(|| {
        format!(
            "Failed to create the migration snapshots folder: {}",
            snapshots_dir.display()
        )
    })?;

    let mut bak_name = snapshots_dir
        .join(db_path.file_name().unwrap_or_default())
        .into_os_string();
    bak_name.push(format!(".pre-migration-v{current}.bak"));
    let bak_path = std::path::PathBuf::from(bak_name);

    if !bak_path.exists() {
        std::fs::copy(db_path, &bak_path).with_context(|| {
            format!(
                "Failed to back up the database to {} before migrating",
                bak_path.display()
            )
        })?;
        println!("Backed up database to {}", bak_path.display());
    }
    Ok(())
}

/// Bring a database up to the latest schema version. Applies any pending
/// migrations in order; a no-op when already current. When `db_path` is given,
/// a pre-migration backup of the database file is ensured first (see
/// [`backup_before_migration`]).
///
/// Per rusqlite_migration's guidance, foreign key enforcement is switched off
/// for the duration of the migration and restored afterwards — SQLite advises
/// running schema changes with FK checks off, and PRAGMA foreign_keys is a
/// no-op inside the migration's own transaction.
pub fn run_migrations(conn: &mut Connection, db_path: Option<&Path>) -> anyhow::Result<()> {
    if let Some(path) = db_path {
        backup_before_migration(conn, path)?;
    }
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
        run_migrations(&mut conn, None).unwrap();
        assert!(matches!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::Inside(_) | SchemaVersion::Outside(_),
        ));
    }

    #[test]
    fn run_migrations_is_idempotent() {
        let mut conn = make_memory_connection().unwrap();
        run_migrations(&mut conn, None).unwrap();
        let after_first = migrations().current_version(&conn).unwrap();

        // Second call should be a no-op, not an error.
        run_migrations(&mut conn, None).unwrap();

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

        run_migrations(&mut conn, None).unwrap();

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

    /// The down path: V1 must be reversible — tags re-materialize in the
    /// legacy ;-joined column, the tag table goes away, and the version stamp
    /// returns to 0. Nothing executes down.sql today (the app only migrates
    /// forward); this test exists so the file stays correct if it's ever
    /// needed for a manual rollback.
    #[test]
    fn v1_down_migration_restores_legacy_format() {
        use std::collections::HashSet;

        use crate::backend::database::make_memory_connection;

        let mut conn = make_memory_connection().unwrap();
        let task_id = uuid::Uuid::new_v4();
        conn.execute(
            "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on)
             VALUES (?1, 'Downgrade me', NULL, NULL, 'High', 'Open', 'work;urgent', ?2, NULL)",
            rusqlite::params![task_id, chrono::Local::now()],
        )
        .unwrap();
        run_migrations(&mut conn, None).unwrap();

        // Sanity: we're at V1 and tags are relational.
        assert_eq!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::Inside(std::num::NonZeroUsize::new(1).unwrap())
        );
        let relational_tags: HashSet<String> = conn
            .prepare("SELECT tag FROM tag WHERE task_id = ?1")
            .unwrap()
            .query_map([task_id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(
            relational_tags,
            HashSet::from_iter(["work".to_string(), "urgent".to_string()])
        );

        // Down to version 0.
        migrations().to_version(&mut conn, 0).unwrap();

        // Version stamped back to none, tag table dropped.
        assert_eq!(
            migrations().current_version(&conn).unwrap(),
            SchemaVersion::NoneSet
        );
        assert!(conn.prepare("SELECT * FROM tag").is_err());

        // Tags re-materialized in the legacy column. group_concat order is
        // unspecified, so compare as the old reader did: split into a set.
        let joined: String = conn
            .query_row("SELECT tags FROM task WHERE id = ?1", [task_id], |row| {
                row.get(0)
            })
            .unwrap();
        let restored: HashSet<String> = joined.split(';').map(str::to_string).collect();
        assert_eq!(
            restored,
            HashSet::from_iter(["work".to_string(), "urgent".to_string()])
        );
    }

    #[test]
    fn backup_created_before_pending_migration() {
        use crate::backend::database::make_connection;

        // A real file-based database at version 0 with data worth protecting.
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("checklist.sqlite");
        crate::backend::database::create_sqlite_db_at(db_path.clone()).unwrap();
        {
            let conn = make_connection(&db_path).unwrap();
            conn.execute(
                "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on)
                 VALUES (?1, 'Precious', NULL, NULL, 'Low', 'Open', 'keep;me', ?2, NULL)",
                rusqlite::params![uuid::Uuid::new_v4(), chrono::Local::now()],
            )
            .unwrap();
        }

        let conn = make_connection(&db_path).unwrap();
        let bak_path = tmp
            .path()
            .join("migration-snapshots")
            .join("checklist.sqlite.pre-migration-v0.bak");
        assert!(!bak_path.exists());

        backup_before_migration(&conn, &db_path).unwrap();
        assert!(bak_path.exists());

        // The snapshot is a faithful copy: same byte length.
        let original_len = std::fs::metadata(&db_path).unwrap().len();
        let bak_len = std::fs::metadata(&bak_path).unwrap().len();
        assert_eq!(original_len, bak_len);
    }

    #[test]
    fn backup_never_clobbered_by_retry_after_corruption() {
        use crate::backend::database::{create_sqlite_db_at, make_connection};

        // The scenario this guards: migration attempt #1 fails and leaves the
        // live database damaged; attempt #2 must not overwrite the pristine
        // pre-migration snapshot with the damaged file.
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("checklist.sqlite");

        // First attempt from a healthy connection creates the snapshot. The
        // file must be valid SQLite for the version probe, so start it as a
        // real version-0 checklist database.
        create_sqlite_db_at(db_path.clone()).unwrap();
        let conn = make_connection(&db_path).unwrap();
        let bak_path = tmp
            .path()
            .join("migration-snapshots")
            .join("checklist.sqlite.pre-migration-v0.bak");
        backup_before_migration(&conn, &db_path).unwrap();
        drop(conn);
        let pristine = std::fs::read(&bak_path).unwrap();

        // "Corruption": the live file becomes garbage; a fresh connection on
        // it can no longer report a usable schema version.
        std::fs::write(&db_path, b"corrupted beyond recognition").unwrap();
        let conn_after = make_connection(&db_path).unwrap();
        let _ = backup_before_migration(&conn_after, &db_path); // may Err; must not clobber

        let after_retry = std::fs::read(&bak_path).unwrap();
        assert_eq!(pristine, after_retry, "backup must survive retry attempts");
    }

    #[test]
    fn backup_skipped_when_already_current() {
        use crate::backend::database::make_memory_connection;

        let mut conn = make_memory_connection().unwrap();
        run_migrations(&mut conn, None).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("checklist.sqlite");
        std::fs::write(&db_path, b"current database").unwrap();

        // Already at latest: no pending migration, so no backup should be
        // created even though the path is writable.
        backup_before_migration(&conn, &db_path).unwrap();
        let bak_path = tmp
            .path()
            .join("migration-snapshots")
            .join("checklist.sqlite.pre-migration-v1.bak");
        assert!(!bak_path.exists());
    }
}
