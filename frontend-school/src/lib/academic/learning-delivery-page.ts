export const LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY = 'schoolorbit:learning-delivery-homerooms';
export const LEARNING_DELIVERY_OVERVIEW_DEPENDENCY = 'schoolorbit:learning-delivery-overview';
export type LearningDeliveryRefreshScope = 'local' | 'homerooms';
export type DeliveryOfferingAddKind = 'basic' | 'additional' | 'activity';
export type LearningDeliveryRouteContext = { academicYearId: string; academicTermId: string };
export function currentDeliveryDate(now = new Date()): string {
	return new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(now);
}
export function defaultRosterTrackingDate(now = new Date()): string {
	return currentDeliveryDate(now);
}
export function readLearningDeliveryRouteContext(url: URL): LearningDeliveryRouteContext | null {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '',
		academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
	return academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
}
