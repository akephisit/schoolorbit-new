import { readCurriculumAlignmentContext } from '#lib/academic-core/curriculum-detail-route.js';
import { getHomeroomDeliveryWorkspace } from '#lib/api/learning-delivery.js';
import type { PageLoad } from './$types';
import { getCurriculum, getCurriculumStructureWorkspace } from '#lib/api/academic-core.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM }
};
export const load: PageLoad = ({ fetch, params, url }) => {
	const alignmentContext = readCurriculumAlignmentContext(url);
	return {
		title: 'แผนการเรียน',
		editionId: params.id,
		levelId: params.levelId,
		studyProgramId: url.searchParams.get('studyProgramId') ?? '',
		alignmentContext,
		alignment: alignmentContext
			? captureRouteLoad(
					getHomeroomDeliveryWorkspace(
						alignmentContext.academicYearId,
						alignmentContext.academicTermId,
						{ deliveryVersionId: alignmentContext.deliveryVersionId, requestFetch: fetch }
					),
					'โหลดข้อมูลเทียบการเปิดสอนไม่สำเร็จ'
				)
			: null,
		edition: captureRouteLoad(
			getCurriculum(params.id, { requestFetch: fetch }),
			'โหลดฉบับหลักสูตรไม่สำเร็จ'
		),
		structure: captureRouteLoad(
			getCurriculumStructureWorkspace(params.levelId, { requestFetch: fetch }).then((workspace) => {
				if (workspace.level.editionId !== params.id)
					throw new Error('ระดับการศึกษาไม่อยู่ในฉบับที่เลือก');
				return workspace;
			}),
			'โหลดแผนการเรียนไม่สำเร็จ'
		)
	};
};
