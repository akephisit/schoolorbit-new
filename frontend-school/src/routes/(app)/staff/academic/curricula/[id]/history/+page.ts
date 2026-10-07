import type { PageLoad } from './$types';
import { listCurriculumPublications } from '#lib/api/academic-core.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM }
};
export const load: PageLoad = ({ fetch, params }) => ({
	title: 'ประวัติการแก้ไขหลักสูตร',
	editionId: params.id,
	publications: captureRouteLoad(
		listCurriculumPublications(params.id, { requestFetch: fetch }),
		'โหลดประวัติการเผยแพร่ไม่สำเร็จ'
	)
});
