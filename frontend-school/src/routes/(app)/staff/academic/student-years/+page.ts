import type { PageLoad } from './$types';
import { STUDENT_YEARS_WORKSPACE_DEPENDENCY } from '$lib/academic-core/foundation-route';
import {
	listHomerooms,
	listPlacementsForAcademicYear,
	listStudentAcademicYears
} from '$lib/api/academic-core';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';
import { loadStudentYearCollections } from '$lib/workspaces/academic-batch';

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
	return {
		title: _meta.menu.title,
		academicYearId,
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
					),
					'โหลดข้อมูลนักเรียนประจำปีไม่สำเร็จ'
				)
			: null
	};
};
