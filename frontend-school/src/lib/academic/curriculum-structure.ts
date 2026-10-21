import type { CreditCriterionAssessment } from '../academic-core/curriculum-credit-criteria.ts';
import { orderedCurriculumPrograms } from '../academic-core/curriculum-program-order.ts';
import type {
	CurriculumDocumentSection,
	CurriculumStructureRequirement,
	CurriculumStructureWorkspace,
	CurriculumValidationNotice
} from '#lib/api/academic-core.js';

export const curriculumSections: ReadonlyArray<{ id: CurriculumDocumentSection; label: string }> = [
	{ id: 'basic_course', label: 'รายวิชาพื้นฐาน' },
	{ id: 'additional_course', label: 'รายวิชาเพิ่มเติม' },
	{ id: 'student_development', label: 'กิจกรรมพัฒนาผู้เรียน' }
];
export const ALL_GRADES = 'all';
export type MetricTotal = { value: string; partial: boolean };
export type CurriculumTotals = {
	credits: MetricTotal;
	basicCredits: MetricTotal;
	additionalCredits: MetricTotal;
	hours: MetricTotal;
	weeklyPeriods: MetricTotal;
	weeklyHours: MetricTotal;
	weeklySchoolHours: MetricTotal;
};
export type CurriculumMatrixRow = {
	key: string;
	section: CurriculumDocumentSection;
	groupId: string | null;
	groupOrder: number | null;
	label: string;
	cells: Record<string, CurriculumStructureRequirement[]>;
};
export type CurriculumMatrix = ReturnType<typeof buildCurriculumMatrix>;
export type CurriculumValidationNoticeView = CurriculumValidationNotice & { key: string };

export function buildCurriculumValidationNoticeViews(
	notices: readonly CurriculumValidationNotice[]
): CurriculumValidationNoticeView[] {
	const seen = new Set<string>();
	return notices.flatMap((notice) => {
		const key = JSON.stringify([notice.code, notice.catalogVersionId ?? null, notice.message]);
		if (seen.has(key)) return [];
		seen.add(key);
		return [{ ...notice, key }];
	});
}

function decimal(value: string | null | undefined): bigint | null {
	const match = /^(\d+)(?:\.(\d{1,2}))?$/.exec(value?.trim() ?? '');
	return match ? BigInt(match[1]) * 100n + BigInt((match[2] ?? '').padEnd(2, '0')) : null;
}
function format(value: bigint): string {
	return `${value / 100n}.${String(value % 100n).padStart(2, '0')}`;
}
export function curriculumMetric(value: string | null | undefined): string {
	const parsed = decimal(value);
	return parsed === null ? '—' : format(parsed);
}
function sum(values: (string | null | undefined)[]): MetricTotal {
	let total = 0n;
	let known = 0;
	for (const value of values) {
		const parsed = decimal(value);
		if (parsed !== null) {
			total += parsed;
			known++;
		}
	}
	return {
		value: values.length && !known ? '—' : format(total),
		partial: known !== values.length
	};
}
// Curriculum planning counts one teaching period as one instructional hour.
// This preserves catalog numbers and does not convert periods to clock minutes.
export function curriculumWeeklySchoolHours(
	rows: readonly CurriculumStructureRequirement[]
): MetricTotal {
	const contexts = new Set(
		rows.map((row) => JSON.stringify([row.studyProgramId, row.gradeLevel.id, row.termSlotId]))
	);
	if (contexts.size > 1) return { value: '—', partial: false };
	return sum(
		rows.map((row) =>
			(row.resourceKind === 'course' && row.metrics.weeklyUnit === 'period') ||
			(row.resourceKind === 'activity' && row.metrics.weeklyUnit === 'hour')
				? row.metrics.weeklyValue
				: null
		)
	);
}
export type CurriculumSummaryMetrics = {
	basicAssessment?: CreditCriterionAssessment | null;
	credits: MetricTotal;
	basicCredits: MetricTotal;
	additionalCredits: MetricTotal;
	weekly: MetricTotal | null;
	hours: MetricTotal;
};
export type CurriculumSummaryRow = {
	key: string;
	label: string;
	totals: Record<string, CurriculumSummaryMetrics>;
};
export function curriculumSummaryMetrics(
	totals: CurriculumTotals,
	weekly = false
): CurriculumSummaryMetrics {
	return {
		credits: totals.credits,
		basicCredits: totals.basicCredits,
		additionalCredits: totals.additionalCredits,
		weekly: weekly ? totals.weeklySchoolHours : null,
		hours: totals.hours
	};
}
export function curriculumTotals(
	rows: readonly CurriculumStructureRequirement[]
): CurriculumTotals {
	const courses = rows.filter((row) => row.resourceKind === 'course');
	const activities = rows.filter((row) => row.resourceKind === 'activity');
	const unknownCategory = courses.some(
		(row) => row.creditCategory !== 'basic' && row.creditCategory !== 'additional'
	);
	function categoryTotal(category: 'basic' | 'additional'): MetricTotal {
		const total = sum(
			courses.filter((row) => row.creditCategory === category).map((row) => row.metrics.credit)
		);
		return { ...total, partial: total.partial || unknownCategory };
	}
	return {
		weeklySchoolHours: curriculumWeeklySchoolHours(rows),
		credits: sum(courses.map((row) => row.metrics.credit)),
		basicCredits: categoryTotal('basic'),
		additionalCredits: categoryTotal('additional'),
		hours: sum(rows.map((row) => row.metrics.totalHours)),
		weeklyPeriods: sum(
			courses.map((row) => (row.metrics.weeklyUnit === 'period' ? row.metrics.weeklyValue : null))
		),
		weeklyHours: sum(
			activities.map((row) => (row.metrics.weeklyUnit === 'hour' ? row.metrics.weeklyValue : null))
		)
	};
}
export function compareCurriculumCredits(
	actual: string,
	target: string
): { relation: number; difference: string } | null {
	const left = decimal(actual),
		right = decimal(target);
	if (left === null || right === null) return null;
	const delta = left - right;
	return {
		relation: delta < 0n ? -1 : delta > 0n ? 1 : 0,
		difference: format(delta < 0n ? -delta : delta)
	};
}
export function matrixCellKey(programId: string, termId: string): string {
	return JSON.stringify([programId, termId]);
}
function nameKey(name: string): string {
	return name.trim().replace(/\s+/g, ' ').toLocaleLowerCase('th');
}
function familyKey(name: string): string {
	return nameKey(name)
		.replace(/\s*[0-9๐-๙]+\s*$/, '')
		.trim();
}
function scope(row: CurriculumStructureRequirement): string {
	return JSON.stringify([row.section, row.subjectGroupId ?? null, row.resourceKind]);
}
function compareRequirements(
	a: CurriculumStructureRequirement,
	b: CurriculumStructureRequirement
): number {
	return (
		a.displayOrder - b.displayOrder ||
		a.code.localeCompare(b.code, 'th', { numeric: true }) ||
		a.id.localeCompare(b.id)
	);
}

// Matching is presentation only. Stable identities win; name fallbacks merge only
// unambiguous, disjoint columns in the same section and authoritative subject group.
function align(requirements: CurriculumStructureRequirement[]): CurriculumMatrixRow[] {
	const identities = new Map<string, CurriculumStructureRequirement[]>();
	for (const requirement of [...requirements].sort(compareRequirements)) {
		const key = JSON.stringify([scope(requirement), requirement.catalogId]);
		const bucket = identities.get(key) ?? [];
		bucket.push(requirement);
		identities.set(key, bucket);
	}
	let clusters = [...identities.values()];
	for (const getName of [nameKey, familyKey]) {
		const buckets = new Map<string, CurriculumStructureRequirement[][]>();
		for (const cluster of clusters) {
			const names = new Set(cluster.map((row) => getName(row.name)));
			// An identity with different names still stays together, but cannot
			// arbitrarily supply one of those names for a weaker match.
			const key =
				names.size === 1 && [...names][0]
					? JSON.stringify([scope(cluster[0]), [...names][0]])
					: null;
			const bucketKey = key ?? JSON.stringify(['identity', cluster[0].id]);
			const bucket = buckets.get(bucketKey) ?? [];
			bucket.push(cluster);
			buckets.set(bucketKey, bucket);
		}
		clusters = [...buckets.values()].flatMap((bucket) => {
			const columns = new Set<string>();
			for (const cluster of bucket) {
				for (const row of cluster) {
					const column = matrixCellKey(row.studyProgramId, row.termSlotId);
					if (columns.has(column)) return bucket;
					columns.add(column);
				}
			}
			return [bucket.flat()];
		});
	}
	return clusters
		.map((cluster) => {
			const sorted = [...cluster].sort(compareRequirements);
			const first = sorted[0];
			const cells: CurriculumMatrixRow['cells'] = {};
			for (const row of sorted) {
				const key = matrixCellKey(row.studyProgramId, row.termSlotId);
				(cells[key] ??= []).push(row);
			}
			return {
				key: JSON.stringify(sorted.map((row) => row.id).sort()),
				section: first.section,
				groupId: first.subjectGroupId ?? null,
				groupOrder: first.subjectGroupOrder ?? null,
				label: familyKey(first.name) || first.name,
				cells
			};
		})
		.sort(
			(a, b) =>
				curriculumSections.findIndex((section) => section.id === a.section) -
					curriculumSections.findIndex((section) => section.id === b.section) ||
				Number(a.groupId === null) - Number(b.groupId === null) ||
				(a.groupOrder ?? Number.MAX_SAFE_INTEGER) - (b.groupOrder ?? Number.MAX_SAFE_INTEGER) ||
				(a.groupId ?? '').localeCompare(b.groupId ?? '') ||
				a.label.localeCompare(b.label, 'th', { numeric: true }) ||
				a.key.localeCompare(b.key)
		);
}

export function buildCurriculumMatrix(
	workspace: CurriculumStructureWorkspace,
	gradeLevelId: string,
	termSlotIds: readonly string[] = workspace.termSlots.map((term) => term.id),
	studyProgramId?: string
) {
	const terms = [...workspace.termSlots]
		.filter((term) => termSlotIds.includes(term.id))
		.sort((a, b) => a.sequence - b.sequence || a.id.localeCompare(b.id));
	const programs = orderedCurriculumPrograms(workspace.programs).filter(
		(program) => !studyProgramId || program.id === studyProgramId
	);
	const grades = [...workspace.gradeLevels]
		.filter((grade) => gradeLevelId === ALL_GRADES || grade.id === gradeLevelId)
		.sort((a, b) => a.level_order - b.level_order);
	const selectedRows = workspace.requirements.filter(
		(row) =>
			programs.some((program) => program.id === row.studyProgramId) &&
			terms.some((term) => term.id === row.termSlotId)
	);
	return {
		programs,
		terms,
		grades: grades.map((grade) => {
			// Align globally so switching views never changes the relative row order.
			const allRows = workspace.requirements.filter(
				(row) => row.gradeLevel.id === grade.id && terms.some((term) => term.id === row.termSlotId)
			);
			const rows = align(allRows).filter((row) =>
				programs.some((program) =>
					terms.some((term) => row.cells[matrixCellKey(program.id, term.id)]?.length)
				)
			);
			return {
				grade,
				sections: curriculumSections.map((section) => ({
					...section,
					rows: rows.filter((row) => row.section === section.id)
				}))
			};
		}),
		summaries: programs.map((program) => ({
			program,
			selected: curriculumTotals(
				selectedRows.filter(
					(row) =>
						row.studyProgramId === program.id &&
						grades.some((grade) => grade.id === row.gradeLevel.id)
				)
			),
			level: curriculumTotals(
				workspace.requirements.filter((row) => row.studyProgramId === program.id)
			),
			weekly: [...workspace.gradeLevels]
				.sort((a, b) => a.level_order - b.level_order)
				.flatMap((grade) =>
					[...workspace.termSlots]
						.sort((a, b) => a.sequence - b.sequence)
						.map((term) => ({
							grade,
							term,
							totals: curriculumTotals(
								workspace.requirements.filter(
									(row) =>
										row.studyProgramId === program.id &&
										row.gradeLevel.id === grade.id &&
										row.termSlotId === term.id
								)
							)
						}))
				)
		}))
	};
}

export function buildProgramComparison(
	workspace: CurriculumStructureWorkspace,
	gradeLevelId: string,
	termSlotIds?: readonly string[]
) {
	return buildCurriculumMatrix(workspace, gradeLevelId, termSlotIds);
}
export function buildCurriculumDocument(
	workspace: CurriculumStructureWorkspace,
	studyProgramId: string,
	gradeLevelId: string,
	termSlotIds?: readonly string[]
) {
	return buildCurriculumMatrix(workspace, gradeLevelId, termSlotIds, studyProgramId);
}
