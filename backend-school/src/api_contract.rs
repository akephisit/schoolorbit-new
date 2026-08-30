use crate::api_response::{
    ApiErrorResponse, ApiErrorResponseWithData, ApiErrorResponseWithOptionalData, ApiResponse,
    EmptyData, UuidIdData,
};
use crate::modules::academic::core::models::*;
use crate::modules::academic::delivery::models::*;
use crate::modules::academic::models::assessment::*;
use crate::modules::academic::models::exam_schedule::*;
use crate::modules::academic::models::timetable::{
    ApplyTemplateRequest, BatchTimetableResult, ClearTimetableRequest,
    CreateBatchTimetableEntriesRequest, CreateTemplateRequest, CreateTimetableEntryRequest,
    FromCurrentRequest, MoveValidityCell, SwapTimetableEntriesRequest,
    SwapTimetableEntriesResponse, TemplateApplyResult, TemplateWithEntries, TimetableEntry,
    TimetableInstructor, TimetableOccupancyCell, TimetableTemplate, TimetableTemplateEntry,
    TimetableTemplateTargetSelector, UpdateTemplateRequest, UpdateTimetableEntryRequest,
    ValidateMovesRequest,
};
use crate::modules::academic::models::timetable_version::{
    CloneTimetableVersionRequest, TimetableVersion, TimetableVersionDisplayState,
    TimetableVersionStatus, TimetableVersionTarget,
};
use crate::modules::academic::services::daily_teaching_service::{
    DailyTeachingEntry, DailyTeachingOverview, DailyTeachingPeriod, DailyTeachingPeriodCell,
    DailyTeachingSummary, DailyTeachingTeacher,
};
use crate::modules::achievement::models::{
    Achievement, AchievementListFilter, CreateAchievementRequest, UpdateAchievementRequest,
};
use crate::modules::admission::handlers::applications::StaffDocumentMultipart;
use crate::modules::admission::handlers::portal::{
    PortalDocumentMultipart, PortalUploadDocumentData,
};
use crate::modules::admission::models::applications::PortalCredentials;
use crate::modules::admission::services::application_service::DocumentUploadResponse;
use crate::modules::auth::models::{
    ChangePasswordRequest, CurrentUserResponse, LoginData, LoginRequest, ProfileResponse,
    SessionListData, SessionResponse, UpdateProfileRequest,
};
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarCategory, CalendarEvent, CalendarEventReminder, CalendarEventTag,
    CalendarEventTarget, CalendarPublicEvent, CalendarTag, CalendarViewerEvent, CalendarVisibility,
};
use crate::modules::certificates::models::{
    AttachCertificateAssetRequest, AttachCertificateBackgroundRequest, CandidateMatchStatus,
    CandidateNameSource, CandidateValidationCode, CandidateValidationStatus,
    CertificateAccountSearchQuery, CertificateBuiltInFont, CertificateCampaignCapabilities,
    CertificateCampaignDetail, CertificateCampaignListQuery, CertificateCampaignPurgeCounts,
    CertificateCampaignPurgeImpact, CertificateCampaignPurgePhase, CertificateCampaignPurgeStatus,
    CertificateCampaignStatus, CertificateCampaignSummary, CertificateCandidateAccount,
    CertificateCandidateBulkRequest, CertificateCandidateBulkResult,
    CertificateCandidateCapabilities, CertificateCandidateDetail, CertificateCandidateImportResult,
    CertificateCandidateListQuery, CertificateCandidateListResponse, CertificateCandidateSummary,
    CertificateCapabilities, CertificateElement, CertificateFontSource,
    CertificateImportBatchSummary, CertificateImportRequest, CertificateImportRowInput,
    CertificateImportSource, CertificateIssueCandidateProblem, CertificateIssueCode,
    CertificateIssueRequestCapabilities, CertificateIssueRequestDetail,
    CertificateIssueRequestItem, CertificateIssueRequestListQuery, CertificateIssueRequestStatus,
    CertificateIssueRequestSummary, CertificateLayoutV1, CertificatePageBox,
    CertificatePageGeometry, CertificatePreviewKind, CertificatePreviewManifestRequest,
    CertificateRenderCampaignValues, CertificateRenderFileGrant, CertificateRenderFontGrant,
    CertificateRenderImageGrant, CertificateRenderManifest, CertificateRenderManifestBatchRequest,
    CertificateReplacementCandidate, CertificateResourceLockCode, CertificateResourceLocked,
    CertificateStatus, CertificateTemplateAsset, CertificateTemplateAssetKind,
    CertificateTemplateCapabilities, CertificateTemplateDeleteDisposition,
    CertificateTemplateDeleteResult, CertificateTemplateDetail, CertificateTemplateVariableCatalog,
    ChangeCertificateCampaignStatusRequest, CreateAccountCertificateCandidateRequest,
    CreateCertificateCampaignRequest, CreateCertificateTemplateRequest,
    CreateManualExternalCandidateRequest, ElementFrame, GeometryAction, ImageElement,
    IssueCertificateOutcome, IssueCertificateRequest, IssuedCertificateDetail,
    IssuedCertificateListQuery, IssuedCertificateSummary, ManualCertificateVerificationRequest,
    NullableUuidUpdate, PublicCertificateRenderRequest, PublicCertificateVerificationData,
    QrCertificateVerificationRequest, QrElement, RecipientType, ReturnCertificateIssueRequest,
    RevokeCertificateRequest, RevokeCertificateResult, StartCertificateCampaignPurgeRequest,
    SubmitCertificateIssueRequest, TextAlignment, TextElement, TextShadow,
    UpdateCertificateCampaignRequest, UpdateCertificateCandidateRequest,
    UpdateCertificateTemplateRequest,
};
use crate::modules::facility::models::Room;
use crate::modules::files::models::{
    FileDeleteResult, FileDownloadGrantResponse, FileMetadata, FileUploadMultipart,
    PublicFileDeliveryResponse,
};
use crate::modules::files::platform_types::{FileLifecycleStatus, FilePurpose};
use crate::modules::lookup::models::{
    AcademicYearLookupItem, GradeLevelLookupItem, HomeroomLookupItem, LookupItem,
    OrganizationUnitLookupItem, RoleLookupItem, StaffLookupItem, StudentLookupItem,
};
use crate::modules::menu::handlers::admin::{
    ApplyAcademicMenuTemplateRequest, CreateMenuGroupRequest, CreateMenuItemRequest,
    CreateMenuWorkspaceRequest, MoveItemToGroupRequest, MovedCountData, ReorderGroupsRequest,
    ReorderItem, ReorderRequest, ReorderWorkspacesRequest, UpdateMenuGroupRequest,
    UpdateMenuItemRequest, UpdateMenuWorkspaceRequest,
};
use crate::modules::menu::handlers::public::UserMenuData;
use crate::modules::menu::models::{
    AcademicMenuTemplateApplyResult, AcademicMenuTemplateMove, AcademicMenuTemplatePreview,
    AcademicMenuTemplateSection, FeatureToggle, MenuGroup, MenuGroupResponse, MenuItem,
    MenuItemResponse, MenuWorkspace,
};
use crate::modules::notification::models::{ListNotificationsResponse, Notification};
use crate::modules::parents::models::{ChildDto, ParentProfile};
use crate::modules::question_bank::models::{
    ImageAlignment, ImageNodeAttributes, MathNodeAttributes, QuestionBankExportDataRequest,
    QuestionBankListQuery, QuestionBankOptions, QuestionBankPage, QuestionBankSubjectOption,
    QuestionBankSummary, QuestionChoice, QuestionDetail, QuestionFile, QuestionSummary,
    RichBlockNode, RichContent, RichDocument, RichInlineNode, RichTextMark,
    UpsertQuestionChoiceRequest, UpsertQuestionRequest,
};
use crate::modules::school::handlers::PublicSchoolInfoData;
use crate::modules::school::models::SchoolSettingsResponse;
use crate::modules::school_fonts::models::{
    AttachSchoolFontBatchRequest, InspectSchoolFontUploadsRequest, SchoolFontDeleteConflict,
    SchoolFontListResponse, SchoolFontStyle, SchoolFontSummary, SchoolFontUploadInspection,
    SchoolFontUploadInspectionFile, SchoolFontUploadStatus,
};
use crate::modules::staff::handlers::organization_delegations::{
    CreateDelegationRequest, DelegationIdData, DelegationItem,
};
use crate::modules::staff::handlers::organization_members::{
    AddMemberRequest, ListMembersQuery, OrganizationMemberItem, UpdateMemberRequest,
};
use crate::modules::staff::handlers::staff::StaffListData;
use crate::modules::staff::models::{
    AdvisorHomeroomItem, AssignRoleRequest, CreateOrganizationUnitRequest, CreateRoleRequest,
    CreateStaffInfoRequest, CreateStaffRequest, OrganizationAssignment,
    OrganizationPermissionGrantInput, OrganizationUnit, OrganizationUnitResponse, Permission, Role,
    RoleResponse, StaffInfoResponse, StaffListItem, StaffProfileResponse, TeachingAssignmentItem,
    UpdateOrganizationPermissionsRequest, UpdateOrganizationUnitRequest, UpdateRoleRequest,
    UpdateStaffRequest, UserRoleAssignmentResponse,
};
use crate::modules::staff::services::dashboard_service::StaffDashboardOverview;
use crate::modules::staff::services::organization_delegation_service::DelegatablePermission;
use crate::modules::staff::services::organization_permission_service::OrganizationPermissionGrant;
use crate::modules::staff::services::staff_service::{
    PublicStaffOrganizationUnit, PublicStaffProfile, PublicStaffRole,
};
use crate::modules::students::models::{
    CreateParentRequest, CreateStudentRequest, CreateStudentResponse, ParentDto, StudentDbRow,
    StudentListItem, StudentListResponse, StudentProfile, UpdateOwnProfileRequest,
    UpdateStudentRequest,
};
use crate::modules::supervision::handlers::{ItemsData, ListObservationsQuery};
use crate::modules::supervision::models::*;
use crate::modules::system::handlers::feature_toggles::{
    FeatureListResponse, FeatureToggleResponse,
};
use serde_json::Value;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::auth::session_handlers::login,
        crate::modules::auth::session_handlers::logout,
        crate::modules::auth::session_handlers::me,
        crate::modules::auth::handlers::get_profile,
        crate::modules::auth::handlers::update_profile,
        crate::modules::auth::session_handlers::change_password,
        crate::modules::auth::session_handlers::list_sessions,
        crate::modules::auth::session_handlers::revoke_session,
        crate::modules::auth::session_handlers::logout_all,
        crate::modules::files::handlers::upload_file,
        crate::modules::files::handlers::get_file_metadata,
        crate::modules::files::handlers::download_file,
        crate::modules::files::handlers::delete_file,
        crate::modules::files::handlers::get_public_file_content,
        crate::modules::files::handlers::get_public_file_delivery,
        crate::modules::school_fonts::handlers::list_school_fonts,
        crate::modules::school_fonts::handlers::inspect_school_font_uploads,
        crate::modules::school_fonts::handlers::attach_school_font_batch,
        crate::modules::school_fonts::handlers::delete_school_font,
        crate::modules::admission::handlers::applications::staff_upload_document,
        crate::modules::admission::handlers::applications::staff_delete_document,
        crate::modules::admission::handlers::portal::portal_upload_document,
        crate::modules::admission::handlers::portal::portal_delete_document,
        crate::modules::admission::handlers::portal::portal_download_document,
        crate::modules::menu::handlers::public::get_user_menu,
        crate::modules::system::handlers::feature_toggles::list_features,
        crate::modules::system::handlers::feature_toggles::get_feature,
        crate::modules::menu::handlers::admin::list_menu_workspaces,
        crate::modules::menu::handlers::admin::create_menu_workspace,
        crate::modules::menu::handlers::admin::update_menu_workspace,
        crate::modules::menu::handlers::admin::delete_menu_workspace,
        crate::modules::menu::handlers::admin::reorder_menu_workspaces,
        crate::modules::menu::handlers::admin::list_menu_groups,
        crate::modules::menu::handlers::admin::create_menu_group,
        crate::modules::menu::handlers::admin::update_menu_group,
        crate::modules::menu::handlers::admin::delete_menu_group,
        crate::modules::menu::handlers::admin::reorder_menu_groups,
        crate::modules::menu::handlers::admin::list_menu_items,
        crate::modules::menu::handlers::admin::create_menu_item,
        crate::modules::menu::handlers::admin::update_menu_item,
        crate::modules::menu::handlers::admin::delete_menu_item,
        crate::modules::menu::handlers::admin::move_item_to_group,
        crate::modules::menu::handlers::admin::reorder_menu_items,
        crate::modules::menu::handlers::admin::preview_recommended_academic_menu_template,
        crate::modules::menu::handlers::admin::apply_recommended_academic_menu_template,
        crate::modules::lookup::handlers::lookup_staff,
        crate::modules::lookup::handlers::lookup_students,
        crate::modules::lookup::handlers::lookup_rooms,
        crate::modules::lookup::handlers::lookup_roles,
        crate::modules::lookup::handlers::lookup_organization_units,
        crate::modules::lookup::handlers::lookup_organization_unit_by_id,
        crate::modules::lookup::handlers::lookup_grade_levels,
        crate::modules::lookup::handlers::lookup_homerooms,
        crate::modules::lookup::handlers::lookup_academic_years,
        crate::modules::lookup::handlers::lookup_subjects,
        crate::modules::staff::handlers::staff::list_staff,
        crate::modules::staff::handlers::staff::get_staff_dashboard,
        crate::modules::staff::handlers::staff::get_staff_profile,
        crate::modules::staff::handlers::staff::get_public_staff_profile,
        crate::modules::staff::handlers::staff::create_staff,
        crate::modules::staff::handlers::staff::update_staff,
        crate::modules::staff::handlers::staff::delete_staff,
        crate::modules::students::handlers::get_own_profile,
        crate::modules::students::handlers::update_own_profile,
        crate::modules::students::handlers::list_students,
        crate::modules::students::handlers::create_student,
        crate::modules::students::handlers::get_student,
        crate::modules::students::handlers::update_student,
        crate::modules::students::handlers::delete_student,
        crate::modules::students::handlers_parents::add_parent_to_student,
        crate::modules::students::handlers_parents::remove_parent_from_student,
        crate::modules::achievement::handlers::list_achievements,
        crate::modules::achievement::handlers::create_achievement,
        crate::modules::achievement::handlers::update_achievement,
        crate::modules::achievement::handlers::delete_achievement,
        crate::modules::certificates::handlers::list_certificate_campaigns,
        crate::modules::certificates::handlers::create_certificate_campaign,
        crate::modules::certificates::handlers::get_certificate_campaign,
        crate::modules::certificates::handlers::update_certificate_campaign,
        crate::modules::certificates::handlers::change_certificate_campaign_status,
        crate::modules::certificates::handlers::get_certificate_campaign_purge_impact,
        crate::modules::certificates::handlers::start_certificate_campaign_purge,
        crate::modules::certificates::handlers::get_certificate_campaign_purge_status,
        crate::modules::certificates::handlers::retry_certificate_campaign_purge,
        crate::modules::certificates::handlers::list_certificate_owner_options,
        crate::modules::certificates::handlers::list_certificate_templates,
        crate::modules::certificates::handlers::create_certificate_template,
        crate::modules::certificates::handlers::get_certificate_template,
        crate::modules::certificates::handlers::update_certificate_template,
        crate::modules::certificates::handlers::delete_certificate_template,
        crate::modules::certificates::handlers::attach_certificate_template_background,
        crate::modules::certificates::handlers::attach_certificate_template_asset,
        crate::modules::certificates::handlers::list_certificate_school_fonts,
        crate::modules::certificates::handlers::inspect_certificate_font_uploads,
        crate::modules::certificates::handlers::attach_certificate_font_batch,
        crate::modules::certificates::handlers::delete_certificate_template_asset,
        crate::modules::certificates::handlers::get_certificate_template_variable_catalog,
        crate::modules::certificates::handlers::create_certificate_template_preview_manifest,
        crate::modules::certificates::handlers::list_certificate_candidates,
        crate::modules::certificates::handlers::import_certificate_candidates,
        crate::modules::certificates::handlers::create_manual_certificate_candidate,
        crate::modules::certificates::handlers::search_certificate_candidate_accounts,
        crate::modules::certificates::handlers::create_account_certificate_candidate,
        crate::modules::certificates::handlers::bulk_update_certificate_candidates,
        crate::modules::certificates::handlers::get_certificate_candidate,
        crate::modules::certificates::handlers::update_certificate_candidate,
        crate::modules::certificates::handlers::delete_certificate_candidate,
        crate::modules::certificates::handlers::list_certificate_campaign_issue_requests,
        crate::modules::certificates::handlers::submit_certificate_issue_request,
        crate::modules::certificates::handlers::list_certificate_issue_requests,
        crate::modules::certificates::handlers::get_certificate_issue_request,
        crate::modules::certificates::handlers::withdraw_certificate_issue_request,
        crate::modules::certificates::handlers::start_certificate_issue_request_review,
        crate::modules::certificates::handlers::return_certificate_issue_request,
        crate::modules::certificates::handlers::issue_certificates,
        crate::modules::certificates::handlers::list_issued_certificates,
        crate::modules::certificates::handlers::get_issued_certificate,
        crate::modules::certificates::handlers::revoke_issued_certificate,
        crate::modules::certificates::handlers::create_issued_certificate_render_manifest,
        crate::modules::certificates::handlers::create_issued_certificate_render_manifests,
        crate::modules::certificates::handlers::list_own_certificates,
        crate::modules::certificates::handlers::get_own_certificate,
        crate::modules::certificates::handlers::create_own_certificate_render_manifest,
        crate::modules::certificates::handlers::verify_certificate_manually,
        crate::modules::certificates::handlers::verify_certificate_by_qr,
        crate::modules::certificates::handlers::create_public_certificate_render_manifest,
        crate::modules::parents::handlers::get_own_parent_profile,
        crate::modules::parents::handlers::get_parent_academic_context_options,
        crate::modules::parents::handlers::get_child_profile,
        crate::modules::parents::handlers::get_child_academic_context_options,
        crate::modules::parents::handlers::get_child_timetable,
        crate::modules::parents::handlers::get_child_exam_schedule,
        crate::modules::parents::handlers::get_child_calendar_events,
        crate::modules::academic::handlers::timetable::get_my_timetable,
        crate::modules::academic::core::handlers::list_my_context_options,
        crate::modules::academic::handlers::timetable::list_timetable_entries,
        crate::modules::academic::handlers::timetable::create_timetable_entry,
        crate::modules::academic::handlers::timetable::create_batch_timetable_entries,
        crate::modules::academic::handlers::timetable::update_timetable_entry,
        crate::modules::academic::handlers::timetable::delete_timetable_entry,
        crate::modules::academic::handlers::timetable::delete_batch_group,
        crate::modules::academic::handlers::timetable::swap_timetable_entries,
        crate::modules::academic::handlers::timetable::validate_timetable_moves,
        crate::modules::academic::handlers::timetable::get_timetable_occupancy,
        crate::modules::academic::handlers::timetable::daily_teaching_overview,
        crate::modules::academic::handlers::timetable_versions::list_versions,
        crate::modules::academic::handlers::timetable_versions::resolve_version,
        crate::modules::academic::handlers::timetable_versions::clone_version,
        crate::modules::academic::handlers::timetable_templates::list_templates,
        crate::modules::academic::handlers::timetable_templates::get_template,
        crate::modules::academic::handlers::timetable_templates::create_template,
        crate::modules::academic::handlers::timetable_templates::update_template,
        crate::modules::academic::handlers::timetable_templates::delete_template,
        crate::modules::academic::handlers::timetable_templates::from_current,
        crate::modules::academic::handlers::timetable_templates::apply_template,
        crate::modules::academic::handlers::timetable_templates::clear_timetable,
        crate::modules::academic::handlers::assessment::list_assessment_plans,
        crate::modules::academic::handlers::assessment::get_assessment_settings,
        crate::modules::academic::handlers::assessment::update_assessment_settings,
        crate::modules::academic::handlers::assessment::get_assessment_plan,
        crate::modules::academic::handlers::assessment::save_assessment_plan,
        crate::modules::academic::handlers::assessment::submit_assessment_plan,
        crate::modules::academic::handlers::exam_schedule::list_rounds,
        crate::modules::academic::handlers::exam_schedule::create_round,
        crate::modules::academic::handlers::exam_schedule::update_round,
        crate::modules::academic::handlers::exam_schedule::get_workspace,
        crate::modules::academic::handlers::exam_schedule::import_items,
        crate::modules::academic::handlers::exam_schedule::clear_mismatched_items,
        crate::modules::academic::handlers::exam_schedule::upsert_day,
        crate::modules::academic::handlers::exam_schedule::update_day,
        crate::modules::academic::handlers::exam_schedule::delete_day,
        crate::modules::academic::handlers::exam_schedule::list_day_room_assignments,
        crate::modules::academic::handlers::exam_schedule::get_invigilator_workspace,
        crate::modules::academic::handlers::exam_schedule::get_invigilator_staff_options,
        crate::modules::academic::handlers::exam_schedule::upsert_day_room_assignment,
        crate::modules::academic::handlers::exam_schedule::update_assignment_invigilators,
        crate::modules::academic::handlers::exam_schedule::assign_assignment_invigilator,
        crate::modules::academic::handlers::exam_schedule::remove_assignment_invigilator,
        crate::modules::academic::handlers::exam_schedule::generate_seats,
        crate::modules::academic::handlers::exam_schedule::place_session,
        crate::modules::academic::handlers::exam_schedule::delete_session,
        crate::modules::academic::handlers::exam_schedule::publish_round,
        crate::modules::academic::handlers::exam_schedule::list_my_exam_schedule,
        crate::modules::academic::handlers::exam_schedule::list_staff_exam_schedule,
        crate::modules::academic::core::handlers::list_context_options,
		crate::modules::academic::core::handlers::list_public_context_options,
        crate::modules::academic::core::handlers::get_academic_setup_workspace,
        crate::modules::academic::core::handlers::list_years,
        crate::modules::academic::core::handlers::create_year,
        crate::modules::academic::core::handlers::get_year,
        crate::modules::academic::core::handlers::update_year,
        crate::modules::academic::core::handlers::list_terms,
        crate::modules::academic::core::handlers::create_term,
        crate::modules::academic::core::handlers::get_term,
        crate::modules::academic::core::handlers::update_term,
        crate::modules::academic::core::handlers::delete_term,
        crate::modules::academic::core::handlers::list_bell_schedules,
        crate::modules::academic::core::handlers::create_bell_schedule,
        crate::modules::academic::core::handlers::get_bell_schedule,
        crate::modules::academic::core::handlers::update_bell_schedule,
        crate::modules::academic::core::handlers::list_bell_schedule_periods,
        crate::modules::academic::core::handlers::replace_bell_schedule_periods,
        crate::modules::academic::core::handlers::list_grade_progressions,
        crate::modules::academic::core::handlers::replace_grade_progressions,
        crate::modules::academic::core::handlers::list_catalog_subjects,
        crate::modules::academic::core::handlers::get_catalog_subject_overview,
        crate::modules::academic::core::handlers::create_catalog_subject,
        crate::modules::academic::core::handlers::get_catalog_subject,
        crate::modules::academic::core::handlers::update_catalog_subject,
        crate::modules::academic::core::handlers::list_subject_versions,
        crate::modules::academic::core::handlers::create_subject_version,
        crate::modules::academic::core::handlers::get_subject_version,
        crate::modules::academic::core::handlers::update_subject_version,
        crate::modules::academic::core::handlers::publish_subject_version,
        crate::modules::academic::core::handlers::list_subject_default_teachers,
        crate::modules::academic::core::handlers::replace_subject_default_teachers,
        crate::modules::academic::core::handlers::list_subject_groups,
        crate::modules::academic::core::handlers::create_subject_group,
        crate::modules::academic::core::handlers::get_subject_group,
        crate::modules::academic::core::handlers::update_subject_group,
        crate::modules::academic::core::handlers::delete_subject_group,
        crate::modules::academic::core::handlers::list_catalog_activities,
        crate::modules::academic::core::handlers::get_catalog_activity_overview,
        crate::modules::academic::core::handlers::create_catalog_activity,
        crate::modules::academic::core::handlers::get_catalog_activity,
        crate::modules::academic::core::handlers::update_catalog_activity,
        crate::modules::academic::core::handlers::list_activity_versions,
        crate::modules::academic::core::handlers::create_activity_version,
        crate::modules::academic::core::handlers::get_activity_version,
        crate::modules::academic::core::handlers::update_activity_version,
        crate::modules::academic::core::handlers::publish_activity_version,
        crate::modules::academic::core::handlers::list_activity_default_teachers,
        crate::modules::academic::core::handlers::replace_activity_default_teachers,
        crate::modules::academic::core::handlers::list_curricula,
        crate::modules::academic::core::handlers::get_curriculum_overview,
        crate::modules::academic::core::handlers::get_curriculum_create_options,
        crate::modules::academic::core::handlers::list_study_program_options_for_year,
        crate::modules::academic::core::handlers::create_curriculum,
        crate::modules::academic::core::handlers::get_curriculum,
        crate::modules::academic::core::handlers::update_curriculum,
        crate::modules::academic::core::handlers::list_curriculum_versions,
        crate::modules::academic::core::handlers::create_curriculum_version,
        crate::modules::academic::core::handlers::get_curriculum_version,
        crate::modules::academic::core::handlers::get_curriculum_management_options,
        crate::modules::academic::core::handlers::update_curriculum_version,
        crate::modules::academic::core::handlers::publish_curriculum_version,
        crate::modules::academic::core::handlers::get_curriculum_structure_workspace,
        crate::modules::academic::core::handlers::replace_curriculum_term_slots,
        crate::modules::academic::core::handlers::list_study_programs,
        crate::modules::academic::core::handlers::create_study_program,
        crate::modules::academic::core::handlers::get_study_program,
        crate::modules::academic::core::handlers::update_study_program,
        crate::modules::academic::core::handlers::replace_curriculum_structure,
        crate::modules::academic::core::handlers::list_homerooms,
        crate::modules::academic::core::handlers::create_homeroom,
        crate::modules::academic::core::handlers::get_homeroom,
        crate::modules::academic::core::handlers::update_homeroom,
        crate::modules::academic::core::handlers::list_homeroom_advisors,
        crate::modules::academic::core::handlers::list_homeroom_advisors_for_year,
        crate::modules::academic::core::handlers::replace_homeroom_advisors,
        crate::modules::academic::core::handlers::list_student_years,
        crate::modules::academic::core::handlers::list_student_year_candidates,
        crate::modules::academic::core::handlers::create_student_year,
        crate::modules::academic::core::handlers::get_student_year,
        crate::modules::academic::core::handlers::update_student_year,
        crate::modules::academic::core::handlers::list_placements,
        crate::modules::academic::core::handlers::list_placements_for_year,
        crate::modules::academic::core::handlers::create_placement,
        crate::modules::academic::core::handlers::transfer_placement,
        crate::modules::academic::delivery::handlers::list_offerings,
        crate::modules::academic::delivery::handlers::get_delivery_overview,
        crate::modules::academic::delivery::handlers::get_homeroom_delivery_workspace,
        crate::modules::academic::delivery::handlers::get_delivery_management_options,
        crate::modules::academic::delivery::handlers::create_offering,
        crate::modules::academic::delivery::handlers::preview_offerings_from_curriculum,
        crate::modules::academic::delivery::handlers::apply_offerings_from_curriculum,
        crate::modules::academic::delivery::handlers::get_offering,
        crate::modules::academic::delivery::handlers::update_offering,
        crate::modules::academic::delivery::handlers::publish_offering,
        crate::modules::academic::delivery::handlers::list_groups_for_term,
        crate::modules::academic::delivery::handlers::list_groups,
        crate::modules::academic::delivery::handlers::create_group,
        crate::modules::academic::delivery::handlers::get_group,
        crate::modules::academic::delivery::handlers::update_group,
        crate::modules::academic::delivery::handlers::list_group_homerooms,
        crate::modules::academic::delivery::handlers::replace_group_homerooms,
        crate::modules::academic::delivery::handlers::list_group_teachers,
        crate::modules::academic::delivery::handlers::replace_group_teachers,
        crate::modules::academic::delivery::handlers::preview_group_roster,
        crate::modules::academic::delivery::handlers::apply_group_roster,
        crate::modules::academic::delivery::handlers::publish_group_roster,
        crate::modules::academic::delivery::handlers::list_term_change_sets,
        crate::modules::academic::delivery::handlers::create_term_change_set,
        crate::modules::academic::delivery::handlers::get_term_change_set,
        crate::modules::academic::delivery::handlers::update_term_change_set,
        crate::modules::academic::delivery::handlers::cancel_term_change_set,
        crate::modules::academic::delivery::handlers::upsert_term_change_item,
        crate::modules::academic::delivery::handlers::delete_term_change_item,
        crate::modules::academic::delivery::handlers::preview_term_change_set,
        crate::modules::academic::delivery::handlers::publish_term_change_set,
        crate::modules::academic::delivery::handlers::list_group_memberships,
        crate::modules::academic::delivery::handlers::add_group_membership,
        crate::modules::academic::delivery::handlers::end_group_membership,
        crate::modules::academic::delivery::handlers::list_my_activity_registrations,
        crate::modules::academic::delivery::handlers::enroll_my_activity_registration,
        crate::modules::academic::delivery::handlers::unenroll_my_activity_registration,
        crate::modules::supervision::handlers::list_cycles,
        crate::modules::supervision::handlers::create_cycle,
        crate::modules::supervision::handlers::update_cycle,
        crate::modules::supervision::handlers::list_templates,
        crate::modules::supervision::handlers::create_template,
        crate::modules::supervision::handlers::get_template,
        crate::modules::supervision::handlers::update_template,
        crate::modules::supervision::handlers::list_observations,
        crate::modules::supervision::handlers::get_observation,
        crate::modules::supervision::handlers::get_observation_review,
        crate::modules::supervision::handlers::evaluator_availability,
        crate::modules::supervision::handlers::observation_timetable_options,
        crate::modules::supervision::handlers::request_observation,
        crate::modules::supervision::handlers::update_requested_observation,
        crate::modules::supervision::handlers::cancel_requested_observation,
        crate::modules::supervision::handlers::update_observation,
        crate::modules::supervision::handlers::replace_observation_evaluators,
        crate::modules::supervision::handlers::cancel_observation,
        crate::modules::supervision::handlers::approve_observation_request,
        crate::modules::supervision::handlers::return_observation_request,
        crate::modules::supervision::handlers::submit_my_evaluation,
        crate::modules::supervision::handlers::certify_observation,
        crate::modules::supervision::handlers::approve_observation,
        crate::modules::supervision::handlers::acknowledge_observation,
        crate::modules::supervision::handlers::cycle_progress,
        crate::modules::supervision::handlers::teacher_status_overview,
        crate::modules::question_bank::handlers::list_options,
        crate::modules::question_bank::handlers::list_questions,
        crate::modules::question_bank::handlers::create_question,
        crate::modules::question_bank::handlers::export_question_data,
        crate::modules::question_bank::handlers::get_question,
        crate::modules::question_bank::handlers::update_question,
        crate::modules::question_bank::handlers::delete_question,
        crate::modules::question_bank::handlers::get_question_file,
        crate::modules::calendar::handlers::list_my_calendar_events,
        crate::modules::calendar::handlers::list_public_calendar_events,
        crate::modules::calendar::handlers::list_calendar_events,
        crate::modules::calendar::handlers::list_calendar_categories,
        crate::modules::calendar::handlers::list_calendar_tags,
        crate::modules::school::handlers::get_public_info,
        crate::modules::school::handlers::get_settings,
        crate::modules::notification::handlers::list_notifications,
        crate::modules::staff::handlers::roles::list_roles,
        crate::modules::staff::handlers::roles::get_role,
        crate::modules::staff::handlers::roles::create_role,
        crate::modules::staff::handlers::roles::update_role,
        crate::modules::staff::handlers::roles::deactivate_role,
        crate::modules::staff::handlers::permissions::list_permissions,
        crate::modules::staff::handlers::permissions::list_permissions_by_module,
        crate::modules::staff::handlers::user_roles::get_user_roles,
        crate::modules::staff::handlers::user_roles::assign_user_role,
        crate::modules::staff::handlers::user_roles::remove_user_role,
        crate::modules::staff::handlers::user_roles::get_user_permissions,
        crate::modules::staff::handlers::roles::list_organization_units,
        crate::modules::staff::handlers::roles::get_organization_unit,
        crate::modules::staff::handlers::roles::create_organization_unit,
        crate::modules::staff::handlers::roles::update_organization_unit,
        crate::modules::staff::handlers::roles::deactivate_organization_unit,
        crate::modules::staff::handlers::organization_permissions::get_organization_permissions,
        crate::modules::staff::handlers::organization_permissions::update_organization_permissions,
        crate::modules::staff::handlers::organization_delegations::list_delegatable_permissions,
        crate::modules::staff::handlers::organization_delegations::list_delegations,
        crate::modules::staff::handlers::organization_delegations::create_delegation,
        crate::modules::staff::handlers::organization_delegations::revoke_delegation,
        crate::modules::staff::handlers::organization_members::list_members,
        crate::modules::staff::handlers::organization_members::add_member,
        crate::modules::staff::handlers::organization_members::update_member,
        crate::modules::staff::handlers::organization_members::remove_member
    ),
    components(schemas(
        LoginRequest,
        LoginData,
        ProfileResponse,
        UpdateProfileRequest,
        ChangePasswordRequest,
        CurrentUserResponse,
        SessionResponse,
        SessionListData,
        ApiResponse<CurrentUserResponse>,
        ApiResponse<SessionListData>,
        ApiResponse<LoginData>,
        ApiResponse<ProfileResponse>,
        EmptyData,
        ApiResponse<EmptyData>,
        UuidIdData,
        ApiResponse<UuidIdData>,
        Role,
        CreateRoleRequest,
        UpdateRoleRequest,
        Permission,
        AssignRoleRequest,
        UserRoleAssignmentResponse,
        ApiResponse<Vec<Role>>,
        ApiResponse<Role>,
        ApiResponse<Vec<Permission>>,
        ApiResponse<std::collections::HashMap<String, Vec<Permission>>>,
        ApiResponse<Vec<UserRoleAssignmentResponse>>,
        ApiResponse<Vec<String>>,
        OrganizationUnit,
        CreateOrganizationUnitRequest,
        UpdateOrganizationUnitRequest,
        OrganizationPermissionGrantInput,
        UpdateOrganizationPermissionsRequest,
        OrganizationPermissionGrant,
        ApiResponse<Vec<OrganizationUnit>>,
        ApiResponse<OrganizationUnit>,
        ApiResponse<Vec<OrganizationPermissionGrant>>,
        DelegatablePermission,
        DelegationItem,
        CreateDelegationRequest,
        DelegationIdData,
        OrganizationMemberItem,
        ListMembersQuery,
        AddMemberRequest,
        UpdateMemberRequest,
        ApiResponse<Vec<DelegatablePermission>>,
        ApiResponse<Vec<DelegationItem>>,
        ApiResponse<DelegationIdData>,
        ApiResponse<Vec<OrganizationMemberItem>>,
        LookupItem,
        StaffLookupItem,
        RoleLookupItem,
        OrganizationUnitLookupItem,
        GradeLevelLookupItem,
        HomeroomLookupItem,
        AcademicYearLookupItem,
        StudentLookupItem,
        Room,
        ApiResponse<Vec<LookupItem>>,
        ApiResponse<Vec<StaffLookupItem>>,
        ApiResponse<Vec<RoleLookupItem>>,
        ApiResponse<Vec<OrganizationUnitLookupItem>>,
        ApiResponse<OrganizationUnitLookupItem>,
        CertificateCampaignStatus,
        CertificateCampaignCapabilities,
        CertificateCampaignSummary,
        CertificateCampaignDetail,
        CertificateCampaignListQuery,
        CertificateCampaignPurgeCounts,
        StartCertificateCampaignPurgeRequest,
        CertificateCampaignPurgeImpact,
        CertificateCampaignPurgePhase,
        CertificateCampaignPurgeStatus,
        CreateCertificateCampaignRequest,
        NullableUuidUpdate,
        UpdateCertificateCampaignRequest,
        ChangeCertificateCampaignStatusRequest,
        ApiResponse<Vec<CertificateCampaignSummary>>,
        ApiResponse<CertificateCampaignDetail>,
        ApiResponse<CertificateCampaignPurgeImpact>,
        ApiResponse<CertificateCampaignPurgeStatus>,
        RecipientType,
        CertificateTemplateAssetKind,
        GeometryAction,
        CertificatePreviewKind,
        CertificateTemplateDeleteDisposition,
        CertificateLayoutV1,
        CertificateElement,
        CertificateFontSource,
        SchoolFontStyle,
        SchoolFontUploadStatus,
        SchoolFontSummary,
        SchoolFontListResponse,
        InspectSchoolFontUploadsRequest,
        AttachSchoolFontBatchRequest,
        SchoolFontUploadInspectionFile,
        SchoolFontUploadInspection,
        SchoolFontDeleteConflict,
        ElementFrame,
        TextElement,
        TextAlignment,
        TextShadow,
        ImageElement,
        QrElement,
        CreateCertificateTemplateRequest,
        UpdateCertificateTemplateRequest,
        AttachCertificateBackgroundRequest,
        AttachCertificateAssetRequest,
        CertificatePageBox,
        CertificatePageGeometry,
        CertificateTemplateCapabilities,
        CertificateTemplateAsset,
        CertificateTemplateDetail,
        CertificateTemplateDeleteResult,
        CertificateTemplateVariableCatalog,
        CertificatePreviewManifestRequest,
        CertificateRenderFileGrant,
        CertificateBuiltInFont,
        CertificateRenderFontGrant,
        CertificateRenderImageGrant,
        CertificateRenderCampaignValues,
        CertificateRenderManifest,
        ApiResponse<Vec<CertificateTemplateDetail>>,
        ApiResponse<CertificateTemplateDetail>,
        ApiResponse<SchoolFontListResponse>,
        ApiResponse<SchoolFontUploadInspection>,
        ApiErrorResponseWithData<SchoolFontDeleteConflict>,
        ApiResponse<CertificateTemplateDeleteResult>,
        ApiResponse<CertificateTemplateVariableCatalog>,
        ApiResponse<CertificateRenderManifest>,
        CertificateImportSource,
        CandidateMatchStatus,
        CandidateValidationStatus,
        CandidateNameSource,
        CandidateValidationCode,
        CertificateImportRequest,
        CertificateImportRowInput,
        CertificateCandidateListQuery,
        CertificateAccountSearchQuery,
        CertificateCandidateCapabilities,
        CertificateCandidateDetail,
        CertificateCandidateSummary,
        CertificateCandidateListResponse,
        CertificateImportBatchSummary,
        CertificateCandidateImportResult,
        CertificateCandidateAccount,
        CreateManualExternalCandidateRequest,
        CreateAccountCertificateCandidateRequest,
        UpdateCertificateCandidateRequest,
        CertificateCandidateBulkRequest,
        CertificateCandidateBulkResult,
        ApiResponse<CertificateCandidateDetail>,
        ApiResponse<CertificateCandidateListResponse>,
        ApiResponse<CertificateCandidateImportResult>,
        ApiResponse<Vec<CertificateCandidateAccount>>,
        ApiResponse<CertificateCandidateBulkResult>,
        CertificateIssueRequestStatus,
        CertificateIssueCode,
        CertificateResourceLockCode,
        SubmitCertificateIssueRequest,
        ReturnCertificateIssueRequest,
        CertificateIssueRequestListQuery,
        CertificateIssueRequestCapabilities,
        CertificateIssueRequestSummary,
        CertificateIssueRequestItem,
        CertificateIssueRequestDetail,
        CertificateResourceLocked,
        ApiResponse<Vec<CertificateIssueRequestSummary>>,
        ApiResponse<CertificateIssueRequestDetail>,
        IssueCertificateRequest,
        CertificateStatus,
        CertificateCapabilities,
        IssuedCertificateListQuery,
        IssuedCertificateSummary,
        IssuedCertificateDetail,
        CertificateIssueCandidateProblem,
        IssueCertificateOutcome,
        RevokeCertificateRequest,
        CertificateReplacementCandidate,
        RevokeCertificateResult,
        CertificateRenderManifestBatchRequest,
        ManualCertificateVerificationRequest,
        QrCertificateVerificationRequest,
        PublicCertificateRenderRequest,
        PublicCertificateVerificationData,
        ApiResponse<IssueCertificateOutcome>,
        ApiResponse<Vec<IssuedCertificateSummary>>,
        ApiResponse<IssuedCertificateDetail>,
        ApiResponse<RevokeCertificateResult>,
        ApiResponse<Vec<CertificateRenderManifest>>,
        ApiResponse<PublicCertificateVerificationData>,
        ApiErrorResponseWithData<CertificateResourceLocked>,
        ApiErrorResponseWithOptionalData<CertificateResourceLocked>,
        ApiResponse<Vec<GradeLevelLookupItem>>,
        ApiResponse<Vec<HomeroomLookupItem>>,
        ApiResponse<Vec<AcademicYearLookupItem>>,
        ApiResponse<Vec<StudentLookupItem>>,
        ApiResponse<Vec<Room>>,
        MenuItemResponse,
        MenuGroupResponse,
        UserMenuData,
        ApiResponse<UserMenuData>,
        MenuGroup,
        MenuItem,
        MenuWorkspace,
        CreateMenuWorkspaceRequest,
        UpdateMenuWorkspaceRequest,
        CreateMenuGroupRequest,
        UpdateMenuGroupRequest,
        CreateMenuItemRequest,
        UpdateMenuItemRequest,
        ReorderItem,
        ReorderRequest,
        ReorderGroupsRequest,
        ReorderWorkspacesRequest,
        MoveItemToGroupRequest,
        MovedCountData,
        AcademicMenuTemplateSection,
        AcademicMenuTemplateMove,
        AcademicMenuTemplatePreview,
        AcademicMenuTemplateApplyResult,
        ApplyAcademicMenuTemplateRequest,
        ApiResponse<MenuWorkspace>,
        ApiResponse<Vec<MenuWorkspace>>,
        ApiResponse<MovedCountData>,
        ApiResponse<Vec<MenuGroup>>,
        ApiResponse<Vec<MenuItem>>,
        ApiResponse<AcademicMenuTemplatePreview>,
        ApiResponse<AcademicMenuTemplateApplyResult>,
        FeatureToggle,
        FeatureListResponse,
        FeatureToggleResponse,
        StaffListItem,
        StaffListData,
        StaffDashboardOverview,
        RoleResponse,
        OrganizationUnitResponse,
        TeachingAssignmentItem,
        AdvisorHomeroomItem,
        StaffInfoResponse,
        StaffProfileResponse,
        CreateStaffInfoRequest,
        CreateStaffRequest,
        OrganizationAssignment,
        UpdateStaffRequest,
        PublicStaffRole,
        PublicStaffOrganizationUnit,
        PublicStaffProfile,
        ApiResponse<StaffListData>,
        ApiResponse<StaffDashboardOverview>,
        ApiResponse<StaffProfileResponse>,
        ApiResponse<PublicStaffProfile>,
        ParentDto,
        StudentDbRow,
        StudentProfile,
        StudentListItem,
        StudentListResponse,
        UpdateOwnProfileRequest,
        CreateStudentRequest,
        CreateParentRequest,
        UpdateStudentRequest,
        CreateStudentResponse,
        Achievement,
        AchievementListFilter,
        CreateAchievementRequest,
        UpdateAchievementRequest,
        ApiResponse<Vec<Achievement>>,
        ApiResponse<Achievement>,
        ChildDto,
        ParentProfile,
        ApiResponse<StudentProfile>,
        ApiResponse<StudentListResponse>,
        ApiResponse<ParentProfile>,
        AcademicYearStatus,
        AcademicTermStatus,
        AcademicTermType,
        VersionStatus,
        StudentAcademicYearStatus,
        GradeProgressionKind,
        CreateAcademicYearRequest,
        UpdateAcademicYearRequest,
        CreateAcademicTermRequest,
        UpdateAcademicTermRequest,
        BellSchedulePeriodInput,
        ReplaceBellSchedulePeriodsRequest,
        GradeProgressionInput,
        ReplaceGradeProgressionsRequest,
        AcademicYear,
        AcademicTerm,
        AcademicYearOption,
        AcademicTermOption,
        AcademicContextOptions,
        CreateBellScheduleRequest,
        UpdateBellScheduleRequest,
        BellSchedule,
        BellSchedulePeriod,
        GradeProgression,
        GradeProgressionSet,
        CatalogDisplayState,
        CatalogOwnerOption,
        CatalogSubject,
        CatalogSubjectOverviewItem,
        CatalogSubjectOverview,
        CreateCatalogSubjectRequest,
        UpdateCatalogSubjectRequest,
        SubjectVersion,
        CreateSubjectVersionRequest,
        UpdateSubjectVersionRequest,
        CatalogActivity,
        CatalogActivityOverviewItem,
        CatalogActivityOverview,
        CreateCatalogActivityRequest,
        UpdateCatalogActivityRequest,
        ActivityVersion,
        CreateActivityVersionRequest,
        UpdateActivityVersionRequest,
        DefaultTeacher,
        ReplaceDefaultTeachersRequest,
        SubjectGroup,
        CreateSubjectGroupRequest,
        UpdateSubjectGroupRequest,
        Curriculum,
        CurriculumDisplayState,
        CurriculumOverviewItem,
        CurriculumOverview,
        CreateCurriculumRequest,
        UpdateCurriculumRequest,
        CurriculumVersion,
        CurriculumVersionView,
        CreateCurriculumVersionRequest,
        UpdateCurriculumVersionRequest,
        StudyProgram,
        StudyProgramOption,
        CreateStudyProgramRequest,
        UpdateStudyProgramRequest,
        RequirementKind,
        RequirementResourceKind,
        CurriculumCatalogVersionOption,
        CurriculumCreateOptions,
        CurriculumManagementOptions,
        CurriculumTermSlot,
        CatalogWeeklyUnit,
        CurriculumDocumentSection,
        CatalogCurriculumMetrics,
        CurriculumStructureRequirement,
        CurriculumValidationNotice,
        CurriculumStructureValidation,
        CurriculumStructureWorkspace,
        CurriculumTermSlotInput,
        ReplaceCurriculumTermSlotsRequest,
        CurriculumStructureRequirementInput,
        ReplaceCurriculumStructureRequest,
        AcademicSetupWorkspace,
        PublishVersionRequest,
        Homeroom,
        CreateHomeroomRequest,
        UpdateHomeroomRequest,
        HomeroomAdvisor,
        HomeroomAdvisorAssignment,
        HomeroomAdvisorInput,
        ReplaceHomeroomAdvisorsRequest,
        StudentAcademicYear,
        StudentYearCandidate,
        CreateStudentAcademicYearRequest,
        UpdateStudentAcademicYearRequest,
        StudentAcademicYearFilter,
        HomeroomPlacementStatus,
        HomeroomPlacement,
        CreateHomeroomPlacementRequest,
        TransferHomeroomPlacementRequest,
        HomeroomPlacementTransfer,
        ApiResponse<AcademicContextOptions>,
        ApiResponse<Vec<AcademicYear>>,
        ApiResponse<AcademicYear>,
        ApiResponse<Vec<AcademicTerm>>,
        ApiResponse<AcademicTerm>,
        ApiResponse<Vec<BellSchedule>>,
        ApiResponse<BellSchedule>,
        ApiResponse<Vec<BellSchedulePeriod>>,
        ApiResponse<GradeProgressionSet>,
        ApiResponse<Vec<CatalogSubject>>,
        ApiResponse<CatalogSubjectOverview>,
        ApiResponse<CatalogSubject>,
        ApiResponse<Vec<SubjectVersion>>,
        ApiResponse<SubjectVersion>,
        ApiResponse<Vec<DefaultTeacher>>,
        ApiResponse<Vec<SubjectGroup>>,
        ApiResponse<SubjectGroup>,
        ApiResponse<Vec<CatalogActivity>>,
        ApiResponse<CatalogActivityOverview>,
        ApiResponse<CatalogActivity>,
        ApiResponse<Vec<ActivityVersion>>,
        ApiResponse<ActivityVersion>,
        ApiResponse<Vec<Curriculum>>,
        ApiResponse<CurriculumOverview>,
        ApiResponse<CurriculumCreateOptions>,
        ApiResponse<CurriculumManagementOptions>,
        ApiResponse<Curriculum>,
        ApiResponse<Vec<CurriculumVersion>>,
        ApiResponse<CurriculumVersion>,
        ApiResponse<Vec<StudyProgram>>,
        ApiResponse<Vec<StudyProgramOption>>,
        ApiResponse<StudyProgram>,
        ApiResponse<CurriculumStructureWorkspace>,
        ApiResponse<AcademicSetupWorkspace>,
        ApiResponse<Vec<Homeroom>>,
        ApiResponse<Homeroom>,
        ApiResponse<Vec<HomeroomAdvisor>>,
        ApiResponse<Vec<HomeroomAdvisorAssignment>>,
        ApiResponse<Vec<StudentAcademicYear>>,
        ApiResponse<StudentAcademicYear>,
        ApiResponse<Vec<StudentYearCandidate>>,
        ApiResponse<Vec<HomeroomPlacement>>,
        ApiResponse<HomeroomPlacement>,
        ApiResponse<HomeroomPlacementTransfer>,
        SaveAssessmentPlanRequest,
        SaveAssessmentCategoryRequest,
        SaveAssessmentItemRequest,
        AssessmentPlanSummary,
        AssessmentPlanDetail,
        AssessmentCategory,
        AssessmentItem,
        AssessmentSettingsResponse,
        UpdateAssessmentSettingsRequest,
        ApiResponse<Vec<AssessmentPlanSummary>>,
        ApiResponse<AssessmentPlanDetail>,
        ApiResponse<AssessmentSettingsResponse>,
        LearningOfferingKind,
        LearningOfferingStatus,
        OfferingTargetKind,
        ActivityRegistrationType,
        ActivitySchedulingMode,
        LearningTeacherRole,
        RosterStatus,
        MembershipStatus,
        RosterOverrideAction,
        CurriculumPreviewAction,
        CourseGradingPolicy,
        ActivityAttendanceRequirement,
        ActivityPassCriteria,
        OfferingTargetInput,
        CreateCourseOfferingRequest,
        CreateActivityOfferingRequest,
        CreateLearningOfferingRequest,
        UpdateLearningOfferingRequest,
        PublishLearningOfferingRequest,
        LearningOfferingQuery,
        HomeroomDeliveryQuery,
        LearningGroupTermQuery,
        PreviewCurriculumOfferingsRequest,
        ApplyCurriculumOfferingsRequest,
        LearningOfferingTarget,
        CourseOfferingSnapshot,
        ActivityOfferingSnapshot,
        LearningOfferingSnapshot,
        LearningOffering,
        LearningOfferingOverviewItem,
        LearningDeliveryOverview,
        HomeroomOfferingState,
        HomeroomGroupMode,
        HomeroomTeacherState,
        HomeroomTimetableState,
        DeliveryPrerequisite,
        UnlinkedDeliveryItem,
        HomeroomDeliveryGroupSummary,
        HomeroomDeliveryItem,
        HomeroomDeliveryRoom,
        HomeroomDeliveryWorkspace,
        DeliveryCatalogVersionOption,
        DeliveryManagementOptions,
        PreparationAction,
        PreparationGroupingState,
        CurriculumGroupProposal,
        PreparationConflict,
        CurriculumPreparationProposal,
        CurriculumPreparationChoice,
        CurriculumOfferingPreview,
        ApplyCurriculumOfferingsResult,
        CreateLearningGroupRequest,
        UpdateLearningGroupRequest,
        TeacherAssignmentInput,
        ReplaceLearningGroupTeachersRequest,
        ReplaceLearningGroupHomeroomsRequest,
        LearningGroupHomeroomIds,
        LearningGroup,
        RosterPreviewStudent,
        RosterPreview,
        RosterOverrideInput,
        ApplyRosterRequest,
        PublishRosterRequest,
        LearningGroupStudent,
        AcademicTermChangeSetStatus,
        AcademicTermChangeActionKind,
        AcademicTermChangeItem,
        AcademicTermChangeSet,
        CreateAcademicTermChangeSetRequest,
        UpdateAcademicTermChangeSetRequest,
        CancelAcademicTermChangeSetRequest,
        AcademicTermChangeSetQuery,
        UpsertAcademicTermChangeItemRequest,
        DeleteAcademicTermChangeItemRequest,
        AcademicChangeFindingSeverity,
        AcademicChangeFindingCode,
        AcademicChangeFinding,
        AcademicChangeImpactCounts,
        AcademicOfferingScheduleCount,
        AcademicTermChangeSetPreview,
        PublishAcademicTermChangeSetRequest,
        DatedRosterMembership,
        AddDatedRosterMembershipRequest,
        RemoveDatedRosterMembershipRequest,
        ActivityResult,
        StudentActivityRegistrationQuery,
        StudentActivityGroupOption,
        StudentActivityOfferingOption,
        StudentActivityRegistrationResult,
        ApiResponse<Vec<LearningOffering>>,
        ApiResponse<LearningOffering>,
        ApiResponse<LearningDeliveryOverview>,
        ApiResponse<HomeroomDeliveryWorkspace>,
        ApiResponse<DeliveryManagementOptions>,
        ApiResponse<CurriculumOfferingPreview>,
        ApiResponse<ApplyCurriculumOfferingsResult>,
        ApiResponse<Vec<LearningGroup>>,
        ApiResponse<LearningGroup>,
        ApiResponse<LearningGroupHomeroomIds>,
        ApiResponse<Vec<TeacherAssignmentInput>>,
        ApiResponse<RosterPreview>,
        ApiResponse<Vec<AcademicTermChangeSet>>,
        ApiResponse<AcademicTermChangeSet>,
        ApiResponse<AcademicTermChangeSetPreview>,
        ApiResponse<Vec<DatedRosterMembership>>,
        ApiResponse<DatedRosterMembership>,
        ApiResponse<Vec<StudentActivityOfferingOption>>,
        ApiResponse<StudentActivityRegistrationResult>,
        TimetableEntry,
        TimetableInstructor,
        CreateTimetableEntryRequest,
        UpdateTimetableEntryRequest,
        CreateBatchTimetableEntriesRequest,
        BatchTimetableResult,
        SwapTimetableEntriesRequest,
        SwapTimetableEntriesResponse,
        ValidateMovesRequest,
        MoveValidityCell,
        TimetableOccupancyCell,
        TimetableTemplate,
        TimetableTemplateEntry,
        TimetableTemplateTargetSelector,
        TemplateWithEntries,
        CreateTemplateRequest,
        UpdateTemplateRequest,
        FromCurrentRequest,
        ApplyTemplateRequest,
        ClearTimetableRequest,
        TemplateApplyResult,
        ApiResponse<TimetableEntry>,
        ApiResponse<Vec<TimetableEntry>>,
        ApiResponse<BatchTimetableResult>,
        ApiResponse<SwapTimetableEntriesResponse>,
        ApiResponse<Vec<MoveValidityCell>>,
        ApiResponse<Vec<TimetableOccupancyCell>>,
        TimetableVersionStatus,
        TimetableVersionDisplayState,
        TimetableVersionTarget,
        TimetableVersion,
        CloneTimetableVersionRequest,
        ApiResponse<Vec<TimetableVersion>>,
        ApiResponse<TimetableVersion>,
        ApiResponse<Vec<TimetableTemplate>>,
        ApiResponse<TimetableTemplate>,
        ApiResponse<TemplateWithEntries>,
        ApiResponse<TemplateApplyResult>,
        DailyTeachingOverview,
        DailyTeachingPeriod,
        DailyTeachingTeacher,
        DailyTeachingPeriodCell,
        DailyTeachingEntry,
        DailyTeachingSummary,
        ApiResponse<DailyTeachingOverview>,
        ExamRound,
        CreateExamRoundRequest,
        UpdateExamRoundRequest,
        ExamDay,
        UpsertExamDayRequest,
        BlockedWindow,
        BlockedWindowInput,
        UpsertDayRoomAssignmentRequest,
        UpdateExamInvigilatorsRequest,
        ImportExamItemsRequest,
        ImportExamItemsResult,
        ClearMismatchedExamItemsResult,
        DayRoomAssignmentView,
        InvigilatorView,
        ExamInvigilatorAssignmentSummary,
        ExamInvigilatorDayWorkload,
        ExamInvigilatorStaffWorkload,
        ExamInvigilatorWorkspace,
        ExamInvigilatorStaffOption,
        GenerateSeatsRequest,
        SeatAssignmentView,
        PlaceExamSessionRequest,
        ExamDayDetail,
        ExamDayRoomAssignmentView,
        ExamInvigilatorView,
        ExamScheduleItem,
        ExamScheduleItemView,
        ExamSession,
        ExamSessionView,
        ExamScheduleWorkspace,
        ExamScheduleReadiness,
        ApiResponse<ExamRound>,
        ApiResponse<Vec<ExamRound>>,
        ApiResponse<ExamScheduleWorkspace>,
        ApiResponse<ImportExamItemsResult>,
        ApiResponse<ClearMismatchedExamItemsResult>,
        ApiResponse<ExamDayDetail>,
        ApiResponse<Vec<DayRoomAssignmentView>>,
        ApiResponse<DayRoomAssignmentView>,
        ApiResponse<ExamInvigilatorWorkspace>,
        ApiResponse<Vec<ExamInvigilatorStaffOption>>,
        ApiResponse<Vec<SeatAssignmentView>>,
        ApiResponse<ExamSessionView>,
        PersonalExamScheduleRound,
        PersonalExamSessionView,
        ApiResponse<Vec<PersonalExamScheduleRound>>,
        StaffPublishedExamScheduleRound,
        StaffPublishedExamDay,
        StaffPublishedExamSession,
        StaffPublishedExamRoomAssignment,
        StaffPublishedExamInvigilator,
        ApiResponse<Vec<StaffPublishedExamScheduleRound>>,
        CalendarAudienceType,
        CalendarVisibility,
        CalendarEventTag,
        CalendarViewerEvent,
        ApiResponse<Vec<CalendarViewerEvent>>,
        CalendarCategory,
        CalendarTag,
        CalendarEventTarget,
        CalendarEventReminder,
        CalendarEvent,
        CalendarPublicEvent,
        ApiResponse<Vec<CalendarCategory>>,
        ApiResponse<Vec<CalendarTag>>,
        ApiResponse<Vec<CalendarEvent>>,
        ApiResponse<Vec<CalendarPublicEvent>>,
        SchoolSettingsResponse,
        PublicSchoolInfoData,
        ApiResponse<SchoolSettingsResponse>,
        ApiResponse<PublicSchoolInfoData>,
        FilePurpose,
        FileLifecycleStatus,
        FileUploadMultipart,
        FileMetadata,
        FileDeleteResult,
        FileDownloadGrantResponse,
        PublicFileDeliveryResponse,
        ApiResponse<FileMetadata>,
        ApiResponse<FileDeleteResult>,
        ApiResponse<FileDownloadGrantResponse>,
        ApiResponse<PublicFileDeliveryResponse>,
        StaffDocumentMultipart,
        PortalDocumentMultipart,
        PortalCredentials,
        PortalUploadDocumentData,
        DocumentUploadResponse,
        ApiResponse<PortalUploadDocumentData>,
        ApiResponse<DocumentUploadResponse>,
        SupervisionCycleStatus,
        SupervisionTemplateStatus,
        SupervisionTargetType,
        SupervisionTemplateItemType,
        SupervisionTemplateStepActorKind,
        SupervisionTemplateStepActionKind,
        SupervisionObservationStatus,
        SupervisionEvaluatorStatus,
        LessonSnapshot,
        SupervisionCycleTarget,
        SupervisionCycle,
        SupervisionTemplateItem,
        SupervisionTemplateSection,
        SupervisionTemplateStep,
        SupervisionTemplate,
        SupervisionEvaluator,
        SupervisionEvaluatorConflict,
        SupervisionEvaluatorAvailability,
        SupervisionAction,
        SupervisionObservation,
        SupervisionReviewResponse,
        SupervisionReviewEvaluatorResult,
        SupervisionReviewItemSummary,
        SupervisionObservationReview,
        ManualLesson,
        CreateSupervisionCycleTargetRequest,
        CreateSupervisionCycleRequest,
        UpdateSupervisionCycleRequest,
        SupervisionCycleQuery,
        CreateSupervisionTemplateItemRequest,
        CreateSupervisionTemplateSectionRequest,
        CreateSupervisionTemplateStepRequest,
        CreateSupervisionTemplateRequest,
        UpdateSupervisionTemplateRequest,
        ManualLessonInput,
        RequestSupervisionObservationRequest,
        UpdateRequestedObservationRequest,
        UpdateSupervisionObservationRequest,
        ReplaceObservationEvaluatorsRequest,
        CancelObservationRequest,
        EvaluatorAssignmentInput,
        ApproveObservationRequest,
        ReturnObservationRequest,
        EvaluationResponseInput,
        SaveEvaluationRequest,
        AcknowledgeObservationRequest,
        ListObservationsQuery,
        SupervisionCycleProgress,
        SupervisionTeacherStatusRow,
        ItemsData<SupervisionCycle>,
        ItemsData<SupervisionTemplate>,
        ItemsData<SupervisionObservation>,
        ItemsData<SupervisionEvaluatorAvailability>,
        ItemsData<TimetableEntry>,
        ItemsData<SupervisionTeacherStatusRow>,
        ApiResponse<ItemsData<SupervisionCycle>>,
        ApiResponse<ItemsData<SupervisionTemplate>>,
        ApiResponse<ItemsData<SupervisionObservation>>,
        ApiResponse<SupervisionCycle>,
        ApiResponse<SupervisionTemplate>,
        ApiResponse<SupervisionObservation>,
        ApiResponse<SupervisionObservationReview>,
        ApiResponse<ItemsData<SupervisionEvaluatorAvailability>>,
        ApiResponse<ItemsData<TimetableEntry>>,
        ApiResponse<SupervisionCycleProgress>,
        ApiResponse<ItemsData<SupervisionTeacherStatusRow>>,
        RichContent,
        RichDocument,
        RichBlockNode,
        RichInlineNode,
        RichTextMark,
        MathNodeAttributes,
        ImageNodeAttributes,
        ImageAlignment,
        QuestionBankListQuery,
        UpsertQuestionRequest,
        UpsertQuestionChoiceRequest,
        QuestionChoice,
        QuestionSummary,
        QuestionDetail,
        QuestionFile,
        QuestionBankSummary,
        QuestionBankPage,
        QuestionBankSubjectOption,
        QuestionBankOptions,
        QuestionBankExportDataRequest,
        ApiResponse<QuestionBankOptions>,
        ApiResponse<QuestionBankPage>,
        ApiResponse<QuestionDetail>,
        ApiResponse<Vec<QuestionDetail>>,
        Notification,
        ListNotificationsResponse,
        ApiResponse<ListNotificationsResponse>,
        ApiErrorResponse
    )),
    tags(
        (name = "auth", description = "Authentication and current-user operations"),
        (name = "roles", description = "Role assignment and role administration"),
        (name = "permissions", description = "Permission discovery and effective permissions"),
        (name = "organization", description = "Organization units and scoped access"),
        (name = "lookup", description = "Authenticated reference-data lookups"),
        (name = "menu", description = "User and administrator menu reads"),
        (name = "system", description = "System feature reads"),
        (name = "staff", description = "Staff directory and profiles"),
        (name = "student", description = "Student self-service reads"),
        (name = "parent", description = "Parent self-service reads"),
        (name = "academic", description = "Academic structure administration and self-service reads"),
        (name = "calendar", description = "Calendar reads"),
        (name = "supervision", description = "Teaching supervision workflows and reports"),
        (name = "question-bank", description = "Authorized question bank and export operations"),
        (name = "school", description = "School settings and public branding reads"),
        (name = "files", description = "Authorized provider-neutral file operations"),
        (name = "admission", description = "Admission document attachment operations"),
        (name = "notifications", description = "Current-user notification reads"),
        (name = "achievement", description = "Scoped staff achievement operations")
    )
)]
struct SchoolApiDoc;

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries = std::mem::take(map).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut entries {
                sort_json(child);
            }
            map.extend(entries);
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

pub fn school_api_value() -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(SchoolApiDoc::openapi())?;
    sort_json(&mut value);
    Ok(value)
}

pub fn render_school_api() -> Result<String, serde_json::Error> {
    let mut output = serde_json::to_string_pretty(&school_api_value()?)?;
    output.push('\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{render_school_api, school_api_value};
    use serde_json::Value;
    use std::collections::{BTreeSet, HashSet};

    fn required(schema: &Value) -> Vec<&str> {
        let mut fields = schema["required"]
            .as_array()
            .expect("required must be an array")
            .iter()
            .map(|value| value.as_str().expect("required entry must be a string"))
            .collect::<Vec<_>>();
        fields.sort_unstable();
        fields
    }

    fn contains_null(schema: &Value) -> bool {
        match schema {
            Value::String(value) => value == "null",
            Value::Array(values) => values.iter().any(contains_null),
            Value::Object(values) => values.values().any(contains_null),
            _ => false,
        }
    }

    fn assert_operations(document: &Value, expected: &[(&str, &str, &str)]) {
        for (path, method, operation_id) in expected {
            assert_eq!(
                document["paths"][path][method]["operationId"], *operation_id,
                "missing or incorrect {method} {path}"
            );
        }
    }

    fn query_contract(document: &Value, path: &str, method: &str) -> BTreeSet<(String, bool)> {
        document["paths"][path][method]["parameters"]
            .as_array()
            .expect("operation parameters must be an array")
            .iter()
            .filter(|parameter| parameter["in"] == "query")
            .map(|parameter| {
                (
                    parameter["name"].as_str().expect("query name").to_string(),
                    parameter["required"].as_bool().unwrap_or(false),
                )
            })
            .collect()
    }

    #[test]
    fn documents_current_user_operation_and_envelopes() {
        let document = school_api_value().expect("document should serialize");
        let operation = &document["paths"]["/api/auth/me"]["get"];
        let success_response =
            &operation["responses"]["200"]["content"]["application/json"]["schema"];
        let error_response =
            &operation["responses"]["401"]["content"]["application/json"]["schema"];

        assert_eq!(operation["operationId"], "getCurrentUser");
        assert_eq!(
            success_response["$ref"],
            "#/components/schemas/ApiResponse_CurrentUserResponse"
        );
        assert_eq!(
            error_response["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );

        let success_schema = &document["components"]["schemas"]["ApiResponse_CurrentUserResponse"];
        assert_eq!(required(success_schema), vec!["data", "success"]);
        assert_eq!(success_schema["properties"]["success"]["type"], "boolean");
        assert_eq!(
            success_schema["properties"]["data"],
            document["components"]["schemas"]["CurrentUserResponse"]
        );

        let error_schema = &document["components"]["schemas"]["ApiErrorResponse"];
        assert_eq!(required(error_schema), vec!["error", "success"]);
        assert_eq!(error_schema["properties"]["success"]["type"], "boolean");
        assert_eq!(error_schema["properties"]["error"]["type"], "string");
    }

    #[test]
    fn current_user_schema_matches_serde() {
        let document = school_api_value().expect("document should serialize");
        let schema = &document["components"]["schemas"]["CurrentUserResponse"];

        assert_eq!(
            required(schema),
            vec![
                "firstName",
                "id",
                "lastName",
                "permissions",
                "profileImageFileId",
                "status",
                "userType",
                "username",
            ]
        );

        let properties = schema["properties"]
            .as_object()
            .expect("properties must exist");
        assert_eq!(properties["id"]["format"], "uuid");
        assert!(contains_null(&properties["profileImageFileId"]));
        for forbidden in ["nationalId", "email", "phone", "createdAt"] {
            assert!(properties.get(forbidden).is_none());
        }
        assert!(document["components"]["schemas"]["UserResponse"].is_null());
    }

    #[test]
    fn registers_session_components_and_paths_after_cutover() {
        let document = school_api_value().expect("document should serialize");
        let schemas = &document["components"]["schemas"];

        for schema in [
            "CurrentUserResponse",
            "LoginData",
            "SessionResponse",
            "SessionListData",
            "ApiResponse_LoginData",
            "ApiResponse_CurrentUserResponse",
            "ApiResponse_SessionListData",
        ] {
            assert!(!schemas[schema].is_null(), "missing schema {schema}");
        }

        let current_user = &schemas["CurrentUserResponse"];
        assert_eq!(
            required(current_user),
            vec![
                "firstName",
                "id",
                "lastName",
                "permissions",
                "profileImageFileId",
                "status",
                "userType",
                "username",
            ]
        );
        for forbidden in [
            "nationalId",
            "email",
            "phone",
            "dateOfBirth",
            "address",
            "createdAt",
        ] {
            assert!(
                current_user["properties"][forbidden].is_null(),
                "unexpected current-user field {forbidden}"
            );
        }
        assert_eq!(
            required(&schemas["SessionResponse"]),
            vec![
                "absoluteExpiresAt",
                "createdAt",
                "deviceLabel",
                "id",
                "idleExpiresAt",
                "isCurrent",
                "lastSeenAt",
                "rememberMe",
            ]
        );
        assert_eq!(required(&schemas["LoginData"]), vec!["user"]);
        assert_eq!(required(&schemas["SessionListData"]), vec!["sessions"]);
        assert!(schemas["LoginData"]["properties"]["token"].is_null());
        assert!(schemas["LoginData"]["properties"]["csrfToken"].is_null());

        assert!(!document["paths"]["/api/auth/sessions"]["get"].is_null());
        assert!(!document["paths"]["/api/auth/sessions/{id}"]["delete"].is_null());
        assert!(!document["paths"]["/api/auth/logout-all"]["post"].is_null());
        assert_eq!(
            document["paths"]["/api/auth/me"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_CurrentUserResponse"
        );
        assert!(schemas["UserResponse"].is_null());
    }

    #[test]
    fn render_is_deterministic_and_newline_terminated() {
        let first = render_school_api().expect("first render");
        let second = render_school_api().expect("second render");

        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
    }

    #[test]
    fn documents_shared_empty_and_uuid_identifier_envelopes() {
        let document = school_api_value().expect("document should serialize");
        let schemas = &document["components"]["schemas"];

        let empty_envelope = &schemas["ApiResponse_EmptyData"];
        assert_eq!(required(empty_envelope), vec!["data", "success"]);
        assert_eq!(empty_envelope["properties"]["data"], schemas["EmptyData"]);
        assert_eq!(
            schemas["EmptyData"]["type"], "object",
            "empty responses must generate an object DTO"
        );

        let id_envelope = &schemas["ApiResponse_UuidIdData"];
        assert_eq!(required(id_envelope), vec!["data", "success"]);
        assert_eq!(required(&schemas["UuidIdData"]), vec!["id"]);
        assert_eq!(schemas["UuidIdData"]["properties"]["id"]["format"], "uuid");
    }

    #[test]
    fn documents_auth_operations_and_transport_shapes() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/auth/login", "post", "login"),
                ("/api/auth/logout", "post", "logout"),
                ("/api/auth/me", "get", "getCurrentUser"),
                ("/api/auth/me/profile", "get", "getCurrentUserProfile"),
                ("/api/auth/me/profile", "put", "updateCurrentUserProfile"),
                (
                    "/api/auth/me/change-password",
                    "post",
                    "changeCurrentUserPassword",
                ),
                ("/api/auth/sessions", "get", "listAuthSessions"),
                ("/api/auth/sessions/{id}", "delete", "revokeAuthSession"),
                ("/api/auth/logout-all", "post", "logoutAllSessions"),
            ],
        );

        let schemas = &document["components"]["schemas"];
        let login = &schemas["LoginRequest"];
        assert_eq!(required(login), vec!["password", "username"]);
        assert!(login["properties"].get("rememberMe").is_some());
        assert!(login["properties"].get("remember_me").is_none());
        assert_eq!(
            document["paths"]["/api/auth/login"]["post"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_LoginData"
        );
        assert_eq!(
            document["paths"]["/api/auth/login"]["post"]["responses"]["400"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert!(document["paths"]["/api/auth/login"]["post"]["responses"]["422"].is_null());

        let profile = &schemas["ProfileResponse"];
        for field in [
            "nationalId",
            "title",
            "nickname",
            "email",
            "phone",
            "emergencyContact",
            "lineId",
            "dateOfBirth",
            "gender",
            "address",
            "profileImageFileId",
            "hiredDate",
        ] {
            assert!(
                required(profile).contains(&field),
                "{field} must be required"
            );
            assert!(
                contains_null(&profile["properties"][field]),
                "{field} must accept null"
            );
        }
        assert!(!required(profile).contains(&"primaryRoleName"));
        assert!(!contains_null(&profile["properties"]["primaryRoleName"]));

        let update = &schemas["UpdateProfileRequest"]["properties"];
        assert!(update.get("emergencyContact").is_some());
        assert!(update.get("dateOfBirth").is_some());
        assert!(update.get("profileImageFileId").is_some());
        let change = &schemas["ChangePasswordRequest"];
        assert_eq!(required(change), vec!["currentPassword", "newPassword"]);
        assert_eq!(
            document["paths"]["/api/auth/me/change-password"]["post"]["responses"]["400"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert!(
            document["paths"]["/api/auth/me/change-password"]["post"]["responses"]["404"].is_null()
        );
    }

    #[test]
    fn documents_role_permission_and_user_role_operations() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/roles", "get", "listRoles"),
                ("/api/roles/{id}", "get", "getRole"),
                ("/api/roles", "post", "createRole"),
                ("/api/roles/{id}", "put", "updateRole"),
                ("/api/roles/{id}", "delete", "deleteRole"),
                ("/api/permissions", "get", "listPermissions"),
                ("/api/permissions/modules", "get", "listPermissionsByModule"),
                ("/api/users/{id}/roles", "get", "getUserRoles"),
                ("/api/users/{id}/roles", "post", "assignUserRole"),
                (
                    "/api/users/{id}/roles/{role_id}",
                    "delete",
                    "removeUserRole",
                ),
                (
                    "/api/users/{id}/permissions",
                    "get",
                    "listUserEffectivePermissions",
                ),
            ],
        );

        let role_delete = &document["paths"]["/api/roles/{id}"]["delete"];
        assert_eq!(
            role_delete["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );
        for status in ["401", "403", "404", "409"] {
            assert_eq!(
                role_delete["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse"
            );
        }
        assert_eq!(
            document["paths"]["/api/roles"]["post"]["responses"]["201"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_UuidIdData"
        );
        assert_eq!(
            document["paths"]["/api/roles"]["post"]["responses"]["400"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert!(document["paths"]["/api/roles"]["post"]["responses"]["409"].is_null());
        assert_eq!(
            document["paths"]["/api/roles/{id}"]["put"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );
        assert!(document["paths"]["/api/roles/{id}"]["put"]["responses"]["409"].is_object());

        let include_inactive = document["paths"]["/api/roles"]["get"]["parameters"]
            .as_array()
            .expect("role list parameters")
            .iter()
            .find(|parameter| parameter["name"] == "include_inactive")
            .expect("include_inactive role query parameter");
        assert_eq!(include_inactive["in"], "query");
        assert_eq!(include_inactive["required"], false);
        assert_eq!(include_inactive["schema"]["type"], "boolean");

        let schemas = &document["components"]["schemas"];
        let role = &schemas["Role"];
        assert!(required(role).contains(&"is_system"));
        assert_eq!(role["properties"]["is_system"]["type"], "boolean");
        for field in ["name_en", "description"] {
            assert!(required(role).contains(&field));
            assert!(contains_null(&role["properties"][field]));
        }
        assert_eq!(schemas["Permission"]["properties"]["id"]["format"], "uuid");

        let assignment = &schemas["UserRoleAssignmentResponse"];
        for field in ["organization_unit_id", "ended_at", "notes"] {
            assert!(required(assignment).contains(&field));
            assert!(contains_null(&assignment["properties"][field]));
        }
        assert!(
            document["paths"]["/api/permissions/modules"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]
                .is_object()
        );
    }

    #[test]
    fn documents_people_staff_mutation_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/staff", "post", "createStaff"),
                ("/api/staff/{id}", "put", "updateStaff"),
                ("/api/staff/{id}", "delete", "deleteStaff"),
            ],
        );

        let create = &document["paths"]["/api/staff"]["post"];
        assert_eq!(
            create["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreateStaffRequest"
        );
        assert_eq!(
            create["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_UuidIdData"
        );
        for status in ["400", "401", "403"] {
            assert_eq!(
                create["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse"
            );
        }

        let update = &document["paths"]["/api/staff/{id}"]["put"];
        assert_eq!(
            update["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/UpdateStaffRequest"
        );
        assert_eq!(
            update["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );
        for status in ["400", "401", "403", "404"] {
            assert_eq!(
                update["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse"
            );
        }

        let delete = &document["paths"]["/api/staff/{id}"]["delete"];
        assert_eq!(
            delete["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );
        for status in ["401", "403", "404"] {
            assert_eq!(
                delete["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse"
            );
        }

        let id_parameter = update["parameters"]
            .as_array()
            .expect("staff update path parameters")
            .iter()
            .find(|parameter| parameter["name"] == "id")
            .expect("staff ID path parameter");
        assert_eq!(id_parameter["in"], "path");
        assert_eq!(id_parameter["required"], true);
        assert_eq!(id_parameter["schema"]["format"], "uuid");
    }

    #[test]
    fn people_student_mutation_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/student/profile", "put", "updateStudentProfile"),
                ("/api/students", "post", "createStudent"),
                ("/api/students/{id}", "put", "updateStudent"),
                ("/api/students/{id}", "delete", "deleteStudent"),
                ("/api/students/{id}/parents", "post", "addStudentParent"),
                (
                    "/api/students/{id}/parents/{parent_id}",
                    "delete",
                    "removeStudentParent",
                ),
            ],
        );

        let schemas = &document["components"]["schemas"];
        let create_request = &schemas["CreateStudentRequest"];
        assert!(required(create_request).contains(&"password"));
        assert!(!required(create_request).contains(&"parents"));
        assert!(create_request["properties"]["parents"]["type"]
            .as_array()
            .expect("optional parents must have nullable array types")
            .iter()
            .any(|value| value == "array"));
        assert_eq!(
            create_request["properties"]["parents"]["items"]["$ref"],
            "#/components/schemas/CreateParentRequest"
        );
        let national_id = &create_request["properties"]["national_id"];
        assert!(national_id.get("example").is_none());
        assert!(national_id.get("default").is_none());
        assert!(schemas["UpdateStudentRequest"]["properties"]
            .get("password")
            .is_none());
        assert!(schemas["UpdateOwnProfileRequest"]["properties"]
            .get("password")
            .is_none());

        let create = &document["paths"]["/api/students"]["post"];
        assert_eq!(
            create["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_CreateStudentResponse"
        );
        for (path, method) in [
            ("/api/student/profile", "put"),
            ("/api/students/{id}", "put"),
            ("/api/students/{id}", "delete"),
            ("/api/students/{id}/parents", "post"),
            ("/api/students/{id}/parents/{parent_id}", "delete"),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiResponse_EmptyData",
                "incorrect success envelope for {method} {path}"
            );
        }
    }

    #[test]
    fn people_achievement_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/achievements", "get", "listAchievements"),
                ("/api/achievements", "post", "createAchievement"),
                ("/api/achievements/{id}", "put", "updateAchievement"),
                ("/api/achievements/{id}", "delete", "deleteAchievement"),
            ],
        );

        let list = &document["paths"]["/api/achievements"]["get"];
        let parameters = list["parameters"]
            .as_array()
            .expect("achievement filters must be query parameters");
        let user_id = parameters
            .iter()
            .find(|parameter| parameter["name"] == "user_id")
            .expect("achievement user filter");
        assert_eq!(user_id["in"], "query");
        assert_eq!(user_id["schema"]["format"], "uuid");
        for name in ["start_date", "end_date"] {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter["name"] == name)
                .unwrap_or_else(|| panic!("missing achievement {name} filter"));
            assert_eq!(parameter["in"], "query");
            assert_eq!(parameter["schema"]["format"], "date");
        }
        assert_eq!(
            list["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_Achievement"
        );

        let create = &document["paths"]["/api/achievements"]["post"];
        assert_eq!(
            create["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Achievement"
        );
        let update = &document["paths"]["/api/achievements/{id}"]["put"];
        assert_eq!(
            update["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Achievement"
        );
        let delete = &document["paths"]["/api/achievements/{id}"]["delete"];
        assert_eq!(
            delete["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );
        for operation in [list, create, update, delete] {
            for status in ["401", "403"] {
                assert_eq!(
                    operation["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                    "#/components/schemas/ApiErrorResponse"
                );
            }
        }
        for operation in [update, delete] {
            assert_eq!(
                operation["responses"]["404"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse"
            );
        }

        let achievement = &document["components"]["schemas"]["Achievement"];
        for field in [
            "description",
            "image_file_id",
            "created_by",
            "user_first_name",
            "user_last_name",
            "user_profile_image_file_id",
        ] {
            assert!(required(achievement).contains(&field));
            assert!(contains_null(&achievement["properties"][field]));
        }
        assert_eq!(achievement["properties"]["id"]["format"], "uuid");
        assert_eq!(
            achievement["properties"]["achievement_date"]["format"],
            "date"
        );
    }

    #[test]
    fn academic_core_and_delivery_cutover_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/academic/context/options",
                    "get",
                    "listAcademicContextOptions",
                ),
                ("/api/academic/years", "get", "listAcademicYears"),
                ("/api/academic/years", "post", "createAcademicYear"),
                ("/api/academic/terms", "get", "listAcademicTerms"),
                ("/api/academic/bell-schedules", "get", "listBellSchedules"),
                (
                    "/api/academic/catalog/subjects",
                    "get",
                    "listCatalogSubjects",
                ),
                (
                    "/api/academic/catalog/subjects/overview",
                    "get",
                    "getCatalogSubjectOverview",
                ),
                (
                    "/api/academic/catalog/activities",
                    "get",
                    "listCatalogActivities",
                ),
                (
                    "/api/academic/catalog/activities/overview",
                    "get",
                    "getCatalogActivityOverview",
                ),
                ("/api/academic/curricula", "get", "listCurricula"),
                ("/api/academic/homerooms", "get", "listHomerooms"),
                (
                    "/api/academic/student-years",
                    "get",
                    "listStudentAcademicYears",
                ),
                (
                    "/api/academic/student-years/candidates",
                    "get",
                    "listStudentYearCandidates",
                ),
                (
                    "/api/academic/student-years/{id}/placements",
                    "get",
                    "listHomeroomPlacements",
                ),
                (
                    "/api/academic/placements/{id}/transfer",
                    "post",
                    "transferHomeroomPlacement",
                ),
                ("/api/academic/offerings", "get", "listLearningOfferings"),
                ("/api/academic/offerings", "post", "createLearningOffering"),
                (
                    "/api/academic/offerings/preview-from-curriculum",
                    "post",
                    "previewLearningOfferingsFromCurriculum",
                ),
                (
                    "/api/academic/learning-groups/{id}/roster",
                    "get",
                    "previewLearningGroupRoster",
                ),
                (
                    "/api/academic/learning-groups/{id}/roster/publish",
                    "post",
                    "publishLearningGroupRoster",
                ),
                (
                    "/api/me/activity-registrations",
                    "get",
                    "listMyActivityRegistrations",
                ),
                (
                    "/api/me/activity-registrations/{group_id}",
                    "post",
                    "enrollMyActivityRegistration",
                ),
                (
                    "/api/me/activity-registrations/{group_id}",
                    "delete",
                    "unenrollMyActivityRegistration",
                ),
            ],
        );

        for retired_path in [
            "/api/academic/structure",
            "/api/academic/semesters",
            "/api/academic/classrooms",
            "/api/academic/enrollments",
            "/api/academic/planning/courses",
            "/api/academic/subjects",
            "/api/academic/study-plans",
        ] {
            assert!(
                document["paths"][retired_path].is_null(),
                "retired path must stay removed: {retired_path}"
            );
        }

        let schemas = &document["components"]["schemas"];
        assert_eq!(
            required(&schemas["AcademicYear"]),
            vec![
                "createdAt",
                "endDate",
                "id",
                "migrated",
                "name",
                "rowVersion",
                "schoolDays",
                "startDate",
                "status",
                "updatedAt",
                "year",
            ]
        );
        assert_eq!(
            required(&schemas["AcademicTerm"]),
            vec![
                "academicYearId",
                "bellScheduleId",
                "blocksYearClosure",
                "closedOn",
                "code",
                "createdAt",
                "id",
                "includedInYearResult",
                "migrated",
                "name",
                "plannedEndDate",
                "rowVersion",
                "sequence",
                "startDate",
                "status",
                "termType",
                "updatedAt",
            ]
        );
        for retired_field in ["semesterId", "classroomCourseId", "isActive"] {
            assert!(schemas["AcademicYear"]["properties"][retired_field].is_null());
            assert!(schemas["AcademicTerm"]["properties"][retired_field].is_null());
        }
        for schema_name in ["AcademicTerm", "AcademicTermOption"] {
            let schema = &schemas[schema_name];
            assert!(required(schema).contains(&"plannedEndDate"));
            assert!(required(schema).contains(&"closedOn"));
            assert!(contains_null(&schema["properties"]["plannedEndDate"]));
            assert!(contains_null(&schema["properties"]["closedOn"]));
            assert!(schema["properties"]["endDate"].is_null());
        }
        for schema_name in ["CreateAcademicTermRequest", "UpdateAcademicTermRequest"] {
            let schema = &schemas[schema_name];
            assert!(contains_null(&schema["properties"]["plannedEndDate"]));
            assert!(schema["properties"]["endDate"].is_null());
        }

        let success = &document["paths"]["/api/academic/offerings"]["get"]["responses"]["200"]
            ["content"]["application/json"]["schema"]["$ref"];
        assert_eq!(
            success,
            "#/components/schemas/ApiResponse_Vec_LearningOffering"
        );
        assert_eq!(
            schemas["CourseGradingPolicy"]["properties"]["totalScore"]["type"],
            "string"
        );
        assert_eq!(
            schemas["CreateLearningOfferingRequest"]["oneOf"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn operational_academic_change_contract_is_typed_and_complete() {
        let document = school_api_value().expect("document should serialize");
        let operations = [
            (
                "/api/academic/term-change-sets",
                "get",
                "listAcademicTermChangeSets",
            ),
            (
                "/api/academic/term-change-sets",
                "post",
                "createAcademicTermChangeSet",
            ),
            (
                "/api/academic/term-change-sets/{id}",
                "get",
                "getAcademicTermChangeSet",
            ),
            (
                "/api/academic/term-change-sets/{id}",
                "patch",
                "updateAcademicTermChangeSet",
            ),
            (
                "/api/academic/term-change-sets/{id}/cancel",
                "post",
                "cancelAcademicTermChangeSet",
            ),
            (
                "/api/academic/term-change-sets/{id}/items",
                "put",
                "upsertAcademicTermChangeItem",
            ),
            (
                "/api/academic/term-change-sets/{id}/items/{itemId}",
                "delete",
                "deleteAcademicTermChangeItem",
            ),
            (
                "/api/academic/term-change-sets/{id}/preview",
                "get",
                "previewAcademicTermChangeSet",
            ),
            (
                "/api/academic/term-change-sets/{id}/publish",
                "post",
                "publishAcademicTermChangeSet",
            ),
            (
                "/api/academic/learning-groups/{id}/memberships",
                "get",
                "listDatedRosterMemberships",
            ),
            (
                "/api/academic/learning-groups/{id}/memberships",
                "post",
                "addDatedRosterMembership",
            ),
            (
                "/api/academic/learning-groups/{id}/memberships/{membershipId}/end",
                "post",
                "endDatedRosterMembership",
            ),
        ];
        assert_operations(&document, &operations);
        assert_eq!(
            query_contract(&document, "/api/academic/term-change-sets", "get"),
            BTreeSet::from([("academicTermId".to_string(), true)])
        );
        for (path, method, expected_names) in [
            (
                "/api/academic/term-change-sets/{id}/items/{itemId}",
                "delete",
                BTreeSet::from(["id", "itemId"]),
            ),
            (
                "/api/academic/learning-groups/{id}/memberships/{membershipId}/end",
                "post",
                BTreeSet::from(["id", "membershipId"]),
            ),
        ] {
            let names = document["paths"][path][method]["parameters"]
                .as_array()
                .expect("path parameters must be an array")
                .iter()
                .filter(|parameter| parameter["in"] == "path")
                .map(|parameter| parameter["name"].as_str().expect("path name"))
                .collect::<BTreeSet<_>>();
            assert_eq!(names, expected_names);
        }
        for (path, method, _) in operations {
            let operation = &document["paths"][path][method];
            for status in ["400", "403", "404", "409"] {
                assert_eq!(
                    operation["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                    "#/components/schemas/ApiErrorResponse",
                    "{method} {path} must document {status}"
                );
            }
        }
        assert_eq!(
            document["paths"]["/api/academic/term-change-sets"]["get"]["responses"]["200"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_AcademicTermChangeSet"
        );
        assert_eq!(
            document["paths"]["/api/academic/term-change-sets/{id}/preview"]["get"]["responses"]
                ["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_AcademicTermChangeSetPreview"
        );
        assert_eq!(
            document["paths"]["/api/academic/learning-groups/{id}/memberships"]["get"]["responses"]
                ["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_DatedRosterMembership"
        );
        let schemas = &document["components"]["schemas"];
        for schema_name in [
            "CreateAcademicTermChangeSetRequest",
            "UpdateAcademicTermChangeSetRequest",
            "CancelAcademicTermChangeSetRequest",
            "DeleteAcademicTermChangeItemRequest",
            "PublishAcademicTermChangeSetRequest",
            "AddDatedRosterMembershipRequest",
            "RemoveDatedRosterMembershipRequest",
        ] {
            assert_ne!(
                schemas[schema_name]["additionalProperties"],
                Value::Bool(true),
                "{schema_name} must not accept arbitrary fields"
            );
        }
    }

    #[test]
    fn documents_organization_unit_and_permission_grant_operations() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/organization/units", "get", "listOrganizationUnits"),
                ("/api/organization/units/{id}", "get", "getOrganizationUnit"),
                ("/api/organization/units", "post", "createOrganizationUnit"),
                (
                    "/api/organization/units/{id}",
                    "put",
                    "updateOrganizationUnit",
                ),
                (
                    "/api/organization/units/{id}",
                    "delete",
                    "deactivateOrganizationUnit",
                ),
                (
                    "/api/organization/units/{id}/permissions",
                    "get",
                    "getOrganizationPermissions",
                ),
                (
                    "/api/organization/units/{id}/permissions",
                    "put",
                    "updateOrganizationPermissions",
                ),
            ],
        );

        let unit_delete = &document["paths"]["/api/organization/units/{id}"]["delete"];
        assert_eq!(
            unit_delete["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_EmptyData"
        );
        for status in ["401", "403", "404", "409"] {
            assert_eq!(
                unit_delete["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse"
            );
        }
        assert_eq!(
            document["paths"]["/api/organization/units"]["post"]["responses"]["201"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_UuidIdData"
        );
        assert_eq!(
            document["paths"]["/api/organization/units"]["post"]["responses"]["400"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert!(document["paths"]["/api/organization/units"]["post"]["responses"]["409"].is_null());
        assert!(
            document["paths"]["/api/organization/units/{id}"]["put"]["responses"]["409"]
                .is_object()
        );
        assert!(
            document["paths"]["/api/organization/units/{id}/permissions"]["put"]["responses"]
                ["404"]
                .is_null()
        );

        let schemas = &document["components"]["schemas"];
        let unit = &schemas["OrganizationUnit"];
        assert!(required(unit).contains(&"is_system"));
        assert_eq!(unit["properties"]["is_system"]["type"], "boolean");
        for field in [
            "name_en",
            "description",
            "parent_unit_id",
            "phone",
            "email",
            "location",
            "subject_group_id",
        ] {
            assert!(required(unit).contains(&field));
            assert!(contains_null(&unit["properties"][field]));
        }

        let include_inactive = document["paths"]["/api/organization/units"]["get"]["parameters"]
            .as_array()
            .expect("organization unit list parameters")
            .iter()
            .find(|parameter| parameter["name"] == "include_inactive")
            .expect("include_inactive organization unit query parameter");
        assert_eq!(include_inactive["in"], "query");
        assert_eq!(include_inactive["required"], false);
        assert_eq!(include_inactive["schema"]["type"], "boolean");

        let grant = &schemas["OrganizationPermissionGrant"];
        assert!(required(grant).contains(&"position_code"));
        assert!(contains_null(&grant["properties"]["position_code"]));
    }

    #[test]
    fn documents_lookup_menu_and_feature_read_operations() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/menu/user", "get", "getUserMenu"),
                ("/api/admin/features", "get", "listFeatures"),
                ("/api/admin/features/{id}", "get", "getFeature"),
                ("/api/admin/menu/workspaces", "get", "listMenuWorkspaces"),
                ("/api/admin/menu/workspaces", "post", "createMenuWorkspace"),
                (
                    "/api/admin/menu/workspaces/{id}",
                    "put",
                    "updateMenuWorkspace",
                ),
                (
                    "/api/admin/menu/workspaces/{id}",
                    "delete",
                    "deleteMenuWorkspace",
                ),
                (
                    "/api/admin/menu/workspaces/reorder",
                    "post",
                    "reorderMenuWorkspaces",
                ),
                ("/api/admin/menu/groups", "get", "listMenuGroups"),
                ("/api/admin/menu/groups", "post", "createMenuGroup"),
                ("/api/admin/menu/groups/{id}", "put", "updateMenuGroup"),
                ("/api/admin/menu/groups/{id}", "delete", "deleteMenuGroup"),
                (
                    "/api/admin/menu/groups/reorder",
                    "post",
                    "reorderMenuGroups",
                ),
                ("/api/admin/menu/items", "get", "listMenuItems"),
                ("/api/admin/menu/items", "post", "createMenuItem"),
                ("/api/admin/menu/items/{id}", "put", "updateMenuItem"),
                ("/api/admin/menu/items/{id}", "delete", "deleteMenuItem"),
                (
                    "/api/admin/menu/items/{id}/group",
                    "put",
                    "moveMenuItemToGroup",
                ),
                ("/api/admin/menu/items/reorder", "post", "reorderMenuItems"),
                (
                    "/api/admin/menu/templates/academic/recommended",
                    "get",
                    "previewRecommendedAcademicMenuTemplate",
                ),
                (
                    "/api/admin/menu/templates/academic/recommended/apply",
                    "post",
                    "applyRecommendedAcademicMenuTemplate",
                ),
                ("/api/lookup/staff", "get", "lookupStaff"),
                ("/api/lookup/students", "get", "lookupStudents"),
                ("/api/lookup/rooms", "get", "lookupRooms"),
                ("/api/lookup/roles", "get", "lookupRoles"),
                (
                    "/api/lookup/organization-units",
                    "get",
                    "lookupOrganizationUnits",
                ),
                (
                    "/api/lookup/organization-units/{id}",
                    "get",
                    "getLookupOrganizationUnit",
                ),
                ("/api/lookup/grade-levels", "get", "lookupGradeLevels"),
                ("/api/lookup/homerooms", "get", "lookupHomerooms"),
                ("/api/lookup/academic-years", "get", "lookupAcademicYears"),
                ("/api/lookup/subjects", "get", "lookupSubjects"),
            ],
        );

        assert_eq!(
            document["paths"]["/api/menu/user"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_UserMenuData"
        );
        assert_eq!(
            document["paths"]["/api/lookup/staff"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_StaffLookupItem"
        );

        let lookup_parameters = document["paths"]["/api/lookup/staff"]["get"]["parameters"]
            .as_array()
            .expect("lookup parameters must be an array");
        for name in ["activeOnly", "search", "limit", "memberOnly"] {
            assert!(lookup_parameters
                .iter()
                .any(|parameter| { parameter["name"] == name && parameter["in"] == "query" }));
        }

        let homeroom_parameters = document["paths"]["/api/lookup/homerooms"]["get"]["parameters"]
            .as_array()
            .expect("homeroom lookup parameters must be an array");
        assert!(homeroom_parameters.iter().any(|parameter| {
            parameter["name"] == "academicYearId"
                && parameter["in"] == "query"
                && parameter["required"] == true
        }));

        let schemas = &document["components"]["schemas"];
        let grade = &schemas["GradeLevelLookupItem"];
        assert!(required(grade).contains(&"short_name"));
        assert!(contains_null(&grade["properties"]["short_name"]));

        let organization = &schemas["OrganizationUnitLookupItem"];
        assert!(!required(organization).contains(&"description"));
        assert!(!contains_null(&organization["properties"]["description"]));

        let menu_group = &schemas["MenuGroup"];
        assert!(required(menu_group).contains(&"name_en"));
        assert!(contains_null(&menu_group["properties"]["name_en"]));
        assert!(required(menu_group).contains(&"workspace_code"));

        let menu_group_response = &schemas["MenuGroupResponse"];
        for field in [
            "displayOrder",
            "workspaceCode",
            "workspaceName",
            "workspaceIcon",
            "workspaceOrder",
        ] {
            assert!(required(menu_group_response).contains(&field));
        }

        let menu_workspace = &schemas["MenuWorkspace"];
        assert!(required(menu_workspace).contains(&"code"));
        assert!(required(menu_workspace).contains(&"display_order"));

        let feature_response = &schemas["FeatureToggleResponse"];
        for field in ["data", "message"] {
            assert!(required(feature_response).contains(&field));
            assert!(contains_null(&feature_response["properties"][field]));
        }
    }

    #[test]
    fn documents_staff_student_and_parent_profile_reads() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/staff", "get", "listStaff"),
                ("/api/staff/dashboard", "get", "getStaffDashboard"),
                ("/api/staff/{id}", "get", "getStaffProfile"),
                (
                    "/api/staff/{id}/public-profile",
                    "get",
                    "getPublicStaffProfile",
                ),
                ("/api/student/profile", "get", "getStudentProfile"),
                ("/api/parent/profile", "get", "getParentProfile"),
                (
                    "/api/parent/students/{student_id}",
                    "get",
                    "getParentChildProfile",
                ),
            ],
        );

        assert_eq!(
            document["paths"]["/api/staff"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_StaffListData"
        );
        assert_eq!(
            document["paths"]["/api/student/profile"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_StudentProfile"
        );

        let schemas = &document["components"]["schemas"];
        let staff = &schemas["StaffProfileResponse"];
        for field in ["national_id", "email", "phone", "staff_info"] {
            assert!(required(staff).contains(&field));
            assert!(contains_null(&staff["properties"][field]));
        }

        let student = &schemas["StudentDbRow"];
        for field in ["national_id", "date_of_birth", "medical_conditions"] {
            assert!(required(student).contains(&field));
            assert!(contains_null(&student["properties"][field]));
        }

        let parent = &schemas["ParentProfile"];
        assert!(required(parent).contains(&"national_id"));
        assert!(contains_null(&parent["properties"]["national_id"]));

        let public_staff = &schemas["PublicStaffProfile"];
        assert!(public_staff["properties"].get("national_id").is_none());
    }

    #[test]
    fn documents_academic_year_scoped_profile_and_calendar_queries() {
        let document = school_api_value().expect("document should serialize");
        assert_eq!(
            query_contract(&document, "/api/students", "get"),
            BTreeSet::from([
                ("academicYearId".to_string(), true),
                ("page".to_string(), false),
                ("pageSize".to_string(), false),
                ("search".to_string(), false),
                ("status".to_string(), false),
            ])
        );

        for path in [
            "/api/students/{id}",
            "/api/student/profile",
            "/api/parent/profile",
            "/api/parent/students/{student_id}",
        ] {
            assert_eq!(
                query_contract(&document, path, "get"),
                BTreeSet::from([("academicYearId".to_string(), true)]),
                "incorrect academic-year query contract for {path}"
            );
        }

        let calendar_query = BTreeSet::from([
            ("academicTermId".to_string(), false),
            ("academicYearId".to_string(), true),
            ("audience".to_string(), false),
            ("categoryId".to_string(), false),
            ("from".to_string(), false),
            ("q".to_string(), false),
            ("tagId".to_string(), false),
            ("to".to_string(), false),
            ("visibility".to_string(), false),
        ]);
        for path in [
            "/api/calendar/events",
            "/api/me/calendar/events",
            "/api/parent/students/{student_id}/calendar/events",
            "/api/public/calendar/events",
        ] {
            assert_eq!(
                query_contract(&document, path, "get"),
                calendar_query,
                "incorrect calendar query contract for {path}"
            );
        }

        let parent_calendar_parameters = document["paths"]
            ["/api/parent/students/{student_id}/calendar/events"]["get"]["parameters"]
            .as_array()
            .expect("parent calendar parameters must be an array");
        assert!(parent_calendar_parameters.iter().any(|parameter| {
            parameter["name"] == "student_id"
                && parameter["in"] == "path"
                && parameter["required"] == true
        }));

        let schemas = &document["components"]["schemas"];
        assert_eq!(
            schemas["CalendarAudienceType"]["enum"],
            serde_json::json!(["all", "staff", "student", "parent"])
        );
        assert_eq!(
            schemas["CalendarVisibility"]["enum"],
            serde_json::json!(["public", "private"])
        );
    }

    #[test]
    fn academic_batch_read_queries_are_camel_case() {
        let document = school_api_value().expect("document should serialize");
        for (path, operation_id, query_name, response_schema) in [
            (
                "/api/academic/learning-groups",
                "listLearningGroupsForTerm",
                "academicTermId",
                "#/components/schemas/ApiResponse_Vec_LearningGroup",
            ),
            (
                "/api/academic/placements",
                "listPlacementsForAcademicYear",
                "academicYearId",
                "#/components/schemas/ApiResponse_Vec_HomeroomPlacement",
            ),
            (
                "/api/academic/homeroom-advisors",
                "listHomeroomAdvisorsForAcademicYear",
                "academicYearId",
                "#/components/schemas/ApiResponse_Vec_HomeroomAdvisorAssignment",
            ),
            (
                "/api/academic/study-program-options",
                "listStudyProgramOptionsForAcademicYear",
                "academicYearId",
                "#/components/schemas/ApiResponse_Vec_StudyProgramOption",
            ),
        ] {
            let operation = &document["paths"][path]["get"];
            assert_eq!(operation["operationId"], operation_id, "path {path}");
            assert_eq!(
                query_contract(&document, path, "get"),
                BTreeSet::from([(query_name.to_string(), true)]),
                "path {path}"
            );
            assert!(operation["parameters"]
                .as_array()
                .unwrap_or_else(|| panic!("{path} query parameters must be an array"))
                .iter()
                .all(|parameter| !parameter["name"]
                    .as_str()
                    .is_some_and(|name| name.contains('_'))));
            assert_eq!(
                operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
                response_schema,
                "path {path}"
            );
            assert_eq!(
                operation["responses"]["400"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse",
                "path {path}"
            );
        }
    }

    #[test]
    fn academic_workspace_reads_are_documented() {
        let document = school_api_value().expect("document should serialize");
        for (path, operation_id, response_schema) in [
            (
                "/api/academic/curricula/overview",
                "getCurriculumOverview",
                "#/components/schemas/ApiResponse_CurriculumOverview",
            ),
            (
                "/api/academic/curricula/management-options",
                "getCurriculumCreateOptions",
                "#/components/schemas/ApiResponse_CurriculumCreateOptions",
            ),
            (
                "/api/academic/curriculum-versions/{id}/management-options",
                "getCurriculumManagementOptions",
                "#/components/schemas/ApiResponse_CurriculumManagementOptions",
            ),
            (
                "/api/academic/curriculum-versions/{curriculumVersionId}/structure",
                "getCurriculumStructureWorkspace",
                "#/components/schemas/ApiResponse_CurriculumStructureWorkspace",
            ),
            (
                "/api/academic/setup/workspace",
                "getAcademicSetupWorkspace",
                "#/components/schemas/ApiResponse_AcademicSetupWorkspace",
            ),
            (
                "/api/academic/delivery/workspace",
                "getLearningDeliveryOverview",
                "#/components/schemas/ApiResponse_LearningDeliveryOverview",
            ),
            (
                "/api/academic/delivery/management-options",
                "getLearningDeliveryManagementOptions",
                "#/components/schemas/ApiResponse_DeliveryManagementOptions",
            ),
        ] {
            let operation = &document["paths"][path]["get"];
            assert_eq!(operation["operationId"], operation_id, "path {path}");
            assert_eq!(
                operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
                response_schema,
                "path {path}"
            );
            assert_eq!(
                operation["responses"]["401"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse",
                "path {path}"
            );
            assert_eq!(
                operation["responses"]["400"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse",
                "path {path}"
            );
            assert_eq!(
                operation["responses"]["403"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse",
                "path {path}"
            );
        }
        assert_eq!(
            document["paths"]["/api/academic/curriculum-versions/{curriculumVersionId}/structure"]
                ["get"]["responses"]["404"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert_eq!(
            document["paths"]["/api/academic/curriculum-versions/{id}/management-options"]["get"]
                ["responses"]["404"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        for schema in [
            "CurriculumOverview",
            "CurriculumCreateOptions",
            "CurriculumManagementOptions",
            "CurriculumCatalogVersionOption",
            "CurriculumTermSlot",
            "CurriculumStructureRequirement",
            "CurriculumStructureWorkspace",
            "AcademicSetupWorkspace",
            "LearningDeliveryOverview",
            "LearningOfferingOverviewItem",
            "DeliveryCatalogVersionOption",
            "DeliveryManagementOptions",
        ] {
            assert!(
                !document["components"]["schemas"][schema].is_null(),
                "missing schema {schema}"
            );
        }
    }

    #[test]
    fn supervision_routes_are_fully_documented() {
        let document = school_api_value().expect("document should serialize");
        let expected = [
            ("/api/supervision/cycles", "get", "listSupervisionCycles"),
            ("/api/supervision/cycles", "post", "createSupervisionCycle"),
            (
                "/api/supervision/cycles/{id}",
                "patch",
                "updateSupervisionCycle",
            ),
            (
                "/api/supervision/templates",
                "get",
                "listSupervisionTemplates",
            ),
            (
                "/api/supervision/templates",
                "post",
                "createSupervisionTemplate",
            ),
            (
                "/api/supervision/templates/{id}",
                "get",
                "getSupervisionTemplate",
            ),
            (
                "/api/supervision/templates/{id}",
                "patch",
                "updateSupervisionTemplate",
            ),
            (
                "/api/supervision/observations",
                "get",
                "listSupervisionObservations",
            ),
            (
                "/api/supervision/observations/requests",
                "post",
                "requestSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}",
                "get",
                "getSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}",
                "patch",
                "updateSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}/review",
                "get",
                "getSupervisionObservationReview",
            ),
            (
                "/api/supervision/observations/{id}/evaluator-availability",
                "get",
                "getSupervisionEvaluatorAvailability",
            ),
            (
                "/api/supervision/observations/{id}/timetable-options",
                "get",
                "getSupervisionObservationTimetableOptions",
            ),
            (
                "/api/supervision/observations/{id}/evaluators",
                "put",
                "replaceSupervisionObservationEvaluators",
            ),
            (
                "/api/supervision/observations/{id}/cancel",
                "post",
                "cancelSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}/request",
                "patch",
                "updateRequestedSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}/request",
                "delete",
                "cancelRequestedSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}/approve-request",
                "post",
                "approveSupervisionObservationRequest",
            ),
            (
                "/api/supervision/observations/{id}/return-request",
                "post",
                "returnSupervisionObservationRequest",
            ),
            (
                "/api/supervision/observations/{id}/evaluations/me/submit",
                "post",
                "submitMySupervisionEvaluation",
            ),
            (
                "/api/supervision/observations/{id}/certify",
                "post",
                "certifySupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}/approve",
                "post",
                "approveSupervisionObservation",
            ),
            (
                "/api/supervision/observations/{id}/acknowledge",
                "post",
                "acknowledgeSupervisionObservation",
            ),
            (
                "/api/supervision/reports/cycles/{id}/progress",
                "get",
                "getSupervisionCycleProgress",
            ),
            (
                "/api/supervision/reports/cycles/{id}/teacher-status",
                "get",
                "getSupervisionTeacherStatusOverview",
            ),
        ];
        assert_operations(&document, &expected);

        assert_eq!(
            query_contract(&document, "/api/supervision/cycles", "get"),
            BTreeSet::from([
                ("academicTermId".to_string(), false),
                ("academicYearId".to_string(), true),
            ])
        );
        assert_eq!(
            query_contract(&document, "/api/supervision/observations", "get"),
            BTreeSet::from([
                ("academicTermId".to_string(), false),
                ("academicYearId".to_string(), true),
                ("cycleId".to_string(), false),
                ("status".to_string(), false),
            ])
        );
        for (path, method, _) in expected {
            if let Some(parameters) = document["paths"][path][method]["parameters"].as_array() {
                assert!(parameters.iter().all(|parameter| !parameter["name"]
                    .as_str()
                    .is_some_and(|name| name.contains('_'))));
            }
        }
    }

    #[test]
    fn question_bank_routes_are_fully_documented() {
        let document = school_api_value().expect("document should serialize");
        let expected = [
            (
                "/api/academic/question-bank/options",
                "get",
                "listQuestionBankOptions",
            ),
            (
                "/api/academic/question-bank/questions",
                "get",
                "listQuestionBankQuestions",
            ),
            (
                "/api/academic/question-bank/questions",
                "post",
                "createQuestionBankQuestion",
            ),
            (
                "/api/academic/question-bank/questions/export-data",
                "post",
                "exportQuestionBankData",
            ),
            (
                "/api/academic/question-bank/questions/{id}",
                "get",
                "getQuestionBankQuestion",
            ),
            (
                "/api/academic/question-bank/questions/{id}",
                "put",
                "updateQuestionBankQuestion",
            ),
            (
                "/api/academic/question-bank/questions/{id}",
                "delete",
                "deleteQuestionBankQuestion",
            ),
            (
                "/api/academic/question-bank/questions/{question_id}/files/{file_id}",
                "get",
                "getQuestionBankQuestionFile",
            ),
        ];
        assert_operations(&document, &expected);

        assert_eq!(
            query_contract(&document, "/api/academic/question-bank/questions", "get"),
            BTreeSet::from([
                ("difficulty".to_string(), false),
                ("page".to_string(), false),
                ("pageSize".to_string(), false),
                ("questionType".to_string(), false),
                ("search".to_string(), false),
                ("status".to_string(), false),
                ("subjectId".to_string(), false),
                ("tag".to_string(), false),
            ])
        );
        let export_schema = &document["components"]["schemas"]["QuestionBankExportDataRequest"]
            ["properties"]["questionIds"];
        assert_eq!(export_schema["minItems"], 1);
        assert_eq!(export_schema["maxItems"], 200);
        assert_eq!(
            document["paths"]["/api/academic/question-bank/questions/export-data"]["post"]
                ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_QuestionDetail"
        );
    }

    #[test]
    fn documents_parent_academic_context_options() {
        let document = school_api_value().expect("document should serialize");
        assert_eq!(
            document["paths"]["/api/parent/academic-context/options"]["get"]["operationId"],
            "listParentAcademicContextOptions"
        );
        assert_eq!(
            document["paths"]["/api/parent/academic-context/options"]["get"]["responses"]["200"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_AcademicContextOptions"
        );
    }

    #[test]
    fn documents_self_service_timetable_exam_and_calendar_reads() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/parent/students/{student_id}/timetable",
                    "get",
                    "getParentChildTimetable",
                ),
                (
                    "/api/parent/students/{student_id}/exam-schedules",
                    "get",
                    "getParentChildExamSchedule",
                ),
                (
                    "/api/parent/students/{student_id}/calendar/events",
                    "get",
                    "getParentChildCalendarEvents",
                ),
                ("/api/me/timetable", "get", "getMyTimetable"),
                ("/api/me/exam-schedules", "get", "listMyExamSchedules"),
                ("/api/staff/exam-schedules", "get", "listStaffExamSchedules"),
                ("/api/me/calendar/events", "get", "listMyCalendarEvents"),
            ],
        );

        assert_eq!(
            document["paths"]["/api/me/timetable"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_TimetableEntry"
        );
        assert_eq!(
            document["paths"]["/api/parent/students/{student_id}/exam-schedules"]["get"]
                ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_PersonalExamScheduleRound"
        );
        assert_eq!(
            document["paths"]["/api/staff/exam-schedules"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_StaffPublishedExamScheduleRound"
        );

        let parent_calendar_parameters = document["paths"]
            ["/api/parent/students/{student_id}/calendar/events"]["get"]["parameters"]
            .as_array()
            .expect("parent calendar parameters must be an array");
        for (name, location) in [
            ("student_id", "path"),
            ("academicYearId", "query"),
            ("academicTermId", "query"),
            ("from", "query"),
            ("to", "query"),
            ("categoryId", "query"),
            ("tagId", "query"),
            ("audience", "query"),
            ("visibility", "query"),
            ("q", "query"),
        ] {
            assert!(parent_calendar_parameters
                .iter()
                .any(|parameter| parameter["name"] == name && parameter["in"] == location));
        }

        let my_timetable_parameters = document["paths"]["/api/me/timetable"]["get"]["parameters"]
            .as_array()
            .expect("staff timetable parameters must be an array");
        for name in ["academicTermId", "date"] {
            assert!(my_timetable_parameters
                .iter()
                .any(|parameter| parameter["name"] == name && parameter["in"] == "query"));
        }

        let schemas = &document["components"]["schemas"];
        let timetable = &schemas["TimetableEntry"];
        for field in [
            "learningGroupId",
            "offeringId",
            "homeroomId",
            "roomId",
            "note",
            "title",
            "subjectId",
            "subjectVersionDisplayLabel",
            "activityId",
            "activityVersionDisplayLabel",
            "activitySchedulingMode",
        ] {
            assert!(!required(timetable).contains(&field));
            assert!(contains_null(&timetable["properties"][field]));
        }
        for forbidden in ["semesterId", "classroomCourseId", "classroomId"] {
            assert!(timetable["properties"].get(forbidden).is_none());
        }

        let exam_round = &schemas["PersonalExamScheduleRound"];
        assert!(required(exam_round).contains(&"publishedAt"));
        assert!(contains_null(&exam_round["properties"]["publishedAt"]));
        let exam_session = &schemas["PersonalExamSessionView"];
        for field in ["buildingName", "seatNumber"] {
            assert!(required(exam_session).contains(&field));
            assert!(contains_null(&exam_session["properties"][field]));
        }

        let staff_round = &schemas["StaffPublishedExamScheduleRound"];
        assert!(required(staff_round).contains(&"publishedAt"));
        assert!(contains_null(&staff_round["properties"]["publishedAt"]));
        assert!(required(staff_round).contains(&"days"));

        let staff_day = &schemas["StaffPublishedExamDay"];
        for field in ["sessions", "roomAssignments"] {
            assert!(required(staff_day).contains(&field));
        }

        let staff_assignment = &schemas["StaffPublishedExamRoomAssignment"];
        for field in ["buildingName", "earliestStartsAt", "latestEndsAt"] {
            assert!(required(staff_assignment).contains(&field));
            assert!(contains_null(&staff_assignment["properties"][field]));
        }

        let staff_invigilator = &schemas["StaffPublishedExamInvigilator"];
        for forbidden in ["username", "email", "phone", "nationalId", "national_id"] {
            assert!(staff_invigilator["properties"].get(forbidden).is_none());
        }

        let calendar_event = &schemas["CalendarViewerEvent"];
        for field in [
            "categoryId",
            "categoryName",
            "categoryColor",
            "description",
            "location",
            "startTime",
            "endTime",
        ] {
            assert!(required(calendar_event).contains(&field));
            assert!(contains_null(&calendar_event["properties"][field]));
        }
    }

    #[test]
    fn documents_release_one_timetable_version_cutover() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/academic/timetable-versions",
                    "get",
                    "listTimetableVersions",
                ),
                (
                    "/api/academic/timetable-versions/resolve",
                    "get",
                    "resolveTimetableVersion",
                ),
                (
                    "/api/academic/timetable-versions/{source_id}/clone",
                    "post",
                    "cloneTimetableVersion",
                ),
            ],
        );

        let schemas = &document["components"]["schemas"];
        for schema_name in [
            "TimetableVersion",
            "TimetableVersionTarget",
            "TimetableVersionStatus",
            "TimetableVersionDisplayState",
            "CloneTimetableVersionRequest",
        ] {
            assert!(!schemas[schema_name].is_null(), "missing {schema_name}");
        }
        for schema_name in [
            "TimetableEntry",
            "CreateTimetableEntryRequest",
            "UpdateTimetableEntryRequest",
            "CreateBatchTimetableEntriesRequest",
            "SwapTimetableEntriesRequest",
            "ValidateMovesRequest",
            "FromCurrentRequest",
            "ApplyTemplateRequest",
            "ClearTimetableRequest",
        ] {
            assert!(
                required(&schemas[schema_name]).contains(&"timetableVersionId"),
                "{schema_name} must require timetableVersionId"
            );
        }
        assert!(required(&schemas["LearningGroup"]).contains(&"teachersLocked"));
        assert!(required(&schemas["HomeroomDeliveryWorkspace"]).contains(&"timetableVersionId"));
        assert!(contains_null(
            &schemas["HomeroomDeliveryWorkspace"]["properties"]["timetableVersionId"]
        ));
        assert!(required(&schemas["HomeroomDeliveryItem"]).contains(&"weeklyPeriodTarget"));
        assert!(contains_null(
            &schemas["HomeroomDeliveryItem"]["properties"]["weeklyPeriodTarget"]
        ));
        assert!(
            schemas["UpdateLearningOfferingRequest"]["properties"]["weeklyPeriodTarget"].is_null()
        );
        assert!(schemas["CourseOfferingSnapshot"]["properties"]["weeklyPeriodTarget"].is_null());

        assert_eq!(
            query_contract(&document, "/api/me/timetable", "get"),
            BTreeSet::from([
                ("academicTermId".to_string(), true),
                ("date".to_string(), true),
            ])
        );
        assert!(query_contract(
            &document,
            "/api/parent/students/{student_id}/timetable",
            "get"
        )
        .contains(&("date".to_string(), true)));
    }

    #[test]
    fn documents_calendar_school_and_notification_reads() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/public/calendar/events",
                    "get",
                    "listPublicCalendarEvents",
                ),
                ("/api/calendar/events", "get", "listCalendarEvents"),
                ("/api/calendar/categories", "get", "listCalendarCategories"),
                ("/api/calendar/tags", "get", "listCalendarTags"),
                ("/api/school/public", "get", "getPublicSchoolInfo"),
                ("/api/school/settings", "get", "getSchoolSettings"),
                ("/api/notifications", "get", "listNotifications"),
            ],
        );

        assert_eq!(
            document["paths"]["/api/calendar/events"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_CalendarEvent"
        );
        assert_eq!(
            document["paths"]["/api/notifications"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_ListNotificationsResponse"
        );

        let calendar_parameters = document["paths"]["/api/calendar/events"]["get"]["parameters"]
            .as_array()
            .expect("calendar parameters must be an array");
        for name in [
            "academicYearId",
            "academicTermId",
            "from",
            "to",
            "categoryId",
            "tagId",
            "audience",
            "visibility",
            "q",
        ] {
            assert!(calendar_parameters
                .iter()
                .any(|parameter| parameter["name"] == name && parameter["in"] == "query"));
        }

        let notification_parameters = document["paths"]["/api/notifications"]["get"]["parameters"]
            .as_array()
            .expect("notification parameters must be an array");
        for name in ["page", "limit", "unread_only"] {
            assert!(notification_parameters
                .iter()
                .any(|parameter| parameter["name"] == name && parameter["in"] == "query"));
        }

        let schemas = &document["components"]["schemas"];
        let calendar_event = &schemas["CalendarEvent"];
        for field in [
            "categoryId",
            "description",
            "startTime",
            "createdBy",
            "updatedBy",
        ] {
            assert!(required(calendar_event).contains(&field));
            assert!(contains_null(&calendar_event["properties"][field]));
        }
        assert!(calendar_event["properties"].get("targets").is_some());
        assert!(calendar_event["properties"].get("reminders").is_some());
        let public_event = &schemas["CalendarPublicEvent"];
        assert!(public_event["properties"].get("targets").is_none());
        assert!(public_event["properties"].get("reminders").is_none());

        for schema_name in ["SchoolSettingsResponse", "PublicSchoolInfoData"] {
            let school = &schemas[schema_name];
            for property in school["properties"]
                .as_object()
                .expect("school schema properties")
                .keys()
            {
                assert!(required(school).contains(&property.as_str()));
                assert!(contains_null(&school["properties"][property]));
            }
        }

        let notification = &schemas["Notification"];
        assert!(notification["properties"].get("type").is_some());
        assert!(notification["properties"].get("type_").is_none());
        for field in ["link", "read_at"] {
            assert!(required(notification).contains(&field));
            assert!(contains_null(&notification["properties"][field]));
        }
    }

    #[test]
    fn documents_delegation_member_and_complete_authorization_inventory() {
        let document = school_api_value().expect("document should serialize");
        let expected = [
            ("/api/auth/login", "post", "login"),
            ("/api/auth/logout", "post", "logout"),
            ("/api/auth/me", "get", "getCurrentUser"),
            ("/api/auth/me/profile", "get", "getCurrentUserProfile"),
            ("/api/auth/me/profile", "put", "updateCurrentUserProfile"),
            (
                "/api/auth/me/change-password",
                "post",
                "changeCurrentUserPassword",
            ),
            ("/api/roles", "get", "listRoles"),
            ("/api/roles/{id}", "get", "getRole"),
            ("/api/roles", "post", "createRole"),
            ("/api/roles/{id}", "put", "updateRole"),
            ("/api/roles/{id}", "delete", "deleteRole"),
            ("/api/permissions", "get", "listPermissions"),
            ("/api/permissions/modules", "get", "listPermissionsByModule"),
            ("/api/users/{id}/roles", "get", "getUserRoles"),
            ("/api/users/{id}/roles", "post", "assignUserRole"),
            (
                "/api/users/{id}/roles/{role_id}",
                "delete",
                "removeUserRole",
            ),
            (
                "/api/users/{id}/permissions",
                "get",
                "listUserEffectivePermissions",
            ),
            ("/api/organization/units", "get", "listOrganizationUnits"),
            ("/api/organization/units/{id}", "get", "getOrganizationUnit"),
            ("/api/organization/units", "post", "createOrganizationUnit"),
            (
                "/api/organization/units/{id}",
                "put",
                "updateOrganizationUnit",
            ),
            (
                "/api/organization/units/{id}",
                "delete",
                "deactivateOrganizationUnit",
            ),
            (
                "/api/organization/units/{id}/permissions",
                "get",
                "getOrganizationPermissions",
            ),
            (
                "/api/organization/units/{id}/permissions",
                "put",
                "updateOrganizationPermissions",
            ),
            (
                "/api/organization/units/{id}/delegatable-permissions",
                "get",
                "listDelegatablePermissions",
            ),
            (
                "/api/organization/units/{id}/delegations",
                "get",
                "listOrganizationDelegations",
            ),
            (
                "/api/organization/units/{id}/delegations",
                "post",
                "createOrganizationDelegation",
            ),
            (
                "/api/organization/delegations/{id}",
                "delete",
                "revokeOrganizationDelegation",
            ),
            (
                "/api/organization/units/{id}/members",
                "get",
                "listOrganizationMembers",
            ),
            (
                "/api/organization/units/{id}/members",
                "post",
                "addOrganizationMember",
            ),
            (
                "/api/organization/units/{id}/members/{user_id}",
                "put",
                "updateOrganizationMember",
            ),
            (
                "/api/organization/units/{id}/members/{user_id}",
                "delete",
                "removeOrganizationMember",
            ),
        ];
        assert_eq!(expected.len(), 32);
        assert_operations(&document, &expected);

        let mut operation_ids = HashSet::new();
        for path_item in document["paths"]
            .as_object()
            .expect("paths must be an object")
            .values()
        {
            for operation in path_item
                .as_object()
                .expect("path item must be an object")
                .values()
            {
                if let Some(operation_id) = operation["operationId"].as_str() {
                    assert!(
                        operation_ids.insert(operation_id),
                        "duplicate operationId: {operation_id}"
                    );
                }
            }
        }
        let schemas = &document["components"]["schemas"];
        let delegation = &schemas["DelegationItem"];
        assert_eq!(
            delegation["properties"]["started_at"]["format"],
            "date-time"
        );
        for field in ["reason", "expires_at"] {
            assert!(required(delegation).contains(&field));
            assert!(contains_null(&delegation["properties"][field]));
        }

        let member = &schemas["OrganizationMemberItem"];
        assert_eq!(member["properties"]["started_at"]["format"], "date");
        for field in ["position_title", "responsibilities"] {
            assert!(required(member).contains(&field));
            assert!(contains_null(&member["properties"][field]));
        }

        let list_members = &document["paths"]["/api/organization/units/{id}/members"]["get"];
        let include_children = list_members["parameters"]
            .as_array()
            .expect("member parameters")
            .iter()
            .find(|parameter| parameter["name"] == "include_children")
            .expect("include_children query parameter");
        assert_eq!(include_children["in"], "query");
        assert_eq!(include_children["required"], false);
        assert_eq!(include_children["schema"]["type"], "boolean");
    }

    #[test]
    fn certificate_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/certificates/campaigns",
                    "get",
                    "listCertificateCampaigns",
                ),
                (
                    "/api/certificates/campaigns",
                    "post",
                    "createCertificateCampaign",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}",
                    "get",
                    "getCertificateCampaign",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}",
                    "put",
                    "updateCertificateCampaign",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/purge-impact",
                    "get",
                    "getCertificateCampaignPurgeImpact",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/purge",
                    "post",
                    "startCertificateCampaignPurge",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/purge-status",
                    "get",
                    "getCertificateCampaignPurgeStatus",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/purge/retry",
                    "post",
                    "retryCertificateCampaignPurge",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/status",
                    "put",
                    "changeCertificateCampaignStatus",
                ),
                (
                    "/api/certificates/owner-options",
                    "get",
                    "listCertificateOwnerOptions",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/templates",
                    "get",
                    "listCertificateTemplates",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/templates",
                    "post",
                    "createCertificateTemplate",
                ),
                (
                    "/api/certificates/templates/{template_id}",
                    "get",
                    "getCertificateTemplate",
                ),
                (
                    "/api/certificates/templates/{template_id}",
                    "put",
                    "updateCertificateTemplate",
                ),
                (
                    "/api/certificates/templates/{template_id}",
                    "delete",
                    "deleteCertificateTemplate",
                ),
                (
                    "/api/certificates/templates/{template_id}/background",
                    "put",
                    "attachCertificateTemplateBackground",
                ),
                (
                    "/api/certificates/templates/{template_id}/assets",
                    "post",
                    "attachCertificateTemplateAsset",
                ),
                (
                    "/api/certificates/templates/{template_id}/fonts",
                    "get",
                    "listCertificateSchoolFonts",
                ),
                (
                    "/api/certificates/templates/{template_id}/fonts/inspect",
                    "post",
                    "inspectCertificateFontUploads",
                ),
                (
                    "/api/certificates/templates/{template_id}/fonts/batch",
                    "post",
                    "attachCertificateFontBatch",
                ),
                (
                    "/api/certificates/templates/{template_id}/assets/{asset_id}",
                    "delete",
                    "deleteCertificateTemplateAsset",
                ),
                (
                    "/api/certificates/templates/{template_id}/variables",
                    "get",
                    "getCertificateTemplateVariableCatalog",
                ),
                (
                    "/api/certificates/templates/{template_id}/preview-manifest",
                    "post",
                    "createCertificateTemplatePreviewManifest",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/candidates",
                    "get",
                    "listCertificateCandidates",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/candidates/import",
                    "post",
                    "importCertificateCandidates",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/candidates/manual",
                    "post",
                    "createManualCertificateCandidate",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/candidates/account-search",
                    "get",
                    "searchCertificateCandidateAccounts",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/candidates/account-search",
                    "post",
                    "createAccountCertificateCandidate",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/candidates/bulk",
                    "post",
                    "bulkUpdateCertificateCandidates",
                ),
                (
                    "/api/certificates/candidates/{candidate_id}",
                    "get",
                    "getCertificateCandidate",
                ),
                (
                    "/api/certificates/candidates/{candidate_id}",
                    "put",
                    "updateCertificateCandidate",
                ),
                (
                    "/api/certificates/candidates/{candidate_id}",
                    "delete",
                    "deleteCertificateCandidate",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/issue-requests",
                    "get",
                    "listCertificateCampaignIssueRequests",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/issue-requests",
                    "post",
                    "submitCertificateIssueRequest",
                ),
                (
                    "/api/certificates/issue-requests",
                    "get",
                    "listCertificateIssueRequests",
                ),
                (
                    "/api/certificates/issue-requests/{request_id}",
                    "get",
                    "getCertificateIssueRequest",
                ),
                (
                    "/api/certificates/issue-requests/{request_id}/withdraw",
                    "post",
                    "withdrawCertificateIssueRequest",
                ),
                (
                    "/api/certificates/issue-requests/{request_id}/review",
                    "post",
                    "startCertificateIssueRequestReview",
                ),
                (
                    "/api/certificates/issue-requests/{request_id}/return",
                    "post",
                    "returnCertificateIssueRequest",
                ),
                (
                    "/api/certificates/issue-requests/{request_id}/issue",
                    "post",
                    "issueCertificates",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/issued",
                    "get",
                    "listIssuedCertificates",
                ),
                (
                    "/api/certificates/{certificate_id}",
                    "get",
                    "getIssuedCertificate",
                ),
                (
                    "/api/certificates/{certificate_id}/revoke",
                    "post",
                    "revokeIssuedCertificate",
                ),
                (
                    "/api/certificates/{certificate_id}/render-manifest",
                    "post",
                    "createIssuedCertificateRenderManifest",
                ),
                (
                    "/api/certificates/campaigns/{campaign_id}/render-manifests",
                    "post",
                    "createIssuedCertificateRenderManifests",
                ),
                ("/api/me/certificates", "get", "listOwnCertificates"),
                (
                    "/api/me/certificates/{certificate_id}",
                    "get",
                    "getOwnCertificate",
                ),
                (
                    "/api/me/certificates/{certificate_id}/render-manifest",
                    "post",
                    "createOwnCertificateRenderManifest",
                ),
                (
                    "/api/public/certificates/verify/manual",
                    "post",
                    "verifyCertificateManually",
                ),
                (
                    "/api/public/certificates/verify/qr",
                    "post",
                    "verifyCertificateByQr",
                ),
                (
                    "/api/public/certificates/render-manifest",
                    "post",
                    "createPublicCertificateRenderManifest",
                ),
            ],
        );

        let schemas = &document["components"]["schemas"];
        let detail = &schemas["CertificateCampaignDetail"];
        for field in [
            "academicYearId",
            "academicYearValue",
            "ownerOrganizationUnitId",
            "name",
            "eventDate",
            "status",
            "hasOpenIssueRequest",
            "capabilities",
            "updatedAt",
        ] {
            assert!(required(detail).contains(&field));
        }
        assert!(
            required(&schemas["CertificateCampaignCapabilities"]).contains(&"canManageTemplates")
        );
        assert!(
            required(&schemas["CertificateCampaignCapabilities"]).contains(&"canPrepareCandidates")
        );
        for field in [
            "ownerOrganizationUnitId",
            "ownerOrganizationUnitCode",
            "ownerOrganizationUnitName",
            "activitySequence",
            "createdBy",
            "updatedBy",
        ] {
            assert!(contains_null(&detail["properties"][field]));
        }

        let nullable_update = &schemas["NullableUuidUpdate"];
        assert!(required(nullable_update).contains(&"value"));
        assert!(contains_null(&nullable_update["properties"]["value"]));
        let template = &schemas["CertificateTemplateDetail"];
        for field in [
            "backgroundFileId",
            "pageGeometry",
            "allowedRecipientTypes",
            "layout",
            "assets",
            "isReady",
            "missingVariableCertificateCount",
            "capabilities",
        ] {
            assert!(required(template).contains(&field));
        }
        assert!(contains_null(&template["properties"]["backgroundFileId"]));
        assert!(contains_null(&template["properties"]["pageGeometry"]));

        let manifest = &schemas["CertificateRenderManifest"];
        for field in [
            "pageGeometry",
            "layout",
            "campaignValues",
            "recipientValues",
            "certificateNumber",
            "qrPayload",
            "builtInFonts",
            "fontGrants",
            "imageGrants",
            "backgroundGrant",
            "suggestedFilename",
        ] {
            assert!(required(manifest).contains(&field));
        }
        let account_properties = schemas["CertificateCandidateAccount"]["properties"]
            .as_object()
            .expect("candidate account schema properties");
        assert_eq!(
            account_properties.keys().cloned().collect::<BTreeSet<_>>(),
            [
                "userId",
                "recipientType",
                "studentId",
                "staffUsername",
                "title",
                "firstName",
                "lastName",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
        );
        for forbidden in [
            "nationalId",
            "email",
            "phone",
            "medicalConditions",
            "allergies",
            "guardian",
        ] {
            assert!(!account_properties.contains_key(forbidden));
        }
        for variant in schemas["CertificateCandidateBulkRequest"]["oneOf"]
            .as_array()
            .expect("candidate bulk operation variants")
        {
            let properties = variant["properties"]
                .as_object()
                .expect("candidate bulk variant properties");
            assert!(properties.contains_key("candidateIds"));
            assert!(!properties.contains_key("candidate_ids"));
        }
        let issue_outcomes = schemas["IssueCertificateOutcome"]["oneOf"]
            .as_array()
            .expect("certificate issue outcome variants");
        for (variant, expected_fields) in issue_outcomes.iter().zip([
            &[
                "issueRunId",
                "requestId",
                "campaignId",
                "activitySequence",
                "firstCertificateSequence",
                "lastCertificateSequence",
                "certificates",
            ][..],
            &[
                "issueRunId",
                "requestId",
                "campaignId",
                "issueCodes",
                "candidateProblems",
            ][..],
        ]) {
            let properties = variant["properties"]
                .as_object()
                .expect("certificate issue outcome properties");
            for field in expected_fields {
                assert!(properties.contains_key(*field));
            }
            assert!(properties.keys().all(|field| !field.contains('_')));
        }
        for (schema_name, required_fields) in [
            ("SubmitCertificateIssueRequest", &["candidateIds"][..]),
            (
                "ReturnCertificateIssueRequest",
                &["issueCodes", "returnNote"][..],
            ),
            (
                "CertificateIssueRequestDetail",
                &["campaignId", "status", "capabilities", "items"][..],
            ),
            ("CertificateResourceLocked", &["code", "requestId"][..]),
            ("IssueCertificateRequest", &["idempotencyKey"][..]),
            (
                "IssuedCertificateSummary",
                &[
                    "certificateNumber",
                    "firstName",
                    "lastName",
                    "status",
                    "capabilities",
                ][..],
            ),
            (
                "RevokeCertificateRequest",
                &["reason", "createReplacementCandidate"][..],
            ),
            (
                "CertificateRenderManifestBatchRequest",
                &["certificateIds"][..],
            ),
            (
                "ManualCertificateVerificationRequest",
                &["certificateNumber", "firstName", "lastName"][..],
            ),
            (
                "QrCertificateVerificationRequest",
                &["certificateNumber", "proof"][..],
            ),
            ("PublicCertificateRenderRequest", &["receipt"][..]),
        ] {
            let schema = &schemas[schema_name];
            for field in required_fields {
                assert!(required(schema).contains(field));
            }
        }
        for path in [
            "/api/public/certificates/verify/manual",
            "/api/public/certificates/verify/qr",
            "/api/public/certificates/render-manifest",
        ] {
            let operations = document["paths"][path]
                .as_object()
                .expect("public certificate path operations");
            assert!(operations.contains_key("post"));
            assert!(!operations.contains_key("get"));
        }
        for (path, method) in [
            ("/api/me/certificates", "get"),
            ("/api/me/certificates/{certificate_id}", "get"),
            (
                "/api/me/certificates/{certificate_id}/render-manifest",
                "post",
            ),
        ] {
            let operation = &document["paths"][path][method];
            let parameters = operation["parameters"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            assert!(parameters.iter().all(|parameter| {
                !matches!(
                    parameter["name"].as_str(),
                    Some("userId" | "user_id" | "targetUserId" | "target_user_id")
                )
            }));
            assert!(operation.get("requestBody").is_none());
        }
        let public_properties = schemas["PublicCertificateVerificationData"]["properties"]
            .as_object()
            .expect("public certificate verification properties");
        assert_eq!(
            public_properties.keys().cloned().collect::<BTreeSet<_>>(),
            [
                "academicYear",
                "activityItem",
                "awardOrRole",
                "campaignName",
                "certificateNumber",
                "firstName",
                "issueDate",
                "issuerSchoolName",
                "lastName",
                "receipt",
                "receiptExpiresAt",
                "replacementCertificateNumber",
                "status",
                "templateName",
                "title",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
        );
        let issued_properties = schemas["IssuedCertificateSummary"]["properties"]
            .as_object()
            .expect("issued certificate summary properties");
        for forbidden in [
            "studentId",
            "staffUsername",
            "nationalId",
            "qrProofEncrypted",
            "qrProofHash",
            "userId",
        ] {
            assert!(!issued_properties.contains_key(forbidden));
        }
        assert_eq!(
            document["paths"]["/api/certificates/campaigns/{campaign_id}/issue-requests"]["post"]
                ["responses"]["409"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponseWithData_CertificateResourceLocked"
        );
        let mutation_conflict_ref =
            "#/components/schemas/ApiErrorResponseWithOptionalData_CertificateResourceLocked";
        for (path, method) in [
            ("/api/certificates/campaigns/{campaign_id}", "put"),
            ("/api/certificates/campaigns/{campaign_id}/status", "put"),
            ("/api/certificates/templates/{template_id}", "put"),
            ("/api/certificates/templates/{template_id}", "delete"),
            (
                "/api/certificates/templates/{template_id}/background",
                "put",
            ),
            ("/api/certificates/templates/{template_id}/assets", "post"),
            (
                "/api/certificates/templates/{template_id}/fonts/batch",
                "post",
            ),
            (
                "/api/certificates/templates/{template_id}/assets/{asset_id}",
                "delete",
            ),
            (
                "/api/certificates/campaigns/{campaign_id}/candidates/bulk",
                "post",
            ),
            ("/api/certificates/candidates/{candidate_id}", "put"),
            ("/api/certificates/candidates/{candidate_id}", "delete"),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["409"]["content"]["application/json"]
                    ["schema"]["$ref"],
                mutation_conflict_ref,
                "{method} {path} must document optional typed lock data"
            );
        }
        assert_eq!(
            document["paths"]["/api/certificates/campaigns/{campaign_id}"]
                .as_object()
                .expect("campaign path")
                .get("delete"),
            None,
            "legacy draft-only DELETE endpoint must be removed"
        );
        assert_eq!(
            document["paths"]["/api/certificates/campaigns/{campaign_id}/purge-impact"]["get"]
                ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_CertificateCampaignPurgeImpact"
        );
        for path in [
            "/api/certificates/campaigns/{campaign_id}/purge",
            "/api/certificates/campaigns/{campaign_id}/purge/retry",
        ] {
            assert_eq!(
                document["paths"][path]["post"]["responses"]["202"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiResponse_CertificateCampaignPurgeStatus"
            );
        }
        assert_eq!(
            document["paths"]["/api/certificates/campaigns/{campaign_id}/purge-status"]["get"]
                ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_CertificateCampaignPurgeStatus"
        );
        assert_eq!(
            document["paths"]["/api/certificates/owner-options"]["get"]["responses"]["200"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_OrganizationUnitLookupItem"
        );
    }

    #[test]
    fn school_font_contracts_are_shared_and_forward_only() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/school-fonts", "get", "listSchoolFonts"),
                (
                    "/api/school-fonts/inspect",
                    "post",
                    "inspectSchoolFontUploads",
                ),
                ("/api/school-fonts/batch", "post", "attachSchoolFontBatch"),
                ("/api/school-fonts/{font_id}", "delete", "deleteSchoolFont"),
                (
                    "/api/certificates/templates/{template_id}/fonts",
                    "get",
                    "listCertificateSchoolFonts",
                ),
                (
                    "/api/certificates/templates/{template_id}/fonts/inspect",
                    "post",
                    "inspectCertificateFontUploads",
                ),
                (
                    "/api/certificates/templates/{template_id}/fonts/batch",
                    "post",
                    "attachCertificateFontBatch",
                ),
            ],
        );

        for legacy_path in [
            "/api/certificates/templates/{template_id}/assets/fonts/inspect",
            "/api/certificates/templates/{template_id}/assets/fonts/batch",
        ] {
            assert!(
                document["paths"].get(legacy_path).is_none(),
                "legacy template-owned font path must be absent: {legacy_path}"
            );
        }

        let schemas = &document["components"]["schemas"];
        for schema_name in [
            "SchoolFontStyle",
            "SchoolFontUploadStatus",
            "SchoolFontSummary",
            "SchoolFontListResponse",
            "InspectSchoolFontUploadsRequest",
            "AttachSchoolFontBatchRequest",
            "SchoolFontUploadInspectionFile",
            "SchoolFontUploadInspection",
            "SchoolFontDeleteConflict",
        ] {
            assert!(
                !schemas[schema_name].is_null(),
                "missing shared school-font schema {schema_name}"
            );
        }

        let font_source = serde_json::to_string(&schemas["CertificateFontSource"]).unwrap();
        assert!(font_source.contains("school_font"));
        assert!(font_source.contains("font_id"));
        assert!(!font_source.contains("asset_id"));

        let grant = &schemas["CertificateRenderFontGrant"];
        assert!(required(grant).contains(&"schoolFontId"));
        assert!(grant["properties"].get("assetId").is_none());

        let purposes = schemas["FilePurpose"]["enum"]
            .as_array()
            .expect("FilePurpose must be an enum");
        assert!(purposes.iter().any(|value| value == "school_font"));
        assert!(!purposes
            .iter()
            .any(|value| value == "certificate_template_font"));
    }

    #[test]
    fn documents_file_platform_without_provider_locators() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/files", "post", "uploadFile"),
                ("/api/files/{id}", "get", "getFileMetadata"),
                ("/api/files/{id}/download", "post", "downloadFile"),
                ("/api/files/{id}", "delete", "deleteFile"),
                (
                    "/api/public/files/{id}/content",
                    "get",
                    "getPublicFileContent",
                ),
                (
                    "/api/public/files/{id}/delivery",
                    "get",
                    "getPublicFileDelivery",
                ),
                (
                    "/api/admission/applications/{application_id}/documents",
                    "post",
                    "staffUploadAdmissionDocument",
                ),
                (
                    "/api/admission/applications/{application_id}/documents/{doc_type}",
                    "delete",
                    "staffDeleteAdmissionDocument",
                ),
                (
                    "/api/admission/portal/upload",
                    "post",
                    "portalUploadAdmissionDocument",
                ),
                (
                    "/api/admission/portal/documents/{doc_type}",
                    "delete",
                    "portalDeleteAdmissionDocument",
                ),
                (
                    "/api/admission/portal/documents/{file_id}/download",
                    "post",
                    "portalDownloadAdmissionDocument",
                ),
            ],
        );

        let schemas = &document["components"]["schemas"];
        let metadata = &schemas["FileMetadata"];
        for required_field in [
            "id",
            "purpose",
            "lifecycleStatus",
            "displayFilename",
            "detectedMimeType",
            "byteSize",
            "currentVersion",
            "publicContentUrl",
        ] {
            assert!(required(metadata).contains(&required_field));
        }
        let serialized = serde_json::to_string(metadata).unwrap();
        for forbidden in [
            "storagePath",
            "thumbnailPath",
            "objectKey",
            "bucket",
            "provider",
            "checksum",
            "signedUrl",
            "inspectionMetadata",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "FileMetadata must not expose {forbidden}"
            );
        }

        let file_purposes = schemas["FilePurpose"]["enum"]
            .as_array()
            .expect("FilePurpose must be an enum");
        for purpose in [
            "certificate_template_background",
            "certificate_template_image",
            "school_font",
        ] {
            assert!(
                file_purposes.iter().any(|value| value == purpose),
                "FilePurpose must expose {purpose}"
            );
        }
        assert!(!file_purposes
            .iter()
            .any(|value| value == "certificate_template_font"));

        let grant = &schemas["FileDownloadGrantResponse"];
        assert!(required(grant).contains(&"url"));
        assert!(required(grant).contains(&"expiresAt"));
        assert_eq!(grant["properties"]["expiresAt"]["format"], "date-time");
        for forbidden in ["bucket", "objectKey", "provider"] {
            assert!(
                grant["properties"].get(forbidden).is_none(),
                "download grant must not expose {forbidden}"
            );
        }

        let public_delivery = &schemas["PublicFileDeliveryResponse"];
        assert!(required(public_delivery).contains(&"url"));
        assert_eq!(public_delivery["properties"].as_object().unwrap().len(), 1);
        assert_eq!(
            document["paths"]["/api/public/files/{id}/delivery"]["get"]["responses"]["200"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_PublicFileDeliveryResponse",
        );

        assert_eq!(
            document["paths"]["/api/files"]["post"]["responses"]["201"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_FileMetadata",
        );
        assert_eq!(
            document["paths"]["/api/files/{id}"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_FileMetadata",
        );
        for path in [
            "/api/files/{id}/download",
            "/api/admission/portal/documents/{file_id}/download",
        ] {
            assert_eq!(
                document["paths"][path]["post"]["responses"]["200"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiResponse_FileDownloadGrantResponse",
            );
            assert!(
                document["paths"][path]["post"]["responses"]
                    .get("303")
                    .is_none(),
                "{path} must return a typed grant instead of a browser redirect"
            );
        }
        assert!(
            document["paths"]["/api/public/files/{id}/content"]["get"]["responses"]["307"]
                ["content"]
                .is_null()
        );
        for (path, method) in [
            ("/api/admission/portal/documents/{doc_type}", "delete"),
            ("/api/admission/portal/documents/{file_id}/download", "post"),
        ] {
            let operation = &document["paths"][path][method];
            assert_eq!(
                operation["requestBody"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/PortalCredentials"
            );
            let parameter_names = operation["parameters"]
                .as_array()
                .expect("portal document parameters")
                .iter()
                .filter_map(|parameter| parameter["name"].as_str())
                .collect::<Vec<_>>();
            assert!(!parameter_names.contains(&"national_id"));
            assert!(!parameter_names.contains(&"date_of_birth"));
        }
    }
}
