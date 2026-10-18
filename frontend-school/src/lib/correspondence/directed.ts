import type { DirectedAssignment, ProcessAction } from '#lib/api/correspondence.js';
import { actionLabels } from './presentation.js';
export function directedActionLabel(action: ProcessAction, assignment: DirectedAssignment): string {
	if (action === 'reject') return 'ไม่อนุมัติทั้งเรื่อง';
	if (action === 'return_for_revision') return 'ส่งทั้งเรื่องกลับแก้ไข';
	if (action === 'review') {
		if (assignment.status === 'waiting_review') return 'ตรวจทานผล';
		if (assignment.purpose === 'decide') return 'พิจารณาเสร็จและจบเรื่อง';
	}
	if (action === 'reassign')
		return assignment.status === 'waiting_review' ? 'เปลี่ยนผู้ตรวจทาน' : 'เปลี่ยนผู้รับผิดชอบ';
	return actionLabels[action];
}
