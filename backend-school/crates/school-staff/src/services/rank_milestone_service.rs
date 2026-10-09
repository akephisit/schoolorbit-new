use super::staff_directory_query::push_staff_access_filter;
use crate::{models::StaffListAccess, personnel::PersonnelStatusFilter, rank_milestones::*};
use chrono::{NaiveDate, Utc};
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, QueryBuilder};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct RankMilestoneOverviewQuery {
    pub status: Option<PersonnelStatusFilter>,
    pub bucket: Option<RankMilestoneStatus>,
    pub page: Option<i64>,
}
#[derive(Debug, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankMilestoneCounts {
    pub future: i64,
    pub due_soon: i64,
    pub time_reached_pending_review: i64,
    pub incomplete: i64,
    pub unsupported: i64,
    pub no_next_rank: i64,
}
impl RankMilestoneCounts {
    fn increment(&mut self, status: RankMilestoneStatus) {
        match status {
            RankMilestoneStatus::Future => self.future += 1,
            RankMilestoneStatus::DueSoon => self.due_soon += 1,
            RankMilestoneStatus::TimeReachedPendingReview => self.time_reached_pending_review += 1,
            RankMilestoneStatus::Incomplete => self.incomplete += 1,
            RankMilestoneStatus::Unsupported => self.unsupported += 1,
            RankMilestoneStatus::NoNextRank => self.no_next_rank += 1,
        }
    }
}
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankMilestonePerson {
    pub staff_id: Uuid,
    pub display_name: String,
    pub milestone: RankMilestone,
}
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RankMilestoneOverview {
    pub as_of: NaiveDate,
    pub counts: RankMilestoneCounts,
    pub filtered_total: i64,
    pub bucket: RankMilestoneStatus,
    pub items: Vec<RankMilestonePerson>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    display_name: String,
    personnel_type: Option<String>,
    personnel_type_date: Option<NaiveDate>,
    position_code: Option<String>,
    position_date: Option<NaiveDate>,
    academic_rank: Option<String>,
    rank_date: Option<NaiveDate>,
}
pub async fn get_rank_milestone_overview(
    pool: &PgPool,
    query: RankMilestoneOverviewQuery,
    access: StaffListAccess,
) -> Result<RankMilestoneOverview, AppError> {
    let page = query.page.unwrap_or(1);
    if !(1..=10_000).contains(&page) {
        return Err(AppError::BadRequest("หน้ารายการไม่ถูกต้อง".into()));
    }
    let status = query.status.unwrap_or(PersonnelStatusFilter::Active);
    let bucket = query.bucket.unwrap_or(RankMilestoneStatus::DueSoon);
    let as_of = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let mut sql=QueryBuilder::<Postgres>::new("SELECT u.id,concat_ws(' ',concat(btrim(u.title),btrim(u.first_name)),nullif(btrim(u.last_name),'')) AS display_name,t.personnel_type,t.effective_date AS personnel_type_date,p.code AS position_code,j.effective_date AS position_date,r.academic_rank,r.effective_date AS rank_date FROM users u LEFT JOIN staff_info info ON info.user_id=u.id LEFT JOIN staff_career_history t ON t.id=info.current_personnel_type_history_id LEFT JOIN staff_career_history j ON j.id=info.current_job_position_history_id LEFT JOIN staff_job_positions p ON p.id=j.job_position_id LEFT JOIN staff_career_history r ON r.id=info.current_academic_rank_history_id WHERE u.user_type='staff'");
    push_staff_access_filter(&mut sql, access);
    if status != PersonnelStatusFilter::All {
        sql.push(" AND u.status=").push_bind(status.as_str());
    }
    let rows: Vec<Row> = sql.build_query_as().fetch_all(pool).await?;
    let mut counts = RankMilestoneCounts::default();
    let filtered_total = rows.len() as i64;
    let mut items = Vec::new();
    for row in rows {
        let facts = RankMilestoneFacts {
            personnel_type: row
                .personnel_type
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| AppError::InternalServerError("ข้อมูลประเภทบุคลากรไม่ถูกต้อง".into()))?,
            personnel_type_date: row.personnel_type_date,
            position_code: row.position_code,
            position_date: row.position_date,
            academic_rank: row
                .academic_rank
                .map(|value| value.parse())
                .transpose()
                .map_err(|_| AppError::InternalServerError("ข้อมูลวิทยฐานะไม่ถูกต้อง".into()))?,
            rank_date: row.rank_date,
        };
        let milestone = calculate_rank_milestone_facts(&facts, as_of);
        counts.increment(milestone.status);
        if milestone.status == bucket {
            items.push(RankMilestonePerson {
                staff_id: row.id,
                display_name: row.display_name,
                milestone,
            });
        }
    }
    items.sort_by(|a, b| {
        a.milestone
            .ordinary_date
            .is_none()
            .cmp(&b.milestone.ordinary_date.is_none())
            .then_with(|| a.milestone.ordinary_date.cmp(&b.milestone.ordinary_date))
            .then_with(|| a.display_name.cmp(&b.display_name))
            .then_with(|| a.staff_id.cmp(&b.staff_id))
    });
    let total = items.len() as i64;
    let items = items
        .into_iter()
        .skip(((page - 1) * 50) as usize)
        .take(50)
        .collect();
    Ok(RankMilestoneOverview {
        as_of,
        counts,
        filtered_total,
        bucket,
        items,
        total,
        page,
        page_size: 50,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_keep_unsupported_and_missing_facts_separate_from_calendar_milestones() {
        let mut counts = RankMilestoneCounts::default();
        for status in [
            RankMilestoneStatus::Future,
            RankMilestoneStatus::DueSoon,
            RankMilestoneStatus::TimeReachedPendingReview,
            RankMilestoneStatus::Incomplete,
            RankMilestoneStatus::Unsupported,
            RankMilestoneStatus::NoNextRank,
        ] {
            counts.increment(status);
        }
        assert_eq!(
            counts.future
                + counts.due_soon
                + counts.time_reached_pending_review
                + counts.incomplete
                + counts.unsupported
                + counts.no_next_rank,
            6
        );
        assert_eq!(counts.incomplete, 1);
        assert_eq!(counts.unsupported, 1);
    }
}

#[cfg(test)]
mod database_tests {
    use super::*;
    use crate::career::{
        CreateStaffCareerRequest, StaffCareerEntryInput, StaffCareerFact, StaffPersonnelType,
    };
    use crate::personnel::StaffAcademicRank;
    use school_test_db::{create_named_test_pool, run_test_migrations};
    #[tokio::test]
    async fn milestones_use_canonical_dates_order_and_bound_pages_without_profile_queries() {
        let pool = create_named_test_pool("rank_milestone_pages").await;
        run_test_migrations(&pool).await;
        let position: Uuid =
            sqlx::query_scalar("SELECT id FROM staff_job_positions WHERE code='teacher'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let today = Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive();
        let start = today.checked_sub_months(chrono::Months::new(48)).unwrap();
        let mut ids = vec![];
        for i in 0..53 {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic-hash',$3,'Test','staff','active')").bind(id).bind(id.to_string()).bind(format!("Person {i:02}")).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO staff_info(user_id) VALUES($1)")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            let mut tx = pool.begin().await.unwrap();
            let date = start - chrono::Duration::days(i);
            let entries = [
                StaffCareerFact::PersonnelType {
                    value: Some(StaffPersonnelType::CivilServant),
                },
                StaffCareerFact::JobPosition {
                    value: Some(position),
                },
                StaffCareerFact::AcademicRank {
                    value: Some(StaffAcademicRank::Proficient),
                },
            ]
            .into_iter()
            .map(|fact| StaffCareerEntryInput {
                fact,
                effective_date: Some(date),
                order_date: None,
                order_number: None,
                note: None,
            })
            .collect();
            super::super::staff_career_service::create_current_career(
                &mut tx,
                id,
                id,
                &CreateStaffCareerRequest { entries },
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
            ids.push(id);
        }
        sqlx::query(
            "UPDATE users SET title=' นาย ',first_name=' ทดสอบ ',last_name=' บุคลากร ' WHERE id=$1",
        )
        .bind(ids[0])
        .execute(&pool)
        .await
        .unwrap();
        let query = |page| RankMilestoneOverviewQuery {
            status: None,
            bucket: Some(RankMilestoneStatus::TimeReachedPendingReview),
            page: Some(page),
        };
        let first = get_rank_milestone_overview(&pool, query(1), StaffListAccess::School)
            .await
            .unwrap();
        assert_eq!(first.filtered_total, 53);
        assert_eq!(first.total, 53);
        assert_eq!(first.counts.time_reached_pending_review, 53);
        assert_eq!(first.items.len(), 50);
        assert_eq!(first.items[0].staff_id, ids[52]);
        assert!(first
            .items
            .windows(2)
            .all(|pair| pair[0].milestone.ordinary_date <= pair[1].milestone.ordinary_date));
        let second = get_rank_milestone_overview(&pool, query(2), StaffListAccess::School)
            .await
            .unwrap();
        assert_eq!(second.items.len(), 3);
        assert_eq!(second.items.last().unwrap().staff_id, ids[0]);
        let own = get_rank_milestone_overview(&pool, query(1), StaffListAccess::Own(ids[0]))
            .await
            .unwrap();
        assert_eq!(own.total, 1);
        assert_eq!(own.items[0].display_name, "นายทดสอบ บุคลากร");
        assert_eq!(first.items[0].display_name, "Person 52 Test");
        assert_eq!(own.items[0].milestone.ordinary_date, Some(today));
        assert!(
            get_rank_milestone_overview(&pool, query(0), StaffListAccess::School)
                .await
                .is_err()
        );
    }
}
