LOCK TABLE staff_leave_types IN SHARE ROW EXCLUSIVE MODE;

WITH candidate AS (
    SELECT uuidv7() AS id
    WHERE NOT EXISTS (
        SELECT 1 FROM staff_leave_types
        WHERE kind = 'official_travel' AND document->>'archived' = 'false'
    )
), catalog_entry AS (
    SELECT id, 'type_' || replace(id::text, '-', '') AS code
    FROM candidate
)
INSERT INTO staff_leave_types (id, code, kind, revision, document)
SELECT id, code, 'official_travel', 1, jsonb_build_object(
    'id', id,
    'code', code,
    'name', 'เดินทางไปราชการ',
    'kind', 'official_travel',
    'archived', false,
    'revision', 1
)
FROM catalog_entry;
