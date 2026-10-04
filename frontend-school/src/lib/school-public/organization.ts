import type { PublicSchoolOrganization } from '#lib/api/school.js';

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
		deputy_head: 'รองหัวหน้า',
		coordinator: 'ผู้ประสานงาน',
		member: 'สมาชิก'
	};
	return titles[code] ?? code;
}

export function groupPublicOrganizationMembers(members: Unit['members']) {
	const order = ['director', 'deputy_director', 'head', 'deputy_head', 'coordinator', 'member'];
	const grouped = new Map<string, Unit['members']>();
	for (const member of members) {
		const group = grouped.get(member.positionCode) ?? [];
		group.push(member);
		grouped.set(member.positionCode, group);
	}
	return [...grouped]
		.sort(([left], [right]) => {
			const rank = (code: string) => (order.includes(code) ? order.indexOf(code) : order.length);
			return rank(left) - rank(right);
		})
		.map(([code, members]) => ({ code, label: publicPositionLabel(code), members }));
}
