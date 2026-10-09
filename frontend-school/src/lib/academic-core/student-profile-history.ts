import {
	listStudentAcademicYears,
	listHomeroomPlacements,
	listHomerooms
} from '#lib/api/academic-core.js';
import type { ApiRequestOptions } from '#lib/api/client.js';

export async function loadStudentProfileHistory(
	studentId: string,
	academicYearId: string,
	options: ApiRequestOptions = {}
) {
	const records = listStudentAcademicYears(academicYearId, { studentId }, options);
	const [years, homerooms] = await Promise.all([records, listHomerooms(academicYearId, options)]);
	const studentYear = years[0] ?? null;
	const placements = studentYear ? await listHomeroomPlacements(studentYear.id, options) : [];
	return { studentYear, homerooms, placements };
}

export function studentProfileReturnTo(value: string | null, academicYearId: string): string {
	const fallback = `/staff/students?academicYearId=${encodeURIComponent(academicYearId)}`;
	if (!value || !value.startsWith('/') || value.startsWith('//')) return fallback;
	const url = new URL(value, 'https://school.example');
	if (
		url.origin !== 'https://school.example' ||
		!/^\/staff\/(?:students|academic\/student-years|academic\/homerooms\/[\w-]+\/students)$/u.test(
			url.pathname
		)
	)
		return fallback;
	if (url.searchParams.get('academicYearId') !== academicYearId) return fallback;
	return url.pathname + url.search;
}
