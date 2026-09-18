export const LEARNING_DELIVERY_PAGE_DEPENDENCY = 'schoolorbit:learning-delivery-page';

export type LearningDeliveryRefreshScope = 'local' | 'page';

export type LearningDeliveryRouteContext = {
	academicYearId: string;
	academicTermId: string;
	timetableVersionId?: string;
};

export function readLearningDeliveryRouteContext(url: URL): LearningDeliveryRouteContext | null {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '';
	const academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
	const timetableVersionId = url.searchParams.get('timetableVersionId')?.trim() || undefined;
	if (!academicYearId || !academicTermId) return null;
	return { academicYearId, academicTermId, timetableVersionId };
}
