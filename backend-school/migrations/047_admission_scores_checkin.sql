-- ================================================================
-- 047: Admission Scores & Check-in System
-- ================================================================

-- ----------------------------------------------------------------
-- 1. วิชาสอบต่อรอบรับสมัคร (Staff กำหนดเอง)
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS admission_exam_subjects (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_period_id UUID NOT NULL REFERENCES admission_periods(id) ON DELETE CASCADE,
    subject_name        VARCHAR(100) NOT NULL,
    subject_code        VARCHAR(50),
    max_score           NUMERIC(8,2)  NOT NULL DEFAULT 100,
    display_order       INT           NOT NULL DEFAULT 0,
    is_active           BOOLEAN       NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    UNIQUE(admission_period_id, subject_code)
);

CREATE INDEX IF NOT EXISTS idx_exam_subjects_period
    ON admission_exam_subjects(admission_period_id);

-- ----------------------------------------------------------------
-- 2. คะแนนแต่ละวิชาต่อใบสมัคร
-- ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS admission_exam_scores (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id   UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    exam_subject_id  UUID NOT NULL REFERENCES admission_exam_subjects(id) ON DELETE CASCADE,
    score            NUMERIC(8,2)  NOT NULL DEFAULT 0,
    recorded_by      UUID REFERENCES users(id),
    recorded_at      TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    UNIQUE(application_id, exam_subject_id)
);

CREATE INDEX IF NOT EXISTS idx_exam_scores_application
    ON admission_exam_scores(application_id);
CREATE INDEX IF NOT EXISTS idx_exam_scores_subject
    ON admission_exam_scores(exam_subject_id);

-- trigger: auto update updated_at
CREATE OR REPLACE TRIGGER trg_exam_scores_updated_at
    BEFORE UPDATE ON admission_exam_scores
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE OR REPLACE TRIGGER trg_exam_subjects_updated_at
    BEFORE UPDATE ON admission_exam_subjects
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ----------------------------------------------------------------
-- 3. เพิ่ม columns ใน admission_selections
--    - study_plan_version_id : สายการเรียนที่จัดให้
--    - assigned_classroom_id : ห้องที่จัดให้ (FK → class_rooms)
--    - checkin_status        : สถานะการรายงานตัว
--    - checked_in_at / by   : ใครยืนยัน เมื่อไหร่
--    - checkin_notes         : หมายเหตุ
-- ----------------------------------------------------------------
ALTER TABLE admission_selections
    ADD COLUMN IF NOT EXISTS study_plan_version_id UUID
        REFERENCES study_plan_versions(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS assigned_classroom_id UUID
        REFERENCES class_rooms(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS checkin_status VARCHAR(20)
        NOT NULL DEFAULT 'pending'
        CHECK (checkin_status IN ('pending','checked_in','absent')),
    ADD COLUMN IF NOT EXISTS checked_in_at   TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS checked_in_by   UUID REFERENCES users(id),
    ADD COLUMN IF NOT EXISTS checkin_notes   TEXT;

CREATE INDEX IF NOT EXISTS idx_selections_checkin_status
    ON admission_selections(checkin_status);
CREATE INDEX IF NOT EXISTS idx_selections_study_plan
    ON admission_selections(study_plan_version_id);
CREATE INDEX IF NOT EXISTS idx_selections_classroom
    ON admission_selections(assigned_classroom_id);

-- ----------------------------------------------------------------
-- 4. ลบ student_user_id ออกจาก admission_selections
--    และสร้าง account ตอน checkin แทน
--    (ยังคง column ไว้เพื่อ backward compat แต่เปลี่ยน logic)
-- ----------------------------------------------------------------
-- student_user_id มีอยู่แล้ว → ใช้ต่อได้เลย ไม่ต้องเพิ่ม

-- ----------------------------------------------------------------
-- 5. View: admission_selections_full
--    ดึงข้อมูลครบในคำสั่งเดียว
-- ----------------------------------------------------------------
CREATE OR REPLACE VIEW admission_selections_full AS
SELECT
    s.id,
    s.application_id,
    s.admission_period_id,
    s.selection_type,
    s.rank,
    s.assigned_grade_level_id,
    s.assigned_classroom_id,
    s.study_plan_version_id,
    s.is_confirmed,
    s.confirmed_at,
    s.confirmation_deadline,
    s.checkin_status,
    s.checked_in_at,
    s.checked_in_by,
    s.checkin_notes,
    s.student_user_id,
    s.notes,
    s.created_at,
    s.updated_at,
    -- applicant info
    CONCAT(COALESCE(a.applicant_title,''), a.applicant_first_name, ' ', a.applicant_last_name)
        AS applicant_name,
    a.application_number,
    a.applicant_national_id,
    a.applicant_gender,
    a.applicant_date_of_birth,
    a.guardian_phone,
    a.guardian_name,
    a.applying_grade_level_id,
    a.total_score AS app_total_score,
    -- grade level
    CASE gl.level_type
        WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
        WHEN 'primary'      THEN CONCAT('ป.', gl.year)
        WHEN 'secondary'    THEN CONCAT('ม.', gl.year)
        ELSE gl.name
    END AS grade_level_name,
    -- applying grade
    CASE agl.level_type
        WHEN 'kindergarten' THEN CONCAT('อ.', agl.year)
        WHEN 'primary'      THEN CONCAT('ป.', agl.year)
        WHEN 'secondary'    THEN CONCAT('ม.', agl.year)
        ELSE agl.name
    END AS applying_grade_level_name,
    -- classroom
    cr.name AS classroom_name,
    cr.code AS classroom_code,
    -- study plan
    sp.name  AS study_plan_name,
    spv.name AS study_plan_version_name,
    -- checker
    CONCAT(COALESCE(cu.title,''), cu.first_name, ' ', cu.last_name) AS checked_in_by_name,
    -- student account
    su.username AS student_username
FROM admission_selections s
JOIN admission_applications  a   ON s.application_id        = a.id
LEFT JOIN grade_levels       gl  ON s.assigned_grade_level_id = gl.id
LEFT JOIN grade_levels       agl ON a.applying_grade_level_id  = agl.id
LEFT JOIN class_rooms        cr  ON s.assigned_classroom_id   = cr.id
LEFT JOIN study_plan_versions spv ON s.study_plan_version_id  = spv.id
LEFT JOIN study_plans        sp  ON spv.study_plan_id         = sp.id
LEFT JOIN users              cu  ON s.checked_in_by           = cu.id
LEFT JOIN users              su  ON s.student_user_id         = su.id;

-- ----------------------------------------------------------------
-- 6. View: admission_score_summary
--    คะแนนรวมต่อใบสมัคร จากตาราง exam_scores จริง
-- ----------------------------------------------------------------
CREATE OR REPLACE VIEW admission_score_summary AS
SELECT
    es.application_id,
    ap.admission_period_id,
    SUM(es.score)                          AS computed_total,
    COUNT(es.id)                           AS subjects_filled,
    jsonb_object_agg(
        subj.subject_code,
        jsonb_build_object(
            'subject_name', subj.subject_name,
            'score',        es.score,
            'max_score',    subj.max_score
        )
    )                                      AS scores_by_subject
FROM admission_exam_scores    es
JOIN admission_exam_subjects  subj ON es.exam_subject_id = subj.id
JOIN admission_applications   ap   ON es.application_id  = ap.id
GROUP BY es.application_id, ap.admission_period_id;
