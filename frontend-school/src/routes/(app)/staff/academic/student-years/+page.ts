import type { PageLoad } from './$types';
import { STUDENT_YEARS_WORKSPACE_DEPENDENCY } from '#lib/academic-core/foundation-route.js';
import {
	listHomerooms,
	listPlacementsForAcademicYear,
	listStudentAcademicYears,
	getStudentAcademicYear,
	listHomeroomPlacements
} from '#lib/api/academic-core.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { loadStudentYearCollections } from '#lib/workspaces/academic-batch.js';

export const _meta = {
	academicContext: 'year_required',
	menu: {
		title: 'นักเรียนประจำปี',
		icon: 'UsersRound',
		group: 'academic_registry',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STUDENT_ACADEMIC_YEAR
	}
};

export const load: PageLoad = ({ depends, fetch, url }) => {
	depends(STUDENT_YEARS_WORKSPACE_DEPENDENCY);
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const studentYearId = url.searchParams.get('studentYearId')?.trim() || null;
	return {
		title: _meta.menu.title,
		academicYearId,
		studentYearId,
		workspace: academicYearId
			? captureRouteLoad(
					loadStudentYearCollections(
						{
							listStudentAcademicYears: (yearId, options) =>
								listStudentAcademicYears(yearId, {}, options),
							listPlacementsForAcademicYear,
							listHomerooms
						},
						academicYearId,
						{ requestFetch: fetch }
					).then(async (collections) => {
						if (!studentYearId) return collections;
						const existing = collections.studentYears.find((record) => record.id === studentYearId);
						const [record, placements] = await Promise.all([
							existing ?? getStudentAcademicYear(studentYearId, { requestFetch: fetch }),
							listHomeroomPlacements(studentYearId, { requestFetch: fetch })
						]);
						if (record.academicYearId !== academicYearId)
							throw new Error('ข้อมูลนักเรียนอยู่ในปีการศึกษาอื่น กรุณาเลือกปีให้ตรงกัน');
						if (!existing) collections.studentYears = [...collections.studentYears, record];
						collections.placementsByStudentYearId.set(studentYearId, placements);
						return collections;
					}),
					'โหลดข้อมูลนักเรียนประจำปีไม่สำเร็จ'
				)
			: null
	};
};
