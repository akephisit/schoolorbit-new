-- Migration 058: Switch subjects versioning to use start_academic_year_id only
--
-- Previously subjects used academic_year_id as the "current year" key (one record per year via bulk_copy).
-- start_academic_year_id was added as an optional field with no enforced semantic.
--
-- New design: "Effective-from versioning"
-- - start_academic_year_id = the year this subject version became effective
-- - A new record is only created when the subject actually changes
-- - To find the right version for year Y: WHERE code = X AND start_year <= Y ORDER BY start_year DESC LIMIT 1
-- - classroom_courses keep subject_id frozen (historical snapshot preserved)

-- 1. Populate start_academic_year_id from academic_year_id where not set
UPDATE subjects SET start_academic_year_id = academic_year_id WHERE start_academic_year_id IS NULL;

-- 2. Enforce NOT NULL
ALTER TABLE subjects ALTER COLUMN start_academic_year_id SET NOT NULL;

-- 3. Drop old unique constraint (code, academic_year_id)
ALTER TABLE subjects DROP CONSTRAINT IF EXISTS subjects_code_year_key;

-- 4. Add new unique constraint (code, start_academic_year_id)
ALTER TABLE subjects ADD CONSTRAINT subjects_code_start_year_key UNIQUE (code, start_academic_year_id);

-- 5. Drop the now-redundant academic_year_id column
ALTER TABLE subjects DROP COLUMN academic_year_id;
