import type {
	AcademicTermChangeSet,
	AcademicTermChangeSetSummary
} from '$lib/api/learning-delivery';

export const LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY = 'schoolorbit:learning-delivery-homerooms';
export const LEARNING_DELIVERY_CHANGE_SETS_DEPENDENCY = 'schoolorbit:learning-delivery-change-sets';
export const LEARNING_DELIVERY_CHANGE_SET_DETAIL_DEPENDENCY =
	'schoolorbit:learning-delivery-change-set-detail';

export type LearningDeliveryRefreshScope = 'local' | 'homerooms';

export type LearningDeliveryRouteContext = {
	academicYearId: string;
	academicTermId: string;
	timetableVersionId?: string;
	changeSetId?: string;
};

export function readLearningDeliveryRouteContext(url: URL): LearningDeliveryRouteContext | null {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '';
	const academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
	const timetableVersionId = url.searchParams.get('timetableVersionId')?.trim() || undefined;
	const changeSetId = url.searchParams.get('changeSetId')?.trim() || undefined;
	if (!academicYearId || !academicTermId) return null;
	return { academicYearId, academicTermId, timetableVersionId, changeSetId };
}

export function selectAcademicTermChangeSetSummary(
	summaries: AcademicTermChangeSetSummary[],
	requestedId?: string
): AcademicTermChangeSetSummary | null {
	return (
		summaries.find((summary) => summary.id === requestedId) ??
		summaries.find((summary) => summary.status === 'draft') ??
		summaries[0] ??
		null
	);
}

export function summarizeAcademicTermChangeSet(
	detail: AcademicTermChangeSet
): AcademicTermChangeSetSummary {
	return {
		id: detail.id,
		academicTermId: detail.academicTermId,
		academicYearId: detail.academicYearId,
		effectiveFrom: detail.effectiveFrom,
		reason: detail.reason,
		status: detail.status,
		targetTimetableVersionId: detail.targetTimetableVersionId,
		updatedAt: detail.updatedAt
	};
}
