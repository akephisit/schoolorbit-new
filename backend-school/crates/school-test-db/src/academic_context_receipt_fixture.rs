//! Sanitized accepted receipts from schema 146, shared by migration and runtime replay tests.
//! These requests predate explicit offering-date inputs. Output dates may evolve through publication.
use chrono::NaiveDate;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub const YEAR: Uuid = Uuid::from_u128(0x10000000000000000000000000000025);
pub const NEXT_YEAR: Uuid = Uuid::from_u128(0x10000000000000000000000000000026);
pub const TERM: Uuid = Uuid::from_u128(0x11000000000000000000000000000251);
pub const ACTOR: Uuid = Uuid::from_u128(0x50000000000000000000000000000001);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDeferredChoice {
    proposal_id: String,
    action: &'static str,
    groups: [(); 0],
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyCurriculumIdentity {
    academic_term_id: Uuid,
    study_program_ids: Vec<Uuid>,
    delivery_version_id: Option<Uuid>,
    source_hash: String,
    choices: Vec<LegacyDeferredChoice>,
}

pub struct LegacyCurriculumReceipt {
    pub request_id: Uuid,
    pub target_term_id: Uuid,
    pub request_body: serde_json::Value,
    pub request_hash: String,
}

pub async fn seed_legacy_curriculum_receipt(
    pool: &PgPool,
    created_offerings: usize,
) -> LegacyCurriculumReceipt {
    let target: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 ORDER BY sequence_no LIMIT 1",
    )
    .bind(NEXT_YEAR)
    .fetch_one(pool)
    .await
    .unwrap();
    let program: Uuid = sqlx::query_scalar(
        "SELECT study_program_id FROM homerooms WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(NEXT_YEAR)
    .fetch_one(pool)
    .await
    .unwrap();
    let request_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    let offering_ids = seed_legacy_offerings(&mut tx, target, created_offerings).await;
    let versions: Vec<Uuid> = sqlx::query_scalar(
        "SELECT subject_version_id FROM course_offering_details WHERE learning_offering_id=ANY($1)",
    )
    .bind(&offering_ids)
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    let mut choices = versions
        .into_iter()
        .map(|version| LegacyDeferredChoice {
            proposal_id: hex::encode(Sha256::digest(
                serde_json::to_vec(&(target, "course", version)).unwrap(),
            )),
            action: "defer_groups",
            groups: [],
        })
        .collect::<Vec<_>>();
    choices.sort_by(|a, b| a.proposal_id.cmp(&b.proposal_id));
    let identity = LegacyCurriculumIdentity {
        academic_term_id: target,
        study_program_ids: vec![program],
        delivery_version_id: None,
        source_hash: "c".repeat(64),
        choices,
    };
    let request_hash = hex::encode(Sha256::digest(serde_json::to_vec(&identity).unwrap()));
    sqlx::query("INSERT INTO learning_delivery_apply_runs(idempotency_key,academic_term_id,request_hash,source_hash,offering_ids,created_offering_count,retained_offering_count,actor_user_id) VALUES($1,$2,$3,$4,$5,$6,1,$7)")
        .bind(request_id).bind(target).bind(&request_hash).bind(&identity.source_hash)
        .bind(&offering_ids).bind(created_offerings as i32).bind(ACTOR).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let mut request_body = serde_json::to_value(identity).unwrap();
    request_body
        .as_object_mut()
        .unwrap()
        .insert("idempotencyKey".into(), serde_json::json!(request_id));
    LegacyCurriculumReceipt {
        request_id,
        target_term_id: target,
        request_body,
        request_hash,
    }
}

/// Model the permitted draft-to-published availability edits after receipt acceptance.
/// Different subsets can publish on separate dates; creation identity is unchanged.
pub async fn retime_legacy_created_offerings(
    pool: &PgPool,
    request_id: Uuid,
    preparation: bool,
) -> Vec<(Uuid, NaiveDate)> {
    let ids: Vec<Uuid> = if preparation {
        sqlx::query_scalar("SELECT offering.id FROM learning_offerings offering JOIN academic_term_preparation_runs ledger ON ledger.target_academic_term_id=offering.academic_term_id AND ledger.created_at=offering.created_at WHERE ledger.request_id=$1 ORDER BY offering.id")
            .bind(request_id).fetch_all(pool).await.unwrap()
    } else {
        sqlx::query_scalar("SELECT offering.id FROM learning_offerings offering JOIN learning_delivery_apply_runs ledger ON ledger.academic_term_id=offering.academic_term_id AND ledger.created_at=offering.created_at WHERE ledger.idempotency_key=$1 ORDER BY offering.id")
            .bind(request_id).fetch_all(pool).await.unwrap()
    };
    let mut facts = Vec::new();
    for (position, id) in ids.into_iter().enumerate() {
        let date = NaiveDate::from_ymd_opt(2026, 6 + position as u32, 1).unwrap();
        sqlx::query("UPDATE learning_offerings SET starts_on=$2,status='published',published_at=$2::date::timestamp AT TIME ZONE 'Asia/Bangkok' WHERE id=$1")
            .bind(id).bind(date).execute(pool).await.unwrap();
        facts.push((id, date));
    }
    facts
}

pub async fn legacy_offering_receipt_facts(pool: &PgPool, target: Uuid) -> serde_json::Value {
    sqlx::query_scalar(r#"SELECT jsonb_build_object(
        'offerings',(SELECT jsonb_agg(to_jsonb(offering) ORDER BY offering.id) FROM learning_offerings offering WHERE academic_term_id=$1),
        'revisions',(SELECT jsonb_agg(to_jsonb(revision) ORDER BY revision.id) FROM academic_term_change_sets revision WHERE academic_term_id=$1),
        'versions',(SELECT jsonb_agg(to_jsonb(version) ORDER BY version.id) FROM academic_delivery_versions version WHERE academic_term_id=$1),
        'audits',(SELECT jsonb_agg(to_jsonb(audit) ORDER BY audit.id) FROM academic_audit_events audit WHERE academic_term_id=$1),
        'preparation',(SELECT jsonb_agg(to_jsonb(receipt)-'offering_starts_on' ORDER BY receipt.id) FROM academic_term_preparation_runs receipt WHERE target_academic_term_id=$1),
        'curriculum',(SELECT jsonb_agg(to_jsonb(receipt)-'starts_on' ORDER BY receipt.idempotency_key) FROM learning_delivery_apply_runs receipt WHERE academic_term_id=$1))"#)
        .bind(target).fetch_one(pool).await.unwrap()
}

async fn seed_legacy_offerings(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: Uuid,
    created_offerings: usize,
) -> Vec<Uuid> {
    assert!(created_offerings <= 2);
    if created_offerings == 0 {
        return Vec::new();
    }
    let retained = Uuid::new_v4();
    let source:Uuid=sqlx::query_scalar("SELECT id FROM learning_offerings WHERE academic_term_id=$1 AND kind='course' ORDER BY id LIMIT 1").bind(TERM).fetch_one(&mut **tx).await.unwrap();
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'academic_term_id',$2::uuid,'academic_year_id',$3::uuid,'starts_on','2026-05-01','ends_on',NULL,'status','draft','published_at',NULL,'created_at','2001-01-01T00:00:00Z','updated_at','2001-01-01T00:00:00Z'))).* FROM learning_offerings source WHERE id=$4")
            .bind(retained).bind(target).bind(NEXT_YEAR).bind(source).execute(&mut **tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,to_jsonb(source)||jsonb_build_object('learning_offering_id',$1::uuid,'academic_term_id',$2::uuid,'academic_year_id',$3::uuid))).* FROM course_offering_details source WHERE learning_offering_id=$4")
            .bind(retained).bind(target).bind(NEXT_YEAR).bind(source).execute(&mut **tx).await.unwrap();
    let mut ids = vec![retained];
    for _ in 0..created_offerings {
        let subject = Uuid::new_v4();
        let subject_version = Uuid::new_v4();
        let offering = Uuid::new_v4();
        sqlx::query("INSERT INTO subjects SELECT (jsonb_populate_record(NULL::subjects,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'code','MIXED-'||left($1::text,12),'identity_key','mixed-'||$1::text))).* FROM subjects source JOIN course_offering_details detail ON detail.subject_id=source.id WHERE detail.learning_offering_id=$2")
            .bind(subject).bind(retained).execute(&mut **tx).await.unwrap();
        sqlx::query("INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'subject_id',$2::uuid,'code','MIXED-'||left($2::text,12)))).* FROM subject_versions source JOIN course_offering_details detail ON detail.subject_version_id=source.id WHERE detail.learning_offering_id=$3")
            .bind(subject_version).bind(subject).bind(retained).execute(&mut **tx).await.unwrap();
        sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(source)||jsonb_build_object('id',$1::uuid,'code_snapshot','MIXED-'||left($1::text,12),'starts_on','2026-05-03','status','draft','published_at',NULL,'created_at',now(),'updated_at',now()))).* FROM learning_offerings source WHERE id=$2")
            .bind(offering).bind(retained).execute(&mut **tx).await.unwrap();
        sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,to_jsonb(source)||jsonb_build_object('learning_offering_id',$1::uuid,'subject_id',$2::uuid,'subject_version_id',$3::uuid,'curriculum_course_requirement_id',NULL))).* FROM course_offering_details source WHERE learning_offering_id=$4")
            .bind(offering).bind(subject).bind(subject_version).bind(retained).execute(&mut **tx).await.unwrap();
        ids.push(offering);
    }
    ids.sort_unstable();
    ids
}

#[derive(serde::Serialize)]
struct OldPreparationIdentity {
    source_term_id: Uuid,
    target_term_id: Uuid,
    modules: Vec<String>,
    mappings: OldPreparationMappings,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OldPreparationMappings {
    entities: Vec<OldEntityMapping>,
    dates: Vec<OldDateMapping>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OldEntityMapping {
    kind: String,
    source_id: Uuid,
    target_id: Uuid,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OldDateMapping {
    source_date: NaiveDate,
    target_date: NaiveDate,
}
pub async fn seed_legacy_preparation_receipt(
    pool: &PgPool,
    created_offerings: usize,
) -> (Uuid, Uuid, Uuid, String) {
    let target: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 ORDER BY sequence_no LIMIT 1",
    )
    .bind(NEXT_YEAR)
    .fetch_one(pool)
    .await
    .unwrap();
    let nonce = Uuid::new_v4();
    let identity = OldPreparationIdentity {
        source_term_id: TERM,
        target_term_id: target,
        modules: vec!["delivery".into()],
        mappings: OldPreparationMappings {
            entities: vec![OldEntityMapping {
                kind: "teacher".into(),
                source_id: ACTOR,
                target_id: ACTOR,
            }],
            dates: vec![OldDateMapping {
                source_date: NaiveDate::from_ymd_opt(2025, 5, 2).unwrap(),
                target_date: NaiveDate::from_ymd_opt(2026, 5, 3).unwrap(),
            }],
        },
    };
    let source_hash = "c".repeat(64);
    let checksum = hex::encode(Sha256::digest(
        serde_json::to_vec(&(ACTOR, nonce, &identity, &source_hash)).unwrap(),
    ));
    let version = Uuid::new_v4();
    let revision = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    let created_count = created_offerings + 1;
    let mut output_ids = seed_legacy_offerings(&mut tx, target, created_offerings).await;
    output_ids.push(revision);
    sqlx::query("INSERT INTO academic_delivery_versions(id,academic_term_id,academic_year_id,reference_date,snapshot,created_by) VALUES($1,$2,$3,'2026-05-10','{\"offerings\":[]}',$4)")
        .bind(version).bind(target).bind(NEXT_YEAR).bind(ACTOR).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_term_change_sets(id,academic_term_id,academic_year_id,reference_date,reason,idempotency_key,creation_request_hash,created_by,target_delivery_version_id) VALUES($1,$2,$3,'2026-05-10','Historical preparation',$4,repeat('d',64),$5,$6)")
        .bind(revision).bind(target).bind(NEXT_YEAR).bind(nonce.to_string()).bind(ACTOR).bind(version).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,academic_term_id,actor_user_id,payload) VALUES('academic_delivery_version.created','academic_delivery_version',$1,$2,$3,$4,$5)")
        .bind(version).bind(NEXT_YEAR).bind(target).bind(ACTOR).bind(sqlx::types::Json(serde_json::json!({"changeSetId":revision,"sourceVersionId":null,"referenceDate":"2026-05-10","requestHash":"d".repeat(64)}))).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO academic_term_preparation_runs(request_id,source_academic_term_id,source_academic_year_id,target_academic_term_id,target_academic_year_id,selected_modules,mappings,source_checksum,request_checksum,outcome,actor_user_id) VALUES($1,$2,$3,$4,$5,ARRAY['delivery'],$6,$7,$8,$9,$10)")
        .bind(nonce).bind(TERM).bind(YEAR).bind(target).bind(NEXT_YEAR).bind(sqlx::types::Json(&identity.mappings)).bind(&source_hash).bind(&checksum)
        .bind(sqlx::types::Json(serde_json::json!([{"module":"delivery","createdCount":created_count,"targetIds":output_ids}]))).bind(ACTOR).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    (nonce, version, revision, checksum)
}
