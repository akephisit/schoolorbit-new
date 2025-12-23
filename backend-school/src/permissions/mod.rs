/// Permission module for granular CRUD permission checking with scope support
/// 
/// Supports:
/// - Granular permissions: "staff.create"
/// - Wildcard permissions: "staff"
/// - Scoped permissions: "attendance.update.own", "grades.read.all"

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    Own,        // Only assigned/owned resources
    Department, // Department-level access  
    All,        // All resources (admin)
}

impl Action {
    pub fn as_str(&self) -> &str {
        match self {
            Action::Create => "create",
            Action::Read => "read",
            Action::Update => "update",
            Action::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Permission {
    pub resource: String,
    pub action: Option<Action>,
    pub scope: Option<Scope>, // None = legacy (treat as All)
}

impl Permission {
    /// Parse permission string into Permission struct
    /// 
    /// Examples:
    /// - "staff.create" -> Permission { resource: "staff", action: Some(Create), scope: Some(All) }
    /// - "staff" -> Permission { resource: "staff", action: None, scope: None }
    /// - "attendance.update.own" -> Permission { resource: "attendance", action: Some(Update), scope: Some(Own) }
    pub fn parse(permission_str: &str) -> Self {
        let parts: Vec<&str> = permission_str.split('.').collect();
        
        match parts.as_slice() {
            [resource] => Permission {
                resource: resource.to_string(),
                action: None, // Wildcard permission
                scope: None,
            },
            [resource, action] => {
                let action = match *action {
                    "create" => Some(Action::Create),
                    "read" => Some(Action::Read),
                    "update" => Some(Action::Update),
                    "delete" => Some(Action::Delete),
                    _ => None,
                };
                Permission {
                    resource: resource.to_string(),
                    action,
                    scope: Some(Scope::All), // Default to All for backward compatibility
                }
            },
            [resource, action, scope] => {
                let action = match *action {
                    "create" => Some(Action::Create),
                    "read" => Some(Action::Read),
                    "update" => Some(Action::Update),
                    "delete" => Some(Action::Delete),
                    _ => None,
                };
                let scope = match *scope {
                    "own" => Some(Scope::Own),
                    "department" => Some(Scope::Department),
                    "all" => Some(Scope::All),
                    _ => Some(Scope::All), // Default to All if unrecognized
                };
                Permission {
                    resource: resource.to_string(),
                    action,
                    scope,
                }
            },
            _ => Permission {
                resource: permission_str.to_string(),
                action: None,
                scope: None,
            },
        }
    }

    /// Convert Permission back to string
    pub fn to_string(&self) -> String {
        match &self.action {
            Some(action) => format!("{}.{}", self.resource, action.as_str()),
            None => self.resource.clone(),
        }
    }
}

/// Check if user has required permission
/// 
/// Logic:
/// 1. Check for exact match (e.g., "staff.create")
/// 2. Check for wildcard permission (e.g., "staff" grants all "staff.*")
/// 
/// Examples:
/// ```
/// let user_perms = vec!["staff.read".to_string(), "students".to_string()];
/// 
/// // Exact match
/// assert!(has_permission(&user_perms, "staff.read"));
/// 
/// // Wildcard match
/// assert!(has_permission(&user_perms, "students.create"));
/// assert!(has_permission(&user_perms, "students.update"));
/// 
/// // No match
/// assert!(!has_permission(&user_perms, "staff.delete"));
/// ```
pub fn has_permission(user_permissions: &[String], required: &str) -> bool {
    // Check for exact match first
    if user_permissions.iter().any(|p| p == required) {
        return true;
    }

    // Parse required permission
    let required_perm = Permission::parse(required);

    // Check for wildcard permission
    // If user has "staff", they should have "staff.create", "staff.read", etc.
    if required_perm.action.is_some() {
        user_permissions.iter().any(|p| {
            let user_perm = Permission::parse(p);
            user_perm.resource == required_perm.resource && user_perm.action.is_none()
        })
    } else {
        false
    }
}

/// Check if user has scoped permission for a specific resource
/// 
/// This checks both permission and scope-based authorization.
/// 
/// # Arguments
/// * `user_permissions` - List of user's permissions
/// * `required` - Required permission (e.g., "attendance.update.own")
/// 
/// # Note
/// This is a simplified version. Full implementation requires:
/// - teaching_assignments table for checking class ownership
/// - department_members table for department checks
/// 
/// For now, scope checking is basic:
/// - If user has permission with .all scope → granted
/// - If user has permission with .own scope → granted (TODO: check ownership)
/// - If user has permission with .department scope → granted (TODO: check department)
/// 
/// # Examples
/// ```
/// let user_perms = vec!["attendance.update.own".to_string()];
/// 
/// // Has the permission
/// assert!(has_scoped_permission(&user_perms, "attendance.update.own"));
/// 
/// // Doesn't have this permission
/// assert!(!has_scoped_permission(&user_perms, "grades.update.own"));
/// ```
pub fn has_scoped_permission(user_permissions: &[String], required: &str) -> bool {
    // Parse required permission
    let required_perm = Permission::parse(required);
    
    // Check if user has the permission (exact or wildcard)
    if !has_permission(user_permissions, required) {
        return false;
    }
    
    // If user has exact match, they have the permission
    if user_permissions.iter().any(|p| p == required) {
        return true;
    }
    
    // Check if user has broader scope
    // e.g., user has "attendance.update.all" but requires "attendance.update.own"
    if let (Some(action), Some(required_scope)) = (&required_perm.action, &required_perm.scope) {
        for user_perm_str in user_permissions {
            let user_perm = Permission::parse(user_perm_str);
            
            // Same resource and action
            if user_perm.resource == required_perm.resource && user_perm.action == Some(action.clone()) {
                if let Some(user_scope) = &user_perm.scope {
                    match (user_scope, required_scope) {
                        // User has .all, can access anything
                        (Scope::All, _) => return true,
                        // User has .department, can access own and department
                        (Scope::Department, Scope::Own) => return true,
                        (Scope::Department, Scope::Department) => return true,
                        // User has .own, can only access own
                        (Scope::Own, Scope::Own) => return true,
                        _ => continue,
                    }
                }
            }
        }
    }
    
    // Check wildcard with All scope
    // User has "attendance" grants "attendance.update.own"
    if required_perm.action.is_some() {
        if user_permissions.iter().any(|p| {
            let user_perm = Permission::parse(p);
            user_perm.resource == required_perm.resource && user_perm.action.is_none()
        }) {
            return true;
        }
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_parse_granular() {
        let perm = Permission::parse("staff.create");
        assert_eq!(perm.resource, "staff");
        assert_eq!(perm.action, Some(Action::Create));
        assert_eq!(perm.scope, Some(Scope::All)); // Default to All
    }

    #[test]
    fn test_permission_parse_wildcard() {
        let perm = Permission::parse("staff");
        assert_eq!(perm.resource, "staff");
        assert_eq!(perm.action, None);
        assert_eq!(perm.scope, None);
    }
    
    #[test]
    fn test_permission_parse_scoped() {
        let perm = Permission::parse("attendance.update.own");
        assert_eq!(perm.resource, "attendance");
        assert_eq!(perm.action, Some(Action::Update));
        assert_eq!(perm.scope, Some(Scope::Own));
        
        let perm2 = Permission::parse("grades.read.all");
        assert_eq!(perm2.resource, "grades");
        assert_eq!(perm2.action, Some(Action::Read));
        assert_eq!(perm2.scope, Some(Scope::All));
        
        let perm3 = Permission::parse("grades.read.department");
        assert_eq!(perm3.resource, "grades");
        assert_eq!(perm3.action, Some(Action::Read));
        assert_eq!(perm3.scope, Some(Scope::Department));
    }

    #[test]
    fn test_permission_to_string() {
        let perm1 = Permission {
            resource: "staff".to_string(),
            action: Some(Action::Create),
            scope: Some(Scope::All),
        };
        assert_eq!(perm1.to_string(), "staff.create");

        let perm2 = Permission {
            resource: "staff".to_string(),
            action: None,
            scope: None,
        };
        assert_eq!(perm2.to_string(), "staff");
    }

    #[test]
    fn test_has_permission_exact_match() {
        let user_perms = vec!["staff.read".to_string(), "students.create".to_string()];
        assert!(has_permission(&user_perms, "staff.read"));
        assert!(has_permission(&user_perms, "students.create"));
    }

    #[test]
    fn test_has_permission_wildcard() {
        let user_perms = vec!["staff".to_string()];
        assert!(has_permission(&user_perms, "staff.create"));
        assert!(has_permission(&user_perms, "staff.read"));
        assert!(has_permission(&user_perms, "staff.update"));
        assert!(has_permission(&user_perms, "staff.delete"));
    }

    #[test]
    fn test_has_permission_no_match() {
        let user_perms = vec!["staff.read".to_string()];
        assert!(!has_permission(&user_perms, "staff.create"));
        assert!(!has_permission(&user_perms, "staff.delete"));
        assert!(!has_permission(&user_perms, "students.read"));
    }

    #[test]
    fn test_has_permission_combined() {
        let user_perms = vec![
            "staff".to_string(),           // Wildcard for staff
            "students.read".to_string(),   // Only read for students
        ];
        
        // Staff wildcard grants all
        assert!(has_permission(&user_perms, "staff.create"));
        assert!(has_permission(&user_perms, "staff.read"));
        
        // Students only has read
        assert!(has_permission(&user_perms, "students.read"));
        assert!(!has_permission(&user_perms, "students.create"));
    }
}
