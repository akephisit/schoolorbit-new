import {
	BarChart3,
	CalendarDays,
	ClipboardCheck,
	ClipboardList,
	Inbox,
	UserRound
} from '@lucide/svelte';
import { userCanAccessRoute } from '#lib/auth/route-access.js';
import type { User } from '#lib/stores/auth.js';

export const supervisionHomePath = '/staff/academic/supervision';
export const supervisionHomeLabel = 'หน้าหลักนิเทศการสอน';

const workflows = [
	{
		section: 'mine',
		label: 'ของฉัน',
		description: 'จองคาบ ติดตามคำขอ และรับทราบผล',
		icon: UserRound,
		group: 'work'
	},
	{
		section: 'evaluate',
		label: 'ประเมิน',
		description: 'ดูคาบที่ได้รับมอบหมายและส่งผลประเมิน',
		icon: ClipboardCheck,
		group: 'work'
	},
	{
		section: 'requests',
		label: 'คำขอจอง',
		description: 'ตรวจคำขอและมอบหมายผู้ประเมิน',
		icon: Inbox,
		group: 'work'
	},
	{
		section: 'overview',
		label: 'ภาพรวม',
		description: 'ติดตามสถานะครูและความคืบหน้ารอบนิเทศ',
		icon: BarChart3,
		group: 'work'
	},
	{
		section: 'cycles',
		label: 'รอบนิเทศ',
		description: 'กำหนดช่วงนิเทศและเปิดรับคำขอ',
		icon: CalendarDays,
		group: 'setup'
	},
	{
		section: 'templates',
		label: 'แบบประเมิน',
		description: 'สร้างหัวข้อและเกณฑ์ประเมิน',
		icon: ClipboardList,
		group: 'setup'
	}
] as const;

export const supervisionDestinations = workflows.map((workflow) => ({
	...workflow,
	path: `${supervisionHomePath}/${workflow.section}`
}));

export function visibleSupervisionDestinations(user: User | null, permissions: string[]) {
	return supervisionDestinations.filter(({ path }) =>
		userCanAccessRoute(user, permissions, `/(app)${path}`)
	);
}
