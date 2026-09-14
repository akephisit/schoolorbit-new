//! Transactional audit logging for mutations that must commit with their audit record.

use serde_json::Value as JsonValue;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuditLogBuilder {
    user_id: Option<Uuid>,
    user_email: Option<String>,
    user_name: Option<String>,
    action: String,
    entity_type: String,
    entity_id: Option<Uuid>,
    entity_name: Option<String>,
    old_values: Option<JsonValue>,
    new_values: Option<JsonValue>,
    changes: Option<JsonValue>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    request_path: Option<String>,
    request_method: Option<String>,
    description: Option<String>,
    metadata: Option<JsonValue>,
}

impl AuditLogBuilder {
    pub fn new(action: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            user_id: None,
            user_email: None,
            user_name: None,
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id: None,
            entity_name: None,
            old_values: None,
            new_values: None,
            changes: None,
            ip_address: None,
            user_agent: None,
            request_path: None,
            request_method: None,
            description: None,
            metadata: None,
        }
    }

    pub fn user(mut self, user_id: Uuid, email: Option<String>, name: Option<String>) -> Self {
        self.user_id = Some(user_id);
        self.user_email = email;
        self.user_name = name;
        self
    }

    pub fn entity(mut self, entity_id: Uuid, entity_name: Option<String>) -> Self {
        self.entity_id = Some(entity_id);
        self.entity_name = entity_name;
        self
    }

    pub fn old_values(mut self, old: JsonValue) -> Self {
        self.old_values = Some(old);
        self
    }

    pub fn new_values(mut self, new: JsonValue) -> Self {
        self.new_values = Some(new);
        self
    }

    pub fn changes(mut self, changes: JsonValue) -> Self {
        self.changes = Some(changes);
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub async fn save_in_transaction(
        self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Uuid, sqlx::Error> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO audit_logs (
                user_id, user_email, user_name, action, entity_type, entity_id, entity_name,
                old_values, new_values, changes, ip_address, user_agent, request_path, request_method,
                description, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::inet, $12, $13, $14, $15, $16)
            RETURNING id",
        )
        .bind(self.user_id)
        .bind(&self.user_email)
        .bind(&self.user_name)
        .bind(&self.action)
        .bind(&self.entity_type)
        .bind(self.entity_id)
        .bind(&self.entity_name)
        .bind(&self.old_values)
        .bind(&self.new_values)
        .bind(&self.changes)
        .bind(&self.ip_address)
        .bind(&self.user_agent)
        .bind(&self.request_path)
        .bind(&self.request_method)
        .bind(&self.description)
        .bind(&self.metadata)
        .fetch_one(&mut **tx)
        .await?;

        Ok(id)
    }
}
