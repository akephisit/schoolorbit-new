import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const budgetReadPermissions = [
	PERMISSIONS.BUDGET_READ_OWN,
	PERMISSIONS.BUDGET_READ_ASSIGNED,
	PERMISSIONS.BUDGET_READ_ORGANIZATION_UNIT,
	PERMISSIONS.BUDGET_READ_ORGANIZATION_TREE,
	PERMISSIONS.BUDGET_READ_SCHOOL
] as const;
export const budgetRoutePermissions = [
	PERMISSION_MODULES.BUDGET,
	PERMISSION_MODULES.BUDGET_PAYMENT,
	PERMISSION_MODULES.BUDGET_ADJUSTMENT
];
export const budgetUpdatePermissions = [
	PERMISSIONS.BUDGET_UPDATE_OWN,
	PERMISSIONS.BUDGET_UPDATE_ORGANIZATION_UNIT,
	PERMISSIONS.BUDGET_UPDATE_ORGANIZATION_TREE,
	PERMISSIONS.BUDGET_UPDATE_SCHOOL
] as const;
export function canReadBudget(): boolean {
	return get(can).hasAny(...budgetReadPermissions);
}
