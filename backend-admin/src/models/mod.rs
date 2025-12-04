pub mod school;
pub mod admin_user;

pub use school::{School, CreateSchool, UpdateSchool};
pub use admin_user::{AdminUser, CreateAdminUser, LoginRequest};
