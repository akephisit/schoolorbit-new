const registryPath = '/staff/students';
const origin = 'https://schoolorbit.invalid';
const registryParameters = ['search', 'registrationStatus', 'accountStatus', 'page'];

export interface StudentRegisterContext {
	returnTo: string;
	academicYearId: string | null;
	source: 'registry' | 'student-years' | 'homeroom';
}

export function studentRegistryHref(url: URL): string {
	const query = new URLSearchParams();
	for (const key of registryParameters) {
		const value = url.searchParams.get(key);
		if (value) query.set(key, value);
	}
	return `${registryPath}${query.size ? `?${query}` : ''}`;
}

/** A year alone never changes a permanent profile into an annual view. */
export function studentRegisterContext(url: URL): StudentRegisterContext {
	const fallback: StudentRegisterContext = {
		returnTo: registryPath,
		academicYearId: null,
		source: 'registry'
	};
	const value = url.searchParams.get('returnTo');
	if (!value || !value.startsWith('/') || value.startsWith('//')) return fallback;
	let destination: URL;
	try {
		destination = new URL(value, origin);
	} catch {
		return fallback;
	}
	if (destination.origin !== origin || destination.hash) return fallback;
	if (destination.pathname === registryPath)
		return { ...fallback, returnTo: studentRegistryHref(destination) };
	const source =
		destination.pathname === '/staff/academic/student-years'
			? 'student-years'
			: /^\/staff\/academic\/homerooms\/[\w-]+\/students$/u.test(destination.pathname)
				? 'homeroom'
				: null;
	const academicYearId = url.searchParams.get('academicYearId');
	if (
		!source ||
		!academicYearId ||
		destination.searchParams.get('academicYearId') !== academicYearId
	)
		return fallback;
	if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(academicYearId))
		return fallback;
	const allowedParameters = new Set(['academicYearId', 'studentYearId']);
	for (const key of destination.searchParams.keys()) {
		if (!allowedParameters.has(key)) return fallback;
	}
	return { returnTo: destination.pathname + destination.search, academicYearId, source };
}

export function withStudentRegisterContext(path: string, context: StudentRegisterContext): string {
	const url = new URL(path, origin);
	if (context.academicYearId) url.searchParams.set('academicYearId', context.academicYearId);
	if (context.returnTo !== registryPath) url.searchParams.set('returnTo', context.returnTo);
	return url.pathname + url.search;
}
