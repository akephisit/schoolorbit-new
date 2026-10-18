//! Narrow public operations for the Staff leave capability.
pub use super::configuration::{
    delete_type, effective_policy, list_types, policies, save_policy, save_type,
};
pub use super::lifecycle::{act, cancel, create, submit, update};
pub use super::queries::{detail, list, options, routing_options, usage};
