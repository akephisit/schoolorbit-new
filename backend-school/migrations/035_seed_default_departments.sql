-- Migration to seed default departments for a standard Thai school structure

-- Helper function to get department ID by code (to linking parents)
-- Note: We assume codes are unique for this seeding process

-- 1. สำนักงานผู้อำนวยการ (Director Office) - Root Level
INSERT INTO departments (id, code, name, name_en, category, org_type, display_order, is_active)
VALUES (gen_random_uuid(), 'DIR-01', 'สำนักงานผู้อำนวยการ', 'Director Office', 'administrative', 'office', 1, true)
ON CONFLICT (code) DO NOTHING;

-- 2. กลุ่มบริหารวิชาการ (Academic Affairs) - Root Level
INSERT INTO departments (id, code, name, name_en, category, org_type, display_order, is_active)
VALUES (gen_random_uuid(), 'ACAD-01', 'กลุ่มบริหารงานวิชาการ', 'Academic Affairs', 'administrative', 'group', 2, true)
ON CONFLICT (code) DO NOTHING;

-- 3. กลุ่มบริหารงานบุคคล (Personnel Affairs) - Root Level
INSERT INTO departments (id, code, name, name_en, category, org_type, display_order, is_active)
VALUES (gen_random_uuid(), 'PER-01', 'กลุ่มบริหารงานบุคคล', 'Personnel Affairs', 'administrative', 'group', 3, true)
ON CONFLICT (code) DO NOTHING;

-- 4. กลุ่มบริหารงานงบประมาณ (Budget & Planning) - Root Level
INSERT INTO departments (id, code, name, name_en, category, org_type, display_order, is_active)
VALUES (gen_random_uuid(), 'BUD-01', 'กลุ่มบริหารงานงบประมาณ', 'Budget & Planning', 'administrative', 'group', 4, true)
ON CONFLICT (code) DO NOTHING;

-- 5. กลุ่มบริหารงานทั่วไป (General Administration) - Root Level
INSERT INTO departments (id, code, name, name_en, category, org_type, display_order, is_active)
VALUES (gen_random_uuid(), 'GEN-01', 'กลุ่มบริหารงานทั่วไป', 'General Administration', 'administrative', 'group', 5, true)
ON CONFLICT (code) DO NOTHING;


-- =============================================
-- Child Departments (Under Academic Affairs)
-- =============================================

-- DO Loop style block is not fully supported in simple migration runner unless using PL/pgSQL
-- We will use common table expression or simple subqueries to get parent IDs

-- 8 สาระการเรียนรู้ (Learning Areas)
INSERT INTO departments (id, code, name, name_en, parent_department_id, category, org_type, display_order, is_active)
SELECT gen_random_uuid(), 'SUBJ-TH', 'กลุ่มสาระการเรียนรู้ภาษาไทย', 'Thai Language Department', id, 'academic', 'unit', 1, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-MA', 'กลุ่มสาระการเรียนรู้คณิตศาสตร์', 'Mathematics Department', id, 'academic', 'unit', 2, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-SC', 'กลุ่มสาระการเรียนรู้วิทยาศาสตร์และเทคโนโลยี', 'Science and Technology Department', id, 'academic', 'unit', 3, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-SO', 'กลุ่มสาระการเรียนรู้สังคมศึกษา ศาสนา และวัฒนธรรม', 'Social Studies, Religion and Culture Department', id, 'academic', 'unit', 4, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-HP', 'กลุ่มสาระการเรียนรู้สุขศึกษาและพลศึกษา', 'Health and Physical Education Department', id, 'academic', 'unit', 5, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-AR', 'กลุ่มสาระการเรียนรู้ศิลปะ', 'Arts Department', id, 'academic', 'unit', 6, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-OT', 'กลุ่มสาระการเรียนรู้การงานอาชีพ', 'Occupations and Technology Department', id, 'academic', 'unit', 7, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'SUBJ-EN', 'กลุ่มสาระการเรียนรู้ภาษาต่างประเทศ', 'Foreign Languages Department', id, 'academic', 'unit', 8, true FROM departments WHERE code = 'ACAD-01'
-- Academic Support Units
UNION ALL
SELECT gen_random_uuid(), 'ACAD-REG', 'งานทะเบียนและวัดผล', 'Registration and Measurement', id, 'academic', 'unit', 9, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'ACAD-CUR', 'งานหลักสูตรสถานศึกษา', 'Curriculum Development', id, 'academic', 'unit', 10, true FROM departments WHERE code = 'ACAD-01'
UNION ALL
SELECT gen_random_uuid(), 'ACAD-ACT', 'กิจกรรมพัฒนาผู้เรียน', 'Learner Development Activities', id, 'academic', 'unit', 11, true FROM departments WHERE code = 'ACAD-01';


-- =============================================
-- Child Departments (Under Personnel Affairs)
-- =============================================
INSERT INTO departments (id, code, name, name_en, parent_department_id, category, org_type, display_order, is_active)
SELECT gen_random_uuid(), 'PER-DIS', 'งานวินัยและกิจการนักเรียน', 'Student Discipline and Affairs', id, 'administrative', 'unit', 1, true FROM departments WHERE code = 'PER-01'
UNION ALL
SELECT gen_random_uuid(), 'PER-ADV', 'งานครูที่ปรึกษา', 'Advisor System', id, 'administrative', 'unit', 2, true FROM departments WHERE code = 'PER-01';


-- =============================================
-- Child Departments (Under Budget & Planning)
-- =============================================
INSERT INTO departments (id, code, name, name_en, parent_department_id, category, org_type, display_order, is_active)
SELECT gen_random_uuid(), 'BUD-PLA', 'งานแผนงานและนโยบาย', 'Policy and Planning', id, 'administrative', 'unit', 1, true FROM departments WHERE code = 'BUD-01'
UNION ALL
SELECT gen_random_uuid(), 'BUD-FIN', 'งานการเงินและบัญชี', 'Finance and Accounting', id, 'administrative', 'unit', 2, true FROM departments WHERE code = 'BUD-01'
UNION ALL
SELECT gen_random_uuid(), 'BUD-AST', 'งานพัสดุและสินทรัพย์', 'Procurement and Assets', id, 'administrative', 'unit', 3, true FROM departments WHERE code = 'BUD-01';


-- =============================================
-- Child Departments (Under General Administration)
-- =============================================
INSERT INTO departments (id, code, name, name_en, parent_department_id, category, org_type, display_order, is_active)
SELECT gen_random_uuid(), 'GEN-BLD', 'งานอาคารสถานที่', 'Building and Grounds', id, 'administrative', 'unit', 1, true FROM departments WHERE code = 'GEN-01'
UNION ALL
SELECT gen_random_uuid(), 'GEN-PR', 'งานประชาสัมพันธ์', 'Public Relations', id, 'administrative', 'unit', 2, true FROM departments WHERE code = 'GEN-01'
UNION ALL
SELECT gen_random_uuid(), 'GEN-HEL', 'งานอนามัยโรงเรียน', 'School Health', id, 'administrative', 'unit', 3, true FROM departments WHERE code = 'GEN-01';
