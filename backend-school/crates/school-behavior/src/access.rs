use crate::{models::BehaviorStudent, rules::invalid};
use chrono::NaiveDate;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct BehaviorAccess {
    pub actor_id: Uuid,
    pub own: bool,
    pub assigned: bool,
    pub school: bool,
    pub create_assigned: bool,
    pub create_school: bool,
    pub cancel_own: bool,
    pub cancel_school: bool,
}
impl BehaviorAccess {
    pub fn for_actor(actor: &ActorContext) -> Self {
        Self {
            actor_id: actor.user_id,
            own: actor.has_permission(codes::BEHAVIOR_READ_OWN),
            assigned: actor.has_permission(codes::BEHAVIOR_READ_ASSIGNED),
            school: actor.has_permission(codes::BEHAVIOR_READ_SCHOOL),
            create_assigned: actor.has_permission(codes::BEHAVIOR_CREATE_ASSIGNED),
            create_school: actor.has_permission(codes::BEHAVIOR_CREATE_SCHOOL),
            cancel_own: actor.has_permission(codes::BEHAVIOR_DELETE_OWN),
            cancel_school: actor.has_permission(codes::BEHAVIOR_DELETE_SCHOOL),
        }
    }
    pub fn require_read(self) -> Result<(), AppError> {
        if self.own || self.assigned || self.school {
            Ok(())
        } else {
            Err(AppError::Forbidden("ไม่มีสิทธิ์ดูพฤติกรรม".into()))
        }
    }
    pub fn require_selector(self) -> Result<(), AppError> {
        if self.create_school || self.create_assigned || self.school || self.assigned || self.own {
            Ok(())
        } else {
            Err(AppError::Forbidden("ไม่มีสิทธิ์เลือกรายชื่อนักเรียน".into()))
        }
    }
    pub fn require_create(self) -> Result<(), AppError> {
        if self.create_school || self.create_assigned {
            Ok(())
        } else {
            Err(AppError::Forbidden("ไม่มีสิทธิ์บันทึกพฤติกรรม".into()))
        }
    }
    pub fn can_cancel(self, creator: Option<Uuid>) -> bool {
        self.cancel_school || (self.cancel_own && creator == Some(self.actor_id))
    }
}
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorRosterRow {
    pub id: Uuid,
    pub student_academic_year_id: Uuid,
    pub name: String,
    pub student_code: Option<String>,
    pub grade_level_id: Uuid,
    pub grade_level: String,
    pub homeroom_id: Option<Uuid>,
    pub homeroom: String,
    pub class_number: Option<i32>,
    pub advisor_ids: Vec<Uuid>,
    pub parent_ids: Vec<Uuid>,
}
use serde::Serialize;
impl BehaviorRosterRow {
    pub fn visible(&self, access: BehaviorAccess) -> bool {
        access.school
            || access.create_school
            || ((access.assigned || access.create_assigned)
                && self.advisor_ids.contains(&access.actor_id))
            || (access.own
                && (self.id == access.actor_id || self.parent_ids.contains(&access.actor_id)))
    }
    pub fn writable(&self, access: BehaviorAccess) -> bool {
        access.create_school
            || (access.create_assigned && self.advisor_ids.contains(&access.actor_id))
    }
    pub fn student(self) -> BehaviorStudent {
        BehaviorStudent {
            id: self.id,
            student_academic_year_id: self.student_academic_year_id,
            name: self.name,
            student_code: self.student_code,
            grade_level_id: self.grade_level_id,
            grade_level: self.grade_level,
            homeroom_id: self.homeroom_id,
            homeroom: self.homeroom,
            class_number: self.class_number,
        }
    }
}
/// Dated placement owner uses the latest applicable span before room filtering,
/// so a moved student's earlier room never wins merely because it was requested.
pub async fn roster(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<BehaviorRosterRow>, AppError> {
    let rows:Vec<BehaviorRosterRow>=sqlx::query_as(r#"SELECT y.student_id id,y.id student_academic_year_id,concat_ws(' ',u.title,u.first_name,u.last_name) name,s.student_id student_code,
      y.grade_level_id,CASE g.level_type WHEN 'kindergarten' THEN 'อ.' WHEN 'primary' THEN 'ป.' ELSE 'ม.' END||g.year::text grade_level,
      p.homeroom_id,COALESCE(h.name,'ยังไม่จัดห้อง') homeroom,p.class_number,
      ARRAY(SELECT a.user_id FROM homeroom_advisors a WHERE a.homeroom_id=h.id ORDER BY a.user_id) advisor_ids,
      ARRAY(SELECT parent.parent_user_id FROM student_parents parent JOIN users pu ON pu.id=parent.parent_user_id AND pu.status='active' WHERE parent.student_user_id=y.student_id ORDER BY parent.parent_user_id) parent_ids
      FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id
      JOIN users u ON u.id=y.student_id AND u.user_type='student' JOIN student_info s ON s.user_id=u.id
      JOIN grade_levels g ON g.id=y.grade_level_id
      LEFT JOIN LATERAL(SELECT p.homeroom_id,p.class_number FROM homeroom_placements p WHERE p.student_academic_year_id=y.id AND p.status IN('current','ended') AND p.start_date<=$2 AND(p.end_date IS NULL OR p.end_date>=$2) ORDER BY p.start_date DESC,p.id LIMIT 1)p ON true
      LEFT JOIN homerooms h ON h.id=p.homeroom_id WHERE t.id=$1 AND(p.homeroom_id IS NOT NULL OR y.status='active') ORDER BY y.student_id LIMIT 10001"#)
      .bind(term).bind(date).fetch_all(connection).await?;
    if rows.len() > 10000 {
        return Err(invalid("รายชื่อนักเรียนเกินขนาดที่รองรับ"));
    }
    Ok(rows)
}
/// Parent/self and assigned are independent OR branches. Creator ownership is
/// deliberately a log-only scope and never grants another student's whole summary.
pub async fn require_student_read(
    connection: &mut PgConnection,
    access: BehaviorAccess,
    student: Uuid,
) -> Result<(), AppError> {
    access.require_read()?;
    let allowed:bool=sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=$1 AND u.user_type='student' AND(
      $2 OR($3 AND(u.id=$4 OR EXISTS(SELECT 1 FROM student_parents p JOIN users pu ON pu.id=p.parent_user_id AND pu.status='active' WHERE p.student_user_id=u.id AND p.parent_user_id=$4)))
      OR($5 AND EXISTS(SELECT 1 FROM homeroom_placements hp JOIN student_academic_years y ON y.id=hp.student_academic_year_id JOIN homeroom_advisors a ON a.homeroom_id=hp.homeroom_id WHERE y.student_id=u.id AND a.user_id=$4))))"#)
      .bind(student).bind(access.school).bind(access.own).bind(access.actor_id).bind(access.assigned).fetch_one(connection).await?;
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์ดูคะแนนนักเรียนนี้".into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_scopes_and_creator_cancellation() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![
                codes::BEHAVIOR_READ_OWN.into(),
                codes::BEHAVIOR_READ_ASSIGNED.into(),
                codes::BEHAVIOR_DELETE_OWN.into(),
            ],
        };
        let a = BehaviorAccess::for_actor(&actor);
        assert!(a.own && a.assigned);
        assert!(a.can_cancel(Some(actor.user_id)));
        assert!(!a.can_cancel(Some(Uuid::new_v4())));
        assert!(!a.can_cancel(None));
    }
}
