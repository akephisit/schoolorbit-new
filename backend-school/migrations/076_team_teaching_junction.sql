-- ============================================
-- Junction tables: ครูต่อ timetable entry + ครูต่อ classroom_course (team teaching)
-- ============================================

CREATE TABLE IF NOT EXISTS timetable_entry_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entry_id UUID NOT NULL REFERENCES academic_timetable_entries(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_entry_instructor UNIQUE (entry_id, instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_tei_entry ON timetable_entry_instructors(entry_id);
CREATE INDEX IF NOT EXISTS idx_tei_instructor ON timetable_entry_instructors(instructor_id);

COMMENT ON TABLE timetable_entry_instructors IS 'ครูที่ schedule จริงใน timetable entry (ต่อ entry × ครู)';

CREATE TABLE IF NOT EXISTS classroom_course_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    classroom_course_id UUID NOT NULL REFERENCES classroom_courses(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_course_instructor UNIQUE (classroom_course_id, instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_cci_course ON classroom_course_instructors(classroom_course_id);
CREATE INDEX IF NOT EXISTS idx_cci_instructor ON classroom_course_instructors(instructor_id);

COMMENT ON TABLE classroom_course_instructors IS 'ครูของ classroom_course (รองรับ team teaching)';
