import type { ExamScheduleReadinessFinding } from '$lib/api/examSchedule';

export function examScheduleReadinessLabel(finding: ExamScheduleReadinessFinding): string {
	switch (finding.code) {
		case 'missing_exam_day':
			return 'ยังไม่ได้เพิ่มวันสอบ';
		case 'missing_exam_items':
			return 'ยังไม่มีรายการสอบที่พร้อมนำเข้า';
		case 'unscheduled_exam_items':
			return `ยังไม่ได้จัดเวลา ${finding.count} รายการ`;
		case 'missing_room_assignments':
			return `ยังไม่ได้กำหนดห้องสอบ ${finding.count} ห้องเรียน/วัน`;
		case 'invalid_exam_sessions':
			return `เวลาสอบไม่ตรงกับเงื่อนไขวันสอบ ${finding.count} รายการ`;
		case 'missing_seat_assignments':
			return `ยังไม่ได้สร้างเลขที่นั่งให้นักเรียน ${finding.count} คน`;
		case 'invigilator_conflicts':
			return `กรรมการคุมสอบมีเวลาชนกัน ${finding.count} จุด`;
		case 'pending_source_changes':
			return `ยังไม่ได้ซิงก์การเปลี่ยนแปลงโครงสร้างคะแนน ${finding.count} รายการ`;
		default:
			return `มีรายการที่ต้องตรวจสอบ ${finding.count} รายการ`;
	}
}
