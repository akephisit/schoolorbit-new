import type { SubjectVersion } from '#lib/api/academic-core.js';

export type CatalogVersionDraft = {
	name: string;
	secondaryName: string;
	exactValue: string;
	totalValue: string;
	standardPeriodsPerWeek: string;
	effectiveFrom: string;
	effectiveUntil: string;
	gradeLevelIds: string[];
	classification: string;
	description?: string | null;
	termCode?: string | null;
};

export function subjectVersionSource(
	displayVersion: SubjectVersion | null | undefined,
	versions: SubjectVersion[]
): SubjectVersion | null {
	if (displayVersion?.status === 'published') return displayVersion;
	return (
		versions
			.filter((item) => item.status === 'published')
			.sort((left, right) => right.versionNo - left.versionNo)[0] ?? null
	);
}

export function createCatalogVersionDraft(source?: SubjectVersion | null): CatalogVersionDraft {
	return {
		name: source?.nameTh ?? '',
		secondaryName: source?.nameEn ?? '',
		exactValue: source?.credit ?? '1.00',
		totalValue: source ? (source.hoursPerSemester?.toString() ?? '') : '20.00',
		standardPeriodsPerWeek: source ? (source.periodsPerWeek?.toString() ?? '') : '1',
		effectiveFrom:
			source?.effectiveFrom ??
			new Intl.DateTimeFormat('en-CA', {
				timeZone: 'Asia/Bangkok',
				year: 'numeric',
				month: '2-digit',
				day: '2-digit'
			}).format(new Date()),
		effectiveUntil: source?.effectiveUntil ?? '',
		gradeLevelIds: [...(source?.gradeLevelIds ?? [])],
		classification: source?.subjectType ?? '',
		description: source?.description ?? null,
		termCode: source?.termCode ?? null
	};
}
