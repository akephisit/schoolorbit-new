-- Domain-owned File Platform references. Legacy path columns remain nullable for
-- rollback compatibility but are no longer written by the application.

ALTER TABLE users
    ADD COLUMN profile_image_file_id UUID REFERENCES files(id) ON DELETE SET NULL;

CREATE INDEX idx_users_profile_image_file_id
    ON users (profile_image_file_id)
    WHERE profile_image_file_id IS NOT NULL;

ALTER TABLE staff_achievements
    ADD COLUMN image_file_id UUID REFERENCES files(id) ON DELETE SET NULL;

CREATE INDEX idx_staff_achievements_image_file_id
    ON staff_achievements (image_file_id)
    WHERE image_file_id IS NOT NULL;
