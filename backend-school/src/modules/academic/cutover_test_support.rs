use sqlx::{migrate::Migrator, PgPool};
use std::{borrow::Cow, error::Error, io};

use super::cutover_preflight::AcademicCorePreflightCode;

type TestSupportResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn migration_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(io::Error::new(io::ErrorKind::InvalidInput, message.into()))
}

pub fn migrations_through(version: i64) -> TestSupportResult<Vec<i64>> {
    let active = sqlx::migrate!("./migrations");
    let all_versions = active
        .iter()
        .map(|migration| migration.version)
        .collect::<Vec<_>>();
    let contiguous = (1..=all_versions.len() as i64).collect::<Vec<_>>();

    if all_versions != contiguous {
        return Err(migration_error(
            "active migration timeline must be contiguous before building a cutover fixture",
        ));
    }
    if version < 1 || !active.version_exists(version) {
        return Err(migration_error(
            "requested cutover fixture migration is outside the active timeline",
        ));
    }

    Ok(all_versions
        .into_iter()
        .filter(|candidate| *candidate <= version)
        .collect())
}

pub async fn apply_migrations_through(pool: &PgPool, version: i64) -> TestSupportResult<()> {
    let versions = migrations_through(version)?;
    let active = sqlx::migrate!("./migrations");
    let migrations = active
        .iter()
        .filter(|migration| versions.binary_search(&migration.version).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    let migrator = Migrator {
        migrations: Cow::Owned(migrations),
        ignore_missing: false,
        locking: false,
        no_tx: active.no_tx,
    };

    migrator.run(pool).await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverFixture {
    Passing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutoverFixtureFault {
    ActiveYearCount,
    ActiveTermCount,
    ActiveYearDate,
    ActiveTermDate,
    InactiveCurrentYear,
    InactiveCurrentTerm,
    YearDateRange,
    TermDateRange,
    TermOutsideYear,
    TermSequence,
    SubjectIdentityBlank,
    SubjectIdentityConflict,
    SubjectVersionOverlap,
    ActivityIdentityConflict,
    ActivityVersionOverlap,
    CurriculumVersion,
    EnrollmentYear,
    EnrollmentStatus,
    HomeroomProgram,
    CourseTermYear,
    SynchronizedActivityPattern,
    ActivityMemberDuplicate,
    AssessmentReference,
    TimetableReference,
    ExamReference,
    SupervisionReference,
    AdmissionProgram,
    AdmissionPlacement,
    PermissionMapping,
}

pub fn all_cutover_fixture_faults() -> &'static [(CutoverFixtureFault, AcademicCorePreflightCode)] {
    use AcademicCorePreflightCode as Code;
    use CutoverFixtureFault as Fault;

    &[
        (Fault::ActiveYearCount, Code::ActiveYearCountInvalid),
        (Fault::ActiveTermCount, Code::ActiveTermCountInvalid),
        (Fault::ActiveYearDate, Code::ActiveYearDateMismatch),
        (Fault::ActiveTermDate, Code::ActiveTermDateMismatch),
        (
            Fault::InactiveCurrentYear,
            Code::InactiveCurrentYearAmbiguous,
        ),
        (
            Fault::InactiveCurrentTerm,
            Code::InactiveCurrentTermAmbiguous,
        ),
        (Fault::YearDateRange, Code::YearDateRangeInvalid),
        (Fault::TermDateRange, Code::TermDateRangeInvalid),
        (Fault::TermOutsideYear, Code::TermOutsideYear),
        (Fault::TermSequence, Code::TermSequenceAmbiguous),
        (Fault::SubjectIdentityBlank, Code::SubjectIdentityBlank),
        (
            Fault::SubjectIdentityConflict,
            Code::SubjectIdentityConflict,
        ),
        (
            Fault::SubjectVersionOverlap,
            Code::SubjectVersionRangeOverlap,
        ),
        (
            Fault::ActivityIdentityConflict,
            Code::ActivityIdentityConflict,
        ),
        (
            Fault::ActivityVersionOverlap,
            Code::ActivityVersionRangeOverlap,
        ),
        (Fault::CurriculumVersion, Code::CurriculumVersionUnresolved),
        (Fault::EnrollmentYear, Code::EnrollmentYearConflict),
        (Fault::EnrollmentStatus, Code::EnrollmentStatusInvalid),
        (Fault::HomeroomProgram, Code::HomeroomProgramUnresolved),
        (Fault::CourseTermYear, Code::CourseTermYearMismatch),
        (
            Fault::SynchronizedActivityPattern,
            Code::SynchronizedActivityPatternConflict,
        ),
        (
            Fault::ActivityMemberDuplicate,
            Code::ActivityMemberDuplicate,
        ),
        (Fault::AssessmentReference, Code::AssessmentReferenceOrphan),
        (Fault::TimetableReference, Code::TimetableReferenceOrphan),
        (Fault::ExamReference, Code::ExamReferenceOrphan),
        (
            Fault::SupervisionReference,
            Code::SupervisionReferenceOrphan,
        ),
        (Fault::AdmissionProgram, Code::AdmissionProgramUnresolved),
        (Fault::PermissionMapping, Code::PermissionMappingUnresolved),
    ]
}

const PASSING_ACADEMIC_CUTOVER_FIXTURE_SQL: &str = r#"
INSERT INTO academic_years (id, year, name, start_date, end_date, is_active)
VALUES
    ('10000000-0000-0000-0000-000000000023', 2023, 'ปีการศึกษา 2023', '2023-05-01', '2024-04-30', false),
    ('10000000-0000-0000-0000-000000000024', 2024, 'ปีการศึกษา 2024', '2024-05-01', '2025-04-30', false),
    ('10000000-0000-0000-0000-000000000025', 2025, 'ปีการศึกษา 2025', '2025-05-01', '2026-04-30', true),
    ('10000000-0000-0000-0000-000000000026', 2026, 'ปีการศึกษา 2026', '2026-05-01', '2027-04-30', false);

INSERT INTO academic_semesters (
    id, academic_year_id, term, name, start_date, end_date, is_active
)
VALUES
    ('11000000-0000-0000-0000-000000000231', '10000000-0000-0000-0000-000000000023', '1', 'ภาคเรียนที่ 1/2023', '2023-05-01', '2023-10-31', false),
    ('11000000-0000-0000-0000-000000000232', '10000000-0000-0000-0000-000000000023', '2', 'ภาคเรียนที่ 2/2023', '2023-11-01', '2024-03-31', false),
    ('11000000-0000-0000-0000-000000000241', '10000000-0000-0000-0000-000000000024', '1', 'ภาคเรียนที่ 1/2024', '2024-05-01', '2024-10-31', false),
    ('11000000-0000-0000-0000-000000000242', '10000000-0000-0000-0000-000000000024', '2', 'ภาคเรียนที่ 2/2024', '2024-11-01', '2025-03-31', false),
    ('11000000-0000-0000-0000-000000000251', '10000000-0000-0000-0000-000000000025', '1', 'ภาคเรียนที่ 1/2025', '2025-05-01', '2025-10-31', true),
    ('11000000-0000-0000-0000-000000000252', '10000000-0000-0000-0000-000000000025', '2', 'ภาคเรียนที่ 2/2025', '2025-11-01', '2026-03-31', false),
    ('11000000-0000-0000-0000-000000000253', '10000000-0000-0000-0000-000000000025', 'SUMMER', 'ภาคฤดูร้อน/2025', '2026-04-01', '2026-04-30', false),
    ('11000000-0000-0000-0000-000000000261', '10000000-0000-0000-0000-000000000026', '1', 'ภาคเรียนที่ 1/2026', '2026-05-01', '2026-10-31', false),
    ('11000000-0000-0000-0000-000000000262', '10000000-0000-0000-0000-000000000026', '2', 'ภาคเรียนที่ 2/2026', '2026-11-01', '2027-03-31', false);

INSERT INTO academic_periods (
    id, academic_year_id, name, start_time, end_time, order_index, applicable_days
)
VALUES (
    '12000000-0000-0000-0000-000000000001',
    '10000000-0000-0000-0000-000000000025',
    'คาบ 1', '08:30', '09:20', 1, 'MON,TUE,WED,THU,FRI'
);

INSERT INTO subjects (
    id, code, name_th, name_en, credit, hours_per_semester, type, group_id,
    start_academic_year_id, term, periods_per_week
)
VALUES
    (
        '20000000-0000-0000-0000-000000000024', 'MATH-CORE', 'คณิตศาสตร์พื้นฐาน',
        'Foundation Mathematics', 1.5, 60, 'BASIC',
        '783a4a9d-9ff1-4eac-b370-06b58daa1eb7',
        '10000000-0000-0000-0000-000000000024', '1', 3
    ),
    (
        '20000000-0000-0000-0000-000000000025', 'math-core', 'คณิตศาสตร์พื้นฐานฉบับปรับปรุง',
        'Foundation Mathematics Revised', 1.5, 60, 'BASIC',
        '783a4a9d-9ff1-4eac-b370-06b58daa1eb7',
        '10000000-0000-0000-0000-000000000025', '1', 3
    ),
    (
        '20000000-0000-0000-0000-000000000026', 'SCI-CORE', 'วิทยาศาสตร์พื้นฐาน',
        'Foundation Science', 1.5, 60, 'BASIC',
        '783a4a9d-9ff1-4eac-b370-06b58daa1eb7',
        '10000000-0000-0000-0000-000000000025', '1', 3
    );

INSERT INTO activity_catalog (
    id, name, activity_type, periods_per_week, scheduling_mode, term,
    grade_level_ids, start_academic_year_id
)
VALUES
    (
        '21000000-0000-0000-0000-000000000024', 'ลูกเสือ', 'scout', 1,
        'synchronized', '1', '["e999190c-d3fc-4124-b787-3445dcb26ee8"]',
        '10000000-0000-0000-0000-000000000024'
    ),
    (
        '21000000-0000-0000-0000-000000000025', 'ลูกเสือ', 'scout', 1,
        'synchronized', '1', '["e999190c-d3fc-4124-b787-3445dcb26ee8"]',
        '10000000-0000-0000-0000-000000000025'
    ),
    (
        '21000000-0000-0000-0000-000000000125', 'แนะแนว', 'guidance', 1,
        'independent', '1', '["e999190c-d3fc-4124-b787-3445dcb26ee8"]',
        '10000000-0000-0000-0000-000000000025'
    );

INSERT INTO study_plans (id, code, name_th, name_en, grade_level_ids)
VALUES (
    '30000000-0000-0000-0000-000000000001', 'GENERAL', 'แผนการเรียนทั่วไป',
    'General Program', '["e999190c-d3fc-4124-b787-3445dcb26ee8"]'
);

INSERT INTO study_plan_versions (
    id, study_plan_id, version_name, start_academic_year_id, end_academic_year_id, is_active
)
VALUES
    (
        '31000000-0000-0000-0000-000000000024',
        '30000000-0000-0000-0000-000000000001', 'ฉบับ 2024',
        '10000000-0000-0000-0000-000000000024',
        '10000000-0000-0000-0000-000000000024', false
    ),
    (
        '31000000-0000-0000-0000-000000000025',
        '30000000-0000-0000-0000-000000000001', 'ฉบับ 2025',
        '10000000-0000-0000-0000-000000000025', NULL, true
    );

INSERT INTO study_plan_subjects (
    id, study_plan_version_id, grade_level_id, term, subject_id, display_order
)
VALUES
    (
        '32000000-0000-0000-0000-000000000024',
        '31000000-0000-0000-0000-000000000024',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', '1',
        '20000000-0000-0000-0000-000000000024', 1
    ),
    (
        '32000000-0000-0000-0000-000000000025',
        '31000000-0000-0000-0000-000000000025',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', '1',
        '20000000-0000-0000-0000-000000000025', 1
    );

INSERT INTO study_plan_version_activities (
    id, study_plan_version_id, activity_catalog_id, term, grade_level_id, display_order
)
VALUES
    (
        '33000000-0000-0000-0000-000000000024',
        '31000000-0000-0000-0000-000000000024',
        '21000000-0000-0000-0000-000000000024', '1',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', 1
    ),
    (
        '33000000-0000-0000-0000-000000000025',
        '31000000-0000-0000-0000-000000000025',
        '21000000-0000-0000-0000-000000000025', '1',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', 1
    ),
    (
        '33000000-0000-0000-0000-000000000125',
        '31000000-0000-0000-0000-000000000025',
        '21000000-0000-0000-0000-000000000125', '1',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', 2
    );

INSERT INTO class_rooms (
    id, code, name, academic_year_id, grade_level_id, room_number,
    study_plan_version_id, capacity
)
VALUES
    (
        '40000000-0000-0000-0000-000000000024', 'M1-1-2024', 'ม.1/1 ปี 2024',
        '10000000-0000-0000-0000-000000000024',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', '1',
        '31000000-0000-0000-0000-000000000024', 40
    ),
    (
        '40000000-0000-0000-0000-000000000025', 'M1-1-2025', 'ม.1/1 ปี 2025',
        '10000000-0000-0000-0000-000000000025',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', '1',
        '31000000-0000-0000-0000-000000000025', 40
    ),
    (
        '40000000-0000-0000-0000-000000000125', 'M1-2-2025', 'ม.1/2 ปี 2025',
        '10000000-0000-0000-0000-000000000025',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', '2',
        '31000000-0000-0000-0000-000000000025', 40
    ),
    (
        '40000000-0000-0000-0000-000000000026', 'M1-1-2026', 'ม.1/1 ปี 2026',
        '10000000-0000-0000-0000-000000000026',
        'e999190c-d3fc-4124-b787-3445dcb26ee8', '1',
        '31000000-0000-0000-0000-000000000025', 40
    );

INSERT INTO users (
    id, email, username, password_hash, first_name, last_name, user_type, status
)
VALUES
    (
        '50000000-0000-0000-0000-000000000001', 'fixture-learner@example.invalid',
        'fixture-learner', 'fixture-not-a-login', 'ผู้เรียน', 'ทดสอบ', 'student', 'active'
    ),
    (
        '50000000-0000-0000-0000-000000000002', 'fixture-teacher@example.invalid',
        'fixture-teacher', 'fixture-not-a-login', 'ครู', 'ทดสอบ', 'staff', 'active'
    ),
    (
        '50000000-0000-0000-0000-000000000003', 'fixture-delegate@example.invalid',
        'fixture-delegate', 'fixture-not-a-login', 'ครูผู้รับมอบ', 'ทดสอบ', 'staff', 'active'
    );

INSERT INTO student_class_enrollments (
    id, student_id, class_room_id, enrollment_date, status, enrollment_type, class_number
)
VALUES
    (
        '51000000-0000-0000-0000-000000000024',
        '50000000-0000-0000-0000-000000000001',
        '40000000-0000-0000-0000-000000000024', '2024-05-01', 'completed', 'regular', 1
    ),
    (
        '51000000-0000-0000-0000-000000000025',
        '50000000-0000-0000-0000-000000000001',
        '40000000-0000-0000-0000-000000000025', '2025-05-01', 'active', 'regular', 1
    ),
    (
        '51000000-0000-0000-0000-000000000026',
        '50000000-0000-0000-0000-000000000001',
        '40000000-0000-0000-0000-000000000026', '2026-05-01', 'active', 'regular', 1
    );

INSERT INTO classroom_courses (
    id, classroom_id, subject_id, academic_semester_id, primary_instructor_id
)
VALUES
    (
        '60000000-0000-0000-0000-000000000024',
        '40000000-0000-0000-0000-000000000024',
        '20000000-0000-0000-0000-000000000024',
        '11000000-0000-0000-0000-000000000241',
        '50000000-0000-0000-0000-000000000002'
    ),
    (
        '60000000-0000-0000-0000-000000000025',
        '40000000-0000-0000-0000-000000000025',
        '20000000-0000-0000-0000-000000000025',
        '11000000-0000-0000-0000-000000000251',
        '50000000-0000-0000-0000-000000000002'
    ),
    (
        '60000000-0000-0000-0000-000000000125',
        '40000000-0000-0000-0000-000000000125',
        '20000000-0000-0000-0000-000000000025',
        '11000000-0000-0000-0000-000000000251',
        '50000000-0000-0000-0000-000000000003'
    );

INSERT INTO activity_slots (
    id, semester_id, registration_type, activity_catalog_id, created_by
)
VALUES
    (
        '70000000-0000-0000-0000-000000000001',
        '11000000-0000-0000-0000-000000000251', 'assigned',
        '21000000-0000-0000-0000-000000000025',
        '50000000-0000-0000-0000-000000000002'
    ),
    (
        '70000000-0000-0000-0000-000000000002',
        '11000000-0000-0000-0000-000000000251', 'assigned',
        '21000000-0000-0000-0000-000000000125',
        '50000000-0000-0000-0000-000000000002'
    );

INSERT INTO activity_slot_classrooms (id, slot_id, classroom_id)
VALUES
    (
        '71000000-0000-0000-0000-000000000001',
        '70000000-0000-0000-0000-000000000001',
        '40000000-0000-0000-0000-000000000025'
    ),
    (
        '71000000-0000-0000-0000-000000000002',
        '70000000-0000-0000-0000-000000000002',
        '40000000-0000-0000-0000-000000000025'
    );

INSERT INTO activity_slot_classroom_assignments (
    id, slot_id, classroom_id, instructor_id
)
VALUES (
    '72000000-0000-0000-0000-000000000001',
    '70000000-0000-0000-0000-000000000002',
    '40000000-0000-0000-0000-000000000025',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO activity_groups (
    id, name, instructor_id, max_capacity, created_by, slot_id
)
VALUES
    (
        '73000000-0000-0000-0000-000000000001', 'กองลูกเสือ ม.1',
        '50000000-0000-0000-0000-000000000002', 40,
        '50000000-0000-0000-0000-000000000002',
        '70000000-0000-0000-0000-000000000001'
    ),
    (
        '73000000-0000-0000-0000-000000000002', 'แนะแนว ม.1/1',
        '50000000-0000-0000-0000-000000000002', 40,
        '50000000-0000-0000-0000-000000000002',
        '70000000-0000-0000-0000-000000000002'
    );

INSERT INTO activity_group_members (
    id, activity_group_id, student_id, result, enrolled_by
)
VALUES (
    '74000000-0000-0000-0000-000000000001',
    '73000000-0000-0000-0000-000000000001',
    '50000000-0000-0000-0000-000000000001', 'pass',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO academic_assessment_plans (
    id, classroom_course_id, academic_semester_id, subject_id, status
)
VALUES (
    '80000000-0000-0000-0000-000000000001',
    '60000000-0000-0000-0000-000000000025',
    '11000000-0000-0000-0000-000000000251',
    '20000000-0000-0000-0000-000000000025', 'saved'
);

INSERT INTO academic_assessment_categories (
    id, plan_id, code, name, max_score, exam_mode, display_order, exam_duration_minutes
)
VALUES (
    '81000000-0000-0000-0000-000000000001',
    '80000000-0000-0000-0000-000000000001',
    'midterm', 'กลางภาค', 12.50, 'in_timetable', 1, 60
);

INSERT INTO academic_assessment_items (
    id, category_id, name, max_score, display_order
)
VALUES
    (
        '82000000-0000-0000-0000-000000000001',
        '81000000-0000-0000-0000-000000000001', 'ข้อเขียน', 7.25, 1
    ),
    (
        '82000000-0000-0000-0000-000000000002',
        '81000000-0000-0000-0000-000000000001', 'ตรวจความพร้อม', 0.10, 2
    );

INSERT INTO academic_timetable_entries (
    id, classroom_course_id, day_of_week, period_id, entry_type, classroom_id,
    academic_semester_id, created_by
)
VALUES (
    '83000000-0000-0000-0000-000000000001',
    '60000000-0000-0000-0000-000000000025', 'MON',
    '12000000-0000-0000-0000-000000000001', 'COURSE',
    '40000000-0000-0000-0000-000000000025',
    '11000000-0000-0000-0000-000000000251',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO academic_exam_rounds (
    id, academic_semester_id, name, status, exam_kind, created_by
)
VALUES (
    '84000000-0000-0000-0000-000000000001',
    '11000000-0000-0000-0000-000000000251', 'สอบกลางภาค', 'draft', 'midterm',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO academic_exam_days (
    id, exam_round_id, exam_date, label, start_time, end_time
)
VALUES (
    '85000000-0000-0000-0000-000000000001',
    '84000000-0000-0000-0000-000000000001', '2025-09-15', 'วันสอบที่ 1',
    '08:30', '16:00'
);

INSERT INTO academic_exam_schedule_items (
    id, exam_round_id, academic_semester_id, assessment_category_id,
    assessment_plan_id, classroom_course_id, classroom_id, subject_id,
    grade_level_id, duration_minutes
)
VALUES (
    '86000000-0000-0000-0000-000000000001',
    '84000000-0000-0000-0000-000000000001',
    '11000000-0000-0000-0000-000000000251',
    '81000000-0000-0000-0000-000000000001',
    '80000000-0000-0000-0000-000000000001',
    '60000000-0000-0000-0000-000000000025',
    '40000000-0000-0000-0000-000000000025',
    '20000000-0000-0000-0000-000000000025',
    'e999190c-d3fc-4124-b787-3445dcb26ee8', 60
);

INSERT INTO supervision_templates (
    id, title, status, rating_min, rating_max, created_by
)
VALUES (
    '87000000-0000-0000-0000-000000000001', 'แบบนิเทศทดสอบ', 'active', 1, 5,
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO supervision_cycles (
    id, academic_year, semester, academic_semester_id, title, template_id,
    starts_at, ends_at, status, created_by
)
VALUES (
    '88000000-0000-0000-0000-000000000001', 2025, '1',
    '11000000-0000-0000-0000-000000000251', 'นิเทศภาคเรียนที่ 1',
    '87000000-0000-0000-0000-000000000001',
    '2025-05-01 00:00:00+00', '2025-10-31 23:59:59+00', 'open',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO supervision_observations (
    id, cycle_id, observed_user_id, requested_by, template_id,
    timetable_entry_id, observed_at, status
)
VALUES (
    '89000000-0000-0000-0000-000000000001',
    '88000000-0000-0000-0000-000000000001',
    '50000000-0000-0000-0000-000000000002',
    '50000000-0000-0000-0000-000000000002',
    '87000000-0000-0000-0000-000000000001',
    '83000000-0000-0000-0000-000000000001',
    '2025-08-25 08:30:00+00', 'planned'
);

INSERT INTO admission_rounds (
    id, academic_year_id, grade_level_id, name, apply_start_date, apply_end_date, status
)
VALUES (
    '90000000-0000-0000-0000-000000000001',
    '10000000-0000-0000-0000-000000000025',
    'e999190c-d3fc-4124-b787-3445dcb26ee8', 'รับสมัคร ม.1 ปี 2025',
    '2025-02-01', '2025-02-28', 'draft'
);

INSERT INTO admission_tracks (
    id, admission_round_id, study_plan_id, name, capacity_override, display_order
)
VALUES (
    '91000000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    '30000000-0000-0000-0000-000000000001', 'แผนทั่วไป', 40, 1
);

INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
VALUES (
    '83500000-0000-0000-0000-000000000001',
    '83000000-0000-0000-0000-000000000001',
    '50000000-0000-0000-0000-000000000002', 'primary'
);

INSERT INTO rooms (id, name_th, code, room_type, capacity, status)
VALUES (
    '92000000-0000-0000-0000-000000000001',
    'ห้องสอบทดสอบ', 'EXAM-FIXTURE', 'GENERAL', 40, 'ACTIVE'
);

INSERT INTO academic_exam_day_room_assignments (
    id, exam_day_id, classroom_id, room_id, created_by
)
VALUES (
    '92100000-0000-0000-0000-000000000001',
    '85000000-0000-0000-0000-000000000001',
    '40000000-0000-0000-0000-000000000025',
    '92000000-0000-0000-0000-000000000001',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO academic_exam_sessions (
    id, exam_schedule_item_id, exam_round_id, exam_day_id,
    starts_at, ends_at, created_by
)
VALUES (
    '92200000-0000-0000-0000-000000000001',
    '86000000-0000-0000-0000-000000000001',
    '84000000-0000-0000-0000-000000000001',
    '85000000-0000-0000-0000-000000000001',
    '09:00', '10:00', '50000000-0000-0000-0000-000000000002'
);

INSERT INTO academic_exam_seat_assignments (
    id, day_room_assignment_id, student_id, seat_number
)
VALUES (
    '92400000-0000-0000-0000-000000000001',
    '92100000-0000-0000-0000-000000000001',
    '50000000-0000-0000-0000-000000000001', 'A01'
);

INSERT INTO academic_question_bank_questions (
    id, subject_id, grade_level_id, owner_user_id, question_type,
    difficulty, points, stem_content, status, created_by
)
VALUES (
    '93000000-0000-0000-0000-000000000001',
    '20000000-0000-0000-0000-000000000025',
    'e999190c-d3fc-4124-b787-3445dcb26ee8',
    '50000000-0000-0000-0000-000000000002', 'single_choice',
    'medium', 0.10, '{"blocks":[]}'::jsonb, 'ready',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO academic_question_bank_choices (
    id, question_id, label, content, is_correct, sort_order
)
VALUES (
    '93100000-0000-0000-0000-000000000001',
    '93000000-0000-0000-0000-000000000001', 'A',
    '{"blocks":[]}'::jsonb, true, 1
);

INSERT INTO admission_applications (
    id, admission_round_id, admission_track_id, application_number,
    national_id, first_name, last_name, status, enrolled_by, enrolled_at,
    created_user_id, national_id_hash
)
VALUES (
    '94000000-0000-0000-0000-000000000001',
    '90000000-0000-0000-0000-000000000001',
    '91000000-0000-0000-0000-000000000001', 'FIXTURE-APP-001',
    'encrypted-fixture-ciphertext', 'ผู้สมัคร', 'ทดสอบ', 'enrolled',
    '50000000-0000-0000-0000-000000000002', '2025-05-01 00:00:00+00',
    '50000000-0000-0000-0000-000000000001',
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
);

INSERT INTO admission_room_assignments (
    id, application_id, class_room_id, rank_in_track, rank_in_room,
    total_score, full_score, assigned_by, student_confirmed
)
VALUES (
    '94100000-0000-0000-0000-000000000001',
    '94000000-0000-0000-0000-000000000001',
    '40000000-0000-0000-0000-000000000025', 1, 1,
    12.50, 12.50, '50000000-0000-0000-0000-000000000002', true
);

INSERT INTO calendar_categories (id, name, color, order_index)
VALUES (
    '95000000-0000-0000-0000-000000000001',
    'กิจกรรมทดสอบ', '#336699', 1
);

INSERT INTO calendar_events (
    id, category_id, title, start_date, end_date, is_public, source_type, source_id,
    created_by
)
VALUES (
    '95100000-0000-0000-0000-000000000001',
    '95000000-0000-0000-0000-000000000001', 'กิจกรรมภาคเรียนทดสอบ',
    '2025-08-01', '2025-08-01', true, 'academic_fixture',
    '83000000-0000-0000-0000-000000000001',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO calendar_event_targets (
    id, event_id, audience_type, class_room_id
)
VALUES (
    '95200000-0000-0000-0000-000000000001',
    '95100000-0000-0000-0000-000000000001', 'student',
    '40000000-0000-0000-0000-000000000025'
);

INSERT INTO certificate_campaigns (
    id, academic_year_id, name, event_date, status, created_by
)
VALUES (
    '96000000-0000-0000-0000-000000000001',
    '10000000-0000-0000-0000-000000000025',
    'เกียรติบัตรทดสอบ', '2025-08-01', 'draft',
    '50000000-0000-0000-0000-000000000002'
);

INSERT INTO role_permissions (role_id, permission_id)
SELECT 'a1b2c957-bf35-47f8-bbf4-8a67ce6b777f', id
FROM permissions
WHERE code = 'academic_structure.read.all';

INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_by, position_code
)
SELECT 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', id,
       '50000000-0000-0000-0000-000000000002', 'head'
FROM permissions
WHERE code = 'academic_curriculum.manage.organization_unit';

INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_by, position_code
)
SELECT 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', id,
	       '50000000-0000-0000-0000-000000000002', 'coordinator'
FROM permissions
WHERE code = 'academic_timetable_today.read.school';

INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_by, position_code
)
SELECT 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2', id,
       '50000000-0000-0000-0000-000000000002', 'member'
FROM permissions
WHERE code = 'academic_promotion.execute.all';

INSERT INTO organization_permission_delegations (
    id, from_user_id, to_user_id, permission_id, organization_unit_id,
    reason, started_at, expires_at
)
SELECT '97000000-0000-0000-0000-000000000001',
       '50000000-0000-0000-0000-000000000002',
       '50000000-0000-0000-0000-000000000003',
       id, 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2',
       'Synthetic academic cutover fixture',
       '2025-05-01 00:00:00+00', '2025-10-31 23:59:59+00'
FROM permissions
WHERE code = 'academic_course_plan.read.all';
"#;

pub async fn seed_academic_cutover_fixture(
    pool: &PgPool,
    fixture: CutoverFixture,
) -> TestSupportResult<()> {
    match fixture {
        CutoverFixture::Passing => {
            sqlx::raw_sql(PASSING_ACADEMIC_CUTOVER_FIXTURE_SQL)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

fn fault_sql(fault: CutoverFixtureFault) -> &'static str {
    use CutoverFixtureFault as Fault;

    match fault {
        Fault::ActiveYearCount => {
            r#"
            UPDATE academic_years
            SET is_active = false
            WHERE id = '10000000-0000-0000-0000-000000000025';
        "#
        }
        Fault::ActiveTermCount => {
            r#"
            UPDATE academic_semesters
            SET is_active = false
            WHERE id = '11000000-0000-0000-0000-000000000251';
        "#
        }
        Fault::ActiveYearDate => {
            r#"
            UPDATE academic_years
            SET start_date = '2025-09-01'
            WHERE id = '10000000-0000-0000-0000-000000000025';
        "#
        }
        Fault::ActiveTermDate => {
            r#"
            UPDATE academic_semesters
            SET start_date = '2025-09-01'
            WHERE id = '11000000-0000-0000-0000-000000000251';
        "#
        }
        Fault::InactiveCurrentYear => {
            r#"
            UPDATE academic_years
            SET start_date = '2025-08-01'
            WHERE id = '10000000-0000-0000-0000-000000000026';
        "#
        }
        Fault::InactiveCurrentTerm => {
            r#"
            UPDATE academic_semesters
            SET start_date = '2025-08-01', end_date = '2025-09-30'
            WHERE id = '11000000-0000-0000-0000-000000000252';
        "#
        }
        Fault::YearDateRange => {
            r#"
            UPDATE academic_years
            SET start_date = '2024-05-01'
            WHERE id = '10000000-0000-0000-0000-000000000023';
        "#
        }
        Fault::TermDateRange => {
            r#"
            UPDATE academic_semesters
            SET start_date = '2024-01-01'
            WHERE id = '11000000-0000-0000-0000-000000000231';
        "#
        }
        Fault::TermOutsideYear => {
            r#"
            UPDATE academic_semesters
            SET end_date = '2024-05-01'
            WHERE id = '11000000-0000-0000-0000-000000000232';
        "#
        }
        Fault::TermSequence => {
            r#"
            UPDATE academic_semesters
            SET term = ' 1 '
            WHERE id = '11000000-0000-0000-0000-000000000252';
        "#
        }
        Fault::SubjectIdentityBlank => {
            r#"
            UPDATE subjects
            SET code = ' '
            WHERE id = '20000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::SubjectIdentityConflict => {
            r#"
            INSERT INTO subjects (
                id, code, name_th, name_en, credit, hours_per_semester, type,
                group_id, start_academic_year_id, term, periods_per_week
            )
            VALUES (
                '20000000-0000-0000-0000-000000000125', ' math-core ',
                'คณิตศาสตร์รายการซ้ำ', 'Duplicate Mathematics', 1.5, 60, 'BASIC',
                '783a4a9d-9ff1-4eac-b370-06b58daa1eb7',
                '10000000-0000-0000-0000-000000000025', '1', 3
            );
        "#
        }
        Fault::SubjectVersionOverlap => {
            r#"
            INSERT INTO academic_years (
                id, year, name, start_date, end_date, is_active
            )
            VALUES (
                '10000000-0000-0000-0000-000000000124', 20245,
                'ช่วง version สังเคราะห์', '2025-04-15', '2025-04-30', false
            );
            INSERT INTO subjects (
                id, code, name_th, name_en, credit, hours_per_semester, type,
                group_id, start_academic_year_id, term, periods_per_week
            )
            VALUES (
                '20000000-0000-0000-0000-000000000224', ' math-core ',
                'คณิตศาสตร์ช่วงทับซ้อน', 'Overlapping Mathematics', 1.5, 60, 'BASIC',
                '783a4a9d-9ff1-4eac-b370-06b58daa1eb7',
                '10000000-0000-0000-0000-000000000124', '1', 3
            );
        "#
        }
        Fault::ActivityVersionOverlap => {
            r#"
            UPDATE academic_years
            SET end_date = '2025-05-15'
            WHERE id = '10000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::ActivityIdentityConflict => {
            r#"
            INSERT INTO activity_catalog (
                id, name, activity_type, periods_per_week, scheduling_mode, term,
                grade_level_ids, start_academic_year_id
            )
            VALUES (
                '21000000-0000-0000-0000-000000000225', ' ลูกเสือ ', 'scout', 1,
                'synchronized', '1',
                '["e999190c-d3fc-4124-b787-3445dcb26ee8"]',
                '10000000-0000-0000-0000-000000000025'
            );
        "#
        }
        Fault::CurriculumVersion => {
            r#"
            UPDATE study_plan_subjects
            SET subject_id = '20000000-0000-0000-0000-000000000025'
            WHERE id = '32000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::EnrollmentYear => {
            r#"
            INSERT INTO class_rooms (
                id, code, name, academic_year_id, grade_level_id, room_number,
                study_plan_version_id, capacity
            )
            VALUES (
                '40000000-0000-0000-0000-000000000125', 'M1-2-2025', 'ม.1/2 ปี 2025',
                '10000000-0000-0000-0000-000000000025',
                'e999190c-d3fc-4124-b787-3445dcb26ee8', '2',
                '31000000-0000-0000-0000-000000000025', 40
            );
            INSERT INTO student_class_enrollments (
                id, student_id, class_room_id, enrollment_date, status, enrollment_type
            )
            VALUES (
                '51000000-0000-0000-0000-000000000125',
                '50000000-0000-0000-0000-000000000001',
                '40000000-0000-0000-0000-000000000125', '2025-05-01', 'active', 'regular'
            );
        "#
        }
        Fault::EnrollmentStatus => {
            r#"
            UPDATE student_class_enrollments
            SET status = 'active'
            WHERE id = '51000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::HomeroomProgram => {
            r#"
            UPDATE class_rooms
            SET study_plan_version_id = '31000000-0000-0000-0000-000000000024'
            WHERE id = '40000000-0000-0000-0000-000000000026';
        "#
        }
        Fault::CourseTermYear => {
            r#"
            UPDATE classroom_courses
            SET academic_semester_id = '11000000-0000-0000-0000-000000000251'
            WHERE id = '60000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::SynchronizedActivityPattern => {
            r#"
            INSERT INTO activity_slot_classroom_assignments (
                id, slot_id, classroom_id, instructor_id
            )
            VALUES (
                '72000000-0000-0000-0000-000000000002',
                '70000000-0000-0000-0000-000000000001',
                '40000000-0000-0000-0000-000000000025',
                '50000000-0000-0000-0000-000000000002'
            );
        "#
        }
        Fault::ActivityMemberDuplicate => {
            r#"
            ALTER TABLE activity_group_members
            DROP CONSTRAINT unique_student_per_group;
            INSERT INTO activity_group_members (
                id, activity_group_id, student_id, result, enrolled_by
            )
            VALUES (
                '74000000-0000-0000-0000-000000000002',
                '73000000-0000-0000-0000-000000000001',
                '50000000-0000-0000-0000-000000000001', 'pass',
                '50000000-0000-0000-0000-000000000002'
            );
        "#
        }
        Fault::AssessmentReference => {
            r#"
            UPDATE academic_assessment_plans
            SET classroom_course_id = '60000000-0000-0000-0000-000000000024'
            WHERE id = '80000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::TimetableReference => {
            r#"
            UPDATE academic_timetable_entries
            SET academic_semester_id = '11000000-0000-0000-0000-000000000241'
            WHERE id = '83000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::ExamReference => {
            r#"
            UPDATE class_rooms
            SET academic_year_id = '10000000-0000-0000-0000-000000000023'
            WHERE id = '40000000-0000-0000-0000-000000000025';
        "#
        }
        Fault::SupervisionReference => {
            r#"
            UPDATE supervision_cycles
            SET semester = '2'
            WHERE id = '88000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::AdmissionProgram => {
            r#"
            UPDATE admission_rounds
            SET academic_year_id = '10000000-0000-0000-0000-000000000023'
            WHERE id = '90000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::AdmissionPlacement => {
            r#"
            UPDATE admission_applications
            SET created_user_id = '50000000-0000-0000-0000-000000000003'
            WHERE id = '94000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::PermissionMapping => {
            r#"
            INSERT INTO permissions (
                id, code, name, module, action, scope, description
            )
            VALUES (
                'f2000000-0000-0000-0000-000000000001',
                'academic_structure.archive.all', 'สิทธิ์เดิมที่ไม่มี mapping',
                'academic_structure', 'archive', 'all', 'Synthetic cutover fixture permission'
            );
            INSERT INTO role_permissions (role_id, permission_id)
            VALUES (
                'a1b2c957-bf35-47f8-bbf4-8a67ce6b777f',
                'f2000000-0000-0000-0000-000000000001'
            );
        "#
        }
    }
}

fn repair_sql(fault: CutoverFixtureFault) -> &'static str {
    use CutoverFixtureFault as Fault;

    match fault {
        Fault::ActiveYearCount => {
            r#"
            UPDATE academic_years SET is_active = true
            WHERE id = '10000000-0000-0000-0000-000000000025';
        "#
        }
        Fault::ActiveTermCount => {
            r#"
            UPDATE academic_semesters SET is_active = true
            WHERE id = '11000000-0000-0000-0000-000000000251';
        "#
        }
        Fault::ActiveYearDate => {
            r#"
            UPDATE academic_years SET start_date = '2025-05-01'
            WHERE id = '10000000-0000-0000-0000-000000000025';
        "#
        }
        Fault::ActiveTermDate => {
            r#"
            UPDATE academic_semesters SET start_date = '2025-05-01'
            WHERE id = '11000000-0000-0000-0000-000000000251';
        "#
        }
        Fault::InactiveCurrentYear => {
            r#"
            UPDATE academic_years SET start_date = '2026-05-01'
            WHERE id = '10000000-0000-0000-0000-000000000026';
        "#
        }
        Fault::InactiveCurrentTerm => {
            r#"
            UPDATE academic_semesters SET start_date = '2025-11-01', end_date = '2026-03-31'
            WHERE id = '11000000-0000-0000-0000-000000000252';
        "#
        }
        Fault::YearDateRange => {
            r#"
            UPDATE academic_years SET start_date = '2023-05-01'
            WHERE id = '10000000-0000-0000-0000-000000000023';
        "#
        }
        Fault::TermDateRange => {
            r#"
            UPDATE academic_semesters SET start_date = '2023-05-01'
            WHERE id = '11000000-0000-0000-0000-000000000231';
        "#
        }
        Fault::TermOutsideYear => {
            r#"
            UPDATE academic_semesters SET end_date = '2024-03-31'
            WHERE id = '11000000-0000-0000-0000-000000000232';
        "#
        }
        Fault::TermSequence => {
            r#"
            UPDATE academic_semesters SET term = '2'
            WHERE id = '11000000-0000-0000-0000-000000000252';
        "#
        }
        Fault::SubjectIdentityBlank => {
            r#"
            UPDATE subjects SET code = 'MATH-CORE'
            WHERE id = '20000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::SubjectIdentityConflict => {
            r#"
            DELETE FROM subjects
            WHERE id = '20000000-0000-0000-0000-000000000125';
        "#
        }
        Fault::SubjectVersionOverlap => {
            r#"
            DELETE FROM subjects
            WHERE id = '20000000-0000-0000-0000-000000000224';
            DELETE FROM academic_years
            WHERE id = '10000000-0000-0000-0000-000000000124';
        "#
        }
        Fault::ActivityVersionOverlap => {
            r#"
            UPDATE academic_years SET end_date = '2025-04-30'
            WHERE id = '10000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::ActivityIdentityConflict => {
            r#"
            DELETE FROM activity_catalog
            WHERE id = '21000000-0000-0000-0000-000000000225';
        "#
        }
        Fault::CurriculumVersion => {
            r#"
            UPDATE study_plan_subjects
            SET subject_id = '20000000-0000-0000-0000-000000000024'
            WHERE id = '32000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::EnrollmentYear => {
            r#"
            DELETE FROM student_class_enrollments
            WHERE id = '51000000-0000-0000-0000-000000000125';
            DELETE FROM class_rooms
            WHERE id = '40000000-0000-0000-0000-000000000125';
        "#
        }
        Fault::EnrollmentStatus => {
            r#"
            UPDATE student_class_enrollments SET status = 'completed'
            WHERE id = '51000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::HomeroomProgram => {
            r#"
            UPDATE class_rooms
            SET study_plan_version_id = '31000000-0000-0000-0000-000000000025'
            WHERE id = '40000000-0000-0000-0000-000000000026';
        "#
        }
        Fault::CourseTermYear => {
            r#"
            UPDATE classroom_courses
            SET academic_semester_id = '11000000-0000-0000-0000-000000000241'
            WHERE id = '60000000-0000-0000-0000-000000000024';
        "#
        }
        Fault::SynchronizedActivityPattern => {
            r#"
            DELETE FROM activity_slot_classroom_assignments
            WHERE id = '72000000-0000-0000-0000-000000000002';
        "#
        }
        Fault::ActivityMemberDuplicate => {
            r#"
            DELETE FROM activity_group_members
            WHERE id = '74000000-0000-0000-0000-000000000002';
            ALTER TABLE activity_group_members
            ADD CONSTRAINT unique_student_per_group
            UNIQUE (activity_group_id, student_id);
        "#
        }
        Fault::AssessmentReference => {
            r#"
            UPDATE academic_assessment_plans
            SET classroom_course_id = '60000000-0000-0000-0000-000000000025'
            WHERE id = '80000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::TimetableReference => {
            r#"
            UPDATE academic_timetable_entries
            SET academic_semester_id = '11000000-0000-0000-0000-000000000251'
            WHERE id = '83000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::ExamReference => {
            r#"
            UPDATE class_rooms
            SET academic_year_id = '10000000-0000-0000-0000-000000000025'
            WHERE id = '40000000-0000-0000-0000-000000000025';
        "#
        }
        Fault::SupervisionReference => {
            r#"
            UPDATE supervision_cycles SET semester = '1'
            WHERE id = '88000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::AdmissionProgram => {
            r#"
            UPDATE admission_rounds
            SET academic_year_id = '10000000-0000-0000-0000-000000000025'
            WHERE id = '90000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::AdmissionPlacement => {
            r#"
            UPDATE admission_applications
            SET created_user_id = '50000000-0000-0000-0000-000000000001'
            WHERE id = '94000000-0000-0000-0000-000000000001';
        "#
        }
        Fault::PermissionMapping => {
            r#"
            DELETE FROM role_permissions
            WHERE permission_id = 'f2000000-0000-0000-0000-000000000001';
            DELETE FROM permissions
            WHERE id = 'f2000000-0000-0000-0000-000000000001';
        "#
        }
    }
}

pub async fn apply_cutover_fixture_fault(
    pool: &PgPool,
    fault: CutoverFixtureFault,
) -> TestSupportResult<()> {
    sqlx::raw_sql(fault_sql(fault)).execute(pool).await?;
    Ok(())
}

pub async fn repair_cutover_fixture_fault(
    pool: &PgPool,
    fault: CutoverFixtureFault,
) -> TestSupportResult<()> {
    sqlx::raw_sql(repair_sql(fault)).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::migrations_through;

    #[test]
    fn legacy_fixture_migrations_are_contiguous_through_040() {
        let versions = migrations_through(40).expect("legacy migration range must be valid");

        assert_eq!(versions.len(), 40);
        assert_eq!(versions.first(), Some(&1));
        assert_eq!(versions.last(), Some(&40));
    }

    #[test]
    fn migration_helper_rejects_a_version_outside_the_active_timeline() {
        assert!(migrations_through(0).is_err());
        assert!(migrations_through(i64::MAX).is_err());
    }
}
