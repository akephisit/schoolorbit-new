import type { AcademicYear, BellSchedule, Homeroom } from '#lib/api/academic-core.js';

export function canUpdateHomeroomCapacity(status: AcademicYear['status']): boolean {
	return status === 'planning' || status === 'ready' || status === 'active';
}

export function homeroomCapacityError(capacity: number, studentCount: number | null | undefined): string {
	if (!Number.isSafeInteger(capacity) || capacity < 1) return 'ระบุความจุเป็นจำนวนเต็มตั้งแต่ 1 คน';
	if (studentCount != null && capacity < studentCount)
		return `ความจุต้องไม่น้อยกว่านักเรียนในห้อง ${studentCount} คน`;
	return '';
}

export function homeroomCapacitySnapshot(room: Homeroom) {
	return { id: room.id, capacity: room.capacity, studentCount: room.studentCount, rowVersion: room.rowVersion };
}

export function bellScheduleBlockMessage(reason: BellSchedule['editBlockedReason']): string {
	if (reason === 'already_used') return 'ตารางนี้ใช้กับภาคเรียนที่เริ่มใช้แล้ว ให้คัดลอกเพื่อตั้งค่าภาคเรียนถัดไป';
	if (reason === 'pinned_timetable') return 'ตารางนี้มีตารางสอนอ้างอิงแล้ว ให้คัดลอกก่อนแก้ไข';
	if (reason === 'no_unopened_term') return 'ปีนี้ไม่มีภาคเรียนที่ยังไม่เริ่มใช้สำหรับตั้งค่าตารางเวลา';
	if (reason === 'year_not_editable') return 'ปีการศึกษานี้ปิดการแก้ไขตารางเวลาแล้ว';
	return '';
}
