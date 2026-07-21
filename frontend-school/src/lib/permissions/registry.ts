export { PERMISSIONS, PERMISSION_MODULES, WILDCARD_PERMISSION } from './registry.generated';
export type { PermissionCode, PermissionModule, RoutePermission } from './registry.generated';

type PermissionMeta = {
	label: string;
	description: string;
	tone: 'default' | 'muted' | 'warning' | 'danger';
};

const SCOPE_META: Record<string, PermissionMeta> = {
	own: {
		label: 'เฉพาะตนเอง',
		description: 'ใช้กับข้อมูลหรือรายการที่เป็นของผู้ใช้นั้นเอง',
		tone: 'muted'
	},
	assigned: {
		label: 'ที่รับผิดชอบ',
		description: 'ใช้กับข้อมูลที่ผู้ใช้นั้นได้รับมอบหมายโดยตรง',
		tone: 'default'
	},
	organization_unit: {
		label: 'หน่วยงานเดียวกัน',
		description: 'ใช้กับข้อมูลในหน่วยงานที่ผู้ใช้สังกัดเท่านั้น',
		tone: 'default'
	},
	organization_tree: {
		label: 'สายงาน/หน่วยงานย่อย',
		description: 'ใช้กับหน่วยงานที่สังกัดและหน่วยงานย่อยในสายงาน',
		tone: 'warning'
	},
	school: {
		label: 'ทั้งโรงเรียน',
		description: 'ใช้กับข้อมูลทั้งโรงเรียน ต้องให้เฉพาะบทบาทที่ดูแลภาพรวม',
		tone: 'danger'
	},
	all: {
		label: 'ทั้งหมด',
		description: 'สิทธิ์ระดับระบบหรืองาน admin ที่ไม่ผูกกับ resource รายบุคคล',
		tone: 'danger'
	},
	global: {
		label: 'ระบบทั้งหมด',
		description: 'สิทธิ์สูงสุดสำหรับผู้ดูแลระบบ',
		tone: 'danger'
	}
};

const ACTION_LABELS: Record<string, string> = {
	all: 'ทั้งหมด',
	approve: 'อนุมัติ',
	assign: 'มอบหมาย',
	create: 'สร้าง',
	delete: 'ลบ',
	enroll: 'มอบตัว',
	execute: 'ดำเนินการ',
	manage: 'จัดการ',
	manage_members: 'จัดการสมาชิก',
	read: 'ดู',
	remove: 'ถอดออก',
	request: 'ส่งคำขอ',
	scores: 'คะแนน',
	update: 'แก้ไข',
	evaluate: 'ประเมิน',
	verify: 'ตรวจสอบ'
};

export function permissionScopeMeta(scope: string | undefined): PermissionMeta {
	return (
		SCOPE_META[scope ?? ''] ?? {
			label: scope || 'ไม่ระบุขอบเขต',
			description: 'ขอบเขตนี้ยังไม่มีคำอธิบายในระบบ',
			tone: 'warning'
		}
	);
}

export function permissionActionLabel(action: string | undefined): string {
	return ACTION_LABELS[action ?? ''] ?? action ?? 'ไม่ระบุการทำงาน';
}

export function permissionScopeToneClass(tone: PermissionMeta['tone']): string {
	switch (tone) {
		case 'danger':
			return 'border-red-200 bg-red-50 text-red-700';
		case 'warning':
			return 'border-amber-200 bg-amber-50 text-amber-700';
		case 'default':
			return 'border-blue-200 bg-blue-50 text-blue-700';
		default:
			return 'border-muted bg-muted/40 text-muted-foreground';
	}
}
