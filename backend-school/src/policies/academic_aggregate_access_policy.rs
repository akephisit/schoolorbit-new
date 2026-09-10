use super::{
    academic_result_access_policy as results, learner_evaluation_access_policy as learner,
};
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};

pub fn require_aggregate_read(actor: &ActorContext) -> Result<(), AppError> {
    let learner_read = actor.has_permission(codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL)
        || learner::can_manage_school(actor)
        || learner::can_lock(actor)
        || learner::can_correct(actor);
    if !results::can_read_student_aggregate(actor) || !learner_read {
        return Err(AppError::Forbidden(
            "ต้องมีสิทธิ์อ่านผลการเรียนและผลประเมินระดับโรงเรียนทั้งสองส่วน".into(),
        ));
    }
    Ok(())
}

pub fn require_aggregate_lock(actor: &ActorContext) -> Result<(), AppError> {
    if !results::can_lock(actor) || !learner::can_lock(actor) {
        return Err(AppError::Forbidden(
            "ต้องมีสิทธิ์ล็อกผลการเรียนและผลประเมินทั้งสองส่วน".into(),
        ));
    }
    Ok(())
}

pub fn require_aggregate_policy_manage(actor: &ActorContext) -> Result<(), AppError> {
    if !results::can_manage_school(actor) || !learner::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "ต้องมีสิทธิ์จัดการนโยบายผลการเรียนและผลประเมินระดับโรงเรียน".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_access_does_not_upgrade_one_domain_or_management_to_lock() {
        let mut actor = ActorContext {
            user_id: uuid::Uuid::new_v4(),
            permissions: vec![codes::ACADEMIC_RESULT_MANAGE_SCHOOL.into()],
        };
        assert!(require_aggregate_read(&actor).is_err());
        actor
            .permissions
            .push(codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into());
        assert!(require_aggregate_read(&actor).is_ok());
        assert!(require_aggregate_lock(&actor).is_err());
        assert!(require_aggregate_policy_manage(&actor).is_err());
        actor.permissions.extend([
            codes::ACADEMIC_RESULT_LOCK_SCHOOL.into(),
            codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL.into(),
        ]);
        assert!(require_aggregate_lock(&actor).is_ok());
        actor
            .permissions
            .push(codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into());
        assert!(require_aggregate_policy_manage(&actor).is_ok());
    }
}
