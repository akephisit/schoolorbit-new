mod academic_resource;
mod cache;
mod organization_scope;
mod permissions;

pub use academic_resource::{
    academic_resource_access_for, academic_resource_list_filter_without_database,
    resolve_academic_resource_list_filter, resolve_academic_resource_list_filter_on_connection,
    AcademicResourceAccess, AcademicResourceListFilter, AcademicResourcePermissions,
};
pub use cache::{PermissionCache, PermissionCacheRevision, TenantUserKey};
pub use organization_scope::{
    accessible_exact_units_for_permission, accessible_exact_units_for_permission_on_connection,
    accessible_tree_units_for_permission, accessible_tree_units_for_permission_on_connection,
    has_exact_unit_permission, has_exact_unit_permission_on_connection,
};
pub use permissions::{
    get_cached_user_permissions, load_actor_context, load_actor_context_for_session,
    module_permission_matches, permission_matches, staff_users_with_all_permissions,
    staff_users_with_all_permissions_on_connection, ActorContext,
};

pub mod resource_access_policy;
