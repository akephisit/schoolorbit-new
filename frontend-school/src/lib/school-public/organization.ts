import type { PublicSchoolOrganization } from '$lib/api/school';

type Unit = PublicSchoolOrganization['units'][number];
export type PublicOrganizationNode = Unit & { children: PublicOrganizationNode[] };

/** Preserve orphaned units and bound cyclic legacy hierarchies without losing a unit. */
export function buildPublicOrganizationTree(units: Unit[]): PublicOrganizationNode[] {
	const ids = new Set(units.map((unit) => unit.id));
	const children = new Map<string, Unit[]>();
	for (const unit of units) {
		if (unit.parentId && ids.has(unit.parentId) && unit.parentId !== unit.id) {
			const siblings = children.get(unit.parentId) ?? [];
			siblings.push(unit);
			children.set(unit.parentId, siblings);
		}
	}
	const visited = new Set<string>();
	function node(unit: Unit): PublicOrganizationNode {
		visited.add(unit.id);
		const result: PublicOrganizationNode = { ...unit, children: [] };
		for (const child of children.get(unit.id) ?? []) {
			if (!visited.has(child.id)) result.children.push(node(child));
		}
		return result;
	}
	const roots = units
		.filter((unit) => !unit.parentId || !ids.has(unit.parentId) || unit.parentId === unit.id)
		.map(node);
	for (const unit of units) {
		if (!visited.has(unit.id)) roots.push(node(unit));
	}
	return roots;
}

export function publicPositionLabel(code: string): string {
	const titles: Record<string, string> = {
		director: 'ผู้อำนวยการ',
		deputy_director: 'รองผู้อำนวยการ',
		head: 'หัวหน้า',
		deputy_head: 'รองหัวหน้า'
	};
	return titles[code] ?? 'ผู้บริหาร';
}
