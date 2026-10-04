import type { AcademicContextOptionsResponse } from '#lib/api/academic-context.js';

export type ScopedAcademicYearResolution = {
	academicYearId: string | null;
	replaceUrl: URL | null;
};

export function urlWithAcademicYear(url: URL, academicYearId: string): URL {
	const next = new URL(url);
	next.searchParams.set('academicYearId', academicYearId);
	next.searchParams.delete('academicTermId');
	return next;
}

export function resolveScopedAcademicYearUrl(
	options: AcademicContextOptionsResponse,
	url: URL
): ScopedAcademicYearResolution {
	if (options.years.length === 0) return { academicYearId: null, replaceUrl: null };

	const requested = url.searchParams.get('academicYearId');
	const valid = options.years.find((year) => year.id === requested)?.id;
	if (valid) return { academicYearId: valid, replaceUrl: null };

	const selected =
		options.years.find((year) => year.id === options.activeAcademicYearId)?.id ??
		options.years[0].id;
	return { academicYearId: selected, replaceUrl: urlWithAcademicYear(url, selected) };
}

export function resolveScopedAcademicContextUrl(
	options: AcademicContextOptionsResponse,
	url: URL,
	termRequired: boolean
): { academicYearId: string; academicTermId: string; replaceUrl: URL | null } {
	const year = resolveScopedAcademicYearUrl(options, url);
	const academicYearId = year.academicYearId ?? '';
	if (!academicYearId) return { academicYearId: '', academicTermId: '', replaceUrl: null };
	const terms = options.terms.filter((term) => term.academicYearId === academicYearId);
	const requested = url.searchParams.get('academicTermId');
	const academicTermId =
		terms.find((term) => term.id === requested)?.id ??
		(termRequired
			? (terms.find((term) => term.id === options.activeAcademicTermId)?.id ?? terms[0]?.id ?? '')
			: '');
	const next = new URL(year.replaceUrl ?? url);
	if (academicTermId) next.searchParams.set('academicTermId', academicTermId);
	else next.searchParams.delete('academicTermId');
	return { academicYearId, academicTermId, replaceUrl: next.href === url.href ? null : next };
}
