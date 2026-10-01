const directoryPath = '/staff/manage';
const listParameters = new Set([
	'search',
	'page',
	'status',
	'page_size',
	'role_id',
	'organization_unit_id',
	'job_position_id',
	'academic_rank',
	'education_level',
	'subject_group_id'
]);

export function staffReturnHref(url: URL): string {
	const destination = url.searchParams.get('returnTo');
	if (!destination || !/^\/staff\/manage(?:\?|$)/.test(destination)) return directoryPath;
	const parsed = new URL(destination, 'https://schoolorbit.invalid');
	if (parsed.pathname !== directoryPath || parsed.hash) return directoryPath;
	for (const name of parsed.searchParams.keys()) {
		if (!listParameters.has(name)) return directoryPath;
	}
	return `${parsed.pathname}${parsed.search}`;
}

export function withStaffReturn(path: string, returnHref: string): string {
	if (returnHref === directoryPath) return path;
	const separator = path.includes('?') ? '&' : '?';
	return `${path}${separator}${new URLSearchParams({ returnTo: returnHref })}`;
}
