import type { components } from '#lib/api/generated/school-api.js';

type Workspace = components['schemas']['TermLifecycleWorkspace'];
type Action = components['schemas']['TermTransitionAction'];

export function canConfirmTransition(
	workspace: Workspace,
	action: Action,
	reason: string,
	acknowledged: string[]
): boolean {
	if (!workspace.availableActions.includes(action)) return false;
	if (action === 'close') {
		if (
			!workspace.coverage.ready ||
			workspace.findings.some((finding) => finding.severity === 'blocking')
		)
			return false;
		const warnings = workspace.findings
			.filter((finding) => finding.severity === 'warning')
			.map((finding) => finding.code);
		return (
			new Set(acknowledged).size === acknowledged.length &&
			warnings.length === acknowledged.length &&
			warnings.every((code) => acknowledged.includes(code))
		);
	}
	if (acknowledged.length > 0) return false;
	if (action === 'reopen') return reason.trim().length > 0 && Array.from(reason).length <= 1000;
	return true;
}

export const transitionLabels: Record<Action, string> = {
	mark_ready: 'พร้อมเริ่มภาคเรียน',
	begin_closing: 'เริ่มขั้นตอนปิดภาคเรียน',
	cancel_closing: 'ยกเลิกขั้นตอนปิด',
	close: 'ปิดภาคเรียน',
	reopen: 'เปิดกลับเพื่อตรวจสอบ',
	cancel: 'ยกเลิกภาคเรียนที่วางแผน',
	activate: 'เริ่มใช้ภาคเรียนนี้'
};

export const termStatusLabels: Record<components['schemas']['AcademicTermStatus'], string> = {
	planning: 'เตรียมภาคเรียน',
	ready: 'พร้อมเริ่ม',
	active: 'ใช้งาน',
	closing: 'กำลังปิด',
	closed: 'ปิดแล้ว',
	cancelled: 'ยกเลิกแล้ว'
};
