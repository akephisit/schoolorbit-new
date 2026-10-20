import type { PublicSchoolOrganization } from '#lib/api/school.js';

type Unit = PublicSchoolOrganization['units'][number];
export type PublicOrganizationMember = Unit['members'][number];
export type PublicPerson = {
	member: PublicOrganizationMember;
	responsibilities: { unitId: string; unitName: string; title: string }[];
};
export type PublicPersonnelGroup = {
	unit: Unit;
	leaders: PublicPerson[];
	members: PublicPerson[];
	children: PublicPersonnelGroup[];
};
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
	const grouped = new Map<string, Unit['members']>();
	for (const member of members) {
		const group = grouped.get(member.positionCode) ?? [];
		group.push(member);
		grouped.set(member.positionCode, group);
	}
	return [...grouped]
		.sort(([left], [right]) => {
			const rank = (code: string) =>
				positionOrder.includes(code) ? positionOrder.indexOf(code) : positionOrder.length;
			return rank(left) - rank(right);
		})
		.map(([code, members]) => ({ code, label: publicPositionLabel(code), members }));
}

const positionOrder = [
	'director',
	'deputy_director',
	'head',
	'deputy_head',
	'coordinator',
	'member'
];
const positionRank = (member: PublicOrganizationMember) => {
	const rank = positionOrder.indexOf(member.positionCode);
	return rank < 0 ? positionOrder.length : rank;
};

/** Combine a person's assignments without merging different people who share a display name. */
function people(assignments: { unit: Unit; member: PublicOrganizationMember }[]): PublicPerson[] {
	const result = new Map<string, PublicPerson>();
	for (const { unit, member } of [...assignments].sort(
		(left, right) => positionRank(left.member) - positionRank(right.member)
	)) {
		let person = result.get(member.personKey);
		if (!person) {
			person = { member, responsibilities: [] };
			result.set(member.personKey, person);
		}
		const title = member.positionTitle?.trim() || publicPositionLabel(member.positionCode);
		if (!person.responsibilities.some((item) => item.unitId === unit.id && item.title === title)) {
			person.responsibilities.push({ unitId: unit.id, unitName: unit.name, title });
		}
	}
	return [...result.values()];
}

function highestMembers(unit: Unit): PublicOrganizationMember[] {
	return groupPublicOrganizationMembers(unit.members)[0]?.members ?? [];
}

export function publicExecutiveTeam(units: Unit[]) {
	// Subject groups and their descendants have their own directory, even in historical hierarchies.
	const administration: Unit[] = [];
	const visit = (nodes: PublicOrganizationNode[]) => {
		for (const node of nodes) {
			if (node.unitType === 'subject_group') continue;
			administration.push(node);
			visit(node.children);
		}
	};
	visit(buildPublicOrganizationTree(units));
	const selected = people(
		administration.flatMap((unit) => {
			const highest = unit.unitType === 'management_group' ? highestMembers(unit) : [];
			return unit.members
				.filter(
					(member) =>
						member.positionCode === 'director' ||
						member.positionCode === 'deputy_director' ||
						highest.includes(member)
				)
				.map((member) => ({ unit, member }));
		})
	);
	const directors = selected.filter((person) => person.member.positionCode === 'director');
	const deputies = selected.filter((person) => person.member.positionCode === 'deputy_director');
	const leaders = selected.filter(
		(person) => !['director', 'deputy_director'].includes(person.member.positionCode)
	);
	return { directors, deputies, leaders };
}

/** Preserve each branch once, including vacant units and the members of the lowest-level work. */
export function publicPersonnelGroups(
	units: Unit[],
	unitType: 'management_group' | 'subject_group'
): PublicPersonnelGroup[] {
	function group(node: PublicOrganizationNode): PublicPersonnelGroup {
		const assignments = node.members.map((member) => ({ unit: node as Unit, member }));
		function inherit(children: PublicOrganizationNode[]) {
			for (const child of children) {
				if (child.unitType === 'subject_group') continue;
				assignments.push(...child.members.map((member) => ({ unit: child, member })));
				inherit(child.children);
			}
		}
		inherit(node.children);
		const all = people(assignments);
		const leaderKeys = new Set(highestMembers(node).map((member) => member.personKey));
		return {
			unit: node,
			leaders: all.filter((person) => leaderKeys.has(person.member.personKey)),
			members: all.filter((person) => !leaderKeys.has(person.member.personKey)),
			children: descendants(node.children)
		};
	}
	function descendants(nodes: PublicOrganizationNode[]): PublicPersonnelGroup[] {
		return nodes
			.filter((node) => node.unitType !== 'subject_group' && node.unitType !== 'management_group')
			.map(group);
	}
	const result: PublicPersonnelGroup[] = [];
	function visit(nodes: PublicOrganizationNode[]) {
		for (const node of nodes) {
			if (unitType === 'subject_group' ? Boolean(node.learningAreaId) : node.unitType === unitType)
				result.push(group(node));
			visit(node.children);
		}
	}
	visit(buildPublicOrganizationTree(units));
	return result;
}
