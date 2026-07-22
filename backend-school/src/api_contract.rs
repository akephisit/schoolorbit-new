use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData, UuidIdData};
use crate::modules::academic::handlers::activity::{
    ActivityAddedCountData, ActivityDeletedCountData, ActivityInsertedCountData,
    ActivityProcessedCountData, AddSlotInstructorRequest, AddSlotInstructorsBatchRequest,
    InstructorRoleRequest,
};
use crate::modules::academic::handlers::course_planning::CourseAssignedCountData;
use crate::modules::academic::handlers::study_plans::{CountData, GenerateCoursesData};
use crate::modules::academic::handlers::timetable::TimetableItemsData;
use crate::modules::academic::models::activity::{
    ActivityGroup, ActivityGroupFilter, ActivityGroupInstructorRole, ActivityGroupMember,
    ActivityMemberResult, ActivityRegistrationType, ActivitySlot, ActivitySlotFilter,
    AddMembersRequest, BatchUpsertSlotClassroomAssignmentsRequest, CreateActivityGroupRequest,
    SlotClassroomAssignment, UpdateActivityGroupRequest, UpdateActivitySlotRequest,
    UpdateMemberResultRequest, UpsertSlotClassroomAssignmentRequest,
};
use crate::modules::academic::models::course_planning::{
    AddCourseInstructorRequest, AssignCoursesRequest, BatchListCourseInstructorsQuery,
    BatchListCourseInstructorsRequest, ClassroomActivityQuery, ClassroomCourse,
    ClassroomCourseSettings, CourseInstructor, CourseInstructorRole, PlanQuery,
    UpdateCourseInstructorRoleRequest, UpdateCourseRequest,
};
use crate::modules::academic::models::curriculum::{
    AddSubjectDefaultInstructorRequest, CreateSubjectRequest, CurriculumInstructorRole,
    DefaultInstructorInput, Subject, SubjectDefaultInstructor, SubjectGroup, SubjectType,
    UpdateSubjectDefaultInstructorRoleRequest, UpdateSubjectRequest,
};
use crate::modules::academic::models::exam_schedule::{
    PersonalExamScheduleRound, PersonalExamSessionView,
};
use crate::modules::academic::models::study_plans::{
    ActivityCatalog, ActivityCatalogType, ActivitySchedulingMode,
    AddCatalogDefaultInstructorRequest, AddSubjectsToVersionRequest, CatalogDefaultInstructor,
    CatalogDefaultInstructorInput, CreateCatalogRequest, CreatePlanActivityRequest,
    CreateStudyPlanRequest, CreateStudyPlanVersionRequest, GenerateActivitiesFromPlanRequest,
    GenerateCoursesFromPlanRequest, GenerateCoursesResponse, StudyPlan, StudyPlanSubject,
    StudyPlanVersion, StudyPlanVersionActivity, SubjectInPlan,
    UpdateCatalogDefaultInstructorRoleRequest, UpdateCatalogRequest, UpdatePlanActivityRequest,
    UpdateStudyPlanRequest, UpdateStudyPlanVersionRequest,
};
use crate::modules::academic::models::timetable::TimetableEntry;
use crate::modules::academic::models::{
    AcademicYear, Classroom, ClassroomAdvisor, ClassroomAdvisorInput, ClassroomAdvisorRole,
    CreateAcademicYearRequest, CreateClassroomRequest, CreateGradeLevelRequest,
    CreateSemesterRequest, EnrollStudentRequest, GradeLevelResponse, GradeLevelType, Semester,
    StudentEnrollment, UpdateAcademicYearRequest, UpdateClassroomRequest, UpdateSemesterRequest,
    UpdateYearLevelsRequest,
};
use crate::modules::academic::services::academic_structure_service::AcademicStructure;
use crate::modules::academic::services::activity_service::{InstructorInfo, SlotInstructorInfo};
use crate::modules::academic::services::course_planning_service::ClassroomActivity;
use crate::modules::academic::services::study_plan_service::GenerateActivitiesFromPlanOutcome;
use crate::modules::achievement::models::{
    Achievement, AchievementListFilter, CreateAchievementRequest, UpdateAchievementRequest,
};
use crate::modules::auth::models::{
    ChangePasswordRequest, LoginData, LoginRequest, ProfileResponse, UpdateProfileRequest,
    UserResponse,
};
use crate::modules::calendar::models::{
    CalendarCategory, CalendarEvent, CalendarEventReminder, CalendarEventTag, CalendarEventTarget,
    CalendarPublicEvent, CalendarTag, CalendarViewerEvent,
};
use crate::modules::facility::models::Room;
use crate::modules::lookup::models::{
    AcademicYearLookupItem, ClassroomLookupItem, GradeLevelLookupItem, LookupItem,
    OrganizationUnitLookupItem, RoleLookupItem, StaffLookupItem, StudentLookupItem,
};
use crate::modules::menu::handlers::public::UserMenuData;
use crate::modules::menu::models::{
    FeatureToggle, MenuGroup, MenuGroupResponse, MenuItem, MenuItemResponse,
};
use crate::modules::notification::models::{ListNotificationsResponse, Notification};
use crate::modules::parents::models::{ChildDto, ParentProfile};
use crate::modules::school::handlers::PublicSchoolInfoData;
use crate::modules::school::models::SchoolSettingsResponse;
use crate::modules::staff::handlers::organization_delegations::{
    CreateDelegationRequest, DelegationIdData, DelegationItem,
};
use crate::modules::staff::handlers::organization_members::{
    AddMemberRequest, ListMembersQuery, OrganizationMemberItem, UpdateMemberRequest,
};
use crate::modules::staff::handlers::staff::StaffListData;
use crate::modules::staff::models::{
    AdvisorClassroomItem, AssignRoleRequest, CreateOrganizationUnitRequest, CreateRoleRequest,
    CreateStaffInfoRequest, CreateStaffRequest, OrganizationAssignment,
    OrganizationPermissionGrantInput, OrganizationUnit, OrganizationUnitResponse, Permission, Role,
    RoleResponse, StaffInfoResponse, StaffListItem, StaffProfileResponse, TeachingCourseItem,
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
    StudentProfile, UpdateOwnProfileRequest, UpdateStudentRequest,
};
use crate::modules::system::handlers::feature_toggles::{
    FeatureListResponse, FeatureToggleResponse,
};
use serde_json::Value;
use std::collections::HashMap;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::modules::auth::handlers::login,
        crate::modules::auth::handlers::logout,
        crate::modules::auth::handlers::me,
        crate::modules::auth::handlers::get_profile,
        crate::modules::auth::handlers::update_profile,
        crate::modules::auth::handlers::change_password,
        crate::modules::menu::handlers::public::get_user_menu,
        crate::modules::system::handlers::feature_toggles::list_features,
        crate::modules::system::handlers::feature_toggles::get_feature,
        crate::modules::menu::handlers::admin::list_menu_groups,
        crate::modules::menu::handlers::admin::list_menu_items,
        crate::modules::lookup::handlers::lookup_staff,
        crate::modules::lookup::handlers::lookup_students,
        crate::modules::lookup::handlers::lookup_rooms,
        crate::modules::lookup::handlers::lookup_roles,
        crate::modules::lookup::handlers::lookup_organization_units,
        crate::modules::lookup::handlers::lookup_organization_unit_by_id,
        crate::modules::lookup::handlers::lookup_grade_levels,
        crate::modules::lookup::handlers::lookup_classrooms,
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
        crate::modules::students::handlers::create_student,
        crate::modules::students::handlers::update_student,
        crate::modules::students::handlers::delete_student,
        crate::modules::students::handlers_parents::add_parent_to_student,
        crate::modules::students::handlers_parents::remove_parent_from_student,
        crate::modules::achievement::handlers::list_achievements,
        crate::modules::achievement::handlers::create_achievement,
        crate::modules::achievement::handlers::update_achievement,
        crate::modules::achievement::handlers::delete_achievement,
        crate::modules::parents::handlers::get_own_parent_profile,
        crate::modules::parents::handlers::get_child_profile,
        crate::modules::parents::handlers::get_child_timetable,
        crate::modules::parents::handlers::get_child_exam_schedule,
        crate::modules::parents::handlers::get_child_calendar_events,
        crate::modules::academic::handlers::timetable::get_my_timetable,
        crate::modules::academic::handlers::exam_schedule::list_my_exam_schedule,
        crate::modules::academic::handlers::exam_schedule::list_staff_exam_schedule,
        crate::modules::academic::handlers::list_academic_structure,
        crate::modules::academic::handlers::create_grade_level,
        crate::modules::academic::handlers::delete_grade_level,
        crate::modules::academic::handlers::create_academic_year,
        crate::modules::academic::handlers::update_academic_year,
        crate::modules::academic::handlers::toggle_active_year,
        crate::modules::academic::handlers::get_year_levels,
        crate::modules::academic::handlers::update_year_levels,
        crate::modules::academic::handlers::create_semester,
        crate::modules::academic::handlers::update_semester,
        crate::modules::academic::handlers::delete_semester,
        crate::modules::academic::handlers::list_classrooms,
        crate::modules::academic::handlers::create_classroom,
        crate::modules::academic::handlers::update_classroom,
        crate::modules::academic::handlers::enroll_students,
        crate::modules::academic::handlers::get_class_enrollments,
        crate::modules::academic::handlers::remove_enrollment,
        crate::modules::academic::handlers::update_enrollment_number,
        crate::modules::academic::handlers::auto_assign_class_numbers,
        crate::modules::academic::handlers::subjects::list_subject_groups,
        crate::modules::academic::handlers::subjects::batch_list_subject_default_instructors,
        crate::modules::academic::handlers::subjects::list_subjects,
        crate::modules::academic::handlers::subjects::create_subject,
        crate::modules::academic::handlers::subjects::update_subject,
        crate::modules::academic::handlers::subjects::delete_subject,
        crate::modules::academic::handlers::subjects::list_subject_default_instructors,
        crate::modules::academic::handlers::subjects::add_subject_default_instructor,
        crate::modules::academic::handlers::subjects::remove_subject_default_instructor,
        crate::modules::academic::handlers::subjects::update_subject_default_instructor_role,
        crate::modules::academic::handlers::study_plans::list_study_plans,
        crate::modules::academic::handlers::study_plans::get_study_plan,
        crate::modules::academic::handlers::study_plans::create_study_plan,
        crate::modules::academic::handlers::study_plans::update_study_plan,
        crate::modules::academic::handlers::study_plans::delete_study_plan,
        crate::modules::academic::handlers::study_plans::list_study_plan_versions,
        crate::modules::academic::handlers::study_plans::get_study_plan_version,
        crate::modules::academic::handlers::study_plans::create_study_plan_version,
        crate::modules::academic::handlers::study_plans::update_study_plan_version,
        crate::modules::academic::handlers::study_plans::delete_study_plan_version,
        crate::modules::academic::handlers::study_plans::list_study_plan_subjects,
        crate::modules::academic::handlers::study_plans::add_subjects_to_version,
        crate::modules::academic::handlers::study_plans::delete_study_plan_subject,
        crate::modules::academic::handlers::study_plans::generate_courses_from_plan,
        crate::modules::academic::handlers::study_plans::list_plan_activities,
        crate::modules::academic::handlers::study_plans::add_plan_activity,
        crate::modules::academic::handlers::study_plans::update_plan_activity,
        crate::modules::academic::handlers::study_plans::delete_plan_activity,
        crate::modules::academic::handlers::study_plans::generate_activities_from_plan,
        crate::modules::academic::handlers::study_plans::list_activity_catalog,
        crate::modules::academic::handlers::study_plans::create_activity_catalog,
        crate::modules::academic::handlers::study_plans::update_activity_catalog,
        crate::modules::academic::handlers::study_plans::delete_activity_catalog,
        crate::modules::academic::handlers::study_plans::list_catalog_default_instructors,
        crate::modules::academic::handlers::study_plans::add_catalog_default_instructor,
        crate::modules::academic::handlers::study_plans::remove_catalog_default_instructor,
        crate::modules::academic::handlers::study_plans::update_catalog_default_instructor_role,
        crate::modules::academic::handlers::activity::list_activity_slots,
        crate::modules::academic::handlers::activity::update_activity_slot,
        crate::modules::academic::handlers::activity::delete_activity_slot,
        crate::modules::academic::handlers::activity::list_slot_instructors,
        crate::modules::academic::handlers::activity::add_slot_instructor,
        crate::modules::academic::handlers::activity::add_slot_instructors_batch,
        crate::modules::academic::handlers::activity::remove_slot_instructor,
        crate::modules::academic::handlers::activity::remove_all_slot_instructors,
        crate::modules::academic::handlers::activity::delete_all_slot_groups,
        crate::modules::academic::handlers::activity::delete_slot_timetable_entries,
        crate::modules::academic::handlers::activity::list_slot_classroom_assignments,
        crate::modules::academic::handlers::activity::batch_upsert_slot_classroom_assignments,
        crate::modules::academic::handlers::activity::delete_all_slot_classroom_assignments,
        crate::modules::academic::handlers::activity::delete_slot_classroom_assignment,
        crate::modules::academic::handlers::activity::list_activity_groups,
        crate::modules::academic::handlers::activity::create_activity_group,
        crate::modules::academic::handlers::activity::update_activity_group,
        crate::modules::academic::handlers::activity::delete_activity_group,
        crate::modules::academic::handlers::activity::list_members,
        crate::modules::academic::handlers::activity::add_members,
        crate::modules::academic::handlers::activity::remove_member,
        crate::modules::academic::handlers::activity::update_member_result,
        crate::modules::academic::handlers::activity::list_instructors,
        crate::modules::academic::handlers::activity::add_instructor,
        crate::modules::academic::handlers::activity::remove_instructor,
        crate::modules::academic::handlers::activity::my_enrollments,
        crate::modules::academic::handlers::activity::self_enroll,
        crate::modules::academic::handlers::activity::self_unenroll,
        crate::modules::academic::handlers::course_planning::list_classroom_courses,
        crate::modules::academic::handlers::course_planning::assign_courses,
        crate::modules::academic::handlers::course_planning::update_course,
        crate::modules::academic::handlers::course_planning::remove_course,
        crate::modules::academic::handlers::course_planning::batch_list_course_instructors,
        crate::modules::academic::handlers::course_planning::batch_list_course_instructors_from_query,
        crate::modules::academic::handlers::course_planning::list_course_instructors,
        crate::modules::academic::handlers::course_planning::add_course_instructor,
        crate::modules::academic::handlers::course_planning::update_course_instructor_role,
        crate::modules::academic::handlers::course_planning::remove_course_instructor,
        crate::modules::academic::handlers::course_planning::list_classroom_activities,
        crate::modules::academic::handlers::course_planning::remove_classroom_from_slot,
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
        UserResponse,
        LoginRequest,
        LoginData,
        ProfileResponse,
        UpdateProfileRequest,
        ChangePasswordRequest,
        ApiResponse<LoginData>,
        ApiResponse<ProfileResponse>,
        ApiResponse<UserResponse>,
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
        ClassroomLookupItem,
        AcademicYearLookupItem,
        StudentLookupItem,
        Room,
        ApiResponse<Vec<LookupItem>>,
        ApiResponse<Vec<StaffLookupItem>>,
        ApiResponse<Vec<RoleLookupItem>>,
        ApiResponse<Vec<OrganizationUnitLookupItem>>,
        ApiResponse<OrganizationUnitLookupItem>,
        ApiResponse<Vec<GradeLevelLookupItem>>,
        ApiResponse<Vec<ClassroomLookupItem>>,
        ApiResponse<Vec<AcademicYearLookupItem>>,
        ApiResponse<Vec<StudentLookupItem>>,
        ApiResponse<Vec<Room>>,
        MenuItemResponse,
        MenuGroupResponse,
        UserMenuData,
        ApiResponse<UserMenuData>,
        MenuGroup,
        MenuItem,
        ApiResponse<Vec<MenuGroup>>,
        ApiResponse<Vec<MenuItem>>,
        FeatureToggle,
        FeatureListResponse,
        FeatureToggleResponse,
        StaffListItem,
        StaffListData,
        StaffDashboardOverview,
        RoleResponse,
        OrganizationUnitResponse,
        TeachingCourseItem,
        AdvisorClassroomItem,
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
        AcademicStructure,
        AcademicYear,
        CreateAcademicYearRequest,
        UpdateAcademicYearRequest,
        Semester,
        CreateSemesterRequest,
        UpdateSemesterRequest,
        GradeLevelResponse,
        CreateGradeLevelRequest,
        Classroom,
        ClassroomAdvisor,
        ClassroomAdvisorInput,
        ClassroomAdvisorRole,
        GradeLevelType,
        CreateClassroomRequest,
        UpdateClassroomRequest,
        StudentEnrollment,
        EnrollStudentRequest,
        UpdateYearLevelsRequest,
        crate::modules::academic::handlers::UpdateEnrollmentNumberRequest,
        crate::modules::academic::handlers::AutoAssignClassNumbersRequest,
        ApiResponse<AcademicStructure>,
        ApiResponse<AcademicYear>,
        ApiResponse<Semester>,
        ApiResponse<GradeLevelResponse>,
        ApiResponse<Vec<Classroom>>,
        ApiResponse<Classroom>,
        ApiResponse<Vec<StudentEnrollment>>,
        SubjectType,
        CurriculumInstructorRole,
        SubjectGroup,
        Subject,
        DefaultInstructorInput,
        CreateSubjectRequest,
        UpdateSubjectRequest,
        SubjectDefaultInstructor,
        AddSubjectDefaultInstructorRequest,
        UpdateSubjectDefaultInstructorRoleRequest,
        StudyPlan,
        CreateStudyPlanRequest,
        UpdateStudyPlanRequest,
        StudyPlanVersion,
        CreateStudyPlanVersionRequest,
        UpdateStudyPlanVersionRequest,
        StudyPlanSubject,
        AddSubjectsToVersionRequest,
        SubjectInPlan,
        GenerateCoursesFromPlanRequest,
        GenerateCoursesResponse,
        CountData<usize>,
        GenerateCoursesData,
        ApiResponse<Vec<SubjectGroup>>,
        ApiResponse<Vec<Subject>>,
        ApiResponse<Subject>,
        ApiResponse<Vec<SubjectDefaultInstructor>>,
        ApiResponse<std::collections::HashMap<String, Vec<SubjectDefaultInstructor>>>,
        ApiResponse<Vec<StudyPlan>>,
        ApiResponse<StudyPlan>,
        ApiResponse<Vec<StudyPlanVersion>>,
        ApiResponse<StudyPlanVersion>,
        ApiResponse<Vec<StudyPlanSubject>>,
        ApiResponse<CountData<usize>>,
        ApiResponse<GenerateCoursesData>,
        CourseInstructorRole,
        ClassroomCourse,
        ClassroomCourseSettings,
        PlanQuery,
        AssignCoursesRequest,
        UpdateCourseRequest,
        CourseInstructor,
        AddCourseInstructorRequest,
        BatchListCourseInstructorsRequest,
        BatchListCourseInstructorsQuery,
        UpdateCourseInstructorRoleRequest,
        ClassroomActivityQuery,
        ClassroomActivity,
        CourseAssignedCountData,
        ApiResponse<Vec<ClassroomCourse>>,
        ApiResponse<Vec<CourseInstructor>>,
        ApiResponse<HashMap<String, Vec<CourseInstructor>>>,
        ApiResponse<Vec<ClassroomActivity>>,
        ApiResponse<CourseAssignedCountData>,
        ActivityCatalogType,
        ActivitySchedulingMode,
        StudyPlanVersionActivity,
        CreatePlanActivityRequest,
        UpdatePlanActivityRequest,
        GenerateActivitiesFromPlanRequest,
        ActivityCatalog,
        CatalogDefaultInstructorInput,
        CreateCatalogRequest,
        UpdateCatalogRequest,
        CatalogDefaultInstructor,
        AddCatalogDefaultInstructorRequest,
        UpdateCatalogDefaultInstructorRoleRequest,
        ActivitySlot,
        ActivityRegistrationType,
        ActivitySlotFilter,
        UpdateActivitySlotRequest,
        ActivityGroup,
        ActivityGroupFilter,
        CreateActivityGroupRequest,
        UpdateActivityGroupRequest,
        ActivityGroupMember,
        ActivityMemberResult,
        ActivityGroupInstructorRole,
        AddMembersRequest,
        UpdateMemberResultRequest,
        InstructorInfo,
        SlotInstructorInfo,
        AddSlotInstructorRequest,
        AddSlotInstructorsBatchRequest,
        InstructorRoleRequest,
        SlotClassroomAssignment,
        UpsertSlotClassroomAssignmentRequest,
        BatchUpsertSlotClassroomAssignmentsRequest,
        ActivityInsertedCountData,
        ActivityAddedCountData,
        ActivityDeletedCountData,
        ActivityProcessedCountData,
        ApiResponse<Vec<ActivitySlot>>,
        ApiResponse<ActivitySlot>,
        ApiResponse<Vec<ActivityGroup>>,
        ApiResponse<ActivityGroup>,
        ApiResponse<Vec<ActivityGroupMember>>,
        ApiResponse<Vec<InstructorInfo>>,
        ApiResponse<Vec<SlotInstructorInfo>>,
        ApiResponse<Vec<SlotClassroomAssignment>>,
        ApiResponse<ActivityInsertedCountData>,
        ApiResponse<ActivityAddedCountData>,
        ApiResponse<ActivityDeletedCountData>,
        ApiResponse<ActivityProcessedCountData>,
        GenerateActivitiesFromPlanOutcome,
        ApiResponse<Vec<StudyPlanVersionActivity>>,
        ApiResponse<StudyPlanVersionActivity>,
        ApiResponse<Vec<ActivityCatalog>>,
        ApiResponse<ActivityCatalog>,
        ApiResponse<Vec<CatalogDefaultInstructor>>,
        ApiResponse<GenerateActivitiesFromPlanOutcome>,
        ChildDto,
        ParentProfile,
        ApiResponse<StudentProfile>,
        ApiResponse<ParentProfile>,
        TimetableEntry,
        TimetableItemsData,
        ApiResponse<Vec<TimetableEntry>>,
        ApiResponse<TimetableItemsData>,
        PersonalExamScheduleRound,
        PersonalExamSessionView,
        ApiResponse<Vec<PersonalExamScheduleRound>>,
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
        (name = "school", description = "School settings and public branding reads"),
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
    use std::collections::HashSet;

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
            "#/components/schemas/ApiResponse_UserResponse"
        );
        assert_eq!(
            error_response["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );

        let success_schema = &document["components"]["schemas"]["ApiResponse_UserResponse"];
        assert_eq!(required(success_schema), vec!["data", "success"]);
        assert_eq!(success_schema["properties"]["success"]["type"], "boolean");
        assert_eq!(
            success_schema["properties"]["data"],
            document["components"]["schemas"]["UserResponse"]
        );

        let error_schema = &document["components"]["schemas"]["ApiErrorResponse"];
        assert_eq!(required(error_schema), vec!["error", "success"]);
        assert_eq!(error_schema["properties"]["success"]["type"], "boolean");
        assert_eq!(error_schema["properties"]["error"]["type"], "string");
    }

    #[test]
    fn current_user_schema_matches_serde() {
        let document = school_api_value().expect("document should serialize");
        let schema = &document["components"]["schemas"]["UserResponse"];

        assert_eq!(
            required(schema),
            vec![
                "createdAt",
                "email",
                "firstName",
                "id",
                "lastName",
                "nationalId",
                "phone",
                "profileImageUrl",
                "status",
                "userType",
                "username",
            ]
        );

        let properties = schema["properties"]
            .as_object()
            .expect("properties must exist");
        assert_eq!(properties["id"]["format"], "uuid");
        assert_eq!(properties["createdAt"]["format"], "date-time");

        for field in ["nationalId", "email", "phone", "profileImageUrl"] {
            assert!(
                contains_null(&properties[field]),
                "{field} must accept null"
            );
        }

        for field in ["primaryRoleName", "permissions"] {
            assert!(!required(schema).contains(&field));
            assert!(
                !contains_null(&properties[field]),
                "{field} is omitted, not null"
            );
        }
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
            document["paths"]["/api/auth/login"]["post"]["responses"]["422"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert!(document["paths"]["/api/auth/login"]["post"]["responses"]["400"].is_null());

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
            "profileImageUrl",
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
        assert!(update.get("profileImageUrl").is_some());
        let change = &schemas["ChangePasswordRequest"];
        assert_eq!(required(change), vec!["currentPassword", "newPassword"]);
        assert_eq!(
            document["paths"]["/api/auth/me/change-password"]["post"]["responses"]["404"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert!(
            document["paths"]["/api/auth/me/change-password"]["post"]["responses"]["400"].is_null()
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
            "image_path",
            "created_by",
            "user_first_name",
            "user_last_name",
            "user_profile_image_url",
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
    fn academic_structure_mutation_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/academic/structure", "get", "getAcademicStructure"),
                ("/api/academic/levels", "post", "createGradeLevel"),
                ("/api/academic/levels/{id}", "delete", "deleteGradeLevel"),
                ("/api/academic/years", "post", "createAcademicYear"),
                ("/api/academic/years/{id}", "put", "updateAcademicYear"),
                (
                    "/api/academic/years/{id}/active",
                    "put",
                    "setActiveAcademicYear",
                ),
                (
                    "/api/academic/years/{id}/levels",
                    "get",
                    "getAcademicYearLevels",
                ),
                (
                    "/api/academic/years/{id}/levels",
                    "put",
                    "updateAcademicYearLevels",
                ),
                ("/api/academic/semesters", "post", "createSemester"),
                ("/api/academic/semesters/{id}", "put", "updateSemester"),
                ("/api/academic/semesters/{id}", "delete", "deleteSemester"),
                ("/api/academic/classrooms", "get", "listClassrooms"),
                ("/api/academic/classrooms", "post", "createClassroom"),
                ("/api/academic/classrooms/{id}", "put", "updateClassroom"),
                ("/api/academic/enrollments", "post", "enrollStudents"),
                (
                    "/api/academic/enrollments/class/{id}",
                    "get",
                    "listClassEnrollments",
                ),
                (
                    "/api/academic/enrollments/{id}",
                    "delete",
                    "removeEnrollment",
                ),
                (
                    "/api/academic/enrollments/{id}/number",
                    "put",
                    "updateEnrollmentNumber",
                ),
                (
                    "/api/academic/enrollments/class/{id}/auto-number",
                    "post",
                    "autoAssignClassNumbers",
                ),
            ],
        );

        let create_year = &document["paths"]["/api/academic/years"]["post"];
        assert_eq!(
            create_year["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreateAcademicYearRequest"
        );
        assert_eq!(
            create_year["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_AcademicYear"
        );

        let create_classroom = &document["paths"]["/api/academic/classrooms"]["post"];
        assert_eq!(
            create_classroom["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreateClassroomRequest"
        );
        assert_eq!(
            create_classroom["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Classroom"
        );

        let enroll = &document["paths"]["/api/academic/enrollments"]["post"];
        assert_eq!(
            enroll["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/EnrollStudentRequest"
        );
        for (path, method) in [
            ("/api/academic/levels/{id}", "delete"),
            ("/api/academic/years/{id}/active", "put"),
            ("/api/academic/years/{id}/levels", "put"),
            ("/api/academic/semesters/{id}", "delete"),
            ("/api/academic/enrollments", "post"),
            ("/api/academic/enrollments/{id}", "delete"),
            ("/api/academic/enrollments/{id}/number", "put"),
            ("/api/academic/enrollments/class/{id}/auto-number", "post"),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiResponse_EmptyData",
                "incorrect empty success envelope for {method} {path}"
            );
        }

        let schemas = &document["components"]["schemas"];
        assert_eq!(
            schemas["AcademicYear"]["properties"]["id"]["format"],
            "uuid"
        );
        assert_eq!(
            schemas["AcademicYear"]["properties"]["start_date"]["format"],
            "date"
        );
        assert_eq!(
            schemas["Classroom"]["properties"]["advisors"]["items"]["$ref"],
            "#/components/schemas/ClassroomAdvisor"
        );
        assert_eq!(
            schemas["GradeLevelResponse"]["properties"]["level_type"]["$ref"],
            "#/components/schemas/GradeLevelType"
        );
        assert_eq!(
            schemas["ClassroomAdvisor"]["properties"]["role"]["$ref"],
            "#/components/schemas/ClassroomAdvisorRole"
        );
        assert_eq!(
            schemas["StudentEnrollment"]["properties"]["enrollment_date"]["format"],
            "date"
        );

        let operation_count = document["paths"]
            .as_object()
            .expect("paths must be an object")
            .values()
            .flat_map(|path| path.as_object().expect("path item").values())
            .filter(|operation| operation.get("operationId").is_some())
            .count();
        assert_eq!(operation_count, 177);
    }

    #[test]
    fn academic_curriculum_core_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/academic/subjects/groups", "get", "listSubjectGroups"),
                (
                    "/api/academic/subjects/default-instructors",
                    "get",
                    "batchListSubjectDefaultInstructors",
                ),
                ("/api/academic/subjects", "get", "listSubjects"),
                ("/api/academic/subjects", "post", "createSubject"),
                ("/api/academic/subjects/{id}", "put", "updateSubject"),
                ("/api/academic/subjects/{id}", "delete", "deleteSubject"),
                (
                    "/api/academic/subjects/{id}/default-instructors",
                    "get",
                    "listSubjectDefaultInstructors",
                ),
                (
                    "/api/academic/subjects/{id}/default-instructors",
                    "post",
                    "addSubjectDefaultInstructor",
                ),
                (
                    "/api/academic/subjects/{id}/default-instructors/{uid}",
                    "delete",
                    "removeSubjectDefaultInstructor",
                ),
                (
                    "/api/academic/subjects/{id}/default-instructors/{uid}",
                    "put",
                    "updateSubjectDefaultInstructorRole",
                ),
                ("/api/academic/study-plans", "get", "listStudyPlans"),
                ("/api/academic/study-plans", "post", "createStudyPlan"),
                ("/api/academic/study-plans/{id}", "get", "getStudyPlan"),
                ("/api/academic/study-plans/{id}", "put", "updateStudyPlan"),
                (
                    "/api/academic/study-plans/{id}",
                    "delete",
                    "deleteStudyPlan",
                ),
                (
                    "/api/academic/study-plan-versions",
                    "get",
                    "listStudyPlanVersions",
                ),
                (
                    "/api/academic/study-plan-versions",
                    "post",
                    "createStudyPlanVersion",
                ),
                (
                    "/api/academic/study-plan-versions/{id}",
                    "get",
                    "getStudyPlanVersion",
                ),
                (
                    "/api/academic/study-plan-versions/{id}",
                    "put",
                    "updateStudyPlanVersion",
                ),
                (
                    "/api/academic/study-plan-versions/{id}",
                    "delete",
                    "deleteStudyPlanVersion",
                ),
                (
                    "/api/academic/study-plan-versions/{id}/subjects",
                    "get",
                    "listStudyPlanSubjects",
                ),
                (
                    "/api/academic/study-plan-versions/{id}/subjects",
                    "post",
                    "addSubjectsToStudyPlanVersion",
                ),
                (
                    "/api/academic/study-plan-subjects/{id}",
                    "delete",
                    "deleteStudyPlanSubject",
                ),
                (
                    "/api/academic/planning/generate-from-plan",
                    "post",
                    "generateCoursesFromStudyPlan",
                ),
            ],
        );

        let create_subject = &document["paths"]["/api/academic/subjects"]["post"];
        assert_eq!(
            create_subject["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreateSubjectRequest"
        );
        assert_eq!(
            create_subject["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Subject"
        );

        let create_plan = &document["paths"]["/api/academic/study-plans"]["post"];
        assert_eq!(
            create_plan["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreateStudyPlanRequest"
        );
        assert_eq!(
            create_plan["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_StudyPlan"
        );

        for (path, method) in [
            ("/api/academic/subjects/{id}", "delete"),
            ("/api/academic/subjects/{id}/default-instructors", "post"),
            (
                "/api/academic/subjects/{id}/default-instructors/{uid}",
                "delete",
            ),
            (
                "/api/academic/subjects/{id}/default-instructors/{uid}",
                "put",
            ),
            ("/api/academic/study-plans/{id}", "delete"),
            ("/api/academic/study-plan-versions/{id}", "delete"),
            ("/api/academic/study-plan-subjects/{id}", "delete"),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiResponse_EmptyData",
                "incorrect empty success envelope for {method} {path}"
            );
        }

        let operation_count = document["paths"]
            .as_object()
            .expect("paths must be an object")
            .values()
            .flat_map(|path| path.as_object().expect("path item").values())
            .filter(|operation| operation.get("operationId").is_some())
            .count();
        assert_eq!(operation_count, 177);
    }

    #[test]
    fn academic_activity_template_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/academic/study-plan-versions/{id}/activities",
                    "get",
                    "listStudyPlanActivities",
                ),
                (
                    "/api/academic/study-plan-versions/{id}/activities",
                    "post",
                    "addStudyPlanActivity",
                ),
                (
                    "/api/academic/study-plan-activities/{id}",
                    "put",
                    "updateStudyPlanActivity",
                ),
                (
                    "/api/academic/study-plan-activities/{id}",
                    "delete",
                    "deleteStudyPlanActivity",
                ),
                (
                    "/api/academic/activities/generate-from-plan",
                    "post",
                    "generateActivitiesFromStudyPlan",
                ),
                (
                    "/api/academic/activity-catalog",
                    "get",
                    "listActivityCatalog",
                ),
                (
                    "/api/academic/activity-catalog",
                    "post",
                    "createActivityCatalog",
                ),
                (
                    "/api/academic/activity-catalog/{id}",
                    "put",
                    "updateActivityCatalog",
                ),
                (
                    "/api/academic/activity-catalog/{id}",
                    "delete",
                    "deleteActivityCatalog",
                ),
                (
                    "/api/academic/activity-catalog/{id}/default-instructors",
                    "get",
                    "listActivityCatalogDefaultInstructors",
                ),
                (
                    "/api/academic/activity-catalog/{id}/default-instructors",
                    "post",
                    "addActivityCatalogDefaultInstructor",
                ),
                (
                    "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
                    "delete",
                    "removeActivityCatalogDefaultInstructor",
                ),
                (
                    "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
                    "put",
                    "updateActivityCatalogDefaultInstructorRole",
                ),
            ],
        );

        for (path, method, request_schema) in [
            (
                "/api/academic/study-plan-versions/{id}/activities",
                "post",
                "CreatePlanActivityRequest",
            ),
            (
                "/api/academic/study-plan-activities/{id}",
                "put",
                "UpdatePlanActivityRequest",
            ),
            (
                "/api/academic/activities/generate-from-plan",
                "post",
                "GenerateActivitiesFromPlanRequest",
            ),
            (
                "/api/academic/activity-catalog",
                "post",
                "CreateCatalogRequest",
            ),
            (
                "/api/academic/activity-catalog/{id}",
                "put",
                "UpdateCatalogRequest",
            ),
            (
                "/api/academic/activity-catalog/{id}/default-instructors",
                "post",
                "AddCatalogDefaultInstructorRequest",
            ),
            (
                "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
                "put",
                "UpdateCatalogDefaultInstructorRoleRequest",
            ),
        ] {
            assert_eq!(
                document["paths"][path][method]["requestBody"]["content"]["application/json"]
                    ["schema"]["$ref"],
                format!("#/components/schemas/{request_schema}")
            );
        }

        assert_eq!(
            document["paths"]["/api/academic/study-plan-versions/{id}/activities"]["post"]
                ["responses"]["201"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_StudyPlanVersionActivity"
        );
        assert_eq!(
            document["paths"]["/api/academic/activity-catalog"]["post"]["responses"]["201"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_ActivityCatalog"
        );
        assert_eq!(
            document["paths"]["/api/academic/activities/generate-from-plan"]["post"]["responses"]
                ["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_GenerateActivitiesFromPlanOutcome"
        );

        for (path, method) in [
            ("/api/academic/study-plan-activities/{id}", "delete"),
            ("/api/academic/activity-catalog/{id}", "delete"),
            (
                "/api/academic/activity-catalog/{id}/default-instructors",
                "post",
            ),
            (
                "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
                "delete",
            ),
            (
                "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
                "put",
            ),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiResponse_EmptyData",
                "incorrect empty success envelope for {method} {path}"
            );
        }

        let schemas = &document["components"]["schemas"];
        assert_eq!(
            schemas["ActivityCatalog"]["properties"]["activity_type"]["$ref"],
            "#/components/schemas/ActivityCatalogType"
        );
        assert_eq!(
            schemas["ActivityCatalog"]["properties"]["scheduling_mode"]["$ref"],
            "#/components/schemas/ActivitySchedulingMode"
        );
        assert_eq!(
            schemas["CatalogDefaultInstructor"]["properties"]["role"]["$ref"],
            "#/components/schemas/CurriculumInstructorRole"
        );

        let operation_count = document["paths"]
            .as_object()
            .expect("paths must be an object")
            .values()
            .flat_map(|path| path.as_object().expect("path item").values())
            .filter(|operation| operation.get("operationId").is_some())
            .count();
        assert_eq!(operation_count, 177);
    }

    #[test]
    fn academic_activity_workspace_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                ("/api/academic/activity-slots", "get", "listActivitySlots"),
                (
                    "/api/academic/activity-slots/{id}",
                    "put",
                    "updateActivitySlot",
                ),
                (
                    "/api/academic/activity-slots/{id}",
                    "delete",
                    "deleteActivitySlot",
                ),
                (
                    "/api/academic/activity-slots/{id}/instructors",
                    "get",
                    "listActivitySlotInstructors",
                ),
                (
                    "/api/academic/activity-slots/{id}/instructors",
                    "post",
                    "addActivitySlotInstructor",
                ),
                (
                    "/api/academic/activity-slots/{id}/instructors/batch",
                    "post",
                    "addActivitySlotInstructorsBatch",
                ),
                (
                    "/api/academic/activity-slots/{id}/instructors/{user_id}",
                    "delete",
                    "removeActivitySlotInstructor",
                ),
                (
                    "/api/academic/activity-slots/{id}/instructors/all",
                    "delete",
                    "removeAllActivitySlotInstructors",
                ),
                (
                    "/api/academic/activity-slots/{id}/groups",
                    "delete",
                    "deleteAllActivitySlotGroups",
                ),
                (
                    "/api/academic/activity-slots/{id}/timetable-entries",
                    "delete",
                    "deleteActivitySlotTimetableEntries",
                ),
                (
                    "/api/academic/activity-slots/{id}/classroom-assignments",
                    "get",
                    "listActivitySlotClassroomAssignments",
                ),
                (
                    "/api/academic/activity-slots/{id}/classroom-assignments",
                    "post",
                    "upsertActivitySlotClassroomAssignments",
                ),
                (
                    "/api/academic/activity-slots/{id}/classroom-assignments/all",
                    "delete",
                    "deleteAllActivitySlotClassroomAssignments",
                ),
                (
                    "/api/academic/activity-slots/{id}/classroom-assignments/{assignment_id}",
                    "delete",
                    "deleteActivitySlotClassroomAssignment",
                ),
                ("/api/academic/activities", "get", "listActivityGroups"),
                ("/api/academic/activities", "post", "createActivityGroup"),
                (
                    "/api/academic/activities/{id}",
                    "put",
                    "updateActivityGroup",
                ),
                (
                    "/api/academic/activities/{id}",
                    "delete",
                    "deleteActivityGroup",
                ),
                (
                    "/api/academic/activities/{id}/members",
                    "get",
                    "listActivityGroupMembers",
                ),
                (
                    "/api/academic/activities/{id}/members",
                    "post",
                    "addActivityGroupMembers",
                ),
                (
                    "/api/academic/activities/{id}/members/{student_id}",
                    "delete",
                    "removeActivityGroupMember",
                ),
                (
                    "/api/academic/activities/members/{member_id}/result",
                    "put",
                    "updateActivityGroupMemberResult",
                ),
                (
                    "/api/academic/activities/{id}/instructors",
                    "get",
                    "listActivityGroupInstructors",
                ),
                (
                    "/api/academic/activities/{id}/instructors",
                    "post",
                    "addActivityGroupInstructor",
                ),
                (
                    "/api/academic/activities/{id}/instructors/{instructor_id}",
                    "delete",
                    "removeActivityGroupInstructor",
                ),
                (
                    "/api/academic/activities/my-enrollments",
                    "get",
                    "listMyActivityEnrollments",
                ),
                (
                    "/api/academic/activities/{id}/enroll",
                    "post",
                    "selfEnrollActivityGroup",
                ),
                (
                    "/api/academic/activities/{id}/enroll",
                    "delete",
                    "selfUnenrollActivityGroup",
                ),
            ],
        );

        for (path, method, request_schema) in [
            (
                "/api/academic/activity-slots/{id}",
                "put",
                "UpdateActivitySlotRequest",
            ),
            (
                "/api/academic/activity-slots/{id}/instructors",
                "post",
                "AddSlotInstructorRequest",
            ),
            (
                "/api/academic/activity-slots/{id}/instructors/batch",
                "post",
                "AddSlotInstructorsBatchRequest",
            ),
            (
                "/api/academic/activity-slots/{id}/classroom-assignments",
                "post",
                "BatchUpsertSlotClassroomAssignmentsRequest",
            ),
            (
                "/api/academic/activities",
                "post",
                "CreateActivityGroupRequest",
            ),
            (
                "/api/academic/activities/{id}",
                "put",
                "UpdateActivityGroupRequest",
            ),
            (
                "/api/academic/activities/{id}/members",
                "post",
                "AddMembersRequest",
            ),
            (
                "/api/academic/activities/members/{member_id}/result",
                "put",
                "UpdateMemberResultRequest",
            ),
            (
                "/api/academic/activities/{id}/instructors",
                "post",
                "InstructorRoleRequest",
            ),
        ] {
            assert_eq!(
                document["paths"][path][method]["requestBody"]["content"]["application/json"]
                    ["schema"]["$ref"],
                format!("#/components/schemas/{request_schema}")
            );
        }

        for (path, method, response_schema) in [
            (
                "/api/academic/activity-slots",
                "get",
                "ApiResponse_Vec_ActivitySlot",
            ),
            (
                "/api/academic/activity-slots/{id}",
                "put",
                "ApiResponse_ActivitySlot",
            ),
            (
                "/api/academic/activities",
                "get",
                "ApiResponse_Vec_ActivityGroup",
            ),
            (
                "/api/academic/activities/{id}/members",
                "get",
                "ApiResponse_Vec_ActivityGroupMember",
            ),
            (
                "/api/academic/activities/{id}/members",
                "post",
                "ApiResponse_ActivityInsertedCountData",
            ),
            (
                "/api/academic/activity-slots/{id}/instructors/batch",
                "post",
                "ApiResponse_ActivityAddedCountData",
            ),
            (
                "/api/academic/activity-slots/{id}/timetable-entries",
                "delete",
                "ApiResponse_ActivityDeletedCountData",
            ),
            (
                "/api/academic/activity-slots/{id}/classroom-assignments",
                "post",
                "ApiResponse_ActivityProcessedCountData",
            ),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["200"]["content"]["application/json"]
                    ["schema"]["$ref"],
                format!("#/components/schemas/{response_schema}")
            );
        }

        assert_eq!(
            document["paths"]["/api/academic/activities/{id}/enroll"]["post"]["responses"]["409"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );
        assert_eq!(
            document["paths"]["/api/academic/activity-slots/{id}"]["put"]["responses"]["400"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );

        let schemas = &document["components"]["schemas"];
        assert_eq!(
            schemas["ActivitySlot"]["properties"]["registration_type"]["$ref"],
            "#/components/schemas/ActivityRegistrationType"
        );
        assert!(schemas["ActivityGroupMember"]["properties"]["result"]
            .to_string()
            .contains("#/components/schemas/ActivityMemberResult"));
        assert!(schemas["InstructorRoleRequest"]["properties"]["role"]
            .to_string()
            .contains("#/components/schemas/ActivityGroupInstructorRole"));

        let operation_count = document["paths"]
            .as_object()
            .expect("paths must be an object")
            .values()
            .flat_map(|path| path.as_object().expect("path item").values())
            .filter(|operation| operation.get("operationId").is_some())
            .count();
        assert_eq!(operation_count, 177);
    }

    #[test]
    fn academic_course_planning_contracts() {
        let document = school_api_value().expect("document should serialize");
        assert_operations(
            &document,
            &[
                (
                    "/api/academic/planning/courses",
                    "get",
                    "listClassroomCourses",
                ),
                ("/api/academic/planning/courses", "post", "assignCourses"),
                (
                    "/api/academic/planning/courses/{id}",
                    "put",
                    "updateClassroomCourse",
                ),
                (
                    "/api/academic/planning/courses/{id}",
                    "delete",
                    "removeClassroomCourse",
                ),
                (
                    "/api/academic/planning/courses/instructors/batch",
                    "post",
                    "batchListCourseInstructors",
                ),
                (
                    "/api/academic/planning/courses/instructors",
                    "get",
                    "batchListCourseInstructorsFromQuery",
                ),
                (
                    "/api/academic/planning/courses/{id}/instructors",
                    "get",
                    "listCourseInstructors",
                ),
                (
                    "/api/academic/planning/courses/{id}/instructors",
                    "post",
                    "addCourseInstructor",
                ),
                (
                    "/api/academic/planning/courses/{id}/instructors/{uid}",
                    "put",
                    "updateCourseInstructorRole",
                ),
                (
                    "/api/academic/planning/courses/{id}/instructors/{uid}",
                    "delete",
                    "removeCourseInstructor",
                ),
                (
                    "/api/academic/planning/classrooms/{classroom_id}/activities",
                    "get",
                    "listClassroomActivities",
                ),
                (
                    "/api/academic/planning/classrooms/{classroom_id}/activities/{slot_id}",
                    "delete",
                    "removeClassroomFromActivitySlot",
                ),
            ],
        );

        let operation_ids: Vec<&str> = document["paths"]
            .as_object()
            .expect("paths must be an object")
            .values()
            .flat_map(|path| path.as_object().expect("path item").values())
            .filter_map(|operation| operation["operationId"].as_str())
            .collect();
        assert_eq!(operation_ids.len(), 177);
        assert_eq!(
            operation_ids.iter().copied().collect::<HashSet<_>>().len(),
            177
        );

        for (path, method, request_schema) in [
            (
                "/api/academic/planning/courses",
                "post",
                "AssignCoursesRequest",
            ),
            (
                "/api/academic/planning/courses/{id}",
                "put",
                "UpdateCourseRequest",
            ),
            (
                "/api/academic/planning/courses/instructors/batch",
                "post",
                "BatchListCourseInstructorsRequest",
            ),
            (
                "/api/academic/planning/courses/{id}/instructors",
                "post",
                "AddCourseInstructorRequest",
            ),
            (
                "/api/academic/planning/courses/{id}/instructors/{uid}",
                "put",
                "UpdateCourseInstructorRoleRequest",
            ),
        ] {
            assert_eq!(
                document["paths"][path][method]["requestBody"]["content"]["application/json"]
                    ["schema"]["$ref"],
                format!("#/components/schemas/{request_schema}"),
                "incorrect request schema for {method} {path}"
            );
            assert_eq!(
                document["paths"][path][method]["responses"]["400"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse",
                "invalid JSON must use the standard 400 envelope for {method} {path}"
            );
        }

        for (path, method) in [
            ("/api/academic/planning/courses/{id}", "put"),
            ("/api/academic/planning/courses/{id}/instructors", "post"),
            (
                "/api/academic/planning/courses/{id}/instructors/{uid}",
                "put",
            ),
            (
                "/api/academic/planning/courses/{id}/instructors/{uid}",
                "delete",
            ),
        ] {
            assert_eq!(
                document["paths"][path][method]["responses"]["409"]["content"]["application/json"]
                    ["schema"]["$ref"],
                "#/components/schemas/ApiErrorResponse",
                "timetable conflicts must use the standard 409 envelope for {method} {path}"
            );
        }

        assert_eq!(
            document["paths"]["/api/academic/planning/courses"]["post"]["responses"]["200"]
                ["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_CourseAssignedCountData"
        );
        assert_eq!(
            document["paths"]["/api/academic/planning/courses/instructors"]["get"]["responses"]
                ["400"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiErrorResponse"
        );

        let schemas = &document["components"]["schemas"];
        let primary_instructor =
            &schemas["UpdateCourseRequest"]["properties"]["primary_instructor_id"];
        assert!(
            primary_instructor.to_string().contains("null"),
            "primary_instructor_id must document explicit null clearing"
        );
        assert!(
            primary_instructor.to_string().contains("uuid"),
            "primary_instructor_id must remain UUID-typed"
        );
        assert_eq!(
            schemas["CourseInstructor"]["properties"]["role"]["$ref"],
            "#/components/schemas/CourseInstructorRole"
        );
        assert_eq!(
            schemas["CourseInstructorRole"]["enum"],
            serde_json::json!(["primary", "secondary"])
        );

        let classroom_course = &schemas["ClassroomCourse"];
        assert_eq!(
            required(classroom_course),
            vec![
                "academic_semester_id",
                "classroom_id",
                "classroom_name",
                "id",
                "instructor_name",
                "primary_instructor_id",
                "settings",
                "subject_code",
                "subject_credit",
                "subject_hours",
                "subject_id",
                "subject_name_en",
                "subject_name_th",
                "subject_type",
            ]
        );
        for nullable_field in [
            "primary_instructor_id",
            "subject_code",
            "subject_name_th",
            "subject_name_en",
            "subject_credit",
            "subject_hours",
            "instructor_name",
            "subject_type",
            "classroom_name",
        ] {
            assert!(
                contains_null(&classroom_course["properties"][nullable_field]),
                "{nullable_field} must be required but nullable"
            );
        }
        assert_eq!(
            classroom_course["properties"]["settings"]["$ref"],
            "#/components/schemas/ClassroomCourseSettings"
        );
        assert_eq!(
            schemas["UpdateCourseRequest"]["properties"]["settings"]["$ref"],
            "#/components/schemas/ClassroomCourseSettings"
        );
        assert_eq!(schemas["ClassroomCourseSettings"]["type"], "object");
        assert!(
            schemas["ClassroomCourseSettings"]
                .get("additionalProperties")
                .is_some(),
            "course settings must allow arbitrary JSON-valued properties"
        );
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
                ("/api/admin/menu/groups", "get", "listMenuGroups"),
                ("/api/admin/menu/items", "get", "listMenuItems"),
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
                ("/api/lookup/classrooms", "get", "lookupClassrooms"),
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
        for name in ["active_only", "search", "limit", "academic_year_id"] {
            assert!(lookup_parameters
                .iter()
                .any(|parameter| { parameter["name"] == name && parameter["in"] == "query" }));
        }

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
            "#/components/schemas/ApiResponse_TimetableItemsData"
        );
        assert_eq!(
            document["paths"]["/api/parent/students/{student_id}/exam-schedules"]["get"]
                ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApiResponse_Vec_PersonalExamScheduleRound"
        );

        let parent_calendar_parameters = document["paths"]
            ["/api/parent/students/{student_id}/calendar/events"]["get"]["parameters"]
            .as_array()
            .expect("parent calendar parameters must be an array");
        for (name, location) in [
            ("student_id", "path"),
            ("from", "query"),
            ("to", "query"),
            ("category_id", "query"),
            ("tag_id", "query"),
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
            .expect("my timetable parameters must be an array");
        for name in ["academic_semester_id", "day_of_week", "include_team_ghosts"] {
            assert!(my_timetable_parameters
                .iter()
                .any(|parameter| parameter["name"] == name && parameter["in"] == "query"));
        }

        let schemas = &document["components"]["schemas"];
        let timetable = &schemas["TimetableEntry"];
        for field in [
            "classroom_course_id",
            "room_id",
            "note",
            "title",
            "classroom_id",
            "activity_slot_id",
            "created_by",
            "updated_by",
        ] {
            assert!(required(timetable).contains(&field));
            assert!(contains_null(&timetable["properties"][field]));
        }
        for field in ["batch_id", "subject_code", "instructor_ids", "start_time"] {
            assert!(!required(timetable).contains(&field));
            assert!(!contains_null(&timetable["properties"][field]));
        }
        assert!(contains_null(
            &timetable["properties"]["instructor_subject_group_ids"]["items"]
        ));

        let exam_round = &schemas["PersonalExamScheduleRound"];
        assert!(required(exam_round).contains(&"publishedAt"));
        assert!(contains_null(&exam_round["properties"]["publishedAt"]));
        let exam_session = &schemas["PersonalExamSessionView"];
        for field in ["buildingName", "seatNumber"] {
            assert!(required(exam_session).contains(&field));
            assert!(contains_null(&exam_session["properties"][field]));
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
            "from",
            "to",
            "category_id",
            "tag_id",
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
        assert_eq!(operation_ids.len(), 177);

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
}
