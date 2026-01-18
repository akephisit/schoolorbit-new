-- Drop old FK constraints referencing staff_info
ALTER TABLE class_rooms DROP CONSTRAINT IF EXISTS class_rooms_advisor_id_fkey;
ALTER TABLE class_rooms DROP CONSTRAINT IF EXISTS class_rooms_co_advisor_id_fkey;

-- Add new FK constraints referencing users
-- Note: We assume the IDs stored are compatible (UUID). 
-- If there was existing data using staff_info IDs, it would be invalid now, 
-- but since we are in dev/initial phase, this schema change is acceptable.
ALTER TABLE class_rooms 
    ADD CONSTRAINT class_rooms_advisor_id_fkey 
    FOREIGN KEY (advisor_id) REFERENCES users(id);

ALTER TABLE class_rooms 
    ADD CONSTRAINT class_rooms_co_advisor_id_fkey 
    FOREIGN KEY (co_advisor_id) REFERENCES users(id);
