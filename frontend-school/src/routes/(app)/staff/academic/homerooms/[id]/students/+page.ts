import type { PageLoad } from './$types';
import { getHomeroomRoster } from '#lib/api/homeroom-roster.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'year_required' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.STUDENT_ACADEMIC_YEAR }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('schoolorbit:homeroom-roster');
	const academicYearId = url.searchParams.get('academicYearId');
	return {
		title: 'นักเรียนในห้อง',
		roomId: params.id,
		academicYearId,
		roster: academicYearId
			? captureRouteLoad(
					getHomeroomRoster(params.id, { requestFetch: fetch }).then((roster) => {
						if (roster.homeroom.academicYearId !== academicYearId)
							throw new Error('ห้องนี้อยู่ในปีการศึกษาอื่น กรุณากลับไปเลือกห้องในปีที่เลือก');
						return roster;
					}),
					'โหลดนักเรียนในห้องไม่สำเร็จ'
				)
			: null
	};
};
