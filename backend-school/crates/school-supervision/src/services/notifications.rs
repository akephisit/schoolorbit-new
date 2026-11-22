use super::shared::evaluator_conflict_status_codes;
use crate::models::{SupervisionObservation, SupervisionObservationStatus};
use school_database::DbSession;
use school_errors::AppError;
use school_notifications::{
    dispatch::{self, NewNotification, NotificationContent, NotificationSource, Presentation},
    publisher::NotificationType,
};
use sqlx::{PgConnection, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const SUPERVISION_PATH: &str = "/staff/academic/supervision";

#[derive(Clone, Copy)]
enum Notice {
    RequestApproved,
    Assigned,
    AssignmentRemoved,
    RequestReturned,
    ScheduleWithdrawn,
    Cancelled,
    ResultPublished,
}

impl Notice {
    fn content(self, observation: &SupervisionObservation) -> NotificationContent {
        let (title, message, type_) = match self {
            Self::RequestApproved => (
                "อนุมัติคำขอนิเทศแล้ว",
                "เปิดรายการนิเทศเพื่อดูรายละเอียดการนัดหมาย",
                NotificationType::Success,
            ),
            Self::Assigned => (
                "ได้รับมอบหมายให้ประเมินการนิเทศ",
                "เปิดรายการนิเทศเพื่อดูรายละเอียดงานประเมิน",
                NotificationType::Info,
            ),
            Self::AssignmentRemoved => (
                "ยกเลิกการมอบหมายงานประเมิน",
                "เปิดรายการงานประเมินเพื่อตรวจสอบงานที่ได้รับมอบหมาย",
                NotificationType::Info,
            ),
            Self::RequestReturned => (
                "ส่งคำขอนิเทศกลับให้ตรวจสอบ",
                "เปิดรายการนิเทศเพื่อดูรายละเอียดและดำเนินการต่อ",
                NotificationType::Warning,
            ),
            Self::ScheduleWithdrawn => (
                "ถอนการนัดหมายงานประเมิน",
                "รายการนิเทศถูกส่งกลับ กรุณาตรวจสอบรายการงานประเมิน",
                NotificationType::Info,
            ),
            Self::Cancelled => (
                "ยกเลิกรายการนิเทศแล้ว",
                "เปิดรายการนิเทศเพื่อตรวจสอบสถานะล่าสุด",
                NotificationType::Info,
            ),
            Self::ResultPublished => (
                "เผยแพร่ผลการนิเทศแล้ว",
                "เปิดรายการนิเทศเพื่อดูผลการนิเทศในระบบ",
                NotificationType::Success,
            ),
        };
        let destination = match self {
            Self::AssignmentRemoved | Self::ScheduleWithdrawn => "evaluate".to_owned(),
            _ => observation.id.to_string(),
        };
        NotificationContent {
            title: title.to_owned(),
            message: message.to_owned(),
            type_,
            link: Some(format!(
                "{SUPERVISION_PATH}/{destination}?academicYearId={}&academicTermId={}",
                observation.academic_year_id, observation.academic_term_id,
            )),
        }
    }
}

async fn accept(
    tx: &mut Transaction<'_, Postgres>,
    action_id: Uuid,
    observation: &SupervisionObservation,
    recipients: Vec<(Uuid, Notice)>,
) -> Result<(), AppError> {
    let mut notices = BTreeMap::new();
    for (recipient, notice) in recipients {
        notices.entry(recipient).or_insert(notice);
    }
    if notices.is_empty() {
        return Ok(());
    }
    let candidates: Vec<_> = notices.keys().copied().collect();
    let active = DbSession::new(&mut **tx)
        .scalar::<Uuid>(
            "supervision.notifications.active_recipients",
            "SELECT id FROM users WHERE id=ANY($1) AND status='active' ORDER BY id",
        )
        .bind(&candidates)
        .fetch_all()
        .await?;
    let source = NotificationSource::SupervisionAction {
        source_id: action_id,
    };
    let idempotency_key = format!("supervision_action:{action_id}");
    let items: Vec<_> = active
        .into_iter()
        .map(|recipient| {
            NewNotification::new(
                recipient,
                source,
                idempotency_key.clone(),
                notices[&recipient].content(observation),
                Presentation::AlertOnce,
            )
        })
        .collect();
    dispatch::create_in_transaction(tx, &items).await?;
    Ok(())
}

pub(super) async fn request_approved(
    tx: &mut Transaction<'_, Postgres>,
    action_id: Uuid,
    observation: &SupervisionObservation,
    evaluator_ids: &[Uuid],
) -> Result<(), AppError> {
    let recipients = std::iter::once((observation.observed_user_id, Notice::RequestApproved))
        .chain(evaluator_ids.iter().map(|&id| (id, Notice::Assigned)))
        .collect();
    accept(tx, action_id, observation, recipients).await
}

pub(super) async fn evaluators_replaced(
    tx: &mut Transaction<'_, Postgres>,
    action_id: Uuid,
    before: &SupervisionObservation,
    evaluator_ids: &[Uuid],
) -> Result<(), AppError> {
    if !evaluator_conflict_status_codes().contains(&before.status.as_str()) {
        return Ok(());
    }
    let previous: BTreeSet<_> = before
        .evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect();
    let current: BTreeSet<_> = evaluator_ids.iter().copied().collect();
    let recipients = current
        .difference(&previous)
        .map(|&id| (id, Notice::Assigned))
        .chain(
            previous
                .difference(&current)
                .map(|&id| (id, Notice::AssignmentRemoved)),
        )
        .collect();
    accept(tx, action_id, before, recipients).await
}

pub(super) async fn status_changed(
    tx: &mut Transaction<'_, Postgres>,
    action_id: Uuid,
    before: &SupervisionObservation,
    status: SupervisionObservationStatus,
) -> Result<(), AppError> {
    if before.status == status {
        return Ok(());
    }
    let (teacher, evaluator_notice) = match status {
        SupervisionObservationStatus::Returned => (
            (
                before.requested_by.unwrap_or(before.observed_user_id),
                Notice::RequestReturned,
            ),
            Some(Notice::ScheduleWithdrawn),
        ),
        SupervisionObservationStatus::Cancelled => (
            (before.observed_user_id, Notice::Cancelled),
            Some(Notice::Cancelled),
        ),
        SupervisionObservationStatus::Published => {
            ((before.observed_user_id, Notice::ResultPublished), None)
        }
        _ => return Ok(()),
    };
    let confirmed = evaluator_conflict_status_codes().contains(&before.status.as_str());
    let mut recipients = vec![teacher];
    if let Some(notice) = evaluator_notice.filter(|_| confirmed) {
        recipients.extend(
            before
                .evaluators
                .iter()
                .map(|evaluator| (evaluator.evaluator_user_id, notice)),
        );
    }
    accept(tx, action_id, before, recipients).await
}

/// Locks match permanent cycle deletion: observation, action, then central receipt.
pub async fn authorize_notification_on_connection(
    connection: &mut PgConnection,
    source_id: Uuid,
) -> Result<bool, AppError> {
    let observation_id = DbSession::new(&mut *connection)
        .scalar::<Uuid>(
            "supervision.notifications.source_observation",
            "SELECT observation_id FROM supervision_actions WHERE id=$1",
        )
        .bind(source_id)
        .fetch_optional()
        .await?;
    let Some(observation_id) = observation_id else {
        return Ok(false);
    };
    let status = DbSession::new(&mut *connection)
        .scalar::<String>(
            "supervision.notifications.lock_observation",
            "SELECT status FROM supervision_observations WHERE id=$1 FOR SHARE",
        )
        .bind(observation_id)
        .fetch_optional()
        .await?;
    let Some(status) = status else {
        return Ok(false);
    };
    // Cancelling the observation invalidates older pending alerts, while its
    // cancellation action still has a valid, previously accepted audience.
    let live = DbSession::new(connection)
        .scalar::<Uuid>(
            "supervision.notifications.lock_action",
            "SELECT id FROM supervision_actions WHERE id=$1 AND observation_id=$2
             AND ($3 <> 'cancelled' OR to_status='cancelled') FOR SHARE",
        )
        .bind(source_id)
        .bind(observation_id)
        .bind(status)
        .fetch_optional()
        .await?;
    Ok(live.is_some())
}
