import type { OrganizationMemberItem, OrganizationUnit } from '#lib/api/staff.js';
import { organizationPositionOptions } from './organization-positions.ts';

export type OrganizationPerson = OrganizationMemberItem & {
	isDirect: boolean;
	sourceAssignments: OrganizationMemberItem[];
};

/** A display projection only: assignments remain the source for every mutation. */
export function organizationPeople(assignments: OrganizationMemberItem[], unitId: string): OrganizationPerson[] {
	const rank = (code: string) => {
		const index = organizationPositionOptions.findIndex((option) => option.value === code);
		return index < 0 ? organizationPositionOptions.length : index;
	};
	const people = new Map<string, OrganizationPerson>();
	for (const source of [...assignments].sort((a, b) =>
		Number(b.organization_unit_id === unitId) - Number(a.organization_unit_id === unitId) ||
		rank(a.position_code) - rank(b.position_code) || a.name.localeCompare(b.name, 'th')
	)) {
		let person = people.get(source.user_id);
		if (!person) {
			const isDirect = source.organization_unit_id === unitId;
			person = { ...source, position_code: isDirect ? source.position_code : 'member', position_title: isDirect ? source.position_title : null, is_primary: isDirect && source.is_primary, isDirect, sourceAssignments: [] };
			people.set(source.user_id, person);
		}
		if (!person.sourceAssignments.some((item) => item.organization_unit_id === source.organization_unit_id)) person.sourceAssignments.push(source);
	}
	return [...people.values()];
}

type Assignment = { organization_unit_id: string; is_primary: boolean };
export function organizationPrimaryDomain(unitId: string, units: OrganizationUnit[]) {
	return units.find((unit) => unit.id === unitId)?.unit_type === 'subject_group' ? 'learning_area' : 'administration';
}
export function organizationPrimaryLabel(unitId: string, units: OrganizationUnit[]) {
	return organizationPrimaryDomain(unitId, units) === 'learning_area' ? 'กลุ่มสาระหลัก' : 'สังกัดบริหารหลัก';
}
export function setOrganizationPrimary<T extends Assignment>(assignments: T[], index: number, units: OrganizationUnit[]): T[] {
	const selected = assignments[index];
	if (!selected) return assignments;
	const domain = organizationPrimaryDomain(selected.organization_unit_id, units);
	return assignments.map((assignment, current) => organizationPrimaryDomain(assignment.organization_unit_id, units) === domain ? { ...assignment, is_primary: current === index } : assignment);
}
export function organizationPrimaryError(assignments: Assignment[], units: OrganizationUnit[]): string {
	for (const domain of ['administration', 'learning_area'] as const) {
		const selected = assignments.filter((assignment) => organizationPrimaryDomain(assignment.organization_unit_id, units) === domain);
		if (selected.length && selected.filter((assignment) => assignment.is_primary).length !== 1) return domain === 'learning_area' ? 'กรุณาระบุกลุ่มสาระหลัก 1 กลุ่มสาระ' : 'กรุณาระบุสังกัดบริหารหลัก 1 หน่วยงาน';
	}
	return '';
}
