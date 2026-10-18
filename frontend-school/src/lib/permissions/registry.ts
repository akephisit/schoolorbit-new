import { PERMISSION_MODULES } from './registry.generated.js';
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
	activate: 'เริ่มใช้งาน',
	all: 'ทั้งหมด',
	approve: 'อนุมัติ',
	assign: 'มอบหมาย',
	close: 'ปิด',
	correct: 'แก้ไขผล',
	create: 'สร้าง',
	delete: 'ลบ',
	enroll: 'มอบตัว',
	execute: 'ดำเนินการ',
	lock: 'ล็อกผล',
	manage: 'จัดการ',
	manage_members: 'จัดการสมาชิก',
	read: 'ดู',
	remove: 'ถอดออก',
	reopen: 'เปิดกลับ',
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
			return 'border-destructive/20 bg-destructive/10 text-destructive';
		case 'warning':
			return 'border-warning/20 bg-warning/10 text-warning';
		case 'default':
			return 'border-primary/20 bg-primary/10 text-primary';
		default:
			return 'border-muted bg-muted/40 text-muted-foreground';
	}
}

const MODULE_LABELS: Record<string, string> = {
	[PERMISSION_MODULES.STAFF_PROFILE]: 'ข้อมูลบุคลากร',
	[PERMISSION_MODULES.STAFF_PII]: 'ข้อมูลส่วนบุคคลของบุคลากร',
	[PERMISSION_MODULES.STAFF]: 'จัดการบุคลากร',
	[PERMISSION_MODULES.STUDENT]: 'ข้อมูลนักเรียน',
	[PERMISSION_MODULES.STUDENT_PII]: 'ข้อมูลส่วนบุคคลของนักเรียน',
	[PERMISSION_MODULES.ROLES]: 'บทบาทและสิทธิ์',
	[PERMISSION_MODULES.CERTIFICATE]: 'เกียรติบัตร',
	[PERMISSION_MODULES.ACHIEVEMENT]: 'ผลงานและรางวัล',
	[PERMISSION_MODULES.CALENDAR]: 'ปฏิทิน',
	[PERMISSION_MODULES.ADMISSION]: 'รับสมัครนักเรียน',
	[PERMISSION_MODULES.ORGANIZATION_WORK]: 'งานของหน่วยงาน',
	[PERMISSION_MODULES.SUPERVISION]: 'การนิเทศ',
	[PERMISSION_MODULES.SETTINGS]: 'ตั้งค่าระบบ',
	[PERMISSION_MODULES.MENU]: 'เมนูบริการ',
	[PERMISSION_MODULES.HOMEROOM]: 'ห้องประจำชั้น',
	[PERMISSION_MODULES.LEARNING_OFFERING]: 'การจัดการเรียนรู้',
	[PERMISSION_MODULES.ACADEMIC_CATALOG]: 'รายวิชา',
	[PERMISSION_MODULES.ACADEMIC_CURRICULUM]: 'หลักสูตร',
	[PERMISSION_MODULES.ACADEMIC_ASSESSMENT]: 'การวัดผล',
	[PERMISSION_MODULES.ACADEMIC_GRADEBOOK]: 'บันทึกคะแนน',
	[PERMISSION_MODULES.ACADEMIC_TIMETABLE]: 'ตารางสอน',
	[PERMISSION_MODULES.ACADEMIC_EXAM_SCHEDULE]: 'ตารางสอบ',
	[PERMISSION_MODULES.ACADEMIC_RESULT]: 'ผลการเรียน',
	[PERMISSION_MODULES.ACADEMIC_QUESTION_BANK]: 'คลังข้อสอบ',
	[PERMISSION_MODULES.ACADEMIC_YEAR]: 'ปีการศึกษา',
	[PERMISSION_MODULES.ACADEMIC_TERM]: 'ภาคเรียน',
	[PERMISSION_MODULES.ACADEMIC_PROMOTION]: 'การเลื่อนชั้น',
	[PERMISSION_MODULES.ACADEMIC_LIFECYCLE]: 'ปิดปีและภาคเรียน',
	[PERMISSION_MODULES.ACADEMIC_LEARNER_EVALUATION]: 'ประเมินผู้เรียน',
	[PERMISSION_MODULES.ACADEMIC_CONTEXT]: 'บริบทการศึกษา',
	[PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY]: 'ตารางสอนประจำวัน',
	[PERMISSION_MODULES.STUDENT_ACADEMIC_YEAR]: 'นักเรียนในปีการศึกษา',
	[PERMISSION_MODULES.FONT]: 'แบบอักษร',
	[PERMISSION_MODULES.FACILITY]: 'สถานที่',
	[PERMISSION_MODULES.FEATURES]: 'บริการของโรงเรียน',
	[PERMISSION_MODULES.DASHBOARD]: 'ภาพรวม',
	[PERMISSION_MODULES.SYSTEM]: 'ระบบ',
	[PERMISSION_MODULES.ATTENDANCE]: 'การมาเรียนและเช็กชื่อ',
	[PERMISSION_MODULES.BEHAVIOR]: 'คะแนนความประพฤติ',
	[PERMISSION_MODULES.BEHAVIOR_CLEANUP]: 'จัดการประวัติคะแนนความประพฤติ',
	[PERMISSION_MODULES.SUPPLY_CATALOG]: 'ทะเบียนพัสดุ',
	[PERMISSION_MODULES.STAFF_LEAVE]: 'การลาและออกนอกโรงเรียน'
};

export function permissionModuleLabel(module: string): string {
	return MODULE_LABELS[module] ?? module;
}
