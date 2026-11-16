use school_database::DbSession;
use school_errors::AppError;
use std::collections::HashMap;

use chrono::NaiveDate;
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use school_academic_core::ports::{
    TermPreparationContext, TermPreparationMappingKind, TermPreparationModule,
    TermPreparationModuleOutcome,
};

#[derive(FromRow)]
struct SourceCycle {
    id: Uuid,
    title: String,
    description: Option<String>,
    template_id: Uuid,
}

#[derive(FromRow)]
struct SourceTarget {
    cycle_id: Uuid,
    target_type: String,
    target_id: Option<Uuid>,
    required_observations: i32,
    priority: i32,
}

pub async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    _dates: &HashMap<NaiveDate, NaiveDate>,
) -> Result<TermPreparationModuleOutcome, AppError> {
    let cycles: Vec<SourceCycle> = DbSession::new(&mut **tx).query_as("supervision.services.term_preparation.apply",
        r#"SELECT id,title,description,template_id
           FROM supervision_cycles WHERE academic_term_id=$1 ORDER BY id"#,
    )
    .bind(context.source_term_id)
    .fetch_all()
    .await?;
    let source_ids = cycles.iter().map(|cycle| cycle.id).collect::<Vec<_>>();
    let targets: Vec<SourceTarget> = if source_ids.is_empty() {
        Vec::new()
    } else {
        DbSession::new(&mut **tx).query_as("supervision.services.term_preparation.apply",
            "SELECT cycle_id,target_type,target_id,required_observations,priority FROM supervision_cycle_targets WHERE cycle_id=ANY($1) ORDER BY cycle_id,priority,id",
        )
        .bind(&source_ids)
        .fetch_all()
        .await?
    };
    let mut cycle_map = HashMap::new();
    let mut target_ids = Vec::new();
    for cycle in cycles {
        let id = Uuid::now_v7();
        DbSession::new(&mut **tx)
            .query(
                "supervision.services.term_preparation.apply",
                r#"INSERT INTO supervision_cycles(
                   id,title,description,template_id,status,created_by,academic_year_id,academic_term_id
               ) VALUES($1,$2,$3,$4,'draft',$5,$6,$7)"#,
            )
            .bind(id)
            .bind(cycle.title)
            .bind(cycle.description)
            .bind(cycle.template_id)
            .bind(actor)
            .bind(context.target_year_id)
            .bind(context.target_term_id)
            .execute()
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
        DbSession::new(&mut **tx)
            .query(
                "supervision.services.term_preparation.apply",
                r#"INSERT INTO supervision_cycle_targets(
                   id,cycle_id,target_type,target_id,required_observations,priority
               ) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5)"#,
            )
            .bind(cycle_map[&target.cycle_id])
            .bind(target.target_type)
            .bind(target_id)
            .bind(target.required_observations)
            .bind(target.priority)
            .execute()
            .await?;
    }
    Ok(TermPreparationModuleOutcome {
        module: TermPreparationModule::Supervision,
        created_count: target_ids.len(),
        target_ids,
    })
}
