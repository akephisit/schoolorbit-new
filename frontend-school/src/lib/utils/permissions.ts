/**
 * Permission utility functions for granular CRUD permission checking
 * 
 * Supports:
 * - Granular permissions: staff.create, staff.read, staff.update, staff.delete
 * - Wildcard permissions: staff (grants all staff.*)
 */

/**
 * Check if user has required permission
 * 
 * Logic:
 * 1. Check for exact match (e.g., "staff.create")
 * 2. Check for wildcard permission (e.g., "staff" grants all "staff.*")
 * 
 * @param userPermissions - Array of user's permission strings
 * @param required - Required permission string
 * @returns true if user has permission
 * 
 * @example
 * ```ts
 * const userPerms = ['staff.read', 'students'];
 * 
 * // Exact match
 * hasPermission(userPerms, 'staff.read'); // true
 * 
 * // Wildcard match
 * hasPermission(userPerms, 'students.create'); // true (wildcard)
 * hasPermission(userPerms, 'students.delete'); // true (wildcard)
 * 
 * // No match
 * hasPermission(userPerms, 'staff.delete'); // false
 * ```
 */
export function hasPermission(
    userPermissions: string[],
    required: string
): boolean {
    // Check for exact match first
    if (userPermissions.includes(required)) {
        return true;
    }

    // Check for wildcard permission
    // If required is "staff.create" and user has "staff", grant access
    const [resource] = required.split('.');
    if (resource && userPermissions.includes(resource)) {
        return true;
    }

    return false;
}

/**
 * Check if user has ANY of the required permissions
 * 
 * @param userPermissions - Array of user's permission strings  
 * @param requiredPermissions - Array of required permission strings (OR logic)
 * @returns true if user has at least one permission
 * 
 * @example
 * ```ts
 * const userPerms = ['staff.read'];
 * 
 * // User has at least one
 * hasAnyPermission(userPerms, ['staff.read', 'staff.create']); // true
 * 
 * // User has none
 * hasAnyPermission(userPerms, ['staff.delete', 'staff.update']); // false
 * ```
 */
export function hasAnyPermission(
    userPermissions: string[],
    requiredPermissions: string[]
): boolean {
    return requiredPermissions.some((required) =>
        hasPermission(userPermissions, required)
    );
}

/**
 * Check if user has ALL of the required permissions
 * 
 * @param userPermissions - Array of user's permission strings
 * @param requiredPermissions - Array of required permission strings (AND logic)
 * @returns true if user has all permissions
 * 
 * @example
 * ```ts
 * const userPerms = ['staff.read', 'staff.create'];
 * 
 * // User has all
 * hasAllPermissions(userPerms, ['staff.read', 'staff.create']); // true
 * 
 * // User missing one
 * hasAllPermissions(userPerms, ['staff.read', 'staff.delete']); // false
 * ```
 */
export function hasAllPermissions(
    userPermissions: string[],
    requiredPermissions: string[]
): boolean {
    return requiredPermissions.every((required) =>
        hasPermission(userPermissions, required)
    );
}
