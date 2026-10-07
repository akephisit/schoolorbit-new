use crate::models::{
    versions::{DeliveryChangeKind, DeliverySnapshot, DeliveryVersionChange},
    LearningTeacherRole,
};
use school_errors::AppError;
use sqlx::{Executor, Postgres};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

fn role(role: LearningTeacherRole) -> &'static str {
    match role {
        LearningTeacherRole::Primary => "ครูหลัก",
        LearningTeacherRole::Secondary => "ครูร่วม",
        LearningTeacherRole::Assistant => "ครูผู้ช่วย",
    }
}

/// Compare captured facts by stable resource IDs, independent of database revisions.
pub fn compare(
    before: &DeliverySnapshot,
    after: &DeliverySnapshot,
    labels: &BTreeMap<Uuid, String>,
) -> Vec<DeliveryVersionChange> {
    let mut result = Vec::new();
    let old: BTreeMap<_, _> = before.offerings.iter().map(|o| (o.id, o)).collect();
    let new: BTreeMap<_, _> = after.offerings.iter().map(|o| (o.id, o)).collect();
    let mut push = |offering_id: Uuid,
                    resource_id: Uuid,
                    label: String,
                    field: &str,
                    before: Option<String>,
                    after: Option<String>| {
        if before == after {
            return;
        }
        let kind = if before.is_none() {
            DeliveryChangeKind::Added
        } else if after.is_none() {
            DeliveryChangeKind::Removed
        } else {
            DeliveryChangeKind::Changed
        };
        result.push(DeliveryVersionChange {
            kind,
            learning_offering_id: offering_id,
            resource_id,
            label,
            field: field.into(),
            before,
            after,
        });
    };
    for id in old
        .keys()
        .chain(new.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        let a = old.get(&id).copied();
        let b = new.get(&id).copied();
        let Some(o) = b.or(a) else {
            continue;
        };
        let label = format!("{} — {}", o.code, o.name);
        push(
            id,
            id,
            label.clone(),
            "รายการเปิดสอน",
            a.map(|x| format!("เปิดสอน {} คาบ/สัปดาห์", x.weekly_period_target)),
            b.map(|x| format!("เปิดสอน {} คาบ/สัปดาห์", x.weekly_period_target)),
        );
        if let (Some(a), Some(b)) = (a, b) {
            push(
                id,
                id,
                label.clone(),
                "ชื่อ",
                Some(a.name.clone()),
                Some(b.name.clone()),
            );
            push(
                id,
                id,
                label.clone(),
                "รหัส",
                Some(a.code.clone()),
                Some(b.code.clone()),
            );
            let target_ids = |x: &crate::models::versions::DeliveryVersionOffering| {
                x.targets
                    .iter()
                    .map(|t| (t.homeroom_id, t.grade_level_id, t.study_program_id))
                    .collect::<BTreeSet<_>>()
            };
            if target_ids(a) != target_ids(b) {
                let describe = |x: &crate::models::versions::DeliveryVersionOffering| {
                    x.targets
                        .iter()
                        .map(|t| {
                            t.homeroom_id.map_or_else(
                                || {
                                    format!(
                                        "{} · {}",
                                        named(labels, t.grade_level_id),
                                        named(labels, t.study_program_id)
                                    )
                                },
                                |id| named(labels, id),
                            )
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                push(
                    id,
                    id,
                    label.clone(),
                    "กลุ่มเป้าหมาย",
                    Some(describe(a)),
                    Some(describe(b)),
                );
            }
            if sorted(&a.homeroom_ids) != sorted(&b.homeroom_ids) {
                push(
                    id,
                    id,
                    label.clone(),
                    "ห้องที่เปิดสอน",
                    Some(describe_ids(labels, &a.homeroom_ids)),
                    Some(describe_ids(labels, &b.homeroom_ids)),
                );
            }
        }
        let ag: BTreeMap<_, _> = a
            .into_iter()
            .flat_map(|o| &o.groups)
            .map(|g| (g.id, g))
            .collect();
        let bg: BTreeMap<_, _> = b
            .into_iter()
            .flat_map(|o| &o.groups)
            .map(|g| (g.id, g))
            .collect();
        for gid in ag
            .keys()
            .chain(bg.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            let a = ag.get(&gid).copied();
            let b = bg.get(&gid).copied();
            let Some(g) = b.or(a) else {
                continue;
            };
            let label = format!("{} · {}", o.name, g.name);
            push(
                id,
                gid,
                label.clone(),
                "กลุ่มเรียน",
                a.map(|x| x.name.clone()),
                b.map(|x| x.name.clone()),
            );
            if let (Some(a), Some(b)) = (a, b) {
                push(
                    id,
                    gid,
                    label.clone(),
                    "ความจุ",
                    Some(a.capacity.map_or("ไม่จำกัด".into(), |x| x.to_string())),
                    Some(b.capacity.map_or("ไม่จำกัด".into(), |x| x.to_string())),
                );
                push(
                    id,
                    gid,
                    label.clone(),
                    "รหัสกลุ่ม",
                    Some(a.code.clone()),
                    Some(b.code.clone()),
                );
                push(
                    id,
                    gid,
                    label.clone(),
                    "คำอธิบายกลุ่ม",
                    Some(a.description.clone().unwrap_or_else(|| "ไม่ได้ระบุ".into())),
                    Some(b.description.clone().unwrap_or_else(|| "ไม่ได้ระบุ".into())),
                );
                if sorted(&a.homeroom_ids) != sorted(&b.homeroom_ids) {
                    push(
                        id,
                        gid,
                        label.clone(),
                        "ห้องที่เรียน",
                        Some(describe_ids(labels, &a.homeroom_ids)),
                        Some(describe_ids(labels, &b.homeroom_ids)),
                    );
                }
                if a.preferred_room_ids != b.preferred_room_ids {
                    push(
                        id,
                        gid,
                        label.clone(),
                        "ห้องเรียนที่ต้องการตามลำดับ",
                        Some(describe_ordered_ids(labels, &a.preferred_room_ids)),
                        Some(describe_ordered_ids(labels, &b.preferred_room_ids)),
                    );
                }
            }
            // A replaced episode for the same teacher/role is not a teaching-content change.
            let at: BTreeMap<_, _> = a
                .into_iter()
                .flat_map(|g| &g.teachers)
                .map(|t| (t.teacher_id, t))
                .collect();
            let bt: BTreeMap<_, _> = b
                .into_iter()
                .flat_map(|g| &g.teachers)
                .map(|t| (t.teacher_id, t))
                .collect();
            for tid in at
                .keys()
                .chain(bt.keys())
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
            {
                let a = at.get(&tid).copied();
                let b = bt.get(&tid).copied();
                let Some(t) = b.or(a) else {
                    continue;
                };
                push(
                    id,
                    gid,
                    format!("{label} · {}", t.display_name),
                    "ครูผู้สอน",
                    a.map(|x| role(x.role).into()),
                    b.map(|x| role(x.role).into()),
                );
            }
        }
    }
    result
}

fn sorted(ids: &[Uuid]) -> BTreeSet<Uuid> {
    ids.iter().copied().collect()
}
fn named(labels: &BTreeMap<Uuid, String>, id: Uuid) -> String {
    labels
        .get(&id)
        .cloned()
        .unwrap_or_else(|| "ชื่อข้อมูลไม่พร้อม กรุณาโหลดใหม่".into())
}
fn describe_ids(labels: &BTreeMap<Uuid, String>, ids: &[Uuid]) -> String {
    describe_ordered_ids(labels, &sorted(ids).into_iter().collect::<Vec<_>>())
}
fn describe_ordered_ids(labels: &BTreeMap<Uuid, String>, ids: &[Uuid]) -> String {
    if ids.is_empty() {
        "ไม่ได้กำหนด".into()
    } else {
        ids.iter()
            .map(|id| named(labels, *id))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub async fn load_labels<'e, E: Executor<'e, Database = Postgres>>(
    executor: E,
    graphs: &[&DeliverySnapshot],
) -> Result<BTreeMap<Uuid, String>, AppError> {
    let ids = graphs
        .iter()
        .flat_map(|s| &s.offerings)
        .flat_map(|o| {
            o.homeroom_ids
                .iter()
                .copied()
                .chain(o.targets.iter().flat_map(|t| {
                    [
                        t.homeroom_id,
                        Some(t.grade_level_id),
                        Some(t.study_program_id),
                    ]
                    .into_iter()
                    .flatten()
                }))
                .chain(o.groups.iter().flat_map(|g| {
                    g.homeroom_ids
                        .iter()
                        .copied()
                        .chain(g.preferred_room_ids.iter().copied())
                }))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let labels:Vec<(Uuid,String)>=sqlx::query_as("SELECT id,name FROM homerooms WHERE id=ANY($1)
        UNION ALL SELECT id,CASE level_type WHEN 'kindergarten' THEN 'อ.' WHEN 'primary' THEN 'ป.' ELSE 'ม.' END||year::text FROM grade_levels WHERE id=ANY($1)
        UNION ALL SELECT id,name_th FROM published_study_programs WHERE id=ANY($1)
        UNION ALL SELECT id,concat_ws(' — ',code,name_th) FROM rooms WHERE id=ANY($1)")
        .bind(ids).fetch_all(executor).await?;
    Ok(labels.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::versions::{
        DeliveryVersionGroup, DeliveryVersionOffering, DeliveryVersionTeacher,
    };
    use crate::models::{
        CourseOfferingSnapshot, LearningOfferingKind, LearningOfferingSnapshot,
        LearningOfferingTarget, OfferingTargetKind,
    };
    fn graph() -> DeliverySnapshot {
        DeliverySnapshot {
            offerings: vec![DeliveryVersionOffering {
                curriculum_sources: Vec::new(),
                id: Uuid::from_u128(1),
                kind: LearningOfferingKind::Course,
                code: "ค11101".into(),
                name: "คณิตศาสตร์".into(),
                owning_organization_unit_id: Uuid::from_u128(2),
                source_requirement_kind: None,
                source_requirement_id: None,
                weekly_period_target: 3,
                catalog: LearningOfferingSnapshot::Course(CourseOfferingSnapshot {
                    subject_version_id: Uuid::from_u128(3),
                    subject_id: Uuid::from_u128(4),
                    curriculum_course_requirement_id: None,
                    credit: "1.0".into(),
                    hours: None,
                    standard_periods_per_week: 3,
                    assessment_total_score: "100".into(),
                }),
                targets: vec![LearningOfferingTarget {
                    id: Uuid::from_u128(5),
                    target_kind: OfferingTargetKind::Homeroom,
                    homeroom_id: Some(Uuid::from_u128(6)),
                    grade_level_id: Uuid::from_u128(7),
                    study_program_id: Uuid::from_u128(8),
                }],
                homeroom_ids: vec![Uuid::from_u128(6)],
                groups: vec![DeliveryVersionGroup {
                    id: Uuid::from_u128(9),
                    code: "ก1".into(),
                    name: "กลุ่มหนึ่ง".into(),
                    description: None,
                    capacity: Some(30),
                    homeroom_ids: vec![Uuid::from_u128(6)],
                    preferred_room_ids: vec![],
                    teachers: vec![DeliveryVersionTeacher {
                        assignment_id: Uuid::from_u128(10),
                        teacher_id: Uuid::from_u128(11),
                        display_name: "ครูตัวอย่าง".into(),
                        role: LearningTeacherRole::Primary,
                    }],
                }],
            }],
        }
    }
    #[test]
    fn names_and_counts_and_roles_show_before_after() {
        let before = graph();
        let mut after = before.clone();
        after.offerings[0].weekly_period_target = 4;
        after.offerings[0].groups[0].teachers[0].role = LearningTeacherRole::Secondary;
        let diff = compare(&before, &after, &BTreeMap::new());
        assert_eq!(diff.len(), 2);
        assert_eq!(diff[0].before.as_deref(), Some("เปิดสอน 3 คาบ/สัปดาห์"));
        assert_eq!(diff[0].after.as_deref(), Some("เปิดสอน 4 คาบ/สัปดาห์"));
        assert!(diff[1].label.contains("ครูตัวอย่าง"));
        assert_eq!(diff[1].after.as_deref(), Some("ครูร่วม"));
    }
    #[test]
    fn stable_ids_match_without_guessing_names() {
        let before = graph();
        let mut after = before.clone();
        after.offerings[0].id = Uuid::from_u128(12);
        let diff = compare(&before, &after, &BTreeMap::new());
        assert!(diff.iter().any(|d| d.kind == DeliveryChangeKind::Removed));
        assert!(diff.iter().any(|d| d.kind == DeliveryChangeKind::Added));
    }
    #[test]
    fn episode_identity_and_order_do_not_imply_content_change() {
        let before = graph();
        let mut after = before.clone();
        after.offerings[0].groups[0].teachers[0].assignment_id = Uuid::from_u128(22);
        after.offerings[0].targets[0].id = Uuid::from_u128(23);
        assert!(compare(&before, &after, &BTreeMap::new()).is_empty());
    }
    #[test]
    fn same_count_different_rooms_are_named() {
        let before = graph();
        let mut after = before.clone();
        after.offerings[0].groups[0].homeroom_ids = vec![Uuid::from_u128(16)];
        let labels = BTreeMap::from([
            (Uuid::from_u128(6), "ม.1/1".into()),
            (Uuid::from_u128(16), "ม.1/2".into()),
        ]);
        let diff = compare(&before, &after, &labels);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].before.as_deref(), Some("ม.1/1"));
        assert_eq!(diff[0].after.as_deref(), Some("ม.1/2"));
    }
}
