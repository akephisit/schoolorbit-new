import type { OrganizationUnit } from '#lib/api/staff.js';

export type UnitTypeFilter = 'all' | 'management_group' | 'division' | 'other';
export type OrganizationHierarchyNode = {
	unit: OrganizationUnit;
	matched: boolean;
	children: OrganizationHierarchyNode[];
};

export function organizationAncestors(unit: OrganizationUnit, units: OrganizationUnit[]) {
	const byId = new Map(units.map((item) => [item.id, item]));
	const ancestors: OrganizationUnit[] = [];
	const visited = new Set([unit.id]);
	let parent = unit.parent_unit_id ? byId.get(unit.parent_unit_id) : undefined;
	while (parent && !visited.has(parent.id)) {
		visited.add(parent.id);
		ancestors.unshift(parent);
		parent = parent.parent_unit_id ? byId.get(parent.parent_unit_id) : undefined;
	}
	return ancestors;
}

export function organizationUnitLabel(unit: OrganizationUnit) {
	if (unit.unit_type === 'school' || unit.code === 'SCHOOL') return 'โรงเรียน';
	if (unit.unit_type === 'management_group') return 'กลุ่มบริหาร';
	if (unit.unit_type === 'division') return 'ฝ่าย/งาน';
	if (unit.unit_type === 'committee') return 'คณะกรรมการ';
	if (unit.unit_type === 'team') return 'ทีม';
	return 'หน่วยงาน';
}

/** Keep ancestors as context when searching/filtering; never promote a matching child. */
export function organizationHierarchy(
	units: OrganizationUnit[],
	search = '',
	filter: UnitTypeFilter = 'all'
): OrganizationHierarchyNode[] {
	const sorted = [...units].sort(
		(a, b) =>
			(a.display_order ?? 0) - (b.display_order ?? 0) ||
			a.name.localeCompare(b.name, 'th') ||
			a.id.localeCompare(b.id)
	);
	const ids = new Set(sorted.map((unit) => unit.id));
	const children = new Map<string, OrganizationUnit[]>();
	for (const unit of sorted) {
		if (unit.parent_unit_id && ids.has(unit.parent_unit_id) && unit.parent_unit_id !== unit.id) {
			const siblings = children.get(unit.parent_unit_id) ?? [];
			siblings.push(unit);
			children.set(unit.parent_unit_id, siblings);
		}
	}
	const query = search.trim().toLocaleLowerCase('th');
	const visited = new Set<string>();
	function node(unit: OrganizationUnit, ancestorMatches = false): OrganizationHierarchyNode | null {
		visited.add(unit.id);
		const searchMatches =
			ancestorMatches ||
			!query ||
			`${unit.name} ${unit.name_en ?? ''}`.toLocaleLowerCase('th').includes(query);
		const typeMatches =
			filter === 'all' ||
			(filter === 'other'
				? !['school', 'management_group', 'division', 'subject_group'].includes(
						unit.unit_type ?? ''
					)
				: unit.unit_type === filter);
		const branches = (children.get(unit.id) ?? [])
			.filter((child) => !visited.has(child.id))
			.map((child) => node(child, searchMatches))
			.filter((child): child is OrganizationHierarchyNode => child !== null);
		const matched = searchMatches && typeMatches;
		return matched || branches.length ? { unit, matched, children: branches } : null;
	}
	const roots: OrganizationHierarchyNode[] = [];
	for (const unit of sorted.filter(
		(unit) =>
			!unit.parent_unit_id || !ids.has(unit.parent_unit_id) || unit.parent_unit_id === unit.id
	)) {
		const branch = node(unit);
		if (branch) roots.push(branch);
	}
	for (const unit of sorted) {
		if (!visited.has(unit.id)) {
			const branch = node(unit);
			if (branch) roots.push(branch);
		}
	}
	return roots;
}
