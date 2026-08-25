import type { AcademicContextOptionsResponse } from '$lib/api/academic-context';

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
