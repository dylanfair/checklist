-- Down migration for V1: restore the ;-joined task.tags column.
--
-- Note: group_concat emits tags in an unspecified order. That is acceptable
-- here because tags are semantically a set (the old reader split them into a
-- HashSet), so ordering never carried meaning.

ALTER TABLE task ADD COLUMN tags TEXT;

UPDATE task
SET tags = (
    SELECT group_concat(tag, ';') FROM tag WHERE tag.task_id = task.id
)
WHERE EXISTS (
    SELECT 1 FROM tag WHERE tag.task_id = task.id
);

DROP TABLE tag;
