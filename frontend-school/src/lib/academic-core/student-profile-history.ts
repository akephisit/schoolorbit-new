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
