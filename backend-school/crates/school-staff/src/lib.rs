mod audit;

pub mod career;
pub mod models;
pub mod personnel;
pub mod rank_milestones;
pub mod services;

pub mod achievement;
pub mod organization_access_policy;
pub mod staff_access_policy;

pub mod profile_cutover;

mod permission_codes_generated;

#[cfg(test)]
mod test_permission_codes {
    include!("test_support/permission_codes_generated.rs");
}
