-- V1: Move tags out of a ;-joined TEXT column into a relational table.
--
-- Before: task.tags held a ;-joined string ("work;urgent"), which could not
--         represent a tag containing ';' and produced junk empty segments
--         from ';;' or a trailing ';'.
-- After:  each (task, tag) pair is its own row in `tag`, so tags may contain
--         any character and are individually addressable/queryable in SQL.

CREATE TABLE tag (
    task_id TEXT NOT NULL,
    tag     TEXT NOT NULL,
    PRIMARY KEY (task_id, tag),
    FOREIGN KEY (task_id) REFERENCES task(id) ON DELETE CASCADE
);

-- Copy existing ;-joined tags into the new table.
--
-- The recursive CTE peels one ;-delimited segment off `rest` per iteration;
-- appending ';' to the seed guarantees a terminator for the final segment.
-- Empty segments (from '', ';;', or a trailing ';') are dropped, matching
-- what the old Rust reader produced. Tag values are copied verbatim -- the
-- old reader never trimmed, so neither does this.
INSERT OR IGNORE INTO tag (task_id, tag)
WITH RECURSIVE split(task_id, value, rest) AS (
    SELECT id, '', tags || ';' FROM task WHERE tags IS NOT NULL
    UNION ALL
    SELECT task_id,
           substr(rest, 1, instr(rest, ';') - 1),
           substr(rest, instr(rest, ';') + 1)
    FROM split
    WHERE rest != ''
)
SELECT task_id, value FROM split WHERE value != '';

-- Retire the old column. Requires SQLite >= 3.35.0; the bundled SQLite in
-- rusqlite is far newer than that.
ALTER TABLE task DROP COLUMN tags;
