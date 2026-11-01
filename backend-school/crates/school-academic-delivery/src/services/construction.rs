//! Identity-only provenance for selected resources without an immutable publication base.
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) struct InitialSource {
    pub offering_ids: Option<Vec<Uuid>>,
    pub group_ids: Vec<Uuid>,
    pub assignment_ids: Vec<Uuid>,
}

pub(super) async fn initial_source(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    year: Uuid,
    base: Option<Uuid>,
    date: NaiveDate,
    explicit: Option<&[Uuid]>,
) -> Result<InitialSource, AppError> {
    if base.is_some() {
        return Ok(InitialSource {
            offering_ids: None,
            group_ids: vec![],
            assignment_ids: vec![],
        });
    }
    let ids:Vec<Uuid>=match explicit {
        Some(ids)=>ids.iter().copied().collect::<std::collections::BTreeSet<_>>().into_iter().collect(),
        None=>DbSession::new(&mut **tx).scalar("academic_delivery.construction.initial_offerings",r#"
            SELECT id FROM learning_offerings o WHERE academic_term_id=$1 AND academic_year_id=$2
              AND status IN('draft','published') AND starts_on<=$3 AND (ends_on IS NULL OR ends_on>=$3)
              AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item WHERE item.learning_offering_id=o.id AND item.action_kind='add_offering')
            ORDER BY id"#).bind(term).bind(year).bind(date).fetch_all().await?,
    };
    let valid:i64=DbSession::new(&mut **tx).scalar("academic_delivery.construction.initial_context",
        "SELECT count(*) FROM learning_offerings WHERE id=ANY($1) AND academic_term_id=$2 AND academic_year_id=$3 AND status IN('draft','published')")
        .bind(&ids).bind(term).bind(year).fetch_one().await?;
    if valid != ids.len() as i64 {
        return Err(AppError::Conflict("รายการเริ่มต้นไม่ตรงกับภาคเรียนที่เตรียม".into()));
    }
    let (groups,assignments):(Vec<Uuid>,Vec<Uuid>)=DbSession::new(&mut **tx).query_as("academic_delivery.construction.initial_teachers",r#"
        WITH selected_groups AS MATERIALIZED(SELECT id FROM learning_groups WHERE learning_offering_id=ANY($1) AND status IN('published','closed'))
        SELECT ARRAY(SELECT id FROM selected_groups ORDER BY id),
            ARRAY(SELECT e.id FROM academic_delivery_effective_teacher_episodes e JOIN selected_groups g ON g.id=e.learning_group_id
                WHERE e.starts_on<=$2 AND (e.effective_ends_on IS NULL OR e.effective_ends_on>=$2) ORDER BY e.id)
    "#).bind(&ids).bind(date).fetch_one().await?;
    Ok(InitialSource {
        offering_ids: Some(ids),
        group_ids: groups,
        assignment_ids: assignments,
    })
}

/// Called only under the exclusive term guard. Published inclusion freezes the
/// date-effective selection; standalone publication freezes the last staged selection.
pub(super) async fn capture_teachers(
    tx: &mut Transaction<'_, Postgres>,
    offering: Uuid,
    revision: Option<Uuid>,
    staged: bool,
    date: Option<NaiveDate>,
) -> Result<(), AppError> {
    let revisions:Vec<Uuid>=DbSession::new(&mut **tx).scalar("academic_delivery.construction.lock_revisions",r#"
        SELECT c.id FROM academic_term_change_sets c JOIN academic_delivery_versions v ON v.id=c.target_delivery_version_id
        WHERE c.status='draft' AND v.status='draft' AND ($2::uuid IS NULL OR c.id=$2)
          AND ( $1=ANY(c.initial_offering_ids) OR EXISTS(SELECT 1 FROM academic_term_change_items item
              WHERE item.change_set_id=c.id AND item.action_kind='add_offering' AND item.learning_offering_id=$1))
          AND NOT($1=ANY(c.excluded_offering_ids)) AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item
              WHERE item.change_set_id=c.id AND item.action_kind='stop_offering' AND item.learning_offering_id=$1)
        ORDER BY c.id FOR UPDATE OF c,v
    "#).bind(offering).bind(revision).fetch_all().await?;
    if revisions.is_empty() {
        return Ok(());
    }
    DbSession::new(&mut **tx).query("academic_delivery.construction.capture_teachers",r#"
        WITH selected AS MATERIALIZED(
            SELECT c.id,g.id group_id,COALESCE($4,c.reference_date) selected_date
            FROM academic_term_change_sets c JOIN learning_groups g ON g.learning_offering_id=$1
            LEFT JOIN academic_delivery_versions b ON b.id=c.base_delivery_version_id
            WHERE c.id=ANY($2) AND (($3 AND g.status='draft') OR (NOT $3 AND g.status IN('published','closed')))
                AND NOT(g.id=ANY(c.initial_teacher_group_ids)) AND NOT(g.id=ANY(c.excluded_group_ids))
                AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(b.snapshot->'offerings') o,jsonb_array_elements(o->'groups') bg WHERE (bg->>'id')::uuid=g.id)
        ), captured AS(
            SELECT selected.id,ARRAY(SELECT group_id FROM selected s WHERE s.id=selected.id ORDER BY group_id) groups,
                ARRAY(SELECT e.id FROM academic_delivery_effective_teacher_episodes e JOIN selected s ON s.group_id=e.learning_group_id
                    WHERE s.id=selected.id AND ($3 OR (e.starts_on<=s.selected_date AND (e.effective_ends_on IS NULL OR e.effective_ends_on>=s.selected_date))) ORDER BY e.id) assignments
            FROM selected GROUP BY selected.id
        ) UPDATE academic_term_change_sets c SET
            initial_teacher_group_ids=ARRAY(SELECT DISTINCT id FROM unnest(c.initial_teacher_group_ids||captured.groups) id ORDER BY id),
            initial_teacher_assignment_ids=ARRAY(SELECT DISTINCT id FROM unnest(c.initial_teacher_assignment_ids||captured.assignments) id ORDER BY id)
        FROM captured WHERE c.id=captured.id
    "#).bind(offering).bind(&revisions).bind(staged).bind(date).execute().await?;
    Ok(())
}
