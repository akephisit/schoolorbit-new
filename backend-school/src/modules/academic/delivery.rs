pub mod handlers;
pub mod version_handlers;

#[cfg(test)]
pub(crate) mod models {
    pub use school_academic_delivery::models::*;
}

#[cfg(test)]
pub(crate) mod services;

#[cfg(test)]
mod services_tests;
