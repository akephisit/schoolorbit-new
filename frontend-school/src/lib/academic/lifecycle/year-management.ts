import type {
	RolloverDecision,
	YearRolloverPreview,
	YearRolloverStudent
} from '#lib/api/academic-year-management.js';
import type { components } from '#lib/api/generated/school-api.js';

export type RolloverOutcome = Exclude<RolloverDecision['outcome'], 'conditional'>;
export type RolloverDraft = Omit<RolloverDecision, 'outcome'> & {
	outcome: RolloverOutcome | '';
	retainedDecision?: RolloverDecision;
};
export const rolloverOutcomeLabels: Record<RolloverOutcome, string> = {
	promote: 'เลื่อนชั้น',
	repeat: 'ซ้ำชั้น',
	graduate: 'จบการศึกษา',
	transfer_out: 'ย้ายออก',
	hold: 'พักไว้'
};
export function academicStatusLabel(
	status: components['schemas']['AcademicYearStatus'] | components['schemas']['AcademicTermStatus']
): string {
	if (status === 'planning' || status === 'ready') return 'เตรียมการ';
	if (status === 'active' || status === 'closing') return 'ใช้งาน';
	return 'ปิดแล้ว';
}
export function gradeLabel(grade: { levelType: string; year: number } | undefined): string {
	if (!grade) return 'ไม่พบข้อมูลชั้น';
	return `${grade.levelType === 'primary' ? 'ป.' : grade.levelType === 'secondary' ? 'ม.' : grade.levelType === 'kindergarten' ? 'อ.' : grade.levelType}${grade.year}`;
}
export function usesDestination(outcome: RolloverDraft['outcome']): boolean {
	return outcome === 'promote' || outcome === 'repeat';
}
export function initialRolloverDraft(student: YearRolloverStudent): RolloverDraft {
	const decision = student.decision ?? student.suggestedDecision;
	if (student.existingTargetStudentYearId && student.suggestedDecision)
		return { outcome: '', retainedDecision: { ...student.suggestedDecision } };
	return decision && decision.outcome !== 'conditional'
		? { ...decision, outcome: decision.outcome }
		: {
				outcome: '',
				targetGradeLevelId: null,
				targetStudyProgramId: null,
				targetHomeroomId: null,
				reason: null,
				condition: null
			};
}
export function changeOutcome(
	draft: RolloverDraft,
	outcome: RolloverDraft['outcome'],
	student: YearRolloverStudent
): RolloverDraft {
	if (draft.retainedDecision) return draft;
	return {
		...draft,
		outcome,
		condition: null,
		targetGradeLevelId: usesDestination(outcome)
			? outcome === 'repeat'
				? student.sourceGradeLevelId
				: draft.targetGradeLevelId
			: null,
		targetStudyProgramId: usesDestination(outcome) ? draft.targetStudyProgramId : null,
		targetHomeroomId: usesDestination(outcome) ? draft.targetHomeroomId : null
	};
}
export function rosterDecisions(
	students: YearRolloverStudent[],
	drafts: Record<string, RolloverDraft>
): components['schemas']['YearRolloverStudentInput'][] {
	return students.flatMap((student) => {
		const draft = drafts[student.studentAcademicYearId];
		if (draft?.retainedDecision && student.existingTargetStudentYearId)
			return [
				{
					studentAcademicYearId: student.studentAcademicYearId,
					decision: { ...draft.retainedDecision }
				}
			];
		if (!draft?.outcome) return [];
		return [
			{
				studentAcademicYearId: student.studentAcademicYearId,
				decision: {
					...draft,
					outcome: draft.outcome,
					reason: draft.reason?.trim() || null,
					condition: null
				}
			}
		];
	});
}
export function rosterDraftError(
	preview: YearRolloverPreview,
	drafts: Record<string, RolloverDraft>
): string {
	for (const student of preview.students) {
		const draft = drafts[student.studentAcademicYearId];
		if (
			draft?.retainedDecision &&
			student.existingTargetStudentYearId &&
			JSON.stringify(draft.retainedDecision) === JSON.stringify(student.suggestedDecision)
		)
			continue;
		if (!draft?.outcome) return `เลือกผลของ ${student.studentName}`;
		if (usesDestination(draft.outcome)) {
			if (!draft.targetGradeLevelId || !draft.targetStudyProgramId)
				return `เลือกชั้นและแผนการเรียนของ ${student.studentName}`;
			if (draft.outcome === 'repeat' && draft.targetGradeLevelId !== student.sourceGradeLevelId)
				return `ซ้ำชั้นต้องเป็นชั้นเดิม: ${student.studentName}`;
			if (draft.outcome === 'promote' && draft.targetGradeLevelId === student.sourceGradeLevelId)
				return `เลื่อนชั้นต้องเลือกชั้นใหม่: ${student.studentName}`;
			if (
				!preview.programs.some(
					(program) =>
						program.id === draft.targetStudyProgramId &&
						program.gradeLevelIds.includes(draft.targetGradeLevelId!)
				)
			)
				return `แผนการเรียนไม่รองรับชั้นของ ${student.studentName}`;
			if (
				draft.targetHomeroomId &&
				!preview.homerooms.some(
					(room) =>
						room.id === draft.targetHomeroomId &&
						room.gradeLevelId === draft.targetGradeLevelId &&
						room.studyProgramId === draft.targetStudyProgramId
				)
			)
				return `ห้องไม่ตรงกับชั้นและแผนการเรียนของ ${student.studentName}`;
		}
		if (Array.from(draft.reason ?? '').length > 1000) return 'เหตุผลยาวได้ไม่เกิน 1,000 ตัวอักษร';
	}
	return '';
}
export function sourceRoomKey(student: YearRolloverStudent): string {
	return `${student.sourceGradeLevelId}:${student.sourceStudyProgramId}:${student.sourceHomeroomId ?? ''}`;
}
