import type { PermissionsByModule } from '#lib/api/roles.js';
import {
	permissionActionLabel,
	permissionModuleLabel,
	permissionScopeMeta
} from '#lib/permissions/registry.js';

/** Filter presentation only; saved grants always come from the complete draft. */
export function filterOrganizationPermissions(
	modules: PermissionsByModule,
	query: string
): PermissionsByModule {
	const terms = query.trim().toLocaleLowerCase('th').split(/\s+/).filter(Boolean);
	const result: PermissionsByModule = {};
	for (const [module, permissions] of Object.entries(modules)) {
		const visible = permissions.filter((permission) => {
			const scope = permissionScopeMeta(permission.scope);
			const text =
				`${permissionModuleLabel(module)} ${module} ${permission.name} ${permission.description ?? ''} ${permission.code} ${permissionActionLabel(permission.action)} ${scope.label} ${scope.description}`.toLocaleLowerCase(
					'th'
				);
			return terms.every((term) => text.includes(term));
		});
		if (visible.length) result[module] = visible;
	}
	return result;
}
