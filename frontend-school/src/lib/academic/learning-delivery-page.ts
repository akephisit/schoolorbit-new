import type {
	AcademicTermChangeSet,
	AcademicTermChangeSetSummary,
	DeliveryVersionSummary
} from '#lib/api/learning-delivery.js';

export const LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY = 'schoolorbit:learning-delivery-homerooms';
export const LEARNING_DELIVERY_CHANGE_SETS_DEPENDENCY = 'schoolorbit:learning-delivery-change-sets';
export const LEARNING_DELIVERY_CHANGE_SET_DETAIL_DEPENDENCY =
	'schoolorbit:learning-delivery-change-set-detail';

export type LearningDeliveryRefreshScope = 'local' | 'homerooms';

export type LearningDeliveryRouteContext = {
	academicYearId: string;
	academicTermId: string;
	deliveryVersionId?: string;
	changeSetId?: string;
};

export function readLearningDeliveryRouteContext(url: URL): LearningDeliveryRouteContext | null {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '';
	const academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
	const deliveryVersionId = url.searchParams.get('deliveryVersionId')?.trim() || undefined;
	const changeSetId = url.searchParams.get('changeSetId')?.trim() || undefined;
	if (!academicYearId || !academicTermId) return null;
	return { academicYearId, academicTermId, deliveryVersionId, changeSetId };
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
		targetDeliveryVersionId: detail.targetDeliveryVersionId,
		updatedAt: detail.updatedAt
	};
}

export function selectDeliveryVersion(
	versions: DeliveryVersionSummary[],
	requestedId?: string
): DeliveryVersionSummary | null {
	if (requestedId) return versions.find((version) => version.id === requestedId) ?? null;
	return (
		versions.find((version) => version.status === 'draft') ??
		versions.find((version) => version.status === 'published') ??
		null
	);
}
