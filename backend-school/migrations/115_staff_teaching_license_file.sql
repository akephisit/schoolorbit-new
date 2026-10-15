-- Additive: existing profiles and licence metadata retain their values.
ALTER TABLE staff_info ADD COLUMN teaching_license_file_id UUID
    REFERENCES files(id) ON DELETE RESTRICT;
CREATE UNIQUE INDEX staff_info_teaching_license_file_idx
    ON staff_info(teaching_license_file_id) WHERE teaching_license_file_id IS NOT NULL;
