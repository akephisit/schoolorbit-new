import type { PageLoad } from './$types';
import { getCurriculum, listCurriculumLevels } from '#lib/api/academic-core.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM }
};
export const load: PageLoad = ({ fetch, params }) => ({
	title: 'ฉบับหลักสูตร',
	editionId: params.id,
	edition: captureRouteLoad(
		getCurriculum(params.id, { requestFetch: fetch }),
		'โหลดฉบับหลักสูตรไม่สำเร็จ'
	),
	levels: captureRouteLoad(
		listCurriculumLevels(params.id, { requestFetch: fetch }),
		'โหลดระดับการศึกษาไม่สำเร็จ'
	)
});
