const overviewPath = '/staff/academic/delivery';
const navigationOrigin = 'https://schoolorbit.invalid';
const overviewParameters = new Set([
	'academicYearId',
	'academicTermId',
	'deliveryVersionId',
	'changeSetId',
	'teacherChangeItemId',
	'view',
	'kind',
	'offeringSearch',
	'subjectGroupId',
	'homeroomSearch',
	'readiness',
	'expandedRooms'
]);

export type DeliveryAcademicContext = {
	academicYearId: string;
	academicTermId: string;
	deliveryVersionId?: string | null;
};
export type DeliveryOverviewState = {
	view: 'homerooms' | 'offerings';
	kind: 'all' | 'course' | 'activity';
	offeringSearch: string;
	subjectGroupId: string;
	homeroomSearch: string;
	readiness: 'all' | 'ready' | 'attention';
	expandedRooms: string[] | null;
};

export function readDeliveryOverviewState(url: URL): DeliveryOverviewState {
	const query = url.searchParams;
	const kind = query.get('kind');
	const readiness = query.get('readiness');
	return {
		view: query.get('view') === 'offerings' ? 'offerings' : 'homerooms',
		kind: kind === 'course' || kind === 'activity' ? kind : 'all',
		offeringSearch: query.get('offeringSearch') ?? '',
		subjectGroupId: query.get('subjectGroupId') || 'all',
		homeroomSearch: query.get('homeroomSearch') ?? '',
		readiness: readiness === 'ready' || readiness === 'attention' ? readiness : 'all',
		expandedRooms: query.has('expandedRooms')
			? [...new Set((query.get('expandedRooms') ?? '').split(',').filter(Boolean))]
			: null
	};
}

export function deliveryOverviewStateHref(url: URL, patch: Partial<DeliveryOverviewState>): string {
	const query = new URLSearchParams(url.search);
	for (const [key, value] of Object.entries(patch)) {
		if (key === 'expandedRooms') {
			if (value === null) query.delete(key);
			else if (Array.isArray(value)) query.set(key, value.join(','));
		} else if (value === '' || value === 'all' || (key === 'view' && value === 'homerooms')) {
			query.delete(key);
		} else if (typeof value === 'string') query.set(key, value);
	}
	return `${overviewPath}${query.size ? `?${query}` : ''}`;
}

export function deliveryOverviewHref(context: DeliveryAcademicContext): string {
	const query = new URLSearchParams({
		academicYearId: context.academicYearId,
		academicTermId: context.academicTermId
	});
	if (context.deliveryVersionId) query.set('deliveryVersionId', context.deliveryVersionId);
	return `${overviewPath}?${query}`;
}

export function deliveryOverviewReturnHref(url: URL, context: DeliveryAcademicContext): string {
	const target = new URL(deliveryOverviewHref(context), navigationOrigin);
	for (const [key, value] of url.searchParams) {
		if (
			overviewParameters.has(key) &&
			!['academicYearId', 'academicTermId', 'deliveryVersionId'].includes(key)
		)
			target.searchParams.set(key, value);
	}
	return target.pathname + target.search;
}

function safeOverviewReturn(value: string | null, context: DeliveryAcademicContext): string | null {
	if (!value || value.length > 8000 || !value.startsWith('/') || value.startsWith('//'))
		return null;
	let target: URL;
	try {
		target = new URL(value, navigationOrigin);
	} catch {
		return null;
	}
	if (target.origin !== navigationOrigin || target.pathname !== overviewPath || target.hash)
		return null;
	for (const name of target.searchParams.keys()) {
		if (!overviewParameters.has(name) || target.searchParams.getAll(name).length !== 1) return null;
	}
	if (
		target.searchParams.get('academicYearId') !== context.academicYearId ||
		target.searchParams.get('academicTermId') !== context.academicTermId
	)
		return null;
	return target.pathname + target.search;
}

export function deliveryReturnHref(url: URL, context: DeliveryAcademicContext): string {
	return (
		safeOverviewReturn(url.searchParams.get('returnTo'), context) ?? deliveryOverviewHref(context)
	);
}

export function deliveryOfferingHref(
	offeringId: string,
	context: DeliveryAcademicContext,
	returnTo?: string
): string {
	const query = new URLSearchParams({
		academicYearId: context.academicYearId,
		academicTermId: context.academicTermId
	});
	if (context.deliveryVersionId) query.set('deliveryVersionId', context.deliveryVersionId);
	const destination = safeOverviewReturn(returnTo ?? null, context);
	if (destination) query.set('returnTo', destination);
	return `${overviewPath}/${encodeURIComponent(offeringId)}?${query}`;
}

export function repairedDeliveryOfferingHref(
	url: URL,
	context: DeliveryAcademicContext
): string | null {
	if (
		url.searchParams.get('academicYearId') === context.academicYearId &&
		url.searchParams.get('academicTermId') === context.academicTermId
	)
		return null;
	const repaired = new URL(url);
	repaired.searchParams.set('academicYearId', context.academicYearId);
	repaired.searchParams.set('academicTermId', context.academicTermId);
	if (repaired.searchParams.get('deliveryVersionId') !== context.deliveryVersionId)
		repaired.searchParams.delete('deliveryVersionId');
	if (
		repaired.searchParams.has('returnTo') &&
		!safeOverviewReturn(repaired.searchParams.get('returnTo'), context)
	)
		repaired.searchParams.delete('returnTo');
	return repaired.pathname + repaired.search;
}
