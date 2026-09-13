import type { components } from '$lib/api/generated/school-api';

type Workspace = components['schemas']['TermLifecycleWorkspace'];
type Action = components['schemas']['TermTransitionAction'];

export function canConfirmTransition(
	workspace: Workspace,
	action: Action,
	closedOn: string | undefined,
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
		if (!closedOn || !/^\d{4}-\d{2}-\d{2}$/.test(closedOn)) return false;
		const timestamp = Date.parse(`${closedOn}T00:00:00Z`);
		if (!Number.isFinite(timestamp) || new Date(timestamp).toISOString().slice(0, 10) !== closedOn)
			return false;
		if (
			closedOn < workspace.context.termStartDate ||
			closedOn < workspace.context.yearStartDate ||
			closedOn > workspace.context.yearEndDate
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
	if (closedOn || acknowledged.length > 0) return false;
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
