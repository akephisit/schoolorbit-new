/**
 * Permission utility functions for granular CRUD permission checking
 *
 * Supports:
 * - Granular permissions: staff.create, staff.read, staff.update, staff.delete
 * - Wildcard permissions: staff (grants all staff.*)
 * - Scoped permissions: attendance.update.own, grades.read.all, grades.read.department
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
export function hasPermission(userPermissions: string[], required: string): boolean {
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
 * Check if user has scoped permission with hierarchy support
 *
 * Scope Hierarchy: .all > .department > .own
 * - User with .all can access .department and .own
 * - User with .department can access .own
 * - User with .own can only access .own
 *
 * @param userPermissions - Array of user's permission strings
 * @param required - Required permission string (e.g., "attendance.update.own")
 * @returns true if user has permission with appropriate scope
 *
 * @example
 * ```ts
 * const userPerms = ['attendance.update.all'];
 *
 * // User with .all can access .own
 * hasScopedPermission(userPerms, 'attendance.update.own'); // true
 * hasScopedPermission(userPerms, 'attendance.update.department'); // true
 * hasScopedPermission(userPerms, 'attendance.update.all'); // true
 *
 * const userPerms2 = ['attendance.update.own'];
 * // User with .own cannot access .all
 * hasScopedPermission(userPerms2, 'attendance.update.all'); // false
 * ```
 */
export function hasScopedPermission(userPermissions: string[], required: string): boolean {
	// Check exact match or wildcard first
	if (hasPermission(userPermissions, required)) {
		return true;
	}

	// Parse required permission
	const parts = required.split('.');
	if (parts.length !== 3) {
		// Not a scoped permission, fallback to regular check
		return hasPermission(userPermissions, required);
	}

	const [resource, action, requiredScope] = parts;

	// Check if user has broader scope
	// Scope hierarchy: all > department > own
	const scopeHierarchy: Record<string, number> = {
		all: 3,
		department: 2,
		own: 1
	};

	const requiredLevel = scopeHierarchy[requiredScope] || 0;

	for (const userPerm of userPermissions) {
		const userParts = userPerm.split('.');

		// Check if it's a scoped permission
		if (userParts.length === 3) {
			const [userResource, userAction, userScope] = userParts;

			// Same resource and action
			if (userResource === resource && userAction === action) {
				const userLevel = scopeHierarchy[userScope] || 0;

				// User has equal or broader scope
				if (userLevel >= requiredLevel) {
					return true;
				}
			}
		} else if (userParts.length === 2) {
			// User has regular permission (treat as .all)
			const [userResource, userAction] = userParts;
			if (userResource === resource && userAction === action) {
				return true; // Regular permission grants all scopes
			}
		} else if (userParts.length === 1) {
			// Wildcard permission
			if (userParts[0] === resource) {
				return true; // Wildcard grants everything
			}
		}
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
	return requiredPermissions.some((required) => hasPermission(userPermissions, required));
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
	return requiredPermissions.every((required) => hasPermission(userPermissions, required));
}
