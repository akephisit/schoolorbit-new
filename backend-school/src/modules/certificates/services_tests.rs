use std::collections::BTreeSet;

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::certificates::{
        models::{
            CertificateCampaignListQuery, CertificateCampaignStatus,
            ChangeCertificateCampaignStatusRequest, CreateCertificateCampaignRequest,
            NullableUuidUpdate, UpdateCertificateCampaignRequest,
        },
        services::campaign_service,
    },
    permissions::registry::codes,
    policies::{
        certificate_access_policy::{require_owner_action, CertificateAction},
        resource_access_policy::accessible_exact_units_for_permission,
    },
    test_helpers::{create_named_test_pool, create_test_user, run_test_migrations},
};

use chrono::NaiveDate;

struct CertificatePolicyFixture {
    pool: PgPool,
    actor: ActorContext,
    unit_a: Uuid,
    unit_b: Uuid,
}

impl CertificatePolicyFixture {
    async fn new(test_name: &str) -> Self {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        let actor_user_id = create_test_user(
            &pool,
            &format!("{test_name}-actor@example.invalid"),
            "test-password",
        )
        .await
        .expect("policy actor should insert");
        let unit_a = insert_unit(&pool, &format!("{test_name}_a"), None).await;
        let unit_b = insert_unit(&pool, &format!("{test_name}_b"), None).await;
        for unit_id in [unit_a, unit_b] {
            sqlx::query(
                "INSERT INTO organization_members
                    (user_id, organization_unit_id, position_code, started_at)
                 VALUES ($1, $2, 'head', CURRENT_DATE)",
            )
            .bind(actor_user_id)
            .bind(unit_id)
            .execute(&pool)
            .await
            .expect("active organization membership should insert");
        }
        Self {
            pool,
            actor: ActorContext {
                user_id: actor_user_id,
                permissions: vec![codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT.to_string()],
            },
            unit_a,
            unit_b,
        }
    }

    async fn with_position_grant(test_name: &str, position: &str) -> Self {
        let fixture = Self::new(test_name).await;
        fixture.add_position_grant(fixture.unit_a, position).await;
        fixture
    }

    async fn permission_id(&self) -> Uuid {
        sqlx::query_scalar("SELECT id FROM permissions WHERE code = $1")
            .bind(codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT)
            .fetch_one(&self.pool)
            .await
            .expect("generated certificate permission should be migrated")
    }

    async fn add_position_grant(&self, unit_id: Uuid, position: &str) {
        sqlx::query(
            "INSERT INTO organization_permission_grants
                (organization_unit_id, permission_id, position_code)
             VALUES ($1, $2, $3)",
        )
        .bind(unit_id)
        .bind(self.permission_id().await)
        .bind(position)
        .execute(&self.pool)
        .await
        .expect("exact-unit permission grant should insert");
    }

    async fn add_scoped_role(
        &self,
        scope: Option<Uuid>,
        started_offset_days: i32,
        ended_offset_days: Option<i32>,
    ) {
        let role_id: Uuid = sqlx::query_scalar(
            "INSERT INTO roles (code, name, user_type, is_active)
             VALUES ($1, $1, 'staff', true)
             RETURNING id",
        )
        .bind(format!("certificate_role_{}", Uuid::new_v4().simple()))
        .fetch_one(&self.pool)
        .await
        .expect("role fixture should insert");
        sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
            .bind(role_id)
            .bind(self.permission_id().await)
            .execute(&self.pool)
            .await
            .expect("role permission fixture should insert");
        sqlx::query(
            "INSERT INTO user_roles
                (user_id, role_id, organization_unit_id, started_at, ended_at)
             VALUES ($1, $2, $3, $4::date, $5::date)",
        )
        .bind(self.actor.user_id)
        .bind(role_id)
        .bind(scope)
        .bind(
            chrono::Utc::now().date_naive()
                + chrono::Duration::days(i64::from(started_offset_days)),
        )
        .bind(ended_offset_days.map(|offset| {
            chrono::Utc::now().date_naive() + chrono::Duration::days(i64::from(offset))
        }))
        .execute(&self.pool)
        .await
        .expect("scoped user role fixture should insert");
    }
}

async fn insert_unit(pool: &PgPool, code: &str, parent_id: Option<Uuid>) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO organization_units
            (code, name, category, unit_type, parent_unit_id, is_active)
         VALUES ($1, $1, 'academic', 'unit', $2, true)
         RETURNING id",
    )
    .bind(code)
    .bind(parent_id)
    .fetch_one(pool)
    .await
    .expect("organization unit fixture should insert")
}

#[tokio::test]
async fn grant_in_unit_a_does_not_authorize_campaign_in_unit_b() {
    let fixture =
        CertificatePolicyFixture::with_position_grant("certificate_exact_unit", "head").await;

    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn exact_unit_grants_require_matching_position_active_membership_and_active_unit() {
    let fixture =
        CertificatePolicyFixture::with_position_grant("certificate_grant_bounds", "coordinator")
            .await;
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE organization_members
         SET position_code = 'coordinator'
         WHERE user_id = $1 AND organization_unit_id = $2",
    )
    .bind(fixture.actor.user_id)
    .bind(fixture.unit_a)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_ok());

    sqlx::query(
        "UPDATE organization_members SET ended_at = CURRENT_DATE
         WHERE user_id = $1 AND organization_unit_id = $2",
    )
    .bind(fixture.actor.user_id)
    .bind(fixture.unit_a)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE organization_members
         SET started_at = CURRENT_DATE + 1, ended_at = NULL
         WHERE user_id = $1 AND organization_unit_id = $2",
    )
    .bind(fixture.actor.user_id)
    .bind(fixture.unit_a)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(fixture.unit_a)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn scoped_roles_and_parent_membership_do_not_leak_to_other_or_child_units() {
    let fixture = CertificatePolicyFixture::new("certificate_role_scope").await;
    fixture.add_scoped_role(Some(fixture.unit_a), 0, None).await;
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    )
    .await
    .is_err());

    let child = insert_unit(
        &fixture.pool,
        "certificate_role_scope_child",
        Some(fixture.unit_a),
    )
    .await;
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(child),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE user_roles
         SET organization_unit_id = NULL, ended_at = NULL
         WHERE user_id = $1",
    )
    .bind(fixture.actor.user_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    for member_unit in [fixture.unit_a, fixture.unit_b] {
        assert!(require_owner_action(
            &fixture.pool,
            &fixture.actor,
            Some(member_unit),
            CertificateAction::Update,
        )
        .await
        .is_ok());
    }
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(child),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query("UPDATE user_roles SET started_at = CURRENT_DATE + 1 WHERE user_id = $1")
        .bind(fixture.actor.user_id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE user_roles SET started_at = CURRENT_DATE - 2, ended_at = CURRENT_DATE
         WHERE user_id = $1",
    )
    .bind(fixture.actor.user_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(fixture.unit_a),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn exact_delegation_authorizes_non_member_only_during_its_active_window() {
    let fixture = CertificatePolicyFixture::new("certificate_delegation").await;
    let delegated_unit = insert_unit(&fixture.pool, "certificate_delegation_target", None).await;
    let grantor = create_test_user(
        &fixture.pool,
        "certificate-delegation-grantor@example.invalid",
        "test-password",
    )
    .await
    .unwrap();
    let delegation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_permission_delegations
            (from_user_id, to_user_id, permission_id, organization_unit_id, started_at, expires_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW() + INTERVAL '1 day')
         RETURNING id",
    )
    .bind(grantor)
    .bind(fixture.actor.user_id)
    .bind(fixture.permission_id().await)
    .bind(delegated_unit)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();

    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_ok());
    sqlx::query(
        "UPDATE organization_permission_delegations
         SET started_at = NOW() + INTERVAL '1 day', expires_at = NOW() + INTERVAL '2 days'
         WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_err());
    sqlx::query(
        "UPDATE organization_permission_delegations
         SET started_at = NOW() - INTERVAL '2 days', expires_at = NOW() - INTERVAL '1 day'
         WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_err());

    sqlx::query(
        "UPDATE organization_permission_delegations
         SET started_at = NOW() - INTERVAL '1 hour', expires_at = NULL, revoked_at = NOW()
         WHERE id = $1",
    )
    .bind(delegation_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        Some(delegated_unit),
        CertificateAction::Update,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn school_override_null_owner_and_list_scope_union_are_explicit() {
    let fixture =
        CertificatePolicyFixture::with_position_grant("certificate_scope_union", "head").await;
    fixture.add_scoped_role(Some(fixture.unit_b), 0, None).await;
    let units = accessible_exact_units_for_permission(
        &fixture.pool,
        fixture.actor.user_id,
        codes::CERTIFICATE_UPDATE_ORGANIZATION_UNIT,
    )
    .await
    .unwrap();
    assert_eq!(
        units.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.unit_a, fixture.unit_b])
    );
    assert!(require_owner_action(
        &fixture.pool,
        &fixture.actor,
        None,
        CertificateAction::Update,
    )
    .await
    .is_err());

    let school_actor = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_UPDATE_SCHOOL.to_string()],
    };
    assert!(require_owner_action(
        &fixture.pool,
        &school_actor,
        None,
        CertificateAction::Update,
    )
    .await
    .is_ok());
    assert!(require_owner_action(
        &fixture.pool,
        &school_actor,
        Some(fixture.unit_b),
        CertificateAction::Update,
    )
    .await
    .is_ok());
}

async fn insert_academic_year(pool: &PgPool, year: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO academic_years
            (year, name, start_date, end_date, is_active)
         VALUES ($1, $2, $3, $4, false)
         RETURNING id",
    )
    .bind(year)
    .bind(format!("ปีการศึกษา {year}"))
    .bind(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap())
    .bind(NaiveDate::from_ymd_opt(2027, 4, 30).unwrap())
    .fetch_one(pool)
    .await
    .expect("academic year fixture should insert")
}

async fn add_exact_grant(pool: &PgPool, unit_id: Uuid, permission_code: &str) {
    sqlx::query(
        "INSERT INTO organization_permission_grants
            (organization_unit_id, permission_id, position_code)
         SELECT $1, id, 'head'
         FROM permissions
         WHERE code = $2
         ON CONFLICT DO NOTHING",
    )
    .bind(unit_id)
    .bind(permission_code)
    .execute(pool)
    .await
    .expect("certificate exact-unit grant should insert");
}

#[tokio::test]
async fn campaign_create_and_list_are_limited_to_the_exact_owner_unit() {
    let fixture = CertificatePolicyFixture::new("certificate_campaign_scope").await;
    let academic_year_id = insert_academic_year(&fixture.pool, 3101).await;
    for permission in [
        codes::CERTIFICATE_CREATE_ORGANIZATION_UNIT,
        codes::CERTIFICATE_READ_ORGANIZATION_UNIT,
    ] {
        add_exact_grant(&fixture.pool, fixture.unit_a, permission).await;
    }

    let created = campaign_service::create_campaign(
        &fixture.pool,
        &fixture.actor,
        CreateCertificateCampaignRequest {
            academic_year_id,
            owner_organization_unit_id: Some(fixture.unit_a),
            name: "  กิจกรรมวันภาษาไทย  ".to_string(),
            event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
        },
    )
    .await
    .expect("exact-unit campaign should be created");
    assert_eq!(created.name, "กิจกรรมวันภาษาไทย");
    assert_eq!(created.owner_organization_unit_id, Some(fixture.unit_a));

    let denied = campaign_service::create_campaign(
        &fixture.pool,
        &fixture.actor,
        CreateCertificateCampaignRequest {
            academic_year_id,
            owner_organization_unit_id: Some(fixture.unit_b),
            name: "กิจกรรมนอกขอบเขต".to_string(),
            event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
        },
    )
    .await;
    assert!(matches!(denied, Err(AppError::Forbidden(_))));

    sqlx::query(
        "INSERT INTO certificate_campaigns
            (academic_year_id, owner_organization_unit_id, name, event_date, created_by, updated_by)
         VALUES ($1, $2, 'กิจกรรมหน่วยงานอื่น', $3, $4, $4)",
    )
    .bind(academic_year_id)
    .bind(fixture.unit_b)
    .bind(NaiveDate::from_ymd_opt(2026, 8, 15).unwrap())
    .bind(fixture.actor.user_id)
    .execute(&fixture.pool)
    .await
    .unwrap();

    let listed = campaign_service::list_campaigns(
        &fixture.pool,
        &fixture.actor,
        CertificateCampaignListQuery::default(),
    )
    .await
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let owner_options = campaign_service::list_owner_options(&fixture.pool, &fixture.actor)
        .await
        .unwrap();
    assert_eq!(owner_options.len(), 1);
    assert_eq!(owner_options[0].id, fixture.unit_a);

    sqlx::query("UPDATE organization_units SET is_active = false WHERE id = $1")
        .bind(fixture.unit_a)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let school_update_actor = ActorContext {
        user_id: fixture.actor.user_id,
        permissions: vec![codes::CERTIFICATE_UPDATE_SCHOOL.to_string()],
    };
    let inactive_owner_update = campaign_service::update_campaign(
        &fixture.pool,
        &school_update_actor,
        created.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: created.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("แก้หน่วยงานที่ปิดใช้งาน".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: false,
        },
    )
    .await;
    assert!(matches!(
        inactive_owner_update,
        Err(AppError::ValidationError(_))
    ));
}

fn school_certificate_actor(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![
            codes::CERTIFICATE_READ_SCHOOL.to_string(),
            codes::CERTIFICATE_CREATE_SCHOOL.to_string(),
            codes::CERTIFICATE_UPDATE_SCHOOL.to_string(),
            codes::CERTIFICATE_DELETE_SCHOOL.to_string(),
            codes::CERTIFICATE_SUBMIT_SCHOOL.to_string(),
            codes::CERTIFICATE_DOWNLOAD_SCHOOL.to_string(),
        ],
    }
}

async fn school_campaign_fixture(test_name: &str, year: i32) -> (PgPool, ActorContext, Uuid) {
    let pool = create_named_test_pool(test_name).await;
    run_test_migrations(&pool).await;
    let user_id = create_test_user(
        &pool,
        &format!("{test_name}-school@example.invalid"),
        "test-password",
    )
    .await
    .unwrap();
    let academic_year_id = insert_academic_year(&pool, year).await;
    (pool, school_certificate_actor(user_id), academic_year_id)
}

fn campaign_create_payload(
    academic_year_id: Uuid,
    owner_organization_unit_id: Option<Uuid>,
    name: &str,
) -> CreateCertificateCampaignRequest {
    CreateCertificateCampaignRequest {
        academic_year_id,
        owner_organization_unit_id,
        name: name.to_string(),
        event_date: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
    }
}

#[tokio::test]
async fn issued_campaign_identity_is_immutable_and_live_text_requires_confirmation() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_campaign_issued_update", 3102).await;
    let second_academic_year_id = insert_academic_year(&pool, 3103).await;
    let campaign = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "การแข่งขันคำคม"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE certificate_campaigns
         SET activity_sequence = 42, status = 'active', updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(campaign.id)
    .execute(&pool)
    .await
    .unwrap();
    let issued = campaign_service::get_campaign(&pool, &actor, campaign.id)
        .await
        .unwrap();

    let immutable_year = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: Some(second_academic_year_id),
            owner_organization_unit_id: None,
            name: None,
            event_date: None,
            confirm_affects_issued_certificates: false,
        },
    )
    .await;
    assert!(matches!(immutable_year, Err(AppError::Conflict(_))));

    let unconfirmed_name = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("การแข่งขันคำคมฉบับแก้ไข".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: false,
        },
    )
    .await;
    assert!(matches!(unconfirmed_name, Err(AppError::Conflict(_))));

    let confirmed = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("การแข่งขันคำคมฉบับแก้ไข".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(confirmed.name, "การแข่งขันคำคมฉบับแก้ไข");

    let stale = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: issued.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: Some(NullableUuidUpdate { value: None }),
            name: Some("ข้อมูลเก่า".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: true,
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));

    sqlx::query(
        "INSERT INTO certificate_issue_requests (campaign_id, submitted_by)
         VALUES ($1, $2)",
    )
    .bind(campaign.id)
    .bind(actor.user_id)
    .execute(&pool)
    .await
    .unwrap();
    let locked = campaign_service::update_campaign(
        &pool,
        &actor,
        campaign.id,
        UpdateCertificateCampaignRequest {
            expected_updated_at: confirmed.updated_at,
            academic_year_id: None,
            owner_organization_unit_id: None,
            name: Some("แก้ระหว่างตรวจไม่ได้".to_string()),
            event_date: None,
            confirm_affects_issued_certificates: true,
        },
    )
    .await;
    assert!(matches!(locked, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn campaign_manual_transitions_delete_rules_and_audit_are_explicit() {
    let (pool, actor, academic_year_id) =
        school_campaign_fixture("certificate_campaign_lifecycle", 3104).await;
    let draft = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "ร่างสำหรับลบ"),
    )
    .await
    .unwrap();
    let activate_draft = campaign_service::change_campaign_status(
        &pool,
        &actor,
        draft.id,
        ChangeCertificateCampaignStatusRequest {
            expected_updated_at: draft.updated_at,
            status: CertificateCampaignStatus::Active,
        },
    )
    .await;
    assert!(matches!(activate_draft, Err(AppError::Conflict(_))));
    campaign_service::delete_campaign(&pool, &actor, draft.id)
        .await
        .unwrap();
    assert!(matches!(
        campaign_service::get_campaign(&pool, &actor, draft.id).await,
        Err(AppError::NotFound(_))
    ));

    let audit_metadata: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata FROM audit_logs
         WHERE entity_type = 'certificate_campaign' AND entity_id = $1
         ORDER BY created_at, id",
    )
    .bind(draft.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audit_metadata.len(), 2);
    let serialized_audit = serde_json::to_string(&audit_metadata).unwrap();
    assert!(!serialized_audit.contains("ร่างสำหรับลบ"));
    assert!(!serialized_audit.contains("firstName"));
    assert!(!serialized_audit.contains("lastName"));

    let issued = campaign_service::create_campaign(
        &pool,
        &actor,
        campaign_create_payload(academic_year_id, None, "กิจกรรมที่ออกแล้ว"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE certificate_campaigns
         SET activity_sequence = 43, status = 'active', updated_at = clock_timestamp()
         WHERE id = $1",
    )
    .bind(issued.id)
    .execute(&pool)
    .await
    .unwrap();
    let mut current = campaign_service::get_campaign(&pool, &actor, issued.id)
        .await
        .unwrap();
    for next in [
        CertificateCampaignStatus::Closed,
        CertificateCampaignStatus::Active,
        CertificateCampaignStatus::Archived,
        CertificateCampaignStatus::Active,
    ] {
        current = campaign_service::change_campaign_status(
            &pool,
            &actor,
            issued.id,
            ChangeCertificateCampaignStatusRequest {
                expected_updated_at: current.updated_at,
                status: next,
            },
        )
        .await
        .unwrap();
        assert_eq!(current.status, next);
    }
    assert!(matches!(
        campaign_service::delete_campaign(&pool, &actor, issued.id).await,
        Err(AppError::Conflict(_))
    ));
}
