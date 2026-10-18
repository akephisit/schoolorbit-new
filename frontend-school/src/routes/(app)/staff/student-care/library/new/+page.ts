import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare } from '#lib/features/student-care/access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { careInstrument } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const id = url.searchParams.get('fromVersionId');
	return {
		term: url.searchParams.get('academicTermId'),
		id,
		initial: id
			? captureRouteLoad(
					readCare([PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL], () =>
						careInstrument(id, { requestFetch: fetch })
					),
					'โหลดแบบต้นฉบับไม่ได้'
				)
			: null
	};
};
