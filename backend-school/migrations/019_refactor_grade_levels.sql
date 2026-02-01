-- ===================================================================
-- Refactor Grade Levels: Use level_type + year instead of manual fields
-- ===================================================================

-- Drop the old unique constraint on code
ALTER TABLE grade_levels DROP CONSTRAINT IF EXISTS grade_levels_code_key;

-- Drop the old index on level_order
DROP INDEX IF EXISTS idx_grade_levels_order;

-- Add new columns
ALTER TABLE grade_levels ADD COLUMN IF NOT EXISTS level_type VARCHAR(20);
ALTER TABLE grade_levels ADD COLUMN IF NOT EXISTS year INTEGER;

-- Migrate existing data
-- Parse level_type from code (K=kindergarten, P=primary, M=secondary)
UPDATE grade_levels SET 
    level_type = CASE 
        WHEN code LIKE 'K%' THEN 'kindergarten'
        WHEN code LIKE 'P%' THEN 'primary'
        WHEN code LIKE 'M%' THEN 'secondary'
        ELSE 'other'
    END,
    year = CASE 
        WHEN code ~ '[0-9]+' THEN (regexp_replace(code, '[^0-9]', '', 'g'))::INTEGER
        ELSE 1
    END
WHERE level_type IS NULL;

-- Make new columns NOT NULL after migration
ALTER TABLE grade_levels ALTER COLUMN level_type SET NOT NULL;
ALTER TABLE grade_levels ALTER COLUMN year SET NOT NULL;

-- Drop old columns that are now computed
ALTER TABLE grade_levels DROP COLUMN IF EXISTS code;
ALTER TABLE grade_levels DROP COLUMN IF EXISTS name;
ALTER TABLE grade_levels DROP COLUMN IF EXISTS short_name;
ALTER TABLE grade_levels DROP COLUMN IF EXISTS level_order;

-- Add unique constraint on (level_type, year)
ALTER TABLE grade_levels ADD CONSTRAINT grade_levels_type_year_unique UNIQUE (level_type, year);

-- Add index for ordering
CREATE INDEX idx_grade_levels_type_year ON grade_levels(level_type, year);

-- Clean up old data and re-seed with clean structure
DELETE FROM grade_levels;

-- Seed with kindergarten levels (อนุบาล)
INSERT INTO grade_levels (level_type, year) VALUES
('kindergarten', 1),
('kindergarten', 2),
('kindergarten', 3);

-- Seed with primary levels (ประถม)
INSERT INTO grade_levels (level_type, year) VALUES
('primary', 1),
('primary', 2),
('primary', 3),
('primary', 4),
('primary', 5),
('primary', 6);

-- Seed with secondary levels (มัธยม)
INSERT INTO grade_levels (level_type, year) VALUES
('secondary', 1),
('secondary', 2),
('secondary', 3),
('secondary', 4),
('secondary', 5),
('secondary', 6);

-- Update next_grade_level_id for promotion logic
-- Kindergarten chain
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'kindergarten' AND year = 2) WHERE level_type = 'kindergarten' AND year = 1;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'kindergarten' AND year = 3) WHERE level_type = 'kindergarten' AND year = 2;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 1) WHERE level_type = 'kindergarten' AND year = 3;

-- Primary chain
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 2) WHERE level_type = 'primary' AND year = 1;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 3) WHERE level_type = 'primary' AND year = 2;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 4) WHERE level_type = 'primary' AND year = 3;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 5) WHERE level_type = 'primary' AND year = 4;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 6) WHERE level_type = 'primary' AND year = 5;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'secondary' AND year = 1) WHERE level_type = 'primary' AND year = 6;

-- Secondary chain
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'secondary' AND year = 2) WHERE level_type = 'secondary' AND year = 1;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'secondary' AND year = 3) WHERE level_type = 'secondary' AND year = 2;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'secondary' AND year = 4) WHERE level_type = 'secondary' AND year = 3;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'secondary' AND year = 5) WHERE level_type = 'secondary' AND year = 4;
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE level_type = 'secondary' AND year = 6) WHERE level_type = 'secondary' AND year = 5;
