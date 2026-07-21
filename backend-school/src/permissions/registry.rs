//! Permission registry backed by the generated cross-stack contract.

use serde::{Deserialize, Serialize};

/// Permission definition structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDef {
    pub code: &'static str,
    pub name: &'static str,
    pub module: &'static str,
    pub action: &'static str,
    pub scope: &'static str,
    pub description: &'static str,
}

include!("registry_generated.rs");
