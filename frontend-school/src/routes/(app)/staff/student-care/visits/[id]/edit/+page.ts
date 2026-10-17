import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careManagePermissions } from '#lib/features/student-care/access.js';

import { careVisit, careStaff } from '#lib/api/student-care.js';
import { careToday } from '#lib/features/student-care/presentation.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careManagePermissions }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId') ?? '';
	const date = careToday();
	return {
		term,
		date,
		id: params.id,
		initial: captureRouteLoad(
			readCare(careManagePermissions, async () => {
				const options = { requestFetch: fetch };
				const record = await careVisit(params.id, options);
				const [staff] = await Promise.all([careStaff(options)]);
				return { record, staff, students: [] };
			}),
			'โหลดแบบเยี่ยมบ้านไม่ได้'
		)
	};
};
