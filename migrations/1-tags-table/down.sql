-- Down migration for V1: restore the legacy ;-joined task.tags column.
--
-- This REBUILDS the table rather than using `ALTER TABLE task ADD COLUMN`.
-- SQLite appends added columns at the end of the table, which would shift
-- every positional reader: older checklist binaries locate columns by index
-- (`SELECT *` + fixed offsets), so the restored layout has to match what
-- init_schema originally created — tags back between status and date_added.
--
-- group_concat emits tags in an unspecified order. That is acceptable here
-- because tags are semantically a set (the old reader split them into a
-- HashSet), so ordering never carried meaning. Untagged tasks get NULL,
-- matching what the old writer stored.

CREATE TABLE task_downgraded (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    latest TEXT,
    urgency TEXT,
    status TEXT NOT NULL,
    tags TEXT,
    date_added DATE NOT NULL,
    completed_on DATE
);

INSERT INTO task_downgraded (
    id, name, description, latest, urgency, status, tags, date_added, completed_on
)
SELECT
    t.id,
    t.name,
    t.description,
    t.latest,
    t.urgency,
    t.status,
    (
        SELECT group_concat(tag, ';')
        FROM tag
        WHERE tag.task_id = t.id
    ),
    t.date_added,
    t.completed_on
FROM task t;

DROP TABLE tag;
DROP TABLE task;
ALTER TABLE task_downgraded RENAME TO task;
