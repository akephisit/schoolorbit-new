use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::modules::school::models::*;
use school_errors::AppError;

#[derive(Debug, FromRow)]
struct StatisticsRow {
    academic_year: Option<i32>,
    academic_year_name: Option<String>,
    grade_id: Option<Uuid>,
    level_type: Option<String>,
    grade_year: Option<i32>,
    homeroom_id: Option<Uuid>,
    homeroom_name: Option<String>,
    male: i64,
    female: i64,
    other_or_unspecified: i64,
    total_teachers: i64,
    total_staff: i64,
    as_of: DateTime<Utc>,
}

fn add_counts(target: &mut PublicStudentCounts, counts: &PublicStudentCounts) {
    target.total += counts.total;
    target.male += counts.male;
    target.female += counts.female;
    target.other_or_unspecified += counts.other_or_unspecified;
}

fn statistics_from_rows(rows: Vec<StatisticsRow>) -> Result<PublicSchoolStatistics, AppError> {
    let first = rows
        .first()
        .ok_or_else(|| AppError::InternalServerError("ไม่สามารถโหลดสถิติโรงเรียนได้".into()))?;
    let mut result = PublicSchoolStatistics {
        academic_year: first
            .academic_year
            .zip(first.academic_year_name.clone())
            .map(|(year, name)| PublicAcademicYear { year, name }),
        students: PublicStudentCounts::default(),
        unassigned_students: PublicStudentCounts::default(),
        total_teachers: first.total_teachers,
        total_staff: first.total_staff,
        total_homerooms: 0,
        grades: Vec::new(),
        as_of: first.as_of,
    };
    let mut grade_indices = BTreeMap::new();
    for row in rows {
        let Some(grade_id) = row.grade_id else {
            continue;
        };
        let index = *grade_indices.entry(grade_id).or_insert_with(|| {
            result.grades.push(PublicGradeStatistics {
                level_type: row.level_type.clone().unwrap_or_default(),
                year: row.grade_year.unwrap_or_default(),
                students: PublicStudentCounts::default(),
                unassigned_students: PublicStudentCounts::default(),
                homerooms: Vec::new(),
            });
            result.grades.len() - 1
        });
        let counts = PublicStudentCounts {
            total: row.male + row.female + row.other_or_unspecified,
            male: row.male,
            female: row.female,
            other_or_unspecified: row.other_or_unspecified,
        };
        let grade = &mut result.grades[index];
        add_counts(&mut grade.students, &counts);
        add_counts(&mut result.students, &counts);
        if row.homeroom_id.is_some() {
            result.total_homerooms += 1;
            grade.homerooms.push(PublicHomeroomStatistics {
                name: row.homeroom_name.unwrap_or_default(),
                students: counts,
            });
        } else {
            add_counts(&mut grade.unassigned_students, &counts);
            add_counts(&mut result.unassigned_students, &counts);
        }
    }
    Ok(result)
}

/// One statement gives all aggregates the same database snapshot. Account suspension
/// does not change educational enrollment; only student-year status owns this count.
pub async fn get_statistics(pool: &PgPool) -> Result<PublicSchoolStatistics, AppError> {
    let rows = sqlx::query_as::<_, StatisticsRow>(r#"
WITH active_year AS (
    SELECT id, year, name FROM academic_years WHERE status = 'active'
), active_rooms AS (
    SELECT h.id, h.grade_level_id, h.name, h.room_number
    FROM homerooms h JOIN active_year y ON y.id = h.academic_year_id
    WHERE h.is_active IS TRUE
), roster AS (
    SELECT s.student_id, s.grade_level_id, u.gender, h.id AS homeroom_id
    FROM student_academic_years s
    JOIN active_year y ON y.id = s.academic_year_id
    JOIN users u ON u.id = s.student_id AND u.user_type = 'student'
    LEFT JOIN homeroom_placements p ON p.student_academic_year_id = s.id
        AND p.status = 'current' AND p.start_date <= CURRENT_DATE
        AND (p.end_date IS NULL OR p.end_date >= CURRENT_DATE)
    LEFT JOIN active_rooms h ON h.id = p.homeroom_id AND h.grade_level_id = s.grade_level_id
    WHERE s.status = 'active'
), buckets AS (
    SELECT grade_level_id, id AS homeroom_id, name AS homeroom_name, room_number FROM active_rooms
    UNION ALL
    SELECT DISTINCT grade_level_id, NULL::uuid, NULL::varchar, NULL::varchar
    FROM roster WHERE homeroom_id IS NULL
), counts AS (
    SELECT b.grade_level_id, b.homeroom_id, b.homeroom_name, b.room_number,
        COUNT(r.student_id) FILTER (WHERE r.gender = 'male') AS male,
        COUNT(r.student_id) FILTER (WHERE r.gender = 'female') AS female,
        COUNT(r.student_id) FILTER (WHERE r.gender IS NULL OR r.gender NOT IN ('male', 'female')) AS other_or_unspecified
    FROM buckets b LEFT JOIN roster r ON r.grade_level_id = b.grade_level_id
        AND r.homeroom_id IS NOT DISTINCT FROM b.homeroom_id
    GROUP BY b.grade_level_id, b.homeroom_id, b.homeroom_name, b.room_number
), staff_counts AS (
    SELECT COUNT(*) AS total_staff,
        COUNT(*) FILTER (WHERE p.code IN ('teacher', 'assistant_teacher', 'contract_teacher', 'government_employee_teacher')) AS total_teachers
    FROM users u LEFT JOIN staff_info i ON i.user_id = u.id
    LEFT JOIN staff_job_positions p ON p.id = i.job_position_id
    WHERE u.user_type = 'staff' AND u.status = 'active'
)
SELECT y.year AS academic_year, y.name AS academic_year_name,
    g.id AS grade_id, g.level_type, g.year AS grade_year,
    c.homeroom_id, c.homeroom_name,
    COALESCE(c.male, 0) AS male, COALESCE(c.female, 0) AS female,
    COALESCE(c.other_or_unspecified, 0) AS other_or_unspecified,
    s.total_teachers, s.total_staff, CURRENT_TIMESTAMP AS as_of
FROM staff_counts s LEFT JOIN active_year y ON TRUE
LEFT JOIN counts c ON TRUE LEFT JOIN grade_levels g ON g.id = c.grade_level_id
ORDER BY CASE g.level_type WHEN 'kindergarten' THEN 1 WHEN 'primary' THEN 2
    WHEN 'secondary' THEN 3 ELSE 4 END, g.level_type, g.year,
    char_length(c.room_number), c.room_number, c.homeroom_name, c.homeroom_id
"#).fetch_all(pool).await.map_err(database_error)?;
    statistics_from_rows(rows)
}

#[derive(Debug, FromRow)]
struct OrganizationRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    unit_type: String,
    member_name: Option<String>,
    position_code: Option<String>,
    position_title: Option<String>,
    avatar_member_id: Option<Uuid>,
}

fn organization_from_rows(rows: Vec<OrganizationRow>) -> PublicSchoolOrganization {
    let mut units: Vec<PublicOrganizationUnit> = Vec::new();
    let mut indices = BTreeMap::new();
    for row in rows {
        let index = *indices.entry(row.id).or_insert_with(|| {
            units.push(PublicOrganizationUnit {
                id: row.id,
                parent_id: row.parent_id,
                name: row.name,
                unit_type: row.unit_type,
                members: Vec::new(),
            });
            units.len() - 1
        });
        if let Some((name, position_code)) = row.member_name.zip(row.position_code) {
            units[index].members.push(PublicOrganizationMember {
                name,
                position_code,
                position_title: row.position_title,
                avatar_url: row
                    .avatar_member_id
                    .map(|id| format!("/api/school/public/organization-members/{id}/avatar")),
            });
        }
    }
    PublicSchoolOrganization { units }
}

pub async fn get_organization(pool: &PgPool) -> Result<PublicSchoolOrganization, AppError> {
    let rows = sqlx::query_as::<_, OrganizationRow>(r#"
SELECT o.id, o.parent_unit_id AS parent_id, o.name, o.unit_type,
    CASE WHEN u.id IS NOT NULL THEN CONCAT(u.title, u.first_name, ' ', u.last_name) END AS member_name,
    CASE WHEN u.id IS NOT NULL THEN m.position_code END AS position_code, m.position_title,
    CASE WHEN profile.id IS NOT NULL THEN m.id END AS avatar_member_id
FROM organization_units o
LEFT JOIN organization_members m ON m.organization_unit_id = o.id
    AND m.started_at <= CURRENT_DATE AND (m.ended_at IS NULL OR m.ended_at > CURRENT_DATE)
LEFT JOIN users u ON u.id = m.user_id AND u.user_type = 'staff' AND u.status = 'active'
LEFT JOIN files profile ON profile.id=u.profile_image_file_id AND profile.owner_user_id=u.id
    AND profile.purpose_code='profile_image' AND profile.visibility='private'
    AND profile.lifecycle_status='ready' AND profile.deleted_at IS NULL
WHERE o.is_active IS TRUE
ORDER BY o.display_order, o.name, o.id,
    CASE m.position_code WHEN 'director' THEN 1 WHEN 'deputy_director' THEN 2 WHEN 'head' THEN 3 WHEN 'deputy_head' THEN 4 WHEN 'coordinator' THEN 5 ELSE 6 END,
    u.first_name, u.last_name, m.id
"#).fetch_all(pool).await.map_err(database_error)?;
    Ok(organization_from_rows(rows))
}

fn database_error(error: sqlx::Error) -> AppError {
    tracing::error!(%error, "Failed to load public school aggregates");
    AppError::InternalServerError("ไม่สามารถโหลดข้อมูลโรงเรียนได้".into())
}

/// Only the profile image of a currently published staff membership is a public directory photo.
/// General private-file URLs and student/parent profiles remain inaccessible anonymously.
pub async fn organization_avatar_file(pool: &PgPool, member_id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar(r#"SELECT profile.id FROM organization_members member
        JOIN organization_units unit ON unit.id=member.organization_unit_id AND unit.is_active
        JOIN users staff ON staff.id=member.user_id AND staff.user_type='staff' AND staff.status='active'
        JOIN files profile ON profile.id=staff.profile_image_file_id AND profile.owner_user_id=staff.id
            AND profile.purpose_code='profile_image' AND profile.visibility='private'
            AND profile.lifecycle_status='ready' AND profile.deleted_at IS NULL
        WHERE member.id=$1 AND member.started_at<=CURRENT_DATE
            AND (member.ended_at IS NULL OR member.ended_at>CURRENT_DATE)"#)
        .bind(member_id).fetch_optional(pool).await.map_err(database_error)?
        .ok_or_else(|| AppError::NotFound("ไม่พบรูปบุคลากรที่เผยแพร่".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_counts_empty_rooms_and_unassigned_students_without_losing_gender_totals() {
        let grade = Uuid::new_v4();
        let row = |room: Option<Uuid>, male, female, other| StatisticsRow {
            academic_year: Some(2569),
            academic_year_name: Some("ปีการศึกษา 2569".into()),
            grade_id: Some(grade),
            level_type: Some("secondary".into()),
            grade_year: Some(1),
            homeroom_id: room,
            homeroom_name: room.map(|_| "ม.1/1".into()),
            male,
            female,
            other_or_unspecified: other,
            total_teachers: 2,
            total_staff: 4,
            as_of: Utc::now(),
        };
        let result = statistics_from_rows(vec![
            row(Some(Uuid::new_v4()), 2, 3, 1),
            row(Some(Uuid::new_v4()), 0, 0, 0),
            row(None, 1, 0, 1),
        ])
        .unwrap();
        assert_eq!(result.students.total, 8);
        assert_eq!(result.students.other_or_unspecified, 2);
        assert_eq!(result.unassigned_students.total, 2);
        assert_eq!(result.total_homerooms, 2);
        assert_eq!(result.grades[0].students.total, result.students.total);
    }

    #[test]
    fn organization_keeps_vacant_units_and_multiple_members_without_exposing_user_ids() {
        let id = Uuid::new_v4();
        let result = organization_from_rows(vec![OrganizationRow {
            id,
            parent_id: None,
            name: "โรงเรียนทดสอบ".into(),
            unit_type: "school".into(),
            member_name: None,
            position_code: None,
            position_title: None,
            avatar_member_id: None,
        }]);
        assert!(result.units[0].members.is_empty());
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("userId"));
        assert!(!serialized.contains("email"));
    }
}
