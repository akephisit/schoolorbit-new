/// Permission module for granular CRUD permission checking
/// 
/// Supports both granular permissions (e.g., "staff.create") and wildcard permissions (e.g., "staff")

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
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
}

impl Permission {
    /// Parse permission string into Permission struct
    /// 
    /// Examples:
    /// - "staff.create" -> Permission { resource: "staff", action: Some(Create) }
    /// - "staff" -> Permission { resource: "staff", action: None }
    pub fn parse(permission_str: &str) -> Self {
        let parts: Vec<&str> = permission_str.split('.').collect();
        
        match parts.as_slice() {
            [resource] => Permission {
                resource: resource.to_string(),
                action: None, // Wildcard permission
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
                }
            },
            _ => Permission {
                resource: permission_str.to_string(),
                action: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_parse_granular() {
        let perm = Permission::parse("staff.create");
        assert_eq!(perm.resource, "staff");
        assert_eq!(perm.action, Some(Action::Create));
    }

    #[test]
    fn test_permission_parse_wildcard() {
        let perm = Permission::parse("staff");
        assert_eq!(perm.resource, "staff");
        assert_eq!(perm.action, None);
    }

    #[test]
    fn test_permission_to_string() {
        let perm1 = Permission {
            resource: "staff".to_string(),
            action: Some(Action::Create),
        };
        assert_eq!(perm1.to_string(), "staff.create");

        let perm2 = Permission {
            resource: "staff".to_string(),
            action: None,
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
