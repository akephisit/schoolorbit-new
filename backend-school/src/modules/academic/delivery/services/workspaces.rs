use std::collections::{HashMap, HashSet};

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::models::StudyProgramOption;
use crate::modules::lookup::models::GradeLevelLookupItem;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

use super::super::models::{
    LearningDeliveryOverview, LearningOfferingOverviewItem, LearningOfferingQuery,
};
use super::offerings;

const MAX_WORKSPACE_GROUPS: i64 = 2_000;

#[derive(Debug, sqlx::FromRow)]
struct GradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct OfferingAggregateRow {
    learning_offering_id: Uuid,
    group_count: i64,
    teacher_assignment_count: i64,
    groups_without_primary_teacher: i64,
    published_roster_count: i64,
}

pub async fn delivery_overview(
    pool: &PgPool,
    academic_term_id: Uuid,
    filter: &AcademicResourceListFilter,
) -> Result<LearningDeliveryOverview, AppError> {
    let offerings =
        offerings::list(pool, LearningOfferingQuery { academic_term_id }, filter).await?;
    if offerings.is_empty() {
        return Ok(LearningDeliveryOverview {
            academic_term_id,
            offerings: Vec::new(),
        });
    }

    let offering_ids: Vec<Uuid> = offerings.iter().map(|offering| offering.id).collect();
    let grade_level_ids: Vec<Uuid> = offerings
        .iter()
        .flat_map(|offering| offering.targets.iter().map(|target| target.grade_level_id))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let study_program_ids: Vec<Uuid> = offerings
        .iter()
        .flat_map(|offering| {
            offering
                .targets
                .iter()
                .map(|target| target.study_program_id)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let grade_rows: Vec<GradeLevelRow> = sqlx::query_as(
        r#"
        SELECT id, level_type, year
        FROM grade_levels
        WHERE id = ANY($1)
        ORDER BY CASE level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
        END, year, id
        "#,
    )
    .bind(&grade_level_ids)
    .fetch_all(pool)
    .await?;
    let grades_by_id: HashMap<Uuid, GradeLevelLookupItem> = grade_rows
        .into_iter()
        .map(grade_level_lookup_item)
        .map(|item| (item.id, item))
        .collect();

    let program_rows: Vec<StudyProgramOption> = sqlx::query_as(
        r#"
        SELECT program.id, program.code, program.name_th AS name,
               curriculum.id AS curriculum_id, curriculum.name_th AS curriculum_name
        FROM study_programs program
        JOIN curriculum_versions version ON version.id = program.curriculum_version_id
        JOIN curricula curriculum ON curriculum.id = version.curriculum_id
        WHERE program.id = ANY($1)
        ORDER BY curriculum.code, program.code, program.id
        "#,
    )
    .bind(&study_program_ids)
    .fetch_all(pool)
    .await?;
    let programs_by_id: HashMap<Uuid, StudyProgramOption> = program_rows
        .into_iter()
        .map(|item| (item.id, item))
        .collect();

    if grades_by_id.len() != grade_level_ids.len()
        || programs_by_id.len() != study_program_ids.len()
    {
        return Err(AppError::InternalServerError(
            "ไม่สามารถแสดงชื่อระดับชั้นหรือแผนการเรียนของรายการเปิดสอนได้".to_string(),
        ));
    }

    let aggregates: Vec<OfferingAggregateRow> = sqlx::query_as(
        r#"
        WITH selected_offerings AS (
            SELECT unnest($1::uuid[]) AS learning_offering_id
        ),
        group_summary AS (
            SELECT learning_group.learning_offering_id,
                   count(*)::bigint AS group_count,
                   count(*) FILTER (
                       WHERE NOT EXISTS (
                           SELECT 1
                           FROM learning_group_teachers primary_teacher
                           JOIN users teacher ON teacher.id = primary_teacher.teacher_id
                           WHERE primary_teacher.learning_group_id = learning_group.id
                             AND primary_teacher.role = 'primary'
                             AND teacher.user_type = 'staff'
                             AND teacher.status = 'active'
                       )
                   )::bigint AS groups_without_primary_teacher,
                   count(*) FILTER (
                       WHERE learning_group.roster_status = 'published'
                   )::bigint AS published_roster_count
            FROM learning_groups learning_group
            WHERE learning_group.learning_offering_id = ANY($1)
            GROUP BY learning_group.learning_offering_id
        ),
        teacher_summary AS (
            SELECT learning_group.learning_offering_id,
                   count(*)::bigint AS teacher_assignment_count
            FROM learning_groups learning_group
            JOIN learning_group_teachers teacher
              ON teacher.learning_group_id = learning_group.id
            WHERE learning_group.learning_offering_id = ANY($1)
            GROUP BY learning_group.learning_offering_id
        )
        SELECT selected.learning_offering_id,
               coalesce(groups.group_count, 0)::bigint AS group_count,
               coalesce(teachers.teacher_assignment_count, 0)::bigint
                   AS teacher_assignment_count,
               coalesce(groups.groups_without_primary_teacher, 0)::bigint
                   AS groups_without_primary_teacher,
               coalesce(groups.published_roster_count, 0)::bigint AS published_roster_count
        FROM selected_offerings selected
        LEFT JOIN group_summary groups
          ON groups.learning_offering_id = selected.learning_offering_id
        LEFT JOIN teacher_summary teachers
          ON teachers.learning_offering_id = selected.learning_offering_id
        ORDER BY selected.learning_offering_id
        "#,
    )
    .bind(&offering_ids)
    .fetch_all(pool)
    .await?;
    let total_groups: i64 = aggregates.iter().map(|item| item.group_count).sum();
    if total_groups > MAX_WORKSPACE_GROUPS {
        return Err(AppError::ValidationError(
            "จำนวนกลุ่มเรียนในพื้นที่ทำงานเกิน 2000 กลุ่ม กรุณาแบ่งข้อมูลก่อนเปิดพื้นที่ทำงาน".to_string(),
        ));
    }
    let mut aggregates_by_id: HashMap<Uuid, OfferingAggregateRow> = aggregates
        .into_iter()
        .map(|item| (item.learning_offering_id, item))
        .collect();

    let mut overview_items = Vec::with_capacity(offerings.len());
    for offering in offerings {
        let aggregate = aggregates_by_id.remove(&offering.id).ok_or_else(|| {
            AppError::InternalServerError("ไม่สามารถสรุปความพร้อมของรายการเปิดสอนได้".to_string())
        })?;
        let mut offering_grade_ids: Vec<Uuid> = offering
            .targets
            .iter()
            .map(|target| target.grade_level_id)
            .collect();
        offering_grade_ids.sort_unstable();
        offering_grade_ids.dedup();
        let mut grade_levels: Vec<GradeLevelLookupItem> = offering_grade_ids
            .into_iter()
            .filter_map(|id| grades_by_id.get(&id).cloned())
            .collect();
        grade_levels.sort_by_key(|item| (item.level_order, item.id));

        let mut offering_program_ids: Vec<Uuid> = offering
            .targets
            .iter()
            .map(|target| target.study_program_id)
            .collect();
        offering_program_ids.sort_unstable();
        offering_program_ids.dedup();
        let mut study_programs: Vec<StudyProgramOption> = offering_program_ids
            .into_iter()
            .filter_map(|id| programs_by_id.get(&id).cloned())
            .collect();
        study_programs.sort_by(|left, right| {
            left.curriculum_name
                .cmp(&right.curriculum_name)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.id.cmp(&right.id))
        });

        overview_items.push(LearningOfferingOverviewItem {
            offering,
            grade_levels,
            study_programs,
            group_count: aggregate.group_count,
            teacher_assignment_count: aggregate.teacher_assignment_count,
            groups_without_primary_teacher: aggregate.groups_without_primary_teacher,
            published_roster_count: aggregate.published_roster_count,
        });
    }

    Ok(LearningDeliveryOverview {
        academic_term_id,
        offerings: overview_items,
    })
}

fn grade_level_lookup_item(row: GradeLevelRow) -> GradeLevelLookupItem {
    let (name, code, short_name, order_base) = match row.level_type.as_str() {
        "kindergarten" => (
            format!("อนุบาลปีที่ {}", row.year),
            format!("K{}", row.year),
            format!("อ.{}", row.year),
            1,
        ),
        "primary" => (
            format!("ประถมศึกษาปีที่ {}", row.year),
            format!("P{}", row.year),
            format!("ป.{}", row.year),
            2,
        ),
        "secondary" => (
            format!("มัธยมศึกษาปีที่ {}", row.year),
            format!("M{}", row.year),
            format!("ม.{}", row.year),
            3,
        ),
        _ => (
            format!("Other {}", row.year),
            format!("O{}", row.year),
            format!("?{}", row.year),
            4,
        ),
    };
    GradeLevelLookupItem {
        id: row.id,
        code,
        name,
        short_name: Some(short_name),
        level_type: row.level_type,
        level_order: order_base * 100 + row.year,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_level_labels_are_human_readable_and_stably_ordered() {
        let item = grade_level_lookup_item(GradeLevelRow {
            id: Uuid::nil(),
            level_type: "secondary".to_string(),
            year: 2,
        });
        assert_eq!(item.name, "มัธยมศึกษาปีที่ 2");
        assert_eq!(item.short_name.as_deref(), Some("ม.2"));
        assert_eq!(item.level_order, 302);
    }
}
