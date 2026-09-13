use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OpeningPolicy {
    pub row_version: i64,
    pub require_homeroom_placements: bool,
    pub require_published_offerings: bool,
    pub require_published_timetable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateOpeningPolicyInput {
    pub row_version: i64,
    pub require_homeroom_placements: bool,
    pub require_published_offerings: bool,
    pub require_published_timetable: bool,
}
