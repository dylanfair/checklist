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
pub fn latest_schema_version() -> usize {
    MIGRATION_DIR.dirs().count()
}

/// Load the embedded migrations. Panics if the migration files are malformed
/// (non-consecutive ids, missing `up.sql`) — that's a build-time authoring bug,
/// not a runtime condition, so failing fast is appropriate.
pub fn migrations() -> Migrations<'static> {
    Migrations::from_directory(&MIGRATION_DIR).expect("embedded migration files should be valid")
}

/// Snapshot the database file before a schema change (up or down), so a bad
/// migration can be recovered from by restoring the backup and reinstalling a
/// matching binary (schema and binary are a matched pair — see README).
///
/// Behavior:
/// - No-op when the database is already at `target` (nothing is about to
///   change). Works for downward moves too — see [`migrate_to_version`].
/// - Snapshots collect in a `checklist-migration-snapshots/` folder next to the
///   database (wherever it lives — the config dir by default, or an
///   `init --set` location), keeping each database's safety net with it.
/// - Backup files carry the source version in their name
///   (`checklist.sqlite.pre-migration-v0.bak`), so each upgrade era gets its
///   own snapshot.
/// - An existing backup for that era is never overwritten: if a migration
///   fails and corrupts the database, retrying cannot clobber the pristine
///   pre-migration snapshot with the damaged file.
pub fn backup_before_schema_change(
    conn: &Connection,
    db_path: &Path,
    target: usize,
) -> anyhow::Result<()> {
    let current = match migrations().current_version(conn)? {
        SchemaVersion::Inside(n) | SchemaVersion::Outside(n) => n.get(),
        SchemaVersion::NoneSet => 0,
    };
    if current == target {
        // Nothing is about to change.
        return Ok(());
    }

    // Collect snapshots in a dedicated folder next to the database rather
    // than cluttering its directory.
    let snapshots_dir = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("checklist-migration-snapshots");
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

/// Which schema version a user asked to move to, from the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedVersion {
    /// One below the database's current version.
    Prior,
    /// The newest this binary supports.
    Latest,
    /// An exact version.
    Exact(usize),
}

/// A resolved migration request, ready to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationPlan {
    pub target: usize,
    pub current: usize,
}

impl MigrationPlan {
    /// True when this plan moves the schema backwards.
    pub fn is_downward(&self) -> bool {
        self.target < self.current
    }

    /// True when executing would change nothing.
    pub fn is_no_op(&self) -> bool {
        self.target == self.current
    }
}

/// Maps schema versions to the last checklist release that used them. Used to
/// tell users which app version can open a database after a downgrade.
///
/// Maintenance convention: when you add migration `N`, add one entry here —
/// `(N, "<current crate version>")` — in the same commit as its SQL. Write
/// the version as a literal string rather than `env!("CARGO_PKG_VERSION")`:
/// the env value is only correct in the release that introduces the
/// migration and would silently drift afterwards. A unit test enforces that
/// this table keeps covering every known schema version.
const SCHEMA_RELEASE_HISTORY: &[(usize, &str)] = &[(1, "0.1.9"), (0, "0.1.8")];

/// Human-readable description of which checklist releases open a database at
/// the given schema version. Derived from [`SCHEMA_RELEASE_HISTORY`]: the
/// first release that moved *past* `schema` is the first one that can no
/// longer read it natively.
pub fn releases_for_schema(schema: usize) -> String {
    match SCHEMA_RELEASE_HISTORY.iter().find(|(v, _)| *v > schema) {
        Some((_, first_incompatible)) => {
            format!("checklist versions before v{first_incompatible}")
        }
        // Every known release uses this schema or older; nothing has moved
        // past it yet.
        None => format!(
            "all checklist releases up to and including v{}",
            env!("CARGO_PKG_VERSION")
        ),
    }
}

/// Turn a user's migration request into a concrete plan against a database.
/// Pure validation — no I/O beyond reading the schema version — so the CLI
/// can prompt before anything executes.
pub fn resolve_target(
    conn: &Connection,
    requested: RequestedVersion,
) -> anyhow::Result<MigrationPlan> {
    let current = match migrations().current_version(conn)? {
        SchemaVersion::Inside(n) | SchemaVersion::Outside(n) => n.get(),
        SchemaVersion::NoneSet => 0,
    };
    let latest = latest_schema_version();

    let target = match requested {
        RequestedVersion::Prior => {
            if current == 0 {
                anyhow::bail!(
                    "Already at schema version 0 — there is no earlier version to migrate to."
                );
            }
            let prior = current - 1;
            if prior > latest {
                anyhow::bail!(
                    "This build understands migrations up to version {latest} and cannot \
                     reach version {prior}. Install a checklist version closer to the one \
                     that last wrote this database."
                );
            }
            prior
        }
        RequestedVersion::Latest => latest,
        RequestedVersion::Exact(n) => {
            if n > latest {
                anyhow::bail!(
                    "Schema version {n} is unknown to this build, which supports versions \
                     0..={latest}."
                );
            }
            n
        }
    };

    Ok(MigrationPlan { target, current })
}

/// Bring a database to exactly `target` (up **or** down), with a pre-change
/// snapshot when the call would actually change something. When `db_path` is
/// given, the snapshot is a file backup; without it (memory databases) no
/// backup is possible.
///
/// Per rusqlite_migration's guidance, foreign key enforcement is switched off
/// for the duration of the migration and restored afterwards — SQLite advises
/// running schema changes with FK checks off, and PRAGMA foreign_keys is a
/// no-op inside the migration's own transaction.
pub fn migrate_to_version(
    conn: &mut Connection,
    db_path: Option<&Path>,
    target: usize,
) -> anyhow::Result<()> {
    if let Some(path) = db_path {
        backup_before_schema_change(conn, path, target)?;
    }
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let result = migrations()
        .to_version(conn, target)
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

/// Bring a database up to the latest schema version. Applies any pending
/// migrations in order; a no-op when already current. Used by normal startup
/// paths — see [`migrate_to_version`] for explicit-version moves.
pub fn run_migrations(conn: &mut Connection, db_path: Option<&Path>) -> anyhow::Result<()> {
    migrate_to_version(conn, db_path, latest_schema_version())
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
            .join("checklist-migration-snapshots")
            .join("checklist.sqlite.pre-migration-v0.bak");
        assert!(!bak_path.exists());

        backup_before_schema_change(&conn, &db_path, 1).unwrap(); // pending: v0 -> v1
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
            .join("checklist-migration-snapshots")
            .join("checklist.sqlite.pre-migration-v0.bak");
        backup_before_schema_change(&conn, &db_path, 1).unwrap(); // pending: v0 -> v1
        drop(conn);
        let pristine = std::fs::read(&bak_path).unwrap();

        // "Corruption": the live file becomes garbage; a fresh connection on
        // it can no longer report a usable schema version.
        std::fs::write(&db_path, b"corrupted beyond recognition").unwrap();
        let conn_after = make_connection(&db_path).unwrap();
        let _ = backup_before_schema_change(&conn_after, &db_path, 1); // may Err; must not clobber

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
        backup_before_schema_change(&conn, &db_path, 1).unwrap();
        let bak_path = tmp
            .path()
            .join("checklist-migration-snapshots")
            .join("checklist.sqlite.pre-migration-v1.bak");
        assert!(!bak_path.exists());
    }

    /// SCHEMA_RELEASE_HISTORY must cover every schema version 0..=latest
    /// exactly once, in descending order — it's what makes downgrade guidance
    /// trustworthy.
    #[test]
    fn schema_release_history_is_complete() {
        let latest = latest_schema_version();
        let expected: Vec<usize> = (0..=latest).rev().collect();
        let actual: Vec<usize> = SCHEMA_RELEASE_HISTORY.iter().map(|(v, _)| *v).collect();
        assert_eq!(
            actual, expected,
            "SCHEMA_RELEASE_HISTORY should list every schema version 0..={latest}, newest first — add an entry when introducing a migration"
        );
    }

    #[test]
    fn resolve_target_prior() {
        let mut conn = make_memory_connection().unwrap();
        run_migrations(&mut conn, None).unwrap(); // now at V1

        let plan = resolve_target(&conn, RequestedVersion::Prior).unwrap();
        assert_eq!(
            plan,
            MigrationPlan {
                target: 0,
                current: 1
            }
        );
        assert!(plan.is_downward());

        // From the bottom there is no prior.
        migrations().to_version(&mut conn, 0).unwrap();
        assert!(resolve_target(&conn, RequestedVersion::Prior).is_err());
    }

    #[test]
    fn resolve_target_rejects_unknown_versions() {
        let conn = make_memory_connection().unwrap();

        let err = resolve_target(&conn, RequestedVersion::Exact(99))
            .expect_err("exact target above latest must be rejected");
        assert!(err.to_string().contains("unknown to this build"));

        // A database claiming to be from a newer checklist can't be reached
        // with --prior either: we lack those down files.
        conn.pragma_update(None, "user_version", &7).unwrap();
        let err = resolve_target(&conn, RequestedVersion::Prior)
            .expect_err("--prior on a newer database must be rejected");
        assert!(err.to_string().contains("cannot"));
    }

    #[test]
    fn resolve_target_latest_and_exact_no_op() {
        let mut conn = make_memory_connection().unwrap();
        run_migrations(&mut conn, None).unwrap();

        let plan = resolve_target(&conn, RequestedVersion::Latest).unwrap();
        assert!(plan.is_no_op(), "latest on a current db is a no-op");

        let plan = resolve_target(&conn, RequestedVersion::Exact(1)).unwrap();
        assert!(plan.is_no_op());
    }

    #[test]
    fn releases_for_schema_derives_guidance() {
        // Schema 0 predates v0.1.9, which introduced schema 1.
        assert_eq!(releases_for_schema(0), "checklist versions before v0.1.9");
        // Nothing has moved past the newest schema yet.
        assert!(releases_for_schema(latest_schema_version()).starts_with("all checklist releases"));
    }
}
