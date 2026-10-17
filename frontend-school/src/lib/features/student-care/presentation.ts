import type {
	CareAssignmentStatus,
	CareRespondent,
	CareFamily,
	CareVisitStatus,
	CareCaseStatus,
	CareEventKind,
	CareRoundStatus
} from '#lib/api/student-care.js';
export const respondentLabels: Record<CareRespondent, string> = {
	self_report: 'นักเรียนประเมินตนเอง',
	teacher: 'ครูประเมิน',
	guardian: 'ผู้ปกครองประเมิน'
};
export const familyLabels: Record<CareFamily, string> = {
	sdq: 'SDQ',
	eq: 'EQ',
	depression: 'คัดกรองภาวะซึมเศร้า',
	other: 'แบบประเมินอื่น'
};
export const assignmentLabels: Record<CareAssignmentStatus, string> = {
	assigned: 'รอตอบ',
	in_progress: 'กำลังตอบ',
	submitted: 'ส่งแล้ว',
	cancelled: 'ยกเลิก'
};
export const roundLabels: Record<CareRoundStatus, string> = {
	open: 'เปิดรับคำตอบ',
	closed: 'ปิดรับคำตอบ',
	cancelled: 'ยกเลิก'
};
export const visitLabels: Record<CareVisitStatus, string> = {
	planned: 'วางแผน',
	completed: 'เยี่ยมแล้ว',
	cancelled: 'ยกเลิก'
};
export const caseLabels: Record<CareCaseStatus, string> = {
	open: 'เปิดการดูแล',
	monitoring: 'ติดตาม',
	closed: 'ปิดการดูแล'
};
export const eventLabels: Record<CareEventKind, string> = {
	note: 'บันทึก',
	contact: 'ติดต่อ',
	referral: 'ส่งต่อ',
	follow_up: 'ติดตาม',
	acknowledgement: 'รับทราบ',
	closure: 'ปิดการดูแล',
	reopened: 'เปิดการดูแลอีกครั้ง'
};
export function careDate(value: string | null | undefined) {
	if (!value) return 'ยังไม่ระบุ';
	const date = new Date(value.length === 10 ? `${value}T12:00:00+07:00` : value);
	return Number.isNaN(date.getTime())
		? value
		: new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium', timeZone: 'Asia/Bangkok' }).format(
				date
			);
}
export function careToday() {
	return new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(new Date());
}
export function careLink(suffix: string, term?: string | null) {
	const query = new URLSearchParams();
	if (term) query.set('academicTermId', term);
	return `/staff/student-care${suffix}${query.size ? `?${query}` : ''}`;
}
