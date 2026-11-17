//! Previewed permanent deletion of one cycle's owned graph.
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;
use crate::models::{DeleteSupervisionCycleRequest, SupervisionCycleDeletionCounts, SupervisionCycleDeletionImpact};
use super::lifecycle::{require_context_writes,SupervisionAcademicContext};

#[derive(sqlx::FromRow)]
struct ImpactRow { revision:String, targets:i64, observations:i64, periods:i64, evaluators:i64, responses:i64, actions:i64 }

fn stale()->AppError {AppError::Conflict("ข้อมูลรอบนิเทศเปลี่ยนแล้ว กรุณาตรวจสอบรายการที่จะลบใหม่".into())}

async fn contexts(connection:&mut PgConnection,id:Uuid)->Result<Vec<SupervisionAcademicContext>,AppError> {
    let cycle:SupervisionAcademicContext=DbSession::new(connection).query_as("supervision.cycle_deletion.context",
        "SELECT academic_year_id,academic_term_id FROM supervision_cycles WHERE id=$1")
        .bind(id).fetch_optional().await?.ok_or_else(||AppError::NotFound("ไม่พบรอบนิเทศ".into()))?;
    let mut result=vec![cycle];
    result.extend(DbSession::new(connection).query_as::<SupervisionAcademicContext>("supervision.cycle_deletion.child_contexts",
        "SELECT DISTINCT academic_year_id,academic_term_id FROM supervision_observations WHERE cycle_id=$1 ORDER BY 1,2")
        .bind(id).fetch_all().await?);
    result.sort_unstable();result.dedup();Ok(result)
}

async fn validate_graph(connection:&mut PgConnection,id:Uuid)->Result<(),AppError> {
    let malformed:bool=DbSession::new(connection).scalar("supervision.cycle_deletion.validate_graph",r#"
        SELECT EXISTS(SELECT 1 FROM supervision_evaluator_responses r
            JOIN supervision_evaluators e ON e.id=r.evaluator_id
            JOIN supervision_observations response_parent ON response_parent.id=r.observation_id
            JOIN supervision_observations evaluator_parent ON evaluator_parent.id=e.observation_id
            WHERE (response_parent.cycle_id=$1 OR evaluator_parent.cycle_id=$1)
              AND r.observation_id<>e.observation_id)
        OR EXISTS(SELECT 1 FROM supervision_observations o JOIN supervision_cycles c ON c.id=o.cycle_id
            WHERE c.id=$1 AND (o.academic_year_id<>c.academic_year_id OR
                (c.academic_term_id IS NOT NULL AND o.academic_term_id<>c.academic_term_id)))
    "#).bind(id).fetch_one().await?;
    let external:bool=DbSession::new(connection).scalar("supervision.cycle_deletion.dependencies",r#"
        SELECT EXISTS(SELECT 1 FROM pg_constraint WHERE contype='f' AND confrelid=ANY(ARRAY[
            'supervision_cycles'::regclass,'supervision_cycle_targets'::regclass,'supervision_observations'::regclass,
            'supervision_observation_periods'::regclass,'supervision_evaluators'::regclass,
            'supervision_evaluator_responses'::regclass,'supervision_actions'::regclass])
          AND NOT EXISTS(SELECT 1 FROM (VALUES
            ('supervision_cycle_targets'::regclass,'supervision_cycles'::regclass,'supervision_cycle_targets_cycle_id_fkey'),
            ('supervision_observations'::regclass,'supervision_cycles'::regclass,'supervision_observations_cycle_id_fkey'),
            ('supervision_observations'::regclass,'supervision_cycles'::regclass,'supervision_observations_cycle_year_fkey'),
            ('supervision_evaluators'::regclass,'supervision_observations'::regclass,'supervision_evaluators_observation_id_fkey'),
            ('supervision_evaluator_responses'::regclass,'supervision_observations'::regclass,'supervision_evaluator_responses_observation_id_fkey'),
            ('supervision_evaluator_responses'::regclass,'supervision_evaluators'::regclass,'supervision_evaluator_responses_evaluator_id_fkey'),
            ('supervision_actions'::regclass,'supervision_observations'::regclass,'supervision_actions_observation_id_fkey'),
            ('supervision_observation_periods'::regclass,'supervision_observations'::regclass,'supervision_periods_observation_context_fkey')
          ) allowed(child,parent,name) WHERE allowed.child=conrelid AND allowed.parent=confrelid
                AND allowed.name=conname AND confdeltype='c'))
    "#).fetch_one().await?;
    if malformed||external {return Err(AppError::Conflict("โครงสร้างข้อมูลรอบนิเทศไม่สมบูรณ์ จึงยังลบถาวรไม่ได้".into()));}
    Ok(())
}

async fn impact(connection:&mut PgConnection,id:Uuid)->Result<SupervisionCycleDeletionImpact,AppError> {
    let row:ImpactRow=DbSession::new(connection).query_as("supervision.cycle_deletion.impact",r#"
        WITH cycle AS (SELECT * FROM supervision_cycles WHERE id=$1),
        targets AS (SELECT * FROM supervision_cycle_targets WHERE cycle_id=$1),
        observations AS (SELECT * FROM supervision_observations WHERE cycle_id=$1),
        periods AS (SELECT p.* FROM supervision_observation_periods p JOIN observations o ON o.id=p.observation_id),
        evaluators AS (SELECT e.* FROM supervision_evaluators e JOIN observations o ON o.id=e.observation_id),
        responses AS (SELECT r.* FROM supervision_evaluator_responses r JOIN observations o ON o.id=r.observation_id),
        actions AS (SELECT a.* FROM supervision_actions a JOIN observations o ON o.id=a.observation_id)
        SELECT encode(sha256(convert_to(jsonb_build_array(
            (SELECT to_jsonb(c) FROM cycle c),
            (SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY id),'[]') FROM targets t),
            (SELECT COALESCE(jsonb_agg(to_jsonb(o) ORDER BY id),'[]') FROM observations o),
            (SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY observation_id,position,id),'[]') FROM periods p),
            (SELECT COALESCE(jsonb_agg(to_jsonb(e) ORDER BY id),'[]') FROM evaluators e),
            (SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY id),'[]') FROM responses r),
            (SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY id),'[]') FROM actions a)
        )::text,'UTF8')),'hex') AS revision,
        (SELECT count(*) FROM targets) AS targets,(SELECT count(*) FROM observations) AS observations,
        (SELECT count(*) FROM periods) AS periods,(SELECT count(*) FROM evaluators) AS evaluators,
        (SELECT count(*) FROM responses) AS responses,(SELECT count(*) FROM actions) AS actions
        FROM cycle
    "#).bind(id).fetch_optional().await?.ok_or_else(||AppError::NotFound("ไม่พบรอบนิเทศ".into()))?;
    Ok(SupervisionCycleDeletionImpact {cycle_id:id,revision:row.revision,counts:SupervisionCycleDeletionCounts {
        targets:row.targets,observations:row.observations,periods:row.periods,evaluators:row.evaluators,
        responses:row.responses,actions:row.actions,
    }})
}

pub async fn cycle_deletion_impact(pool:&PgPool,id:Uuid)->Result<SupervisionCycleDeletionImpact,AppError> {
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"supervision.cycle_deletion.preview").await?;
    // One repeatable snapshot pairs graph validation, complete counts and digest.
    DbSession::new(&mut tx).query("supervision.cycle_deletion.preview_snapshot",
        "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute().await?;
    contexts(&mut tx,id).await?;
    validate_graph(&mut tx,id).await?;
    let result=impact(&mut tx,id).await?;
    tx.commit().await?;Ok(result)
}

pub async fn delete_cycle(pool:&PgPool,id:Uuid,input:DeleteSupervisionCycleRequest)->Result<SupervisionCycleDeletionImpact,AppError> {
    if input.expected_revision.len()!=64 {return Err(stale());}
    let mut tx=school_tenancy::TenantPool::begin_observed(pool,"supervision.cycle_deletion.delete").await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let before=contexts(&mut tx,id).await?;
    require_context_writes(&mut tx,&before).await?;
    let locked:Option<Uuid>=DbSession::new(&mut tx).scalar("supervision.cycle_deletion.lock_cycle",
        "SELECT id FROM supervision_cycles WHERE id=$1 FOR UPDATE").bind(id).fetch_optional().await?;
    if locked.is_none(){return Err(AppError::NotFound("ไม่พบรอบนิเทศ".into()));}
    if contexts(&mut tx,id).await?!=before {return Err(stale());}
    DbSession::new(&mut tx).query("supervision.cycle_deletion.lock_observations",
        "SELECT id FROM supervision_observations WHERE cycle_id=$1 ORDER BY id FOR UPDATE")
        .bind(id).execute().await?;
    // Response insertion also references evaluator_id independently. Fence it
    // before validating that no malformed edge can cascade outside this cycle.
    DbSession::new(&mut tx).query("supervision.cycle_deletion.lock_evaluators",
        "SELECT e.id FROM supervision_evaluators e JOIN supervision_observations o ON o.id=e.observation_id
         WHERE o.cycle_id=$1 ORDER BY e.id FOR UPDATE OF e").bind(id).execute().await?;
    validate_graph(&mut tx,id).await?;
    let result=impact(&mut tx,id).await?;
    if result.revision!=input.expected_revision {return Err(stale());}
    DbSession::new(&mut tx).query("supervision.cycle_deletion.delete_graph",
        "DELETE FROM supervision_cycles WHERE id=$1").bind(id).execute().await?;
    tx.commit().await?;Ok(result)
}
