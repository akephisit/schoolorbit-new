-- Migration 007: Drop unused staff_info columns
-- ลบคอลัมน์ที่ไม่ได้ใช้งาน: employee_id และ employment_type

-- Drop unused columns from staff_info
ALTER TABLE staff_info 
    DROP COLUMN IF EXISTS employee_id,
    DROP COLUMN IF EXISTS employment_type;

-- Drop index if exists
DROP INDEX IF EXISTS idx_staff_info_employee_id;

-- Update table comment
COMMENT ON TABLE staff_info IS 'ข้อมูลเฉพาะบุคลากร - Updated: Removed employee_id and employment_type (unused fields)';
