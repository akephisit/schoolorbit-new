#[cfg(test)]
mod tests {
    use school_supervision::models::{
        RequestSupervisionObservationRequest, UpdateRequestedObservationRequest,
        UpdateSupervisionObservationRequest,
    };
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn schedule_only_requests_reject_all_retired_and_forged_inputs() {
        let create = json!({"cycleId":Uuid::new_v4(),"academicTermId":Uuid::new_v4(),"observedDate":"2026-11-17",
            "timetableBlockGroupIds":[Uuid::new_v4()],"expectedSourceDigest":"current"});
        let own = json!({"observedDate":"2026-11-17","timetableBlockGroupIds":[Uuid::new_v4()],"expectedSourceDigest":"current"});
        for field in [
            "manualLesson",
            "observedAt",
            "timetableBlockGroupId",
            "observedUserId",
            "homeroomId",
            "subjectName",
            "endsAt",
        ] {
            let mut request = create.clone();
            request[field] = json!("forged");
            assert!(
                serde_json::from_value::<RequestSupervisionObservationRequest>(request).is_err(),
                "{field}"
            );
            let mut request = own.clone();
            request[field] = json!("forged");
            assert!(
                serde_json::from_value::<UpdateRequestedObservationRequest>(request).is_err(),
                "{field}"
            );
            let mut request = json!({});
            request[field] = json!("forged");
            assert!(
                serde_json::from_value::<UpdateSupervisionObservationRequest>(request).is_err(),
                "{field}"
            );
        }
        assert!(serde_json::from_value::<RequestSupervisionObservationRequest>(create).is_ok());
        assert!(serde_json::from_value::<UpdateRequestedObservationRequest>(own).is_ok());
        for missing in [
            "observedDate",
            "timetableBlockGroupIds",
            "expectedSourceDigest",
        ] {
            let mut request = json!({"cycleId":Uuid::new_v4(),"academicTermId":Uuid::new_v4(),"observedDate":"2026-11-17",
                "timetableBlockGroupIds":[Uuid::new_v4()],"expectedSourceDigest":"current"});
            request.as_object_mut().unwrap().remove(missing);
            assert!(
                serde_json::from_value::<RequestSupervisionObservationRequest>(request).is_err(),
                "{missing}"
            );
        }
    }
}
