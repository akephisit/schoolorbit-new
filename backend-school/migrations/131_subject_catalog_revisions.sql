-- Subject revisions append to one stable identity, including corrections effective on
-- the same date. Consumers pinned to a version retain their original immutable row.
ALTER TABLE subject_versions DROP CONSTRAINT subjects_code_start_year_key;
DROP TRIGGER subject_versions_no_overlap ON subject_versions;

-- Keep the subject effective-range check, revision-number uniqueness, all published
-- immutability triggers, and the independently owned activity overlap restriction.
