export const STAFF_STATUS_OPTIONS = [
	{ value: 'active', label: 'ใช้งาน' },
	{ value: 'inactive', label: 'ปิดการใช้งาน' },
	{ value: 'suspended', label: 'ระงับ' },
	{ value: 'resigned', label: 'ลาออก' },
	{ value: 'retired', label: 'เกษียณ' }
];

export function staffStatusLabel(status: string): string {
	return STAFF_STATUS_OPTIONS.find((option) => option.value === status)?.label ?? status;
}
