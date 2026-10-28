//! Explicit revision audience intent; registry workflows remain unchanged without it.
use crate::models::versions::{DeliverySnapshot, DeliveryVersionOffering};
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use serde::Serialize;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AudienceConstraint {
    pub delivery_version_id: Uuid,
    pub offering: Option<DeliveryVersionOffering>,
}
impl AudienceConstraint {
    pub fn allows_group(&self, group: Uuid) -> bool {
        self.offering
            .as_ref()
            .is_some_and(|o| o.groups.iter().any(|g| g.id == group))
    }
    pub fn allows_homeroom(&self, group: Uuid, room: Option<Uuid>) -> bool {
        self.offering.as_ref().is_some_and(|o| {
            room.is_some_and(|room| {
                o.homeroom_ids.contains(&room)
                    && o.groups.iter().any(|g| {
                        g.id == group
                            && (g.homeroom_ids.is_empty() || g.homeroom_ids.contains(&room))
                    })
            })
        })
    }
}

#[derive(sqlx::FromRow)]
struct AudienceSourceRow {
    id: Uuid,
    snapshot: sqlx::types::Json<DeliverySnapshot>,
    scoped_offerings: Vec<Uuid>,
}
async fn source_constraints(
    connection: &mut PgConnection,
    versions: &[Uuid],
    offerings: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, AudienceConstraint>, AppError> {
    if versions.is_empty() || offerings.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let sources:Vec<AudienceSourceRow>=DbSession::new(connection).query_as("academic_delivery.services.audiences.source_constraints",
        "SELECT v.id,v.snapshot,ARRAY(SELECT a.learning_offering_id FROM academic_delivery_revision_audiences a JOIN academic_term_change_sets c ON c.id=a.change_set_id WHERE c.target_delivery_version_id=v.id AND a.learning_offering_id=ANY($2) ORDER BY a.learning_offering_id) scoped_offerings FROM academic_delivery_versions v WHERE v.id=ANY($1) AND v.status='published' ORDER BY v.id")
        .bind(versions).bind(offerings).fetch_all().await?;
    let mut result = std::collections::HashMap::new();
    for source in sources {
        for id in source.scoped_offerings {
            result.insert(
                id,
                AudienceConstraint {
                    delivery_version_id: source.id,
                    offering: source
                        .snapshot
                        .0
                        .offerings
                        .iter()
                        .find(|o| o.id == id)
                        .cloned(),
                },
            );
        }
    }
    Ok(result)
}
pub async fn pinned_constraints_on_connection(
    connection: &mut PgConnection,
    version: Uuid,
    offerings: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, AudienceConstraint>, AppError> {
    if offerings.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let count:i64=DbSession::new(&mut *connection).scalar("academic_delivery.services.audiences.pinned_constraints_on_connection", "SELECT count(*) FROM learning_offerings o JOIN academic_delivery_versions v ON v.id=$1 AND v.academic_term_id=o.academic_term_id AND v.status='published' WHERE o.id=ANY($2)").bind(version).bind(offerings).fetch_one().await?;
    let unique: std::collections::HashSet<_> = offerings.iter().collect();
    if count != unique.len() as i64 {
        return Err(AppError::Conflict("รุ่นเปิดสอนไม่ตรงกับรายการที่อ้างอิง".into()));
    }
    source_constraints(connection, &[version], offerings).await
}
pub async fn dated_constraints_on_connection(
    connection: &mut PgConnection,
    offerings: &[Uuid],
    date: NaiveDate,
) -> Result<std::collections::HashMap<Uuid, AudienceConstraint>, AppError> {
    if offerings.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows:Vec<(Uuid,Option<Uuid>)>=DbSession::new(&mut *connection).query_as("academic_delivery.services.audiences.dated_constraints_on_connection", "SELECT o.id,selected.id FROM learning_offerings o LEFT JOIN LATERAL(SELECT v.id FROM academic_delivery_versions v WHERE v.academic_term_id=o.academic_term_id AND v.status='published' AND v.effective_from<=$2 ORDER BY v.effective_from DESC,v.id DESC LIMIT 1) selected ON true WHERE o.id=ANY($1) ORDER BY o.id")
        .bind(offerings).bind(date).fetch_all().await?;
    let unique: std::collections::HashSet<_> = offerings.iter().collect();
    if rows.len() != unique.len() {
        return Err(AppError::Conflict("ไม่พบรายการเปิดสอนที่ใช้ตรวจขอบเขต".into()));
    }
    let versions: Vec<_> = rows
        .into_iter()
        .filter_map(|(_, version)| version)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    source_constraints(connection, &versions, offerings).await
}
pub async fn pinned_constraint_on_connection(
    connection: &mut PgConnection,
    version: Uuid,
    offering: Uuid,
) -> Result<Option<AudienceConstraint>, AppError> {
    Ok(
        pinned_constraints_on_connection(connection, version, &[offering])
            .await?
            .remove(&offering),
    )
}
pub async fn dated_constraint_on_connection(
    connection: &mut PgConnection,
    offering: Uuid,
    date: NaiveDate,
) -> Result<Option<AudienceConstraint>, AppError> {
    Ok(
        dated_constraints_on_connection(connection, &[offering], date)
            .await?
            .remove(&offering),
    )
}
#[derive(Default)]
pub struct GroupAudienceConstraints {
    offering_by_group: std::collections::HashMap<Uuid, Uuid>,
    constraints: std::collections::HashMap<Uuid, AudienceConstraint>,
}
impl GroupAudienceConstraints {
    /// Query only IDs requested from the batch constructor. Unknown requested
    /// groups fail there; None for a known group means unchanged legacy scope.
    pub fn constraint_for_group(&self, group: Uuid) -> Option<&AudienceConstraint> {
        self.offering_by_group
            .get(&group)
            .and_then(|id| self.constraints.get(id))
    }
}
async fn group_offerings(
    connection: &mut PgConnection,
    groups: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Uuid>, AppError> {
    if groups.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<(Uuid, Uuid)> = DbSession::new(connection)
        .query_as(
            "academic_delivery.services.audiences.group_offerings",
            "SELECT id,learning_offering_id FROM learning_groups WHERE id=ANY($1) ORDER BY id",
        )
        .bind(groups)
        .fetch_all()
        .await?;
    let unique: std::collections::HashSet<_> = groups.iter().collect();
    if rows.len() != unique.len() {
        return Err(AppError::Conflict("ไม่พบกลุ่มเรียนที่ใช้ตรวจขอบเขต".into()));
    }
    Ok(rows.into_iter().collect())
}
pub async fn dated_group_constraints_on_connection(
    connection: &mut PgConnection,
    groups: &[Uuid],
    date: NaiveDate,
) -> Result<GroupAudienceConstraints, AppError> {
    let offering_by_group = group_offerings(connection, groups).await?;
    let offerings: Vec<_> = offering_by_group.values().copied().collect();
    let constraints = dated_constraints_on_connection(connection, &offerings, date).await?;
    Ok(GroupAudienceConstraints {
        offering_by_group,
        constraints,
    })
}
pub async fn pinned_group_constraints_on_connection(
    connection: &mut PgConnection,
    version: Uuid,
    groups: &[Uuid],
) -> Result<GroupAudienceConstraints, AppError> {
    let offering_by_group = group_offerings(connection, groups).await?;
    let offerings: Vec<_> = offering_by_group.values().copied().collect();
    let constraints = pinned_constraints_on_connection(connection, version, &offerings).await?;
    Ok(GroupAudienceConstraints {
        offering_by_group,
        constraints,
    })
}

#[derive(Serialize, serde::Deserialize)]
pub(super) struct RevisionAudienceIntent {
    pub offering: Uuid,
    pub rooms: Vec<Uuid>,
}
#[derive(Serialize, serde::Deserialize)]
pub(super) struct RevisionExclusions {
    pub offerings: Vec<Uuid>,
    pub groups: Vec<Uuid>,
    pub audiences: Vec<RevisionAudienceIntent>,
}
#[derive(Serialize)]
pub(super) struct RevisionContext {
    pub revision_id: Uuid,
    pub version_id: Uuid,
    pub revision_row_version: i64,
    pub version_row_version: i64,
    pub exclusions: RevisionExclusions,
    pub snapshot: DeliverySnapshot,
}
pub(super) async fn revision_context(
    tx: &mut Transaction<'_, Postgres>,
    version: Uuid,
    term: Uuid,
) -> Result<RevisionContext, AppError> {
    let row:Option<(Uuid,i64,i64,sqlx::types::Json<RevisionExclusions>,sqlx::types::Json<DeliverySnapshot>)>=DbSession::new(&mut **tx).query_as("academic_delivery.services.audiences.revision_context",
        "SELECT c.id,c.row_version,v.row_version,jsonb_build_object('offerings',c.excluded_offering_ids,'groups',c.excluded_group_ids,'audiences',COALESCE((SELECT jsonb_agg(jsonb_build_object('offering',a.learning_offering_id,'rooms',COALESCE((SELECT jsonb_agg(r.homeroom_id ORDER BY r.homeroom_id) FROM academic_delivery_revision_audience_rooms r WHERE r.change_set_id=a.change_set_id AND r.learning_offering_id=a.learning_offering_id),'[]'::jsonb)) ORDER BY a.learning_offering_id) FROM academic_delivery_revision_audiences a WHERE a.change_set_id=c.id),'[]'::jsonb)),v.snapshot FROM academic_term_change_sets c JOIN academic_delivery_versions v ON v.id=c.target_delivery_version_id WHERE v.id=$1 AND v.academic_term_id=$2 AND v.status='draft' AND c.status='draft' FOR UPDATE OF c,v")
        .bind(version).bind(term).fetch_optional().await?;
    let (revision_id, revision_row_version, version_row_version, exclusions, snapshot) = row
        .ok_or_else(|| {
            AppError::Conflict("การเตรียมรายการต้องอ้างอิงรุ่นเปิดสอนแบบร่างในภาคเรียนที่เลือก".into())
        })?;
    Ok(RevisionContext {
        revision_id,
        version_id: version,
        revision_row_version,
        version_row_version,
        exclusions: exclusions.0,
        snapshot: snapshot.0,
    })
}
pub(super) async fn set_revision_audience(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    offering: Uuid,
    rooms: &[Uuid],
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.set_revision_audience",
        "INSERT INTO academic_delivery_revision_audiences(change_set_id,learning_offering_id,academic_term_id,academic_year_id) SELECT id,$2,academic_term_id,academic_year_id FROM academic_term_change_sets WHERE id=$1 ON CONFLICT DO NOTHING")
        .bind(revision).bind(offering).execute().await?;
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.set_revision_audience", "DELETE FROM academic_delivery_revision_audience_rooms WHERE change_set_id=$1 AND learning_offering_id=$2")
        .bind(revision).bind(offering).execute().await?;
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.set_revision_audience",
        "INSERT INTO academic_delivery_revision_audience_rooms(change_set_id,learning_offering_id,academic_term_id,academic_year_id,homeroom_id) SELECT a.change_set_id,a.learning_offering_id,a.academic_term_id,a.academic_year_id,room FROM academic_delivery_revision_audiences a CROSS JOIN unnest($3::uuid[]) room WHERE a.change_set_id=$1 AND a.learning_offering_id=$2")
        .bind(revision).bind(offering).bind(rooms).execute().await?;
    Ok(())
}
pub(super) async fn inherit(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    base: Option<Uuid>,
) -> Result<(), AppError> {
    let Some(base) = base else { return Ok(()) };
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.inherit", "INSERT INTO academic_delivery_revision_audiences SELECT $1,a.learning_offering_id,a.academic_term_id,a.academic_year_id FROM academic_delivery_revision_audiences a JOIN academic_term_change_sets c ON c.id=a.change_set_id WHERE c.target_delivery_version_id=$2")
        .bind(revision).bind(base).execute().await?;
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.inherit", "INSERT INTO academic_delivery_revision_audience_rooms SELECT $1,r.learning_offering_id,r.academic_term_id,r.academic_year_id,r.homeroom_id FROM academic_delivery_revision_audience_rooms r JOIN academic_term_change_sets c ON c.id=r.change_set_id WHERE c.target_delivery_version_id=$2")
        .bind(revision).bind(base).execute().await?;
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.inherit", "UPDATE academic_term_change_sets child SET excluded_group_ids=parent.excluded_group_ids FROM academic_term_change_sets parent WHERE child.id=$1 AND parent.target_delivery_version_id=$2")
        .bind(revision).bind(base).execute().await?;
    Ok(())
}
pub(super) async fn clear_omitted_group_commands(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    offering: Uuid,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("academic_delivery.services.audiences.clear_omitted_group_commands", "DELETE FROM academic_term_change_items item USING learning_groups g WHERE item.change_set_id=$1 AND item.learning_group_id=g.id AND g.learning_offering_id=$2 AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(academic_delivery_revision_snapshot($1)->'offerings') o CROSS JOIN jsonb_array_elements(o->'groups') entry WHERE (entry->>'id')::uuid=g.id)")
        .bind(revision).bind(offering).execute().await?;
    Ok(())
}
