import type {
	AssessmentExamArrangement,
	AssessmentPhaseCode,
	AssessmentPlanSummary
} from '#lib/api/academicAssessments.js';
import type { SpreadsheetSheet, SpreadsheetValue } from '#lib/utils/spreadsheet.js';

export type AssessmentReportKind = 'overview' | 'exam';

export const phaseCodes = ['before_midterm', 'midterm', 'after_midterm', 'final'] as const;

export function phaseFor(plan: AssessmentPlanSummary, code: AssessmentPhaseCode) {
	return plan.phases.find((phase) => phase.phaseCode === code) ?? null;
}

export function phaseLabel(code: AssessmentPhaseCode): string {
	return {
		before_midterm: 'ก่อนกลางภาค',
		midterm: 'กลางภาค',
		after_midterm: 'หลังกลางภาค',
		final: 'ปลายภาค'
	}[code];
}

export function arrangementLabel(arrangement: AssessmentExamArrangement): string {
	return {
		none: 'ไม่จัดสอบ',
		in_timetable: 'ในตาราง',
		outside_timetable: 'นอกตาราง'
	}[arrangement];
}

export function hasScheduledAssessmentExam(plan: AssessmentPlanSummary): boolean {
	return plan.phases.some(
		({ phaseCode, examArrangement }) =>
			(phaseCode === 'midterm' || phaseCode === 'final') &&
			(examArrangement === 'in_timetable' || examArrangement === 'outside_timetable')
	);
}

function reportRow(plan: AssessmentPlanSummary): SpreadsheetValue[] {
	const saved = Boolean(plan.planId);
	const midterm = saved ? phaseFor(plan, 'midterm') : null;
	const final = saved ? phaseFor(plan, 'final') : null;
	return [
		plan.offeringCode,
		plan.offeringName,
		plan.subjectVersionDisplayLabel,
		plan.learningGroupCount,
		plan.assessmentCoordinatorName ?? null,
		saved ? (plan.readiness.ready ? 'พร้อมใช้' : 'ต้องตรวจสอบ') : 'ยังไม่บันทึก',
		...phaseCodes.map((code) => {
			const phase = saved ? phaseFor(plan, code) : null;
			return phase ? Number(phase.maxScore) : null;
		}),
		saved ? Number(plan.readiness.totalScore) : null,
		Number(plan.readiness.expectedTotalScore),
		midterm ? arrangementLabel(midterm.examArrangement) : null,
		midterm?.examDurationMinutes ?? null,
		final ? arrangementLabel(final.examArrangement) : null,
		final?.examDurationMinutes ?? null
	];
}

export function buildAssessmentReport(
	kind: AssessmentReportKind,
	plans: readonly AssessmentPlanSummary[]
): SpreadsheetSheet {
	return {
		name: kind === 'overview' ? 'Overview' : 'Exams',
		rows: [
			[
				'รหัสวิชา',
				'รายวิชา',
				'รุ่นรายวิชา',
				'จำนวนกลุ่มเรียน',
				'ผู้รับผิดชอบ',
				'ความพร้อม',
				...phaseCodes.map(phaseLabel),
				'คะแนนรวม',
				'เกณฑ์คะแนนรวม',
				'รูปแบบสอบกลางภาค',
				'เวลาสอบกลางภาค (นาที)',
				'รูปแบบสอบปลายภาค',
				'เวลาสอบปลายภาค (นาที)'
			],
			...plans.map(reportRow)
		],
		widths: [16, 36, 32, 19, 30, 18, 16, 14, 16, 14, 14, 19, 22, 26, 22, 26]
	};
}
