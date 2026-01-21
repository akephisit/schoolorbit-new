-- Create classroom_courses table for assigning subjects to classrooms per semester

CREATE TABLE classroom_courses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    classroom_id UUID NOT NULL REFERENCES classrooms(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE RESTRICT,
    
    -- Optional override instructor (if different from global subject assignment or for specific class)
    primary_instructor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    
    -- Specific settings for this class-subject (e.g. custom schedule, override credits)
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure unique subject per classroom per semester
    UNIQUE (classroom_id, subject_id, academic_semester_id)
);

CREATE INDEX idx_cc_classroom ON classroom_courses(classroom_id);
CREATE INDEX idx_cc_semester ON classroom_courses(academic_semester_id);
CREATE INDEX idx_cc_subject ON classroom_courses(subject_id);
