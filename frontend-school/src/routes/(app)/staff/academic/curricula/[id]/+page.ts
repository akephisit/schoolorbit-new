import type { PageLoad } from './$types';
import { readCurriculumAlignmentContext } from '#lib/academic-core/curriculum-detail-route.js';
import {
	getCurriculum,
	getCurriculumStructureWorkspace,
	listCurriculumVersions,
	type CurriculumStructureWorkspace
} from '#lib/api/academic-core.js';
import { getHomeroomDeliveryWorkspace } from '#lib/api/learning-delivery.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_CURRICULUM
	}
};

export const load: PageLoad = ({ fetch, params, url }) => {
	const curriculumId = params.id;
	const requestedVersionId = url.searchParams.get('versionId')?.trim() || null;
	const alignmentContext = readCurriculumAlignmentContext(url);
	const curriculum = captureRouteLoad(
		getCurriculum(curriculumId, { requestFetch: fetch }),
		'โหลดรายละเอียดหลักสูตรไม่สำเร็จ'
	);
	const versions = captureRouteLoad(
		listCurriculumVersions(curriculumId, { requestFetch: fetch }),
		'โหลดรายการรุ่นหลักสูตรไม่สำเร็จ'
	);
	const loadStructure = (
		versionId: string
	): Promise<RouteLoadResult<CurriculumStructureWorkspace | null>> =>
		captureRouteLoad(
			getCurriculumStructureWorkspace(versionId, { requestFetch: fetch }).then((workspace) => {
				if (workspace.curriculumVersion.curriculumId !== curriculumId)
					throw new Error('รุ่นหลักสูตรไม่อยู่ในหลักสูตรที่เลือก');
				return workspace;
			}),
			'โหลดแผนการเรียนไม่สำเร็จ'
		);
	const structure = requestedVersionId
		? loadStructure(requestedVersionId)
		: versions.then((result) =>
				result.ok && result.data[0]
					? loadStructure(result.data[0].version.id)
					: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>)
			);
	const alignment = alignmentContext
		? captureRouteLoad(
				getHomeroomDeliveryWorkspace(
					alignmentContext.academicYearId,
					alignmentContext.academicTermId,
					{
						timetableVersionId: alignmentContext.timetableVersionId,
						requestFetch: fetch
					}
				),
				'โหลดข้อมูลเทียบการเปิดสอนไม่สำเร็จ'
			)
		: null;
	return {
		title: 'รายละเอียดหลักสูตร',
		curriculumId,
		requestedVersionId,
		alignmentContext,
		curriculum,
		versions,
		structure,
		alignment
	};
};
