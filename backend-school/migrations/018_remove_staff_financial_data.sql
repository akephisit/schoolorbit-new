-- ===================================================================
-- Migration 018: Remove Unnecessary Data from Staff Info
-- Description: ลบข้อมูลการเงินและข้อมูลการทำงานออกจากระบบ (ลดความเสี่ยง PDPA)
-- Reason: ระบบโรงเรียนไม่จำเป็นต้องเก็บข้อมูลเหล่านี้
--         - ข้อมูลการเงิน → ควรอยู่ในระบบบัญชี/HR แยกต่างหาก
--         - ข้อมูลเวลาทำงาน → ไม่จำเป็นสำหรับระบบโรงเรียน
-- Date: 2026-01-08
-- ===================================================================

-- ===================================================================
-- 1. Drop Financial and Work Schedule Columns from staff_info
-- ===================================================================
ALTER TABLE staff_info
    -- Financial Data (ข้อมูลการเงิน)
    DROP COLUMN IF EXISTS salary,
    DROP COLUMN IF EXISTS bank_account,
    DROP COLUMN IF EXISTS bank_name,
    DROP COLUMN IF EXISTS tax_id,
    DROP COLUMN IF EXISTS social_security_id,
    
    -- Work Schedule (ข้อมูลเวลาทำงาน)
    DROP COLUMN IF EXISTS work_days,
    DROP COLUMN IF EXISTS work_hours_start,
    DROP COLUMN IF EXISTS work_hours_end;

-- ===================================================================
-- 2. Add Comments
-- ===================================================================
COMMENT ON TABLE staff_info IS 'ข้อมูลเฉพาะบุคลากร (เก็บเฉพาะข้อมูลที่จำเป็นต่อการบริหารจัดการโรงเรียนเท่านั้น)';

-- ===================================================================
-- 3. Verify Changes
-- ===================================================================
SELECT 
    column_name,
    data_type,
    is_nullable
FROM information_schema.columns
WHERE table_name = 'staff_info'
ORDER BY ordinal_position;
