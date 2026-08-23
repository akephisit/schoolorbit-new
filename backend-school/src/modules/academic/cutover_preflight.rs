use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedAcademicYearStatus {
    Planning,
    Active,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedAcademicTermStatus {
    Planning,
    Active,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AcademicCorePreflightCode {
    ActiveTermCountInvalid,
    ActiveTermDateMismatch,
    ActiveYearCountInvalid,
    ActiveYearDateMismatch,
    ActivityIdentityConflict,
    ActivityMemberDuplicate,
    ActivityVersionRangeOverlap,
    AdmissionProgramUnresolved,
    AssessmentReferenceOrphan,
    CourseTermYearMismatch,
    CurriculumVersionUnresolved,
    EnrollmentStatusInvalid,
    EnrollmentYearConflict,
    ExamReferenceOrphan,
    HistoricalResultsUnavailable,
    HomeroomProgramUnresolved,
    InactiveCurrentTermAmbiguous,
    InactiveCurrentYearAmbiguous,
    PermissionMappingUnresolved,
    SubjectIdentityBlank,
    SubjectIdentityConflict,
    SubjectVersionRangeOverlap,
    SupervisionReferenceOrphan,
    SynchronizedActivityPatternConflict,
    TermDateRangeInvalid,
    TermOutsideYear,
    TermSequenceAmbiguous,
    TimetableReferenceOrphan,
    YearDateRangeInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, thiserror::Error)]
pub enum PreflightError {
    #[error("ACADEMIC_CORE_PREFLIGHT_READ_ONLY_TRANSACTION_FAILED")]
    ReadOnlyTransactionFailed,
    #[error("ACADEMIC_CORE_PREFLIGHT_QUERY_FAILED")]
    QueryFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcademicCorePreflightFinding {
    pub code: AcademicCorePreflightCode,
    pub severity: PreflightSeverity,
    pub affected_count: i64,
    pub resource_ids: Vec<Uuid>,
    pub guidance_th: String,
}

impl AcademicCorePreflightFinding {
    pub fn new(
        code: AcademicCorePreflightCode,
        severity: PreflightSeverity,
        affected_count: i64,
        mut resource_ids: Vec<Uuid>,
        guidance_th: String,
    ) -> Self {
        resource_ids.truncate(20);
        Self {
            code,
            severity,
            affected_count,
            resource_ids,
            guidance_th,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcademicCorePreflightReport {
    pub schema: String,
    pub generated_at: DateTime<Utc>,
    pub can_cut_over: bool,
    pub source_counts: BTreeMap<String, i64>,
    pub expected_target_counts: BTreeMap<String, i64>,
    pub findings: Vec<AcademicCorePreflightFinding>,
}

pub fn build_preflight_report(
    schema: String,
    source_counts: BTreeMap<String, i64>,
    expected_target_counts: BTreeMap<String, i64>,
    findings: Vec<AcademicCorePreflightFinding>,
) -> AcademicCorePreflightReport {
    let can_cut_over = findings
        .iter()
        .all(|finding| finding.severity != PreflightSeverity::Blocking);

    AcademicCorePreflightReport {
        schema,
        generated_at: Utc::now(),
        can_cut_over,
        source_counts,
        expected_target_counts,
        findings,
    }
}

pub fn preflight_check_codes() -> &'static [AcademicCorePreflightCode] {
    use AcademicCorePreflightCode as Code;

    &[
        Code::ActiveYearCountInvalid,
        Code::ActiveTermCountInvalid,
        Code::ActiveYearDateMismatch,
        Code::ActiveTermDateMismatch,
        Code::InactiveCurrentYearAmbiguous,
        Code::InactiveCurrentTermAmbiguous,
        Code::YearDateRangeInvalid,
        Code::TermDateRangeInvalid,
        Code::TermOutsideYear,
        Code::TermSequenceAmbiguous,
        Code::SubjectIdentityBlank,
        Code::SubjectIdentityConflict,
        Code::SubjectVersionRangeOverlap,
        Code::ActivityIdentityConflict,
        Code::ActivityVersionRangeOverlap,
        Code::CurriculumVersionUnresolved,
        Code::EnrollmentYearConflict,
        Code::EnrollmentStatusInvalid,
        Code::HomeroomProgramUnresolved,
        Code::CourseTermYearMismatch,
        Code::SynchronizedActivityPatternConflict,
        Code::ActivityMemberDuplicate,
        Code::AssessmentReferenceOrphan,
        Code::TimetableReferenceOrphan,
        Code::ExamReferenceOrphan,
        Code::SupervisionReferenceOrphan,
        Code::AdmissionProgramUnresolved,
        Code::PermissionMappingUnresolved,
        Code::HistoricalResultsUnavailable,
    ]
}

struct FindingCheck {
    code: AcademicCorePreflightCode,
    severity: PreflightSeverity,
    sql: &'static str,
    uses_cutover_date: bool,
    guidance_th: &'static str,
}

const SOURCE_COUNT_QUERIES: &[(&str, &str)] = &[
    (
        "academicYears",
        "SELECT COUNT(*)::bigint FROM academic_years",
    ),
    (
        "academicTerms",
        "SELECT COUNT(*)::bigint FROM academic_semesters",
    ),
    ("subjects", "SELECT COUNT(*)::bigint FROM subjects"),
    (
        "activities",
        "SELECT COUNT(*)::bigint FROM activity_catalog",
    ),
    ("curricula", "SELECT COUNT(*)::bigint FROM study_plans"),
    (
        "curriculumVersions",
        "SELECT COUNT(*)::bigint FROM study_plan_versions",
    ),
    (
        "courseRequirements",
        "SELECT COUNT(*)::bigint FROM study_plan_subjects",
    ),
    (
        "activityRequirements",
        "SELECT COUNT(*)::bigint FROM study_plan_version_activities",
    ),
    ("homerooms", "SELECT COUNT(*)::bigint FROM class_rooms"),
    (
        "enrollments",
        "SELECT COUNT(*)::bigint FROM student_class_enrollments",
    ),
    (
        "classroomCourses",
        "SELECT COUNT(*)::bigint FROM classroom_courses",
    ),
    (
        "activitySlots",
        "SELECT COUNT(*)::bigint FROM activity_slots",
    ),
    (
        "activityGroups",
        "SELECT COUNT(*)::bigint FROM activity_groups",
    ),
    (
        "activityMembers",
        "SELECT COUNT(*)::bigint FROM activity_group_members",
    ),
    (
        "assessmentPlans",
        "SELECT COUNT(*)::bigint FROM academic_assessment_plans",
    ),
    (
        "assessmentItems",
        "SELECT COUNT(*)::bigint FROM academic_assessment_items",
    ),
    (
        "timetableEntries",
        "SELECT COUNT(*)::bigint FROM academic_timetable_entries",
    ),
    (
        "examItems",
        "SELECT COUNT(*)::bigint FROM academic_exam_schedule_items",
    ),
];

const EXPECTED_TARGET_COUNT_QUERIES: &[(&str, &str)] = &[
    (
        "academicYears",
        "SELECT COUNT(*)::bigint FROM academic_years",
    ),
    (
        "academicTerms",
        "SELECT COUNT(*)::bigint FROM academic_semesters",
    ),
    (
        "stableSubjects",
        r#"SELECT COUNT(DISTINCT lower(regexp_replace(btrim(normalize(code, NFKC)), '\s+', ' ', 'g')))::bigint
           FROM subjects
           WHERE btrim(normalize(code, NFKC)) <> ''"#,
    ),
    ("subjectVersions", "SELECT COUNT(*)::bigint FROM subjects"),
    (
        "stableActivities",
        r#"SELECT COUNT(DISTINCT (lower(btrim(activity_type)), lower(regexp_replace(btrim(normalize(name, NFKC)), '\s+', ' ', 'g'))))::bigint
           FROM activity_catalog"#,
    ),
    ("activityVersions", "SELECT COUNT(*)::bigint FROM activity_catalog"),
    ("curricula", "SELECT COUNT(*)::bigint FROM study_plans"),
    ("curriculumVersions", "SELECT COUNT(*)::bigint FROM study_plan_versions"),
    (
        "curriculumCourseRequirements",
        "SELECT COUNT(*)::bigint FROM study_plan_subjects",
    ),
    (
        "curriculumActivityRequirements",
        "SELECT COUNT(*)::bigint FROM study_plan_version_activities",
    ),
    (
        "programs",
        "SELECT COUNT(*)::bigint FROM study_plan_versions",
    ),
    (
        "studentAcademicYears",
        r#"SELECT COUNT(DISTINCT (enrollment.student_id, homeroom.academic_year_id))::bigint
           FROM student_class_enrollments enrollment
           JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id"#,
    ),
    (
        "homeroomPlacements",
        "SELECT COUNT(*)::bigint FROM student_class_enrollments",
    ),
    (
        "courseOfferings",
        "SELECT COUNT(DISTINCT (academic_semester_id, subject_id))::bigint FROM classroom_courses",
    ),
    ("courseGroups", "SELECT COUNT(*)::bigint FROM classroom_courses"),
    ("activityOfferings", "SELECT COUNT(*)::bigint FROM activity_slots"),
    (
        "learningGroups",
        "SELECT ((SELECT COUNT(*) FROM classroom_courses) + (SELECT COUNT(*) FROM activity_groups))::bigint",
    ),
    (
        "groupStudents",
        r#"SELECT (
               (SELECT COUNT(*)
                FROM classroom_courses course
                JOIN student_class_enrollments enrollment
                  ON enrollment.class_room_id = course.classroom_id
                 AND enrollment.status IN ('active', 'completed', 'transferred'))
               +
               (SELECT COUNT(*) FROM (
                    SELECT DISTINCT activity_group_id, student_id
                    FROM activity_group_members
               ) activity_roster)
           )::bigint"#,
    ),
    (
        "assessmentPlans",
        "SELECT COUNT(*)::bigint FROM academic_assessment_plans",
    ),
    (
        "assessmentItems",
        "SELECT COUNT(*)::bigint FROM academic_assessment_items",
    ),
    (
        "timetableEntries",
        "SELECT COUNT(*)::bigint FROM academic_timetable_entries",
    ),
    (
        "examItems",
        "SELECT COUNT(*)::bigint FROM academic_exam_schedule_items",
    ),
];

fn finding_checks() -> [FindingCheck; 29] {
    use AcademicCorePreflightCode as Code;
    use PreflightSeverity::{Blocking, Warning};

    [
        FindingCheck {
            code: Code::ActiveYearCountInvalid,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_years WHERE is_active IS TRUE
                    )
                    SELECT CASE WHEN COUNT(*) = 1 THEN 0 ELSE GREATEST(COUNT(*), 1::bigint) END,
                           COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ต้องมีปีการศึกษาที่ active เพียง 1 ปี ก่อนย้ายข้อมูล",
        },
        FindingCheck {
            code: Code::ActiveTermCountInvalid,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_semesters WHERE is_active IS TRUE
                    )
                    SELECT CASE WHEN COUNT(*) = 1 THEN 0 ELSE GREATEST(COUNT(*), 1::bigint) END,
                           COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ต้องมีภาคเรียนที่ active เพียง 1 ภาคเรียน ก่อนย้ายข้อมูล",
        },
        FindingCheck {
            code: Code::ActiveYearDateMismatch,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_years
                        WHERE is_active IS TRUE AND NOT (start_date <= $1 AND $1 <= end_date)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: true,
            guidance_th: "แก้ช่วงวันที่ของปี active ให้ครอบคลุมวัน cutover",
        },
        FindingCheck {
            code: Code::ActiveTermDateMismatch,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_semesters
                        WHERE is_active IS TRUE AND NOT (start_date <= $1 AND $1 <= end_date)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: true,
            guidance_th: "แก้ช่วงวันที่ของภาคเรียน active ให้ครอบคลุมวัน cutover",
        },
        FindingCheck {
            code: Code::InactiveCurrentYearAmbiguous,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_years
                        WHERE is_active IS NOT TRUE AND start_date <= $1 AND $1 <= end_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: true,
            guidance_th: "ปีที่ครอบคลุมวัน cutover ต้องระบุ active ให้ชัดเจน",
        },
        FindingCheck {
            code: Code::InactiveCurrentTermAmbiguous,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_semesters
                        WHERE is_active IS NOT TRUE AND start_date <= $1 AND $1 <= end_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: true,
            guidance_th: "ภาคเรียนที่ครอบคลุมวัน cutover ต้องระบุ active ให้ชัดเจน",
        },
        FindingCheck {
            code: Code::YearDateRangeInvalid,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_years WHERE start_date > end_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "วันที่เริ่มปีการศึกษาต้องไม่อยู่หลังวันที่สิ้นสุด",
        },
        FindingCheck {
            code: Code::TermDateRangeInvalid,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM academic_semesters WHERE start_date > end_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "วันที่เริ่มภาคเรียนต้องไม่อยู่หลังวันที่สิ้นสุด",
        },
        FindingCheck {
            code: Code::TermOutsideYear,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT term.id
                        FROM academic_semesters term
                        JOIN academic_years year ON year.id = term.academic_year_id
                        WHERE term.start_date < year.start_date OR term.end_date > year.end_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ช่วงวันที่ภาคเรียนต้องอยู่ภายในปีการศึกษาเดียวกัน",
        },
        FindingCheck {
            code: Code::TermSequenceAmbiguous,
            severity: Blocking,
            sql: r#"WITH normalized AS (
                        SELECT id, academic_year_id,
                               lower(regexp_replace(btrim(normalize(term, NFKC)), '\s+', ' ', 'g')) AS term_key,
                               start_date, end_date
                        FROM academic_semesters
                    ), duplicate_keys AS (
                        SELECT academic_year_id, term_key
                        FROM normalized
                        GROUP BY academic_year_id, term_key
                        HAVING COUNT(*) > 1
                    ), duplicate_dates AS (
                        SELECT academic_year_id, start_date, end_date
                        FROM normalized
                        GROUP BY academic_year_id, start_date, end_date
                        HAVING COUNT(*) > 1
                    ), affected AS (
                        SELECT normalized.id
                        FROM normalized
                        JOIN duplicate_keys USING (academic_year_id, term_key)
                        UNION
                        SELECT normalized.id
                        FROM normalized
                        JOIN duplicate_dates USING (academic_year_id, start_date, end_date)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "แก้รหัสหรือลำดับภาคเรียนในปีเดียวกันให้ไม่ซ้ำและเรียงได้แน่นอน",
        },
        FindingCheck {
            code: Code::SubjectIdentityBlank,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT id FROM subjects WHERE btrim(normalize(code, NFKC)) = ''
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "รายวิชาทุก version ต้องมีรหัสวิชาที่ไม่ว่าง",
        },
        FindingCheck {
            code: Code::SubjectIdentityConflict,
            severity: Blocking,
            sql: r#"WITH normalized AS (
                        SELECT id, start_academic_year_id,
                               lower(regexp_replace(btrim(normalize(code, NFKC)), '\s+', ' ', 'g')) AS identity_key
                        FROM subjects
                    ), conflicts AS (
                        SELECT identity_key, start_academic_year_id
                        FROM normalized
                        GROUP BY identity_key, start_academic_year_id
                        HAVING COUNT(*) > 1
                    ), affected AS (
                        SELECT normalized.id FROM normalized
                        JOIN conflicts USING (identity_key, start_academic_year_id)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "รหัสวิชาที่ normalize แล้วห้ามมีหลาย version เริ่มในปีเดียวกัน",
        },
        FindingCheck {
            code: Code::SubjectVersionRangeOverlap,
            severity: Blocking,
            sql: r#"WITH ordered AS (
                        SELECT subject.id,
                               year.start_date,
                               lag(year.end_date) OVER (
                                   PARTITION BY lower(regexp_replace(btrim(normalize(subject.code, NFKC)), '\s+', ' ', 'g'))
                                   ORDER BY year.start_date, subject.id
                               ) AS previous_end
                        FROM subjects subject
                        JOIN academic_years year ON year.id = subject.start_academic_year_id
                    ), affected AS (
                        SELECT id FROM ordered WHERE previous_end >= start_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ช่วงปีที่มีผลของ version รายวิชาเดียวกันต้องไม่ทับซ้อน",
        },
        FindingCheck {
            code: Code::ActivityIdentityConflict,
            severity: Blocking,
            sql: r#"WITH normalized AS (
                        SELECT id, start_academic_year_id,
                               lower(btrim(activity_type)) AS type_key,
                               lower(regexp_replace(btrim(normalize(name, NFKC)), '\s+', ' ', 'g')) AS name_key
                        FROM activity_catalog
                    ), conflicts AS (
                        SELECT type_key, name_key, start_academic_year_id
                        FROM normalized
                        GROUP BY type_key, name_key, start_academic_year_id
                        HAVING COUNT(*) > 1
                    ), affected AS (
                        SELECT normalized.id FROM normalized
                        JOIN conflicts USING (type_key, name_key, start_academic_year_id)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "กิจกรรมชื่อและประเภทเดียวกันห้ามมีหลาย version เริ่มในปีเดียวกัน",
        },
        FindingCheck {
            code: Code::ActivityVersionRangeOverlap,
            severity: Blocking,
            sql: r#"WITH ordered AS (
                        SELECT activity.id,
                               year.start_date,
                               lag(year.end_date) OVER (
                                   PARTITION BY lower(btrim(activity.activity_type)),
                                                lower(regexp_replace(btrim(normalize(activity.name, NFKC)), '\s+', ' ', 'g'))
                                   ORDER BY year.start_date, activity.id
                               ) AS previous_end
                        FROM activity_catalog activity
                        JOIN academic_years year ON year.id = activity.start_academic_year_id
                    ), affected AS (
                        SELECT id FROM ordered WHERE previous_end >= start_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ช่วงปีที่มีผลของ version กิจกรรมเดียวกันต้องไม่ทับซ้อน",
        },
        FindingCheck {
            code: Code::CurriculumVersionUnresolved,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT version.id
                        FROM study_plan_versions version
                        JOIN academic_years starts ON starts.id = version.start_academic_year_id
                        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
                        WHERE ends.id IS NOT NULL AND ends.start_date < starts.start_date
                        UNION
                        SELECT requirement.id
                        FROM study_plan_subjects requirement
                        JOIN study_plan_versions version ON version.id = requirement.study_plan_version_id
                        JOIN academic_years version_start ON version_start.id = version.start_academic_year_id
                        JOIN subjects subject ON subject.id = requirement.subject_id
                        JOIN academic_years subject_start ON subject_start.id = subject.start_academic_year_id
                        WHERE subject_start.start_date > version_start.start_date
                        UNION
                        SELECT requirement.id
                        FROM study_plan_version_activities requirement
                        JOIN study_plan_versions version ON version.id = requirement.study_plan_version_id
                        JOIN academic_years version_start ON version_start.id = version.start_academic_year_id
                        JOIN activity_catalog activity ON activity.id = requirement.activity_catalog_id
                        JOIN academic_years activity_start ON activity_start.id = activity.start_academic_year_id
                        WHERE activity_start.start_date > version_start.start_date
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "กำหนดช่วงใช้หลักสูตรและ version ของรายวิชา/กิจกรรมให้สัมพันธ์กัน",
        },
        FindingCheck {
            code: Code::EnrollmentYearConflict,
            severity: Blocking,
            sql: r#"WITH current_rows AS (
                        SELECT enrollment.id, enrollment.student_id, homeroom.academic_year_id
                        FROM student_class_enrollments enrollment
                        JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
                        WHERE enrollment.status = 'active'
                    ), conflicts AS (
                        SELECT student_id, academic_year_id
                        FROM current_rows
                        GROUP BY student_id, academic_year_id
                        HAVING COUNT(*) > 1
                    ), affected AS (
                        SELECT current_rows.id FROM current_rows
                        JOIN conflicts USING (student_id, academic_year_id)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "นักเรียนหนึ่งคนต้องมีห้องปัจจุบันได้เพียงหนึ่งห้องต่อปีการศึกษา",
        },
        FindingCheck {
            code: Code::EnrollmentStatusInvalid,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT enrollment.id
                        FROM student_class_enrollments enrollment
                        JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
                        JOIN academic_years year ON year.id = homeroom.academic_year_id
                        WHERE (enrollment.status = 'active' AND year.end_date < $1)
                           OR (enrollment.status = 'completed' AND year.end_date >= $1)
                           OR enrollment.status NOT IN ('active', 'completed', 'transferred', 'dropped')
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: true,
            guidance_th: "แก้สถานะการเข้าเรียนให้สอดคล้องกับสถานะปีการศึกษา",
        },
        FindingCheck {
            code: Code::HomeroomProgramUnresolved,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT homeroom.id
                        FROM class_rooms homeroom
                        JOIN academic_years homeroom_year ON homeroom_year.id = homeroom.academic_year_id
                        LEFT JOIN study_plan_versions version ON version.id = homeroom.study_plan_version_id
                        LEFT JOIN academic_years starts ON starts.id = version.start_academic_year_id
                        LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
                        WHERE version.id IS NULL
                           OR starts.start_date > homeroom_year.start_date
                           OR (ends.id IS NOT NULL AND ends.end_date < homeroom_year.end_date)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ทุกห้องต้องอ้างหลักสูตร version ที่ใช้ได้ในปีของห้องนั้น",
        },
        FindingCheck {
            code: Code::CourseTermYearMismatch,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT course.id
                        FROM classroom_courses course
                        JOIN class_rooms homeroom ON homeroom.id = course.classroom_id
                        JOIN academic_semesters term ON term.id = course.academic_semester_id
                        JOIN academic_years term_year ON term_year.id = term.academic_year_id
                        JOIN subjects subject ON subject.id = course.subject_id
                        JOIN academic_years subject_start ON subject_start.id = subject.start_academic_year_id
                        WHERE homeroom.academic_year_id <> term.academic_year_id
                           OR subject_start.start_date > term_year.start_date
                           OR EXISTS (
                                SELECT 1
                                FROM subjects newer
                                JOIN academic_years newer_start ON newer_start.id = newer.start_academic_year_id
                                WHERE lower(regexp_replace(btrim(normalize(newer.code, NFKC)), '\s+', ' ', 'g')) =
                                      lower(regexp_replace(btrim(normalize(subject.code, NFKC)), '\s+', ' ', 'g'))
                                  AND newer_start.start_date <= term_year.start_date
                                  AND newer_start.start_date > subject_start.start_date
                           )
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "รายวิชาที่เปิดสอน ห้อง และภาคเรียนต้องอยู่ในปีเดียวกันและใช้ version ที่มีผล",
        },
        FindingCheck {
            code: Code::SynchronizedActivityPatternConflict,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT slot.id
                        FROM activity_slots slot
                        JOIN academic_semesters term ON term.id = slot.semester_id
                        JOIN activity_catalog activity ON activity.id = slot.activity_catalog_id
                        LEFT JOIN academic_years activity_start ON activity_start.id = activity.start_academic_year_id
                        LEFT JOIN academic_years term_year ON term_year.id = term.academic_year_id
                        WHERE activity_start.start_date > term_year.start_date
                           OR (activity.term IS NOT NULL AND lower(btrim(activity.term)) <> lower(btrim(term.term)))
                           OR (activity.scheduling_mode = 'synchronized' AND EXISTS (
                                SELECT 1 FROM activity_slot_classroom_assignments assignment
                                WHERE assignment.slot_id = slot.id
                           ))
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "รูปแบบ synchronized/independent และภาคเรียนของกิจกรรมต้องตรงกับ slot",
        },
        FindingCheck {
            code: Code::ActivityMemberDuplicate,
            severity: Blocking,
            sql: r#"WITH duplicates AS (
                        SELECT activity_group_id
                        FROM activity_group_members
                        GROUP BY activity_group_id, student_id
                        HAVING COUNT(*) > 1
                    ), affected AS (
                        SELECT DISTINCT activity_group_id AS id FROM duplicates
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ลบสมาชิกกิจกรรมที่ซ้ำในกลุ่มเดียวกันให้เหลือหนึ่งรายการ",
        },
        FindingCheck {
            code: Code::AssessmentReferenceOrphan,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT plan.id
                        FROM academic_assessment_plans plan
                        LEFT JOIN academic_semesters term ON term.id = plan.academic_semester_id
                        LEFT JOIN subjects subject ON subject.id = plan.subject_id
                        LEFT JOIN classroom_courses course ON course.id = plan.classroom_course_id
                        WHERE term.id IS NULL OR subject.id IS NULL
                           OR (course.id IS NOT NULL AND (
                                course.academic_semester_id <> plan.academic_semester_id
                                OR course.subject_id <> plan.subject_id
                           ))
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "โครงสร้างคะแนนต้องอ้างภาคเรียน รายวิชา และรายวิชาที่เปิดสอนชุดเดียวกัน",
        },
        FindingCheck {
            code: Code::TimetableReferenceOrphan,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT entry.id
                        FROM academic_timetable_entries entry
                        LEFT JOIN classroom_courses course ON course.id = entry.classroom_course_id
                        LEFT JOIN activity_slots slot ON slot.id = entry.activity_slot_id
                        WHERE (entry.entry_type = 'COURSE' AND (
                                  course.id IS NULL OR course.academic_semester_id <> entry.academic_semester_id
                              ))
                           OR (entry.entry_type = 'ACTIVITY' AND (
                                  slot.id IS NULL OR slot.semester_id <> entry.academic_semester_id
                              ))
                           OR (entry.entry_type NOT IN ('COURSE', 'ACTIVITY')
                               AND entry.classroom_course_id IS NOT NULL
                               AND course.academic_semester_id <> entry.academic_semester_id)
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "รายการตารางสอนต้องอ้างกลุ่มเรียนหรือกิจกรรมในภาคเรียนเดียวกัน",
        },
        FindingCheck {
            code: Code::ExamReferenceOrphan,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT item.id
                        FROM academic_exam_schedule_items item
                        LEFT JOIN academic_exam_rounds round ON round.id = item.exam_round_id
                        LEFT JOIN academic_assessment_plans plan ON plan.id = item.assessment_plan_id
                        LEFT JOIN classroom_courses course ON course.id = item.classroom_course_id
                        LEFT JOIN class_rooms homeroom ON homeroom.id = item.classroom_id
                        LEFT JOIN academic_semesters term ON term.id = item.academic_semester_id
                        WHERE round.id IS NULL OR plan.id IS NULL OR course.id IS NULL OR homeroom.id IS NULL OR term.id IS NULL
                           OR round.academic_semester_id <> item.academic_semester_id
                           OR plan.academic_semester_id <> item.academic_semester_id
                           OR plan.subject_id <> item.subject_id
                           OR course.academic_semester_id <> item.academic_semester_id
                           OR course.subject_id <> item.subject_id
                           OR homeroom.academic_year_id <> term.academic_year_id
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "ตารางสอบ แผนคะแนน กลุ่มเรียน ห้อง และภาคเรียนต้องอยู่ในบริบทเดียวกัน",
        },
        FindingCheck {
            code: Code::SupervisionReferenceOrphan,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT cycle.id
                        FROM supervision_cycles cycle
                        LEFT JOIN academic_semesters term ON term.id = cycle.academic_semester_id
                        LEFT JOIN academic_years year ON year.id = term.academic_year_id
                        WHERE term.id IS NULL OR year.id IS NULL
                           OR cycle.academic_year <> year.year
                           OR lower(btrim(cycle.semester)) <> lower(btrim(term.term))
                        UNION
                        SELECT observation.id
                        FROM supervision_observations observation
                        JOIN supervision_cycles cycle ON cycle.id = observation.cycle_id
                        JOIN academic_timetable_entries entry ON entry.id = observation.timetable_entry_id
                        WHERE cycle.academic_semester_id IS DISTINCT FROM entry.academic_semester_id
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "รอบนิเทศและคาบที่นิเทศต้องอ้างภาคเรียนเดียวกันโดยไม่ใช้ข้อความที่ขัดกัน",
        },
        FindingCheck {
            code: Code::AdmissionProgramUnresolved,
            severity: Blocking,
            sql: r#"WITH affected AS (
                        SELECT track.id
                        FROM admission_tracks track
                        JOIN admission_rounds round ON round.id = track.admission_round_id
                        JOIN academic_years round_year ON round_year.id = round.academic_year_id
                        WHERE (
                            SELECT COUNT(*)
                            FROM study_plan_versions version
                            JOIN academic_years starts ON starts.id = version.start_academic_year_id
                            LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
                            WHERE version.study_plan_id = track.study_plan_id
                              AND starts.start_date <= round_year.start_date
                              AND (ends.id IS NULL OR ends.end_date >= round_year.end_date)
                        ) <> 1
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "สายรับสมัครต้องหา curriculum version ที่ใช้ในปีรับสมัครได้เพียงหนึ่งรายการ",
        },
        FindingCheck {
            code: Code::PermissionMappingUnresolved,
            severity: Blocking,
            sql: r#"WITH granted_permissions AS (
                        SELECT permission_id FROM role_permissions
                        UNION
                        SELECT permission_id FROM organization_permission_grants
                        UNION
                        SELECT permission_id FROM organization_permission_delegations
                    ), affected AS (
                        SELECT DISTINCT permission.id
                        FROM granted_permissions grant_row
                        JOIN permissions permission ON permission.id = grant_row.permission_id
                        WHERE (
                              permission.code LIKE 'academic_structure.%'
                           OR permission.code LIKE 'academic_classroom.%'
                           OR permission.code LIKE 'academic_enrollment.%'
                           OR permission.code LIKE 'academic_course_plan.%'
                           OR permission.code LIKE 'academic_curriculum.%'
                           OR permission.code LIKE 'activity.%'
                        )
                        AND permission.code NOT IN (
                            'academic_structure.read.all',
                            'academic_structure.manage.all',
                            'academic_classroom.create.all',
                            'academic_classroom.delete.all',
                            'academic_classroom.read.all',
                            'academic_classroom.update.all',
                            'academic_enrollment.read.all',
                            'academic_enrollment.update.all',
                            'academic_course_plan.read.all',
                            'academic_course_plan.manage.all',
                            'academic_curriculum.read.all',
                            'academic_curriculum.read.organization_tree',
                            'academic_curriculum.create.all',
                            'academic_curriculum.update.all',
                            'academic_curriculum.delete.all',
                            'academic_curriculum.manage.organization_unit',
                            'academic_curriculum.manage.organization_tree',
                            'activity.read.all',
                            'activity.manage.all',
                            'activity.manage_members.all',
                            'activity.manage.own'
                        )
                    )
                    SELECT COUNT(*), COALESCE((array_agg(id ORDER BY id))[1:20], ARRAY[]::uuid[])
                    FROM affected"#,
            uses_cutover_date: false,
            guidance_th: "เพิ่ม mapping ให้ permission เดิมที่ยังมีผู้ได้รับสิทธิ์ก่อน cutover",
        },
        FindingCheck {
            code: Code::HistoricalResultsUnavailable,
            severity: Warning,
            sql: r#"WITH affected AS (
                        SELECT enrollment.id
                        FROM student_class_enrollments enrollment
                        JOIN class_rooms homeroom ON homeroom.id = enrollment.class_room_id
                        JOIN academic_years year ON year.id = homeroom.academic_year_id
                        WHERE year.end_date < $1
                    )
                    SELECT COUNT(*), ARRAY[]::uuid[] FROM affected"#,
            uses_cutover_date: true,
            guidance_th: "ระบบเดิมไม่มีผลการเรียนรายวิชาในอดีต จึงย้ายได้เฉพาะประวัติห้องและผลกิจกรรมที่มีอยู่",
        },
    ]
}

async fn collect_counts(
    connection: &mut PgConnection,
    queries: &[(&str, &str)],
) -> Result<BTreeMap<String, i64>, PreflightError> {
    let mut counts = BTreeMap::new();
    for (key, sql) in queries {
        let count = sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(&mut *connection)
            .await
            .map_err(|_| PreflightError::QueryFailed)?;
        counts.insert((*key).to_string(), count);
    }
    Ok(counts)
}

async fn collect_findings(
    connection: &mut PgConnection,
    cutover_date: NaiveDate,
) -> Result<Vec<AcademicCorePreflightFinding>, PreflightError> {
    let mut findings = Vec::new();

    for (index, check) in finding_checks().iter().enumerate() {
        debug_assert_eq!(preflight_check_codes()[index], check.code);
        let (affected_count, resource_ids) = if check.uses_cutover_date {
            sqlx::query_as::<_, (i64, Vec<Uuid>)>(check.sql)
                .bind(cutover_date)
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| PreflightError::QueryFailed)?
        } else {
            sqlx::query_as::<_, (i64, Vec<Uuid>)>(check.sql)
                .fetch_one(&mut *connection)
                .await
                .map_err(|_| PreflightError::QueryFailed)?
        };

        if affected_count > 0 {
            findings.push(AcademicCorePreflightFinding::new(
                check.code,
                check.severity,
                affected_count,
                resource_ids,
                check.guidance_th.to_string(),
            ));
        }
    }

    Ok(findings)
}

pub async fn run_academic_core_preflight(
    pool: &PgPool,
    schema_label: &str,
    cutover_date: NaiveDate,
) -> Result<AcademicCorePreflightReport, PreflightError> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| PreflightError::ReadOnlyTransactionFailed)?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *transaction)
        .await
        .map_err(|_| PreflightError::ReadOnlyTransactionFailed)?;

    let source_counts = collect_counts(&mut transaction, SOURCE_COUNT_QUERIES).await?;
    let expected_target_counts =
        collect_counts(&mut transaction, EXPECTED_TARGET_COUNT_QUERIES).await?;
    let findings = collect_findings(&mut transaction, cutover_date).await?;

    transaction
        .rollback()
        .await
        .map_err(|_| PreflightError::ReadOnlyTransactionFailed)?;

    Ok(build_preflight_report(
        schema_label.to_string(),
        source_counts,
        expected_target_counts,
        findings,
    ))
}

pub fn normalize_academic_identity(value: &str) -> String {
    value
        .nfkc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn classify_year(
    is_active: bool,
    start_date: NaiveDate,
    end_date: NaiveDate,
    cutover_date: NaiveDate,
) -> Result<MappedAcademicYearStatus, AcademicCorePreflightCode> {
    if start_date > end_date {
        return Err(AcademicCorePreflightCode::YearDateRangeInvalid);
    }

    if is_active {
        return if start_date <= cutover_date && cutover_date <= end_date {
            Ok(MappedAcademicYearStatus::Active)
        } else {
            Err(AcademicCorePreflightCode::ActiveYearDateMismatch)
        };
    }

    if end_date < cutover_date {
        return Ok(MappedAcademicYearStatus::Closed);
    }

    if start_date > cutover_date {
        return Ok(MappedAcademicYearStatus::Planning);
    }

    Err(AcademicCorePreflightCode::InactiveCurrentYearAmbiguous)
}

pub fn classify_term(
    is_active: bool,
    start_date: NaiveDate,
    end_date: NaiveDate,
    cutover_date: NaiveDate,
) -> Result<MappedAcademicTermStatus, AcademicCorePreflightCode> {
    if start_date > end_date {
        return Err(AcademicCorePreflightCode::TermDateRangeInvalid);
    }

    if is_active {
        return if start_date <= cutover_date && cutover_date <= end_date {
            Ok(MappedAcademicTermStatus::Active)
        } else {
            Err(AcademicCorePreflightCode::ActiveTermDateMismatch)
        };
    }

    if end_date < cutover_date {
        return Ok(MappedAcademicTermStatus::Closed);
    }

    if start_date > cutover_date {
        return Ok(MappedAcademicTermStatus::Planning);
    }

    Err(AcademicCorePreflightCode::InactiveCurrentTermAmbiguous)
}

#[cfg(test)]
mod tests {
    use super::{
        build_preflight_report, classify_term, classify_year, normalize_academic_identity,
        preflight_check_codes, AcademicCorePreflightCode, AcademicCorePreflightFinding,
        MappedAcademicTermStatus, MappedAcademicYearStatus, PreflightSeverity,
    };
    use chrono::NaiveDate;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("test date must be valid")
    }

    #[test]
    fn normalization_is_stable_for_nfkc_case_and_internal_whitespace() {
        assert_eq!(
            normalize_academic_identity("  ＭＡＴＨ\t  พื้นฐาน  "),
            "math พื้นฐาน"
        );
    }

    #[test]
    fn active_year_maps_to_active_when_dates_cover_cutover() {
        assert_eq!(
            classify_year(true, date(2025, 5, 1), date(2026, 3, 31), date(2025, 8, 23),),
            Ok(MappedAcademicYearStatus::Active)
        );
    }

    #[test]
    fn inactive_year_before_cutover_maps_to_closed() {
        assert_eq!(
            classify_year(
                false,
                date(2024, 5, 1),
                date(2025, 3, 31),
                date(2025, 8, 23),
            ),
            Ok(MappedAcademicYearStatus::Closed)
        );
    }

    #[test]
    fn inactive_year_after_cutover_maps_to_planning() {
        assert_eq!(
            classify_year(
                false,
                date(2026, 5, 1),
                date(2027, 3, 31),
                date(2025, 8, 23),
            ),
            Ok(MappedAcademicYearStatus::Planning)
        );
    }

    #[test]
    fn inactive_year_covering_cutover_is_ambiguous() {
        assert_eq!(
            classify_year(
                false,
                date(2025, 5, 1),
                date(2026, 3, 31),
                date(2025, 8, 23),
            ),
            Err(AcademicCorePreflightCode::InactiveCurrentYearAmbiguous)
        );
    }

    #[test]
    fn invalid_year_date_range_is_rejected_before_status_mapping() {
        assert_eq!(
            classify_year(true, date(2026, 3, 31), date(2025, 5, 1), date(2025, 8, 23),),
            Err(AcademicCorePreflightCode::YearDateRangeInvalid)
        );
    }

    #[test]
    fn active_year_outside_cutover_date_is_rejected() {
        assert_eq!(
            classify_year(true, date(2024, 5, 1), date(2025, 3, 31), date(2025, 8, 23),),
            Err(AcademicCorePreflightCode::ActiveYearDateMismatch)
        );
    }

    #[test]
    fn term_status_uses_term_specific_findings() {
        assert_eq!(
            classify_term(
                false,
                date(2025, 5, 1),
                date(2025, 10, 1),
                date(2025, 8, 23),
            ),
            Err(AcademicCorePreflightCode::InactiveCurrentTermAmbiguous)
        );
        assert_eq!(
            classify_term(
                false,
                date(2025, 10, 1),
                date(2025, 5, 1),
                date(2025, 8, 23),
            ),
            Err(AcademicCorePreflightCode::TermDateRangeInvalid)
        );
        assert_eq!(
            classify_term(true, date(2025, 11, 1), date(2026, 3, 1), date(2025, 8, 23),),
            Err(AcademicCorePreflightCode::ActiveTermDateMismatch)
        );
        assert_eq!(
            classify_term(true, date(2025, 5, 1), date(2025, 10, 1), date(2025, 8, 23),),
            Ok(MappedAcademicTermStatus::Active)
        );
    }

    #[test]
    fn report_blocks_cutover_only_for_blocking_findings_and_caps_samples() {
        let warning = AcademicCorePreflightFinding::new(
            AcademicCorePreflightCode::HistoricalResultsUnavailable,
            PreflightSeverity::Warning,
            25,
            (0..25).map(|_| Uuid::new_v4()).collect(),
            "ข้อมูลปีเก่าไม่มีผลการเรียนที่ล็อกไว้".to_string(),
        );
        let warning_only = build_preflight_report(
            "tenant_demo".to_string(),
            BTreeMap::from([("academic_years".to_string(), 3)]),
            BTreeMap::from([("academic_years".to_string(), 3)]),
            vec![warning.clone()],
        );

        assert!(warning_only.can_cut_over);
        assert_eq!(warning_only.findings[0].resource_ids.len(), 20);

        let blocking = AcademicCorePreflightFinding::new(
            AcademicCorePreflightCode::ActiveYearCountInvalid,
            PreflightSeverity::Blocking,
            2,
            Vec::new(),
            "ต้องมีปีการศึกษาที่ใช้งานอยู่เพียงหนึ่งปี".to_string(),
        );
        let blocked = build_preflight_report(
            "tenant_demo".to_string(),
            BTreeMap::new(),
            BTreeMap::new(),
            vec![warning, blocking],
        );

        assert!(!blocked.can_cut_over);
    }

    #[test]
    fn database_preflight_declares_every_stable_finding_code_once() {
        let codes = preflight_check_codes();

        assert_eq!(codes.len(), 29);
        assert_eq!(
            codes.first(),
            Some(&AcademicCorePreflightCode::ActiveYearCountInvalid)
        );
        assert_eq!(
            codes.last(),
            Some(&AcademicCorePreflightCode::HistoricalResultsUnavailable)
        );
        for (index, code) in codes.iter().enumerate() {
            assert!(
                !codes[..index].contains(code),
                "preflight code {code:?} must be declared exactly once"
            );
        }
    }
}
