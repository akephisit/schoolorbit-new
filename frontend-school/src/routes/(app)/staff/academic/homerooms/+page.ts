import type { PageLoad } from './$types';
import { HOMEROOMS_WORKSPACE_DEPENDENCY } from '#lib/academic-core/foundation-route.js';
import {
	listGradeLevelOptions,
	listHomeroomAdvisorsForAcademicYear,
	listHomerooms,
	listStudyProgramOptionsForAcademicYear
} from '#lib/api/academic-core.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { loadHomeroomCollections } from '#lib/workspaces/academic-batch.js';

export const _meta = {
	academicContext: 'year_required',
	menu: {
		title: 'ห้องประจำชั้น',
		icon: 'School',
		group: 'academic_registry',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.HOMEROOM
	}
};

export const load: PageLoad = ({ depends, fetch, url }) => {
	depends(HOMEROOMS_WORKSPACE_DEPENDENCY);
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	return {
		title: _meta.menu.title,
		academicYearId,
		workspace: academicYearId
			? captureRouteLoad(
					loadHomeroomCollections(
						{
							listHomerooms,
							listHomeroomAdvisorsForAcademicYear,
							listGradeLevelOptions,
							listStudyProgramOptionsForAcademicYear
						},
						academicYearId,
						{ requestFetch: fetch }
					),
					'โหลดห้องประจำชั้นไม่สำเร็จ'
				)
			: null
	};
};
