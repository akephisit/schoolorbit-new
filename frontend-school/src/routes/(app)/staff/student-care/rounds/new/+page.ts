import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare } from '#lib/features/student-care/access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { careInstruments } from '#lib/api/student-care.js';
import { careToday } from '#lib/features/student-care/presentation.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	return {
		term: url.searchParams.get('academicTermId'),
		date: careToday(),
		initial: captureRouteLoad(
			readCare([PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL], () =>
				careInstruments({ limit: 100 }, { requestFetch: fetch })
			),
			'โหลดรุ่นแบบประเมินไม่ได้'
		)
	};
};
