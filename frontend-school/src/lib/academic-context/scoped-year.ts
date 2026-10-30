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

	if (requested !== null) return { academicYearId: null, replaceUrl: null };
	const selected = options.years.find((year) => year.id === options.defaultAcademicYearId)?.id;
	return selected
		? { academicYearId: selected, replaceUrl: urlWithAcademicYear(url, selected) }
		: { academicYearId: null, replaceUrl: null };
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
	if (requested !== null && !terms.some((term) => term.id === requested))
		return { academicYearId: '', academicTermId: '', replaceUrl: null };
	const academicTermId =
		terms.find((term) => term.id === requested)?.id ??
		(termRequired
			? (terms.find((term) => term.id === options.defaultAcademicTermId)?.id ?? '')
			: '');
	const next = new URL(year.replaceUrl ?? url);
	if (academicTermId) next.searchParams.set('academicTermId', academicTermId);
	else next.searchParams.delete('academicTermId');
	return { academicYearId, academicTermId, replaceUrl: next.href === url.href ? null : next };
}

export function resolveScopedAcademicTerm(options: AcademicContextOptionsResponse, url: URL) {
	const requested = url.searchParams.get('academicTermId');
	const yearId = url.searchParams.get('academicYearId');
	const termId = requested ?? options.defaultAcademicTermId;
	const term = options.terms.find((candidate) => candidate.id === termId);
	if (!term || !options.years.some((year) => year.id === term.academicYearId)) return undefined;
	if (yearId !== null && term.academicYearId !== yearId) return undefined;
	if (requested === null && term.academicYearId !== options.defaultAcademicYearId) return undefined;
	return term;
}
