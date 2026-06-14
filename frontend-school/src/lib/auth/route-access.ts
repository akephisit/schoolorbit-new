import type { User } from '$lib/stores/auth';
import type { RoutePermission } from '$lib/permissions/registry';
import {
	hasModulePermission,
	hasPermission,
	hasWorkflowManagePermission
} from '$lib/stores/permissions';

type RouteAccessMeta = {
	user_type?: string;
	permission?: RoutePermission;
	workflowManage?: boolean;
};

type RouteMetaModule = {
	_meta?: {
		menu?: RouteAccessMeta;
		access?: RouteAccessMeta;
	};
};

export type RouteAccess = {
	userType?: string;
	permission?: RoutePermission;
	workflowManage?: boolean;
};

export type DashboardPath = '/staff' | '/student' | '/parent';

const routeModules = import.meta.glob('/src/routes/(app)/**/+page.ts', {
	eager: true
}) as Record<string, RouteMetaModule>;

const routeAccessById = new Map<string, RouteAccess>();

for (const [filePath, module] of Object.entries(routeModules)) {
	const access = module._meta?.access ?? module._meta?.menu;
	if (!access?.user_type && !access?.permission && !access?.workflowManage) continue;

	const routeId = filePath.replace('/src/routes', '').replace('/+page.ts', '');
	routeAccessById.set(routeId, {
		userType: access.user_type,
		permission: access.permission,
		workflowManage: access.workflowManage
	});
}

export function getRouteAccess(routeId: string | null): RouteAccess | undefined {
	if (!routeId) return undefined;

	let currentRouteId = routeId;
	while (currentRouteId.length > 0) {
		const access = routeAccessById.get(currentRouteId);
		if (access) return access;

		const lastSlash = currentRouteId.lastIndexOf('/');
		if (lastSlash <= 0) break;
		currentRouteId = currentRouteId.slice(0, lastSlash);
	}

	return undefined;
}

export function routePermissionMatches(
	permissions: string[],
	requiredPermission?: string,
	workflowManage = false
): boolean {
	if (workflowManage && !hasWorkflowManagePermission(permissions)) return false;
	if (!requiredPermission) return true;

	return requiredPermission.includes('.')
		? hasPermission(permissions, requiredPermission)
		: hasModulePermission(permissions, requiredPermission);
}

export function userCanAccessRoute(
	user: User | null,
	permissions: string[],
	routeId: string | null
): boolean {
	const access = getRouteAccess(routeId);
	if (!access) return true;
	if (!user) return false;

	if (access.userType && user.user_type !== access.userType) {
		return false;
	}

	return routePermissionMatches(permissions, access.permission, access.workflowManage);
}

export function dashboardPathForUser(user: User | null): DashboardPath {
	if (user?.user_type === 'student') return '/student';
	if (user?.user_type === 'parent') return '/parent';
	return '/staff';
}
