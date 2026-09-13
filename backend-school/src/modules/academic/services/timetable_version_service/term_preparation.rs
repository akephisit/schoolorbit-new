use std::collections::HashMap;

use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::academic::lifecycle::models::{
        TermPreparationContext, TermPreparationMappingKind, TermPreparationModule,
        TermPreparationModuleOutcome,
    },
};

#[derive(FromRow)]
struct SourceVersion {
    id: Uuid,
}

#[derive(FromRow)]
struct SourceBlock {
    id: Uuid,
    bell_schedule_period_id: Uuid,
    day_of_week: String,
    block_kind: String,
    scheduling_mode: Option<String>,
    learning_offering_id: Option<Uuid>,
    structural_kind: Option<String>,
    title: Option<String>,
    note: Option<String>,
    series_id: Option<Uuid>,
}

#[derive(FromRow)]
struct SourceBlockGroup {
    id: Uuid,
    block_id: Uuid,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    room_id: Option<Uuid>,
}

#[derive(FromRow)]
struct SourceInstructor {
    block_group_id: Uuid,
    instructor_id: Uuid,
    role: String,
    display_order: i32,
}

#[derive(FromRow)]
struct SourceHomeroom {
    block_id: Uuid,
    homeroom_id: Uuid,
    target_kind: String,
    room_id: Option<Uuid>,
}

#[derive(FromRow)]
struct SourceTeacher {
    block_id: Uuid,
    teacher_id: Uuid,
}

fn mapped(
    mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    kind: TermPreparationMappingKind,
    source: Uuid,
) -> Result<Uuid, AppError> {
    mappings
        .get(&(kind, source))
        .copied()
        .ok_or_else(|| AppError::Conflict(format!("ข้อมูลจับคู่ {} สำหรับตารางสอนไม่ครบ", kind.as_str())))
}

fn reserve_resource_slot(
    slots: &mut HashMap<(String, Uuid, Uuid), Uuid>,
    day: &str,
    period: Uuid,
    resource: Uuid,
    block: Uuid,
) -> bool {
    match slots.entry((day.to_owned(), period, resource)) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(block);
            true
        }
        std::collections::hash_map::Entry::Occupied(entry) => *entry.get() == block,
    }
}

pub(crate) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
) -> Result<TermPreparationModuleOutcome, AppError> {
    let source: Option<SourceVersion> = sqlx::query_as(
        "SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1",
    )
    .bind(context.source_term_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(source) = source else {
        return Ok(TermPreparationModuleOutcome {
            module: TermPreparationModule::Timetable,
            created_count: 0,
            target_ids: Vec::new(),
        });
    };
    let target_bell_schedule_id: Uuid = sqlx::query_scalar(
        "SELECT bell_schedule_id FROM academic_terms WHERE id=$1 AND academic_year_id=$2",
    )
    .bind(context.target_term_id)
    .bind(context.target_year_id)
    .fetch_one(&mut **tx)
    .await?;
    let version_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_timetable_versions(
               id,academic_term_id,academic_year_id,effective_from,status,
               source_version_id,change_set_id,bell_schedule_id,row_version,created_by
           ) VALUES($1,$2,$3,$4,'draft',NULL,NULL,$5,1,$6)"#,
    )
    .bind(version_id)
    .bind(context.target_term_id)
    .bind(context.target_year_id)
    .bind(context.target_start_date)
    .bind(target_bell_schedule_id)
    .bind(actor)
    .execute(&mut **tx)
    .await?;

    let source_targets: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT learning_offering_id FROM academic_timetable_version_targets WHERE timetable_version_id=$1 ORDER BY learning_offering_id",
    )
    .bind(source.id)
    .fetch_all(&mut **tx)
    .await?;
    for (source_offering_id,) in source_targets {
        let target_offering_id = mapped(
            mappings,
            TermPreparationMappingKind::LearningOffering,
            source_offering_id,
        )?;
        let weekly_period_target: Option<i32> = sqlx::query_scalar(
            r#"SELECT COALESCE(
                   (SELECT version.periods_per_week
                    FROM course_offering_details detail
                    JOIN subject_versions version ON version.id=detail.subject_version_id
                    WHERE detail.learning_offering_id=offering.id),
                   (SELECT version.periods_per_week
                    FROM activity_offering_details detail
                    JOIN activity_versions version ON version.id=detail.activity_version_id
                    WHERE detail.learning_offering_id=offering.id)
               )
               FROM learning_offerings offering WHERE offering.id=$1 AND offering.academic_term_id=$2"#,
        )
        .bind(target_offering_id)
        .bind(context.target_term_id)
        .fetch_optional(&mut **tx)
        .await?
        .flatten()
        .ok_or_else(|| AppError::Conflict("รายการเปิดสอนเป้าหมายไม่มีค่าคาบมาตรฐาน".into()))?;
        sqlx::query(
            "INSERT INTO academic_timetable_version_targets(timetable_version_id,learning_offering_id,academic_term_id,academic_year_id,weekly_period_target) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(version_id)
        .bind(target_offering_id)
        .bind(context.target_term_id)
        .bind(context.target_year_id)
        .bind(weekly_period_target)
        .execute(&mut **tx)
        .await?;
    }

    let blocks: Vec<SourceBlock> = sqlx::query_as(
        r#"SELECT id,bell_schedule_period_id,day_of_week,block_kind,scheduling_mode,
                  learning_offering_id,structural_kind,title,note,series_id
           FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND is_active
           ORDER BY day_of_week,bell_schedule_period_id,id"#,
    )
    .bind(source.id)
    .fetch_all(&mut **tx)
    .await?;
    let block_ids = blocks.iter().map(|row| row.id).collect::<Vec<_>>();
    let block_groups: Vec<SourceBlockGroup> = if block_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
        "SELECT id,block_id,learning_group_id,learning_offering_id,room_id FROM academic_timetable_block_groups WHERE block_id=ANY($1) AND is_active ORDER BY block_id,id",
    ).bind(&block_ids).fetch_all(&mut **tx).await?
    };
    let source_group_ids = block_groups.iter().map(|row| row.id).collect::<Vec<_>>();
    let instructors: Vec<SourceInstructor> = if source_group_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
        "SELECT block_group_id,instructor_id,role,display_order FROM academic_timetable_block_group_instructors WHERE block_group_id=ANY($1) ORDER BY block_group_id,display_order,id",
    ).bind(&source_group_ids).fetch_all(&mut **tx).await?
    };
    let homerooms: Vec<SourceHomeroom> = if block_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
        "SELECT block_id,homeroom_id,target_kind,room_id FROM academic_timetable_block_homerooms WHERE block_id=ANY($1) AND is_active ORDER BY block_id,id",
    ).bind(&block_ids).fetch_all(&mut **tx).await?
    };
    let teachers: Vec<SourceTeacher> = if block_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
        "SELECT block_id,teacher_id FROM academic_timetable_block_teachers WHERE block_id=ANY($1) AND is_active ORDER BY block_id,id",
    ).bind(&block_ids).fetch_all(&mut **tx).await?
    };

    let mut block_map = HashMap::new();
    let mut block_group_map = HashMap::new();
    let mut teacher_slots = HashMap::new();
    let mut room_slots = HashMap::new();
    let mut homeroom_slots = HashMap::new();
    let mut series_map = HashMap::new();
    for block in blocks {
        let period_id = mapped(
            mappings,
            TermPreparationMappingKind::BellPeriod,
            block.bell_schedule_period_id,
        )?;
        let offering_id = block
            .learning_offering_id
            .map(|id| mapped(mappings, TermPreparationMappingKind::LearningOffering, id))
            .transpose()?;
        let target_block_id = Uuid::new_v4();
        let target_series_id = block.series_id.map(|source_series_id| {
            *series_map
                .entry(source_series_id)
                .or_insert_with(Uuid::new_v4)
        });
        sqlx::query(
            r#"INSERT INTO academic_timetable_blocks(
                   id,timetable_version_id,academic_term_id,academic_year_id,bell_schedule_id,
                   bell_schedule_period_id,day_of_week,block_kind,scheduling_mode,
                   learning_offering_id,structural_kind,title,note,series_id,row_version,
                   is_active,created_by,updated_by
               ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,1,true,$15,$15)"#,
        )
        .bind(target_block_id)
        .bind(version_id)
        .bind(context.target_term_id)
        .bind(context.target_year_id)
        .bind(target_bell_schedule_id)
        .bind(period_id)
        .bind(&block.day_of_week)
        .bind(&block.block_kind)
        .bind(&block.scheduling_mode)
        .bind(offering_id)
        .bind(&block.structural_kind)
        .bind(&block.title)
        .bind(&block.note)
        .bind(target_series_id)
        .bind(actor)
        .execute(&mut **tx)
        .await?;
        block_map.insert(block.id, (target_block_id, block.day_of_week, period_id));
    }
    for group in block_groups {
        let (target_block_id, day, period) = block_map[&group.block_id].clone();
        let target_group_id = mapped(
            mappings,
            TermPreparationMappingKind::LearningGroup,
            group.learning_group_id,
        )?;
        let target_offering_id = mapped(
            mappings,
            TermPreparationMappingKind::LearningOffering,
            group.learning_offering_id,
        )?;
        let room_id = group
            .room_id
            .map(|id| mapped(mappings, TermPreparationMappingKind::Room, id))
            .transpose()?;
        if room_id.is_some_and(|room| {
            !reserve_resource_slot(&mut room_slots, &day, period, room, target_block_id)
        }) {
            return Err(AppError::Conflict("ห้องเรียนเป้าหมายชนกันในตารางที่เตรียม".into()));
        }
        let target_block_group_id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO academic_timetable_block_groups(id,block_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,room_id,row_version,is_active,created_by,updated_by)
                       VALUES($1,$2,$3,$4,$5,$6,$7,1,true,$8,$8)"#)
            .bind(target_block_group_id).bind(target_block_id).bind(target_group_id).bind(target_offering_id)
            .bind(context.target_term_id).bind(context.target_year_id).bind(room_id).bind(actor).execute(&mut **tx).await?;
        block_group_map.insert(
            group.id,
            (target_block_group_id, target_block_id, day, period),
        );
    }
    for instructor in instructors {
        let (target_block_group_id, target_block_id, day, period) =
            block_group_map[&instructor.block_group_id].clone();
        let teacher = mapped(
            mappings,
            TermPreparationMappingKind::Teacher,
            instructor.instructor_id,
        )?;
        if !reserve_resource_slot(&mut teacher_slots, &day, period, teacher, target_block_id) {
            return Err(AppError::Conflict("ครูเป้าหมายชนกันในตารางที่เตรียม".into()));
        }
        sqlx::query("INSERT INTO academic_timetable_block_group_instructors(id,block_group_id,instructor_id,role,display_order) VALUES(gen_random_uuid(),$1,$2,$3,$4)")
            .bind(target_block_group_id).bind(teacher).bind(instructor.role).bind(instructor.display_order).execute(&mut **tx).await?;
    }
    for homeroom in homerooms {
        let (target_block_id, day, period) = block_map[&homeroom.block_id].clone();
        let target_homeroom = mapped(
            mappings,
            TermPreparationMappingKind::Homeroom,
            homeroom.homeroom_id,
        )?;
        let room = homeroom
            .room_id
            .map(|id| mapped(mappings, TermPreparationMappingKind::Room, id))
            .transpose()?;
        if !reserve_resource_slot(
            &mut homeroom_slots,
            &day,
            period,
            target_homeroom,
            target_block_id,
        ) {
            return Err(AppError::Conflict(
                "ห้องประจำชั้นเป้าหมายชนกันในตารางที่เตรียม".into(),
            ));
        }
        if room.is_some_and(|room| {
            !reserve_resource_slot(&mut room_slots, &day, period, room, target_block_id)
        }) {
            return Err(AppError::Conflict("ห้องเรียนเป้าหมายชนกันในตารางที่เตรียม".into()));
        }
        sqlx::query(r#"INSERT INTO academic_timetable_block_homerooms(id,block_id,homeroom_id,academic_term_id,academic_year_id,target_kind,room_id,row_version,is_active,created_by,updated_by)
                       VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,$6,1,true,$7,$7)"#)
            .bind(target_block_id).bind(target_homeroom).bind(context.target_term_id).bind(context.target_year_id).bind(homeroom.target_kind).bind(room).bind(actor).execute(&mut **tx).await?;
    }
    for teacher in teachers {
        let (target_block_id, day, period) = block_map[&teacher.block_id].clone();
        let target_teacher = mapped(
            mappings,
            TermPreparationMappingKind::Teacher,
            teacher.teacher_id,
        )?;
        if !reserve_resource_slot(
            &mut teacher_slots,
            &day,
            period,
            target_teacher,
            target_block_id,
        ) {
            return Err(AppError::Conflict("ครูเป้าหมายชนกันในตารางที่เตรียม".into()));
        }
        sqlx::query(r#"INSERT INTO academic_timetable_block_teachers(id,block_id,teacher_id,academic_term_id,academic_year_id,row_version,is_active,created_by,updated_by)
                       VALUES(gen_random_uuid(),$1,$2,$3,$4,1,true,$5,$5)"#)
            .bind(target_block_id).bind(target_teacher).bind(context.target_term_id).bind(context.target_year_id).bind(actor).execute(&mut **tx).await?;
    }
    Ok(TermPreparationModuleOutcome {
        module: TermPreparationModule::Timetable,
        created_count: block_map.len() + 1,
        target_ids: vec![version_id],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_block_can_reserve_the_same_resource_but_distinct_blocks_collide() {
        let mut slots = HashMap::new();
        let period = Uuid::new_v4();
        let teacher = Uuid::new_v4();
        let shared_block = Uuid::new_v4();

        assert!(reserve_resource_slot(
            &mut slots,
            "MONDAY",
            period,
            teacher,
            shared_block
        ));
        assert!(reserve_resource_slot(
            &mut slots,
            "MONDAY",
            period,
            teacher,
            shared_block
        ));
        assert!(!reserve_resource_slot(
            &mut slots,
            "MONDAY",
            period,
            teacher,
            Uuid::new_v4()
        ));
    }
}
