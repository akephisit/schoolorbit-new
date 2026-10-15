use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyCategory {
    pub id: Uuid,
    pub name: String,
    pub revision: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyCategoryWrite {
    pub name: String,
    pub revision: Option<i64>,
}
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyVersion {
    pub id: Uuid,
    pub version: i32,
    pub price: String,
    pub shop: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyItem {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub category_id: Uuid,
    pub category_name: String,
    pub unit: String,
    pub image_file_id: Option<Uuid>,
    pub latest_version: SupplyVersion,
    pub favorite: bool,
    pub revision: i64,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyItemDetail {
    #[serde(flatten)]
    pub item: SupplyItem,
    pub versions: Vec<SupplyVersion>,
    pub versions_total: i64,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyItemPage {
    pub items: Vec<SupplyItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in=Query))]
pub struct SupplyItemQuery {
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub favorites_only: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyItemWrite {
    pub name: String,
    pub description: String,
    pub category_id: Uuid,
    pub unit: String,
    pub image_file_id: Option<Uuid>,
    pub price: String,
    pub shop: String,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyItemUpdate {
    pub name: String,
    pub description: String,
    pub category_id: Uuid,
    pub unit: String,
    #[serde(deserialize_with = "required_nullable_image")]
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub image_file_id: Option<Uuid>,
    pub revision: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyVersionWrite {
    pub price: String,
    pub shop: String,
    pub revision: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyFavoriteWrite {
    pub favorite: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyBundleEntryWrite {
    pub item_id: Uuid,
    pub quantity: i32,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyBundleWrite {
    pub name: String,
    pub shared: bool,
    pub entries: Vec<SupplyBundleEntryWrite>,
    pub revision: Option<i64>,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyBundleEntry {
    pub item: SupplyItem,
    pub quantity: i32,
    pub subtotal: String,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyBundle {
    pub id: Uuid,
    pub name: String,
    pub shared: bool,
    pub owner_user_id: Uuid,
    pub entries: Vec<SupplyBundleEntry>,
    pub total_price: String,
    pub revision: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in=Query))]
pub struct SupplyPageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyBundlePage {
    pub items: Vec<SupplyBundle>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyRevision {
    pub revision: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyProposalWrite {
    pub name: String,
    pub description: String,
    pub category_id: Uuid,
    pub unit: String,
    pub image_file_id: Option<Uuid>,
    pub price: String,
    pub shop: String,
    pub source_file_id: Option<Uuid>,
    pub source_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyProposal {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub category_id: Uuid,
    pub unit: String,
    pub image_file_id: Option<Uuid>,
    pub price: String,
    pub shop: String,
    pub source_file_id: Option<Uuid>,
    pub source_url: Option<String>,
    pub status: String,
    pub proposed_by: Uuid,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub item_id: Option<Uuid>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in=Query))]
pub struct SupplyProposalQuery {
    pub review: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyProposalPage {
    pub items: Vec<SupplyProposal>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyProposalApprove {
    pub revision: i64,
    pub existing_item_id: Option<Uuid>,
    pub existing_item_revision: Option<i64>,
    pub name: String,
    pub description: String,
    pub category_id: Uuid,
    pub unit: String,
    #[serde(deserialize_with = "required_nullable_image")]
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub image_file_id: Option<Uuid>,
    pub price: String,
    pub shop: String,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyProposalReject {
    pub revision: i64,
    pub reason: String,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupplyVersionAppend {
    pub price: String,
    pub shop: String,
    pub revision: i64,
    #[serde(deserialize_with = "required_nullable_image")]
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub image_file_id: Option<Uuid>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "api-docs", into_params(parameter_in=Query))]
pub struct SupplyDuplicateQuery {
    pub name: String,
    pub unit: String,
}

// Missing image fields must never mean an implicit destructive removal.
fn required_nullable_image<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Uuid>::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn append_requires_an_explicit_image_decision() {
        let missing = serde_json::json!({"price":"10.00","shop":"Shop","revision":1});
        assert!(serde_json::from_value::<SupplyVersionAppend>(missing).is_err());
        let clear =
            serde_json::json!({"price":"10.00","shop":"Shop","revision":1,"imageFileId":null});
        assert!(serde_json::from_value::<SupplyVersionAppend>(clear)
            .unwrap()
            .image_file_id
            .is_none());
        let id = Uuid::now_v7();
        let retain =
            serde_json::json!({"price":"10.00","shop":"Shop","revision":1,"imageFileId":id});
        assert_eq!(
            serde_json::from_value::<SupplyVersionAppend>(retain)
                .unwrap()
                .image_file_id,
            Some(id)
        );
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SupplyVersionPage {
    pub items: Vec<SupplyVersion>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
