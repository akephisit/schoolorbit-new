use chrono::{NaiveDate, NaiveTime, Utc};
use school_academic_core::services::attendance_roster as core;
pub use school_attendance::application::*;
use school_attendance::{
    models::*,
    services::{sessions, settings, specials},
};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;
#[cfg(test)]
mod db_tests;
pub mod finalization;
