use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
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
struct SourceCycle {
    id: Uuid,
    title: String,
    description: Option<String>,
    template_id: Uuid,
    booking_opens_at: Option<DateTime<Utc>>,
    booking_closes_at: Option<DateTime<Utc>>,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct SourceTarget {
    cycle_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
    required_observations: i32,
    priority: i32,
}

fn mapped_date(
    value: DateTime<Utc>,
    dates: &HashMap<NaiveDate, NaiveDate>,
) -> Result<DateTime<Utc>, AppError> {
    let target_date = dates.get(&value.date_naive()).copied().ok_or_else(|| {
        AppError::Conflict(format!("ไม่พบการจับคู่วันที่ของรอบนิเทศ {}", value.date_naive()))
    })?;
    Ok(DateTime::from_naive_utc_and_offset(
        target_date.and_time(value.time()),
        Utc,
    ))
}

pub(crate) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    dates: &HashMap<NaiveDate, NaiveDate>,
) -> Result<TermPreparationModuleOutcome, AppError> {
    let cycles: Vec<SourceCycle> = sqlx::query_as(
        r#"SELECT id,title,description,template_id,booking_opens_at,booking_closes_at,starts_at,ends_at
           FROM supervision_cycles WHERE academic_term_id=$1 ORDER BY starts_at,id"#,
    )
    .bind(context.source_term_id)
    .fetch_all(&mut **tx)
    .await?;
    let source_ids = cycles.iter().map(|cycle| cycle.id).collect::<Vec<_>>();
    let targets: Vec<SourceTarget> = if source_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT cycle_id,target_type,target_id,required_observations,priority FROM supervision_cycle_targets WHERE cycle_id=ANY($1) ORDER BY cycle_id,priority,id",
        )
        .bind(&source_ids)
        .fetch_all(&mut **tx)
        .await?
    };
    let mut cycle_map = HashMap::new();
    let mut target_ids = Vec::new();
    for cycle in cycles {
        let starts_at = mapped_date(cycle.starts_at, dates)?;
        let ends_at = mapped_date(cycle.ends_at, dates)?;
        let booking_opens_at = cycle
            .booking_opens_at
            .map(|value| mapped_date(value, dates))
            .transpose()?;
        let booking_closes_at = cycle
            .booking_closes_at
            .map(|value| mapped_date(value, dates))
            .transpose()?;
        if starts_at.date_naive() < context.target_start_date
            || ends_at.date_naive() > context.target_end_date
        {
            return Err(AppError::Conflict(
                "ช่วงเวลารอบนิเทศต้องอยู่ในภาคเรียนเป้าหมาย".into(),
            ));
        }
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO supervision_cycles(
                   id,title,description,template_id,booking_opens_at,booking_closes_at,
                   starts_at,ends_at,status,created_by,academic_year_id,academic_term_id
               ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'draft',$9,$10,$11)"#,
        )
        .bind(id)
        .bind(cycle.title)
        .bind(cycle.description)
        .bind(cycle.template_id)
        .bind(booking_opens_at)
        .bind(booking_closes_at)
        .bind(starts_at)
        .bind(ends_at)
        .bind(actor)
        .bind(context.target_year_id)
        .bind(context.target_term_id)
        .execute(&mut **tx)
        .await?;
        cycle_map.insert(cycle.id, id);
        target_ids.push(id);
    }
    for target in targets {
        let target_id = match (target.target_type.as_str(), target.target_id) {
            ("staff", Some(source_id)) => Some(
                mappings
                    .get(&(TermPreparationMappingKind::Teacher, source_id))
                    .copied()
                    .ok_or_else(|| AppError::Conflict("ข้อมูลจับคู่บุคลากรรอบนิเทศไม่ครบ".into()))?,
            ),
            (_, value) => value,
        };
        sqlx::query(
            r#"INSERT INTO supervision_cycle_targets(
                   id,cycle_id,target_type,target_id,required_observations,priority
               ) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5)"#,
        )
        .bind(cycle_map[&target.cycle_id])
        .bind(target.target_type)
        .bind(target_id)
        .bind(target.required_observations)
        .bind(target.priority)
        .execute(&mut **tx)
        .await?;
    }
    Ok(TermPreparationModuleOutcome {
        module: TermPreparationModule::Supervision,
        created_count: target_ids.len(),
        target_ids,
    })
}
