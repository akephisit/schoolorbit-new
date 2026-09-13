use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};
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
struct SourceRound {
    id: Uuid,
    name: String,
    description: Option<String>,
    exam_kind: String,
}

#[derive(FromRow)]
struct SourceDay {
    id: Uuid,
    exam_round_id: Uuid,
    exam_date: NaiveDate,
    label: Option<String>,
    start_time: NaiveTime,
    end_time: NaiveTime,
}

#[derive(FromRow)]
struct SourceRoomAssignment {
    id: Uuid,
    exam_day_id: Uuid,
    homeroom_id: Uuid,
    room_id: Uuid,
    capacity_override: Option<i32>,
}

#[derive(FromRow)]
struct SourceItem {
    id: Uuid,
    exam_round_id: Uuid,
    assessment_phase_id: Uuid,
    learning_offering_id: Uuid,
    learning_group_id: Uuid,
    homeroom_id: Uuid,
    duration_minutes: i32,
}

#[derive(FromRow)]
struct SourceSession {
    exam_schedule_item_id: Uuid,
    exam_round_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
}

fn mapped(
    mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    kind: TermPreparationMappingKind,
    source: Uuid,
) -> Result<Uuid, AppError> {
    mappings
        .get(&(kind, source))
        .copied()
        .ok_or_else(|| AppError::Conflict(format!("ข้อมูลจับคู่ {} สำหรับตารางสอบไม่ครบ", kind.as_str())))
}

pub(crate) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    dates: &HashMap<NaiveDate, NaiveDate>,
) -> Result<TermPreparationModuleOutcome, AppError> {
    let rounds: Vec<SourceRound> = sqlx::query_as(
        "SELECT id,name,description,exam_kind FROM academic_exam_rounds WHERE academic_term_id=$1 ORDER BY created_at,id",
    )
    .bind(context.source_term_id)
    .fetch_all(&mut **tx)
    .await?;
    let source_round_ids = rounds.iter().map(|row| row.id).collect::<Vec<_>>();
    let days: Vec<SourceDay> = if source_round_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT id,exam_round_id,exam_date,label,start_time,end_time FROM academic_exam_days WHERE exam_round_id=ANY($1) ORDER BY exam_round_id,exam_date,id",
        )
        .bind(&source_round_ids)
        .fetch_all(&mut **tx)
        .await?
    };
    let source_day_ids = days.iter().map(|row| row.id).collect::<Vec<_>>();
    let assignments: Vec<SourceRoomAssignment> = if source_day_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT id,exam_day_id,homeroom_id,room_id,capacity_override FROM academic_exam_day_room_assignments WHERE exam_day_id=ANY($1) ORDER BY exam_day_id,id",
        )
        .bind(&source_day_ids)
        .fetch_all(&mut **tx)
        .await?
    };
    let items: Vec<SourceItem> = if source_round_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT id,exam_round_id,assessment_phase_id,learning_offering_id,learning_group_id,homeroom_id,duration_minutes FROM academic_exam_schedule_items WHERE exam_round_id=ANY($1) ORDER BY exam_round_id,id",
        )
        .bind(&source_round_ids)
        .fetch_all(&mut **tx)
        .await?
    };
    let source_item_ids = items.iter().map(|row| row.id).collect::<Vec<_>>();
    let sessions: Vec<SourceSession> = if source_item_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT exam_schedule_item_id,exam_round_id,exam_day_id,starts_at,ends_at FROM academic_exam_sessions WHERE exam_schedule_item_id=ANY($1) ORDER BY exam_round_id,exam_day_id,starts_at,exam_schedule_item_id",
        )
        .bind(&source_item_ids)
        .fetch_all(&mut **tx)
        .await?
    };

    let mut round_map = HashMap::new();
    let mut target_ids = Vec::new();
    for round in rounds {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO academic_exam_rounds(
                   id,academic_year_id,academic_term_id,name,description,exam_kind,status,
                   published_at,published_by,created_by,updated_by,row_version
               ) VALUES($1,$2,$3,$4,$5,$6,'draft',NULL,NULL,$7,$7,1)"#,
        )
        .bind(id)
        .bind(context.target_year_id)
        .bind(context.target_term_id)
        .bind(round.name)
        .bind(round.description)
        .bind(round.exam_kind)
        .bind(actor)
        .execute(&mut **tx)
        .await?;
        round_map.insert(round.id, id);
        target_ids.push(id);
    }

    let mut day_map = HashMap::new();
    for day in days {
        let target_date = dates
            .get(&day.exam_date)
            .copied()
            .ok_or_else(|| AppError::Conflict(format!("ไม่พบการจับคู่วันสอบ {}", day.exam_date)))?;
        if target_date < context.target_start_date || target_date > context.target_end_date {
            return Err(AppError::Conflict("วันสอบเป้าหมายอยู่นอกภาคเรียน".into()));
        }
        let id = Uuid::new_v4();
        let target_round = round_map[&day.exam_round_id];
        sqlx::query(
            r#"INSERT INTO academic_exam_days(
                   id,exam_round_id,exam_date,label,start_time,end_time,
                   academic_term_id,academic_year_id
               ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)"#,
        )
        .bind(id)
        .bind(target_round)
        .bind(target_date)
        .bind(day.label)
        .bind(day.start_time)
        .bind(day.end_time)
        .bind(context.target_term_id)
        .bind(context.target_year_id)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO academic_exam_day_grade_levels(exam_day_id,grade_level_id)
               SELECT $1,grade_level_id FROM academic_exam_day_grade_levels WHERE exam_day_id=$2"#,
        )
        .bind(id)
        .bind(day.id)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO academic_exam_day_blocked_windows(id,exam_day_id,label,start_time,end_time)
               SELECT gen_random_uuid(),$1,label,start_time,end_time
               FROM academic_exam_day_blocked_windows WHERE exam_day_id=$2"#,
        )
        .bind(id)
        .bind(day.id)
        .execute(&mut **tx)
        .await?;
        day_map.insert(day.id, id);
    }

    for assignment in assignments {
        let target_day = day_map[&assignment.exam_day_id];
        let homeroom = mapped(
            mappings,
            TermPreparationMappingKind::Homeroom,
            assignment.homeroom_id,
        )?;
        let room = mapped(
            mappings,
            TermPreparationMappingKind::Room,
            assignment.room_id,
        )?;
        sqlx::query(
            r#"INSERT INTO academic_exam_day_room_assignments(
                   id,exam_day_id,academic_term_id,academic_year_id,homeroom_id,room_id,
                   capacity_override,created_by,updated_by
               ) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,$6,$7,$7)"#,
        )
        .bind(target_day)
        .bind(context.target_term_id)
        .bind(context.target_year_id)
        .bind(homeroom)
        .bind(room)
        .bind(assignment.capacity_override)
        .bind(actor)
        .execute(&mut **tx)
        .await?;
    }

    let mut item_map = HashMap::new();
    for item in items {
        let offering = mapped(
            mappings,
            TermPreparationMappingKind::LearningOffering,
            item.learning_offering_id,
        )?;
        let group = mapped(
            mappings,
            TermPreparationMappingKind::LearningGroup,
            item.learning_group_id,
        )?;
        let homeroom = mapped(
            mappings,
            TermPreparationMappingKind::Homeroom,
            item.homeroom_id,
        )?;
        let (phase_code,): (String,) =
            sqlx::query_as("SELECT phase_code FROM course_assessment_phases WHERE id=$1")
                .bind(item.assessment_phase_id)
                .fetch_one(&mut **tx)
                .await?;
        let (plan_id, phase_id, subject_id, grade_level_id): (Uuid,Uuid,Uuid,Uuid) = sqlx::query_as(
            r#"SELECT plan.id,phase.id,detail.subject_id,homeroom.grade_level_id
               FROM course_assessment_plans plan
               JOIN course_assessment_phases phase ON phase.plan_id=plan.id AND phase.phase_code=$2
               JOIN course_offering_details detail ON detail.learning_offering_id=plan.learning_offering_id
               CROSS JOIN homerooms homeroom
               WHERE plan.learning_offering_id=$1 AND plan.academic_term_id=$3 AND homeroom.id=$4"#,
        ).bind(offering).bind(&phase_code).bind(context.target_term_id).bind(homeroom)
        .fetch_optional(&mut **tx).await?.ok_or_else(|| AppError::Conflict("โครงสร้างคะแนนเป้าหมายของรายการสอบยังไม่พร้อม".into()))?;
        let id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO academic_exam_schedule_items(
                          id,exam_round_id,academic_term_id,academic_year_id,assessment_phase_id,
                          course_assessment_plan_id,learning_offering_id,learning_group_id,homeroom_id,
                          subject_id,grade_level_id,duration_minutes,row_version
                      ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,1)"#)
            .bind(id).bind(round_map[&item.exam_round_id]).bind(context.target_term_id).bind(context.target_year_id)
            .bind(phase_id).bind(plan_id).bind(offering).bind(group).bind(homeroom).bind(subject_id).bind(grade_level_id)
            .bind(item.duration_minutes).execute(&mut **tx).await?;
        item_map.insert(item.id, id);
    }
    for session in sessions {
        sqlx::query(r#"INSERT INTO academic_exam_sessions(
                          id,exam_schedule_item_id,exam_round_id,exam_day_id,starts_at,ends_at,created_by,updated_by)
                      VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,$6,$6)"#)
            .bind(item_map[&session.exam_schedule_item_id]).bind(round_map[&session.exam_round_id])
            .bind(day_map[&session.exam_day_id]).bind(session.starts_at).bind(session.ends_at).bind(actor)
            .execute(&mut **tx).await?;
    }
    Ok(TermPreparationModuleOutcome {
        module: TermPreparationModule::Exams,
        created_count: target_ids.len(),
        target_ids,
    })
}
