import type { components } from '$lib/api/generated/school-api';

type Schema = components['schemas'];
type Outcome = Schema['PromotionDecisionOutcome'];
type Item = Schema['PromotionRunItem'];
type Workspace = Schema['PromotionRunWorkspace'];
export type DecisionDraft = Omit<Schema['PromotionDecisionInput'], 'outcome'> & {
	outcome: Outcome | '';
};

export function initialDecision(item: Item): DecisionDraft {
	if (item.decision) return { ...item.decision };
	return {
		outcome: item.recommendation.suggestedOutcome ?? '',
		targetGradeLevelId: item.recommendation.targetGradeLevelId ?? null,
		targetStudyProgramId: item.recommendation.targetStudyProgramId ?? null,
		targetHomeroomId: null,
		reason: null,
		condition: null
	};
}
export function decisionUsesDestination(outcome: Outcome | ''): boolean {
	return outcome === 'promote' || outcome === 'repeat' || outcome === 'conditional';
}
export function decisionError(draft: DecisionDraft, item: Item): string {
	if (!draft.outcome) return 'เลือกผลพิจารณาของนักเรียน';
	if (decisionUsesDestination(draft.outcome)) {
		if (!draft.targetGradeLevelId || !draft.targetStudyProgramId)
			return 'เลือกชั้นและแผนการเรียนปลายทาง';
		if (
			(draft.outcome === 'repeat' && draft.targetGradeLevelId !== item.sourceGradeLevelId) ||
			(draft.outcome === 'promote' && draft.targetGradeLevelId === item.sourceGradeLevelId)
		)
			return 'ซ้ำชั้นต้องเป็นชั้นเดิม ส่วนเลื่อนชั้นต้องเป็นชั้นใหม่';
	} else if (draft.targetGradeLevelId || draft.targetStudyProgramId || draft.targetHomeroomId)
		return 'ผลนี้ไม่สร้างข้อมูลนักเรียนปีใหม่ จึงไม่ระบุปลายทาง';
	const reason = draft.reason?.trim() ?? '';
	const condition = draft.condition?.trim() ?? '';
	for (const text of [reason, condition]) {
		if (Array.from(text).length > 1000) return 'เหตุผลและเงื่อนไขยาวได้ไม่เกิน 1000 ตัวอักษร';
		let digits = 0;
		for (const character of text) {
			if (/\p{N}/u.test(character)) {
				if (++digits >= 13) return 'ไม่ใส่เลขประจำตัวประชาชนในเหตุผลหรือเงื่อนไข';
			} else if (/\p{L}/u.test(character)) digits = 0;
		}
	}
	if (draft.outcome === 'conditional' && !condition) return 'ระบุเงื่อนไขที่ต้องติดตาม';
	if (draft.outcome !== 'conditional' && condition) return 'ระบุเงื่อนไขเฉพาะผลแบบมีเงื่อนไข';
	const follows =
		item.recommendation.findings.length === 0 &&
		draft.outcome === item.recommendation.suggestedOutcome &&
		draft.targetGradeLevelId === item.recommendation.targetGradeLevelId &&
		draft.targetStudyProgramId === item.recommendation.targetStudyProgramId;
	if (!follows && !reason) return 'ระบุเหตุผลเมื่อพักรายการหรือเลือกผลต่างจากข้อเสนอ';
	return '';
}
export function canApproveRun(workspace: Workspace): boolean {
	return (
		workspace.run.status === 'reviewed' &&
		workspace.students.length > 0 &&
		workspace.students.some((row) => row.item.status !== 'executed') &&
		workspace.students.every(
			(row) =>
				row.item.status === 'executed' ||
				(row.item.status === 'reviewed' &&
					!!row.item.decision &&
					row.annualResultCurrent &&
					!row.needsRecalculation)
		)
	);
}
export function canExecuteRun(workspace: Workspace): boolean {
	if (
		!workspace.run.approvalId ||
		!['approved', 'executing', 'failed'].includes(workspace.run.status) ||
		workspace.students.length === 0
	)
		return false;
	return workspace.students.every(
		(row) =>
			row.item.status === 'executed' ||
			(['reviewed', 'failed'].includes(row.item.status) &&
				!!row.item.decision &&
				row.annualResultCurrent &&
				!row.needsRecalculation)
	);
}

export const runStatusLabels: Record<Schema['PromotionRunStatus'], string> = {
	draft: 'ยังไม่คำนวณ',
	calculated: 'รอตรวจผล',
	reviewed: 'ตรวจครบแล้ว',
	approved: 'อนุมัติแล้ว',
	executing: 'กำลังดำเนินการ',
	completed: 'ดำเนินการครบแล้ว',
	failed: 'ต้องตรวจหรือลองใหม่'
};
export const outcomeLabels: Record<Outcome, string> = {
	promote: 'เลื่อนชั้น',
	repeat: 'ซ้ำชั้น',
	graduate: 'จบการศึกษา',
	transfer_out: 'ย้ายออก',
	hold: 'พักรายการ',
	conditional: 'มีเงื่อนไข'
};
export const findingLabels: Record<Schema['PromotionRecommendationFinding'], string> = {
	policy_rule_missing: 'ไม่พบเกณฑ์สำหรับชั้นและแผนนี้',
	annual_result_missing: 'ยังไม่มีผลรายปีที่ล็อก',
	annual_result_stale: 'ผลรายปีเปลี่ยนแล้ว',
	reviewed_annual_hold: 'ผลรายปีมีรายการพักรอ',
	insufficient_earned_credits: 'หน่วยกิตที่ผ่านยังไม่ถึงเกณฑ์',
	exceptional_outcomes: 'มีผลการเรียนที่ต้องจัดการ',
	activities_not_passed: 'กิจกรรมยังไม่ผ่าน',
	learner_evaluation_not_passed: 'ผลประเมินยังไม่ถึงเกณฑ์',
	learner_evaluation_missing: 'ผลประเมินยังไม่ครบ'
};
