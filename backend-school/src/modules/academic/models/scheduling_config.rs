use serde::{Deserialize, Deserializer, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::scheduling::TimeSlot;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Patch<T> {
    #[default]
    Unchanged,
    Clear,
    Set(T),
}

pub fn deserialize_patch<'de, D, T>(deserializer: D) -> Result<Patch<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|value| match value {
        Some(value) => Patch::Set(value),
        None => Patch::Clear,
    })
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct SaveSchedulingConfigurationRequest {
    pub scheduler_settings: Option<SchedulerSettingsPatch>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<Uuid>>, nullable = true)]
    pub instructor_order: Patch<Vec<Uuid>>,
    #[serde(default)]
    pub instructors: Vec<InstructorConstraintPatch>,
    #[serde(default)]
    pub subjects: Vec<SubjectConstraintPatch>,
    #[serde(default)]
    pub classroom_courses: Vec<ClassroomCourseConstraintPatch>,
    #[serde(default)]
    pub preferred_rooms: Vec<ClassroomCoursePreferredRoomsPatch>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct SchedulerSettingsPatch {
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub default_max_consecutive: Patch<i32>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct InstructorConstraintPatch {
    pub id: Uuid,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<TimeSlot>>, nullable = true)]
    pub hard_unavailable_slots: Patch<Vec<TimeSlot>>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub max_periods_per_day: Patch<i32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<TimeSlot>>, nullable = true)]
    pub preferred_slots: Patch<Vec<TimeSlot>>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Uuid>, nullable = true)]
    pub assigned_room_id: Patch<Uuid>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct SubjectConstraintPatch {
    pub id: Uuid,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub min_consecutive_periods: Patch<i32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<i32>, nullable = true)]
    pub max_consecutive_periods: Patch<i32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<bool>, nullable = true)]
    pub allow_single_period: Patch<bool>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<Uuid>>, nullable = true)]
    pub allowed_period_ids: Patch<Vec<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<String>>, nullable = true)]
    pub allowed_days: Patch<Vec<String>>,
}

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ClassroomCourseConstraintPatch {
    pub id: Uuid,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<i32>>, nullable = true)]
    pub consecutive_pattern: Patch<Vec<i32>>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<bool>, nullable = true)]
    pub same_day_unique: Patch<bool>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    #[schema(value_type = Option<Vec<TimeSlot>>, nullable = true)]
    pub hard_unavailable_slots: Patch<Vec<TimeSlot>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ClassroomCoursePreferredRoomsPatch {
    pub classroom_course_id: Uuid,
    pub rooms: Vec<PreferredRoomInput>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PreferredRoomInput {
    pub room_id: Uuid,
    pub rank: i32,
    #[serde(default)]
    pub is_required: bool,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListClassroomCourseConstraintsQuery {
    pub instructor_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SchedulerSettingsView {
    pub default_max_consecutive: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstructorConstraintView {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub hard_unavailable_slots: Option<Vec<TimeSlot>>,
    #[schema(required = true)]
    pub max_periods_per_day: Option<i32>,
    #[schema(required = true)]
    pub min_periods_per_day: Option<i32>,
    #[schema(required = true)]
    pub assigned_room_id: Option<Uuid>,
    #[schema(required = true)]
    pub assigned_room_name: Option<String>,
    pub priority: i32,
    pub primary_course_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubjectConstraintView {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub min_consecutive_periods: i32,
    #[schema(required = true)]
    pub max_consecutive_periods: Option<i32>,
    #[schema(required = true)]
    pub allow_single_period: Option<bool>,
    #[schema(required = true)]
    pub periods_per_week: Option<i32>,
    #[schema(required = true)]
    pub allowed_period_ids: Option<Vec<Uuid>>,
    #[schema(required = true)]
    pub allowed_days: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClassroomCourseConstraintView {
    pub id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    #[schema(required = true)]
    pub periods_per_week: Option<i32>,
    #[schema(required = true)]
    pub primary_instructor_id: Option<Uuid>,
    #[schema(required = true)]
    pub primary_instructor_name: Option<String>,
    #[schema(required = true)]
    pub consecutive_pattern: Option<Vec<i32>>,
    pub same_day_unique: bool,
    pub hard_unavailable_slots: Vec<TimeSlot>,
    pub team_unavailable_slots: Vec<TimeSlot>,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct CcPreferredRoomView {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub room_id: Uuid,
    pub room_code: String,
    pub room_name: String,
    pub rank: i32,
    pub is_required: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct SchedulingRoomView {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    #[schema(required = true)]
    pub room_type: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, Default)]
pub struct SchedulingConfigurationSaveResult {
    pub changed: bool,
    pub scheduler_settings_changed: bool,
    pub instructor_order_updated: usize,
    pub instructor_constraints_updated: usize,
    pub subject_constraints_updated: usize,
    pub classroom_course_constraints_updated: usize,
    pub preferred_room_sets_updated: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_patch_distinguishes_missing_null_and_value() {
        let missing: SaveSchedulingConfigurationRequest = serde_json::from_str("{}").unwrap();
        let cleared: SaveSchedulingConfigurationRequest =
            serde_json::from_str(r#"{"scheduler_settings":{"default_max_consecutive":null}}"#)
                .unwrap();
        let set: SaveSchedulingConfigurationRequest =
            serde_json::from_str(r#"{"scheduler_settings":{"default_max_consecutive":6}}"#)
                .unwrap();

        assert!(missing.scheduler_settings.is_none());
        assert!(matches!(
            cleared.scheduler_settings.unwrap().default_max_consecutive,
            Patch::Clear
        ));
        assert!(matches!(
            set.scheduler_settings.unwrap().default_max_consecutive,
            Patch::Set(6)
        ));
    }

    #[test]
    fn nullable_row_fields_distinguish_clear_and_unchanged() {
        let request: SaveSchedulingConfigurationRequest = serde_json::from_str(
            r#"{
                "instructors":[{"id":"00000000-0000-0000-0000-000000000001","assigned_room_id":null}],
                "subjects":[{"id":"00000000-0000-0000-0000-000000000002","allowed_days":null}],
                "classroom_courses":[{"id":"00000000-0000-0000-0000-000000000003","consecutive_pattern":null}]
            }"#,
        )
        .unwrap();

        assert!(matches!(
            request.instructors[0].assigned_room_id,
            Patch::Clear
        ));
        assert!(matches!(
            request.instructors[0].preferred_slots,
            Patch::Unchanged
        ));
        assert!(matches!(request.subjects[0].allowed_days, Patch::Clear));
        assert!(matches!(
            request.subjects[0].allowed_period_ids,
            Patch::Unchanged
        ));
        assert!(matches!(
            request.classroom_courses[0].consecutive_pattern,
            Patch::Clear
        ));
    }

    #[test]
    fn omitted_collections_default_to_empty_and_order_is_tristate() {
        let missing: SaveSchedulingConfigurationRequest = serde_json::from_str("{}").unwrap();
        let cleared: SaveSchedulingConfigurationRequest =
            serde_json::from_str(r#"{"instructor_order":null}"#).unwrap();
        let set: SaveSchedulingConfigurationRequest = serde_json::from_str(
            r#"{"instructor_order":["00000000-0000-0000-0000-000000000001"]}"#,
        )
        .unwrap();

        assert!(missing.instructors.is_empty());
        assert!(missing.subjects.is_empty());
        assert!(missing.classroom_courses.is_empty());
        assert!(missing.preferred_rooms.is_empty());
        assert!(matches!(missing.instructor_order, Patch::Unchanged));
        assert!(matches!(cleared.instructor_order, Patch::Clear));
        assert!(matches!(set.instructor_order, Patch::Set(ids) if ids.len() == 1));
    }
}
