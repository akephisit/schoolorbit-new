import type {
	CurriculumCreditCriterion,
	CurriculumLevel,
	GradeLevelOption
} from '#lib/api/academic-core.js';
import {
	compareCurriculumCredits,
	curriculumMetric,
	type MetricTotal
} from '../academic/curriculum-structure.ts';

export type CreditCriterionAssessment = {
	status: 'deficit' | 'excess' | 'unknown';
	message: string;
};
export function sameCoveredGrades(saved: readonly string[], current: readonly string[]): boolean {
	return (
		new Set(saved).size === saved.length &&
		saved.length === current.length &&
		saved.every((id) => current.includes(id))
	);
}
export function assessBasicCredits(
	total: MetricTotal,
	criterion: CurriculumCreditCriterion | undefined,
	coveredGradeIds: readonly string[],
	scope: 'annual' | 'level',
	gradeId?: string
): CreditCriterionAssessment | null {
	if (!criterion?.enabled || criterion.scope !== scope) return null;
	if (!sameCoveredGrades(criterion.gradeLevelIds, coveredGradeIds))
		return {
			status: 'unknown',
			message: 'ชั้นที่ครอบคลุมเปลี่ยนไป กรุณาตรวจสอบและบันทึกเกณฑ์อีกครั้งก่อนเปรียบเทียบ'
		};
	if (scope === 'annual' && !criterion.gradeLevelIds.includes(gradeId ?? '')) return null;
	if (validateCreditCriteria([criterion]))
		return {
			status: 'unknown',
			message: 'ข้อมูลเกณฑ์ไม่สมบูรณ์ กรุณาตรวจสอบและบันทึกเกณฑ์อีกครั้ง'
		};
	if (total.partial || total.value === '—')
		return {
			status: 'unknown',
			message:
				'ข้อมูลหน่วยกิตหรือประเภทวิชาบางรายการไม่ครบ จึงยังตรวจเกณฑ์พื้นฐานไม่ได้ ยอดพื้นฐานและเพิ่มเติมอาจไม่เท่ากับยอดรวมจนกว่าจะตรวจประเภทวิชา'
		};
	const comparison = compareCurriculumCredits(total.value, criterion.targetCredits);
	if (!comparison)
		return {
			status: 'unknown',
			message: 'ข้อมูลหน่วยกิตหรือเกณฑ์ไม่สมบูรณ์ จึงยังเปรียบเทียบไม่ได้'
		};
	const scopeLabel = scope === 'annual' ? 'ต่อปี · ทุกภาคเรียน' : 'ทั้งระดับ · ทุกชั้น ทุกภาคเรียน';
	const context = `หน่วยกิตพื้นฐานตามโครงสร้าง ${total.value} / เกณฑ์${criterion.comparison === 'minimum' ? 'อย่างน้อย' : 'เท่ากับ'} ${curriculumMetric(criterion.targetCredits)} ${scopeLabel}`;
	if (comparison.relation < 0)
		return { status: 'deficit', message: `${context} · ขาด ${comparison.difference} หน่วยกิต` };
	if (comparison.relation > 0 && criterion.comparison === 'equal')
		return { status: 'excess', message: `${context} · เกิน ${comparison.difference} หน่วยกิต` };
	return null;
}
export type CreditCriteriaPreset = {
	id: 'lower' | 'upper';
	label: string;
	criterion: CurriculumCreditCriterion;
};
export function creditCriteriaPresets(
	grades: readonly GradeLevelOption[],
	coveredIds: readonly string[]
): CreditCriteriaPreset[] {
	const covered = coveredIds.map((id) => grades.find((grade) => grade.id === id));
	if (!covered.length || covered.some((grade) => !grade || grade.level_type !== 'secondary'))
		return [];
	if (covered.every((grade) => grade && grade.level_order >= 301 && grade.level_order <= 303))
		return [
			{
				id: 'lower',
				label: 'มัธยมศึกษาตอนต้น · พื้นฐาน 22 หน่วยกิตต่อปี',
				criterion: {
					scope: 'annual',
					comparison: 'equal',
					targetCredits: '22.00',
					enabled: true,
					gradeLevelIds: [...coveredIds]
				}
			}
		];
	if (
		covered.length === 3 &&
		[304, 305, 306].every((order) => covered.some((grade) => grade?.level_order === order))
	)
		return [
			{
				id: 'upper',
				label: 'มัธยมศึกษาตอนปลาย ม.4–6 · พื้นฐาน 41 หน่วยกิตทั้งระดับ',
				criterion: {
					scope: 'level',
					comparison: 'equal',
					targetCredits: '41.00',
					enabled: true,
					gradeLevelIds: [...coveredIds]
				}
			}
		];
	return [];
}
export function applyCreditCriteriaPreset(
	criteria: readonly CurriculumCreditCriterion[],
	preset: CurriculumCreditCriterion
): CurriculumCreditCriterion[] {
	return [
		...criteria.filter((criterion) => criterion.scope !== preset.scope),
		{ ...preset, gradeLevelIds: [...preset.gradeLevelIds] }
	];
}
export function validateCreditCriteria(criteria: readonly CurriculumCreditCriterion[]): string {
	if (
		criteria.length > 2 ||
		new Set(criteria.map((criterion) => criterion.scope)).size !== criteria.length
	)
		return 'กำหนดได้หนึ่งเกณฑ์ต่อปีและหนึ่งเกณฑ์ทั้งระดับ';
	if (
		criteria.some(
			(criterion) => !/^(0|[1-9][0-9]{0,3})(?:\.[0-9]{1,2})?$/.test(criterion.targetCredits.trim())
		)
	)
		return 'ระบุหน่วยกิตตั้งแต่ 0 ถึง 9999.99 และทศนิยมไม่เกิน 2 ตำแหน่ง';
	if (
		criteria.some(
			(criterion) =>
				criterion.gradeLevelIds.length === 0 ||
				criterion.gradeLevelIds.length > 500 ||
				new Set(criterion.gradeLevelIds).size !== criterion.gradeLevelIds.length
		)
	)
		return 'เลือกระดับชั้นที่ครอบคลุม 1–500 ชั้นโดยไม่ซ้ำ';
	return '';
}

type CriteriaLevelSnapshot = Pick<
	CurriculumLevel,
	'id' | 'draftId' | 'creditCriteria' | 'creditCriteriaRowVersion'
>;
// A structure/slot response owns its own fields, but can carry an older snapshot
// of independently saved criteria. Published/history and different drafts never merge.
export function preserveNewerCreditCriteria<T extends CriteriaLevelSnapshot>(
	current: CriteriaLevelSnapshot | null,
	incoming: T
): T {
	if (
		!current?.draftId ||
		current.id !== incoming.id ||
		current.draftId !== incoming.draftId ||
		current.creditCriteriaRowVersion <= incoming.creditCriteriaRowVersion
	)
		return incoming;
	return {
		...incoming,
		creditCriteria: current.creditCriteria,
		creditCriteriaRowVersion: current.creditCriteriaRowVersion
	};
}
