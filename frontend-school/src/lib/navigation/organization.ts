const path = '/staff/organization';
const contextParameters = new Set(['unit', 'tab', 'group', 'section', 'q', 'type']);

export function organizationDetailHref(unitId: string, context: Pick<URL, 'search'>): string {
	const parameters = new URLSearchParams();
	for (const [key, value] of new URLSearchParams(context.search)) {
		if (contextParameters.has(key)) parameters.set(key, value);
	}
	parameters.set('unit', unitId);
	parameters.set('tab', 'administration');
	return `${path}/${unitId}?${new URLSearchParams({ returnTo: `${path}?${parameters}` })}`;
}

export function organizationReturnHref(url: Pick<URL, 'search'>): string {
	const destination = new URLSearchParams(url.search).get('returnTo');
	if (!destination || !/^\/staff\/organization(?:\?|$)/.test(destination)) return path;
	const parsed = new URL(destination, 'https://schoolorbit.invalid');
	if (parsed.pathname !== path || parsed.hash) return path;
	for (const key of parsed.searchParams.keys()) {
		if (!contextParameters.has(key)) return path;
	}
	return `${parsed.pathname}${parsed.search}`;
}
