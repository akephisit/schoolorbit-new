use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub user_email: Option<String>,
    pub user_name: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
    pub old_values: Option<JsonValue>,
    pub new_values: Option<JsonValue>,
    pub changes: Option<JsonValue>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<JsonValue>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

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

    pub fn request_context(
        mut self,
        ip: Option<String>,
        user_agent: Option<String>,
        path: Option<String>,
        method: Option<String>,
    ) -> Self {
        self.ip_address = ip;
        self.user_agent = user_agent;
        self.request_path = path;
        self.request_method = method;
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn metadata(mut self, meta: JsonValue) -> Self {
        self.metadata = Some(meta);
        self
    }

    pub async fn save(self, pool: &PgPool) -> Result<Uuid, sqlx::Error> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO audit_logs (
                user_id, user_email, user_name, action, entity_type, entity_id, entity_name,
                old_values, new_values, changes, ip_address, user_agent, request_path, request_method,
                description, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id",
        )
        .bind(&self.user_id)
        .bind(&self.user_email)
        .bind(&self.user_name)
        .bind(&self.action)
        .bind(&self.entity_type)
        .bind(&self.entity_id)
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
        .fetch_one(pool)
        .await?;

        Ok(id)
    }
}

// Helper functions for common audit actions

pub async fn log_create(
    pool: &PgPool,
    user_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    entity_name: Option<String>,
    new_values: JsonValue,
) -> Result<Uuid, sqlx::Error> {
    AuditLogBuilder::new("create", entity_type)
        .user(user_id, None, None)
        .entity(entity_id, entity_name)
        .new_values(new_values)
        .description(format!("Created {} with ID {}", entity_type, entity_id))
        .save(pool)
        .await
}

pub async fn log_update(
    pool: &PgPool,
    user_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    entity_name: Option<String>,
    old_values: JsonValue,
    new_values: JsonValue,
    changes: JsonValue,
) -> Result<Uuid, sqlx::Error> {
    AuditLogBuilder::new("update", entity_type)
        .user(user_id, None, None)
        .entity(entity_id, entity_name)
        .old_values(old_values)
        .new_values(new_values)
        .changes(changes)
        .description(format!("Updated {} with ID {}", entity_type, entity_id))
        .save(pool)
        .await
}

pub async fn log_delete(
    pool: &PgPool,
    user_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    entity_name: Option<String>,
    old_values: JsonValue,
) -> Result<Uuid, sqlx::Error> {
    AuditLogBuilder::new("delete", entity_type)
        .user(user_id, None, None)
        .entity(entity_id, entity_name)
        .old_values(old_values)
        .description(format!("Deleted {} with ID {}", entity_type, entity_id))
        .save(pool)
        .await
}

pub async fn log_login(
    pool: &PgPool,
    user_id: Uuid,
    user_email: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
) -> Result<Uuid, sqlx::Error> {
    AuditLogBuilder::new("login", "user")
        .user(user_id, Some(user_email.clone()), None)
        .entity(user_id, Some(user_email))
        .request_context(ip_address, user_agent, Some("/api/auth/login".to_string()), Some("POST".to_string()))
        .description("User logged in".to_string())
        .save(pool)
        .await
}
