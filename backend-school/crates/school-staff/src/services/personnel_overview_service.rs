use super::staff_directory_query::{push_staff_access_filter, CURRENT_SUBJECT_GROUPS};
use crate::{models::StaffListAccess, personnel::*};
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, QueryBuilder};

pub async fn get_personnel_overview(
    pool: &PgPool,
    query: PersonnelOverviewQuery,
    access: StaffListAccess,
) -> Result<PersonnelOverview, AppError> {
    let status = query.status.unwrap_or(PersonnelStatusFilter::Active);
    let mut sql=QueryBuilder::<Postgres>::new("WITH matched AS (SELECT u.id,u.status,info.job_position_id,info.academic_rank,info.education_level FROM users u LEFT JOIN staff_info info ON info.user_id=u.id WHERE u.user_type='staff'");
    push_staff_access_filter(&mut sql, access);
    sql.push("), filtered AS (SELECT * FROM matched");
    if status != PersonnelStatusFilter::All {
        sql.push(" WHERE status=").push_bind(status.as_str());
    }
    sql.push("), groups AS (").push(CURRENT_SUBJECT_GROUPS).push(r#"), buckets AS (
      SELECT 'status' AS dimension,status AS key,status AS label,count(*) AS count FROM matched GROUP BY status
      UNION ALL SELECT 'job_position',coalesce(position.id::text,'unspecified'),coalesce(position.name,'ยังไม่ระบุตำแหน่ง'),count(*) FROM filtered f LEFT JOIN staff_job_positions position ON position.id=f.job_position_id GROUP BY position.id,position.name
      UNION ALL SELECT 'academic_rank',coalesce(academic_rank,'unspecified'),coalesce(academic_rank,'ยังไม่ระบุวิทยฐานะ'),count(*) FROM filtered GROUP BY academic_rank
      UNION ALL SELECT 'education_level',coalesce(education_level,'unspecified'),coalesce(education_level,'ยังไม่ระบุวุฒิ'),count(*) FROM filtered GROUP BY education_level
      UNION ALL SELECT 'subject_group',coalesce(g.id::text,'unassigned'),coalesce(g.name,'ยังไม่มีสังกัดกลุ่มสาระ'),count(*) FROM filtered f LEFT JOIN groups g ON g.user_id=f.id GROUP BY g.id,g.name
    ), arrays AS (SELECT dimension,jsonb_agg(jsonb_build_object('key',key,'label',label,'count',count) ORDER BY count DESC,label,key) AS items FROM buckets GROUP BY dimension)
    SELECT jsonb_build_object('asOf',CURRENT_TIMESTAMP,'total',(SELECT count(*) FROM matched),'active',(SELECT count(*) FROM matched WHERE status='active'),'otherStatuses',(SELECT count(*) FROM matched WHERE status<>'active'),'filteredTotal',(SELECT count(*) FROM filtered),
    'statuses',coalesce((SELECT items FROM arrays WHERE dimension='status'),'[]'::jsonb),
    'subjectGroups',coalesce((SELECT items FROM arrays WHERE dimension='subject_group'),'[]'::jsonb),
    'jobPositions',coalesce((SELECT items FROM arrays WHERE dimension='job_position'),'[]'::jsonb),
    'academicRanks',coalesce((SELECT items FROM arrays WHERE dimension='academic_rank'),'[]'::jsonb),
    'educationLevels',coalesce((SELECT items FROM arrays WHERE dimension='education_level'),'[]'::jsonb))"#);
    let Json(mut overview): Json<PersonnelOverview> =
        sql.build_query_scalar().fetch_one(pool).await?;
    label_buckets(
        &mut overview.statuses,
        PersonnelStatusFilter::ALL
            .iter()
            .map(|v| (v.as_str(), v.label())),
    );
    label_buckets(
        &mut overview.academic_ranks,
        StaffAcademicRank::ALL
            .iter()
            .map(|v| (v.as_str(), v.label())),
    );
    label_buckets(
        &mut overview.education_levels,
        StaffEducationLevel::ALL
            .iter()
            .map(|v| (v.as_str(), v.label())),
    );
    Ok(overview)
}
fn label_buckets<'a>(
    buckets: &mut [PersonnelBucket],
    labels: impl Iterator<Item = (&'a str, &'a str)>,
) {
    for (key, label) in labels {
        if let Some(bucket) = buckets.iter_mut().find(|b| b.key == key) {
            bucket.label = label.to_string();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overview_labels_distinguish_unknown_from_explicit_no_rank() {
        let mut buckets = vec![
            PersonnelBucket {
                key: "none".into(),
                label: "none".into(),
                count: 1,
            },
            PersonnelBucket {
                key: "unspecified".into(),
                label: "ยังไม่ระบุวิทยฐานะ".into(),
                count: 1,
            },
        ];
        label_buckets(
            &mut buckets,
            StaffAcademicRank::ALL
                .iter()
                .map(|v| (v.as_str(), v.label())),
        );
        assert_eq!(buckets[0].label, "ไม่มีวิทยฐานะ");
        assert_eq!(buckets[1].label, "ยังไม่ระบุวิทยฐานะ");
    }
}
