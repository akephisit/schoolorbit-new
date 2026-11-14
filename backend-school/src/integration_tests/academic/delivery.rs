#[cfg(test)]
pub(crate) mod models {
    pub use school_academic_delivery::models::*;
}

#[cfg(test)]
pub(crate) mod services;

#[cfg(test)]
mod services_tests;

#[cfg(test)]
mod http_fixture;
#[cfg(test)]
pub(crate) use http_fixture::academic_router as authenticated_academic_router;
