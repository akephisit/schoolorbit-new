export type OrganizationPositionCode =
	'director' | 'deputy_director' | 'head' | 'deputy_head' | 'coordinator' | 'member';

type OrganizationPositionOption = {
	value: OrganizationPositionCode;
	label: string;
	group: 'leadership' | 'coordination' | 'member';
};

export const organizationPositionOptions: readonly OrganizationPositionOption[] = [
	{ value: 'director', label: 'ผู้อำนวยการ', group: 'leadership' },
	{ value: 'deputy_director', label: 'รองผู้อำนวยการ', group: 'leadership' },
	{ value: 'head', label: 'หัวหน้า', group: 'leadership' },
	{ value: 'deputy_head', label: 'รองหัวหน้า', group: 'coordination' },
	{ value: 'coordinator', label: 'ผู้ประสานงาน', group: 'coordination' },
	{ value: 'member', label: 'สมาชิก', group: 'member' }
];

export function organizationPositionLabel(code: string) {
	return organizationPositionOptions.find((position) => position.value === code)?.label ?? code;
}
