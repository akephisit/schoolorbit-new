-- ===================================================================
-- Migration 005: Staff Management System
-- คำอธิบาย: ระบบจัดการบุคลากรแบบยืดหยุ่น (Flexible User Management)
-- รองรับ: Multi-role, Multi-department, Permission-based access
-- ===================================================================

-- ===================================================================
-- 1. Update users table to support user types
-- ===================================================================
ALTER TABLE users 
    ADD COLUMN IF NOT EXISTS title VARCHAR(50),
    ADD COLUMN IF NOT EXISTS nickname VARCHAR(50),
    ADD COLUMN IF NOT EXISTS user_type VARCHAR(50) NOT NULL DEFAULT 'staff',
    ADD COLUMN IF NOT EXISTS emergency_contact VARCHAR(20),
    ADD COLUMN IF NOT EXISTS line_id VARCHAR(100),
    ADD COLUMN IF NOT EXISTS gender VARCHAR(20),
    ADD COLUMN IF NOT EXISTS profile_image_url TEXT,
    ADD COLUMN IF NOT EXISTS hired_date DATE,
    ADD COLUMN IF NOT EXISTS resigned_date DATE;

-- Create index for user_type
CREATE INDEX IF NOT EXISTS idx_users_user_type ON users(user_type);

-- Add check constraint for user_type
ALTER TABLE users 
    ADD CONSTRAINT chk_user_type 
    CHECK (user_type IN ('student', 'staff', 'parent'));

-- Add check constraint for gender
ALTER TABLE users 
    ADD CONSTRAINT chk_gender 
    CHECK (gender IN ('male', 'female', 'other') OR gender IS NULL);

-- Add check constraint for status
ALTER TABLE users 
    DROP CONSTRAINT IF EXISTS chk_status;
    
ALTER TABLE users 
    ADD CONSTRAINT chk_status 
    CHECK (status IN ('active', 'inactive', 'suspended', 'resigned', 'retired'));

COMMENT ON COLUMN users.user_type IS 'ประเภทผู้ใช้: student, staff, parent';
COMMENT ON COLUMN users.title IS 'คำนำหน้า: นาย, นาง, นางสาว, ดร., ศ.ดร.';
COMMENT ON COLUMN users.hired_date IS 'วันที่เริ่มงาน (สำหรับ staff)';

-- ===================================================================
-- 2. Roles Table (บทบาท/ตำแหน่งในระบบ)
-- ===================================================================
CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    name_en VARCHAR(100),
    description TEXT,
    
    -- User Type (staff, student, parent)
    user_type VARCHAR(20) NOT NULL DEFAULT 'staff',
    
    -- Priority/Level for approvals
    level INTEGER DEFAULT 0,
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_roles_code ON roles(code);
CREATE INDEX IF NOT EXISTS idx_roles_user_type ON roles(user_type);
CREATE INDEX IF NOT EXISTS idx_roles_is_active ON roles(is_active);
CREATE INDEX IF NOT EXISTS idx_roles_level ON roles(level);

-- Add check constraint for user_type
ALTER TABLE roles 
    ADD CONSTRAINT roles_user_type_check 
    CHECK (user_type IN ('staff', 'student', 'parent'));

COMMENT ON TABLE roles IS 'บทบาท/ตำแหน่งในระบบ';
COMMENT ON COLUMN roles.user_type IS 'ประเภทผู้ใช้: staff, student, parent';
COMMENT ON COLUMN roles.level IS 'ระดับอำนาจ (ยิ่งสูงยิ่งมีอำนาจมาก)';

-- ===================================================================
-- 3. Permission Tables (Normalized Schema)
-- ===================================================================

-- Permissions Table
CREATE TABLE IF NOT EXISTS permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(100) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    module VARCHAR(50) NOT NULL,
    action VARCHAR(50) NOT NULL,
    scope VARCHAR(50) NOT NULL DEFAULT 'all',
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_permissions_code ON permissions(code);
CREATE INDEX IF NOT EXISTS idx_permissions_module ON permissions(module);
CREATE INDEX IF NOT EXISTS idx_permissions_scope ON permissions(scope);

-- Role Permissions Junction Table
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);

CREATE INDEX IF NOT EXISTS idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission ON role_permissions(permission_id);

COMMENT ON TABLE permissions IS 'สิทธิ์การใช้งานระบบ';
COMMENT ON TABLE role_permissions IS 'ตารางเชื่อมระหว่าง Role และ Permission';

-- ===================================================================
-- 4. User Roles Table (ความสัมพันธ์ระหว่าง User และ Role)
-- ===================================================================
CREATE TABLE IF NOT EXISTS user_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    
    -- Additional Info
    is_primary BOOLEAN DEFAULT false,
    started_at DATE NOT NULL DEFAULT CURRENT_DATE,
    ended_at DATE,
    notes TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(user_id, role_id, started_at)
);

CREATE INDEX IF NOT EXISTS idx_user_roles_user_id ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role_id ON user_roles(role_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_is_primary ON user_roles(is_primary);
CREATE INDEX IF NOT EXISTS idx_user_roles_active ON user_roles(user_id) 
    WHERE ended_at IS NULL;

COMMENT ON TABLE user_roles IS 'ความสัมพันธ์ระหว่างผู้ใช้และบทบาท (Many-to-Many)';
COMMENT ON COLUMN user_roles.is_primary IS 'บทบาทหลักของผู้ใช้';

-- ===================================================================
-- 5. Departments Table (ฝ่าย/แผนก)
-- ===================================================================
CREATE TABLE IF NOT EXISTS departments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    name_en VARCHAR(200),
    description TEXT,
    
    -- Hierarchy
    parent_department_id UUID REFERENCES departments(id),
    
    -- Contact
    phone VARCHAR(20),
    email VARCHAR(255),
    location VARCHAR(200),
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    -- Display Order
    display_order INTEGER DEFAULT 0,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_departments_code ON departments(code);
CREATE INDEX IF NOT EXISTS idx_departments_parent ON departments(parent_department_id);
CREATE INDEX IF NOT EXISTS idx_departments_is_active ON departments(is_active);

COMMENT ON TABLE departments IS 'ฝ่าย/แผนก';
COMMENT ON COLUMN departments.parent_department_id IS 'ฝ่ายแม่ (สำหรับฝ่ายย่อย)';

-- ===================================================================
-- 6. Department Members Table (สมาชิกในฝ่าย)
-- ===================================================================
CREATE TABLE IF NOT EXISTS department_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    department_id UUID NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    
    -- Position in Department
    position VARCHAR(100) NOT NULL,
    
    -- Additional Info
    is_primary_department BOOLEAN DEFAULT false,
    responsibilities TEXT,
    
    -- Time Period
    started_at DATE NOT NULL DEFAULT CURRENT_DATE,
    ended_at DATE,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    UNIQUE(user_id, department_id, started_at)
);

CREATE INDEX IF NOT EXISTS idx_dept_members_user_id ON department_members(user_id);
CREATE INDEX IF NOT EXISTS idx_dept_members_dept_id ON department_members(department_id);
CREATE INDEX IF NOT EXISTS idx_dept_members_position ON department_members(position);
CREATE INDEX IF NOT EXISTS idx_dept_members_active ON department_members(user_id) 
    WHERE ended_at IS NULL;

COMMENT ON TABLE department_members IS 'สมาชิกในแต่ละฝ่าย';
COMMENT ON COLUMN department_members.position IS 'ตำแหน่ง: head, deputy_head, member, coordinator';

-- ===================================================================
-- 7. Update Teaching Assignments Table
-- ===================================================================
ALTER TABLE classes 
    DROP COLUMN IF EXISTS teacher_id;

-- Recreation with better structure
CREATE TABLE IF NOT EXISTS teaching_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    teacher_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    
    -- Teaching Info
    subject VARCHAR(200) NOT NULL,
    grade_level VARCHAR(20),
    
    -- Hours per week
    hours_per_week DECIMAL(5,2),
    
    -- Teacher Type
    teacher_type VARCHAR(50) DEFAULT 'main_teacher',
    
    -- Homeroom Teacher
    is_homeroom_teacher BOOLEAN DEFAULT false,
    
    -- Academic Year/Semester
    academic_year VARCHAR(10) NOT NULL,
    semester VARCHAR(10) NOT NULL,
    
    -- Time Period
    started_at DATE NOT NULL DEFAULT CURRENT_DATE,
    ended_at DATE,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(teacher_id, class_id, academic_year, semester)
);

CREATE INDEX IF NOT EXISTS idx_teaching_teacher_id ON teaching_assignments(teacher_id);
CREATE INDEX IF NOT EXISTS idx_teaching_class_id ON teaching_assignments(class_id);
CREATE INDEX IF NOT EXISTS idx_teaching_academic ON teaching_assignments(academic_year, semester);
CREATE INDEX IF NOT EXISTS idx_teaching_active ON teaching_assignments(teacher_id) 
    WHERE ended_at IS NULL;

COMMENT ON TABLE teaching_assignments IS 'การมอบหมายการสอน (สำหรับครู)';
COMMENT ON COLUMN teaching_assignments.teacher_type IS 'ประเภท: main_teacher, co_teacher, substitute';

-- ===================================================================
-- 8. Staff Info Table (ข้อมูลเฉพาะบุคลากร)
-- ===================================================================
CREATE TABLE IF NOT EXISTS staff_info (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Employment Info (employee_id removed as it's unused)
    employment_type VARCHAR(50),
    
    -- Education
    education_level VARCHAR(100),
    major VARCHAR(200),
    university VARCHAR(200),
    
    -- Teaching License (for teachers)
    teaching_license_number VARCHAR(100),
    teaching_license_expiry DATE,
    
    -- Additional Data
    metadata JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for user_id is sufficient
CREATE INDEX IF NOT EXISTS idx_staff_info_user_id ON staff_info(user_id);

COMMENT ON TABLE staff_info IS 'ข้อมูลเฉพาะบุคลากร';
COMMENT ON COLUMN staff_info.employment_type IS 'ประเภทการจ้าง: permanent, contract, temporary, part_time';

-- ===================================================================
-- 9. Student Info Table (ข้อมูลเฉพาะนักเรียน)
-- ===================================================================
CREATE TABLE IF NOT EXISTS student_info (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Student Info
    student_id VARCHAR(50) UNIQUE NOT NULL,
    grade_level VARCHAR(20),
    class_room VARCHAR(50),
    student_number INTEGER,
    
    -- Parent
    parent_id UUID REFERENCES users(id),
    
    -- Enrollment
    enrollment_date DATE,
    expected_graduation_date DATE,
    
    -- Medical Info
    blood_type VARCHAR(10),
    allergies TEXT,
    medical_conditions TEXT,
    
    -- Additional Data
    metadata JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_student_info_student_id ON student_info(student_id);
CREATE INDEX IF NOT EXISTS idx_student_info_user_id ON student_info(user_id);
CREATE INDEX IF NOT EXISTS idx_student_info_grade ON student_info(grade_level);
CREATE INDEX IF NOT EXISTS idx_student_info_parent ON student_info(parent_id);

COMMENT ON TABLE student_info IS 'ข้อมูลเฉพาะนักเรียน';

-- ===================================================================
-- 10. Parent Info Table (ข้อมูลเฉพาะผู้ปกครอง)
-- ===================================================================
CREATE TABLE IF NOT EXISTS parent_info (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Relationship
    relationship VARCHAR(50),
    
    -- Work Info
    occupation VARCHAR(200),
    workplace VARCHAR(200),
    work_phone VARCHAR(20),
    monthly_income DECIMAL(10,2),
    
    -- Additional Data
    metadata JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_parent_info_user_id ON parent_info(user_id);

COMMENT ON TABLE parent_info IS 'ข้อมูลเฉพาะผู้ปกครอง';
COMMENT ON COLUMN parent_info.relationship IS 'ความสัมพันธ์: father, mother, guardian';

-- ===================================================================
-- 11. Insert Default Role Templates
-- ===================================================================
-- Create role templates without permissions
-- Admin will assign permissions through the UI
INSERT INTO roles (code, name, name_en, description, user_type, level) VALUES
    ('TEACHER', 'ครูผู้สอน', 'Teacher', 'ครูผู้สอนทั่วไป', 'staff', 10),
    ('DEPT_HEAD', 'หัวหน้าฝ่าย', 'Department Head', 'หัวหน้าฝ่าย', 'staff', 50),
    ('VICE_DIRECTOR', 'รองผู้อำนวยการ', 'Vice Director', 'รองผู้อำนวยการ', 'staff', 80),
    ('DIRECTOR', 'ผู้อำนวยการ', 'Director', 'ผู้อำนวยการโรงเรียน', 'staff', 100),
    ('SECRETARY', 'ธุรการ', 'Secretary', 'ธุรการทั่วไป', 'staff', 20),
    ('LIBRARIAN', 'บรรณารักษ์', 'Librarian', 'เจ้าหน้าที่ห้องสมุด', 'staff', 15),
    ('ADMIN', 'ผู้ดูแลระบบ', 'System Admin', 'ผู้ดูแลระบบทั้งหมด', 'staff', 999)
ON CONFLICT (code) DO NOTHING;

-- Note: ADMIN role will be updated by migration 015 to have wildcard (*) permission

-- ===================================================================
-- 12. Insert Default Departments
-- ===================================================================
INSERT INTO departments (code, name, name_en, description, display_order) VALUES
    ('ACADEMIC', 'ฝ่ายวิชาการ', 'Academic Affairs', 'รับผิดชอบด้านการเรียนการสอน', 1),
    ('STUDENT_AFFAIRS', 'ฝ่ายกิจการนักเรียน', 'Student Affairs', 'ดูแลกิจกรรมและพัฒนานักเรียน', 2),
    ('ADMINISTRATION', 'ฝ่ายบริหารทั่วไป', 'Administration', 'งานธุรการและบริหารทั่วไป', 3),
    ('FINANCE', 'ฝ่ายการเงิน', 'Finance', 'รับผิดชอบด้านการเงินและบัญชี', 4),
    ('LIBRARY', 'ห้องสมุด', 'Library', 'จัดการห้องสมุดและสื่อการเรียนรู้', 5)
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 13. Default Permissions
-- ===================================================================
-- Permissions are now managed by the Rust permission registry 
-- (src/permissions/registry.rs) and synced automatically.

-- ===================================================================
-- 14. Create Updated At Trigger Function (if not exists)
-- ===================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ===================================================================
-- 15. Add Updated At Triggers
-- ===================================================================
DROP TRIGGER IF EXISTS update_roles_updated_at ON roles;
CREATE TRIGGER update_roles_updated_at
    BEFORE UPDATE ON roles
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_user_roles_updated_at ON user_roles;
CREATE TRIGGER update_user_roles_updated_at
    BEFORE UPDATE ON user_roles
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_departments_updated_at ON departments;
CREATE TRIGGER update_departments_updated_at
    BEFORE UPDATE ON departments
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_department_members_updated_at ON department_members;
CREATE TRIGGER update_department_members_updated_at
    BEFORE UPDATE ON department_members
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_teaching_assignments_updated_at ON teaching_assignments;
CREATE TRIGGER update_teaching_assignments_updated_at
    BEFORE UPDATE ON teaching_assignments
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_staff_info_updated_at ON staff_info;
CREATE TRIGGER update_staff_info_updated_at
    BEFORE UPDATE ON staff_info
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_student_info_updated_at ON student_info;
CREATE TRIGGER update_student_info_updated_at
    BEFORE UPDATE ON student_info
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_parent_info_updated_at ON parent_info;
CREATE TRIGGER update_parent_info_updated_at
    BEFORE UPDATE ON parent_info
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
