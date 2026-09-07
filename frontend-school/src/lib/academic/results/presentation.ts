import type {
	AcademicResultReadiness,
	CourseResultPreparationWorkspace,
	EffectiveResultKind
} from '../../api/academicResults';

export type CourseOutcomeSelection =
	CourseResultPreparationWorkspace['students'][number]['selection'];
export type ResultBlocker = AcademicResultReadiness['activities'][number]['blockers'][number];

export const COURSE_OUTCOME_OPTIONS: ReadonlyArray<{
	value: CourseOutcomeSelection;
	label: string;
}> = [
	{ value: 'derived', label: 'ตามคะแนน' },
	{ value: 'manual_zero', label: '0' },
	{ value: 'incomplete', label: 'ร' },
	{ value: 'insufficient_attendance', label: 'มส' }
];

export function courseOutcomeSelectionLabel(value: CourseOutcomeSelection): string {
	return COURSE_OUTCOME_OPTIONS.find((option) => option.value === value)?.label ?? value;
}

export function activityOutcomeLabel(value: 'pass' | 'fail' | null | undefined): string {
	if (value === 'pass') return 'ผ';
	if (value === 'fail') return 'มผ';
	return 'ยังไม่ประเมิน';
}

export function resultKindLabel(kind: EffectiveResultKind): string {
	switch (kind) {
		case 'course':
			return 'ผลการเรียนรายวิชา';
		case 'activity':
			return 'ผลกิจกรรม';
		case 'learner_evaluation':
			return 'ผลประเมินผู้เรียน';
	}
}

export function resultBlockerLabel(blocker: ResultBlocker): string {
	switch (blocker.code) {
		case 'missing_phase_confirmation':
			return 'ยังยืนยันคะแนนไม่ครบทุกช่วง';
		case 'stale_phase_confirmation':
			return 'คะแนนเปลี่ยนหลังยืนยัน กรุณายืนยันช่วงคะแนนใหม่';
		case 'invalid_assessment_plan':
			return 'โครงสร้างคะแนนยังไม่พร้อม';
		case 'invalid_grading_policy':
			return 'เกณฑ์ตัดผลการเรียนยังไม่พร้อม';
		case 'missing_activity_outcome':
			return 'ผลกิจกรรมยังขาดนักเรียนบางคน';
		case 'missing_primary_teacher':
			return 'ยังไม่มีครูหลัก';
		case 'missing_group_confirmation':
			return 'ครูหลักยังไม่ยืนยันผล';
		case 'stale_group_confirmation':
			return 'ข้อมูลเปลี่ยนหลังยืนยัน กรุณายืนยันใหม่';
		case 'already_locked':
			return 'ล็อกผลแล้ว';
	}
}

export function sortAssignedFirst<
	T extends {
		assigned: boolean;
		code?: string;
		name?: string;
		groupName?: string;
	}
>(items: readonly T[]): T[] {
	return [...items].sort((left, right) => {
		if (left.assigned !== right.assigned) return left.assigned ? -1 : 1;
		const leftLabel = `${left.code ?? ''} ${left.name ?? ''} ${left.groupName ?? ''}`;
		const rightLabel = `${right.code ?? ''} ${right.name ?? ''} ${right.groupName ?? ''}`;
		return leftLabel.localeCompare(rightLabel, 'th');
	});
}

export function isSubjectReady(groups: readonly { ready: boolean }[]): boolean {
	return groups.length > 0 && groups.every((group) => group.ready);
}

export function formatEffectiveResultValue(
	value:
		| {
				kind: 'course';
				outcome: 'numeric' | 'incomplete' | 'insufficient_attendance';
				numericGrade?: string | null;
		  }
		| { kind: 'activity'; outcome: 'pass' | 'fail' }
		| { kind: 'learner_evaluation'; qualityLevel: number }
): string {
	if (value.kind === 'activity') return activityOutcomeLabel(value.outcome);
	if (value.kind === 'learner_evaluation') {
		return `${value.qualityLevel} · ${learnerQualityLabel(value.qualityLevel)}`;
	}
	if (value.outcome === 'incomplete') return 'ร';
	if (value.outcome === 'insufficient_attendance') return 'มส';
	return value.numericGrade ?? '0';
}

function learnerQualityLabel(level: number): string {
	return ['ไม่ผ่าน', 'ผ่าน', 'ดี', 'ดีเยี่ยม'][level] ?? `${level}`;
}
