mod models;
mod services;

pub mod applications {
    pub use crate::models::applications::{
        AdmissionApplication, ApplicationDocument, ApplicationFilter, AssignRoomsGlobalRequest,
        AssignRoomsRequest, BulkScoreEntry, BulkUpdateScoresRequest, ChangeTrackRequest,
        CompleteEnrollmentRequest, EnrollmentForm, ExamScore, MarkAbsentRequest, MoveRoomRequest,
        PortalConfirmRequest, PortalCredentials, PortalFormRequest, RejectApplicationRequest,
        StudentIdRow, SubmitApplicationRequest, UpdateAdmissionTrackRequest,
        UpdateApplicationRequest, UpdateApplicationScoresRequest, UpdatePortalApplicationRequest,
        UpdateScoreEntry, UpdateStudentIdItem,
    };
    pub use crate::services::application_service::{
        attach_document, auto_assign_student_ids, batch_update_student_ids,
        change_application_track, complete_enrollment, delete_document_record,
        document_upload_response, fetch_application_files_then_delete,
        get_application_with_documents, list_applications, list_enrollment_pending,
        list_student_ids, mark_absent, move_application_room, reject_application,
        sort_room_students, submit_application, unverify_application, update_admission_track,
        update_application, verify_application, AppListRow, DocumentUploadResponse,
        DocumentUploadResult, EnrollmentPendingRow, EnrollmentResult, VALID_DOC_TYPES,
    };
}

pub mod exam_rooms {
    pub use crate::services::exam_room_service::{
        add_exam_room, assign_exam_seats, copy_exam_rooms_from_round, get_application_exam_seat,
        get_exam_config, get_exam_room, get_exam_seats, list_exam_rooms, remove_exam_room,
        update_exam_config, update_exam_room, AssignSeatsResult, AssignSeatsRoomSummary,
        ExamConfigResponse, ExamRoomRow, ExamSeatDetail, ListExamRoomsResult, RoomGroup, SeatRow,
    };
}

pub mod portal {
    pub use crate::services::portal_service::{
        authorize_document_change, authorize_document_download, check_application,
        confirm_enrollment, get_enrollment_form, get_exam_seat, get_status, submit_enrollment_form,
        update_application, CheckStatusRow, ExamSeatInfo, PortalAssignment, PortalExamSeatRequest,
        PortalStatusResult, VALID_DOC_TYPES,
    };
}

pub mod rounds {
    pub use crate::models::rounds::{
        AdmissionExamSubject, AdmissionRound, AdmissionTrack, CreateAdmissionRoundRequest,
        CreateAdmissionTrackRequest, CreateExamSubjectRequest, SelectionSettings,
        SelectionSettingsPatch, UpdateAdmissionRoundRequest, UpdateAdmissionTrackRequest,
        UpdateExamSubjectRequest, UpdateRoundStatusRequest, UpdateRoundVisibilityRequest,
        UpdateSelectionSettingsRequest,
    };
    pub use crate::services::round_service::{
        create_exam_subject, create_round, create_track, delete_exam_subject, delete_round,
        delete_track, get_public_round_info, get_round, get_track_capacity, list_exam_subjects,
        list_public_rounds, list_rounds, list_tracks, toggle_round_visibility, update_exam_subject,
        update_round, update_round_status, update_track, RoomCapacityRow,
    };
}

pub mod scores {
    pub use crate::services::score_service::{
        bulk_update_scores, get_all_scores, get_application_scores, update_application_scores,
        ScoreRow,
    };
}

pub mod selections {
    pub use crate::services::selection_service::{
        assign_rooms, assign_rooms_global, get_global_ranking, get_round_ranking, get_round_rooms,
        get_track_ranking, reset_all_room_assignments, update_selection_settings,
        GlobalRankingEntry, GlobalRankingResult, RankingRoomSummary, RoomBasic, RoundRankingEntry,
        RoundRankingResult, TrackRankingEntry, TrackRankingResult,
    };
}
