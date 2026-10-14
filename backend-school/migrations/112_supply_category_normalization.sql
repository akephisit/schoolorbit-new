-- Preserve canonical application-written keys and repair raw-mirrored seed keys.
-- Existing collisions refuse the migration through the unique constraint.
UPDATE supply_categories
SET normalized_name = lower(normalize(name, NFKC))
WHERE normalized_name = name;
