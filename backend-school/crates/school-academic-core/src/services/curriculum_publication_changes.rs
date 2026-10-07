use crate::models::CurriculumPublicationChange;
use school_errors::AppError;
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Entry {
    key: String,
    resource_kind: String,
    name: String,
    description: String,
    fingerprint: String,
    publication_id: Uuid,
}

pub(super) async fn compare(
    pool: &PgPool,
    before: Option<Uuid>,
    after: Uuid,
) -> Result<Vec<CurriculumPublicationChange>, AppError> {
    let rows: Vec<Entry> = sqlx::query_as(r#"
SELECT 'level:'||r.id::text AS key,'level'::text AS resource_kind,r.name_th AS name,concat(r.name_th,' • ชั้น ',(SELECT string_agg(g.level_type||' '||g.year::text,', ' ORDER BY g.year,g.id) FROM grade_levels g WHERE r.grade_level_ids ? g.id::text),' • ',COALESCE(r.description,'')) AS description,(to_jsonb(r)-ARRAY['id','publication_id','row_version','created_at','updated_at','migration_provenance'])::text AS fingerprint,r.publication_id FROM curriculum_publication_levels r WHERE r.publication_id=ANY($1)
UNION ALL
SELECT 'program:'||r.id::text AS key,'program'::text AS resource_kind,r.name_th AS name,concat(r.name_th,' • ',CASE WHEN r.is_default THEN 'แผนหลัก' ELSE 'แผนการเรียน' END,' • ',r.status) AS description,(to_jsonb(r)-ARRAY['id','publication_id','row_version','created_at','updated_at','migration_provenance'])::text AS fingerprint,r.publication_id FROM curriculum_publication_programs r WHERE r.publication_id=ANY($1)
UNION ALL
SELECT 'term_slot:'||r.id::text AS key,'term_slot'::text AS resource_kind,r.name AS name,concat(r.name,' • ',r.term_type,' ',r.type_occurrence::text,' • ลำดับ ',r.sequence::text) AS description,(to_jsonb(r)-ARRAY['id','publication_id','row_version','created_at','updated_at','migration_provenance'])::text AS fingerprint,r.publication_id FROM curriculum_publication_slots r WHERE r.publication_id=ANY($1)
UNION ALL
SELECT concat('course:',r.study_program_id,':',r.grade_level_id,':',r.term_slot_id,':',r.subject_version_id) AS key,
 'course'::text AS resource_kind,v.name_th AS name,
 concat(c.code,' ',v.name_th,' • ',p.name_th,' • ',g.level_type,' ',g.year,' • ',s.name,' • ',r.requirement_kind,' • ลำดับ ',r.display_order) AS description,
 (to_jsonb(r)-ARRAY['id','publication_id','row_version','created_at','updated_at','migration_provenance'])::text AS fingerprint,r.publication_id
 FROM curriculum_publication_courses r JOIN subject_versions v ON v.id=r.subject_version_id JOIN subjects c ON c.id=v.subject_id
 JOIN curriculum_publication_programs p ON p.publication_id=r.publication_id AND p.id=r.study_program_id
 JOIN curriculum_publication_slots s ON s.publication_id=r.publication_id AND s.id=r.term_slot_id
 JOIN grade_levels g ON g.id=r.grade_level_id WHERE r.publication_id=ANY($1)
UNION ALL
SELECT concat('activity:',r.study_program_id,':',r.grade_level_id,':',r.term_slot_id,':',r.activity_version_id) AS key,
 'activity'::text AS resource_kind,v.name AS name,
 concat(c.code,' ',v.name,' • ',p.name_th,' • ',g.level_type,' ',g.year,' • ',s.name,' • ',r.requirement_kind,' • ลำดับ ',r.display_order) AS description,
 (to_jsonb(r)-ARRAY['id','publication_id','row_version','created_at','updated_at','migration_provenance'])::text AS fingerprint,r.publication_id
 FROM curriculum_publication_activities r JOIN activity_versions v ON v.id=r.activity_version_id JOIN activities c ON c.id=v.activity_id
 JOIN curriculum_publication_programs p ON p.publication_id=r.publication_id AND p.id=r.study_program_id
 JOIN curriculum_publication_slots s ON s.publication_id=r.publication_id AND s.id=r.term_slot_id
 JOIN grade_levels g ON g.id=r.grade_level_id WHERE r.publication_id=ANY($1)
 LIMIT 100001"#).bind(before.into_iter().chain(std::iter::once(after)).collect::<Vec<_>>()).fetch_all(pool).await?;
    if rows.len() > 100_000 {
        return Err(AppError::ValidationError(
            "ข้อมูลประวัติหลักสูตรเกินขอบเขตที่รองรับ".into(),
        ));
    }
    Ok(changes(rows, before, after))
}

fn changes(
    rows: Vec<Entry>,
    before: Option<Uuid>,
    after: Uuid,
) -> Vec<CurriculumPublicationChange> {
    let mut older = BTreeMap::new();
    let mut newer = BTreeMap::new();
    for row in rows {
        if row.publication_id == after {
            newer.insert(row.key.clone(), row);
        } else if Some(row.publication_id) == before {
            older.insert(row.key.clone(), row);
        }
    }
    let keys = older
        .keys()
        .chain(newer.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    keys.into_iter()
        .filter_map(|key| {
            let a = older.get(&key);
            let b = newer.get(&key);
            if a.zip(b)
                .is_some_and(|(a, b)| a.fingerprint == b.fingerprint)
            {
                return None;
            }
            let row = b.or(a)?;
            Some(CurriculumPublicationChange {
                resource_kind: row.resource_kind.clone(),
                name: row.name.clone(),
                before: a.map(|r| r.description.clone()),
                after: b.map(|r| r.description.clone()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(publication_id: Uuid, key: &str, value: &str) -> Entry {
        Entry {
            key: key.into(),
            resource_kind: "course".into(),
            name: "คณิตศาสตร์".into(),
            description: value.into(),
            fingerprint: value.into(),
            publication_id,
        }
    }
    #[test]
    fn semantic_comparison_ignores_new_row_ids_but_reports_actual_edits() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let rows = vec![
            entry(a, "same", "เดิม"),
            entry(b, "same", "เดิม"),
            entry(a, "remove", "ลบ"),
            entry(b, "add", "เพิ่ม"),
            entry(a, "edit", "ก่อน"),
            entry(b, "edit", "หลัง"),
        ];
        let result = changes(rows, Some(a), b);
        assert_eq!(result.len(), 3);
        assert!(result
            .iter()
            .any(|r| r.before.is_none() && r.after.as_deref() == Some("เพิ่ม")));
        assert!(result
            .iter()
            .any(|r| r.before.as_deref() == Some("ลบ") && r.after.is_none()));
        assert!(result
            .iter()
            .any(|r| r.before.as_deref() == Some("ก่อน") && r.after.as_deref() == Some("หลัง")));
    }
}
