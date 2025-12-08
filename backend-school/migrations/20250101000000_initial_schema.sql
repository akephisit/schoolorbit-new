-- School Database Initial Schema
-- This SQL will be run when creating a new school database

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Admin Users (for school administrators)
CREATE TABLE admin_users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255),
    national_id_hash VARCHAR(255) UNIQUE NOT NULL,
    national_id_encrypted TEXT NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(50) DEFAULT 'admin',
    is_active BOOLEAN DEFAULT true,
    last_login TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Students
CREATE TABLE students (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    student_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    national_id_hash VARCHAR(255) UNIQUE NOT NULL,
    national_id_encrypted TEXT NOT NULL,
    grade VARCHAR(50),
    class_id UUID,
    parent_name VARCHAR(255),
    parent_phone VARCHAR(20),
    address TEXT,
    birth_date DATE,
    gender VARCHAR(10),
    photo_url TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Teachers
CREATE TABLE teachers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    teacher_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    national_id_hash VARCHAR(255) UNIQUE NOT NULL,
    national_id_encrypted TEXT NOT NULL,
    email VARCHAR(255),
    phone VARCHAR(20),
    subject VARCHAR(100),
    department VARCHAR(100),
    photo_url TEXT,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Classes
CREATE TABLE classes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    grade VARCHAR(50),
    section VARCHAR(50),
    room VARCHAR(50),
    teacher_id UUID REFERENCES teachers(id) ON DELETE SET NULL,
    academic_year VARCHAR(10),
    capacity INT DEFAULT 40,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Attendance
CREATE TABLE attendance (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID REFERENCES classes(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    status VARCHAR(20) NOT NULL, -- present, absent, late, sick, excused
    check_in_time TIME,
    check_out_time TIME,
    note TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(student_id, date)
);

-- Grades
CREATE TABLE grades (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID REFERENCES classes(id) ON DELETE CASCADE,
    subject VARCHAR(100) NOT NULL,
    semester VARCHAR(10),
    academic_year VARCHAR(10),
    score DECIMAL(5,2),
    grade VARCHAR(5),
    note TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Announcements
CREATE TABLE announcements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    author_id UUID REFERENCES admin_users(id),
    target_audience VARCHAR(50), -- all, teachers, students, parents
    is_pinned BOOLEAN DEFAULT false,
    published_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Add foreign key to students
ALTER TABLE students ADD CONSTRAINT fk_students_class
    FOREIGN KEY (class_id) REFERENCES classes(id) ON DELETE SET NULL;

-- Indexes for performance
CREATE INDEX idx_students_class ON students(class_id);
CREATE INDEX idx_students_grade ON students(grade);
CREATE INDEX idx_students_active ON students(is_active);
CREATE INDEX idx_teachers_subject ON teachers(subject);
CREATE INDEX idx_classes_grade ON classes(grade);
CREATE INDEX idx_classes_teacher ON classes(teacher_id);
CREATE INDEX idx_attendance_student ON attendance(student_id);
CREATE INDEX idx_attendance_date ON attendance(date);
CREATE INDEX idx_grades_student ON grades(student_id);
CREATE INDEX idx_grades_subject ON grades(subject);

-- Views for common queries
CREATE VIEW active_students AS
SELECT * FROM students WHERE is_active = true;

CREATE VIEW active_teachers AS
SELECT * FROM teachers WHERE is_active = true;

CREATE VIEW active_classes AS
SELECT * FROM classes WHERE is_active = true;

-- Insert default admin (created from school creation params)
-- Will be replaced by actual admin data from API
