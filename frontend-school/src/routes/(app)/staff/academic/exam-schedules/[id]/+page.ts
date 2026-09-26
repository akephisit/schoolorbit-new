import type { PageLoad } from './$types';
import { listGradeLevelOptions } from '$lib/api/academic-core';
import { getExamScheduleWorkspace } from '$lib/api/examSchedule';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';

const TITLE = 'จัดตารางสอบ';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL
	},
	preview: {
		title: TITLE
	}
};

export const load: PageLoad = ({ fetch, params, url }) => {
	const workspace = captureRouteLoad(
		getExamScheduleWorkspace(params.id, { requestFetch: fetch }),
		'ไม่สามารถโหลดพื้นที่จัดตารางสอบได้'
	);
	return {
		title: TITLE,
		roundId: params.id,
		academicYearId: url.searchParams.get('academicYearId')?.trim() || null,
		academicTermId: url.searchParams.get('academicTermId')?.trim() || null,
		workspace,
		gradeLevels: workspace.then((result) =>
			result.ok
				? captureRouteLoad(
						listGradeLevelOptions(result.data.round.academicYearId, { requestFetch: fetch }),
						'ไม่สามารถโหลดระดับชั้นได้'
					)
				: null
		)
	};
};
