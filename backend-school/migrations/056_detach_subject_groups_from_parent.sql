-- Migration 056: แยกกลุ่มสาระ (SUBJ-*) ออกจากกลุ่มบริหารวิชาการ
-- กลุ่มสาระมีหน้าจัดการแยกต่างหาก ไม่ต้องอยู่ใต้กลุ่มบริหารวิชาการ

UPDATE departments
SET parent_department_id = NULL
WHERE code LIKE 'SUBJ-%';
